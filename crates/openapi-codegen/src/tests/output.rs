use std::collections::BTreeSet;

use super::*;

#[test]
fn materialized_output_writes_a_versioned_contract_manifest() {
    let opts = opts_for(Lang::Py);
    let output = generate_for_target(
        TARGET_PROFILE_SPEC,
        &opts,
        TargetProfile::Python(PythonTarget::Py314),
    )
    .unwrap();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "openapi-codegen-manifest-{}-{nonce}",
        std::process::id()
    ));

    output.write_to_dir(&dir).unwrap();
    let manifest: GenerationManifest =
        serde_json::from_str(&fs::read_to_string(dir.join(MANIFEST_FILE)).unwrap()).unwrap();
    assert_eq!(manifest.schema_version, 2);
    assert_eq!(manifest.generator, "openapi-codegen");
    assert_eq!(manifest.generator_version, env!("CARGO_PKG_VERSION"));
    assert!(!manifest.openapi_sha256.is_empty());
    assert_eq!(manifest.service_compatibility, None);
    assert_eq!(manifest.target, "python-3.14");
    assert_eq!(manifest.language, "python");
    assert_eq!(manifest.minimum_version, "3.14");
    assert!(dir.join("models.py").is_file());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn manifest_records_sha256_of_spec_bytes_and_detects_one_byte_change() {
    let opts = opts_for(Lang::Ts).with_target(TargetProfile::TypeScript(TypeScriptTarget::Ts50));
    let out1 = generate(MINIMAL, &opts).unwrap();
    let out2 = generate(MINIMAL, &opts).unwrap();
    let m1 = out1.manifest().unwrap();
    let m2 = out2.manifest().unwrap();
    assert_eq!(m1.openapi_sha256, m2.openapi_sha256);
    assert!(!m1.openapi_sha256.is_empty());

    // One-byte change to the spec gives a different SHA
    let modified_spec = MINIMAL.replacen("Mini", "Mina", 1);
    let out_mod = generate(&modified_spec, &opts).unwrap();
    let m_mod = out_mod.manifest().unwrap();
    assert_ne!(m1.openapi_sha256, m_mod.openapi_sha256);

    // Assert only the SHA is written and no spec content is leaked in the hash
    assert_eq!(m1.openapi_sha256.len(), 64);
    assert!(m1
        .openapi_sha256
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
}

#[test]
fn manifest_carries_service_compatibility_label_verbatim() {
    let opts = opts_for(Lang::Ts)
        .with_target(TargetProfile::TypeScript(TypeScriptTarget::Ts50))
        .with_service_compatibility("service-v2026.10-release");
    let out = generate(MINIMAL, &opts).unwrap();
    let manifest = out.manifest().unwrap();
    assert_eq!(
        manifest.service_compatibility.as_deref(),
        Some("service-v2026.10-release")
    );
    assert_eq!(
        out.service_compatibility(),
        Some("service-v2026.10-release")
    );
}

#[test]
fn rejects_empty_or_whitespace_service_compatibility_label_before_generation() {
    let empty_opts = opts_for(Lang::Ts).with_service_compatibility("");
    let err_empty = generate(MINIMAL, &empty_opts).unwrap_err();
    assert_eq!(
        err_empty.downcast_ref::<ServiceCompatibilityError>(),
        Some(&ServiceCompatibilityError::EmptyOrWhitespace)
    );

    let ws_opts = opts_for(Lang::Ts).with_service_compatibility("   \t\n ");
    let err_ws = generate(MINIMAL, &ws_opts).unwrap_err();
    assert_eq!(
        err_ws.downcast_ref::<ServiceCompatibilityError>(),
        Some(&ServiceCompatibilityError::EmptyOrWhitespace)
    );

    // Also assert try_with_service_compatibility fails typed
    let try_err = GenOptions::new(Lang::Ts, "", "", "")
        .try_with_service_compatibility("  ")
        .unwrap_err();
    assert_eq!(try_err, ServiceCompatibilityError::EmptyOrWhitespace);
}

fn extract_third_party_imports(files: &[GeneratedFile], lang: Lang) -> BTreeSet<String> {
    let mut imports = BTreeSet::new();
    for file in files {
        for line in file.contents.lines() {
            let line = line.trim();
            match lang {
                Lang::Ts => {
                    if let Some(rest) = line.strip_prefix("import ") {
                        if let Some(idx) = rest.find("from ") {
                            let spec = rest[idx + 5..]
                                .trim()
                                .trim_matches(|c| c == '"' || c == '\'' || c == ';');
                            if !spec.starts_with('.') && !spec.is_empty() {
                                imports.insert(spec.to_string());
                            }
                        }
                    }
                }
                Lang::Py => {
                    if let Some(rest) = line.strip_prefix("from ") {
                        let module = rest.split_whitespace().next().unwrap_or("");
                        if module.starts_with('.') {
                            continue;
                        }
                        let top = module.split('.').next().unwrap_or("");
                        if !top.is_empty() && !is_py_stdlib(top) {
                            imports.insert(top.to_string());
                        }
                    } else if let Some(rest) = line.strip_prefix("import ") {
                        let module = rest.split_whitespace().next().unwrap_or("");
                        if module.starts_with('.') {
                            continue;
                        }
                        let top = module.split('.').next().unwrap_or("");
                        if !top.is_empty() && !is_py_stdlib(top) {
                            imports.insert(top.to_string());
                        }
                    }
                }
                Lang::Rust => {
                    for crate_name in ["reqwest", "serde", "serde_json"] {
                        if file.contents.contains(&format!("{crate_name}::"))
                            || file.contents.contains(&format!("use {crate_name}"))
                        {
                            imports.insert(crate_name.to_string());
                        }
                    }
                }
            }
        }
    }
    imports
}

fn is_py_stdlib(module: &str) -> bool {
    matches!(
        module,
        "__future__"
            | "asyncio"
            | "base64"
            | "collections"
            | "concurrent"
            | "dataclasses"
            | "datetime"
            | "decimal"
            | "enum"
            | "functools"
            | "ipaddress"
            | "itertools"
            | "json"
            | "math"
            | "os"
            | "pathlib"
            | "re"
            | "socket"
            | "ssl"
            | "sys"
            | "threading"
            | "time"
            | "typing"
            | "urllib"
            | "uuid"
    )
}

fn check_dependency_coverage(
    output: &GeneratedOutput,
    manifest: &GenerationManifest,
    lang: Lang,
) -> Result<(), String> {
    let third_party_imports = extract_third_party_imports(&output.files, lang);
    for imp in &third_party_imports {
        let covered = manifest.runtime_dependencies.iter().any(|dep| {
            dep == imp
                || dep.starts_with(&format!("{imp}>="))
                || dep.starts_with(&format!("{imp}=="))
                || dep.starts_with(&format!("{imp}<="))
                || dep.starts_with(&format!("{imp}>"))
                || dep.starts_with(&format!("{imp}:"))
        });
        if !covered {
            return Err(format!(
                "manifest missing required runtime dependency: {imp}"
            ));
        }
    }
    Ok(())
}

#[test]
fn runtime_dependencies_cover_all_emitted_third_party_imports() {
    // 1. Python
    let py_opts = opts_for(Lang::Py).with_target(TargetProfile::Python(PythonTarget::Py314));
    let py_out = generate(MINIMAL, &py_opts).unwrap();
    let py_manifest = py_out.manifest().unwrap();
    check_dependency_coverage(&py_out, &py_manifest, Lang::Py).unwrap();

    // 2. TypeScript with axios and TanStack Query hooks
    let ts_opts = opts_for(Lang::Ts)
        .with_target(TargetProfile::TypeScript(TypeScriptTarget::Ts50))
        .with_http_client(HttpClient::Axios)
        .with_emit_hooks(true);
    let ts_out = generate(MINIMAL, &ts_opts).unwrap();
    let ts_manifest = ts_out.manifest().unwrap();
    check_dependency_coverage(&ts_out, &ts_manifest, Lang::Ts).unwrap();

    // 3. Rust
    let rust_opts = opts_for(Lang::Rust).with_target(TargetProfile::Rust(RustTarget::Rust2021));
    let rust_out = generate(MINIMAL, &rust_opts).unwrap();
    let rust_manifest = rust_out.manifest().unwrap();
    check_dependency_coverage(&rust_out, &rust_manifest, Lang::Rust).unwrap();
}

#[test]
fn negative_control_removing_runtime_dependency_fails_coverage_test_naming_dependency() {
    // TypeScript: removing axios
    let ts_opts = opts_for(Lang::Ts)
        .with_target(TargetProfile::TypeScript(TypeScriptTarget::Ts50))
        .with_http_client(HttpClient::Axios);
    let ts_out = generate(MINIMAL, &ts_opts).unwrap();
    let mut ts_manifest = ts_out.manifest().unwrap();
    ts_manifest.runtime_dependencies.retain(|d| d != "axios");
    let err = check_dependency_coverage(&ts_out, &ts_manifest, Lang::Ts).unwrap_err();
    assert!(
        err.contains("axios"),
        "expected error to name missing dependency 'axios', got: {err}"
    );

    // Python: removing pydantic
    let py_opts = opts_for(Lang::Py).with_target(TargetProfile::Python(PythonTarget::Py314));
    let py_out = generate(MINIMAL, &py_opts).unwrap();
    let mut py_manifest = py_out.manifest().unwrap();
    py_manifest.runtime_dependencies.clear();
    let err = check_dependency_coverage(&py_out, &py_manifest, Lang::Py).unwrap_err();
    assert!(
        err.contains("pydantic"),
        "expected error to name missing dependency 'pydantic', got: {err}"
    );

    // Rust: removing reqwest
    let rust_opts = opts_for(Lang::Rust).with_target(TargetProfile::Rust(RustTarget::Rust2021));
    let rust_out = generate(MINIMAL, &rust_opts).unwrap();
    let mut rust_manifest = rust_out.manifest().unwrap();
    rust_manifest
        .runtime_dependencies
        .retain(|d| d != "reqwest");
    let err = check_dependency_coverage(&rust_out, &rust_manifest, Lang::Rust).unwrap_err();
    assert!(
        err.contains("reqwest"),
        "expected error to name missing dependency 'reqwest', got: {err}"
    );
}

#[test]
fn legacy_output_emits_no_manifest() {
    let opts = opts_for(Lang::Ts);
    let out = generate(MINIMAL, &opts).unwrap();
    assert!(out.manifest().is_none());
    assert_eq!(out.target, None);
}

#[test]
fn rejects_escaping_output_before_writing_anything() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "openapi-codegen-prevalidate-{}-{nonce}",
        std::process::id()
    ));
    let output = GeneratedOutput::legacy(vec![
        GeneratedFile {
            rel_path: "safe.ts".to_string(),
            contents: "export const safe = true;\n".to_string(),
        },
        GeneratedFile {
            rel_path: "../escape.ts".to_string(),
            contents: "should never be written\n".to_string(),
        },
    ]);

    let error = output.write_to_dir(&dir).unwrap_err();
    assert_eq!(
        error.to_string(),
        "generated file path must stay under output directory: \"../escape.ts\""
    );
    assert!(!dir.exists());
}
