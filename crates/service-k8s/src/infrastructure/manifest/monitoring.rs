//! Prometheus Operator children: a `ServiceMonitor` that scrapes a client
//! Service and a `PrometheusRule` of alert rules. Manifest only, no cluster
//! calls; the same shape as [`super::cron_job`] and [`super::pdb`].

use serde_json::{json, Value};

use super::RenderCtx;

/// Parameters for [`service_monitor`].
pub struct ServiceMonitor<'a> {
    pub cx: &'a RenderCtx<'a>,
    /// Name of the ServiceMonitor object.
    pub name: &'a str,
    /// Component label of the client Service to scrape (the same `component` given to `client_service`).
    pub component: &'a str,
    /// Name of the Service port to scrape (`"http"` for `client_service`).
    pub port: &'a str,
    /// Metrics path, for example `"/metrics"`.
    pub path: &'a str,
    /// Scrape interval, for example `"30s"`.
    pub interval: &'a str,
}

/// A ServiceMonitor that selects the client Service of a component.
pub fn service_monitor(p: ServiceMonitor) -> Value {
    let cx = p.cx;
    json!({
        "apiVersion": "monitoring.coreos.com/v1",
        "kind": "ServiceMonitor",
        "metadata": cx.meta(p.name, p.component),
        "spec": {
            "selector": { "matchLabels": cx.selector(p.component) },
            "endpoints": [ { "port": p.port, "path": p.path, "interval": p.interval } ],
        },
    })
}

/// One alert rule.
pub struct AlertRule<'a> {
    pub alert: &'a str,
    pub expr: &'a str,
    /// `for` duration, for example `"5m"`.
    pub for_duration: &'a str,
    /// `severity` label value, for example `"critical"`.
    pub severity: &'a str,
    pub summary: &'a str,
}

/// Parameters for [`prometheus_rule`].
pub struct PrometheusRule<'a> {
    pub cx: &'a RenderCtx<'a>,
    pub name: &'a str,
    pub component: &'a str,
    /// Rule group name.
    pub group: &'a str,
    pub rules: Vec<AlertRule<'a>>,
}

/// A PrometheusRule with one rule group holding the given alert rules, in order.
pub fn prometheus_rule(p: PrometheusRule) -> Value {
    let cx = p.cx;
    let rules: Vec<Value> = p
        .rules
        .iter()
        .map(|r| {
            json!({
                "alert": r.alert,
                "expr": r.expr,
                "for": r.for_duration,
                "labels": { "severity": r.severity },
                "annotations": { "summary": r.summary },
            })
        })
        .collect();
    json!({
        "apiVersion": "monitoring.coreos.com/v1",
        "kind": "PrometheusRule",
        "metadata": cx.meta(p.name, p.component),
        "spec": { "groups": [ { "name": p.group, "rules": rules } ] },
    })
}

/// An owned alert rule: the same fields as [`AlertRule`], plus the metrics its `expr` reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DefaultAlertRule {
    pub alert: String,
    pub expr: String,
    /// `for` duration, for example `"5m"`.
    pub for_duration: String,
    /// `severity` label value, for example `"critical"`.
    pub severity: String,
    pub summary: String,
    /// Every metric name that `expr` reads (`up` is the Prometheus built-in).
    pub metrics: Vec<String>,
}

impl DefaultAlertRule {
    /// Borrow as an [`AlertRule`] to pass to [`PrometheusRule`] next to an application's own rules.
    pub fn as_rule(&self) -> AlertRule<'_> {
        AlertRule {
            alert: &self.alert,
            expr: &self.expr,
            for_duration: &self.for_duration,
            severity: &self.severity,
            summary: &self.summary,
        }
    }
}

/// Names of the default alerts, in the order `default_alert_rules` returns them.
pub const DEFAULT_ALERT_NAMES: [&str; 4] = [
    "ServiceDown",
    "OperatorNoLeader",
    "OperatorReconcileErrors",
    "OperatorReconcileSlow",
];

/// The default alert rules for the conditions core owns.
/// `manager` is the operator field-manager name (`lumen-operator`), `service` the client Service name.
/// An alert whose name is in `disabled` is left out; an unknown name in `disabled` is ignored.
pub fn default_alert_rules(
    manager: &str,
    service: &str,
    disabled: &[&str],
) -> Vec<DefaultAlertRule> {
    let p = manager.replace('-', "_");
    let mut rules = vec![
        DefaultAlertRule {
            alert: "ServiceDown".into(),
            expr: format!(r#"up{{service="{service}"}} == 0"#),
            for_duration: "5m".into(),
            severity: "critical".into(),
            summary: "The service has no scrape target up".into(),
            metrics: vec!["up".into()],
        },
        DefaultAlertRule {
            alert: "OperatorNoLeader".into(),
            expr: format!("sum({p}_leader) == 0"),
            for_duration: "5m".into(),
            severity: "critical".into(),
            summary: "No operator replica holds the leader Lease".into(),
            metrics: vec![format!("{p}_leader")],
        },
        DefaultAlertRule {
            alert: "OperatorReconcileErrors".into(),
            expr: format!(
                "sum(rate({p}_reconcile_errors_total[5m])) / sum(rate({p}_reconcile_total[5m])) > 0.1"
            ),
            for_duration: "10m".into(),
            severity: "warning".into(),
            summary: "More than 10% of operator reconciles fail".into(),
            metrics: vec![
                format!("{p}_reconcile_errors_total"),
                format!("{p}_reconcile_total"),
            ],
        },
        DefaultAlertRule {
            alert: "OperatorReconcileSlow".into(),
            expr: format!(
                "histogram_quantile(0.99, sum by (le) (rate({p}_reconcile_duration_seconds_bucket[5m]))) > 10"
            ),
            for_duration: "15m".into(),
            severity: "warning".into(),
            summary: "Operator reconcile p99 is above 10 seconds".into(),
            metrics: vec![format!("{p}_reconcile_duration_seconds_bucket")],
        },
    ];
    rules.retain(|r| !disabled.contains(&r.alert.as_str()));
    rules
}
