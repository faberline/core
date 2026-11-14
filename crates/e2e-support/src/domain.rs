//! Pure e2e domain: DNS and hex checks, image ids, evidence, pool placement,
//! inspect helpers and the build identity.

mod evidence;
mod identity;
mod image;
mod inspect;
mod names;
mod pool;

pub use evidence::{evidence_line, EVIDENCE_PREFIX};
pub use identity::Identity;
pub use image::normalize_runtime_image_id;
pub use inspect::{env_count, mounts_named, named};
pub use names::{is_dns_label, is_lower_hex};
pub use pool::PoolPlacement;
