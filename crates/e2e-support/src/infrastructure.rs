//! Infrastructure: environment, step output, `kubectl`, waits and the HTTP client.

mod api;
mod env;
mod kube;
mod security;
mod step;
mod wait;

pub use api::Api;
pub use env::{namespace, required};
pub use kube::{DumpOnFailure, Kube};
pub use security::{
    assert_accepts_credential, assert_rejects_expired_certificate,
    assert_rejects_missing_credential, assert_rejects_wrong_role, assert_rotation,
    assert_serves_certificate,
};
pub use step::{evidence, say, Step};
pub use wait::{wait_for, wait_pods_exist, wait_ready};
