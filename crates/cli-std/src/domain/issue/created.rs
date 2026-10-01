#[cfg(any(feature = "online", test))]
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CreatedIssue {
    pub(crate) url: String,
    pub(crate) labels: Vec<String>,
}

#[cfg(any(feature = "online", test))]
pub(crate) fn created_issue_from_response(value: &serde_json::Value) -> CreatedIssue {
    let labels = value
        .get("labels")
        .and_then(|labels| labels.as_array())
        .into_iter()
        .flatten()
        .filter_map(|label| {
            label
                .as_str()
                .or_else(|| label.get("name").and_then(|name| name.as_str()))
        })
        .map(str::to_string)
        .collect();
    CreatedIssue {
        url: value
            .get("html_url")
            .and_then(|url| url.as_str())
            .unwrap_or("(issue created)")
            .to_string(),
        labels,
    }
}

#[cfg(any(feature = "online", test))]
fn missing_created_issue_labels(requested: &[String], returned: &[String]) -> Vec<String> {
    let mut missing = Vec::new();
    for label in requested {
        if !returned.iter().any(|returned| returned == label)
            && !missing
                .iter()
                .any(|already_missing| already_missing == label)
        {
            missing.push(label.clone());
        }
    }
    missing
}

#[cfg(any(feature = "online", test))]
pub(crate) fn created_issue_label_state(requested: &[String], returned: &[String]) -> String {
    let missing = missing_created_issue_labels(requested, returned);
    if missing.is_empty() {
        "labels: applied".to_string()
    } else {
        format!(
            "labels: pending repository reconciliation: {}",
            missing.join(", ")
        )
    }
}
