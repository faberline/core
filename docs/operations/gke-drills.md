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
