use super::*;

#[test]
fn python_profiles_record_requirements_and_use_their_supported_typing_syntax() {
    let mut opts = full_opts();
    opts.lang = Lang::Py;
    opts.emit_hooks = false;
    for (target, version, alias) in [
        (PythonTarget::Py311, "3.11", "Label = str"),
        (PythonTarget::Py312, "3.12", "type Label = str"),
        (PythonTarget::Py313, "3.13", "type Label = str"),
        (PythonTarget::Py314, "3.14", "type Label = str"),
    ] {
        let out =
            generate_for_target(TARGET_PROFILE_SPEC, &opts, TargetProfile::Python(target)).unwrap();

        assert_eq!(out.target, Some(TargetProfile::Python(target)));
        let requirements = out.requirements.expect("profile requirements");
        assert_eq!(requirements.minimum_version, version);
        assert_eq!(requirements.runtime_dependencies, &["pydantic>=2"]);
        assert!(content(&out, "models.py").contains(alias));
        assert!(!content(&out, "models.py").contains("Optional["));
    }
}

#[test]
fn python_profiles_compile_with_each_available_target_interpreter() {
    let mut opts = full_opts();
    opts.lang = Lang::Py;
    opts.emit_hooks = false;

    for (interpreter, target) in [
        ("python3.11", PythonTarget::Py311),
        ("python3.12", PythonTarget::Py312),
        ("python3.13", PythonTarget::Py313),
        ("python3.14", PythonTarget::Py314),
    ] {
        let available = Command::new(interpreter)
            .arg("--version")
            .output()
            .is_ok_and(|status| status.status.success());
        if !available {
            continue;
        }
        let output =
            generate_for_target(TARGET_PROFILE_SPEC, &opts, TargetProfile::Python(target)).unwrap();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "openapi-codegen-profile-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        let mut paths = Vec::new();
        for file in &output.files {
            let path = dir.join(&file.rel_path);
            fs::write(&path, &file.contents).unwrap();
            paths.push(path);
        }
        let result = Command::new(interpreter)
            .arg("-m")
            .arg("py_compile")
            .args(&paths)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{interpreter} cannot compile {} output\n{}",
            output.target.expect("profile target").id(),
            String::from_utf8_lossy(&result.stderr)
        );
        let _ = fs::remove_dir_all(dir);
    }
}

#[test]
fn rust_2024_profile_escapes_gen_field_without_changing_rust_2021() {
    let mut opts = full_opts();
    opts.lang = Lang::Rust;
    opts.emit_hooks = false;
    let rust_2021 = generate_for_target(
        TARGET_PROFILE_SPEC,
        &opts,
        TargetProfile::Rust(RustTarget::Rust2021),
    )
    .unwrap();
    let rust_2024 = generate_for_target(
        TARGET_PROFILE_SPEC,
        &opts,
        TargetProfile::Rust(RustTarget::Rust2024),
    )
    .unwrap();

    assert!(content(&rust_2021, "models.rs").contains("pub gen: String,"));
    assert!(content(&rust_2024, "models.rs").contains("pub gen_: String,"));
    assert!(content(&rust_2024, "models.rs").contains("#[serde(rename = \"gen\")]"));
    assert_eq!(
        rust_2024
            .requirements
            .expect("profile requirements")
            .minimum_version,
        "1.85"
    );
}

#[test]
fn target_profile_must_match_the_requested_language() {
    let error = generate_for_target(
        MINIMAL,
        &full_opts(),
        TargetProfile::Python(PythonTarget::Py311),
    )
    .unwrap_err();
    assert!(error.to_string().contains("python-3.11"));
}
