use super::*;
use crate::domain::modules::builtin_modules::{create_builtins_module, create_typing_module};
use crate::domain::modules::import::ImportedName;
use std::env;

mod basics;
mod index;
mod resolution;
