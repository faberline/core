//! Pins the exit codes of the filesystem-writing entry `openapi_codegen::run`:
//! 0 ok, 1 generation or write error, 2 spec read error.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use openapi_codegen::{run, GenOptions, Lang, MANIFEST_FILE};

const SPEC: &str = r##"{
  "openapi": "3.1.0",
  "info": { "title": "Run entry", "version": "1.0.0" },
  "paths": {
    "/ping": {
      "get": {
        "operationId": "ping",
        "responses": { "204": { "description": "ok" } }
      }
    }
  }
}"##;

fn temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "openapi-codegen-run-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create run temp directory");
    path
}

fn opts(spec_path: &Path, out_dir: &Path) -> GenOptions {
    GenOptions::new(Lang::Ts, spec_path, out_dir, "createClient").with_emit_hooks(false)
}

#[test]
fn a_spec_that_cannot_be_read_exits_2() {
    let dir = temp_dir("missing");
    let out = dir.join("out");
    assert_eq!(run(&opts(&dir.join("absent.json"), &out)), 2);
    assert!(!out.exists());
    fs::remove_dir_all(dir).expect("remove run temp directory");
}

#[test]
fn a_spec_that_does_not_parse_exits_1() {
    let dir = temp_dir("unparsable");
    let spec = dir.join("spec.json");
    fs::write(&spec, "not json").expect("write spec");
    let out = dir.join("out");
    assert_eq!(run(&opts(&spec, &out)), 1);
    assert!(!out.exists());
    fs::remove_dir_all(dir).expect("remove run temp directory");
}

#[test]
fn an_output_directory_that_cannot_be_written_exits_1() {
    let dir = temp_dir("unwritable");
    let spec = dir.join("spec.json");
    fs::write(&spec, SPEC).expect("write spec");
    let out = dir.join("out");
    fs::write(&out, "a file where the output directory should be").expect("write blocker");
    assert_eq!(run(&opts(&spec, &out)), 1);
    fs::remove_dir_all(dir).expect("remove run temp directory");
}

#[test]
fn a_good_spec_writes_the_generated_files_and_exits_0() {
    let dir = temp_dir("ok");
    let spec = dir.join("spec.json");
    fs::write(&spec, SPEC).expect("write spec");
    let out = dir.join("out");
    assert_eq!(run(&opts(&spec, &out)), 0);
    let written: Vec<_> = fs::read_dir(&out)
        .expect("read output directory")
        .map(|entry| entry.expect("output entry").file_name())
        .collect();
    assert!(!written.is_empty());
    assert!(!out.join(MANIFEST_FILE).exists(), "no target, no manifest");
    fs::remove_dir_all(dir).expect("remove run temp directory");
}
