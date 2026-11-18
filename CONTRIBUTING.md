# Contributing

This is the Faberline core repository, a Rust workspace of shared crates (`crates/*`).
Repository-wide rules are here. Each crate's own file says how to change that crate.

## Crates

- `crates/build-stamp`
- `crates/claim-token`
- `crates/cli-std`
- `crates/compass`
- `crates/e2e-support`
- `crates/index-text`
- `crates/metrics-prometheus`
- `crates/metrics-remote-write`
- `crates/openapi-codegen`
- `crates/peer-tls`
- `crates/raft-core`
- `crates/raft-runtime`
- `crates/server-http`
- `crates/server-lifecycle`
- `crates/server-tcp`
- `crates/service-auth`
- `crates/service-backup`
- `crates/service-collector`
- `crates/service-executor`
- `crates/service-http`
- `crates/service-k8s`
- `crates/service-mcp`
- `crates/service-observability`
- `crates/service-projection`
- `crates/shard-routing`
- `crates/storage-durable`
- `crates/storage-object`
- `crates/storage-segment`
- `crates/surface`
- `crates/transport-h2c`
- `crates/transport-otlp`
- `crates/ui-runtime`

## Verification

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
```

CI (`.github/workflows/ci.yml`) runs them, and `ci/check-*.sh` hold the other CI shape checks.

## Shared delivery entrypoint

Delivery runs through one shared skill, `aw-worker` (`/faberline:aw-worker`),
from [faberline/aw at a pinned commit](https://github.com/faberline/aw/tree/6807ca7eb10a8d95eb13295e417ba36c2721c75e/plugins/faberline/skills/aw-worker).

One agent does triage, spec, tdd and review in stages, selected by the issue's
`stage:` labels. `aw` opens and merges the pull request. CI alone owns done: an
issue is done only when the required CI gates pass on its current inputs.
Review runs no tests.

The authoring conventions below still apply. The `aw-worker` spec stage writes
each issue contract, and its review stage reads the authored sections.

## Standards

Shared standards for services, CLIs and docs live in <https://github.com/faberline/aw/tree/main/docs/standards>.

## Service auth — one Bearer-token contract

*(policy-only — judgment, not trait-enforced; candidate for a future
`service_auth` trait once `crates/service-auth` adoption is mechanically
checkable)*

Service auth is shared infrastructure, not a per-project design space. Every
long-running service uses `crates/service-auth` for request authentication:
extract `Authorization: Bearer <token>`, verify it through a service-supplied
`Verifier`, reject with the shared JSON error shape, and inject the authenticated
principal into handlers. Services use the shared registry verifier
(`service_auth::StaticRoleMapVerifier` — role hierarchy, wildcard
grants, registry-file loader) or signed tokens through `crates/claim-token`, but
the HTTP contract and middleware shape stay the same.

Production server config follows one env pattern:

```
<SVC>_AUTH=required
<SVC>_TOKEN_REGISTRY_FILE=/var/run/secrets/<svc>/token-registry.json
```

Local/dev may use `<SVC>_AUTH=off`. Production must not depend on inline token
lists as the primary path. In Kubernetes or cloud-native deployments, the
operator/render layer mounts the registry file from a Kubernetes Secret, CSI
Secret Store, or cloud Secret Manager sync, and owns the env wiring. Clients do
not need connection strings for auth; they use `<SVC>_URL` for routing and
`<SVC>_TOKEN` for credentials, then send `Authorization: Bearer <token>`.

**The registry's map key is never the credential.** A document keyed by bearer
secret cannot be read by anyone not also entitled to authenticate as everyone in
it, so the permission table — the thing an operator most needs to review —
inherits the blast radius of the secrets. The registry carries **two disjoint
key namespaces**: `tokens` keys are bearer secrets, `identities` keys are
provider-verified subjects (a Google email, a Kubernetes ServiceAccount). A
presented bearer value resolves against `tokens` only and a verified identity
against `identities` only, so a secret whose text happens to be a valid email
can never inherit that email's grants, and an `identities`-only registry holds
no credential at all — it is ordinary reviewable configuration.

A section-less document stays valid and is read entirely as `tokens`, which is
what lets a deployed registry survive the change untouched. **The
discriminator has two clauses, and the second is not optional**: a document is
sectioned only when every top-level key is `tokens` or `identities` *and* no
top-level value carries a `subject` field. Without the second clause a flat
registry whose one secret is literally spelled `tokens` is read as a section
and silently loses its only credential. Any second decoder — a CLI resolving a
dev token, a migration tool — must replicate both clauses and pin the copy with
a test; `crates/cli-std`'s `connect::bearer_secrets` is the reference, duplicated
rather than shared because `service-auth` depends on `cli-std` and not the
reverse.

**Authentication is on unless a CR asks for it off, and there is exactly one
place a registry can come from.** The operator's `spec.auth` defaults to
`required`, not to off — a field that fails open is a field that ships
unauthenticated whenever someone forgets it. When both a Kubernetes Secret and
a CSI `SecretProviderClass` name a registry, reject the CR with an
`x-kubernetes-validations` CEL rule at `kubectl apply` instead of resolving it
with a documented precedence: with a silent winner there is no way to tell
which registry is actually being served, and the loser looks applied.

Two spellings that look interchangeable and are not: the CRD enum variant is
**`disabled`**, the process env value is **`off`**. Kubernetes parses CRs as
YAML 1.1, where a bare `off` is the boolean `false` — so a `type: string` field
defaulting to `"off"` rejects the very value its own description tells readers
to write, and the error names a type mismatch rather than the missing quotes. A
free-string auth field is worse still: `auth: requred` is accepted and read as
"not required", which turns a typo into an open service. Use a unit enum
(`#[serde(rename = "disabled")]`) — structural schemas represent those fine;
it is only enums with divergent per-variant schemas they cannot express.

Keep the boundary explicit: `crates/service-auth` authenticates callers;
service handlers authorize per resource, tenant, collection, queue, workflow, or
admin action. Standard probe/spec/scrape endpoints stay auth-exempt according to
the standard-endpoint contract above.

## Test layout — `tests/it/`, one integration binary per crate

Every `crates/<p>` crate judges its externally observable behavior under
`tests/`. It is written in Rust, it runs under plain `cargo test -p <crate>`,
and Cargo discovers it: the manifest does not list test targets.

| Where | What lives there | How it runs |
|---|---|---|
| `crates/<p>/tests/it/main.rs` + `tests/it/<case>.rs` | externally observable behavior, one module per case, all in one test binary | `cargo test -p <crate> --test it -- <case>::` |
| `crates/<p>/tests/<name>.rs` | a standalone binary, allowed only as **A standalone binary needs a reason** below says | `cargo test -p <crate> --test <name>` |
| `crates/<p>/src/**` | rules observable only inside the implementation, colocated | `cargo test -p <crate> --lib` |

These rules make the tree honest:

- **One binary per crate.** `tests/it/main.rs` is a plain list: `mod support;`
  when the crate has shared helpers, then one `mod <case>;` per case in
  alphabetical order, where the module name is the case's file name. That list
  is the inventory. A file under `tests/it/` that `main.rs` does not declare is
  never compiled, so a case starts or stops running only by a line in the diff.
  One binary links the crate and its dependencies once instead of once per
  case. A crate with a single case may keep it as `tests/<name>.rs`, and moves
  it into `tests/it/` when the second case lands. `crates/raft-runtime` is the
  reference shape: one `it` binary plus two separate binaries, each with its
  reason in its header.
- **A standalone binary needs a reason.** A `tests/<name>.rs` file outside
  `tests/it/` is allowed only when one of these holds:
  1. It is the crate's only test file: no other `tests/<name>.rs` and no
     `tests/it/`.
  2. It contains one of the `test_isolation_markers` of the retired `rust_arch`
     policy (`set_var(`,
     `remove_var(`, `#[global_allocator]`). These change process-global state,
     so the file must not share a process with other cases.
  3. Its leading `//!` block has a line `//! isolation: <reason>` with a
     non-empty reason. The block runs from the top of the file to the first
     line that is not blank, a `//!` line, or a `#![..]` attribute.

  Every other test file moves into `tests/it/`.

  The cases in `it` run concurrently in one process, so they share its global
  state. A reason names that state: the case changes process-global state the
  other cases can see — `#[global_allocator]`, `set_var` of an environment
  variable read by code other cases reach (child processes inherit it too),
  `set_current_dir`, a global tracing subscriber or metrics recorder, a signal
  to itself, `setrlimit` — or its assertion or gate evidence is a
  whole-process measurement: allocation counts, RSS, `getrusage(RUSAGE_SELF)`,
  a process-global counter. Being slow or flaky is not a reason.
- **Test data is not a test.** `.rs` files under `tests/fixtures/`,
  `tests/compile_fail/`, and `tests/ui/` (the checker's `test_data_dirs`) are
  inputs the cases read. The rules in this section do not apply to them.
- **Proptest seeds move with the case.** proptest writes the failing seed of a
  case in `tests/it/<module>.rs` to `tests/proptest-regressions/<module>.txt`.
  The old seed file beside the source, `tests/<name>.proptest-regressions`, is
  no longer replayed once the case moves, so move it to the new path in the
  same commit.
- **Feature gates live in the module.** A case that needs a feature starts with
  `#![cfg(feature = "…")]` right after its `//!` block, and a support module
  used only by gated cases carries the same gate. A separate binary keeps
  `required-features` in a `[[test]]` stanza — the only stanza a manifest may
  carry. Check a crate with `-p` alone and with each feature it gates on: under
  `--workspace`, feature unification can enable a feature a case forgot to
  require.
- **Shared helpers are declared once.** They live in `tests/it/support.rs`,
  with any submodules under `tests/it/support/` (no `mod.rs`), declared by
  `main.rs` and reached as `crate::support::…`, so a `static` in them exists
  once per process. A separate binary includes what it needs with
  `#[path = "it/support/<x>.rs"] mod <x>;`. Non-Rust files the cases use sit at
  the `tests/` root, not beside the modules.
- **Filter by module.** Test names are `<case>::<fn>` and libtest filters by
  substring, so `-- <case>::` runs one case and `--exact` needs the full
  `<case>::<fn>`. No case name may be a suffix of another (`drain` beside
  `graceful_drain` would make `drain::` match both). A case that re-runs its
  own binary through `std::env::current_exe()` must pass `--exact
  <case>::<fn>` to the child. With no filter the child runs every case in the
  crate, and a bare `--exact <fn>` matches nothing, so the child passes
  without running.
- **Write the case first.** The case lands before the `src/**` change it
  judges, and must be observed failing for the stated reason before the change
  makes it pass. A case written afterwards is fitted to the implementation it
  was supposed to constrain.

`crates/<p>/tech-design/`, `crates/<p>/external-contracts/`, and
`crates/<p>/e2e/` are superseded: no authored source belongs in them. Do not
create one, do not add a file to one, and migrate a surviving case into
`tests/` rather than editing it in place. The Python spec model and its
`src/cases/*.py` verifiers are retired across the repository — never author a
new one. The only thing that may still appear under `external-contracts/` is
generated evidence written by an EC gate run, which is output, never contract.
The behavior contracts that existed before this rule now live, read-only, under
`crates/<p>/docs/contracts/`; they record what the tests were written against
and are not edited.

## Project build and release contract

Every project `build.sh` must use the same two-mode contract. The per-project
`<project>-build-{debug,release}` skill wrappers are deleted (2026-09-02), and
`lumen-build-release` folded into `build-release` (2026-09-04):
`build-release <app>` is the candidate-first release chain for lumen, tape,
sift, keep, relay, and defer, and its first step is the release mode below. A
plain compile is `cargo build -p <crate>` directly; `build-debug` is a local
kind cluster run for keep/defer/relay/loom and not this contract. The
versioned debug/release modes below run the downstream app's own
`build.sh` from that app's repository root.

### Debug builds

Debug builds are local install checkpoints:

1. If the tree is dirty, commit first with a debug checkpoint commit.
2. Read the project's configured base version and check whether
   `<project>@<version>` already exists locally or on `origin`.
3. If the tag exists, advance to the next version using the repository base-64
   carry rule: patch increments first; when patch would exceed 63, reset patch
   to 0 and increment minor; when minor would exceed 63, reset minor to 0 and
   increment major.
4. Build with the Cargo debug profile and a SemVer build-metadata suffix:
   `<base-version>+<git-sha>`.
5. Install the debug binary locally, then restore manifest and lockfile edits so
   the debug-only version suffix is not left in the worktree.

### Release builds

Release builds are not complete until the GitHub Release is visible, and the
tag is never the first artifact. The required chain is `build-release
<project>`, whose steps are:

1. **release-prep**: run the project `build.sh release` from the repository
   root. A legacy script checks local and remote tag collisions for
   `<project>@<version>`, advances the version with the same base-64 carry rule
   when needed, runs the Cargo release build, installs the local release binary,
   commits version files as `release(<project>): <project>@X.Y.Z`, and prints
   `RELEASE_TAG=<project>@X.Y.Z`. A lumen-shaped script (lumen, tape, and every
   app listed in `scripts/release/apps.sh`) reads the version from
   `Cargo.toml`, syncs the Kubernetes image pins, and builds without bumping or
   committing; the version bump is an ordinary commit. Neither may create or
   push the tag.
2. **land**: run `git-land` as-is so the release commit lands on `main`. Do not
   tag before this step; rebases and squash merges can orphan a pre-land tag.
3. **candidate**: `scripts/release/candidate.sh <project> <version> <sha>`
   dispatches `<project>-release-candidate.yml` from the landed `main` sha,
   watches it, and downloads the immutable candidate bundle (archives, digest
   pinned image, signature, provenance, SBOM attestations, final manifest).
4. **verify**: the project's `verify-release-candidate.sh --mode full` re-proves
   the bundle independently of the workflow's own verdict.
5. **GKE gate**: the candidate image `ghcr.io/faberline/<project>@<digest>`
   is deployed and verified on GKE by the project's harness, and the evidence
   is bound into `<project>-gke-receipt.json` plus its `.sha256` sidecar.
6. **promote**: `scripts/release/promote.sh` creates the one annotated
   `<project>@X.Y.Z` tag at the candidate's exact commit, dispatches
   `<project>-release.yml` at that tag with the receipt, and watches it; the
   promotion retags the same digest as semver and `latest` and publishes the
   GitHub Release from the candidate bytes, never rebuilding.
7. **public verification**: `verify-release-artifacts.sh --mode public` must
   pass before the release is reported complete or the tracker is closed.

`scripts/project-build-monitor-release.sh` remains for projects still on a
tag-triggered `<project>-release.yml`; it is not part of the chain above.

The release identity is always:

```
<project>@X.Y.Z        # e.g. lumen@0.4.4, vat@0.3.62
```

Projects should release **independently**. A releasing project should prefer its
own `version` in its project `Cargo.toml`, so bumping one project does not bump
the others. A few crates still inherit `[workspace.package].version`; until they
are migrated, their `build.sh` must explicitly name the version source and every
manifest it edits. Do not silently bump the workspace version for an unrelated
project release.
