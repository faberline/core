use std::collections::BTreeMap;

use super::TextDocument;
use crate::{DocumentId, DocumentVersion};

const DOCUMENT_JSON: &str =
    r#"{"external_id":"doc-1","version":3,"fields":{"body":"hello world","title":"Greeting"}}"#;

const BARE_DOCUMENT_JSON: &str = r#"{"external_id":"doc-2","version":0,"fields":{}}"#;

fn document() -> TextDocument {
    TextDocument {
        external_id: DocumentId::new("doc-1"),
        version: DocumentVersion::new(3),
        fields: BTreeMap::from([
            ("title".to_string(), "Greeting".to_string()),
            ("body".to_string(), "hello world".to_string()),
        ]),
    }
}

#[test]
fn text_document_json_is_pinned() {
    assert_eq!(serde_json::to_string(&document()).unwrap(), DOCUMENT_JSON);
    assert_eq!(
        serde_json::from_str::<TextDocument>(DOCUMENT_JSON).unwrap(),
        document()
    );
}

#[test]
fn text_document_without_fields_is_pinned() {
    let bare = TextDocument::new(DocumentId::new("doc-2"), DocumentVersion::new(0));
    assert_eq!(serde_json::to_string(&bare).unwrap(), BARE_DOCUMENT_JSON);
    assert_eq!(
        serde_json::from_str::<TextDocument>(BARE_DOCUMENT_JSON).unwrap(),
        bare
    );
}
