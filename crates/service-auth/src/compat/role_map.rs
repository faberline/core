//! Static bearer-token role-map RBAC — the reusable model behind the
//! archetype's `<SVC>_AUTH=off|required` + `<SVC>_TOKEN_REGISTRY_FILE`
//! contract (originally lumen's hand-rolled `src/auth.rs`, generalized here
//! so keep/loom/relay/beam don't each fork it).
//!
//! ## Shape
//!
//! - [`Role`]: a hierarchy, `Admin` ⊇ `Write` ⊇ `Read`, compared with
//!   [`Role::covers`].
//! - [`TokenClaims`]: a bearer token's `subject` plus its `roles`, keyed by a
//!   generic **resource** string (lumen's `collection_id`, keep's
//!   `namespace`, ...). The literal key `*` is a wildcard grant applied when
//!   no more specific entry matches.
//! - [`Registry`]: the two-namespace credential registry (#2678) — bearer
//!   secrets in `tokens`, provider-verified identities (Google emails) in
//!   `identities`. Kept disjoint so a bearer secret shaped like an email can
//!   never match an identity entry.
//! - [`load_registry_files`]: load a [`Registry`] from several projected
//!   files ([`RegistrySource`]), unioning them for a service that resolves both
//!   namespaces.
//! - [`load_registry_file`]: parse a [`Registry`] from a single registry-file path.
//! - [`load_registry`]: the bearer-only loader — a registry-file path
//!   (production, mounted from a Secret) or legacy inline JSON, failing fast
//!   when auth is required but the resolved registry ends up empty, and
//!   refusing a document that carries identity-keyed entries it could not
//!   resolve. Env-var *naming* stays the caller's concern — this fn only knows
//!   the resolved values plus the label strings to use in error context, so the
//!   wording stays byte-identical to whatever env vars a service actually reads.
//! - [`StaticRoleMapVerifier`]: a [`Verifier`] over that registry —
//!   `authenticate` resolves a bearer token to a [`RoleMapPrincipal`] (or
//!   `Open`, in non-required/dev mode, when no token is presented).
//! - [`RoleMapPrincipal::ensure`]: the per-resource authorization check a
//!   handler runs after authentication — rejects (as a structured
//!   [`RoleMapDenied`]) unless the principal's claim on the resource (or its
//!   wildcard grant) covers the needed role.

pub use crate::application::role_map::StaticRoleMapVerifier;
pub use crate::domain::authorization::{
    Registry, Role, RoleMapDenied, RoleMapPrincipal, TokenClaims,
};
pub use crate::infrastructure::{
    load_registry, load_registry_file, load_registry_files, RegistrySource,
};
