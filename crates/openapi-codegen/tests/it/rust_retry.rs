use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use openapi_codegen::{
    generate_for_target, GenOptions, GeneratedOutput, Lang, RustTarget, TargetProfile,
};

const SPEC: &str = r##"{
  "openapi": "3.1.0",
  "info": { "title": "Retry", "version": "1.0.0" },
  "paths": {
    "/safe": {
      "get": {
        "operationId": "safeRead",
        "x-retry-class": "safe-read",
        "responses": {
          "200": { "description": "ok", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Pong" } } } }
        }
      }
    },
    "/never": {
      "get": {
        "operationId": "neverRetry",
        "x-retry-class": "non-retryable",
        "responses": {
          "200": { "description": "ok", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Pong" } } } }
        }
      }
    },
    "/keyed": {
      "get": {
        "operationId": "keyed",
        "x-retry-class": "idempotency-key",
        "parameters": [{ "name": "Idempotency-Key", "in": "header", "required": false, "schema": { "type": "string" } }],
        "responses": {
          "200": { "description": "ok", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Pong" } } } }
        }
      }
    },
    "/plain": {
      "get": {
        "operationId": "plain",
        "responses": {
          "200": { "description": "ok", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Pong" } } } }
        }
      }
    }
  },
  "components": {
    "schemas": {
      "Pong": { "type": "object", "properties": { "ok": { "type": "boolean" } }, "required": ["ok"] }
    }
  }
}"##;

/// Body is `{"ok":true}`, 11 bytes.
const FAST_REPLY: &[u8] =
    b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}";

/// A transient failure with no body.
const FAIL_REPLY: &[u8] =
    b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";

const CONSUMER: &str = r#"use std::time::Duration;

use generated_retry::client::{CallOptions, CancelToken, Client, ClientError, RetryPolicy, TransportPolicy};

fn retrying() -> TransportPolicy {
    TransportPolicy {
        retry: Some(RetryPolicy { max_attempts: 3, delay: |_| Duration::from_millis(1) }),
        ..TransportPolicy::default()
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mode = args.next().expect("mode");
    let base_url = args.next().expect("base url");
    let client = if mode.ends_with("_policy") {
        Client::with_policy(base_url, retrying())
    } else {
        Client::new(base_url)
    };
    match mode.as_str() {
        "safe_policy" | "safe_nopolicy" => match client.safe_read() {
            Ok(pong) => println!("ok {}", pong.ok),
            Err(_) => println!("err"),
        },
        "never_policy" => match client.never_retry() {
            Ok(pong) => println!("ok {}", pong.ok),
            Err(_) => println!("err"),
        },
        "keyed_nokey_policy" => match client.keyed(None) {
            Ok(pong) => println!("ok {}", pong.ok),
            Err(_) => println!("err"),
        },
        "keyed_key_policy" => match client.keyed(Some("k1".to_string())) {
            Ok(pong) => println!("ok {}", pong.ok),
            Err(_) => println!("err"),
        },
        "plain_policy" => match client.plain() {
            Ok(pong) => println!("ok {}", pong.ok),
            Err(_) => println!("err"),
        },
        "deadline_policy" => {
            let options = CallOptions {
                deadline: Some(Duration::from_millis(50)),
                ..Default::default()
            };
            match client.safe_read_with(options) {
                Err(ClientError::DeadlineExceeded(_)) => println!("deadline"),
                Err(err) => println!("other {err}"),
                Ok(pong) => println!("ok {}", pong.ok),
            }
        }
        "cancel_policy" => {
            let token = CancelToken::new();
            let remote = token.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(50));
                remote.cancel();
            });
            let options = CallOptions {
                cancel: Some(token),
                ..Default::default()
            };
            match client.safe_read_with(options) {
                Err(ClientError::Cancelled) => println!("cancelled"),
                Err(err) => println!("other {err}"),
                Ok(pong) => println!("ok {}", pong.ok),
            }
        }
        other => panic!("unknown mode {other}"),
    }
}
"#;

fn opts(target: TargetProfile) -> GenOptions {
    GenOptions::new(Lang::Rust, PathBuf::new(), PathBuf::new(), "createClient")
        .with_target(target)
        .with_emit_hooks(false)
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

fn materialize(output: &GeneratedOutput, path: &Path) {
    output.write_to_dir(path).expect("write generated output");
    assert!(path.join(".openapi-codegen.json").is_file());
}

fn assert_success(output: &Output, what: &str) {
    assert!(
        output.status.success(),
        "{what}: exit {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Reads one request head (up to the blank line) from the stream.
fn read_head(stream: &mut std::net::TcpStream) {
    let mut head = Vec::new();
    let mut chunk = [0u8; 1024];
    while !head.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = stream.read(&mut chunk).expect("read stub request head");
        if n == 0 {
            break;
        }
        head.extend_from_slice(&chunk[..n]);
    }
}

/// Counts each request. The first `fail_first` requests get a 503; the rest get `{"ok":true}`.
fn flaky_server(count: Arc<AtomicUsize>, fail_first: usize) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind flaky server");
    let addr = listener.local_addr().expect("flaky server address");
    thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.expect("accept flaky request");
            read_head(&mut stream);
            let seen = count.fetch_add(1, Ordering::SeqCst);
            if seen < fail_first {
                stream.write_all(FAIL_REPLY).expect("write failed reply");
            } else {
                stream.write_all(FAST_REPLY).expect("write fast reply");
            }
        }
    });
    addr
}

/// Accepts every connection, counts it, and never answers. Each stream is
/// kept open, so the client waits until its own deadline or cancel ends the call.
fn slow_server(count: Arc<AtomicUsize>) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind slow server");
    let addr = listener.local_addr().expect("slow server address");
    thread::spawn(move || {
        let mut held = Vec::new();
        for stream in listener.incoming() {
            let mut stream = stream.expect("accept slow request");
            read_head(&mut stream);
            count.fetch_add(1, Ordering::SeqCst);
            held.push(stream);
        }
    });
    addr
}

fn run(binary: &Path, mode: &str, addr: SocketAddr) -> Output {
    Command::new(binary)
        .arg(mode)
        .arg(format!("http://{addr}"))
        .env_remove("HTTP_PROXY")
        .env_remove("http_proxy")
        .env("NO_PROXY", "127.0.0.1")
        .env("no_proxy", "127.0.0.1")
        .output()
        .expect("spawn generated retry consumer")
}

#[test]
fn generated_rust_client_retry_hooks() {
    let root = temp_dir("rust");
    let source = root.join("src");
    let target = TargetProfile::Rust(RustTarget::Rust2021);
    let generated =
        generate_for_target(SPEC, &opts(target), target).expect("generate Rust retry client");
    materialize(&generated, &source);
    fs::rename(source.join("mod.rs"), source.join("lib.rs")).expect("install Rust lib root");
    fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "generated_retry"
version = "0.0.0"
edition = "2021"

[dependencies]
reqwest = { version = "0.12", features = ["blocking", "json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
"#,
    )
    .expect("write generated Rust manifest");
    fs::write(source.join("main.rs"), CONSUMER).expect("write generated Rust consumer");
    let build = Command::new("cargo")
        .arg("build")
        .arg("--quiet")
        .arg("--offline")
        .current_dir(&root)
        .env("CARGO_TARGET_DIR", root.join("target"))
        .output()
        .expect("spawn cargo build for generated retry client");
    assert_success(&build, "compile generated Rust retry client");

    let binary = root.join("target/debug/generated_retry");

    // Each case: a fresh server, its request count, and the expected output.
    let cases: [(&str, usize, usize, &str); 6] = [
        // (mode, fail_first, expected requests, expected stdout)
        ("safe_policy", 1, 2, "ok true"),
        ("never_policy", 1, 1, "err"),
        ("keyed_nokey_policy", 1, 1, "err"),
        ("keyed_key_policy", 1, 2, "ok true"),
        ("safe_nopolicy", 1, 1, "err"),
        ("plain_policy", 1, 1, "err"),
    ];
    for (mode, fail_first, expected, stdout) in cases {
        let count = Arc::new(AtomicUsize::new(0));
        let addr = flaky_server(Arc::clone(&count), fail_first);
        let out = run(&binary, mode, addr);
        assert_success(&out, mode);
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).trim(),
            stdout,
            "{mode}"
        );
        assert_eq!(
            count.load(Ordering::SeqCst),
            expected,
            "{mode} request count"
        );
    }

    // A deadline ends a call that never gets an answer, after one request.
    let count = Arc::new(AtomicUsize::new(0));
    let slow = slow_server(Arc::clone(&count));
    let out = run(&binary, "deadline_policy", slow);
    assert_success(&out, "deadline_policy");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "deadline");
    assert_eq!(count.load(Ordering::SeqCst), 1, "deadline sent one request");

    // A cancel ends a call that never gets an answer, after one request.
    let count = Arc::new(AtomicUsize::new(0));
    let slow = slow_server(Arc::clone(&count));
    let out = run(&binary, "cancel_policy", slow);
    assert_success(&out, "cancel_policy");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "cancelled");
    assert_eq!(count.load(Ordering::SeqCst), 1, "cancel sent one request");

    let _ = fs::remove_dir_all(&root);
}
