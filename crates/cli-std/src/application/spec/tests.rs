use super::*;
use anyhow::anyhow;
use std::cell::RefCell;
use std::path::PathBuf;

fn openapi() -> anyhow::Result<String> {
    Ok("{\"openapi\":\"3.1.0\"}".to_string())
}

fn yaml() -> anyhow::Result<String> {
    Ok("openapi: 3.1.0".to_string())
}

fn json_schema() -> anyhow::Result<String> {
    Ok("{\"$defs\":{}}".to_string())
}

fn json_schema_array() -> anyhow::Result<String> {
    Ok("[1]".to_string())
}

fn routes() -> anyhow::Result<String> {
    Ok("routes: []".to_string())
}

fn routes_json() -> anyhow::Result<String> {
    Ok("{\"routes\":[]}".to_string())
}

fn shapes() -> anyhow::Result<String> {
    Ok("shapes doc".to_string())
}

fn boom() -> anyhow::Result<String> {
    Err(anyhow!("boom"))
}

fn text(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes).expect("utf-8 output")
}

fn print(params: &SpecParams, opts: &PrintOptions) -> (i32, String, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = run_print(params, opts, &mut out, &mut err);
    (code, text(out), text(err))
}

fn gen_params(accepts_target: bool, accepts_http: bool, end: GenEnd) -> GenParams {
    GenParams {
        factory_name: "createClient",
        accepts_target,
        accepts_http,
        end,
    }
}

fn request(lang: Lang) -> GenRequest {
    GenRequest {
        lang,
        out: PathBuf::from("out"),
        target: None,
        http: None,
        factory_name: "x",
    }
}

fn gen_run(
    params: &SpecParams,
    req: &GenRequest,
    cb: &dyn Fn(&GenRequest, &str) -> anyhow::Result<GenOutcome>,
) -> (i32, String, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = run_gen(params, req, cb, &mut out, &mut err);
    (code, text(out), text(err))
}

fn outcome_with_files() -> GenOutcome {
    GenOutcome {
        files: vec![PathBuf::from("out").join("client.ts")],
        manifest: None,
        target: None,
    }
}

// Print mode

#[test]
fn print_openapi_is_raw_document_and_exit_0() {
    let params = SpecParams::new(openapi);
    let (code, out, err) = print(&params, &PrintOptions::new(Format::OpenApi));
    assert_eq!(out, "{\"openapi\":\"3.1.0\"}\n");
    assert_eq!(err, "");
    assert_eq!(code, EXIT_OK);
    assert_eq!(code, 0);
}

#[test]
fn print_does_not_double_the_newline() {
    fn with_newline() -> anyhow::Result<String> {
        Ok("{}\n".to_string())
    }
    let params = SpecParams::new(with_newline);
    let (code, out, _) = print(&params, &PrintOptions::new(Format::OpenApi));
    assert_eq!(out, "{}\n");
    assert_eq!(code, EXIT_OK);
}

#[test]
fn print_yaml_json_schema_and_routes_use_their_own_source() {
    let params = SpecParams::new(openapi)
        .with_openapi_yaml(yaml)
        .with_json_schema(json_schema)
        .with_routes(routes);

    let (code, out, _) = print(&params, &PrintOptions::new(Format::OpenApiYaml));
    assert_eq!((code, out.as_str()), (0, "openapi: 3.1.0\n"));

    let (code, out, _) = print(&params, &PrintOptions::new(Format::JsonSchema));
    assert_eq!((code, out.as_str()), (0, "{\"$defs\":{}}\n"));

    let (code, out, _) = print(&params, &PrintOptions::new(Format::Routes));
    assert_eq!((code, out.as_str()), (0, "routes: []\n"));
}

#[test]
fn print_line_marker_appends_next_done() {
    let params = SpecParams::new(openapi).with_next_marker(NextMarker::Line("done"));
    let (code, out, err) = print(&params, &PrintOptions::new(Format::OpenApi));
    assert_eq!(out, "{\"openapi\":\"3.1.0\"}\nnext: done\n");
    assert_eq!(err, "");
    assert_eq!(code, 0);
}

#[test]
fn print_embedded_marker_wraps_openapi_json() {
    let params = SpecParams::new(openapi).with_next_marker(NextMarker::Embedded {
        wrap_openapi_as: "openapi",
        value: "done",
    });
    let (code, out, _) = print(&params, &PrintOptions::new(Format::OpenApi));
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).expect("json");
    assert_eq!(v["openapi"]["openapi"], "3.1.0");
    assert_eq!(v["next"], "done");
}

#[test]
fn print_embedded_marker_adds_next_to_json_schema_object() {
    let params = SpecParams::new(openapi)
        .with_json_schema(json_schema)
        .with_next_marker(NextMarker::Embedded {
            wrap_openapi_as: "openapi",
            value: "done",
        });
    let (code, out, _) = print(&params, &PrintOptions::new(Format::JsonSchema));
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).expect("json");
    assert!(v.get("$defs").is_some());
    assert_eq!(v["next"], "done");
    assert_eq!(v.as_object().unwrap().keys().next_back().unwrap(), "next");
}

#[test]
fn print_embedded_marker_adds_next_to_routes_object() {
    let params = SpecParams::new(openapi)
        .with_routes(routes_json)
        .with_next_marker(NextMarker::Embedded {
            wrap_openapi_as: "openapi",
            value: "done",
        });
    let (code, out, _) = print(&params, &PrintOptions::new(Format::Routes));
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).expect("json");
    assert!(v.get("routes").is_some());
    assert_eq!(v["next"], "done");
}

#[test]
fn print_embedded_marker_yaml_comment() {
    let params = SpecParams::new(openapi)
        .with_openapi_yaml(yaml)
        .with_next_marker(NextMarker::Embedded {
            wrap_openapi_as: "openapi",
            value: "done",
        });
    let (code, out, _) = print(&params, &PrintOptions::new(Format::OpenApiYaml));
    assert_eq!(code, 0);
    assert_eq!(out, "openapi: 3.1.0\n# next: done\n");
}

#[test]
fn print_embedded_marker_rejects_non_object() {
    let params = SpecParams::new(openapi)
        .with_json_schema(json_schema_array)
        .with_next_marker(NextMarker::Embedded {
            wrap_openapi_as: "openapi",
            value: "done",
        });
    let (code, out, err) = print(&params, &PrintOptions::new(Format::JsonSchema));
    assert_eq!(code, EXIT_ERROR);
    assert_eq!(out, "");
    assert!(err.starts_with("Error: "), "got {err:?}");
}

#[test]
fn print_catalog_overrides_format() {
    let params = SpecParams::new(openapi).with_catalog("shapes", shapes);
    let (code, out, _) = print(
        &params,
        &PrintOptions::new(Format::OpenApi).with_catalog("shapes"),
    );
    assert_eq!(code, 0);
    assert_eq!(out, "shapes doc\n");
}

#[test]
fn print_unknown_catalog_is_usage_error() {
    let params = SpecParams::new(openapi).with_catalog("shapes", shapes);
    let (code, out, err) = print(
        &params,
        &PrintOptions::new(Format::OpenApi).with_catalog("fields"),
    );
    assert_eq!(code, EXIT_USAGE);
    assert_eq!(out, "");
    assert!(err.contains("unsupported catalog"), "got {err:?}");
}

#[test]
fn print_format_without_source_is_usage_error() {
    // defer has no JSON Schema source: the format must be refused, not guessed.
    let params = SpecParams::new(openapi);
    let (code, out, err) = print(&params, &PrintOptions::new(Format::JsonSchema));
    assert_eq!(code, EXIT_USAGE);
    assert_eq!(out, "");
    assert!(err.contains("unsupported format"), "got {err:?}");
}

#[test]
fn print_source_error_is_exit_1() {
    let params = SpecParams::new(boom);
    let (code, out, err) = print(&params, &PrintOptions::new(Format::OpenApi));
    assert_eq!(code, EXIT_ERROR);
    assert_eq!(out, "");
    assert_eq!(err, "Error: boom\n");
}

// Format parsing: each per-service difference is a parameter

#[test]
fn parse_format_default_name_is_a_parameter() {
    let params = SpecParams::new(openapi);
    assert_eq!(params.default_format_name(), "openapi");
    assert_eq!(params.parse_format("openapi"), Ok(Format::OpenApi));
    assert!(params.parse_format("openapi-json").is_err());

    let sift = SpecParams::new(openapi).with_openapi_name("openapi-json");
    assert_eq!(sift.default_format_name(), "openapi-json");
    assert_eq!(sift.parse_format("openapi-json"), Ok(Format::OpenApi));
    assert!(sift.parse_format("openapi").is_err());
}

#[test]
fn parse_format_aliases_are_a_parameter() {
    let without = SpecParams::new(openapi).with_openapi_yaml(yaml);
    assert!(without.parse_format("yaml").is_err());
    assert_eq!(
        without.parse_format("openapi-yaml"),
        Ok(Format::OpenApiYaml)
    );

    let with = SpecParams::new(openapi)
        .with_openapi_yaml(yaml)
        .with_yaml_aliases(&["yaml", "openapi.yaml"]);
    assert_eq!(with.parse_format("yaml"), Ok(Format::OpenApiYaml));
    assert_eq!(with.parse_format("openapi.yaml"), Ok(Format::OpenApiYaml));
    assert_eq!(with.parse_format("openapi-yaml"), Ok(Format::OpenApiYaml));
}

#[test]
fn parse_format_set_is_a_parameter() {
    let bare = SpecParams::new(openapi);
    assert_eq!(
        bare.parse_format("routes"),
        Err("unsupported format `routes`".to_string())
    );
    assert!(bare.parse_format("json-schema").is_err());
    assert!(bare.parse_format("openapi-yaml").is_err());

    let full = SpecParams::new(openapi)
        .with_routes(routes)
        .with_json_schema(json_schema);
    assert_eq!(full.parse_format("routes"), Ok(Format::Routes));
    assert_eq!(full.parse_format("json-schema"), Ok(Format::JsonSchema));
}

// Gen mode

#[test]
fn gen_entry_file_end_and_manifest_and_target() {
    let params = SpecParams::new(openapi).with_gen(gen_params(true, true, GenEnd::EntryFile));
    let cb = |_: &GenRequest, _: &str| -> anyhow::Result<GenOutcome> {
        Ok(GenOutcome {
            files: vec![
                PathBuf::from("out").join("types.ts"),
                PathBuf::from("out").join("client.ts"),
                PathBuf::from("out").join("index.ts"),
            ],
            manifest: Some(PathBuf::from("out").join(".openapi-codegen.json")),
            target: Some(("typescript-5.0".to_string(), "5.0".to_string())),
        })
    };
    let (code, out, err) = gen_run(&params, &request(Lang::Ts), &cb);
    assert_eq!(
        out,
        "generated out/types.ts\n\
         generated out/client.ts\n\
         generated out/index.ts\n\
         generated out/.openapi-codegen.json\n\
         target: typescript-5.0 (minimum ts 5.0)\n\
         next: out/index.ts\n"
    );
    assert_eq!(err, "");
    assert_eq!(code, 0);
}

#[test]
fn gen_entry_file_per_language() {
    assert_eq!(Lang::Ts.name(), "ts");
    assert_eq!(Lang::Py.name(), "py");
    assert_eq!(Lang::Rust.name(), "rust");

    let params = SpecParams::new(openapi).with_gen(gen_params(true, true, GenEnd::EntryFile));
    let cb = |_: &GenRequest, _: &str| -> anyhow::Result<GenOutcome> { Ok(outcome_with_files()) };

    let (_, out, _) = gen_run(&params, &request(Lang::Py), &cb);
    assert!(out.ends_with("next: out/__init__.py\n"), "got {out:?}");

    let (_, out, _) = gen_run(&params, &request(Lang::Rust), &cb);
    assert!(out.ends_with("next: out/mod.rs\n"), "got {out:?}");
}

#[test]
fn gen_done_end_without_manifest_or_target() {
    let params = SpecParams::new(openapi).with_gen(gen_params(false, false, GenEnd::Done));
    let cb = |_: &GenRequest, _: &str| -> anyhow::Result<GenOutcome> { Ok(outcome_with_files()) };
    let (code, out, err) = gen_run(&params, &request(Lang::Ts), &cb);
    assert_eq!(out, "generated out/client.ts\nnext: done\n");
    assert_eq!(err, "");
    assert_eq!(code, 0);
}

#[test]
fn gen_factory_name_comes_from_params() {
    let seen = RefCell::new(String::new());
    let cb = |r: &GenRequest, _: &str| -> anyhow::Result<GenOutcome> {
        seen.borrow_mut().push_str(r.factory_name);
        Ok(GenOutcome::default())
    };

    let defer = SpecParams::new(openapi).with_gen(GenParams {
        factory_name: "createDeferClient",
        accepts_target: false,
        accepts_http: false,
        end: GenEnd::Done,
    });
    let (code, _, _) = gen_run(&defer, &request(Lang::Ts), &cb);
    assert_eq!(code, 0);
    assert_eq!(seen.borrow().as_str(), "createDeferClient");

    seen.borrow_mut().clear();
    let tape = SpecParams::new(openapi).with_gen(gen_params(true, true, GenEnd::EntryFile));
    let (code, _, _) = gen_run(&tape, &request(Lang::Ts), &cb);
    assert_eq!(code, 0);
    assert_eq!(seen.borrow().as_str(), "createClient");
}

#[test]
fn gen_passes_the_openapi_json_to_the_callback() {
    let seen = RefCell::new(String::new());
    let cb = |_: &GenRequest, doc: &str| -> anyhow::Result<GenOutcome> {
        seen.borrow_mut().push_str(doc);
        Ok(GenOutcome::default())
    };
    let params = SpecParams::new(openapi).with_gen(gen_params(true, true, GenEnd::Done));
    let (code, _, _) = gen_run(&params, &request(Lang::Ts), &cb);
    assert_eq!(code, 0);
    assert_eq!(seen.borrow().as_str(), "{\"openapi\":\"3.1.0\"}");
}

#[test]
fn gen_rejects_target_when_not_accepted() {
    let params = SpecParams::new(openapi).with_gen(gen_params(false, true, GenEnd::Done));
    let cb =
        |_: &GenRequest, _: &str| -> anyhow::Result<GenOutcome> { panic!("callback must not run") };
    let mut req = request(Lang::Ts);
    req.target = Some("ts".to_string());
    let (code, out, err) = gen_run(&params, &req, &cb);
    assert_eq!(code, EXIT_USAGE);
    assert_eq!(out, "");
    assert!(err.contains("--target"), "got {err:?}");
}

#[test]
fn gen_rejects_http_when_not_accepted() {
    let params = SpecParams::new(openapi).with_gen(gen_params(true, false, GenEnd::Done));
    let cb =
        |_: &GenRequest, _: &str| -> anyhow::Result<GenOutcome> { panic!("callback must not run") };
    let mut req = request(Lang::Ts);
    req.http = Some("axios".to_string());
    let (code, out, err) = gen_run(&params, &req, &cb);
    assert_eq!(code, EXIT_USAGE);
    assert_eq!(out, "");
    assert!(err.contains("--http"), "got {err:?}");
}

#[test]
fn gen_without_gen_params_is_usage_error() {
    let params = SpecParams::new(openapi);
    let cb =
        |_: &GenRequest, _: &str| -> anyhow::Result<GenOutcome> { panic!("callback must not run") };
    let (code, out, err) = gen_run(&params, &request(Lang::Ts), &cb);
    assert_eq!(code, EXIT_USAGE);
    assert_eq!(out, "");
    assert!(err.contains("not configured"), "got {err:?}");
}

#[test]
fn gen_callback_error_is_exit_1() {
    let params = SpecParams::new(openapi).with_gen(gen_params(true, true, GenEnd::Done));
    let cb =
        |_: &GenRequest, _: &str| -> anyhow::Result<GenOutcome> { Err(anyhow!("write failed")) };
    let (code, out, err) = gen_run(&params, &request(Lang::Ts), &cb);
    assert_eq!(code, EXIT_ERROR);
    assert_eq!(out, "");
    assert_eq!(err, "Error: write failed\n");
}

#[test]
fn gen_source_error_is_exit_1() {
    let params = SpecParams::new(boom).with_gen(gen_params(true, true, GenEnd::Done));
    let cb =
        |_: &GenRequest, _: &str| -> anyhow::Result<GenOutcome> { panic!("callback must not run") };
    let (code, out, err) = gen_run(&params, &request(Lang::Ts), &cb);
    assert_eq!(code, EXIT_ERROR);
    assert_eq!(out, "");
    assert_eq!(err, "Error: boom\n");
}
