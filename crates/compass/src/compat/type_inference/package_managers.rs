//! Package manager detection and integration (P1 M5.1)
//!
//! Automatically detects and integrates with Python package managers:
//! - uv (modern, fastest) - Priority 1
//! - Poetry (dependency resolution and packaging) - Priority 2
//! - Pipenv (virtual environment management) - Priority 3
//! - pip (standard package installer) - Priority 4 (fallback)
//!
//! Provides:
//! - Automatic project configuration detection
//! - Dependency parsing from lockfiles
//! - Virtual environment discovery
//! - Framework detection enhancement

pub use crate::domain::package::manager::{Dependency, PackageManager, PackageManagerDetection};
pub use crate::infrastructure::package::detector::PackageManagerDetector;
