//! File-backed adapters: the JSONL quarantine and JSON checkpoints.

mod checkpoint;
mod quarantine;

pub use checkpoint::{load_json_checkpoint, save_json_checkpoint};
pub use quarantine::{append_jsonl, JsonlQuarantine};
