//! The default alert rules only name metrics that core exports (#369).
//!
//! The check reads the real exposition of `ControllerMetrics`, so a rule that
//! names `svc_operator_reconcile_error_total` (a typo) or a metric core never
//! renders fails here. Needs the `controller` feature, which owns the metrics.

#[cfg(feature = "controller")]
use service_k8s::metrics::ControllerMetrics;
#[cfg(feature = "controller")]
use service_k8s::render::{default_alert_rules, DefaultAlertRule};

/// True when the exposition has a sample line for `name` (not a `#` comment),
/// with the name followed by a label set or a space.
#[cfg(feature = "controller")]
fn is_exported(exposition: &str, name: &str) -> bool {
    exposition.lines().any(|line| {
        line.strip_prefix(name)
            .is_some_and(|rest| rest.starts_with('{') || rest.starts_with(' '))
    })
}

#[cfg(feature = "controller")]
#[test]
fn every_default_rule_reads_only_exported_metrics() {
    let exposition = ControllerMetrics::new("svc-operator").render(true);
    let rules = default_alert_rules("svc-operator", "s", &[]);
    assert!(!rules.is_empty());
    for rule in &rules {
        for name in &rule.metrics {
            assert!(
                name == "up" || is_exported(&exposition, name),
                "{} reads {name}, which core does not export",
                rule.alert
            );
            assert!(
                rule.expr.contains(name.as_str()),
                "{} lists {name} but its expr does not read it",
                rule.alert
            );
        }
        for token in rule
            .expr
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        {
            if token.starts_with("svc_operator_") {
                assert!(
                    rule.metrics.iter().any(|m| m == token),
                    "{} reads {token} but does not list it in metrics",
                    rule.alert
                );
            }
        }
    }
}

#[cfg(feature = "controller")]
#[test]
fn a_rule_naming_a_metric_core_does_not_export_is_caught() {
    let exposition = ControllerMetrics::new("svc-operator").render(true);
    let bad = DefaultAlertRule {
        alert: "Missing".into(),
        expr: "svc_operator_missing_total > 0".into(),
        for_duration: "5m".into(),
        severity: "warning".into(),
        summary: "negative control".into(),
        metrics: vec!["svc_operator_missing_total".into()],
    };
    assert!(bad.metrics.iter().any(|m| !is_exported(&exposition, m)));
    assert!(is_exported(&exposition, "svc_operator_reconcile_total"));
}
