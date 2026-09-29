# cli-std

## Brief

`cli-std` provides the standard agent-facing command implementations shared by
axiom CLIs: `llm`, `upgrade`, and `issue`, plus chainable output helpers.

## Capabilities

A promise with no gate under it is not claimed.

### Capability Index

| Capability | Root WI | Notes |
|---|---:|---|
| Standard Agent CLI Commands | - | shared llm, upgrade, issue, and chainable output APIs |
| CLI Module Auto Registration | - | link-time CLI subcommand registry behind the `registry` feature |

### Standard Agent CLI Commands

Projects can expose consistent agent-facing CLI commands without duplicating
GitHub issue, self-update, or LLM orientation logic.

- Root WI: none; this capability predates the tracker.
- Surfaces: Rust API: `cli_std::{llm, upgrade, issue, chainable}`.
- Gate — behavior: `cargo test -p cli-std` - shared CLI command contract
  coverage
- Gate: `cargo test -p cli-std`
- Source: `crates/cli-std/src/application/{llm,upgrade,issue,chainable}.rs`
- Evidence: `cargo test -p cli-std`; crates/cli-std/src/application/{llm,upgrade,issue,chainable}.rs

### CLI Module Auto Registration

Rust crates self-register CLI subcommands through a shared `CliModule` trait
and `linkme` distributed slice, so the main CLI discovers command definitions
and dispatches them without a hand-maintained central command table. It sits
behind the `registry` feature because it is the only clap-typed API here.

- Root WI: none; this capability predates the tracker (formerly the
  monorepo's `cclab-cli-registry`).
- Surfaces: Rust API: `cli_std::registry::{CliModule, CLI_MODULES,
  find_module, registered_names}`.
- Gate — behavior: `cargo test -p cli-std --features registry` - module
  registry access and name inventory behavior
- Gate: `cargo test -p cli-std --features registry`
- Source: `crates/cli-std/src/application/registry.rs`
- Evidence: `cargo test -p cli-std --features registry`; crates/cli-std/src/application/registry.rs
