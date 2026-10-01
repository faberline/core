//! Shared peer-mTLS material loading for the ecosystem's mutually
//! authenticated peer/replication ports.
//!
//! PEM cert/key/CA-bundle loading, rustls server/client config builders, and
//! the `Once`-guarded default-crypto-provider install are identical for
//! every service with a peer transport — only the env var *names* are
//! service-specific. [`PeerTlsConfig::from_env`] takes a caller-supplied
//! prefix (lumen passes `"LUMEN_PEER"`) and derives `<prefix>_TLS_CERT` /
//! `<prefix>_TLS_KEY` / `<prefix>_TLS_CA` / `<prefix>_MTLS` from it; the rest
//! of the logic — chain/key/CA parsing, the server/client builders, the
//! mTLS-required client-cert-verifier wiring — is the same for every caller.
//!
//! Lifted verbatim from lumen's `tls.rs` (#971): lumen keeps a thin adapter
//! over this crate with its `LUMEN_PEER_TLS_*`/`LUMEN_PEER_MTLS` env names
//! and pub API unchanged. keep/relay/beam adoption is out of scope here.

mod config;
pub mod material;
mod provider;
pub mod reload;

pub use config::PeerTlsConfig;
pub use material::{
    validate, IdentityExpectation, MaterialPem, Rejection, RejectionReason, ValidatedMaterial,
};
pub use provider::install_default_crypto_provider;
pub use reload::{
    spawn_material_watcher, FileMaterialSource, MaterialSource, MemoryMaterialSource,
    ReloadableTls, TlsReloadStatus, TlsRuntimeProfile, DEFAULT_MATERIAL_POLL_INTERVAL,
};
