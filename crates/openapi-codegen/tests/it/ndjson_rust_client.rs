use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use openapi_codegen::{
    generate_for_target, GenOptions, GeneratedOutput, Lang, RustTarget, TargetProfile,
};

const SPEC: &str = r##"{
  "openapi": "3.1.0",
  "info": { "title": "NDJSON", "version": "1.0.0" },
  "paths": {
    "/items": {
      "get": {
        "operationId": "listItems",
        "responses": {
          "200": { "description": "ok", "content": { "application/x-ndjson": { "schema": { "$ref": "#/components/schemas/Item" } } } }
        }
      },
      "post": {
        "operationId": "uploadItems",
        "requestBody": { "required": true, "content": { "application/x-ndjson": { "schema": { "$ref": "#/components/schemas/Item" } } } },
        "responses": { "204": { "description": "stored" } }
      }
    }
  },
  "components": {
    "schemas": {
      "Item": { "type": "object", "properties": { "id": { "type": "integer" } }, "required": ["id"] }
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
        "openapi-codegen-ndjson-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create NDJSON temp directory");
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

/// Serves two connections in order: the NDJSON `GET`, then the NDJSON `POST`.
/// The `GET` reply has a malformed third line and ends when the connection
/// closes. The `POST` is read to the end of its chunked body, then answered
/// `204`. The thread returns the `POST` request as text.
fn ndjson_stub_server() -> (SocketAddr, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind stub server");
    let addr = listener.local_addr().expect("stub server address");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept GET");
        read_head(&mut stream);
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nConnection: close\r\n\r\n{\"id\":1}\n{\"id\":2}\nnot-json\n",
            )
            .expect("write GET reply");
        drop(stream);

        let (mut stream, _) = listener.accept().expect("accept POST");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("set read timeout");
        let mut request = Vec::new();
        let mut chunk = [0u8; 1024];
        loop {
            match stream.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => request.extend_from_slice(&chunk[..n]),
            }
            if request.ends_with(b"0\r\n\r\n") {
                break;
            }
        }
        stream
            .write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n")
            .expect("write POST reply");
        String::from_utf8_lossy(&request).into_owned()
    });
    (addr, handle)
}

#[test]
fn generated_rust_client_streams_ndjson_records() {
    let root = temp_dir("rust");
    let source = root.join("src");
    let target = TargetProfile::Rust(RustTarget::Rust2021);
    let generated = generate_for_target(SPEC, &opts(Lang::Rust, target), target)
        .expect("generate Rust NDJSON client");
    materialize(&generated, &source);
    fs::rename(source.join("mod.rs"), source.join("lib.rs")).expect("install Rust lib root");
    fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "generated_ndjson"
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
        r#"use generated_ndjson::client::Client;
use generated_ndjson::models::Item;

fn main() {
    let base_url = std::env::args().nth(1).expect("base url");
    let client = Client::new(base_url);
    for record in client.list_items().expect("start the NDJSON stream") {
        match record {
            Ok(item) => println!("item {}", item.id),
            Err(error) => println!("error {error}"),
        }
    }
    client
        .upload_items(vec![Item { id: 7 }].into_iter())
        .expect("upload the NDJSON request");
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
        .expect("spawn cargo build for generated NDJSON client");
    assert_success(build, "compile generated Rust NDJSON client");

    let (addr, handle) = ndjson_stub_server();
    let run = Command::new(root.join("target/debug/generated_ndjson"))
        .arg(format!("http://{addr}"))
        .env_remove("HTTP_PROXY")
        .env_remove("http_proxy")
        .env("NO_PROXY", "127.0.0.1")
        .env("no_proxy", "127.0.0.1")
        .output()
        .expect("spawn generated NDJSON consumer");
    let post = handle.join().expect("join stub server");
    assert!(
        run.status.success(),
        "run generated Rust NDJSON client: {:?}\nstderr:\n{}",
        run.status.code(),
        String::from_utf8_lossy(&run.stderr)
    );
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        stdout.starts_with("item 1\nitem 2\nerror malformed NDJSON record at line 3:"),
        "stdout:\n{stdout}"
    );
    assert!(post.contains("{\"id\":7}\n"), "POST body:\n{post}");
    assert!(
        post.to_ascii_lowercase()
            .contains("content-type: application/x-ndjson"),
        "POST head:\n{post}"
    );
    let _ = fs::remove_dir_all(&root);
}
