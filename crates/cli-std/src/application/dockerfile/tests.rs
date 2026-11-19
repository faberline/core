use super::*;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const SRC: &str = "# SPEC-MANAGED: x\nFROM rust:1 AS build\n# CODEGEN-BEGIN\nRUN cargo build\n# CODEGEN-END\n# HANDWRITE-BEGIN keep\nENTRYPOINT [\"/usr/local/bin/tape\"]\n";
const REL: &str = "# SPEC-MANAGED: x\n#   docker build -f Dockerfile.release -t tape:0.0.0 \\\n#     --build-arg TAPE_VERSION=tape@0.0.0 .\nARG TAPE_VERSION=tape@0.0.0\nFROM scratch\n";
const SIFT_SRC: &str =
    "# SPEC-MANAGED: keep\n# HANDWRITE-BEGIN a\nFROM rust:1\n# HANDWRITE-END\nCMD [\"sift\"]\n";
const SIFT_REL: &str =
    "# HANDWRITE-BEGIN a\nFROM scratch\nARG SIFT_VERSION=REPLACE_ME\n# HANDWRITE-END\n";

const TAPE_SOURCE_OUT: &str = "FROM rust:1 AS build\nRUN cargo build\n# HANDWRITE-BEGIN keep\nENTRYPOINT [\"/usr/local/bin/tape\"]\n";
const TAPE_RELEASE_OUT: &str = "#   docker build -f Dockerfile.release -t tape:0.4.7 \\\n#     --build-arg TAPE_VERSION=tape@0.4.7 .\nARG TAPE_VERSION=tape@0.4.7\nFROM scratch\n";

fn tape() -> DockerfileParams {
    DockerfileParams::new("tape", "TAPE_VERSION", SRC, REL, "0.4.7")
}

fn sift() -> DockerfileParams {
    DockerfileParams::new("sift", "SIFT_VERSION", SIFT_SRC, SIFT_REL, "1.2.3")
        .with_default_variant(Variant::Release)
        .with_version_form(VersionForm::Bare)
        .with_release_rewrite(ReleaseRewrite::Placeholder)
        .with_marker_rule(MarkerRule::Handwrite)
        .with_out_path_rule(OutPathRule::ByFileSystem)
        .with_next(Next::Lines {
            stdout: "docker build -f Dockerfile -t sift:dev .",
            file_prefix: "kubectl apply -f ",
        })
}

fn render(params: &DockerfileParams, req: &DockerfileRequest) -> (i32, String, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = run_render(params, req, &mut out, &mut err);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

fn release(version: Option<&str>) -> DockerfileRequest {
    DockerfileRequest {
        variant: Some(Variant::Release),
        version: version.map(str::to_string),
        out: None,
    }
}

fn to_file(variant: Option<Variant>, path: PathBuf) -> DockerfileRequest {
    DockerfileRequest {
        variant,
        version: None,
        out: Some(path),
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "cli-std-dockerfile-{name}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn source_render_strips_ownership_markers() {
    let (code, out, err) = render(&tape(), &DockerfileRequest::default());
    assert_eq!(code, EXIT_OK);
    assert_eq!(err, "");
    assert_eq!(out, TAPE_SOURCE_OUT);
}

#[test]
fn release_render_rewrites_comments_and_arg_with_the_fallback_version() {
    let (code, out, _) = render(&tape(), &release(None));
    assert_eq!(code, EXIT_OK);
    assert_eq!(out, TAPE_RELEASE_OUT);
}

#[test]
fn release_version_flag_and_prefixed_version_give_the_same_output() {
    let (ca, a, _) = render(&tape(), &release(Some("9.9.9")));
    let (cb, b, _) = render(&tape(), &release(Some("tape@9.9.9")));
    assert_eq!((ca, cb), (EXIT_OK, EXIT_OK));
    assert_eq!(a, b);
    assert!(a.contains("ARG TAPE_VERSION=tape@9.9.9\n"));
    assert!(a.contains("-t tape:9.9.9 \\\n"));
}

#[test]
fn blank_version_falls_back_to_the_package_version() {
    let (code, out, _) = render(&tape(), &release(Some("  ")));
    assert_eq!(code, EXIT_OK);
    assert_eq!(out, TAPE_RELEASE_OUT);
}

#[test]
fn source_variant_ignores_the_version() {
    let req = DockerfileRequest {
        variant: Some(Variant::Source),
        version: Some("9.9.9".to_string()),
        out: None,
    };
    let (code, out, _) = render(&tape(), &req);
    assert_eq!(code, EXIT_OK);
    assert_eq!(out, TAPE_SOURCE_OUT);
}

#[test]
fn arg_only_rewrite_keeps_comment_lines() {
    let params = DockerfileParams::new(
        "defer",
        "DEFER_VERSION",
        "",
        "# SPEC-MANAGED: x\n#   docker build -f Dockerfile.release -t defer:0.0.0 \\\nARG DEFER_VERSION=defer@0.0.0\nENTRYPOINT [\"/usr/local/bin/defer\"]",
        "0.5.0",
    )
    .with_release_rewrite(ReleaseRewrite::ArgOnly);
    let (code, out, _) = render(&params, &release(Some("1.0.0")));
    assert_eq!(code, EXIT_OK);
    assert_eq!(
        out,
        "#   docker build -f Dockerfile.release -t defer:0.0.0 \\\nARG DEFER_VERSION=defer@1.0.0\nENTRYPOINT [\"/usr/local/bin/defer\"]\n"
    );
}

#[test]
fn sift_release_uses_the_bare_version_and_handwrite_markers() {
    let (code, out, err) = render(&sift(), &DockerfileRequest::default());
    assert_eq!(code, EXIT_OK);
    assert_eq!(err, "");
    assert_eq!(
        out,
        "FROM scratch\nARG SIFT_VERSION=1.2.3\nnext: docker build -f Dockerfile -t sift:dev .\n"
    );
}

#[test]
fn sift_version_drops_the_project_prefix() {
    for version in ["sift@2.0.0", "2.0.0"] {
        let req = DockerfileRequest {
            variant: None,
            version: Some(version.to_string()),
            out: None,
        };
        let (code, out, _) = render(&sift(), &req);
        assert_eq!(code, EXIT_OK);
        assert!(out.contains("ARG SIFT_VERSION=2.0.0\n"));
        assert!(!out.contains("sift@"));
    }
}

#[test]
fn sift_blank_version_is_an_error() {
    let req = DockerfileRequest {
        variant: None,
        version: Some(" ".to_string()),
        out: None,
    };
    let (code, out, err) = render(&sift(), &req);
    assert_eq!(code, EXIT_ERROR);
    assert_eq!(out, "");
    assert_eq!(err, "Error: release Dockerfile version must not be empty\n");
}

#[test]
fn sift_source_keeps_spec_managed_lines() {
    let req = DockerfileRequest {
        variant: Some(Variant::Source),
        version: None,
        out: None,
    };
    let (code, out, _) = render(&sift(), &req);
    assert_eq!(code, EXIT_OK);
    assert_eq!(
        out,
        "# SPEC-MANAGED: keep\nFROM rust:1\nCMD [\"sift\"]\nnext: docker build -f Dockerfile -t sift:dev .\n"
    );
}

#[test]
fn next_none_prints_no_next_line() {
    let (_, out, _) = render(&tape(), &DockerfileRequest::default());
    assert!(!out.contains("next:"));

    let dir = temp_dir("next-none");
    let path = dir.join("Dockerfile.out");
    let (code, out, _) = render(&tape(), &to_file(None, path.clone()));
    assert_eq!(code, EXIT_OK);
    assert_eq!(out, format!("wrote {}\n", path.display()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn next_done_on_file_only() {
    let params = tape().with_next(Next::DoneOnFile);
    let (_, out, _) = render(&params, &DockerfileRequest::default());
    assert!(!out.contains("next:"));

    let dir = temp_dir("done-on-file");
    let path = dir.join("out.txt");
    let (code, out, _) = render(&params, &to_file(None, path.clone()));
    assert_eq!(code, EXIT_OK);
    assert_eq!(out, format!("wrote {}\nnext: done\n", path.display()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn next_done_always() {
    let params = tape().with_next(Next::DoneAlways);
    let (_, out, _) = render(&params, &DockerfileRequest::default());
    assert!(out.ends_with("next: done\n"));

    let dir = temp_dir("done-always");
    let path = dir.join("out.txt");
    let (code, out, _) = render(&params, &to_file(None, path.clone()));
    assert_eq!(code, EXIT_OK);
    assert_eq!(out, format!("wrote {}\nnext: done\n", path.display()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn next_lines_use_the_file_prefix_after_a_write() {
    let dir = temp_dir("next-lines");
    let path = dir.join("Dockerfile.release");
    let (code, out, _) = render(&sift(), &to_file(None, path.clone()));
    assert_eq!(code, EXIT_OK);
    assert_eq!(
        out,
        format!(
            "wrote {}\nnext: kubectl apply -f {}\n",
            path.display(),
            path.display()
        )
    );
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "FROM scratch\nARG SIFT_VERSION=1.2.3\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn out_by_extension_treats_a_name_without_extension_as_a_directory() {
    let dir = temp_dir("by-extension-dir");
    let build = dir.join("build");

    let (code, _, _) = render(&tape(), &to_file(Some(Variant::Source), build.clone()));
    assert_eq!(code, EXIT_OK);
    assert!(build.is_dir());
    assert_eq!(
        std::fs::read_to_string(build.join("Dockerfile")).unwrap(),
        TAPE_SOURCE_OUT
    );

    let (code, _, _) = render(&tape(), &to_file(Some(Variant::Release), build.clone()));
    assert_eq!(code, EXIT_OK);
    assert_eq!(
        std::fs::read_to_string(build.join("Dockerfile.release")).unwrap(),
        TAPE_RELEASE_OUT
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn out_by_file_system_treats_a_name_without_extension_as_a_file() {
    let dir = temp_dir("by-filesystem");
    let build = dir.join("build");
    let (code, _, _) = render(&sift(), &to_file(None, build.clone()));
    assert_eq!(code, EXIT_OK);
    assert!(build.is_file());
    assert_eq!(
        std::fs::read_to_string(&build).unwrap(),
        "FROM scratch\nARG SIFT_VERSION=1.2.3\n"
    );

    let existing = dir.join("existing");
    std::fs::create_dir_all(&existing).unwrap();
    let (code, _, _) = render(&sift(), &to_file(None, existing.clone()));
    assert_eq!(code, EXIT_OK);
    assert_eq!(
        std::fs::read_to_string(existing.join("Dockerfile.release")).unwrap(),
        "FROM scratch\nARG SIFT_VERSION=1.2.3\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn out_creates_missing_parent_directories() {
    let dir = temp_dir("parents");
    let path = dir.join("a").join("b").join("Dockerfile.out");
    let (code, _, _) = render(&tape(), &to_file(None, path.clone()));
    assert_eq!(code, EXIT_OK);
    assert!(path.is_file());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn written_file_equals_the_stdout_render() {
    let dir = temp_dir("file-equals-stdout");
    let path = dir.join("Dockerfile.release");
    let (_, stdout, _) = render(&tape(), &release(None));
    let (code, _, _) = render(
        &tape(),
        &DockerfileRequest {
            variant: Some(Variant::Release),
            version: None,
            out: Some(path.clone()),
        },
    );
    assert_eq!(code, EXIT_OK);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), stdout);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_error_is_exit_1_without_wrote_line() {
    let dir = temp_dir("write-error");
    let file = dir.join("f");
    std::fs::write(&file, "not a directory").unwrap();
    let (code, out, err) = render(&tape(), &to_file(None, file.join("x.txt")));
    assert_eq!(code, EXIT_ERROR);
    assert_eq!(out, "");
    assert!(err.starts_with("Error: "));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn default_variant_accessor() {
    assert_eq!(tape().default_variant(), Variant::Source);
    assert_eq!(sift().default_variant(), Variant::Release);
}
