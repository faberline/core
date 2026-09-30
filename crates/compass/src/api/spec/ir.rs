//! Intermediate representation for spec-to-code generation
//!
//! SpecIR serves as the common representation between spec parsers
//! (JSON Schema, OpenAPI, AsyncAPI, Mermaid) and code generators.

pub use crate::domain::spec::ir::{
    ChannelDef, ControlFlowSpec, DataModelSpec, EndpointDef, EnumDef, EnumValue, EnumVariant,
    EventApiSpec, FieldConstraints, FieldDef, FlowEdge, FlowNode, FlowNodeType, ForeignKey,
    ForeignKeyAction, HttpMethod, MethodDef, ModelDef, OperationDef, ParamDef, QueryParam,
    RelationType, Relationship, RequestBody, ResponseDef, RestApiSpec, SecurityScheme,
    SecuritySchemeType, ServerDef, SpecIR, StateDef, StateMachineSpec, StringFormat, TransitionDef,
    TypeParam, Visibility,
};
