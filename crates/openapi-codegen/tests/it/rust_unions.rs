use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use openapi_codegen::{
    generate, generate_for_target, GenOptions, GeneratedOutput, Lang, RustTarget, TargetProfile,
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
      "Blob": { "oneOf": [ { "type": "object" }, { "type": "object" } ] },
      "Outcome": { "oneOf": [
        { "type": "object", "required": ["rows"], "properties": { "rows": { "type": "array", "items": { "type": "string" } } } },
        { "type": "object", "required": ["total"], "properties": { "total": { "type": "integer" } } }
      ] },
      "Clash": { "oneOf": [
        { "type": "object", "required": ["id"], "properties": { "id": { "type": "string" } } },
        { "type": "object", "required": ["id"], "properties": { "id": { "type": "string" } } }
      ] },
      "Node": {
        "oneOf": [
          { "type": "object", "required": ["kind", "rows"], "properties": { "kind": { "type": "string", "enum": ["rows"] }, "rows": { "type": "array", "items": { "type": "string" } } } },
          { "type": "object", "required": ["kind", "total"], "properties": { "kind": { "type": "string", "enum": ["total"] }, "total": { "type": "integer" } } }
        ],
        "discriminator": { "propertyName": "kind" }
      },
      "Loose": {
        "oneOf": [
          { "type": "object", "required": ["kind", "rows"], "properties": { "kind": { "type": "string", "enum": ["a", "b"] }, "rows": { "type": "string" } } },
          { "type": "object", "required": ["kind", "total"], "properties": { "kind": { "type": "string", "enum": ["c"] }, "total": { "type": "integer" } } }
        ],
        "discriminator": { "propertyName": "kind" }
      }
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
    assert_eq!(generated.warnings().len(), 3);
    assert!(generated.warnings().iter().any(|w| w.contains("`Blob`")));
    assert!(generated.warnings().iter().any(|w| w.contains("`Clash`")));
    assert!(generated.warnings().iter().any(|w| w.contains("`Loose`")));
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
        r##"use generated_unions::models::{Blob, Id, Loose, Node, Outcome, Pet, Shape};

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

    let outcome: Outcome = serde_json::from_str(r#"{"rows":["a"]}"#).expect("outcome rows");
    match &outcome {
        Outcome::OutcomeRows(r) => println!("outcome:rows:{}", r.rows.len()),
        other => panic!("expected rows outcome, got {}", serde_json::to_string(other).unwrap()),
    }
    println!("{}", serde_json::to_string(&outcome).expect("serialize outcome rows"));

    let outcome: Outcome = serde_json::from_str(r#"{"total":3}"#).expect("outcome total");
    match &outcome {
        Outcome::OutcomeTotal(t) => println!("outcome:total:{}", t.total),
        other => panic!("expected total outcome, got {}", serde_json::to_string(other).unwrap()),
    }
    println!("{}", serde_json::to_string(&outcome).expect("serialize outcome total"));

    let node: Node = serde_json::from_str(r#"{"kind":"rows","rows":["a"]}"#).expect("node rows");
    match &node {
        Node::NodeVariant1(r) => println!("node:rows:{}", r.rows.len()),
        other => panic!("expected rows node, got {}", serde_json::to_string(other).unwrap()),
    }
    println!("{}", serde_json::to_string(&node).expect("serialize node rows"));

    let node: Node = serde_json::from_str(r#"{"kind":"total","total":3}"#).expect("node total");
    match &node {
        Node::NodeVariant2(t) => println!("node:total:{}", t.total),
        other => panic!("expected total node, got {}", serde_json::to_string(other).unwrap()),
    }
    println!("{}", serde_json::to_string(&node).expect("serialize node total"));

    if serde_json::from_str::<Node>(r#"{"kind":"other","rows":["a"]}"#).is_err() {
        println!("node:unknown:err");
    }

    let loose: Loose = serde_json::json!({"kind": "a"});
    println!("loose:{loose}");
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
            "outcome:rows:1",
            r#"{"rows":["a"]}"#,
            "outcome:total:3",
            r#"{"total":3}"#,
            "node:rows:1",
            r#"{"kind":"rows","rows":["a"]}"#,
            "node:total:3",
            r#"{"kind":"total","total":3}"#,
            "node:unknown:err",
            r#"loose:{"kind":"a"}"#,
        ]
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn inline_union_names_match_across_targets() {
    let rust = generate_for_target(
        SPEC,
        &opts(TargetProfile::Rust(RustTarget::Rust2021)),
        TargetProfile::Rust(RustTarget::Rust2021),
    )
    .expect("generate Rust union models");
    let rust_models = &rust
        .files
        .iter()
        .find(|f| f.rel_path.ends_with("models.rs"))
        .expect("Rust models file")
        .contents;
    assert!(
        rust_models.contains("pub struct NodeVariant1 {"),
        "{rust_models}"
    );
    assert!(
        rust_models.contains("pub struct NodeVariant2 {"),
        "{rust_models}"
    );

    let py_opts = GenOptions::new(Lang::Py, PathBuf::new(), PathBuf::new(), "createClient");
    let py = generate(SPEC, &py_opts).expect("generate Python union models");
    let py_models = &py
        .files
        .iter()
        .find(|f| f.rel_path.ends_with("models.py"))
        .expect("Python models file")
        .contents;
    assert!(
        py_models.contains("class NodeVariant1(BaseModel):"),
        "{py_models}"
    );
    assert!(
        py_models.contains("class NodeVariant2(BaseModel):"),
        "{py_models}"
    );
    assert!(
        py_models.contains("class Node(RootModel[Union[NodeVariant1, NodeVariant2]]):"),
        "{py_models}"
    );

    let ts_opts = GenOptions::new(Lang::Ts, PathBuf::new(), PathBuf::new(), "createClient");
    let ts = generate(SPEC, &ts_opts).expect("generate TypeScript union models");
    let ts_types = &ts
        .files
        .iter()
        .find(|f| f.rel_path.ends_with("types.ts"))
        .expect("TypeScript types file")
        .contents;
    assert!(
        ts_types.contains(
            r#"export type Node = { kind: "rows"; rows: string[] } | { kind: "total"; total: number };"#
        ),
        "{ts_types}"
    );
}

const RECURSIVE_SPEC: &str = r##"{
  "openapi": "3.1.0",
  "info": { "title": "Recursive", "version": "1.0.0" },
  "paths": {},
  "components": {
    "schemas": {
      "TrackTotalMode": { "type": "string", "enum": ["exact", "bounded"] },
      "TrackTotal": { "oneOf": [
        { "type": "object", "required": ["mode"], "properties": { "mode": { "$ref": "#/components/schemas/TrackTotalMode" } } },
        { "type": "object", "required": ["limit"], "properties": { "limit": { "type": "integer" } } }
      ] },
      "QueryNode": { "oneOf": [
        { "type": "object", "required": ["not"], "properties": { "not": { "$ref": "#/components/schemas/QueryNode" } } },
        { "type": "object", "required": ["term"], "properties": { "term": { "type": "string" } } }
      ] }
    }
  }
}"##;

const LUMEN_SPEC: &str = include_str!("../fixtures/lumen-c595089.json");

#[test]
fn generated_rust_shadowed_and_recursive_models_compile() {
    let root = temp_dir("recursive");
    let source = root.join("src");
    let target = TargetProfile::Rust(RustTarget::Rust2021);
    let generated = generate_for_target(RECURSIVE_SPEC, &opts(target), target)
        .expect("generate recursive Rust models");
    assert_eq!(generated.warnings().len(), 0);
    materialize(&generated, &source);
    fs::rename(source.join("mod.rs"), source.join("lib.rs")).expect("install Rust lib root");
    fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "generated_recursive"
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
        r##"use generated_recursive::models::{QueryNode, TrackTotal, TrackTotalMode};

fn main() {
    let node: QueryNode = serde_json::from_str(r#"{"not":{"term":"x"}}"#).expect("not node");
    match &node {
        QueryNode::QueryNodeNot(n) => match &*n.not {
            QueryNode::QueryNodeTerm(t) => println!("node:not:{}", t.term),
            other => panic!("expected term inside not, got {}", serde_json::to_string(other).unwrap()),
        },
        other => panic!("expected not node, got {}", serde_json::to_string(other).unwrap()),
    }

    let total: TrackTotal = serde_json::from_str(r#"{"mode":"exact"}"#).expect("total mode");
    match &total {
        TrackTotal::TrackTotalModeInline(m) => match m.mode {
            TrackTotalMode::Exact => println!("total:mode:exact"),
            _ => panic!("expected exact mode"),
        },
        other => panic!("expected mode total, got {}", serde_json::to_string(other).unwrap()),
    }
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
        .expect("spawn cargo build for recursive Rust models");
    assert_success(build, "compile generated recursive Rust models");

    let run = Command::new(root.join("target/debug/generated_recursive"))
        .output()
        .expect("spawn generated recursive consumer");
    assert!(
        run.status.success(),
        "run generated recursive consumer: {:?}\nstderr:\n{}",
        run.status.code(),
        String::from_utf8_lossy(&run.stderr)
    );
    let stdout = String::from_utf8_lossy(&run.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines, vec!["node:not:x", "total:mode:exact"]);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn generated_rust_lumen_document_models_compile() {
    let root = temp_dir("lumen");
    let source = root.join("src");
    let target = TargetProfile::Rust(RustTarget::Rust2021);
    let generated =
        generate_for_target(LUMEN_SPEC, &opts(target), target).expect("generate lumen Rust models");
    materialize(&generated, &source);
    fs::rename(source.join("mod.rs"), source.join("lib.rs")).expect("install Rust lib root");
    fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "generated_lumen"
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
        r##"use generated_lumen::models::{QueryNode, TrackTotal, TrackTotalMode};

fn main() {
    let _: Option<(QueryNode, TrackTotal, TrackTotalMode)> = None;
    println!("ok");
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
        .expect("spawn cargo build for lumen Rust models");
    assert_success(build, "compile generated lumen Rust models");

    let run = Command::new(root.join("target/debug/generated_lumen"))
        .output()
        .expect("spawn generated lumen consumer");
    assert!(
        run.status.success(),
        "run generated lumen consumer: {:?}\nstderr:\n{}",
        run.status.code(),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "ok");
    let _ = fs::remove_dir_all(&root);
}
