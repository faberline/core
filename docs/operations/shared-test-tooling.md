# Shared test tooling

This document decides which parts of lumen's test tooling become shared, where
they live, and the public API of the first shared piece: the e2e harness that
relay and defer copy today. It is a design. It creates no crate, changes no
script and starts no GKE run.

Sources: the survey in the comments of faberline/core#203 (lumen `79a7c1c5`,
relay `33050a4`, defer `addd12b`). The three service repos were not read for
this design.

## Scope

Shared: the e2e harness helpers (logging, evidence, Kubernetes access, waits,
failure dumps, the HTTP client, identity checks, pool placement) and the
guard test that keeps suite folders in step with the workflow.

Not shared: product fixtures, journeys, the raft and queue logic of each
service, the Job YAML, and each repo's workflow. Each repo keeps these.

No test, threshold or feature matrix is weakened in any repo. This work starts
no GKE run.

## Where the code lives

Decision: a new core crate `e2e-support` at `crates/e2e-support`. It is a
library with `publish = false`, used as a dev-dependency by the service repos.

Reason: core is the one repo that all service repos already pin by tag (see
[How downstream repos consume core](README.md#how-downstream-repos-consume-core)).
The `vat` repo is rejected as the home, because relay, defer and lumen do not
depend on `vat`.

What stays in each service repo:

- lumen: the 100k-document fixture and `hits_for` (product fixtures), the
  journeys in `cluster.rs`, the kustomize Job YAML under `k8s/gke-test-job/`,
  and `.github/workflows/gke-test.yml`.
- relay: the `/raftz` logic, the product fixtures, the Job YAML and its
  workflow.
- defer: the queue and task API, the StatefulSet, PDB and PVC expectations,
  the Job YAML and its workflow.

## Public API of e2e-support

| Item | Signature | Taken from |
|------|-----------|------------|
| `required` | `pub fn required(name: &str) -> String`; panics, never skips, when the env var is missing | lumen `support.rs` |
| `say` | `pub fn say(line: impl AsRef<str>)` | lumen |
| `evidence` | `pub fn evidence(suite: &str, step: &str, data: serde_json::Value)` | lumen |
| `Step` | `pub struct Step`; `begin(suite: &'static str, name: impl Into<String>) -> Self`; `end(self)` | lumen |
| `Kube` | `pub struct Kube { pub namespace: String }`; `from_env()`; `try_run(&self, args: &[&str]) -> Result<String, String>`; `run`; `get -> Value`; `exists(&self, what: &str) -> bool`; `pods(&self, label: &str) -> Value` | lumen |
| `wait_for` | `pub fn wait_for<T>(what: &str, timeout: Duration, probe: impl FnMut() -> Result<T, String>) -> T` | relay and defer |
| `wait_pods_exist`, `wait_ready` | `pub fn wait_pods_exist(kube: &Kube, label: &str, expected: usize, timeout: Duration)`; `wait_ready` takes the same arguments | lumen |
| `DumpOnFailure` | `pub struct DumpOnFailure<'a> { pub suite: &'static str, pub kube: &'a Kube, pub dump_commands: Vec<Vec<String>> }`; the dump commands are a parameter, not hard-coded | lumen, which hard-codes `lumens.lumen.dev` and `deploy/lumen-operator` |
| `Api` | `pub struct Api { pub base: String }`; `new(base: &str)`; `from_env()`; `async call(&self, method: reqwest::Method, path: &str, body: Option<&Value>, timeout: Duration) -> Value` with 5 transport retries; HTTP 4xx and 5xx panic; `get`, `post`, `put`, `answers`, `wait_for` | lumen |
| `is_dns_label`, `is_lower_hex` | `pub fn is_dns_label(s: &str) -> bool`; `pub fn is_lower_hex(s: &str, len: usize) -> bool` | lumen |
| `Identity` | `pub struct Identity { pub image, pub root_digest, pub runtime_digest, pub version, pub git_sha: String }`; the constructor takes `image_repo: &str` as a parameter; methods `assert_version`, `assert_template_image`, `assert_pods` as in lumen `identity.rs` | lumen; relay `Expected` |
| `normalize_runtime_image_id` | `pub fn normalize_runtime_image_id(raw: &str) -> Option<&str>`; handles `cri-o://`, `docker://` and text after `@` | lumen |
| `PoolPlacement` | `pub struct PoolPlacement { pub key: String, pub value: String, pub pull_secret: String }`; `assert_on_pool(&self, pod: &Value)`; `assert_pool_placement(&self, what: &str, workload: &Value)` | lumen `identity.rs`, `cluster.rs` |
| `named`, `env_count`, `mounts_named` | signatures as in lumen `cluster.rs` | lumen |

## Choices where the repos differ

| Topic | Choice | Reason |
|-------|--------|--------|
| Layout | Modules `support`, `identity` and `cluster`-style helpers. `Kube` is a struct. relay and defer keep thin wrappers for their free functions in their own repo. | One type carries the namespace, so no global state is needed. The wrappers keep relay and defer call sites unchanged. |
| HTTP | An async `Api` with transport retry. relay and defer wrap it in a runtime. | It is a superset of the blocking client: it has the retry that lumen has and the async path that the others need. |
| Evidence JSON | lumen's shape `{"suite","step","data"}`; see section 5. | The crate emits lumen's shape plus `ok`. Whether the readers accept `ok` is not checked here; relay and defer adopt the shape only after the decision in section 5. |
| Namespace env | `E2E_NAMESPACE`, with `NAMESPACE` read as a fallback. | relay and defer Jobs keep working without a change to their workflows. |
| Identity | The version must equal the caller's version argument. The runtime digest may equal the root digest only when the caller sets `allow_single_manifest = true`. | relay and defer set it and lumen does not, so each repo keeps its own check and no repo gets a weaker one. |
| Pool placement | In the crate, parameterised by `PoolPlacement`. | The key, value and pull secret differ per repo, and the check itself is the same. |
| Waits | All of `wait_for`, `wait_pods_exist`, `wait_ready` and `DumpOnFailure`. | Each repo already uses these; they are the same logic with different arguments. |
| imageID parse | lumen's `normalize_runtime_image_id`. | It is a superset of the other two parsers. |
| Guard test | A helper `assert_suite_guards(job_dir, workflow, suites)` checks the Job folders, the `job.yaml` filter and the workflow mention. The release-candidate workflow check stays in lumen. | The Job and workflow layout is the same in all three repos; the release-candidate check is lumen-only. |

## Evidence JSON and release receipts

`E2E_EVIDENCE {json}` lines are read by `scripts/gke-perf/extract-receipts.py`
(lumen) and by `scripts/release/make-gke-release-receipt.py` (relay and defer).
Neither script was read for this design.

relay and defer write `"ok": true` with flat detail. lumen writes `data`. So
the shared `evidence` emits `{"suite","step","ok":true,"data":{...}}` and no
other keys.

### Decision needed

A person must confirm that both receipt scripts accept this shape before relay
and defer adopt the crate. Until then the crate's first release keeps lumen's
exact shape, and relay and defer do not switch. Neither script is changed by
this design.

## Adoption by relay and defer

1. Pin core by tag in the repo's root `Cargo.toml`, as in the README.
2. Add `e2e-support` as a dev-dependency with the same tag.
3. Delete the local copy of the harness module. Keep the suites.
4. Run the repo's own `it` tests with `--ignored` only in the GKE Job.

A person opens one issue in relay and one in defer. This document does not
open them.

## Later pieces

- Performance gate helper: from lumen `perf_gate.rs`, as a module `perf` of
  `crates/e2e-support`. Thresholds are parameters. Defaults are never lowered.
- Security test templates (auth, authorization, TLS rotation): a module
  `security` of `crates/e2e-support`. Credentials and thresholds are
  parameters.

Child issues of #203 that cover these pieces, by title:

- "test: crate e2e-support with the shared e2e harness"
- "test: performance gate helper in e2e-support"
- "test: security test templates in e2e-support"
