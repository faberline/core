//! Entry surfaces: the filesystem-writing CLI entry and the LLM topic
//! provider.

mod cli;
mod llm;

pub use cli::run;
pub use llm::{topic, TOPIC};
