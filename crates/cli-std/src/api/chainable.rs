//! Chainable-output conformance — the reusable check a project's
//! `chainable_output` baseline capability cites as its gate
//! (`CONTRIBUTING.md` § "CLI convention: stdout tells the agent the next
//! step" — anchor `chainable-output-conformance`).
//!
//! The convention recognizes two real shapes, both accepted here:
//!
//! - **The full `aw.cli.v1` envelope** — `aw`'s reference implementation,
//!   split across two real call sites: `apps/agentic-workflow/src/runtime/envelope.rs`'s
//!   `Envelope::Dispatch` carries a runnable step at `invoke.command`;
//!   `apps/agentic-workflow/src/cli/run.rs`'s `WorkflowEnvelope` (the
//!   `aw run` loop-driver output) carries it at `next.command` instead, and
//!   its sole terminal marker is `completion.workflow_complete == true` (a
//!   terminal envelope omits `next.command` entirely — see
//!   `workflow_envelope_serializes_optional_artifact_quality_profile` in
//!   `run.rs` for the real serialized shape).
//! - **The lightweight form** every other CLI may use instead per
//!   CONTRIBUTING's "Simple CLIs without a full envelope MAY use a lighter
//!   conforming form": either a single top-level JSON `next` field (a
//!   command string, or the literal `"done"`), or — for CLIs that emit plain
//!   text — a fixed trailing stdout line `next: <cmd>` / `next: done`.
//!
//! An output matching none of these is a chainable-output defect: the agent
//! has no way to know what happens next. [`assert_chainable`] is the check.

pub use crate::application::chainable::{assert_chainable, ChainableViolation};
