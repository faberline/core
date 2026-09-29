/// The GitHub issue-creation JSON payload (`labels` omitted when empty).
pub fn issue_payload(title: &str, body: &str, labels: &[String]) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    map.insert("title".into(), title.into());
    map.insert("body".into(), body.into());
    if !labels.is_empty() {
        map.insert("labels".into(), labels.iter().cloned().collect());
    }
    serde_json::Value::Object(map)
}

/// The GitHub issue update payload for reopening an issue.
#[cfg(feature = "online")]
pub(crate) fn reopen_payload() -> serde_json::Value {
    let mut map = serde_json::Map::new();
    map.insert("state".into(), "open".into());
    serde_json::Value::Object(map)
}

/// The GitHub issue-comment JSON payload.
pub fn comment_payload(body: &str) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    map.insert("body".into(), body.into());
    serde_json::Value::Object(map)
}
