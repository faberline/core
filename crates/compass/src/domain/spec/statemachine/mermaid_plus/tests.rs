use super::super::validator::StateMachineValidator;
use super::*;
use serde_json::json;

mod basic;
mod nested;
mod parallel;
mod transitions;

fn parse_machine(json: serde_json::Value) -> StateMachineDef {
    serde_json::from_value(json).unwrap()
}
