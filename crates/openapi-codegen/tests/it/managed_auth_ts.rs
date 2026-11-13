use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use openapi_codegen::{
    generate, generate_with_auth, FileBearerScheme, GenOptions, GenerationAuth, Lang, ManagedAuth,
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
    GenOptions::new(Lang::Ts, PathBuf::new(), PathBuf::new(), "createClient").with_emit_hooks(false)
}

fn temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "openapi-codegen-managed-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create managed temp directory");
    path
}

fn managed() -> ManagedAuth {
    ManagedAuth::new("/etc/ca/ca.crt", [FileBearerScheme::Https]).expect("valid managed contract")
}

#[test]
fn managed_ts_output_differs_only_in_runtime_ts() {
    let auth = GenerationAuth::new(None, Some(managed())).expect("valid auth selection");
    let managed_out = generate_with_auth(SPEC, &opts(), &auth).expect("generate managed TS");
    let plain = generate(SPEC, &opts()).expect("generate plain TS");

    let managed_paths: Vec<&str> = managed_out
        .files
        .iter()
        .map(|f| f.rel_path.as_str())
        .collect();
    let plain_paths: Vec<&str> = plain.files.iter().map(|f| f.rel_path.as_str()).collect();
    assert_eq!(managed_paths, plain_paths);

    for (managed_file, plain_file) in managed_out.files.iter().zip(plain.files.iter()) {
        if managed_file.rel_path == "runtime.ts" {
            assert_ne!(managed_file.contents, plain_file.contents);
            assert!(managed_file.contents.contains("MANAGED_TOKEN_PATH"));
        } else {
            assert_eq!(
                managed_file.contents, plain_file.contents,
                "{} must not change",
                managed_file.rel_path
            );
        }
    }
}

#[test]
fn no_auth_and_file_bearer_output_are_unchanged() {
    let with_default =
        generate_with_auth(SPEC, &opts(), &GenerationAuth::default()).expect("generate default");
    let plain = generate(SPEC, &opts()).expect("generate plain TS");
    assert_eq!(with_default.files.len(), plain.files.len());
    for (a, b) in with_default.files.iter().zip(plain.files.iter()) {
        assert_eq!(a.rel_path, b.rel_path);
        assert_eq!(a.contents, b.contents, "{} must not change", a.rel_path);
    }
}

#[test]
fn generated_managed_node_client_refuses_http_before_any_transport_call() {
    let root = temp_dir("typescript");
    let generated_dir = root.join("generated");
    let auth = GenerationAuth::new(None, Some(managed())).expect("valid auth selection");
    let output = generate_with_auth(SPEC, &opts(), &auth).expect("generate managed TS client");
    output
        .write_to_dir(&generated_dir)
        .expect("write generated managed output");

    let script = r#"import { request } from "./generated/runtime.ts";
let calls = 0;
const trap: any = async () => { calls += 1; throw new Error("transport called"); };
for (const baseUrl of ["http://svc.example.internal", "ftp://svc.example.internal"]) {
  let refused = false;
  try {
    await request<void>({ baseUrl, fetch: trap }, { method: "GET", path: "/ping", expectBody: false });
  } catch (error) {
    refused = String(error).includes("accept only https");
  }
  if (!refused) throw new Error(`not refused: ${baseUrl}`);
}
if (calls !== 0) throw new Error("transport ran before the https check");
"#;
    let script_path = root.join("contract.ts");
    fs::write(&script_path, script).expect("write managed runtime contract");
    let run = Command::new("node")
        .arg("--experimental-strip-types")
        .arg("--no-warnings")
        .arg(&script_path)
        .output()
        .expect("spawn node");
    assert!(
        run.status.success(),
        "managed node contract failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    fs::remove_dir_all(root).expect("remove managed temp directory");
}
