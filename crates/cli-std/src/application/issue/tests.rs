use super::diagnostics::{followup_comment_body, render_diagnostics};
use super::labels::report_labels;
use super::repo::resolve_repo;
use crate::domain::issue::body::assemble_body;
use crate::domain::issue::created::{created_issue_from_response, created_issue_label_state};
#[cfg(feature = "online")]
use crate::domain::issue::payload::reopen_payload;
use crate::domain::issue::payload::{comment_payload, issue_payload};
use crate::domain::issue::url::prefilled_url;
use crate::ToolInfo;

const TOOL: ToolInfo = ToolInfo {
    project: "lumen",
    repo: "faberline/lumen",
    target: "aarch64-apple-darwin",
    version: "0.4.3",
    git_sha: "abc1234",
    built_at: "1700000000",
};

#[test]
fn diagnostics_and_body() {
    let d = render_diagnostics(&TOOL, None);
    for n in ["lumen version: 0.4.3", "aarch64-apple-darwin", "abc1234"] {
        assert!(d.contains(n), "missing {n}");
    }
    let b = assemble_body(Some("boom"), &d);
    assert!(b.find("boom").unwrap() < b.find("## Diagnostics").unwrap());
    assert!(assemble_body(None, &d).starts_with("## Diagnostics"));
}

#[test]
fn url_and_repo_and_payload() {
    let u = prefilled_url("o/n", "a b&c", "x\ny", &[]);
    assert!(u.starts_with("https://github.com/o/n/issues/new?title="));
    assert!(u.contains("a%20b%26c") && u.contains("x%0Ay") && !u.contains(' '));
    assert!(!u.contains("labels="));
    // Labels survive the no-token URL fallback (convention `app:<name>`).
    let ul = prefilled_url("o/n", "t", "b", &["app:jet".into(), "bug".into()]);
    assert!(ul.contains("&labels=app%3Ajet%2Cbug"));
    assert_eq!(resolve_repo(&TOOL, None), "faberline/lumen");
    assert_eq!(resolve_repo(&TOOL, Some("o/n")), "o/n");

    let p = issue_payload("t", "b", &["bug".into()]);
    assert_eq!(p["title"], "t");
    assert_eq!(p["labels"], serde_json::json!(["bug"]));
    assert!(issue_payload("t", "b", &[]).get("labels").is_none());
    assert_eq!(
        report_labels(&TOOL, &["severity:high".into(), "app:lumen".into()]),
        vec!["severity:high", "app:lumen", "type:report"]
    );
}

#[test]
fn created_issue_response_reports_applied_labels() {
    let created = created_issue_from_response(&serde_json::json!({
        "html_url": "https://github.com/o/n/issues/42",
        "labels": [
            {"name": "app:lumen"},
            {"name": "type:report"},
            {"name": "severity:high"}
        ]
    }));
    assert_eq!(created.url, "https://github.com/o/n/issues/42");
    assert_eq!(
        created_issue_label_state(&["app:lumen".into(), "type:report".into()], &created.labels,),
        "labels: applied"
    );
}

#[test]
fn created_issue_response_reports_missing_labels_in_requested_order() {
    let created = created_issue_from_response(&serde_json::json!({
        "html_url": "https://github.com/o/n/issues/42",
        "labels": [{"name": "severity:high"}, {"name": "app:lumen"}]
    }));
    assert_eq!(
        created_issue_label_state(
            &[
                "type:report".into(),
                "app:lumen".into(),
                "priority:p0".into(),
                "type:report".into(),
            ],
            &created.labels,
        ),
        "labels: pending repository reconciliation: type:report, priority:p0"
    );
}

#[test]
fn created_issue_response_fails_closed_for_absent_or_malformed_labels() {
    for response in [
        serde_json::json!({}),
        serde_json::json!({"labels": "app:lumen"}),
        serde_json::json!({"labels": [null, {}, {"name": 7}]}),
    ] {
        let created = created_issue_from_response(&response);
        assert_eq!(created.url, "(issue created)");
        assert_eq!(
            created_issue_label_state(&["app:lumen".into(), "type:report".into()], &created.labels,),
            "labels: pending repository reconciliation: app:lumen, type:report"
        );
    }
}

#[test]
fn created_issue_response_accepts_forwarded_string_label_shape() {
    let created = created_issue_from_response(&serde_json::json!({
        "html_url": "https://github.com/o/n/issues/42",
        "labels": ["app:lumen", "type:report"]
    }));
    assert_eq!(
        created_issue_label_state(&["app:lumen".into(), "type:report".into()], &created.labels,),
        "labels: applied"
    );
}

#[test]
fn comment_payload_and_followup_body() {
    #[cfg(feature = "online")]
    assert_eq!(reopen_payload()["state"], "open");
    assert_eq!(comment_payload("still failing")["body"], "still failing");

    let body = followup_comment_body(&TOOL, Some("user verification still fails"));
    assert!(body.contains("user verification still fails"));
    assert!(body.contains("## Diagnostics"));
    assert!(body.contains("lumen version: 0.4.3"));

    let default_body = followup_comment_body(&TOOL, Some("  "));
    assert!(default_body.contains("User-side verification failed after closure"));
}

#[test]
fn golden_diagnostics_and_followup_bytes() {
    let os_arch = format!("{}/{}", std::env::consts::OS, std::env::consts::ARCH);
    let diagnostics = format!(
        "## Diagnostics\n- lumen version: 0.4.3\n- target: aarch64-apple-darwin\n- git sha: abc1234\n- built at: 1700000000\n- os/arch: {os_arch}\n"
    );
    assert_eq!(render_diagnostics(&TOOL, None), diagnostics);
    assert_eq!(
        render_diagnostics(&TOOL, Some("node-1 ok")),
        format!("{diagnostics}- node: node-1 ok\n")
    );
    assert_eq!(
        followup_comment_body(&TOOL, Some("still broken")),
        format!("still broken\n\n---\n{diagnostics}")
    );
    assert_eq!(
        TOOL.issue_label(),
        "app:lumen",
        "the tracker label is part of every filed issue"
    );
    assert_eq!(TOOL.tag_prefix(), "lumen@");
    assert_eq!(TOOL.asset_name(), "lumen-aarch64-apple-darwin.tar.gz");
    assert_eq!(TOOL.inner_binary_path(), "lumen-aarch64-apple-darwin/lumen");
}

#[test]
fn representative_issue_outputs_are_chainable() {
    for output in [
        "repo:  faberline/lumen\ntitle: lumen: bug\n---\nbody\nnext: done\n",
        "filed: https://github.com/faberline/lumen/issues/42\nlabels: applied\nnext: done\n",
        "#1142 [open] lumen: add lightweight chainable output\nnext: done\n",
        "#1142 [open] lumen: add lightweight chainable output\nhttps://github.com/faberline/lumen/issues/1142\n---\nbody\nnext: done\n",
    ] {
        crate::chainable::assert_chainable(output)
            .expect("shared issue outputs should satisfy the lightweight chainable contract");
    }
}
