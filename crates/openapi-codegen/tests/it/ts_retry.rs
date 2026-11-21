use std::fs;
use std::io::{Read, Write};
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
  "info": { "title": "Retry contract", "version": "1.0.0" },
  "paths": {
    "/read": {
      "get": {
        "operationId": "readThing",
        "x-retry-class": "safe-read",
        "responses": { "200": { "description": "ok", "content": { "application/json": { "schema": { "type": "object", "properties": { "ok": { "type": "boolean" } } } } } } }
      }
    },
    "/write": {
      "post": {
        "operationId": "writeThing",
        "x-retry-class": "idempotency-key",
        "responses": { "200": { "description": "ok", "content": { "application/json": { "schema": { "type": "object", "properties": { "ok": { "type": "boolean" } } } } } } }
      }
    },
    "/mutate": {
      "delete": {
        "operationId": "mutateThing",
        "x-retry-class": "non-retryable",
        "responses": { "200": { "description": "ok", "content": { "application/json": { "schema": { "type": "object", "properties": { "ok": { "type": "boolean" } } } } } } }
      }
    },
    "/plain": {
      "get": {
        "operationId": "plainThing",
        "responses": { "200": { "description": "ok", "content": { "application/json": { "schema": { "type": "object", "properties": { "ok": { "type": "boolean" } } } } } } }
      }
    }
  }
}"##;

/// `__EXTRA__` is `axios: fakeAxios,` for the axios backend and empty for fetch.
const CONTRACT: &str = r##"import { createClient } from "./generated/client.ts";
import { DeadlineExceededError } from "./generated/runtime.ts";

const hang = setTimeout(() => {
  console.error("hang");
  process.exit(2);
}, 20000);
hang.unref();

const fakeAxios: any = {
  request: async (cfg: any) => {
    const r = await fetch(cfg.baseURL + cfg.url, { method: cfg.method, signal: cfg.signal });
    if (!r.ok) throw Object.assign(new Error(`HTTP ${r.status}`), { response: { status: r.status } });
    return { data: await r.json() };
  },
};

const ports = process.argv.slice(2);
const retry = { maxAttempts: 3, delayMs: () => 5 };
const client = (i: number, extra: any = {}): any =>
  createClient({ baseUrl: `http://127.0.0.1:${ports[i]}`, __EXTRA__ retry, ...extra });

async function failure(call: () => Promise<unknown>): Promise<unknown> {
  try {
    await call();
  } catch (error) {
    return error;
  }
  throw new Error("expected the call to fail");
}

const same = (a: unknown, b: unknown, what: string) => {
  if (JSON.stringify(a) !== JSON.stringify(b)) throw new Error(`${what}: got ${JSON.stringify(a)}`);
};

// (a) a safe-read call succeeds on the third attempt.
same(await client(0).readThing(), { ok: true }, "a");

// (b) a safe-read call stops at maxAttempts and rejects with the 503.
const b = await failure(() => client(1).readThing());
if (!String((b as any).message).includes("503")) throw new Error(`b: got ${String(b)}`);

// (c) non-retryable is sent once.
await failure(() => client(2).mutateThing());

// (d) an unclassified operation is sent once.
await failure(() => client(3).plainThing());

// (e) idempotency-key without a key is sent once.
await failure(() => client(4).writeThing());

// (e2) idempotency-key with a key is retried and succeeds on the third attempt.
same(await client(5, { headers: { "Idempotency-Key": "k1" } }).writeThing(), { ok: true }, "e2");

// (f) with no retry policy a safe-read call is sent once.
await failure(() => client(6, { retry: undefined }).readThing());

// (g) a 404 is not retried.
await failure(() => client(7).readThing());

// (h) a deadline is not retried.
const h = await failure(() => client(8).readThing({ timeoutMs: 300 }));
if (!(h instanceof DeadlineExceededError)) throw new Error(`h: got ${String(h)}`);

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
        "openapi-codegen-retry-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create retry temp directory");
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

/// Counts every request. The first `fail_first` requests get `status`; later
/// ones get `{"ok":true}`. Each response closes its connection.
fn status_server(count: Arc<AtomicUsize>, fail_first: usize, status: u16) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind status server");
    let port = listener.local_addr().expect("status server address").port();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else {
                continue;
            };
            let count = Arc::clone(&count);
            thread::spawn(move || {
                if read_request_head(&mut stream) {
                    let n = count.fetch_add(1, Ordering::SeqCst);
                    let reply: Vec<u8> = if n >= fail_first {
                        b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}".to_vec()
                    } else {
                        format!(
                            "HTTP/1.1 {status} X\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        )
                        .into_bytes()
                    };
                    let _ = stream.write_all(&reply);
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

    // Nine servers, in the order the script reads its ports.
    let counts: Vec<Arc<AtomicUsize>> = (0..9).map(|_| Arc::new(AtomicUsize::new(0))).collect();
    let plan: [(usize, u16); 8] = [
        (2, 503),    // 0: safe-read, succeeds on the third attempt
        (1000, 503), // 1: safe-read, always 503
        (1000, 503), // 2: non-retryable
        (1000, 503), // 3: unclassified
        (1000, 503), // 4: idempotency-key without a key
        (2, 503),    // 5: idempotency-key with a key, succeeds on the third attempt
        (1000, 503), // 6: no retry policy
        (1000, 404), // 7: safe-read, 404 is not retried
    ];
    let mut ports: Vec<u16> = plan
        .iter()
        .zip(&counts)
        .map(|(&(fail_first, status), count)| status_server(Arc::clone(count), fail_first, status))
        .collect();
    ports.push(slow_server(Arc::clone(&counts[8])));

    let extra = if axios_stub { "axios: fakeAxios," } else { "" };
    let script_path = root.join("contract.ts");
    fs::write(&script_path, CONTRACT.replace("__EXTRA__", extra)).expect("write contract script");
    let mut command = Command::new("node");
    command
        .arg("--experimental-strip-types")
        .arg("--no-warnings")
        .arg(&script_path);
    for port in &ports {
        command.arg(port.to_string());
    }
    let run = command.output().expect("spawn node");
    assert!(
        run.status.success(),
        "{label} retry contract failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout).trim_end(), "ok");
    let seen: Vec<usize> = counts.iter().map(|c| c.load(Ordering::SeqCst)).collect();
    assert_eq!(
        seen,
        vec![3, 3, 1, 1, 1, 3, 1, 1, 1],
        "{label}: requests per server (safe-read 0/1/6/7, non-retryable 2, unclassified 3, idempotency-key 4/5, deadline 8)"
    );
    fs::remove_dir_all(root).expect("remove retry temp directory");
}

#[test]
fn fetch_backend_retry() {
    if !node_is_installed() {
        eprintln!("skipping: node is not installed");
        return;
    }
    run_contract("fetch", HttpClient::Fetch, false);
}

#[test]
fn axios_backend_retry() {
    if !node_is_installed() {
        eprintln!("skipping: node is not installed");
        return;
    }
    run_contract("axios", HttpClient::Axios, true);
}
