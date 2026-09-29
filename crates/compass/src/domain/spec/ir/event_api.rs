//! Event API IR (from AsyncAPI).

use super::data_model::DataModelSpec;
use crate::type_inference::Type;

// ============================================================================
// Event API Specification (from AsyncAPI)
// ============================================================================

/// Event-driven API specification
#[derive(Debug, Clone, Default)]
pub struct EventApiSpec {
    /// API title
    pub title: String,
    /// API version
    pub version: String,
    /// Description
    pub description: Option<String>,
    /// Channels (topics/queues)
    pub channels: Vec<ChannelDef>,
    /// Message schemas
    pub messages: DataModelSpec,
}

/// Channel (topic/queue) definition
#[derive(Debug, Clone)]
pub struct ChannelDef {
    /// Channel name/path
    pub name: String,
    /// Description
    pub description: Option<String>,
    /// Subscribe operation (consuming messages)
    pub subscribe: Option<OperationDef>,
    /// Publish operation (producing messages)
    pub publish: Option<OperationDef>,
}

/// Channel operation definition
#[derive(Debug, Clone)]
pub struct OperationDef {
    pub operation_id: Option<String>,
    pub summary: Option<String>,
    pub description: Option<String>,
    /// Message schema
    pub message: Type,
}
