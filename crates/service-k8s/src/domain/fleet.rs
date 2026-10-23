//! Pure Fleet decision domain: child keys, ownership, adoption, desired-state hashes,
//! prune decisions, and Fleet reconciliation planning.

pub mod adoption;
pub mod child_key;
pub mod desired_hash;
pub mod ownership;
pub mod plan;
pub mod plan_error;
pub mod prune;

pub use adoption::AdoptionPolicy;
pub use child_key::ChildKey;
pub use desired_hash::{canonicalize_json, desired_hash};
pub use ownership::Ownership;
pub use plan::{plan_fleet, DesiredChild, FleetAction, FleetPlan, ObservedChild};
pub use plan_error::FleetPlanError;
pub use prune::PrunePolicy;
