//! Generation use cases: check an explicit target profile against the
//! options, then dispatch the spec to the emitter for the chosen language.

mod generate;

pub use generate::{
    generate, generate_for_target, generate_for_target_with_file_bearer_auth,
    generate_with_file_bearer_auth,
};
