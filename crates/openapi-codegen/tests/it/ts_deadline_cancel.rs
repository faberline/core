use std::fs;
use std::io::Read;
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use openapi_codegen::{generate, GenOptions, HttpClient, Lang};

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

/// `__EXTRA__` is `axios: fakeAxios,` for the axios backend and empty for fetch.
const CONTRACT: &str = r##"import { createClient } from "./generated/client.ts";
import { CancelledError, DeadlineExceededError } from "./generated/runtime.ts";

const hang = setTimeout(() => {
  console.error("hang");
  process.exit(2);
}, 10000);
hang.unref();

// Long enough that the request is written before the deadline on a loaded
// runner. A shorter deadline can expire first, and then the server counts
// one request too few.
const DEADLINE_MS = 300;

const [slowPort, fastPort] = process.argv.slice(2);
const slow = `http://127.0.0.1:${slowPort}`;
const fast = `http://127.0.0.1:${fastPort}`;

const fakeAxios: any = {
  request: async (cfg: any) => {
    const r = await fetch(cfg.baseURL + cfg.url, { method: cfg.method, signal: cfg.signal });
    return { data: await r.json() };
  },
};
const configFor = (baseUrl: string): any => ({ baseUrl, __EXTRA__ });

const slowClient = createClient(configFor(slow));
const fastClient = createClient(configFor(fast));

// (a) a deadline on a server that never answers rejects with DeadlineExceededError.
let a: unknown;
try {
  await slowClient.ping({ timeoutMs: DEADLINE_MS });
} catch (error) {
  a = error;
}
if (!(a instanceof DeadlineExceededError)) {
  throw new Error(`(a) expected DeadlineExceededError, got ${String(a)}`);
}

// (b) a caller abort after 50 ms rejects with CancelledError.
const controller = new AbortController();
setTimeout(() => controller.abort(), DEADLINE_MS);
let b: unknown;
try {
  await slowClient.ping({ signal: controller.signal });
} catch (error) {
  b = error;
}
if (!(b instanceof CancelledError)) {
  throw new Error(`(b) expected CancelledError, got ${String(b)}`);
}

// (d) a call with no options still returns the JSON of a fast server.
const ok = await fastClient.ping();
if (JSON.stringify(ok) !== JSON.stringify({ ok: true })) {
  throw new Error(`(d) unexpected body ${JSON.stringify(ok)}`);
}

// (e) an already aborted signal rejects with CancelledError and sends nothing.
let e: unknown;
try {
  await slowClient.ping({ signal: AbortSignal.abort() });
} catch (error) {
  e = error;
}
if (!(e instanceof CancelledError)) {
  throw new Error(`(e) expected CancelledError, got ${String(e)}`);
}

console.log("ok");
"##;

fn opts(http_client: HttpClient) -> GenOptions {
    GenOptions::new(Lang::Ts, PathBuf::new(), PathBuf::new(), "createClient")
        .with_emit_hooks(false)
        .with_http_client(http_client)
}

fn temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "openapi-codegen-deadline-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create deadline temp directory");
    path
}

/// Reads one request head, byte by byte, up to the blank line. False when the
/// client closes first.
fn read_request_head(stream: &mut TcpStream) -> bool {
    let mut seen = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match stream.read(&mut byte) {
            Ok(1) => {
                seen.push(byte[0]);
                if seen.ends_with(b"\r\n\r\n") {
                    return true;
                }
            }
            _ => return false,
        }
    }
}

/// Counts every connection that sends a request, and never answers it. Each
/// socket stays open, so only the client can end the call.
fn slow_server(count: Arc<AtomicUsize>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind slow server");
    let port = listener.local_addr().expect("slow server address").port();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else {
                continue;
            };
            let count = Arc::clone(&count);
            thread::spawn(move || {
                if read_request_head(&mut stream) {
                    count.fetch_add(1, Ordering::SeqCst);
                }
                std::mem::forget(stream);
            });
        }
    });
    port
}

/// Answers every request with `{"ok":true}`.
fn fast_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind fast server");
    let port = listener.local_addr().expect("fast server address").port();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else {
                continue;
            };
            thread::spawn(move || {
                if read_request_head(&mut stream) {
                    use std::io::Write;
                    let _ = stream.write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}",
                    );
                }
            });
        }
    });
    port
}

/// Two rewrites, made only in this test's copy of the output, so Node can run
/// it. Node's ESM loader needs the `.ts` extension on a relative import. Node's
/// strip-only mode cannot run a parameter property, which the plain runtime keeps
/// for `tsc`; the managed and file-bearer runtimes make the same constructor
/// rewrite in `runtime.rs`. The generated code this test checks is unchanged.
fn make_node_runnable(dir: &Path) {
    for name in ["client.ts", "runtime.ts"] {
        let path = dir.join(name);
        let text = fs::read_to_string(&path).expect("read generated TypeScript");
        let text = text
            .replace("from \"./runtime\"", "from \"./runtime.ts\"")
            .replace("from \"./types\"", "from \"./types.ts\"")
            .replacen(
                "  constructor(private readonly maxInFlight: number) {}",
                "  private readonly maxInFlight: number;\n  constructor(maxInFlight: number) { this.maxInFlight = maxInFlight; }",
                1,
            );
        fs::write(&path, text).expect("write Node-runnable TypeScript");
    }
}

fn node_is_installed() -> bool {
    Command::new("node").arg("--version").output().is_ok()
}

fn run_contract(label: &str, http_client: HttpClient, axios_stub: bool) {
    let root = temp_dir(label);
    let generated_dir = root.join("generated");
    let output = generate(SPEC, &opts(http_client)).expect("generate TypeScript client");
    output
        .write_to_dir(&generated_dir)
        .expect("write generated output");
    make_node_runnable(&generated_dir);
    if axios_stub {
        // axios is not installed here: a stub package stands in for it. The
        // script passes its own instance, so the stub's `create` is never used.
        let package = root.join("node_modules/axios");
        fs::create_dir_all(&package).expect("create axios stub directory");
        fs::write(
            package.join("package.json"),
            r#"{"name":"axios","type":"module","main":"index.js"}"#,
        )
        .expect("write axios stub manifest");
        fs::write(
            package.join("index.js"),
            r#"export default { create() { throw new Error("stub axios: pass config.axios"); } };
"#,
        )
        .expect("write axios stub");
    }

    let slow_count = Arc::new(AtomicUsize::new(0));
    let slow_port = slow_server(Arc::clone(&slow_count));
    let fast_port = fast_server();

    let extra = if axios_stub { "axios: fakeAxios," } else { "" };
    let script_path = root.join("contract.ts");
    fs::write(&script_path, CONTRACT.replace("__EXTRA__", extra)).expect("write contract script");
    let run = Command::new("node")
        .arg("--experimental-strip-types")
        .arg("--no-warnings")
        .arg(&script_path)
        .arg(slow_port.to_string())
        .arg(fast_port.to_string())
        .output()
        .expect("spawn node");
    assert!(
        run.status.success(),
        "{label} deadline contract failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout).trim_end(), "ok");
    assert_eq!(
        slow_count.load(Ordering::SeqCst),
        2,
        "{label}: one request for the deadline call and one for the cancel call; no re-send"
    );
    fs::remove_dir_all(root).expect("remove deadline temp directory");
}

#[test]
fn fetch_backend_deadline_and_cancel() {
    if !node_is_installed() {
        eprintln!("skipping: node is not installed");
        return;
    }
    run_contract("fetch", HttpClient::Fetch, false);
}

#[test]
fn axios_backend_deadline_and_cancel() {
    if !node_is_installed() {
        eprintln!("skipping: node is not installed");
        return;
    }
    run_contract("axios", HttpClient::Axios, true);
}
