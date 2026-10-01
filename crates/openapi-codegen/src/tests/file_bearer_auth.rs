use super::*;

#[test]
fn file_bearer_auth_is_generic_and_validated_at_generation_time() {
    let auth = FileBearerAuth::new(
        "/private/tmp/service-token",
        ".example.internal",
        [FileBearerScheme::Https, FileBearerScheme::Http],
    )
    .unwrap();
    assert_eq!(auth.hostname_suffix(), ".example.internal");
    assert_eq!(
        auth.schemes().collect::<Vec<_>>(),
        vec![FileBearerScheme::Http, FileBearerScheme::Https]
    );

    for suffix in [
        "",
        "example.internal",
        ".example.internal.",
        ".Example.internal",
        ".bad..internal",
        ".-bad.internal",
    ] {
        assert!(
            FileBearerAuth::new(
                "/private/tmp/service-token",
                suffix,
                [FileBearerScheme::Https]
            )
            .is_err(),
            "accepted invalid suffix {suffix:?}"
        );
    }
    assert!(FileBearerAuth::new("", ".example.internal", [FileBearerScheme::Https]).is_err());
    assert!(FileBearerAuth::new("/private/tmp/service-token", ".example.internal", []).is_err());
}

#[test]
fn legacy_generation_stays_opted_out_and_targeted_auth_keeps_manifest() {
    let token_path = "/private/tmp/file-bearer-opt-in-canary";
    let auth = FileBearerAuth::new(
        token_path,
        ".example.internal",
        [FileBearerScheme::Http, FileBearerScheme::Https],
    )
    .unwrap();

    for (lang, target, auth_file) in [
        (
            Lang::Ts,
            TargetProfile::TypeScript(TypeScriptTarget::Ts50),
            "runtime.ts",
        ),
        (
            Lang::Py,
            TargetProfile::Python(PythonTarget::Py311),
            "client.py",
        ),
        (
            Lang::Rust,
            TargetProfile::Rust(RustTarget::Rust2021),
            "client.rs",
        ),
    ] {
        let mut opts = full_opts();
        opts.lang = lang;
        opts.target = Some(target);
        opts.emit_hooks = matches!(lang, Lang::Ts);

        let legacy = generate(MINIMAL, &opts).unwrap();
        assert!(legacy
            .files
            .iter()
            .all(|file| !file.contents.contains(token_path)));

        let extended =
            generate_for_target_with_file_bearer_auth(MINIMAL, &opts, target, &auth).unwrap();
        assert_eq!(extended.target, Some(target));
        assert!(extended.manifest().is_some());
        assert!(content(&extended, auth_file).contains(token_path));
        for (before, after) in legacy.files.iter().zip(&extended.files) {
            if before.rel_path != auth_file {
                assert_eq!(before.rel_path, after.rel_path);
                assert_eq!(
                    before.contents, after.contents,
                    "{} changed outside auth runtime",
                    before.rel_path
                );
            }
        }
    }
}
