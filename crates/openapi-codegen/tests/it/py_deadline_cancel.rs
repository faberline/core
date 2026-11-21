use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use openapi_codegen::{generate, GenOptions, Lang, PythonTarget, TargetProfile};

const SPEC: &str = r##"{
  "openapi": "3.1.0",
  "info": { "title": "Deadline contract", "version": "1.0.0" },
  "paths": {
    "/ping": {
      "get": {
        "operationId": "ping",
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
        "openapi-codegen-deadline-py-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create deadline Python temp directory");
    path
}

#[test]
fn generated_python_client_deadline_and_cancel() {
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

    let script = r##"import asyncio
import pathlib
import signal
import socket
import sys
import threading
import time

signal.alarm(30)
root = pathlib.Path(sys.argv[1])
sys.path.insert(0, str(root))
from generated_api import AsyncClient, Client
from generated_api.client import CancelledError, DeadlineExceededError

# A server that accepts TCP connections and never answers.
conns = []
srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
srv.bind(("127.0.0.1", 0))
srv.listen(16)
slow = "http://127.0.0.1:%d" % srv.getsockname()[1]

def accept_loop():
    while True:
        try:
            conn, _ = srv.accept()
        except OSError:
            return
        conns.append(conn)

threading.Thread(target=accept_loop, daemon=True).start()

def wait_conns(expected):
    deadline = time.monotonic() + 3
    while len(conns) < expected and time.monotonic() < deadline:
        time.sleep(0.01)
    time.sleep(0.1)
    assert len(conns) == expected, "connections: %d, expected %d" % (len(conns), expected)

class FastResponse:
    def raise_for_status(self):
        return None
    def json(self):
        return {"ok": True}

class FastClient:
    def __init__(self):
        self.timeouts = []
    def request(self, method, url, *, params, headers, json=None, data=None, content=None, timeout=None):
        self.timeouts.append(timeout)
        return FastResponse()

class AsyncFastClient(FastClient):
    async def request(self, method, url, *, params, headers, json=None, data=None, content=None, timeout=None):
        self.timeouts.append(timeout)
        return FastResponse()

class RaisesTimeout:
    def request(self, method, url, **kwargs):
        raise TimeoutError()

class Sleeps:
    async def request(self, method, url, **kwargs):
        await asyncio.sleep(10)

async def main():
    loop = asyncio.get_running_loop()

    # (a) a deadline raises DeadlineExceededError, sync and async, one connection each
    try:
        Client(slow).ping(timeout=0.05)
        raise AssertionError("sync deadline not raised")
    except DeadlineExceededError:
        pass
    wait_conns(1)
    try:
        await AsyncClient(slow).ping(timeout=0.05)
        raise AssertionError("async deadline not raised")
    except DeadlineExceededError:
        pass
    wait_conns(2)

    # (b) cancel_event set during the call raises CancelledError, one more connection
    ev = asyncio.Event()
    loop.call_later(0.05, ev.set)
    try:
        await AsyncClient(slow).ping(cancel_event=ev)
        raise AssertionError("cancel_event not honored")
    except CancelledError:
        pass
    wait_conns(3)

    # a cancel_event that is already set sends no request
    done = asyncio.Event()
    done.set()
    try:
        await AsyncClient(slow).ping(cancel_event=done)
        raise AssertionError("preset cancel_event not honored")
    except CancelledError:
        pass
    wait_conns(3)

    # (c) cancelling the asyncio task stays asyncio.CancelledError
    task = asyncio.ensure_future(AsyncClient(slow).ping())
    await asyncio.sleep(0.05)
    task.cancel()
    try:
        await task
        raise AssertionError("task cancel not raised")
    except asyncio.CancelledError:
        pass
    wait_conns(4)

    # (e) no options: the injected transport gets timeout None; an explicit value is passed on
    fast = FastClient()
    Client("http://fast.example.internal", client=fast).ping()
    Client("http://fast.example.internal", client=fast).ping(timeout=1.5)
    assert fast.timeouts == [None, 1.5], fast.timeouts
    afast = AsyncFastClient()
    await AsyncClient("http://fast.example.internal", client=afast).ping()
    await AsyncClient("http://fast.example.internal", client=afast).ping(timeout=1.5)
    assert afast.timeouts == [None, 1.5], afast.timeouts

    # (f) a transport timeout maps to DeadlineExceededError; _call enforces a deadline a transport ignores
    try:
        Client("http://fast.example.internal", client=RaisesTimeout()).ping()
        raise AssertionError("transport timeout not mapped")
    except DeadlineExceededError:
        pass
    started = time.monotonic()
    try:
        await AsyncClient("http://fast.example.internal", client=Sleeps()).ping(timeout=0.05)
        raise AssertionError("ignored deadline not enforced")
    except DeadlineExceededError:
        pass
    assert time.monotonic() - started < 1.0

asyncio.run(main())
srv.close()
print("ok")
"##;
    let script_path = root.join("contract.py");
    fs::write(&script_path, script).expect("write deadline Python contract");
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
        .expect("spawn uv for the deadline Python contract");
    assert!(
        run.status.success(),
        "execute generated deadline Python client: exit {:?}\nstdout:\n{}\nstderr:\n{}",
        run.status.code(),
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    fs::remove_dir_all(root).expect("remove deadline Python temp directory");
}
