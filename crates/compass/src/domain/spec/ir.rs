//! Intermediate representation for spec-to-code generation
//!
//! SpecIR serves as the common representation between spec parsers
//! (JSON Schema, OpenAPI, AsyncAPI, Mermaid) and code generators.

mod control_flow;
mod data_model;
mod event_api;
mod rest_api;
mod state_machine;

pub use control_flow::{ControlFlowSpec, FlowEdge, FlowNode, FlowNodeType};
pub use data_model::{
    DataModelSpec, EnumDef, EnumValue, EnumVariant, FieldConstraints, FieldDef, ForeignKey,
    ForeignKeyAction, MethodDef, ModelDef, ParamDef, RelationType, Relationship, StringFormat,
    TypeParam, Visibility,
};
pub use event_api::{ChannelDef, EventApiSpec, OperationDef};
pub use rest_api::{
    EndpointDef, HttpMethod, QueryParam, RequestBody, ResponseDef, RestApiSpec, SecurityScheme,
    SecuritySchemeType, ServerDef,
};
pub use state_machine::{StateDef, StateMachineSpec, TransitionDef};

/// Main SpecIR enum representing different spec types
#[derive(Debug, Clone)]
pub enum SpecIR {
    /// Data model specification (from JSON Schema, Mermaid classDiagram/ERD)
    DataModel(DataModelSpec),
    /// REST API specification (from OpenAPI)
    RestApi(RestApiSpec),
    /// Event-driven API specification (from AsyncAPI)
    EventApi(EventApiSpec),
    /// State machine specification (from Mermaid stateDiagram)
    StateMachine(StateMachineSpec),
    /// Control flow specification (from Mermaid flowchart)
    ControlFlow(ControlFlowSpec),
}

#[cfg(test)]
mod tests;
