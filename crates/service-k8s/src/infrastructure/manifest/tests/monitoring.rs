use super::*;

fn monitor_for<'a>(cx: &'a RenderCtx<'a>, component: &'a str) -> Value {
    service_monitor(ServiceMonitor {
        cx,
        name: "s",
        component,
        port: "http",
        path: "/metrics",
        interval: "30s",
    })
}

fn two_rules() -> Vec<AlertRule<'static>> {
    vec![
        AlertRule {
            alert: "SvcDown",
            expr: "up == 0",
            for_duration: "5m",
            severity: "critical",
            summary: "Svc is down",
        },
        AlertRule {
            alert: "SvcSlow",
            expr: "latency > 1",
            for_duration: "10m",
            severity: "warning",
            summary: "Svc is slow",
        },
    ]
}

#[test]
fn service_monitor_shape() {
    let cx = cx();
    let monitor = service_monitor(ServiceMonitor {
        cx: &cx,
        name: "s",
        component: "server",
        port: "http",
        path: "/metrics",
        interval: "30s",
    });
    assert_eq!(monitor["kind"], "ServiceMonitor");
    assert_eq!(monitor["apiVersion"], "monitoring.coreos.com/v1");
    assert_eq!(monitor["metadata"]["name"], "s");
    assert_eq!(monitor["metadata"]["namespace"], "ns");
    assert_eq!(
        monitor["spec"]["endpoints"][0],
        json!({ "port": "http", "path": "/metrics", "interval": "30s" })
    );
}

#[test]
fn service_monitor_selects_client_service() {
    let cx = cx();
    let service = client_service(&cx, "s", "server", 7000);
    let monitor = monitor_for(&cx, "server");
    let match_labels = monitor["spec"]["selector"]["matchLabels"]
        .as_object()
        .expect("matchLabels is an object");
    assert!(!match_labels.is_empty());
    for (key, value) in match_labels {
        assert_eq!(
            &service["metadata"]["labels"][key], value,
            "Service label {key} must match the monitor selector"
        );
    }

    // Negative check: a monitor for another component must not select this Service.
    let other = monitor_for(&cx, "other");
    assert_ne!(
        other["spec"]["selector"]["matchLabels"]["app.kubernetes.io/component"],
        service["metadata"]["labels"]["app.kubernetes.io/component"]
    );
}

#[test]
fn service_monitor_owner_reference() {
    let owned = svc_with_owner();
    let monitor = service_monitor(ServiceMonitor {
        cx: &owned,
        name: "s",
        component: "server",
        port: "http",
        path: "/metrics",
        interval: "30s",
    });
    let owner = &monitor["metadata"]["ownerReferences"][0];
    assert_eq!(owner["uid"], "uid-1");
    assert_eq!(owner["controller"], true);

    let plain = cx();
    let monitor = monitor_for(&plain, "server");
    assert!(monitor["metadata"]["ownerReferences"].is_null());
}

#[test]
fn prometheus_rule_shape() {
    let cx = cx();
    let rule = prometheus_rule(PrometheusRule {
        cx: &cx,
        name: "s",
        component: "server",
        group: "svc.rules",
        rules: two_rules(),
    });
    assert_eq!(rule["kind"], "PrometheusRule");
    assert_eq!(rule["apiVersion"], "monitoring.coreos.com/v1");
    assert_eq!(rule["spec"]["groups"][0]["name"], "svc.rules");
    let rules = rule["spec"]["groups"][0]["rules"].as_array().unwrap();
    assert_eq!(rules.len(), 2);
    assert_eq!(rules[0]["alert"], "SvcDown");
    assert_eq!(rules[0]["expr"], "up == 0");
    assert_eq!(rules[0]["for"], "5m");
    assert_eq!(rules[0]["labels"]["severity"], "critical");
    assert_eq!(rules[0]["annotations"]["summary"], "Svc is down");
    assert_eq!(rules[1]["alert"], "SvcSlow");
}

#[test]
fn prometheus_rule_owner_reference() {
    let owned = svc_with_owner();
    let rule = prometheus_rule(PrometheusRule {
        cx: &owned,
        name: "s",
        component: "server",
        group: "svc.rules",
        rules: two_rules(),
    });
    let owner = &rule["metadata"]["ownerReferences"][0];
    assert_eq!(owner["uid"], "uid-1");
    assert_eq!(owner["controller"], true);
}

#[test]
fn prometheus_rule_empty_rules() {
    let cx = cx();
    let rule = prometheus_rule(PrometheusRule {
        cx: &cx,
        name: "s",
        component: "server",
        group: "svc.rules",
        rules: vec![],
    });
    let rules = &rule["spec"]["groups"][0]["rules"];
    assert!(rules.is_array());
    assert_eq!(rules.as_array().unwrap().len(), 0);
}

fn svc_with_owner() -> RenderCtx<'static> {
    cx().with_owner(owner_ref("svc.dev/v1", "Svc", "s", "uid-1"))
}
