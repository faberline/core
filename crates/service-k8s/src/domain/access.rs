//! Domain model and validation for app-declared access grants (RBAC).

pub mod condition;
pub mod error;
pub mod grant;
pub mod policy;
pub mod rule;
pub mod subject;

pub use condition::{access_policy_condition, GrantOutcome, ACCESS_POLICY_READY_CONDITION};
pub use error::AccessGrantError;
pub use grant::AccessGrant;
pub use policy::validate_policy;
pub use rule::AccessRule;
pub use subject::AccessSubject;
