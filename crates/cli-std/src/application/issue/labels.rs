use crate::ToolInfo;

/// Canonical labels for CLI-created intake Reports.
///
/// Callers may add domain labels, but the shared issue surface always owns the
/// work-item type and app identity so every created issue enters AW's typed
/// intake queue.
pub fn report_labels(tool: &ToolInfo, labels: &[String]) -> Vec<String> {
    let mut canonical = labels.to_vec();
    for required in [tool.issue_label(), "type:report".to_string()] {
        if !canonical.iter().any(|label| label == &required) {
            canonical.push(required);
        }
    }
    canonical
}
