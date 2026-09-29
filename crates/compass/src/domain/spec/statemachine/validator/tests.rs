use super::*;
use serde_json::json;

mod compound;
mod nesting;
mod references;
mod structure;

fn parse_machine(json: serde_json::Value) -> StateMachineDef {
    serde_json::from_value(json).unwrap()
}
