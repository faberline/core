# Failure drills

A drill is a planned failure that a cluster must survive, followed by a check
of what the cluster did. Every drill ends with an expected result that a
person checks; a drill without that check is not done.

## Rules

- A drill on `kind` runs on a local or CI cluster and costs no cloud money.
- GKE runs and the 24 hour soak start only after a person approves the cost figure of the drill in a comment on the issue that asks for the run.
- A drill that has no code on main says `not available on main`; do not invent a command.
- The cost estimates for the paid GKE runs are written in #375; until a person approves a figure, the figure here is the words `no estimate`.

## Setup

```sh
NS=<namespace of the instance>             # the Kubernetes namespace that holds the instance
INSTANCE=<instance name>                   # pods carry the label app.kubernetes.io/instance=$INSTANCE
PODS="kubectl -n $NS get pods -l app.kubernetes.io/instance=$INSTANCE -o wide"   # list the pods of the instance
```

Core fixes no status route for the leader. Use the service's own raft status
endpoint (its JSON has `role` and `is_leader`) through
`kubectl -n $NS port-forward pod/<pod> <port>:<port>` and `curl`. The readiness
route is `GET /readyz`.

## Leader loss

- Runs on: kind (the drill needs only pods and the status endpoint).
- Cost estimate: 0 (runs on a kind cluster, no cloud cost)
- Commands:
  ```sh
  # find the leader pod with the raft status endpoint, then:
  kubectl -n "$NS" delete pod "$LEADER" --grace-period=0 --force
  kubectl -n "$NS" wait --for=condition=Ready pod -l app.kubernetes.io/instance="$INSTANCE" --timeout=180s
  # read the status endpoint of another pod again
  ```
- Expected result: a different pod is leader within 180 s, the term is higher, and every write that was acknowledged before the kill is still readable.

kind code for this drill: issue #362 (row `drill:leader-loss`).

## Node loss

- Runs on: paid GKE cluster (a kind node shares the host disk and kernel, so it cannot model a lost machine).
- Cost estimate: no estimate
- Commands:
  ```sh
  kubectl get nodes -o wide
  kubectl cordon <node>
  kubectl drain <node> --ignore-daemonsets --delete-emptydir-data --force --timeout=300s
  gcloud compute instances delete <instance> --zone <zone>
  kubectl -n "$NS" wait --for=condition=Ready pod -l app.kubernetes.io/instance="$INSTANCE" --timeout=600s
  ```
- Expected result: the pods of the lost node are rescheduled, quorum stays or returns, and no acknowledged write is lost.

## Network partition

- Runs on: kind.
- Cost estimate: 0 (runs on a kind cluster, no cloud cost)
- Commands:
  ```sh
  # isolate one pod: no ingress and no egress
  kubectl -n "$NS" apply -f - <<'YAML'
  apiVersion: networking.k8s.io/v1
  kind: NetworkPolicy
  metadata:
    name: drill-isolate
  spec:
    podSelector:
      matchLabels:
        statefulset.kubernetes.io/pod-name: <pod>
    policyTypes: [Ingress, Egress]
  YAML
  # check from the other two pods that the quorum side keeps serving, then:
  kubectl -n "$NS" delete networkpolicy drill-isolate
  ```
- Expected result: the two-pod side keeps a leader and accepts writes, the isolated pod rejects writes, and after the policy is deleted it rejoins and catches up.

The cluster's CNI must enforce NetworkPolicy: the default kind CNI does not, so use a kind cluster with Calico or Cilium, or say the drill is not valid.

kind code: issue #362 (row `drill:partition`).

## Disk full

- Runs on: paid GKE cluster (a full persistent disk needs a real volume with a quota).
- Cost estimate: no estimate
- Commands:
  ```sh
  kubectl -n "$NS" exec <pod> -- df -h /data
  kubectl -n "$NS" exec <pod> -- sh -c 'dd if=/dev/zero of=/data/fill.bin bs=1M count=100000 || true'
  kubectl -n "$NS" logs <pod> --tail=100
  # recovery
  kubectl -n "$NS" exec <pod> -- rm /data/fill.bin
  ```
- Expected result: the node stops accepting writes with an error and does not corrupt its log; after the file is removed it serves again.

There is no kind drill for this; kind rows stay out of scope (issue #362).

## Backup and restore

- Runs on: paid GKE cluster.
- Cost estimate: no estimate
- Commands:
  ```sh
  # not available on main: no backup or restore code has landed
  ```
- Expected result: `not available on main`

Work: #19 and #114, #116, #117, #118.

## Rolling upgrade

- Runs on: kind.
- Cost estimate: 0 (runs on a kind cluster, no cloud cost)
- Commands:
  ```sh
  kubectl -n "$NS" set image statefulset/"$INSTANCE" <container>=<new image>
  kubectl -n "$NS" rollout status statefulset/"$INSTANCE" --timeout=600s
  # then read the status endpoint of each pod
  ```
- Expected result: pods restart one at a time, quorum holds, and no acknowledged write is lost.

kind code: issue #362 (row `drill:rolling-upgrade`); the upgrade and rollback test of the previous release is issue #359.

## 24 hour soak

- Runs on: paid GKE cluster.
- Cost estimate: no estimate
- Commands:
  ```sh
  # source crates/service-observability/scripts/soak-metrics.sh in a caller that owns the workload
  # sample the resident memory of each pod every 60 s for 86400 s:
  while sleep 60; do kubectl -n "$NS" top pod -l app.kubernetes.io/instance="$INSTANCE" --no-headers >> soak-rss.txt; done
  ```
- Expected result: the memory of each pod reaches a plateau. Memory growth check: the last 4 hours of samples grow by less than 10 percent over the first sample of that window; otherwise the drill fails.

The helper in `crates/service-observability/scripts/soak-metrics.sh` is a sourced library (functions `service_soak_rss_sampler_start`, see the file head), not a standalone command. It measures a metrics window, not data loss, so a write-loss check must be added by the caller.
