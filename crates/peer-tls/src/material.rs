//! What a candidate TLS update must prove before anything starts using it.
//!
//! #3112 R3. The property that matters is not "these bytes parse" — it is "a
//! handshake against these bytes would succeed". So validation runs through
//! rustls' own [`WebPkiServerVerifier`](rustls::client::WebPkiServerVerifier) / [`WebPkiClientVerifier`](rustls::server::WebPkiClientVerifier) rather than a
//! hand-rolled chain walk: whatever the handshake would reject, this rejects,
//! for the same reason and at the same instant. A validator that only parsed PEM
//! would happily activate a leaf signed by a CA nobody trusts, and the failure
//! would surface one hop later as a client-side error nobody could attribute
//! back to the rotation that caused it.
//!
//! Two checks rustls' verifiers do not cover are done here directly against the
//! leaf: the SPIFFE URI SAN (webpki verifies DNS names, not URI names) and the
//! validity window as a *number*, which the reload state needs in order to
//! report seconds-to-expiry and to refuse to keep serving past it (R6, R7).

mod candidate;
mod parse;
mod rejection;
mod validated;
mod validation;

pub use candidate::{IdentityExpectation, MaterialPem};
pub use rejection::{Rejection, RejectionReason};
pub use validated::ValidatedMaterial;
pub use validation::validate;
