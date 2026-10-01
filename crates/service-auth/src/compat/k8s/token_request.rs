//! Asking the apiserver for a ServiceAccount token, as the caller.
//!
//! The mirror image of [`super::projected`]. That module reads a token the
//! kubelet mounted for a workload; this one mints a token for a *human or
//! automation* that already has a Kubernetes credential of its own — a
//! kubeconfig, usually with an exec credential plugin behind it.
//!
//! ```text
//!   kubeconfig identity  --(exec plugin, TLS cert, whatever)-->  kube-apiserver
//!            |                                       RBAC: may this identity
//!            |                                       `create` the `token`
//!            |                                       subresource of *this one*
//!            v                                       ServiceAccount?
//!   TokenRequest(namespace, serviceAccount, audience, expirationSeconds)
//!            |
//!            v
//!   a short-lived, audience-bound token  ---->  the audience-bound service
//! ```
//!
//! ## Why the caller's own credential never goes any further
//!
//! The identity in the kubeconfig may be a Google account, a cloud IAM service
//! account, a client certificate, or an OIDC subject. None of that is any of
//! the callee's business, and none of it is forwarded: it authenticates to
//! kube-apiserver and stops there. What continues is the minted token, whose
//! `aud` is the callee and whose lifetime is minutes. That is the entire
//! reason to make this round trip rather than reusing the credential already
//! in hand — the caller's credential is long-lived, broadly scoped, and
//! addressed to somebody else.
//!
//! ## What is in this module and what is not
//!
//! Same split as the rest of `k8s`: everything that decides is pure and
//! tested, and only [`KubeTokenMinter`] opens a socket.
//!
//! - [`TokenRequestTarget`] is the request, validated. Its
//!   [`request_body`](TokenRequestTarget::request_body) is the literal JSON
//!   that goes on the wire, so a test can assert the audience and duration
//!   without a cluster and without the assertion being a restatement.
//! - [`MintedToken`] is the answer plus the server's expiry — which is
//!   authoritative and frequently shorter than what was asked for.
//! - [`TokenSource`] is the refresh clock: mint once, reuse until the token is
//!   near its end, mint again. It fails rather than presenting a token it
//!   knows is stale.
//! - [`TokenMinter`] is the seam. A fake implementation is what makes the
//!   refresh behaviour testable at all, since the interesting cases are an
//!   hour apart.
//!
//! Nothing here names a service or an audience. The audience is the callee's
//! to declare, and it belongs in the callee's own crate next to the
//! [`super::delegated`] configuration that checks it.

pub use crate::application::k8s::{
    MintedToken, TokenMinter, TokenRequestError, TokenRequestTarget, TokenSource,
    DEFAULT_EXPIRATION_SECONDS, MIN_EXPIRATION_SECONDS,
};

#[cfg(feature = "k8s")]
pub use crate::infrastructure::k8s::kube_token_minter::KubeTokenMinter;
