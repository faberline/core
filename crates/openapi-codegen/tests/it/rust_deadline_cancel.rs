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
  "info": { "title": "Deadline", "version": "1.0.0" },
  "paths": {
    "/ping": {
      "get": {
        "operationId": "ping",
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

const CONSUMER: &str = r#"use std::time::Duration;

use generated_deadline::client::{CallOptions, CancelToken, Client, ClientError};

fn main() {
    let mut args = std::env::args().skip(1);
    let mode = args.next().expect("mode");
    let base_url = args.next().expect("base url");
    let client = Client::new(base_url);
    match mode.as_str() {
        "plain" => {
            let pong = client.ping().expect("plain ping");
            println!("ok {}", pong.ok);
        }
        "default" => match client.ping_with(CallOptions::default()) {
            Ok(pong) => println!("ok {}", pong.ok),
            Err(err) => println!("other {err}"),
        },
        "deadline" => {
            let options = CallOptions {
                deadline: Some(Duration::from_millis(50)),
                ..Default::default()
            };
            match client.ping_with(options) {
                Err(ClientError::DeadlineExceeded(_)) => println!("deadline"),
                Err(err) => println!("other {err}"),
                Ok(pong) => println!("ok {}", pong.ok),
            }
        }
        "cancel" => {
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
            match client.ping_with(options) {
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
        "openapi-codegen-deadline-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create deadline temp directory");
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

/// Accepts every connection and answers `{"ok":true}`.
fn fast_server() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind fast server");
    let addr = listener.local_addr().expect("fast server address");
    thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.expect("accept fast request");
            read_head(&mut stream);
            stream.write_all(FAST_REPLY).expect("write fast reply");
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
        .expect("spawn generated deadline consumer")
}

#[test]
fn generated_rust_client_deadline_and_cancel() {
    let root = temp_dir("rust");
    let source = root.join("src");
    let target = TargetProfile::Rust(RustTarget::Rust2021);
    let generated =
        generate_for_target(SPEC, &opts(target), target).expect("generate Rust deadline client");
    materialize(&generated, &source);
    fs::rename(source.join("mod.rs"), source.join("lib.rs")).expect("install Rust lib root");
    fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "generated_deadline"
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
        .expect("spawn cargo build for generated deadline client");
    assert_success(&build, "compile generated Rust deadline client");

    let binary = root.join("target/debug/generated_deadline");
    let count = Arc::new(AtomicUsize::new(0));
    let slow = slow_server(Arc::clone(&count));
    let fast = fast_server();

    // (a) A deadline ends a call that never gets an answer, after one request.
    let out = run(&binary, "deadline", slow);
    assert_success(&out, "deadline call");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "deadline");
    assert_eq!(
        count.load(Ordering::SeqCst),
        1,
        "deadline call sent one request"
    );

    // (b) A cancel from another thread ends a call that never gets an answer.
    //     The request is sent once: the counter goes from 1 to 2, no re-send.
    let out = run(&binary, "cancel", slow);
    assert_success(&out, "cancel call");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "cancelled");
    assert_eq!(
        count.load(Ordering::SeqCst),
        2,
        "cancel call sent one request"
    );

    // (d) Default options return the body of a fast server.
    let out = run(&binary, "default", fast);
    assert_success(&out, "default call");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "ok true");

    // (e) The plain method keeps its old signature and still works.
    let out = run(&binary, "plain", fast);
    assert_success(&out, "plain call");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "ok true");

    let _ = fs::remove_dir_all(&root);
}
