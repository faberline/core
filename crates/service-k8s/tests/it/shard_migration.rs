use serde_json::json;

use service_k8s::render::{sharded_statefulset, RenderCtx, ShardedStatefulSet};
use service_k8s::{plan_shard_migration, ShardMigrationPlan, StatefulSetShape};

#[test]
fn old_render_and_shard_zero_render_keep_the_same_pvc_names() {
    let cx = RenderCtx::new("svc", "svc-operator", "svc.dev/v1", "Svc", "s", "ns");
    let old = sharded_statefulset(ShardedStatefulSet {
        cx: &cx,
        name: "s",
        component: "server",
        image: "img:1",
        image_pull_policy: "IfNotPresent",
        command: vec!["serve".into()],
        ports: vec![("http", 7000)],
        headless_service: "s-headless",
        shard_count: 1,
        replicas_per_shard: 3,
        voter_count: 3,
        headless_env_key: "SVC_HEADLESS_SERVICE",
        cpu: "1",
        memory: "1Gi",
        extra_env: vec![],
        volume_claim: Some(json!({"metadata": {"name": "raft"}, "spec": {}})),
    })
    .expect("old single-shard render");

    // Fixture of the #395 shard-0 render: same name, claim and selector plus `shard=0`.
    let mut new_json = old.clone();
    new_json["spec"]["selector"]["matchLabels"]["shard"] = json!("0");

    let old_shape = StatefulSetShape::from_value(&old).expect("old shape");
    let new_shape = StatefulSetShape::from_value(&new_json).expect("new shape");

    assert_eq!(old_shape.pvc_names(3), new_shape.pvc_names(3));
    assert_eq!(
        old_shape.pvc_names(3),
        vec!["raft-s-0", "raft-s-1", "raft-s-2"]
    );

    assert_eq!(
        plan_shard_migration(&old_shape, &new_shape, 1),
        ShardMigrationPlan::RecreateOrphan
    );
    assert_eq!(
        plan_shard_migration(&new_shape, &new_shape, 1),
        ShardMigrationPlan::NoChange
    );
}
