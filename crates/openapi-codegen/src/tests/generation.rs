use super::*;

#[test]
fn generates_all_files() {
    let out = generate(MINIMAL, &full_opts()).unwrap();
    let names: Vec<&str> = out.files.iter().map(|f| f.rel_path.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "types.ts",
            "runtime.ts",
            "client.ts",
            "hooks.ts",
            "index.ts"
        ]
    );
}

#[test]
fn types_only_skips_client_and_hooks() {
    let mut opts = full_opts();
    opts.emit_client = false;
    opts.emit_hooks = false;
    let out = generate(MINIMAL, &opts).unwrap();
    let names: Vec<&str> = out.files.iter().map(|f| f.rel_path.as_str()).collect();
    assert_eq!(names, vec!["types.ts", "index.ts"]);
}

#[test]
fn deterministic_across_runs() {
    let a = generate(MINIMAL, &full_opts()).unwrap();
    let b = generate(MINIMAL, &full_opts()).unwrap();
    for (fa, fb) in a.files.iter().zip(b.files.iter()) {
        assert_eq!(fa.rel_path, fb.rel_path);
        assert_eq!(fa.contents, fb.contents);
    }
}

#[test]
fn invalid_spec_is_an_error() {
    assert!(generate("{ not json", &full_opts()).is_err());
}

#[test]
fn every_lang_generates_non_empty() {
    for lang in [Lang::Ts, Lang::Py, Lang::Rust] {
        let mut opts = full_opts();
        opts.lang = lang;
        let out = generate(MINIMAL, &opts).expect("emitter runs");
        assert!(!out.files.is_empty(), "{lang:?} produced no files");
    }
}

#[test]
fn custom_client_name() {
    let mut opts = full_opts();
    opts.client_name = "makeApi".to_string();
    let out = generate(MINIMAL, &opts).unwrap();
    let client = out
        .files
        .iter()
        .find(|f| f.rel_path == "client.ts")
        .unwrap();
    assert!(client
        .contents
        .contains("export function makeApi(config: ClientConfig)"));
    assert!(client.contents.contains("ReturnType<typeof makeApi>"));
}

#[test]
fn http_backend_only_changes_runtime() {
    let fetch = generate(MINIMAL, &full_opts()).unwrap();
    let mut axios_opts = full_opts();
    axios_opts.http_client = HttpClient::Axios;
    let axios = generate(MINIMAL, &axios_opts).unwrap();

    // Everything except runtime.ts is byte-identical across backends.
    for name in ["types.ts", "client.ts", "hooks.ts", "index.ts"] {
        assert_eq!(
            content(&fetch, name),
            content(&axios, name),
            "{name} differs across backends"
        );
    }

    // The fetch runtime uses native fetch; the axios runtime imports axios.
    let fetch_rt = content(&fetch, "runtime.ts");
    assert!(fetch_rt.contains("const doFetch = config.fetch ?? fetch;"));
    assert!(!fetch_rt.contains("axios"));
    let axios_rt = content(&axios, "runtime.ts");
    assert!(axios_rt.contains("import axios from \"axios\";"));
    assert!(axios_rt.contains("axios?: AxiosInstance;"));
    assert!(axios_rt.contains("config.axios ?? axios.create()"));
}
