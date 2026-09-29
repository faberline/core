//! Framework support (Sprint 6 - Track 1)
//!
//! Provides type inference for popular Python frameworks:
//! - Django: models, views, templates
//! - Flask: routes, blueprints
//! - FastAPI: endpoints, dependency injection
//! - Pydantic: models, validators
//! - SQLAlchemy: ORM mappings

pub use crate::domain::frameworks::detection::{Framework, FrameworkDetection};
pub use crate::domain::frameworks::django::{
    DjangoField, DjangoFieldType, DjangoModel, DjangoRelation, DjangoRelationType,
    DjangoTypeProvider,
};
pub use crate::domain::frameworks::fastapi::{FastAPIEndpoint, FastAPITypeProvider};
pub use crate::domain::frameworks::provider::{FrameworkTypeProvider, MethodType};
pub use crate::domain::frameworks::pydantic::{
    PydanticConfig, PydanticExtra, PydanticField, PydanticModel, PydanticTypeProvider,
};
pub use crate::domain::frameworks::registry::FrameworkRegistry;
pub use crate::infrastructure::frameworks::detector::FrameworkDetector;
