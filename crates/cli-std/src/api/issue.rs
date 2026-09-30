//! `<tool> issue <verb>` — the shared issue interface every CLI ships.
//!
//! - [`search`] — find this tool's issues on the tracker (filtered to the
//!   `app:<name>` label), optionally by free text. Read-only.
//! - [`view`] — print a single issue by number. Read-only.
//! - [`create`] — assemble a diagnostics block + the operator's description and
//!   file a GitHub issue (`POST /repos/{repo}/issues` via `GITHUB_TOKEN`), or
//!   print a pre-filled `issues/new` URL when no token is available.
//!   `--dry-run` prints without submitting.
//! - [`comment`] — add a diagnostics-rich follow-up comment and ensure the
//!   issue is open first, for downstream/user verification failures after
//!   closure.
//!
//! Body assembly / URL pre-fill / repo resolution / payload shaping are pure and
//! unit-tested; everything network-facing lives behind the `online` feature.
//!
//! **Courier proxy mode** (#1320). `$AXIOM_COURIER_URL` being set routes all
//! four verbs through courier's `/v1/issues/{owner}/{name}...` endpoints; unset
//! or blank falls through to the direct `api.github.com` path, and that
//! fallback is contractually **byte-identical** to what this module sent before
//! courier existed. That contract is why every URL is built by a named pure
//! function — `github_search_url` / `courier_search_url` and their siblings —
//! rather than formatted inline inside the verb.
//!
//! Their purity is also the entire test strategy here: this crate carries no
//! HTTP-mock dev-dependency, so proxy-mode routing is verified by asserting the
//! exact request shape each verb hands to those builders, never by a live
//! round trip. A case that needs a real response does not belong in this crate.

#[cfg(feature = "online")]
pub use crate::app::issue::{comment, create, search, view};
#[cfg(not(feature = "online"))]
pub use crate::application::issue::comment::comment;
pub use crate::application::issue::comment::CommentOptions;
#[cfg(not(feature = "online"))]
pub use crate::application::issue::create::create;
pub use crate::application::issue::create::CreateOptions;
pub use crate::application::issue::diagnostics::{followup_comment_body, render_diagnostics};
pub use crate::application::issue::labels::report_labels;
pub use crate::application::issue::repo::resolve_repo;
#[cfg(not(feature = "online"))]
pub use crate::application::issue::search::search;
pub use crate::application::issue::search::SearchOptions;
#[cfg(not(feature = "online"))]
pub use crate::application::issue::view::view;
pub use crate::domain::issue::body::assemble_body;
pub use crate::domain::issue::payload::{comment_payload, issue_payload};
pub use crate::domain::issue::url::prefilled_url;
