use super::*;
use crate::domain::modules::builtin_modules::{create_builtins_module, create_typing_module};
use crate::domain::modules::import::ImportedName;

mod basics;
mod index;
mod resolution;

#[test]
fn no_fixed_temp_dir_names_in_resolver_tests() {
    for (name, src) in [
        ("tests/index.rs", include_str!("tests/index.rs")),
        ("tests/resolution.rs", include_str!("tests/resolution.rs")),
    ] {
        assert!(
            !src.contains("env::temp_dir().join("),
            "{name} uses a fixed directory under the shared temp dir; use tempfile::tempdir()"
        );
    }
}
