# core repository instructions

core is the shared Rust workspace behind the faberline services and CLIs (transport, server runtime, consensus, storage, auth, observability, Kubernetes integration, CLI standard commands).

## Output style

- Write in the user's language. Keep paths, commands and exact output contracts intact.
- Use ASD-STE100 Simplified Technical English: active voice, short sentences, one idea in each sentence.
- Explain a technical term when you first use it.
- Report what changed, what passed, what is unknown and the next action.
- Offer at most two choices and recommend one.

## Scope

- The repository is a Rust workspace of shared crates in `crates/*`.
- Repository-wide rules are in `CONTRIBUTING.md`. Each crate's own `CONTRIBUTING.md` says how to change that crate.
- Docs are in `docs/README.md`.
- The repository is public. Write no secret, token, private host name, e-mail address or personal path.
- A test is evidence for local behavior only. It is not evidence for a live GKE or production claim.

## Issue work

- Work on a claimed issue only through the `aw` CLI: `aw claim`, the work, `aw report`.
  `aw` does the branch, push, pull request, merge and signal comment.
- Edit labels only in the plan step of the `aw-worker` skill.

## Verify

Run these from the repository root. `ci` runs the first three:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked --exclude service-k8s --exclude openapi-codegen --exclude service-backup -- --include-ignored`
- Quick check of one crate (not a CI command): `cargo test -p <crate>`

Do not delete or weaken a test. Do not lower `line` in `[standards.coverage]` of `aw.toml`.
