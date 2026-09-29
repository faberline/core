//! The async counterpart to [`Verifier`], for credentials that cannot be
//! resolved from local state alone.
//!
//! [`Verifier::authenticate`] is synchronous, which is correct for every
//! verifier that answers from memory — a role-map lookup, an HMAC check. It
//! cannot express a verifier that must *ask someone else*: Google's token
//! introspection endpoint is a network round trip, and a signing key rotated
//! out from under a cached JWKS has to be refetched before the request can be
//! answered at all.
//!
//! Rather than make [`Verifier::authenticate`] async — which would break every
//! existing implementor for the benefit of one — this module adds a parallel
//! trait and leaves the synchronous one untouched.
//!
//! ## Choosing between them
//!
//! | | [`Verifier`] | [`AsyncVerifier`] |
//! |---|---|---|
//! | Answers from | memory | memory **or** an identity provider |
//! | Middleware | [`auth_middleware`](crate::auth_middleware) | [`async_auth_middleware`] |
//! | Cost of a miss | none | one upstream call |
//!
//! A synchronous verifier reaches the async middleware through [`AsAsync`].
//! There is deliberately no blanket `impl<V: Verifier> AsyncVerifier for V`:
//! it would collide with the direct implementations this trait exists to
//! carry, because a blanket impl over a local trait forecloses every explicit
//! one.

pub use crate::application::http::{async_auth_middleware, AsAsync, AsyncVerifier};
