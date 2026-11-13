use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use openapi_codegen::{
    generate, generate_with_auth, FileBearerScheme, GenOptions, GeneratedOutput, GenerationAuth,
    Lang, ManagedAuth, RustTarget, TargetProfile,
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

// A public, self-signed test certificate. Its private key was discarded at
// generation time; never add a private key here.
const TEST_CA_PEM: &str = "-----BEGIN CERTIFICATE-----
MIIBhDCCASugAwIBAgIUWm5F+i5rPjM6lV7TCBidQ92EqLYwCgYIKoZIzj0EAwIw
FzEVMBMGA1UEAwwMbWFuYWdlZC10ZXN0MCAXDTI2MTExMzEwMzUyMFoYDzIxMjYx
MDIwMTAzNTIwWjAXMRUwEwYDVQQDDAxtYW5hZ2VkLXRlc3QwWTATBgcqhkjOPQIB
BggqhkjOPQMBBwNCAASCbcUKreZMVdDJdrruKYDx7aM/gTLLdKacGWz/kgmEnghM
7UxzA5GZemhg+ZBYueDyMG0i0VGMFCSZXyw7s3mco1MwUTAdBgNVHQ4EFgQU0OOl
BkMpW/85AJpM194/UiUPwO0wHwYDVR0jBBgwFoAU0OOlBkMpW/85AJpM194/UiUP
wO0wDwYDVR0TAQH/BAUwAwEB/zAKBggqhkjOPQQDAgNHADBEAiBK+HVBve+CdQlj
0ORlEE4Zk/R8/ThpaPm3Ckc5HgOgEgIgDzOLmUf0LOiJLt9FmHEAewX1j36ef/Df
N53bgDAKu5E=
-----END CERTIFICATE-----
";

fn rust_opts(target: Option<TargetProfile>) -> GenOptions {
    let opts = GenOptions::new(Lang::Rust, PathBuf::new(), PathBuf::new(), "createClient")
        .with_emit_hooks(false);
    match target {
        Some(target) => opts.with_target(target),
        None => opts,
    }
}

fn temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "openapi-codegen-managed-rs-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create managed Rust temp directory");
    path
}

fn materialize(output: &GeneratedOutput, path: &Path) {
    output.write_to_dir(path).expect("write generated output");
    assert!(path.join(".openapi-codegen.json").is_file());
}

fn output(command: &mut Command, what: &str) -> Output {
    command
        .output()
        .unwrap_or_else(|error| panic!("{what}: failed to spawn: {error}"))
}

fn managed(ca_path: &Path) -> ManagedAuth {
    ManagedAuth::new(ca_path, [FileBearerScheme::Https]).expect("valid managed contract")
}

fn file<'a>(output: &'a GeneratedOutput, rel_path: &str) -> &'a str {
    output
        .files
        .iter()
        .find(|file| file.rel_path == rel_path)
        .unwrap_or_else(|| panic!("missing {rel_path}"))
        .contents
        .as_str()
}

#[test]
fn managed_rs_output_differs_only_in_client_rs() {
    let root = temp_dir("differs");
    let target = TargetProfile::Rust(RustTarget::Rust2021);
    let auth = GenerationAuth::new(None, Some(managed(&root.join("ca.pem")))).unwrap();
    let managed_out = generate_with_auth(SPEC, &rust_opts(Some(target)), &auth)
        .expect("generate managed Rust client");
    let plain_out = generate(SPEC, &rust_opts(Some(target))).expect("generate plain Rust client");

    let managed_paths: Vec<&str> = managed_out
        .files
        .iter()
        .map(|file| file.rel_path.as_str())
        .collect();
    let plain_paths: Vec<&str> = plain_out
        .files
        .iter()
        .map(|file| file.rel_path.as_str())
        .collect();
    assert_eq!(managed_paths, plain_paths);
    for rel_path in plain_paths {
        if rel_path == "client.rs" {
            assert_ne!(file(&managed_out, rel_path), file(&plain_out, rel_path));
            assert!(file(&managed_out, rel_path).contains("MANAGED_TOKEN_PATH"));
        } else {
            assert_eq!(file(&managed_out, rel_path), file(&plain_out, rel_path));
        }
    }
    fs::remove_dir_all(&root).ok();
}

#[test]
fn no_auth_rs_output_is_unchanged() {
    let plain = generate(SPEC, &rust_opts(None)).expect("generate plain Rust client");
    let auth = GenerationAuth::new(None, None).unwrap();
    let no_auth = generate_with_auth(SPEC, &rust_opts(None), &auth)
        .expect("generate Rust client with no auth");
    assert_eq!(plain.files.len(), no_auth.files.len());
    for (left, right) in plain.files.iter().zip(&no_auth.files) {
        assert_eq!(left.rel_path, right.rel_path);
        assert_eq!(left.contents, right.contents);
    }
}

#[test]
fn generated_managed_rust_client_refuses_before_any_transport_call() {
    let root = temp_dir("runtime");
    let source = root.join("src");
    let target = TargetProfile::Rust(RustTarget::Rust2021);
    let auth = GenerationAuth::new(None, Some(managed(&root.join("ca.pem")))).unwrap();
    let generated = generate_with_auth(SPEC, &rust_opts(Some(target)), &auth)
        .expect("generate managed Rust client");
    materialize(&generated, &source);
    fs::rename(source.join("mod.rs"), source.join("lib.rs")).expect("install Rust lib root");
    fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "generated_managed"
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
        r#"use generated_managed::client::{Client, TransportPolicy};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args[1].as_str();
    let arg = &args[2];
    let result = match mode {
        // arg = path of a CA file
        "ca" => Client::with_ca_path("https://svc.example.internal", TransportPolicy::default(), arg).map(|_| ()),
        // arg = base URL; the CA file path is args[3]
        "http" => Client::with_ca_path(arg, TransportPolicy::default(), &args[3])
            .and_then(|client| client.ping()),
        _ => panic!("unknown mode"),
    };
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(7);
    }
}
"#,
    )
    .expect("write managed Rust main");

    let build = output(
        Command::new("cargo")
            .arg("build")
            .arg("--quiet")
            .arg("--offline")
            .current_dir(&root),
        "cargo build managed Rust client",
    );
    assert!(
        build.status.success(),
        "cargo build: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let binary = root.join("target/debug/generated_managed");

    let missing = root.join("missing.pem");
    let run = output(
        Command::new(&binary).args(["ca", missing.to_str().unwrap()]),
        "managed missing CA",
    );
    assert_refused(&run, "managed CA bundle is unavailable", "missing CA");

    let empty = root.join("empty.pem");
    fs::write(&empty, "\n").expect("write empty CA");
    let run = output(
        Command::new(&binary).args(["ca", empty.to_str().unwrap()]),
        "managed empty CA",
    );
    assert_refused(&run, "managed CA bundle is empty", "empty CA");

    let bad = root.join("bad.pem");
    fs::write(&bad, "not a certificate\n").expect("write bad CA");
    let run = output(
        Command::new(&binary).args(["ca", bad.to_str().unwrap()]),
        "managed bad CA",
    );
    assert_refused(&run, "is not a PEM certificate", "bad CA");

    let ca = root.join("ca.pem");
    fs::write(&ca, TEST_CA_PEM).expect("write test CA");
    let run = output(
        Command::new(&binary).args(["http", "http://127.0.0.1:9", ca.to_str().unwrap()]),
        "managed http base URL",
    );
    assert_refused(&run, "managed clients accept only https: URLs", "http URL");
    assert!(
        !String::from_utf8_lossy(&run.stderr).contains("error sending request"),
        "the refusal must happen before any transport call: {}",
        String::from_utf8_lossy(&run.stderr)
    );

    fs::remove_dir_all(&root).ok();
}

fn assert_refused(run: &Output, expected: &str, what: &str) {
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert_eq!(
        run.status.code(),
        Some(7),
        "{what}: exit {:?}\nstderr:\n{stderr}",
        run.status.code()
    );
    assert!(
        stderr.contains(expected),
        "{what}: expected `{expected}` in stderr:\n{stderr}"
    );
}
