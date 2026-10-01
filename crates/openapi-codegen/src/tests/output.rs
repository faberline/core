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
    assert_eq!(manifest.schema_version, 1);
    assert_eq!(manifest.target, "python-3.14");
    assert_eq!(manifest.language, "python");
    assert_eq!(manifest.minimum_version, "3.14");
    assert!(dir.join("models.py").is_file());
    let _ = fs::remove_dir_all(dir);
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
