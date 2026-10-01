//! `<tool> upgrade` — self-update the installed binary from the tool's own
//! GitHub releases (`<project>@X.Y.Z` tags, `<project>-<target>.tar.gz` assets
//! with a `.sha256` sidecar, inner layout `<project>-<target>/<project>`).
//!
//! Pure version/asset/checksum/extraction logic is unit-tested; the HTTPS
//! download + atomic self-replacement live behind the `online` feature.

#[cfg(feature = "online")]
pub use crate::app::upgrade::run;
#[cfg(not(feature = "online"))]
pub use crate::application::upgrade::run;
pub use crate::application::upgrade::{
    decide_action, extract_binary, parse_tag, select_version, verify_sha256, Action, Options,
};
