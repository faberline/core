use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use openapi_codegen::{
    generate, generate_with_auth, FileBearerScheme, GenOptions, GeneratedOutput, GenerationAuth,
    Lang, ManagedAuth, PythonTarget, TargetProfile,
};

const SPEC: &str = r##"{
  "openapi": "3.1.0",
  "info": { "title": "Managed contract", "version": "1.0.0" },
  "paths": {
    "/ping": {
      "get": {
        "operationId": "ping",
        "responses": { "204": { "description": "ok" } }
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
        "openapi-codegen-managed-py-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create managed Python temp directory");
    path
}

fn managed(ca: &Path) -> ManagedAuth {
    ManagedAuth::new(ca, [FileBearerScheme::Https]).expect("valid managed contract")
}

fn materialize(output: &GeneratedOutput, path: &Path) {
    output.write_to_dir(path).expect("write generated output");
    assert!(path.join(".openapi-codegen.json").is_file());
}

fn assert_success(output: Output, what: &str) {
    assert!(
        output.status.success(),
        "{what}: exit {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn managed_py_output_differs_only_in_client_py() {
    let root = temp_dir("differs");
    let auth = GenerationAuth::new(None, Some(managed(&root.join("ca.pem"))))
        .expect("valid auth selection");
    let managed_out = generate_with_auth(SPEC, &opts(), &auth).expect("generate managed Python");
    let plain = generate(SPEC, &opts()).expect("generate plain Python");

    let managed_paths: Vec<&str> = managed_out
        .files
        .iter()
        .map(|f| f.rel_path.as_str())
        .collect();
    let plain_paths: Vec<&str> = plain.files.iter().map(|f| f.rel_path.as_str()).collect();
    assert_eq!(managed_paths, plain_paths);

    for (managed_file, plain_file) in managed_out.files.iter().zip(plain.files.iter()) {
        if managed_file.rel_path == "client.py" {
            assert_ne!(managed_file.contents, plain_file.contents);
            assert!(managed_file.contents.contains("_MANAGED_TOKEN_PATH"));
        } else {
            assert_eq!(
                managed_file.contents, plain_file.contents,
                "{} must not change",
                managed_file.rel_path
            );
        }
    }
    fs::remove_dir_all(root).expect("remove managed Python temp directory");
}

#[test]
fn no_auth_py_output_is_unchanged() {
    let with_default =
        generate_with_auth(SPEC, &opts(), &GenerationAuth::default()).expect("generate default");
    let plain = generate(SPEC, &opts()).expect("generate plain Python");
    assert_eq!(with_default.files.len(), plain.files.len());
    for (a, b) in with_default.files.iter().zip(plain.files.iter()) {
        assert_eq!(a.rel_path, b.rel_path);
        assert_eq!(a.contents, b.contents, "{} must not change", a.rel_path);
    }
}

#[test]
fn generated_managed_python_client_fails_before_any_transport_call_and_never_drops_the_token() {
    let root = temp_dir("contract");
    let ca = root.join("ca.pem");
    let auth = GenerationAuth::new(None, Some(managed(&ca))).expect("valid auth selection");
    let target_opts = opts().with_target(TargetProfile::Python(PythonTarget::Py311));
    let output = generate_with_auth(SPEC, &target_opts, &auth).expect("generate managed client");
    materialize(&output, &root.join("generated_api"));

    let script = r##"import asyncio
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
ca = pathlib.Path(sys.argv[2])
sys.path.insert(0, str(root))
from generated_api import AsyncClient, Client

TOKEN_PATH = "/var/run/secrets/kubernetes.io/serviceaccount/token"
real_read_text = pathlib.Path.read_text
state = {"token": "tok-one\n"}

def fake_read_text(self, *args, **kwargs):
    if str(self) == TOKEN_PATH:
        if state["token"] is None:
            raise FileNotFoundError(TOKEN_PATH)
        return state["token"]
    return real_read_text(self, *args, **kwargs)

pathlib.Path.read_text = fake_read_text

class Response:
    def __init__(self, status): self.status = status
    def raise_for_status(self):
        if self.status >= 400: raise RuntimeError(f"HTTP {self.status}")
    def json(self): return None

class Transport:
    def __init__(self, status=204): self.calls = []; self.status = status
    def request(self, method, url, **kwargs):
        self.calls.append((method, url, dict(kwargs["headers"])))
        return Response(self.status)

class AsyncTransport(Transport):
    async def request(self, method, url, **kwargs):
        return Transport.request(self, method, url, **kwargs)

def auth_values(headers):
    return [value for name, value in headers.items() if name.casefold() == "authorization"]

base = "https://svc.example.internal"
ca.write_text("-----BEGIN CERTIFICATE-----\nx\n-----END CERTIFICATE-----\n", encoding="utf-8")

# http and other schemes are refused before any transport call
for bad in ("http://svc.example.internal", "ftp://svc.example.internal"):
    trap = Transport()
    try:
        Client(bad, client=trap, ca_path=str(ca)).ping()
        raise AssertionError("not refused: " + bad)
    except ValueError as error:
        assert "https" in str(error)
    assert not trap.calls

# the token is re-read on every request and replaces a caller header
transport = Transport()
client = Client(base, client=transport, ca_path=str(ca), default_headers={"authorization": "Bearer caller"})
client.ping()
state["token"] = "tok-two\n"
client.ping()
assert [auth_values(call[2]) for call in transport.calls] == [["Bearer tok-one"], ["Bearer tok-two"]]

# a missing or empty token fails before transport
for token in (None, "", "  \n"):
    state["token"] = token
    trap = Transport()
    try:
        Client(base, client=trap, ca_path=str(ca)).ping()
        raise AssertionError("token failure expected")
    except ValueError as error:
        assert "managed token" in str(error)
    assert not trap.calls

# a missing or empty CA fails before transport
state["token"] = "tok-one"
for contents in (None, "", "\n"):
    if ca.exists(): ca.unlink()
    if contents is not None: ca.write_text(contents, encoding="utf-8")
    trap = Transport()
    try:
        Client(base, client=trap, ca_path=str(ca)).ping()
        raise AssertionError("CA failure expected")
    except ValueError as error:
        assert "managed CA bundle" in str(error)
    assert not trap.calls

# a 401 does not cause an anonymous retry: exactly one call, with the token
ca.write_text("-----BEGIN CERTIFICATE-----\nx\n-----END CERTIFICATE-----\n", encoding="utf-8")
denied = Transport(status=401)
try:
    Client(base, client=denied, ca_path=str(ca)).ping()
    raise AssertionError("401 expected")
except RuntimeError:
    pass
assert len(denied.calls) == 1 and auth_values(denied.calls[0][2]) == ["Bearer tok-one"]

async def async_contract():
    state["token"] = "async-one"
    transport = AsyncTransport()
    client = AsyncClient(base, client=transport, ca_path=str(ca))
    await client.ping()
    state["token"] = None
    try:
        await client.ping()
        raise AssertionError("token failure expected")
    except ValueError as error:
        assert "managed token" in str(error)
    assert len(transport.calls) == 1 and auth_values(transport.calls[0][2]) == ["Bearer async-one"]
    try:
        await AsyncClient("http://svc.example.internal", client=AsyncTransport(), ca_path=str(ca)).ping()
        raise AssertionError("not refused")
    except ValueError:
        pass

asyncio.run(async_contract())
"##;
    let script_path = root.join("contract.py");
    fs::write(&script_path, script).expect("write managed Python contract");
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
        .arg(&ca)
        .env("UV_CACHE_DIR", root.join("uv-cache"))
        .env("PYTHONPYCACHEPREFIX", root.join("pycache"))
        .output()
        .expect("spawn uv for the managed Python contract");
    assert_success(run, "execute generated managed Python client");
    fs::remove_dir_all(root).expect("remove managed Python temp directory");
}
