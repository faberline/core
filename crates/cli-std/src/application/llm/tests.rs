use super::*;

const T: &[Topic] = &[Topic::new("workflow", "how it works", "# the body")];

#[test]
fn outline_lists_topics_and_standard_commands() {
    let o = render("lumen", "0.4.3", T, "outline", Format::Md).unwrap();
    assert!(o.contains("`workflow`"));
    assert!(o.contains("lumen upgrade"));
    assert!(o.contains("lumen issue search"));
    assert!(o.contains("comment <n>"));
    assert!(!o.contains("report-issue"));
}

#[test]
fn topic_body_and_unknown() {
    assert_eq!(
        render("lumen", "0.4.3", T, "workflow", Format::Md).unwrap(),
        "# the body"
    );
    assert!(render("lumen", "0.4.3", T, "nope", Format::Md).is_err());
}

#[test]
fn json_outline_shape() {
    let j = render("lumen", "0.4.3", T, "outline", Format::Json).unwrap();
    assert!(j.contains("\"project\"") && j.contains("\"topics\""));
    assert_eq!(Format::parse("JSON"), Format::Json);
    assert_eq!(Format::parse("md"), Format::Md);
}

// --- SectionedTopic / TopicSection (#2494 Phase 1) ---

fn fixed_fact() -> String {
    "the sky is blue".to_string()
}

const SECTIONED: &[SectionedTopic] = &[SectionedTopic::new(
    "workflow",
    "how it works",
    &[
        TopicSection::Prose("# intro prose"),
        TopicSection::Generated {
            id: "fact",
            render: fixed_fact,
        },
    ],
)];

#[test]
fn static_topic_unchanged_behavior() {
    // The existing Topic + render() signatures and outputs are untouched
    // by adding SectionedTopic — same exact-match assertions as before.
    assert_eq!(
        render("lumen", "0.4.3", T, "workflow", Format::Md).unwrap(),
        "# the body"
    );
    let o = render("lumen", "0.4.3", T, "outline", Format::Md).unwrap();
    assert!(o.contains("`workflow`") && o.contains("lumen upgrade"));
    // assert_topics_render also accepts a plain `&[Topic]` registry.
    assert_topics_render(T);
}

#[test]
fn sectioned_topic_renders_prose_then_generated_in_order() {
    let body = render_sectioned("lumen", "0.4.3", SECTIONED, "workflow", Format::Md).unwrap();
    let prose_at = body.find("# intro prose").expect("prose section present");
    let fact_at = body
        .find("the sky is blue")
        .expect("generated section present");
    assert!(
        prose_at < fact_at,
        "prose must render before the generated section: {body:?}"
    );
}

#[test]
fn conformance_helper_catches_empty_generated_section() {
    fn empty() -> String {
        String::new()
    }
    const BROKEN: &[SectionedTopic] = &[SectionedTopic::new(
        "broken",
        "has a dead generator",
        &[TopicSection::Generated {
            id: "dead",
            render: empty,
        }],
    )];
    let result = std::panic::catch_unwind(|| assert_topics_render(BROKEN));
    assert!(
        result.is_err(),
        "assert_topics_render must panic on an empty generated section"
    );
}

#[test]
fn conformance_helper_passes_healthy_sectioned_topics() {
    assert_topics_render(SECTIONED);
}

#[test]
fn sectioned_json_format_shape() {
    let j = render_sectioned("lumen", "0.4.3", SECTIONED, "workflow", Format::Json).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&j).unwrap();
    assert_eq!(parsed["project"], "lumen");
    assert_eq!(parsed["topic"], "workflow");
    assert_eq!(parsed["summary"], "how it works");
    assert!(parsed["body"].as_str().unwrap().contains("the sky is blue"));
    let sections = parsed["sections"].as_array().unwrap();
    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0]["kind"], "prose");
    assert_eq!(sections[1]["kind"], "generated");
    assert_eq!(sections[1]["id"], "fact");
    assert_eq!(sections[1]["content"], "the sky is blue");
}

// --- golden bytes of `<tool> llm` output (pinned before P2) ---

const GOLDEN_OUTLINE_MD: &str = "# lumen — agent topic outline\n\nRun `lumen llm --topic <topic>` for detail (add `--format json` for a machine-readable form).\n\n## Topics\n\n- `workflow` — how it works\n\n## Standard agent commands\n\n- `lumen llm [--topic <t>] [--format md|json]` — this self-documentation (offline)\n- `lumen upgrade [--version <tag>] [--check]` — self-update from GitHub releases\n- `lumen issue search [query]` · `view <n>` · `create [--title <t>] [message...]` · `comment <n> [message...]` — search, read, file, and comment on diagnostics-rich issues; comment ensures the issue is open\n";

const GOLDEN_OUTLINE_JSON: &str = "{\n  \"project\": \"lumen\",\n  \"version\": \"0.4.3\",\n  \"topics\": [\n    {\n      \"id\": \"workflow\",\n      \"summary\": \"how it works\"\n    }\n  ]\n}";

#[test]
fn golden_topic_output_bytes() {
    assert_eq!(
        render("lumen", "0.4.3", T, "outline", Format::Md).unwrap(),
        GOLDEN_OUTLINE_MD
    );
    assert_eq!(
        render("lumen", "0.4.3", T, "outline", Format::Json).unwrap(),
        GOLDEN_OUTLINE_JSON
    );
    assert_eq!(
        render("lumen", "0.4.3", T, "workflow", Format::Md).unwrap(),
        "# the body"
    );
    assert_eq!(
        render("lumen", "0.4.3", T, "workflow", Format::Json).unwrap(),
        "{\n  \"project\": \"lumen\",\n  \"topic\": \"workflow\",\n  \"summary\": \"how it works\",\n  \"body\": \"# the body\"\n}"
    );
}

#[test]
fn golden_sectioned_topic_output_bytes() {
    assert_eq!(
        render_sectioned("lumen", "0.4.3", SECTIONED, "outline", Format::Md).unwrap(),
        GOLDEN_OUTLINE_MD
    );
    assert_eq!(
        render_sectioned("lumen", "0.4.3", SECTIONED, "outline", Format::Json).unwrap(),
        GOLDEN_OUTLINE_JSON
    );
    assert_eq!(
        render_sectioned("lumen", "0.4.3", SECTIONED, "workflow", Format::Md).unwrap(),
        "# intro prose\n\nthe sky is blue"
    );
    assert_eq!(
        render_sectioned("lumen", "0.4.3", SECTIONED, "workflow", Format::Json).unwrap(),
        "{\n  \"project\": \"lumen\",\n  \"topic\": \"workflow\",\n  \"summary\": \"how it works\",\n  \"body\": \"# intro prose\\n\\nthe sky is blue\",\n  \"sections\": [\n    {\n      \"id\": \"prose-0\",\n      \"kind\": \"prose\",\n      \"content\": \"# intro prose\"\n    },\n    {\n      \"id\": \"fact\",\n      \"kind\": \"generated\",\n      \"content\": \"the sky is blue\"\n    }\n  ]\n}"
    );
}

// --- assert_snapshot_matches (#9) ---

#[test]
fn snapshot_matches_equal_strings_pass() {
    assert_snapshot_matches("alpha\nbeta\n", "alpha\nbeta\n", "regen-cmd");
}

#[test]
fn snapshot_matches_line_endings_and_single_trailing_newline_pass() {
    // Differing only in \r\n vs \n
    assert_snapshot_matches("alpha\r\nbeta\r\n", "alpha\nbeta\n", "regen-cmd");
    // Differing only in presence of a single trailing newline
    assert_snapshot_matches("alpha\nbeta", "alpha\nbeta\n", "regen-cmd");
    assert_snapshot_matches("alpha\nbeta\n", "alpha\nbeta", "regen-cmd");
    // Both line endings and trailing newline differences
    assert_snapshot_matches("alpha\r\nbeta", "alpha\nbeta\n", "regen-cmd");
}

#[test]
fn snapshot_matches_differing_line_panics_with_line_number_and_hint() {
    let result = std::panic::catch_unwind(|| {
        assert_snapshot_matches(
            "line 1\nline 2\nline 3\n",
            "line 1\nline 2 differing\nline 3\n",
            "cargo run -- regen",
        );
    });
    assert!(result.is_err(), "differing line must panic");
    let err = result.unwrap_err();
    let msg = err
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| err.downcast_ref::<&'static str>().copied())
        .expect("panic message is string");
    assert!(
        msg.contains("2"),
        "message must mention line number 2: {msg}"
    );
    assert!(
        msg.contains("cargo run -- regen"),
        "message must mention regen hint: {msg}"
    );

    // Mismatch when one string has an extra line
    let result_extra = std::panic::catch_unwind(|| {
        assert_snapshot_matches(
            "line 1\nline 2\n",
            "line 1\nline 2\nline 3\n",
            "my-tool regen",
        );
    });
    assert!(result_extra.is_err(), "extra line must panic");
    let err_extra = result_extra.unwrap_err();
    let msg_extra = err_extra
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| err_extra.downcast_ref::<&'static str>().copied())
        .expect("panic message is string");
    assert!(
        msg_extra.contains("3"),
        "message must mention line number 3: {msg_extra}"
    );
    assert!(
        msg_extra.contains("my-tool regen"),
        "message must mention regen hint: {msg_extra}"
    );
}
