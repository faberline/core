use super::*;
use crate::domain::check::lint_config::LintConfig;
use crate::domain::lint::checker::Checker;

fn make(s: &str) -> ParsedFile {
    ParsedFile::line_based(s.to_string(), Language::Dockerfile)
}
fn codes(d: &[Diagnostic]) -> Vec<&str> {
    d.iter().map(|d| d.code.as_str()).collect()
}
fn count(d: &[Diagnostic], code: &str) -> usize {
    d.iter().filter(|d| d.code == code).count()
}

#[test]
fn dk002_ignores_earlier_stage_alias() {
    let d = DockerfileChecker::new().check(
        &make("FROM rust:1.82 AS build\nFROM build\n"),
        &LintConfig::default(),
    );
    assert_eq!(
        count(&d, "DK002"),
        0,
        "expected no DK002, got {:?}",
        codes(&d)
    );
}

#[test]
fn dk002_still_reports_untagged_image() {
    let d = DockerfileChecker::new().check(&make("FROM rust\n"), &LintConfig::default());
    assert_eq!(
        count(&d, "DK002"),
        1,
        "expected one DK002, got {:?}",
        codes(&d)
    );
}

#[test]
fn dk002_still_reports_latest_tag() {
    let d = DockerfileChecker::new().check(&make("FROM rust:latest\n"), &LintConfig::default());
    assert_eq!(
        count(&d, "DK002"),
        1,
        "expected one DK002, got {:?}",
        codes(&d)
    );
}

#[test]
fn dk004_ignores_intermediate_stage() {
    let d = DockerfileChecker::new().check(
        &make(
            "FROM rust:1.82 AS build\nCOPY . /src\nFROM debian:12\nCOPY --chown=app:app app /app\n",
        ),
        &LintConfig::default(),
    );
    assert_eq!(
        count(&d, "DK004"),
        0,
        "expected no DK004, got {:?}",
        codes(&d)
    );
}

#[test]
fn dk004_ignores_copy_from_stage() {
    let d = DockerfileChecker::new().check(
        &make("FROM rust:1.82 AS build\nFROM debian:12\nCOPY --from=build /out/app /app\n"),
        &LintConfig::default(),
    );
    assert_eq!(
        count(&d, "DK004"),
        0,
        "expected no DK004, got {:?}",
        codes(&d)
    );
}

#[test]
fn dk004_still_reports_final_stage_copy_without_chown() {
    let d = DockerfileChecker::new().check(
        &make("FROM debian:12\nCOPY app /app\n"),
        &LintConfig::default(),
    );
    assert_eq!(
        count(&d, "DK004"),
        1,
        "expected one DK004, got {:?}",
        codes(&d)
    );
}
