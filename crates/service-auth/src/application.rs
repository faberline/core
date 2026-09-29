//! Verifiers and ports: the HTTP verifier traits and middleware, the static
//! and hot-reloadable role-map verifiers, the Google verifier with its key
//! cache and upstream ports, and the Kubernetes delegated authenticator,
//! review port and TokenRequest source.

pub(crate) mod google;
pub(crate) mod http;
pub(crate) mod k8s;
pub(crate) mod role_map;
