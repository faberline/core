//! Issue bodies, request payloads, created-issue responses and URLs, and the
//! ports to the tracker.

pub(crate) mod body;
pub(crate) mod created;
pub(crate) mod payload;
#[cfg(feature = "online")]
pub(crate) mod tracker;
pub(crate) mod url;
