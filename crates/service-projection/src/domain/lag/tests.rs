use super::ProjectionLag;

const LAG_JSON: &str = r#"{"error":"projection_lag","projection":"logs","required_cursor":120,"current_cursor":42,"retryable":true,"retry_after_seconds":1}"#;

#[test]
fn projection_lag_json_is_pinned() {
    let lag = ProjectionLag::new("logs", 120, 42, 1);
    assert_eq!(serde_json::to_string(&lag).unwrap(), LAG_JSON);
    assert_eq!(
        serde_json::from_str::<ProjectionLag>(LAG_JSON).unwrap(),
        lag
    );
}
