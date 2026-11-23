# core drills on the shared GKE test cluster

The drill harness runs the StatefulSet that the `service-k8s` render produces
on the shared GKE test cluster. It checks only what core controls: placement,
rescheduling, NetworkPolicy, rollout order and status conditions. The pods run
`registry.k8s.io/pause:3.10`, so no raft traffic is tested.

Two facts hold for the cluster (see `docs/standards/test-cluster.md` in faberline/aw):

- the cluster enforces NetworkPolicy (Dataplane V2);
- the core pools `core-server` and `core-client` fit one `gke-test.yml` call.

To start a run: Actions, `gke-drill`, Run workflow. It is manual only. The run fails when the gke_drill filter matches no test (ci/run-gke-drill.sh).

To add a drill: add one `#[ignore = "needs the shared GKE cluster: ..."]` test in
`crates/service-k8s/tests/it/`. Its first line is
`if !on_gke_drill_cluster("<name>") { return; }`, because the kind rows run every
ignored test of service-k8s with `--include-ignored`. Then use
`e2e_support::Kube::from_env()`, `DRILL_POD_LABEL` and `ready_pod`.

The probe pod of the partition drill carries its `command` in the `--overrides`
container, because `kubectl run` applies `--overrides` as a merge patch that
replaces the `containers` array. `probe_pod_kind::probe_pod_prints_its_marker_and_termination_message`
proves it on kind.

To regenerate the overlay after a render change:
`UPDATE_GKE_OVERLAY=1 cargo test -p service-k8s --test it gke_drill::overlay_file_matches_the_render`.

## Leader-loss drill

`gke_drill_leader_loss::leader_loss_pod_is_rescheduled_on_the_core_server_pool` deletes the pod
`drill-0`, waits until a new `drill-0` (a new uid) is Running and Ready, checks that every pod is
still on the `core-server` pool on its own node, and that the StatefulSet reports 3 ready
replicas. It checks no write: acknowledged writes are in the plan of core#361. It runs in the
`gke-drill` workflow with the smoke test.

## Rolling-upgrade drill

`gke_drill_rolling_upgrade::rolling_upgrade_replaces_the_pods_from_the_highest_ordinal` patches the pod
template of the `drill` StatefulSet (a new annotation and `minReadySeconds: 5`), waits for
`kubectl rollout status`, and checks that every pod was replaced in the order `drill-2`, `drill-1`,
`drill-0` (by creation time), that every pod is Ready, and that the StatefulSet reports 3 updated and
3 ready replicas on one revision. It checks no write: acknowledged writes are in the plan of core#361.
It runs in the `gke-drill` workflow with the other drills. There is no kind row and no `ci.yml` entry:
kind is retired (core#378).

## Partition drill

`gke_drill_partition::partition_isolates_one_pod_and_heals_when_the_policy_is_removed` applies a
NetworkPolicy built from the `service-k8s` render that isolates `drill-0` (it selects the pod by
`statefulset.kubernetes.io/pod-name` and denies all ingress and egress). A one-off busybox pod on the
`core-client` pool probes port 8080: before the policy the pod answers with a refusal (reachable),
under the policy the probe times out (isolated) while `drill-1` stays reachable, and after the policy
is removed `drill-0` is reachable again. It checks no write: acknowledged writes are in the plan of
core#361. It runs in the `gke-drill` workflow with the other drills.

## Upgrade-rollback drill

`gke_drill_upgrade_rollback::upgrade_and_rollback_keep_the_objects_on_a_real_apiserver` applies the
old render, then the current render, then the old render again (server-side apply), and checks that
each object (the ServiceAccount and the Service of the probe) keeps its uid and its `spec` in all
three states. It builds the old render from the newest `v0.*` tag on the runner (about 5 to 10
minutes) unless `UPGRADE_ROLLBACK_OLD_RENDER` names a file. It checks no `status` and no write: the
status proof is the in-process test `tests/it/upgrade_rollback.rs` (#373), and acknowledged writes are
in the plan of core#361. It creates no CRD and runs no old operator binary. It runs in the
`gke-drill` workflow with the other drills, and there is no kind row.

## Zero-downtime drill (2 of 3 pods stay Ready)

`gke_drill_zero_downtime::rolling_update_and_rollback_keep_two_of_three_pods_ready` patches the pod
template of the `drill` StatefulSet (annotation `core.faberline.dev/drill-rev: zero-downtime-up`,
`minReadySeconds: 5`), and then runs `kubectl rollout undo` for the rollback. While both rollouts run
it samples the pod list every 2 seconds. It checks that at least 2 of the 3 pods are Ready at every
sample, and that the revision after the rollback equals the revision before the upgrade. It records the
minimum Ready count, the sample counts and `elapsed_seconds` as evidence.

The evidence name is "rolling update and rollback keep 2 of 3 pods Ready". This is NOT a zero-downtime
proof under client load, and core#359 does not count it as one: the pods run `pause` and serve no
request, and core ships no serving image (faberline/aw#416, desk decision faberline/aw#611). No test
image is built, and no client load is sent.

Cost: a few minutes of the existing drill nodes inside the one run that the owner approved (cap 10 USD,
core#385). The price-page check of that approval is marked "not needed" for this run. It runs in the
`gke-drill` workflow with the other drills, and there is no kind row (kind is retired, core#378).
