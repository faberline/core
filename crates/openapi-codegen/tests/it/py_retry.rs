use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use openapi_codegen::{generate, GenOptions, Lang, PythonTarget, TargetProfile};

const SPEC: &str = r##"{
  "openapi": "3.1.0",
  "info": { "title": "Retry contract", "version": "1.0.0" },
  "paths": {
    "/read": {
      "get": {
        "operationId": "readThing",
        "x-retry-class": "safe-read",
        "responses": {
          "200": {
            "description": "ok",
            "content": {
              "application/json": {
                "schema": { "type": "object", "properties": { "ok": { "type": "boolean" } } }
              }
            }
          }
        }
      }
    },
    "/write": {
      "post": {
        "operationId": "writeThing",
        "x-retry-class": "idempotency-key",
        "responses": {
          "200": {
            "description": "ok",
            "content": {
              "application/json": {
                "schema": { "type": "object", "properties": { "ok": { "type": "boolean" } } }
              }
            }
          }
        }
      }
    },
    "/mutate": {
      "delete": {
        "operationId": "mutateThing",
        "x-retry-class": "non-retryable",
        "responses": {
          "200": {
            "description": "ok",
            "content": {
              "application/json": {
                "schema": { "type": "object", "properties": { "ok": { "type": "boolean" } } }
              }
            }
          }
        }
      }
    },
    "/plain": {
      "get": {
        "operationId": "plainThing",
        "responses": {
          "200": {
            "description": "ok",
            "content": {
              "application/json": {
                "schema": { "type": "object", "properties": { "ok": { "type": "boolean" } } }
              }
            }
          }
        }
      }
    }
  }
}"##;

fn opts() -> GenOptions {
    GenOptions::new(Lang::Py, PathBuf::new(), PathBuf::new(), "createClient").with_emit_hooks(false)
}

fn temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "openapi-codegen-retry-py-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create retry Python temp directory");
    path
}

#[test]
fn generated_python_client_retry() {
    if Command::new("uv").arg("--version").output().is_err() {
        eprintln!("skipping: uv is not installed");
        return;
    }

    let root = temp_dir("contract");
    let target_opts = opts().with_target(TargetProfile::Python(PythonTarget::Py311));
    let output = generate(SPEC, &target_opts).expect("generate Python client");
    output
        .write_to_dir(&root.join("generated_api"))
        .expect("write generated output");

    let script = r##"import asyncio, pathlib, signal, sys, time, types
signal.alarm(30)
root = pathlib.Path(sys.argv[1])
sys.path.insert(0, str(root))
from generated_api import AsyncClient, Client
from generated_api.client import CancelledError, DeadlineExceededError, RetryPolicy

class StatusError(Exception):
    def __init__(self, status):
        super().__init__("HTTP %d" % status)
        self.response = types.SimpleNamespace(status_code=status)

class Resp:
    def __init__(self, status):
        self.status = status
    def raise_for_status(self):
        if self.status >= 400:
            raise StatusError(self.status)
    def json(self):
        return {"ok": True}

class Script:
    """Plays one outcome per call: an int is a status, an exception is raised. The last outcome repeats."""
    def __init__(self, *plan):
        self.plan = list(plan)
        self.calls = 0
        self.headers = []
    def _next(self, headers):
        outcome = self.plan[min(self.calls, len(self.plan) - 1)]
        self.calls += 1
        self.headers.append(dict(headers))
        if isinstance(outcome, BaseException):
            raise outcome
        return Resp(outcome)
    def request(self, method, url, *, params, headers, json=None, data=None, content=None, timeout=None):
        return self._next(headers)

class AsyncScript(Script):
    async def request(self, method, url, *, params, headers, json=None, data=None, content=None, timeout=None):
        return self._next(headers)

FAST = RetryPolicy(max_attempts=3, delay=lambda attempt: 0)
MODES = ("sync", "async")

def new_script(mode, *plan):
    return Script(*plan) if mode == "sync" else AsyncScript(*plan)

def new_client(mode, script, **kw):
    if mode == "sync":
        return Client("http://x.example.internal", client=script, **kw)
    return AsyncClient("http://x.example.internal", client=script, **kw)

def call(mode, client, name, **kw):
    method = getattr(client, name)
    if mode == "sync":
        return method(**kw)
    return asyncio.run(method(**kw))

def expect(exc_type, fn):
    try:
        fn()
    except exc_type as exc:
        return exc
    raise AssertionError("expected %s" % exc_type.__name__)

def expect_status(mode, client, name, status):
    exc = expect(StatusError, lambda: call(mode, client, name))
    assert exc.response.status_code == status, (mode, exc.response.status_code)

OK = {"ok": True}

for mode in MODES:
    # (a) a safe read retries past two 503s and recovers
    s = new_script(mode, 503, 503, 200)
    assert call(mode, new_client(mode, s, retry=FAST), "read_thing") == OK, mode
    assert s.calls == 3, (mode, s.calls)

    # (b) the cap holds: three attempts, then the 503 reaches the caller
    s = new_script(mode, 503)
    expect_status(mode, new_client(mode, s, retry=FAST), "read_thing", 503)
    assert s.calls == 3, (mode, s.calls)

    # (c) a non-retryable operation is sent once
    s = new_script(mode, 503)
    expect_status(mode, new_client(mode, s, retry=FAST), "mutate_thing", 503)
    assert s.calls == 1, (mode, s.calls)

    # (d) an unclassified operation is sent once
    s = new_script(mode, 503)
    expect_status(mode, new_client(mode, s, retry=FAST), "plain_thing", 503)
    assert s.calls == 1, (mode, s.calls)

    # (e) idempotency-key without a key is sent once
    s = new_script(mode, 503)
    expect_status(mode, new_client(mode, s, retry=FAST), "write_thing", 503)
    assert s.calls == 1, (mode, s.calls)

    # (e2) idempotency-key with a key retries and sends the key each time
    s = new_script(mode, 503, 200)
    client = new_client(mode, s, retry=FAST, default_headers={"Idempotency-Key": "k1"})
    assert call(mode, client, "write_thing") == OK, mode
    assert s.calls == 2, (mode, s.calls)
    assert s.headers[0]["Idempotency-Key"] == "k1", (mode, s.headers)
    assert s.headers[1]["Idempotency-Key"] == "k1", (mode, s.headers)

    # (e3) an empty key does not count
    s = new_script(mode, 503)
    client = new_client(mode, s, retry=FAST, default_headers={"Idempotency-Key": ""})
    expect_status(mode, client, "write_thing", 503)
    assert s.calls == 1, (mode, s.calls)

    # (f) no policy means no retry
    s = new_script(mode, 503)
    expect_status(mode, new_client(mode, s), "read_thing", 503)
    assert s.calls == 1, (mode, s.calls)

    # (g) a status that is not retryable is sent once
    s = new_script(mode, 404)
    expect_status(mode, new_client(mode, s, retry=FAST), "read_thing", 404)
    assert s.calls == 1, (mode, s.calls)

    # (h) a connection reset is retried and recovers
    s = new_script(mode, ConnectionResetError(), 200)
    assert call(mode, new_client(mode, s, retry=FAST), "read_thing") == OK, mode
    assert s.calls == 2, (mode, s.calls)

    # (i) a deadline is not retried
    s = new_script(mode, TimeoutError())
    expect(DeadlineExceededError, lambda: call(mode, new_client(mode, s, retry=FAST), "read_thing", timeout=0.5))
    assert s.calls == 1, (mode, s.calls)

    # (j) the delay callable receives the attempt numbers 1 and 2
    delays = []
    policy = RetryPolicy(max_attempts=3, delay=lambda attempt: delays.append(attempt) or 0)
    s = new_script(mode, 503, 503, 200)
    assert call(mode, new_client(mode, s, retry=policy), "read_thing") == OK, mode
    assert delays == [1, 2], (mode, delays)

    # (k) max_attempts of one sends once
    s = new_script(mode, 503)
    one = RetryPolicy(max_attempts=1, delay=lambda attempt: 0)
    expect_status(mode, new_client(mode, s, retry=one), "read_thing", 503)
    assert s.calls == 1, (mode, s.calls)

# (l) async only: a cancel during the wait before a retry ends the call
async def cancel_case():
    ev = asyncio.Event()
    asyncio.get_running_loop().call_later(0.05, ev.set)
    s = AsyncScript(503)
    policy = RetryPolicy(max_attempts=3, delay=lambda attempt: 0.3)
    client = AsyncClient("http://x.example.internal", client=s, retry=policy)
    try:
        await client.read_thing(cancel_event=ev)
    except CancelledError:
        return s.calls
    raise AssertionError("expected CancelledError")

assert asyncio.run(cancel_case()) == 1

# (m) the default policy
assert RetryPolicy().max_attempts == 3
assert abs(RetryPolicy().delay(2) - 0.2) < 1e-9

print("ok")
"##;
    let script_path = root.join("contract.py");
    fs::write(&script_path, script).expect("write retry Python contract");
    let run = Command::new("uv")
        .arg("run")
        .arg("--quiet")
        .arg("--no-project")
        .arg("--python")
        .arg("3.13")
        .arg("--with")
        .arg("pydantic==2.12.5")
        .arg("python")
        .arg(&script_path)
        .arg(&root)
        .env("UV_CACHE_DIR", root.join("uv-cache"))
        .env("PYTHONPYCACHEPREFIX", root.join("pycache"))
        .output()
        .expect("spawn uv for the retry Python contract");
    assert!(
        run.status.success(),
        "execute generated retry Python client: exit {:?}\nstdout:\n{}\nstderr:\n{}",
        run.status.code(),
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout).trim(),
        "ok",
        "retry Python contract stdout"
    );
    fs::remove_dir_all(root).expect("remove retry Python temp directory");
}
