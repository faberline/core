use super::*;

const T: &[Topic] = &[Topic {
    id: "workflow",
    summary: "how it works",
    body: "# the body",
}];

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

const SECTIONED: &[SectionedTopic] = &[SectionedTopic {
    id: "workflow",
    summary: "how it works",
    sections: &[
        TopicSection::Prose("# intro prose"),
        TopicSection::Generated {
            id: "fact",
            render: fixed_fact,
        },
    ],
}];

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
    const BROKEN: &[SectionedTopic] = &[SectionedTopic {
        id: "broken",
        summary: "has a dead generator",
        sections: &[TopicSection::Generated {
            id: "dead",
            render: empty,
        }],
    }];
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
