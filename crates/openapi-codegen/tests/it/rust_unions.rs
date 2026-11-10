use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use openapi_codegen::{
    generate_for_target, GenOptions, GeneratedOutput, Lang, RustTarget, TargetProfile,
};

const SPEC: &str = r##"{
  "openapi": "3.1.0",
  "info": { "title": "Unions", "version": "1.0.0" },
  "paths": {},
  "components": {
    "schemas": {
      "Cat": { "type": "object", "properties": { "name": { "type": "string" } }, "required": ["name"] },
      "Dog": { "type": "object", "properties": { "bark": { "type": "boolean" } }, "required": ["bark"] },
      "Pet": {
        "oneOf": [ { "$ref": "#/components/schemas/Cat" }, { "$ref": "#/components/schemas/Dog" } ],
        "discriminator": {
          "propertyName": "kind",
          "mapping": { "cat": "#/components/schemas/Cat", "dog": "#/components/schemas/Dog" }
        }
      },
      "Shape": { "oneOf": [ { "$ref": "#/components/schemas/Cat" }, { "$ref": "#/components/schemas/Dog" } ] },
      "Id": { "anyOf": [ { "type": "string" }, { "type": "integer" } ] },
      "Blob": { "oneOf": [ { "type": "object" }, { "type": "object" } ] }
    }
  }
}"##;

fn opts(target: TargetProfile) -> GenOptions {
    GenOptions::new(Lang::Rust, PathBuf::new(), PathBuf::new(), "createClient")
        .with_target(target)
        .with_emit_client(false)
        .with_emit_hooks(false)
}

fn temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "openapi-codegen-unions-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create unions temp directory");
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

#[test]
fn generated_rust_unions_round_trip() {
    let root = temp_dir("rust");
    let source = root.join("src");
    let target = TargetProfile::Rust(RustTarget::Rust2021);
    let generated =
        generate_for_target(SPEC, &opts(target), target).expect("generate Rust union models");
    assert_eq!(generated.warnings().len(), 1);
    assert!(generated.warnings()[0].contains("`Blob`"));
    materialize(&generated, &source);
    fs::rename(source.join("mod.rs"), source.join("lib.rs")).expect("install Rust lib root");
    fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "generated_unions"
version = "0.0.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
"#,
    )
    .expect("write generated Rust manifest");
    fs::write(
        source.join("main.rs"),
        r##"use generated_unions::models::{Blob, Id, Pet, Shape};

fn main() {
    let pet: Pet = serde_json::from_str(r#"{"kind":"cat","name":"Tom"}"#).expect("cat");
    match &pet {
        Pet::Cat(c) => println!("cat:{}", c.name),
        other => panic!("expected cat, got {other:?}"),
    }
    println!("{}", serde_json::to_string(&pet).expect("serialize cat"));

    let pet: Pet = serde_json::from_str(r#"{"kind":"dog","bark":true}"#).expect("dog");
    match &pet {
        Pet::Dog(d) => println!("dog:{}", d.bark),
        other => panic!("expected dog, got {other:?}"),
    }
    println!("{}", serde_json::to_string(&pet).expect("serialize dog"));

    let unknown = serde_json::from_str::<Pet>(r#"{"kind":"bird"}"#);
    if unknown.is_err() {
        println!("unknown:err");
    }

    let shape: Shape = serde_json::from_str(r#"{"name":"Tom"}"#).expect("shape cat");
    match &shape {
        Shape::Cat(_) => println!("shape:cat"),
        other => panic!("expected cat shape, got {}", serde_json::to_string(other).unwrap()),
    }
    println!("{}", serde_json::to_string(&shape).expect("serialize shape cat"));

    let shape: Shape = serde_json::from_str(r#"{"bark":false}"#).expect("shape dog");
    match &shape {
        Shape::Dog(_) => println!("shape:dog"),
        other => panic!("expected dog shape, got {}", serde_json::to_string(other).unwrap()),
    }
    println!("{}", serde_json::to_string(&shape).expect("serialize shape dog"));

    let id: Id = serde_json::from_str(r#""abc""#).expect("id string");
    match &id {
        Id::String(_) => println!("id:string"),
        other => panic!("expected string id, got {}", serde_json::to_string(other).unwrap()),
    }
    println!("{}", serde_json::to_string(&id).expect("serialize id string"));

    let id: Id = serde_json::from_str("7").expect("id integer");
    match &id {
        Id::Integer(_) => println!("id:int"),
        other => panic!("expected integer id, got {}", serde_json::to_string(other).unwrap()),
    }
    println!("{}", serde_json::to_string(&id).expect("serialize id integer"));

    let b: Blob = serde_json::json!({"x": 1});
    println!("blob:{b}");
}
"##,
    )
    .expect("write generated Rust consumer");
    let build = Command::new("cargo")
        .arg("build")
        .arg("--quiet")
        .arg("--offline")
        .current_dir(&root)
        .env("CARGO_TARGET_DIR", root.join("target"))
        .output()
        .expect("spawn cargo build for generated union models");
    assert_success(build, "compile generated Rust union models");

    let run = Command::new(root.join("target/debug/generated_unions"))
        .output()
        .expect("spawn generated union consumer");
    assert!(
        run.status.success(),
        "run generated Rust union consumer: {:?}\nstderr:\n{}",
        run.status.code(),
        String::from_utf8_lossy(&run.stderr)
    );
    let stdout = String::from_utf8_lossy(&run.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines,
        vec![
            "cat:Tom",
            r#"{"kind":"cat","name":"Tom"}"#,
            "dog:true",
            r#"{"kind":"dog","bark":true}"#,
            "unknown:err",
            "shape:cat",
            r#"{"name":"Tom"}"#,
            "shape:dog",
            r#"{"bark":false}"#,
            "id:string",
            r#""abc""#,
            "id:int",
            "7",
            r#"blob:{"x":1}"#,
        ]
    );
    let _ = fs::remove_dir_all(&root);
}
