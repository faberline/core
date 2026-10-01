use serde::Serialize;

pub const PROTOCOL: &str = "cclab.llm.v2";

/// JSON Schema for the additive `cclab.llm.v2` JSON envelopes.
///
/// It describes the public wire shape, so clients can validate an outline or
/// detail runbook without linking this Rust crate.
pub fn json_schema() -> serde_json::Value {
    serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": PROTOCOL,
        "oneOf": [
            {
                "title": "Task manifest",
                "type": "object",
                "required": ["topic", "markdown", "protocol", "tasks"],
                "properties": {
                    "topic": { "const": "outline" },
                    "markdown": { "type": "string" },
                    "protocol": { "const": PROTOCOL },
                    "tasks": { "type": "array", "items": { "$ref": "#/$defs/task" } }
                }
            },
            {
                "title": "Typed runbook",
                "type": "object",
                "required": ["topic", "markdown", "protocol", "task", "runbook"],
                "properties": {
                    "topic": { "type": "string", "minLength": 1 },
                    "markdown": { "type": "string" },
                    "protocol": { "const": PROTOCOL },
                    "task": { "$ref": "#/$defs/task" },
                    "runbook": { "$ref": "#/$defs/runbook" },
                    "providers": {
                        "type": "array",
                        "items": { "$ref": "#/$defs/provider" }
                    }
                }
            }
        ],
        "$defs": {
            "provider": {
                "type": "object",
                "required": ["id", "summary", "markdown"],
                "properties": {
                    "id": { "type": "string", "minLength": 1 },
                    "summary": { "type": "string" },
                    "markdown": { "type": "string", "minLength": 1 }
                }
            },
            "task": {
                "type": "object",
                "required": ["id", "use_when", "requires", "reads", "produces", "risk", "topic", "contract_refs"],
                "properties": {
                    "id": { "type": "string", "minLength": 1 },
                    "use_when": { "type": "string" },
                    "requires": { "type": "array", "items": { "type": "string" } },
                    "reads": { "type": "array", "items": { "type": "string" } },
                    "produces": { "type": "array", "items": { "type": "string" } },
                    "risk": { "enum": ["inspect", "local_write", "remote_write"] },
                    "topic": { "type": "string", "minLength": 1 },
                    "contract_refs": { "type": "array", "items": { "type": "string" } }
                }
            },
            "input": {
                "type": "object",
                "required": ["name", "type", "description", "required"],
                "properties": {
                    "name": { "type": "string", "minLength": 1 },
                    "type": { "type": "string", "minLength": 1 },
                    "description": { "type": "string" },
                    "required": { "type": "boolean" }
                }
            },
            "runbook": {
                "type": "object",
                "required": ["purpose", "preconditions", "inputs", "constraints", "steps", "verification", "references"],
                "properties": {
                    "purpose": { "type": "string" },
                    "preconditions": { "type": "array", "items": { "type": "string" } },
                    "inputs": { "type": "array", "items": { "$ref": "#/$defs/input" } },
                    "constraints": { "type": "array", "items": { "type": "string" } },
                    "steps": { "type": "array", "items": { "type": "object" } },
                    "verification": { "type": "array", "items": { "type": "string" } },
                    "references": { "type": "array", "items": { "type": "string" } }
                }
            }
        }
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    Inspect,
    LocalWrite,
    RemoteWrite,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Task {
    pub id: String,
    pub use_when: String,
    pub requires: Vec<String>,
    pub reads: Vec<String>,
    pub produces: Vec<String>,
    pub risk: Risk,
    pub topic: String,
    pub contract_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Input {
    pub name: String,
    #[serde(rename = "type")]
    pub value_type: String,
    pub description: String,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Step {
    pub id: String,
    pub instruction: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_template: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<Input>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Runbook {
    pub purpose: String,
    pub preconditions: Vec<String>,
    pub inputs: Vec<Input>,
    pub constraints: Vec<String>,
    pub steps: Vec<Step>,
    pub verification: Vec<String>,
    pub references: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Topic {
    pub task: Task,
    pub runbook: Runbook,
}

/// Content owned by a shared library and composed into one app task topic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderContent {
    pub id: String,
    pub summary: String,
    pub markdown: String,
}
