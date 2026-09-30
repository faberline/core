//! GitHub and courier issue endpoints, the optional node status probe, and
//! where the tracker settings come from.

pub(crate) mod courier_api;
pub(crate) mod github_api;
pub(crate) mod node_status;

use super::courier::resolve_courier_url;
use super::github::resolve_github_token;
use crate::domain::issue::tracker::TrackerAccess;

/// The [`TrackerAccess`] port over the process environment:
/// `$AXIOM_COURIER_URL`, then `$GH_TOKEN`, `$GITHUB_TOKEN` or `gh auth token`.
pub(crate) struct EnvTracker;

impl TrackerAccess for EnvTracker {
    fn courier_url(&self) -> Option<String> {
        resolve_courier_url()
    }

    fn github_token(&self) -> Option<String> {
        resolve_github_token()
    }
}
