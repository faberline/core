//! What a reviewed token is allowed to *be*.
//!
//! `TokenReview` answering `authenticated: true` is not the end of the
//! question. A GKE cluster's authenticator will happily authenticate a Google
//! user or a Google service account against kube-apiserver and hand back
//! `alice@example.com` or `svc@project.iam.gserviceaccount.com`. Those are
//! real, verified identities — they are simply not the kind of identity a
//! delegating service accepts, because accepting them would make the service a
//! second Google identity verifier with its own parallel authorization story.
//!
//! So the reviewed username has to *parse*, strictly, as
//! `system:serviceaccount:<namespace>:<name>` before anything else happens.
//! "Strictly" is doing real work here:
//!
//! - `system:serviceaccount:a:b:c` is not a ServiceAccount, it is a username
//!   with the right prefix and the wrong shape. Kubernetes would never mint it,
//!   so something else did.
//! - `system:serviceaccount::name` and `system:serviceaccount:ns:` name nothing.
//! - `system:anonymous` and the `system:unauthenticated` group are the
//!   apiserver's way of saying "nobody", which is never a caller.
//! - A namespace and a ServiceAccount name are DNS-1123 *labels*. Checking that
//!   is not pedantry: it is what makes the parse total, because a label cannot
//!   contain a colon, so the split above can never be ambiguous.

pub use crate::domain::k8s::{
    PrincipalRejection, ReviewedIdentity, ServiceAccountPrincipal, ServiceAccountRef,
    SERVICE_ACCOUNT_PREFIX,
};
