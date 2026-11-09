use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use openapi_codegen::{
    generate_for_target, GenOptions, GeneratedOutput, Lang, RustTarget, TargetProfile,
};

const SPEC: &str = r##"{
  "openapi": "3.1.0",
  "info": { "title": "Typed errors", "version": "1.0.0" },
  "paths": {
    "/tasks/{id}": {
      "get": {
        "operationId": "getTask",
        "parameters": [{ "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }],
        "responses": {
          "200": { "description": "ok", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Task" } } } },
          "404": { "description": "missing", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/ErrorEnvelope" } } } },
          "4XX": { "description": "client", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/ErrorEnvelope" } } } },
          "default": { "description": "other" }
        }
      }
    },
    "/other": {
      "get": {
        "operationId": "getOther",
        "responses": {
          "200": { "description": "ok" },
          "404": { "description": "missing", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/ErrorEnvelope" } } } }
        }
      }
    }
  },
  "components": {
    "schemas": {
      "Task": { "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] },
      "ErrorEnvelope": {
        "type": "object",
        "properties": { "error": { "type": "string" }, "message": { "type": "string" } },
        "required": ["error", "message"]
      }
    }
  }
}"##;

fn opts(lang: Lang, target: TargetProfile) -> GenOptions {
    GenOptions::new(lang, PathBuf::new(), PathBuf::new(), "createClient")
        .with_target(target)
        .with_emit_hooks(false)
}

fn temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "openapi-codegen-typed-errors-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create typed errors temp directory");
    path
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

/// Answers one request per entry, in order, with the given status line and body.
fn stub_server(
    responses: Vec<(&'static str, &'static str)>,
) -> (SocketAddr, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind stub server");
    let addr = listener.local_addr().expect("stub server address");
    let handle = thread::spawn(move || {
        for (status, body) in responses {
            let (mut stream, _) = listener.accept().expect("accept stub request");
            let mut request = Vec::new();
            let mut chunk = [0u8; 1024];
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                let n = stream.read(&mut chunk).expect("read stub request");
                if n == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..n]);
            }
            let reply = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream
                .write_all(reply.as_bytes())
                .expect("write stub reply");
        }
    });
    (addr, handle)
}

#[test]
fn generated_rust_client_decodes_declared_errors() {
    let root = temp_dir("rust");
    let source = root.join("src");
    let target = TargetProfile::Rust(RustTarget::Rust2021);
    let generated = generate_for_target(SPEC, &opts(Lang::Rust, target), target)
        .expect("generate Rust typed-error client");
    materialize(&generated, &source);
    fs::rename(source.join("mod.rs"), source.join("lib.rs")).expect("install Rust lib root");
    fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "generated_typed_errors"
version = "0.0.0"
edition = "2021"

[dependencies]
reqwest = { version = "0.12", features = ["blocking", "json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
"#,
    )
    .expect("write generated Rust manifest");
    fs::write(
        source.join("main.rs"),
        r#"use generated_typed_errors::client::{ApiError, Client, GetTaskError};

fn main() {
    let base_url = std::env::args().nth(1).expect("base url");
    let client = Client::new(base_url);
    match client.get_task("t1".to_string()) {
        Err(ApiError::Api(GetTaskError::Status404(e))) => println!("404:{}:{}", e.error, e.message),
        other => panic!("expected the 404 variant, ok={}", other.is_ok()),
    }
    match client.get_other() {
        Err(ApiError::Client(_)) => println!("client"),
        other => panic!("expected the generic error path, ok={}", other.is_ok()),
    }
    match client.get_task("t1".to_string()) {
        Ok(task) => println!("ok:{}", task.id),
        other => panic!("expected success, ok={}", other.is_ok()),
    }
}
"#,
    )
    .expect("write generated Rust consumer");
    let build = Command::new("cargo")
        .arg("build")
        .arg("--quiet")
        .arg("--offline")
        .current_dir(&root)
        .env("CARGO_TARGET_DIR", root.join("target"))
        .output()
        .expect("spawn cargo build for generated typed-error client");
    assert_success(build, "compile generated Rust typed-error client");

    let (addr, handle) = stub_server(vec![
        (
            "404 Not Found",
            r#"{"error":"not_found","message":"no task"}"#,
        ),
        ("418 I'm a teapot", "{}"),
        ("200 OK", r#"{"id":"t1"}"#),
    ]);
    let run = Command::new(root.join("target/debug/generated_typed_errors"))
        .arg(format!("http://{addr}"))
        .env_remove("HTTP_PROXY")
        .env_remove("http_proxy")
        .env("NO_PROXY", "127.0.0.1")
        .env("no_proxy", "127.0.0.1")
        .output()
        .expect("spawn generated typed-error consumer");
    handle.join().expect("join stub server");
    assert!(
        run.status.success(),
        "run generated Rust typed-error client: {:?}\nstderr:\n{}",
        run.status.code(),
        String::from_utf8_lossy(&run.stderr)
    );
    let stdout = String::from_utf8_lossy(&run.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines, vec!["404:not_found:no task", "client", "ok:t1"]);
    let _ = fs::remove_dir_all(&root);
}
