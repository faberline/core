//! Render helpers for the Prometheus Operator children of an instance:
//! `ServiceMonitor` (scrapes the client Service) and `PrometheusRule` (alert
//! rules). Manifest only: no cluster calls.

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

/// Render a `ServiceMonitor` that selects the client Service of `component`.
pub fn service_monitor(p: ServiceMonitor) -> Value {
    json!({
        "apiVersion": "monitoring.coreos.com/v1",
        "kind": "ServiceMonitor",
        "metadata": p.cx.meta(p.name, p.component),
        "spec": {
            "selector": { "matchLabels": p.cx.selector(p.component) },
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

/// Render a `PrometheusRule` with one group holding `rules` in the given order.
pub fn prometheus_rule(p: PrometheusRule) -> Value {
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
        "metadata": p.cx.meta(p.name, p.component),
        "spec": {
            "groups": [ { "name": p.group, "rules": rules } ],
        },
    })
}
