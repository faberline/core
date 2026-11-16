# core drills on the shared GKE test cluster

The drill harness runs the StatefulSet that the `service-k8s` render produces
on the shared GKE test cluster. It checks only what core controls: placement,
rescheduling, NetworkPolicy, rollout order and status conditions. The pods run
`registry.k8s.io/pause:3.10`, so no raft traffic is tested.

Two facts hold for the cluster (see `docs/standards/test-cluster.md` in faberline/aw):

- the cluster enforces NetworkPolicy (Dataplane V2);
- the core pools `core-server` and `core-client` fit one `gke-test.yml` call.

To start a run: Actions, `gke-drill`, Run workflow. It is manual only.

To add a drill: add one `#[ignore = "needs the shared GKE cluster: ..."]` test in
`crates/service-k8s/tests/it/`. Its first line is
`if !on_gke_drill_cluster("<name>") { return; }`, because the kind rows run every
ignored test of service-k8s with `--include-ignored`. Then use
`e2e_support::Kube::from_env()`, `DRILL_POD_LABEL` and `ready_pod`.

To regenerate the overlay after a render change:
`UPDATE_GKE_OVERLAY=1 cargo test -p service-k8s --test it gke_drill::overlay_file_matches_the_render`.

## Leader-loss drill

`gke_drill_leader_loss::leader_loss_pod_is_rescheduled_on_the_core_server_pool` deletes the pod
`drill-0`, waits until a new `drill-0` (a new uid) is Running and Ready, checks that every pod is
still on the `core-server` pool on its own node, and that the StatefulSet reports 3 ready
replicas. It checks no write: acknowledged writes are in the plan of core#361. It runs in the
`gke-drill` workflow with the smoke test.

## Partition drill

`gke_drill_partition::partition_isolates_one_pod_and_heals_when_the_policy_is_removed` applies a
NetworkPolicy built from the `service-k8s` render that isolates `drill-0` (it selects the pod by
`statefulset.kubernetes.io/pod-name` and denies all ingress and egress). A one-off busybox pod on the
`core-client` pool probes port 8080: before the policy the pod answers with a refusal (reachable),
under the policy the probe times out (isolated) while `drill-1` stays reachable, and after the policy
is removed `drill-0` is reachable again. It checks no write: acknowledged writes are in the plan of
core#361. It runs in the `gke-drill` workflow with the other drills.
