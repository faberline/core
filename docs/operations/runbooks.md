# Runbooks

One section for each default alert of `service-k8s`. The alert names come from `DEFAULT_ALERT_NAMES` in `crates/service-k8s/src/infrastructure/manifest/monitoring.rs`; `ci/test-runbooks.sh` fails when an alert has no section. A service can turn an alert off through the `disabled` argument of `default_alert_rules`. Replace `<ns>` with the namespace of the instance and `<operator>` with the operator Deployment name (the `manager` name, for example `lumen-operator`). In metric names, `<p>` is the operator name with `-` replaced by `_` (for example `lumen_operator`).

## ServiceDown

**What it means**: Prometheus has had no scrape target with `up == 1` for the client Service for 5 minutes. Clients cannot rely on the service. The alert is critical and its rule waits 5 minutes (`up{service="<service>"} == 0`).

**How to check**: run `kubectl -n <ns> get pods` and `kubectl -n <ns> get endpoints <service>`. Then run `kubectl -n <ns> describe pod <pod>` and `kubectl -n <ns> logs <pod> --previous`. Check the ServiceMonitor with `kubectl -n <ns> get servicemonitor`.

**How to fix**: when the pods are crash-looping, read the previous logs and fix the cause (configuration, a missing Secret, a failed probe). When the pods are healthy but `up` is 0, the ServiceMonitor selector or the metrics port is wrong. Compare it with the Service labels and the metrics port (default `0.0.0.0:9090`; `OPERATOR_METRICS_ADDR` overrides it for the operator).

**When to escalate**: the alert is still firing 15 minutes after the first fix, or more than one instance fires at once (this points to a cluster or Prometheus problem).

## OperatorNoLeader

**What it means**: no operator replica holds the leader Lease, so nothing is reconciled (no rollout, no repair, no status update). The alert is critical and its rule waits 5 minutes (`sum(<p>_leader) == 0`). The leader is the replica that holds the Lease named like the operator. A replica that does not hold it publishes `<p>_leader 0`. The Lease lasts 15 seconds and is renewed every 5 seconds.

**How to check**: run `kubectl -n <ns> get lease <operator> -o yaml` and read `holderIdentity` and `renewTime`. A renew time older than 15 seconds means that there is no holder. Then run `kubectl -n <ns> get deploy <operator>` and `kubectl -n <ns> logs deploy/<operator>`.

**How to fix**: when the Deployment has 0 ready replicas, restore it with `kubectl -n <ns> rollout restart deploy/<operator>`. When replicas run but none holds the Lease, check that the operator ServiceAccount may `get`, `create` and `update` `leases.coordination.k8s.io` in the namespace (RBAC), and that the apiserver answers.

**When to escalate**: there is still no holder 15 minutes after the restart, or the Lease `holderIdentity` keeps changing every few seconds (a leader flap; check the apiserver latency).

## OperatorReconcileErrors

**What it means**: more than 10% of the operator reconciles fail over 5 minutes. The alert is a warning and its rule waits 10 minutes (`sum(rate(<p>_reconcile_errors_total[5m])) / sum(rate(<p>_reconcile_total[5m])) > 0.1`).

**How to check**: run `kubectl -n <ns> logs deploy/<operator> | grep -i error` and `kubectl get events -n <ns> --sort-by=.lastTimestamp`. Read the metrics `<p>_reconcile_errors_total` and `<p>_reconcile_total`.

**How to fix**: read the error of the failing object. Common causes are a refused apply (RBAC or an admission webhook), an invalid custom resource, or a missing dependency. Fix the resource or the permission. The next reconcile then clears the error.

**When to escalate**: the error rate stays above 10% after the fix, or the error is an apiserver error (HTTP 5xx or timeouts) that you cannot change.

## OperatorReconcileSlow

**What it means**: the 99th percentile of a reconcile is above 10 seconds over 5 minutes. The operator is stuck behind a slow or throttled apiserver. The alert is a warning and its rule waits 15 minutes (`histogram_quantile(0.99, sum by (le) (rate(<p>_reconcile_duration_seconds_bucket[5m]))) > 10`). The largest histogram bucket is 10 seconds.

**How to check**: read the histogram `<p>_reconcile_duration_seconds_bucket`. Run `kubectl -n <ns> logs deploy/<operator>`. Check the apiserver latency in the cluster monitoring, and the number of objects the operator owns.

**How to fix**: remove the load (many objects that change at once, or a hot loop on one object). Wait for the apiserver throttling to end. Do not raise the alert threshold to hide the problem.

**When to escalate**: the p99 stays above 10 seconds for 1 hour, or `OperatorReconcileErrors` fires at the same time.
