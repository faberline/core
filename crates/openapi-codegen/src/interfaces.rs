//! Entry surfaces: the LLM topic provider. The filesystem-writing CLI entry
//! (`run`) is wiring and lives in the composition root, `src/app/`.

mod llm;

pub use llm::{topic, TOPIC};
