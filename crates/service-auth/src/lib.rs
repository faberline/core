//! Shared request-auth middleware for the ecosystem's HTTP services.
//!
//! This is the service-kit auth layer: the generic **extract -> verify ->
//! reject -> inject** plumbing plus a [`Verifier`] trait each service
//! implements. It owns the transport-level shape of authentication, not the
//! crypto and not per-resource authorization:
//!
//! - **Token crypto is elsewhere, except the static role-map.** keep and loom
//!   share scoped claim-check HMAC tokens via `crates/claim-token`; a service's
//!   [`Verifier`] *composes* that (its `authenticate` calls
//!   `claim_token::verify`). Services that instead want a static,
//!   config-driven token→role registry (the archetype's
//!   `<SVC>_AUTH=off|required` + `<SVC>_TOKEN_REGISTRY_FILE` shape) can use
//!   [`StaticRoleMapVerifier`] directly — lumen's original
//!   hand-rolled role-map RBAC, generalized here so keep/loom/relay/beam
//!   don't each fork it. This lib has no opinion on which scheme a service
//!   picks.
//! - **Authorization stays in handlers.** Per-resource policy (lumen's
//!   per-collection RBAC, keep's scope-vs-key) runs in the service's handlers
//!   on the concrete [`Verifier::Principal`], not here. The middleware only
//!   answers "who is this caller?" and injects that principal; "may they touch
//!   *this* resource?" is the handler's call.
//!
//! It layers onto a router built with `crates/service-http`'s data-plane routes:
//! attach [`auth_middleware`] with
//! [`from_fn_with_state`](axum::middleware::from_fn_with_state), passing the
//! service's `Arc<V>` verifier as state.
//!
//! ## Shape
//!
//! 1. A service defines a concrete principal type and a [`Verifier`] whose
//!    `authenticate` turns headers into that principal (or an [`AuthError`]).
//! 2. [`auth_middleware`] runs the verifier, injects the principal into request
//!    extensions on success, and renders the [`AuthError`] on failure.
//! 3. Handlers read the principal concretely via
//!    `axum::extract::Extension<MyPrincipal>` — no `Any`, no downcast.
//!
//! ```ignore
//! use axum::{extract::Extension, http::HeaderMap, middleware::from_fn_with_state};
//! use service_auth::{auth_middleware, bearer_token, AuthError, Verifier};
//! use std::sync::Arc;
//!
//! #[derive(Clone)]
//! struct Principal { subject: String }
//!
//! struct MyVerifier { /* secret / role-map / ... */ }
//!
//! impl Verifier for MyVerifier {
//!     type Principal = Principal;
//!     fn authenticate(&self, headers: &HeaderMap) -> Result<Principal, AuthError> {
//!         let token = bearer_token(headers).ok_or(AuthError::Unauthenticated)?;
//!         // e.g. claim_token::verify(secret, token, now) — crypto lives there.
//!         Ok(Principal { subject: token.to_string() })
//!     }
//! }
//!
//! async fn handler(Extension(p): Extension<Principal>) -> String { p.subject }
//!
//! let verifier = Arc::new(MyVerifier { /* ... */ });
//! let app = axum::Router::new()
//!     .route("/things", axum::routing::get(handler))
//!     .layer(from_fn_with_state(verifier, auth_middleware::<MyVerifier>));
//! ```
//!
//! ## When the credential cannot be judged locally
//!
//! [`Verifier::authenticate`] is synchronous, which suits every verifier that
//! answers from memory. A verifier that must *ask an identity provider* — the
//! Google paths in [`gcp`] — implements [`AsyncVerifier`] instead and attaches
//! via [`async_auth_middleware`]; a synchronous verifier reaches that same
//! middleware through [`AsAsync`]. Neither trait replaces the other, and no
//! existing [`Verifier`] implementation changed to make room for the second.
//!
//! ## Delegating both halves to Kubernetes
//!
//! A service whose callers are all Kubernetes workloads can skip having a
//! credential store at all. [`k8s`] delegates authentication to `TokenReview`
//! and authorization to `SubjectAccessReview`, so the only place policy lives
//! is `RoleBinding`s in the cluster. It accepts exactly one kind of caller —
//! a `system:serviceaccount:<ns>:<name>` identity holding an audience-bound
//! token — which is what keeps a delegating service from quietly becoming a
//! second identity provider for whatever the cluster's authenticator happens
//! to verify. That module names no service's resources; a caller maps its own
//! operations onto [`k8s::ResourceAttributes`] (#2869).
//!
//! ## Static role-map RBAC
//!
//! The static bearer-token role-map RBAC is the reusable model behind the
//! archetype's `<SVC>_AUTH=off|required` + `<SVC>_TOKEN_REGISTRY_FILE`
//! contract (originally lumen's hand-rolled `src/auth.rs`, generalized here
//! so keep/loom/relay/beam don't each fork it).
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
//!
//! ## Two credential namespaces, deliberately disjoint
//!
//! [`Registry`] holds bearer secrets (`tokens`) and provider-verified
//! identities (`identities`) in two maps, not one. The key of the first is a
//! *secret* — knowing it is the proof. The key of the second is a *public
//! email* — knowing it proves nothing, and only a verified Google credential
//! may reach it. Sharing one map would mean a bearer secret spelled like an
//! email silently grants that identity's roles to anyone who read it off a CR.
//! A service that resolves only secrets keeps using [`load_registry`], which
//! now rejects rather than ignores an identity-keyed document it cannot honour;
//! a service that resolves both uses [`load_registry_files`] (#2678).

mod api;
mod app;
mod application;
mod domain;
mod infrastructure;
mod interfaces;

pub use api::{gcp, k8s, llm, reload};
pub use application::google::{Credential, GoogleVerifier, JwksSource};
pub use application::http::{
    async_auth_middleware, auth_middleware, bearer_token, AsAsync, AsyncVerifier, AuthError,
    Verifier,
};
pub use application::role_map::{
    spawn_registry_file_watcher, spawn_registry_file_watcher_with_interval,
    ReloadableRoleMapVerifier, StaticRoleMapVerifier, DEFAULT_REGISTRY_FILE_WATCH_INTERVAL,
};
pub use domain::authorization::{
    AuditedRoleMapPrincipal, AuthEvent, AuthEventSink, AuthorizationDecision, AuthorizationReason,
    NoopAuthEventSink, Registry, RegistryError, ReloadFailure, Role, RoleMapDenied,
    RoleMapPrincipal, TokenClaims,
};
pub use domain::google::{
    AccessTokenIntrospection, GoogleAuthConfig, GoogleAuthError, InvalidReason,
};
pub use infrastructure::{
    load_registry, load_registry_file, load_registry_files, RegistrySource, TracingAuthEventSink,
};
pub use interfaces::scoped::{
    scoped_authorization_middleware, ScopedAuthorization, ScopedAuthorizationOutcome,
};

#[cfg(test)]
mod tests;
