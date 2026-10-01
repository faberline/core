//! Python code generators for cclab ecosystem
//!
//! Generators for:
//! - cclab.shield (data validation models)
//! - cclab.titan (PostgreSQL ORM)
//! - cclab.nebula (MongoDB documents)
//! - cclab.photon (HTTP client)
//! - cclab.quasar (API routes)
//! - cclab.meteor (event handlers)
//!

pub mod meteor;
pub mod nebula;
pub mod photon;
pub mod quasar;
pub mod rust_scanner;
pub mod shield;
pub mod test_extractor;
pub mod titan;

pub use meteor::SwarmGenerator;
pub use nebula::NebulaGenerator;
pub use photon::PhotonGenerator;
pub use quasar::QuasarGenerator;
pub use rust_scanner::{
    RustEnum, RustEnumVariant, RustExports, RustField, RustFunction, RustMethod, RustParam,
    RustScanner, RustStruct, StructKind,
};
pub use shield::ShieldGenerator;
pub use test_extractor::{RustTest, TestExtractor, TestExtractorConfig};
pub use titan::TitanGenerator;

pub use crate::infrastructure::codegen::python::{
    format_to_python_type, get_type_imports, type_to_python,
};
