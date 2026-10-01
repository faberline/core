use super::*;

fn cx() -> RenderCtx<'static> {
    RenderCtx::new(
        "pgpool",
        "pgpool-operator",
        "pgpool.axiom.dev/v1alpha1",
        "Pgpool",
        "pool",
        "database",
    )
}

#[test]
fn pod_template_preserves_runtime_and_drain_fields() {
    let cx = cx();
    let template = ServicePodTemplate {
        cx: &cx,
        component: "pool",
        image: "pgpool:1",
        image_pull_policy: "IfNotPresent",
        command: vec!["pgpool".into()],
        args: vec!["serve".into()],
        ports: vec![json!({ "name": "postgres", "containerPort": 6432 })],
        env: vec![json!({ "name": "DB_HOST", "value": "remote-db" })],
        env_from: vec![json!({ "secretRef": { "name": "database" } })],
        resources: guaranteed_resources("500m", "512Mi"),
        readiness_probe: Some(json!({ "tcpSocket": { "port": "postgres" } })),
        liveness_probe: Some(json!({ "httpGet": { "path": "/healthz", "port": 9080 } })),
        startup_probe: Some(json!({ "httpGet": { "path": "/healthz", "port": 9080 } })),
        lifecycle: Some(json!({ "preStop": { "httpGet": { "path": "/drain", "port": 9080 } } })),
        container_security_context: Some(json!({ "runAsNonRoot": true })),
        pod_security_context: Some(json!({ "seccompProfile": { "type": "RuntimeDefault" } })),
        service_account_name: Some("pool"),
        termination_grace_period_seconds: Some(60),
        volumes: vec![json!({ "name": "tmp", "emptyDir": {} })],
        volume_mounts: vec![json!({ "name": "tmp", "mountPath": "/tmp" })],
        pod_annotations: Some(json!({ "prometheus.io/scrape": "true" })),
        topology_spread_constraints: vec![json!({
            "maxSkew": 1,
            "topologyKey": "kubernetes.io/hostname",
            "whenUnsatisfiable": "ScheduleAnyway",
            "labelSelector": { "matchLabels": cx.selector("pool") },
        })],
    }
    .render();

    let pod = &template["spec"];
    let container = &pod["containers"][0];
    for key in [
        "command",
        "args",
        "ports",
        "env",
        "envFrom",
        "resources",
        "readinessProbe",
        "livenessProbe",
        "startupProbe",
        "lifecycle",
        "securityContext",
        "volumeMounts",
    ] {
        assert!(!container[key].is_null(), "missing container field {key}");
    }
    for key in [
        "serviceAccountName",
        "terminationGracePeriodSeconds",
        "securityContext",
        "volumes",
        "topologySpreadConstraints",
    ] {
        assert!(!pod[key].is_null(), "missing pod field {key}");
    }
    assert_eq!(
        template["metadata"]["annotations"]["prometheus.io/scrape"],
        "true"
    );
}

#[test]
fn ordinary_children_are_cluster_ip_and_non_sticky() {
    let cx = cx();
    let service = client_service(&cx, "pool", "pool", 6432);
    assert_eq!(service["spec"]["type"], "ClusterIP");
    assert!(service["spec"]["sessionAffinity"].is_null());
    assert_eq!(service_account(&cx, "pool")["kind"], "ServiceAccount");
    assert_eq!(pdb(&cx, "pool", "pool", 1)["spec"]["maxUnavailable"], 1);
}

fn policy(peer_ports: Vec<i32>, extra_egress: Vec<Value>) -> Value {
    let cx = cx();
    network_policy(NetworkPolicy {
        cx: &cx,
        name: "pool",
        component: "pool",
        client_ports: vec![6432],
        peer_ports,
        extra_egress,
    })
}

#[test]
fn peer_ports_are_never_reachable_from_outside_the_instance() {
    let policy = policy(vec![9999], vec![]);
    let ingress = policy["spec"]["ingress"].as_array().expect("ingress rules");

    // The whole point of the policy: the consensus port must not appear in
    // any rule whose source is the cluster at large. If it ever does, every
    // pod in every namespace can speak the replication protocol.
    let open_to_cluster = ingress
        .iter()
        .find(|rule| !rule["from"][0]["namespaceSelector"].is_null())
        .expect("client rule");
    assert_eq!(
        open_to_cluster["ports"],
        json!([{"protocol": "TCP", "port": 6432}])
    );

    let peer_rule = ingress
        .iter()
        .find(|rule| !rule["from"][0]["podSelector"].is_null())
        .expect("peer rule");
    assert_eq!(
        peer_rule["ports"],
        json!([{"protocol": "TCP", "port": 9999}])
    );
    // Instance-scoped, not app-scoped: a second Pgpool in this namespace
    // must not be admitted to this one's consensus port.
    assert_eq!(
        peer_rule["from"][0]["podSelector"]["matchLabels"]["app.kubernetes.io/instance"],
        "pool"
    );
}

#[test]
fn egress_baseline_resolves_dns_and_allows_tls_but_not_plaintext() {
    let policy = policy(vec![9999], vec![]);
    let egress = policy["spec"]["egress"].as_array().expect("egress rules");
    let baseline = egress
        .iter()
        .find(|rule| rule["to"].is_null())
        .expect("unrestricted-destination rule");
    assert_eq!(
        baseline["ports"],
        json!([
            { "protocol": "UDP", "port": 53 },
            { "protocol": "TCP", "port": 53 },
            { "protocol": "TCP", "port": 443 },
        ]),
        "a truncated UDP answer retries over TCP/53; plaintext :80 is not granted"
    );
}

#[test]
fn a_service_with_no_peers_emits_no_peer_rules_and_still_denies_by_default() {
    let policy = policy(vec![], vec![]);
    assert_eq!(policy["spec"]["ingress"].as_array().unwrap().len(), 1);
    assert_eq!(policy["spec"]["egress"].as_array().unwrap().len(), 1);
    assert_eq!(
        policy["spec"]["policyTypes"],
        json!(["Ingress", "Egress"]),
        "both directions must stay declared, or the unlisted one is unrestricted"
    );
}

#[test]
fn extra_egress_is_appended_not_substituted() {
    let broker = json!({ "ports": [{ "protocol": "TCP", "port": 4222 }] });
    let policy = policy(vec![9999], vec![broker.clone()]);
    let egress = policy["spec"]["egress"].as_array().expect("egress rules");
    assert_eq!(egress.last(), Some(&broker));
    assert!(
        egress.iter().any(|rule| rule["ports"][0]["port"] == 53),
        "a custom destination must not displace DNS"
    );
}
