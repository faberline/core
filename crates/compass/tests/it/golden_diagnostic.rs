//! Golden bytes for the diagnostic wire format (compass E2).
//!
//! `Diagnostic` JSON is what the agent output, the JSON reporter and the LSP
//! server emit. These literals pin today's bytes so that later refactors (the
//! `RuleCode` newtype, moved types) cannot change them.

use std::fmt::Debug;
use std::path::PathBuf;

use compass::diagnostic::{QuickFix, TextEdit};
use compass::syntax::Language;
use compass::{
    Diagnostic, DiagnosticCategory, DiagnosticSeverity, FileResult, OutputFormat, Position, Range,
    Reporter,
};
use serde::de::DeserializeOwned;
use serde::Serialize;

/// Encodes `value` to `expected` and decodes `expected` back to `value`.
#[track_caller]
fn pin<T: Serialize + DeserializeOwned + Debug>(value: &T, expected: &str) {
    assert_eq!(serde_json::to_string(value).unwrap(), expected);
    let decoded: T = serde_json::from_str(expected).unwrap();
    assert_eq!(format!("{decoded:?}"), format!("{value:?}"));
}

#[track_caller]
fn pin_text(actual: &str, expected: &str) {
    assert_eq!(actual, expected);
}

fn range(sl: u32, sc: u32, el: u32, ec: u32) -> Range {
    Range::new(Position::new(sl, sc), Position::new(el, ec))
}

fn plain() -> Diagnostic {
    Diagnostic::new(
        range(2, 4, 2, 10),
        DiagnosticSeverity::Warning,
        "PY001",
        DiagnosticCategory::Style,
        "unused import `os`",
    )
}

fn with_fix() -> Diagnostic {
    Diagnostic::error(
        range(0, 0, 0, 5),
        "SEC001",
        DiagnosticCategory::Security,
        "possible SQL injection",
    )
    .with_fix(
        "Use a bound parameter",
        vec![TextEdit {
            range: range(0, 0, 0, 5),
            new_text: "query(?)".to_string(),
        }],
    )
}

#[test]
fn diagnostic_json_bytes_are_pinned() {
    pin(&plain(), "{\"range\":{\"start\":{\"line\":2,\"character\":4},\"end\":{\"line\":2,\"character\":10}},\"severity\":\"Warning\",\"code\":\"PY001\",\"category\":\"Style\",\"message\":\"unused import `os`\"}");
    pin(&with_fix(), "{\"range\":{\"start\":{\"line\":0,\"character\":0},\"end\":{\"line\":0,\"character\":5}},\"severity\":\"Error\",\"code\":\"SEC001\",\"category\":\"Security\",\"message\":\"possible SQL injection\",\"quick_fixes\":[{\"title\":\"Use a bound parameter\",\"edits\":[{\"range\":{\"start\":{\"line\":0,\"character\":0},\"end\":{\"line\":0,\"character\":5}},\"new_text\":\"query(?)\"}]}]}");
}

#[test]
fn every_severity_and_category_json_is_pinned() {
    let severities = [
        DiagnosticSeverity::Error,
        DiagnosticSeverity::Warning,
        DiagnosticSeverity::Information,
        DiagnosticSeverity::Hint,
    ];
    pin(
        &severities,
        "[\"Error\",\"Warning\",\"Information\",\"Hint\"]",
    );
    let categories = [
        DiagnosticCategory::Syntax,
        DiagnosticCategory::Type,
        DiagnosticCategory::Names,
        DiagnosticCategory::Logic,
        DiagnosticCategory::Security,
        DiagnosticCategory::Style,
        DiagnosticCategory::Custom,
    ];
    pin(
        &categories,
        "[\"Syntax\",\"Type\",\"Names\",\"Logic\",\"Security\",\"Style\",\"Custom\"]",
    );
}

#[test]
fn quick_fix_json_bytes_are_pinned() {
    let fix = QuickFix {
        title: "Remove".to_string(),
        edits: vec![TextEdit {
            range: range(1, 0, 2, 0),
            new_text: String::new(),
        }],
    };
    pin(&fix, "{\"title\":\"Remove\",\"edits\":[{\"range\":{\"start\":{\"line\":1,\"character\":0},\"end\":{\"line\":2,\"character\":0}},\"new_text\":\"\"}]}");
}

fn results() -> Vec<FileResult> {
    vec![
        FileResult {
            path: PathBuf::from("src/app.py"),
            language: Language::Python,
            diagnostics: vec![plain(), with_fix()],
        },
        FileResult {
            path: PathBuf::from("src/clean.rs"),
            language: Language::Rust,
            diagnostics: Vec::new(),
        },
    ]
}

#[test]
fn reporter_output_bytes_are_pinned() {
    let results = results();
    let report = |format| Reporter::new(format).generate(&results);
    pin_text(&report(OutputFormat::Json), "{\n  \"files\": [\n    {\n      \"path\": \"src/app.py\",\n      \"language\": \"python\",\n      \"diagnostics\": [\n        {\n          \"range\": {\n            \"start\": {\n              \"line\": 2,\n              \"character\": 4\n            },\n            \"end\": {\n              \"line\": 2,\n              \"character\": 10\n            }\n          },\n          \"severity\": \"Warning\",\n          \"code\": \"PY001\",\n          \"category\": \"Style\",\n          \"message\": \"unused import `os`\"\n        },\n        {\n          \"range\": {\n            \"start\": {\n              \"line\": 0,\n              \"character\": 0\n            },\n            \"end\": {\n              \"line\": 0,\n              \"character\": 5\n            }\n          },\n          \"severity\": \"Error\",\n          \"code\": \"SEC001\",\n          \"category\": \"Security\",\n          \"message\": \"possible SQL injection\",\n          \"quick_fixes\": [\n            {\n              \"title\": \"Use a bound parameter\",\n              \"edits\": [\n                {\n                  \"range\": {\n                    \"start\": {\n                      \"line\": 0,\n                      \"character\": 0\n                    },\n                    \"end\": {\n                      \"line\": 0,\n                      \"character\": 5\n                    }\n                  },\n                  \"new_text\": \"query(?)\"\n                }\n              ]\n            }\n          ]\n        }\n      ]\n    },\n    {\n      \"path\": \"src/clean.rs\",\n      \"language\": \"rust\",\n      \"diagnostics\": []\n    }\n  ],\n  \"summary\": {\n    \"files_checked\": 2,\n    \"files_with_issues\": 1,\n    \"total_errors\": 1,\n    \"total_warnings\": 1\n  }\n}");
    pin_text(&report(OutputFormat::GitHub), "::warning file=src/app.py,line=3,col=5,endLine=3,endColumn=11,title=PY001::unused import `os`\n::error file=src/app.py,line=1,col=1,endLine=1,endColumn=6,title=SEC001::possible SQL injection\n");
    pin_text(&report(OutputFormat::GitLab), "[\n  {\n    \"type\": \"issue\",\n    \"check_name\": \"PY001\",\n    \"description\": \"unused import `os`\",\n    \"content\": {\n      \"body\": \"[PY001] unused import `os`\"\n    },\n    \"categories\": [\n      \"Style\"\n    ],\n    \"location\": {\n      \"path\": \"src/app.py\",\n      \"lines\": {\n        \"begin\": 3,\n        \"end\": 3\n      }\n    },\n    \"severity\": \"major\",\n    \"fingerprint\": \"98b235def6f01d55\"\n  },\n  {\n    \"type\": \"issue\",\n    \"check_name\": \"SEC001\",\n    \"description\": \"possible SQL injection\",\n    \"content\": {\n      \"body\": \"[SEC001] possible SQL injection\"\n    },\n    \"categories\": [\n      \"Style\"\n    ],\n    \"location\": {\n      \"path\": \"src/app.py\",\n      \"lines\": {\n        \"begin\": 1,\n        \"end\": 1\n      }\n    },\n    \"severity\": \"critical\",\n    \"fingerprint\": \"dbb7bb048d28ec21\"\n  }\n]");
    pin_text(&report(OutputFormat::Markdown), "# Lint Report\n\n## src/app.py\n\n- ⚠\u{fe0f} **PY001** (line 3): unused import `os`\n- ❌ **SEC001** (line 1): possible SQL injection\n\n## Summary\n\n- Files checked: 2\n- Errors: 1\n- Warnings: 1\n");
}
