//! `<cli> connect` — the k8s-native service CLI's port-forward lifecycle +
//! token-registry Secret resolution (feature `k8s`). Extracted from `lumen
//! connect` (#1321/#1376): every k8s-native service CLI wants the same
//! `kubectl port-forward` process lifecycle and the same token-registry
//! Secret convention (map key IS the bearer token), so this module owns the
//! reusable primitives. Each adopter supplies only its own flag surface,
//! CR-kind lookup convention, and role mapping into [`Role`] — see
//! `apps/lumen/src/bin/lumen.rs`'s `connect`/`resolve_token` for the
//! reference thin adapter.

pub use crate::application::connect::{resolve_cr_tokens_secret, resolve_token};
pub use crate::domain::connect::{
    cr_tokens_secret, select_token, Role, TokenClaims, TOKEN_REGISTRY_SECRET_KEY,
};
pub use crate::infrastructure::connect::{
    free_local_port, kubectl_get_json, secret_data_bytes, wait_for_local_port_ready, ChildGuard,
};
