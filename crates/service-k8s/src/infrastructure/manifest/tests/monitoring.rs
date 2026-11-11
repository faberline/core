use super::*;

fn rules() -> Vec<AlertRule<'static>> {
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
    let m = service_monitor(ServiceMonitor {
        cx: &cx,
        name: "s",
        component: "server",
        port: "http",
        path: "/metrics",
        interval: "30s",
    });
    assert_eq!(m["kind"], "ServiceMonitor");
    assert_eq!(m["apiVersion"], "monitoring.coreos.com/v1");
    assert_eq!(m["metadata"]["name"], "s");
    assert_eq!(m["metadata"]["namespace"], "ns");
    assert_eq!(
        m["spec"]["endpoints"][0],
        json!({ "port": "http", "path": "/metrics", "interval": "30s" })
    );
}

#[test]
fn service_monitor_selects_client_service() {
    let cx = cx();
    let svc = client_service(&cx, "s", "server", 7000);
    let m = service_monitor(ServiceMonitor {
        cx: &cx,
        name: "s",
        component: "server",
        port: "http",
        path: "/metrics",
        interval: "30s",
    });
    let match_labels = m["spec"]["selector"]["matchLabels"].as_object().unwrap();
    assert!(!match_labels.is_empty());
    for (key, value) in match_labels {
        assert_eq!(&svc["metadata"]["labels"][key], value, "label {key}");
    }

    // Negative: a monitor for another component must not select this Service.
    let other = service_monitor(ServiceMonitor {
        cx: &cx,
        name: "s",
        component: "other",
        port: "http",
        path: "/metrics",
        interval: "30s",
    });
    assert_ne!(
        other["spec"]["selector"]["matchLabels"]["app.kubernetes.io/component"],
        svc["metadata"]["labels"]["app.kubernetes.io/component"]
    );
}

#[test]
fn service_monitor_owner_reference() {
    let owner = owner_ref("svc.dev/v1", "Svc", "s", "uid-1");
    let owned = cx().with_owner(owner);
    let m = service_monitor(ServiceMonitor {
        cx: &owned,
        name: "s",
        component: "server",
        port: "http",
        path: "/metrics",
        interval: "30s",
    });
    assert_eq!(m["metadata"]["ownerReferences"][0]["uid"], "uid-1");
    assert_eq!(m["metadata"]["ownerReferences"][0]["controller"], true);

    let cx = cx();
    let m = service_monitor(ServiceMonitor {
        cx: &cx,
        name: "s",
        component: "server",
        port: "http",
        path: "/metrics",
        interval: "30s",
    });
    assert!(m["metadata"]["ownerReferences"].is_null());
}

#[test]
fn prometheus_rule_shape() {
    let cx = cx();
    let r = prometheus_rule(PrometheusRule {
        cx: &cx,
        name: "s",
        component: "server",
        group: "svc.rules",
        rules: rules(),
    });
    assert_eq!(r["kind"], "PrometheusRule");
    assert_eq!(r["apiVersion"], "monitoring.coreos.com/v1");
    assert_eq!(r["spec"]["groups"][0]["name"], "svc.rules");
    let out = r["spec"]["groups"][0]["rules"].as_array().unwrap();
    assert_eq!(out.len(), 2);
    assert_eq!(out[0]["alert"], "SvcDown");
    assert_eq!(out[0]["expr"], "up == 0");
    assert_eq!(out[0]["for"], "5m");
    assert_eq!(out[0]["labels"]["severity"], "critical");
    assert_eq!(out[0]["annotations"]["summary"], "Svc is down");
    assert_eq!(out[1]["alert"], "SvcSlow");
}

#[test]
fn prometheus_rule_owner_reference() {
    let owner = owner_ref("svc.dev/v1", "Svc", "s", "uid-1");
    let owned = cx().with_owner(owner);
    let r = prometheus_rule(PrometheusRule {
        cx: &owned,
        name: "s",
        component: "server",
        group: "svc.rules",
        rules: rules(),
    });
    assert_eq!(r["metadata"]["ownerReferences"][0]["uid"], "uid-1");
    assert_eq!(r["metadata"]["ownerReferences"][0]["controller"], true);

    let cx = cx();
    let r = prometheus_rule(PrometheusRule {
        cx: &cx,
        name: "s",
        component: "server",
        group: "svc.rules",
        rules: rules(),
    });
    assert!(r["metadata"]["ownerReferences"].is_null());
}

#[test]
fn prometheus_rule_empty_rules() {
    let cx = cx();
    let r = prometheus_rule(PrometheusRule {
        cx: &cx,
        name: "s",
        component: "server",
        group: "svc.rules",
        rules: vec![],
    });
    let rules = &r["spec"]["groups"][0]["rules"];
    assert!(rules.is_array());
    assert_eq!(rules.as_array().unwrap().len(), 0);
}
