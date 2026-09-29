use super::*;
use crate::domain::llm::v2::{json_schema, Input, Risk, Runbook, Step, Task};

const PROVIDER: crate::llm::Topic = crate::llm::Topic {
    id: "openapi-codegen",
    summary: "Shared generated-client rules.",
    body: "# Shared generator\n\nThe library owns these bytes.",
};
const SECOND_PROVIDER: crate::llm::Topic = crate::llm::Topic {
    id: "transport-policy",
    summary: "Shared transport rules.",
    body: "# Shared transport\n\nThe transport library owns these bytes.",
};

fn sample() -> ProtocolDocument {
    ProtocolDocument::new(
        "lumen",
        vec![Topic {
            task: Task {
                id: "local-search".into(),
                use_when: "inspect a local search contract".into(),
                requires: vec![],
                reads: vec!["lumen spec --fields".into()],
                produces: vec!["validated request body".into()],
                risk: Risk::Inspect,
                topic: "search".into(),
                contract_refs: vec!["spec.fields".into()],
            },
            runbook: Runbook {
                purpose: "Read the offline contract before querying.".into(),
                preconditions: vec![],
                inputs: vec![],
                constraints: vec!["No network is required.".into()],
                steps: vec![Step {
                    id: "read-fields".into(),
                    instruction: "Read field capabilities.".into(),
                    command: Some("lumen spec --fields".into()),
                    command_template: None,
                    inputs: vec![],
                }],
                verification: vec!["Choose a supported operator.".into()],
                references: vec!["spec.fields".into()],
            },
        }],
    )
    .unwrap()
}

#[test]
fn json_keeps_compatibility_fields_and_adds_protocol() {
    let json = sample().render("outline", Format::Json).unwrap();
    assert!(json.contains("\"topic\": \"outline\""));
    assert!(json.contains("\"markdown\""));
    assert!(json.contains(PROTOCOL));
    assert!(json.contains("\"tasks\""));
}

#[test]
fn schema_covers_manifest_and_typed_runbook_envelopes() {
    let schema = json_schema();
    assert_eq!(schema["$id"], PROTOCOL);
    assert_eq!(schema["oneOf"].as_array().unwrap().len(), 2);
    assert!(schema["$defs"]["task"]["required"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| field == "contract_refs"));
    assert!(schema["$defs"]["runbook"]["required"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| field == "verification"));
    assert_eq!(
        schema["oneOf"][1]["properties"]["providers"]["items"]["$ref"],
        "#/$defs/provider"
    );
    assert!(!schema["oneOf"][1]["required"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| field == "providers"));
}

#[test]
fn provider_is_ordered_and_does_not_change_the_outline() {
    let mut base = sample();
    let mut other = base.topics[0].clone();
    other.task.id = "other-task".into();
    other.task.topic = "other".into();
    base.topics.push(other);
    let outline_before = base.render("outline", Format::Json).unwrap();
    let outline_markdown_before = base.render("outline", Format::Md).unwrap();
    let other_before = base.render("other", Format::Json).unwrap();
    let other_markdown_before = base.render("other", Format::Md).unwrap();
    let document = base
        .with_topic_provider("search", &PROVIDER)
        .unwrap()
        .with_topic_provider("search", &SECOND_PROVIDER)
        .unwrap();

    assert_eq!(
        document.render("outline", Format::Json).unwrap(),
        outline_before,
        "providers must not change the task manifest"
    );
    assert_eq!(
        document.render("outline", Format::Md).unwrap(),
        outline_markdown_before,
        "providers must not change the outline Markdown"
    );
    assert_eq!(
        document.render("other", Format::Json).unwrap(),
        other_before,
        "a provider must not change another topic's detail envelope"
    );
    assert_eq!(
        document.render("other", Format::Md).unwrap(),
        other_markdown_before,
        "a provider must not change another topic's Markdown"
    );
    let detail: serde_json::Value =
        serde_json::from_str(&document.render("search", Format::Json).unwrap()).unwrap();
    assert_eq!(detail["providers"][0]["id"], "openapi-codegen");
    assert_eq!(detail["providers"][1]["id"], "transport-policy");
    assert_eq!(
        detail["providers"][0]["markdown"],
        "# Shared generator\n\nThe library owns these bytes."
    );
    let markdown = document.render("search", Format::Md).unwrap();
    assert!(markdown.contains("## Shared providers"));
    assert!(markdown.contains("The library owns these bytes."));
    assert!(
        markdown.find(PROVIDER.body).unwrap() < markdown.find(SECOND_PROVIDER.body).unwrap(),
        "provider Markdown must follow registration order"
    );
}

#[test]
fn provider_registration_rejects_unknown_empty_and_duplicate_inputs() {
    assert!(sample().with_topic_provider("missing", &PROVIDER).is_err());

    const EMPTY_ID: crate::llm::Topic = crate::llm::Topic {
        id: " ",
        summary: "invalid",
        body: "body",
    };
    const EMPTY_BODY: crate::llm::Topic = crate::llm::Topic {
        id: "empty-body",
        summary: "invalid",
        body: " \n",
    };
    assert!(sample().with_topic_provider("search", &EMPTY_ID).is_err());
    assert!(sample().with_topic_provider("search", &EMPTY_BODY).is_err());
    assert!(sample()
        .with_topic_provider("search", &PROVIDER)
        .unwrap()
        .with_topic_provider("search", &PROVIDER)
        .is_err());
    let mut two_topics = sample();
    let mut other = two_topics.topics[0].clone();
    other.task.id = "other-task".into();
    other.task.topic = "other".into();
    two_topics.topics.push(other);
    assert!(two_topics
        .with_topic_provider("search", &PROVIDER)
        .unwrap()
        .with_topic_provider("other", &PROVIDER)
        .is_ok());
}

#[test]
fn unbound_commands_require_a_template() {
    let mut document = sample();
    let mut topic = document.topics.remove(0);
    topic.runbook.steps[0].command = Some("lumen query --url <url>".into());
    assert!(ProtocolDocument::new("lumen", vec![topic]).is_err());
}

#[test]
fn templates_must_name_each_typed_input_exactly_once_or_more() {
    let mut document = sample();
    let mut topic = document.topics.remove(0);
    topic.runbook.steps[0].command = None;
    topic.runbook.steps[0].command_template = Some("lumen query --url {url}".into());
    topic.runbook.steps[0].inputs = vec![Input {
        name: "other".into(),
        value_type: "string".into(),
        description: "wrong input".into(),
        required: true,
    }];
    assert!(ProtocolDocument::new("lumen", vec![topic]).is_err());
}
