//! The lifecycle phase machine: phases and their legal edges, the published
//! observation, subscriptions to it, and the controller that drives
//! transitions and shutdown.

mod controller;
mod error;
mod observation;
mod phase;
mod subscription;

pub use controller::LifecycleController;
pub use error::{
    LifecycleDeadlineError, LifecycleError, LifecycleEventError, LifecycleSubscriptionError,
};
pub use observation::LifecycleObservation;
pub use phase::LifecyclePhase;
pub use subscription::{LifecycleEventSubscription, LifecycleSubscription};
