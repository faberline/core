//! `cli-std` — the standard agent-facing CLI commands every axiom tool
//! ships, per the convention in `CONTRIBUTING.md` ("every CLI ships `llm`,
//! `upgrade`, `issue`"):
//!
//! - [`llm`] — offline self-documentation (how do I drive this?)
//! - [`upgrade`] — self-update from the tool's GitHub releases (am I current?)
//! - [`issue`] — search, view, file, and comment on diagnostics-rich GitHub
//!   issues; comments automatically reopen issues for post-closure verification
//!   failures.
//!
//! The logic is parameterized by a [`ToolInfo`] the calling binary fills from
//! its own build stamps, and is **clap-agnostic**: each CLI keeps its own clap
//! registration (derive or builder) and calls these `run` functions. The
//! network paths (`upgrade` install, `issue` search/view/create/comment`) live behind the
//! `online` feature; the offline paths (`llm`, `upgrade --check` messaging,
//! `issue create --dry-run` / pre-filled-URL fallback, `issue comment --dry-run`
//! / manual-comment fallback) always build.
//!
//! One more module rides along that is not a CLI subcommand: [`chainable`] —
//! a test harness (not a `run` function a binary calls) backing the
//! `chainable_output` archetype trait's gate: `CONTRIBUTING.md` § "CLI
//! convention: stdout tells the agent the next step".
//!
//! A fourth verb, [`connect`], rides behind the `k8s` feature: the
//! port-forward lifecycle + token-registry Secret resolution every
//! k8s-native service CLI's `<cli> connect` wants (extracted from `lumen
//! connect`, #1321/#1376 — see `CONTRIBUTING.md` § "Deploy artifacts").
//!
//! [`registry`], behind the `registry` feature, is the one clap-typed piece:
//! the link-time subcommand registry (`CliModule` + the `CLI_MODULES` linkme
//! slice) a main binary dispatches through instead of a hand-kept table.
//!
//! **Courier proxy mode** (#1320). With `$AXIOM_COURIER_URL` set, [`issue`]'s
//! four verbs route through courier's `/v1/issues/...` endpoints instead of
//! calling `api.github.com` directly, authenticating with
//! `$AXIOM_COURIER_TOKEN` — a courier-issued client credential, not a personal
//! GitHub token. Unset or blank means unconfigured, and unconfigured is not a
//! degraded mode: the direct-GitHub path runs unchanged.

mod application;
mod compat;
mod domain;
mod infrastructure;
mod interfaces;

pub use application::tool_info::ToolInfo;
#[cfg(feature = "k8s")]
pub use compat::connect;
#[cfg(feature = "registry")]
pub use compat::registry;
pub use compat::{artifact, chainable, issue, llm, report_issue, upgrade};

#[cfg(feature = "online")]
pub(crate) use infrastructure::confirm::confirm;
#[cfg(feature = "online")]
pub(crate) use infrastructure::courier::{resolve_courier_token, resolve_courier_url};
#[cfg(feature = "online")]
pub(crate) use infrastructure::github::{
    download_bytes, download_text, github_get, resolve_github_token,
};
#[cfg(feature = "online")]
pub(crate) use infrastructure::self_install::install_over_self;
