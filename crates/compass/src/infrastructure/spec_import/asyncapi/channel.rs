//! Channels, operations and messages.

use super::{AsyncApiError, AsyncApiParser};
use crate::domain::spec::ir::{ChannelDef, OperationDef};
use crate::type_inference::Type;
use serde_json::Value;

impl AsyncApiParser {
    /// Parse channels
    pub(super) fn parse_channels(&self, value: &Value) -> Result<Vec<ChannelDef>, AsyncApiError> {
        let channels_obj = value
            .get("channels")
            .and_then(|c| c.as_object())
            .ok_or_else(|| AsyncApiError::MissingField("channels".into()))?;

        let mut channels = Vec::new();

        for (name, channel_value) in channels_obj {
            let description = channel_value
                .get("description")
                .and_then(|v| v.as_str())
                .map(String::from);

            let subscribe = self.parse_operation(channel_value, "subscribe")?;
            let publish = self.parse_operation(channel_value, "publish")?;

            channels.push(ChannelDef {
                name: name.clone(),
                description,
                subscribe,
                publish,
            });
        }

        Ok(channels)
    }

    /// Parse a channel operation (subscribe/publish)
    fn parse_operation(
        &self,
        channel: &Value,
        op_type: &str,
    ) -> Result<Option<OperationDef>, AsyncApiError> {
        let op = match channel.get(op_type) {
            Some(o) => o,
            None => return Ok(None),
        };

        let operation_id = op
            .get("operationId")
            .and_then(|v| v.as_str())
            .map(String::from);
        let summary = op.get("summary").and_then(|v| v.as_str()).map(String::from);
        let description = op
            .get("description")
            .and_then(|v| v.as_str())
            .map(String::from);

        // Parse message
        let message = self.parse_operation_message(op)?;

        Ok(Some(OperationDef {
            operation_id,
            summary,
            description,
            message,
        }))
    }

    /// Parse operation message schema
    fn parse_operation_message(&self, op: &Value) -> Result<Type, AsyncApiError> {
        let message = match op.get("message") {
            Some(m) => m,
            None => return Ok(Type::Any),
        };

        // Handle $ref
        if let Some(ref_str) = message.get("$ref").and_then(|r| r.as_str()) {
            return self.resolve_message_ref(ref_str);
        }

        // Handle oneOf (multiple message types)
        if let Some(one_of) = message.get("oneOf").and_then(|o| o.as_array()) {
            let types: Vec<Type> = one_of
                .iter()
                .filter_map(|m| self.parse_message_schema(m).ok())
                .collect();
            return Ok(if types.len() == 1 {
                types.into_iter().next().unwrap()
            } else {
                Type::Union(types)
            });
        }

        self.parse_message_schema(message)
    }

    /// Parse a single message schema
    fn parse_message_schema(&self, message: &Value) -> Result<Type, AsyncApiError> {
        // Handle $ref
        if let Some(ref_str) = message.get("$ref").and_then(|r| r.as_str()) {
            return self.resolve_message_ref(ref_str);
        }

        // Get payload
        let payload = message.get("payload").unwrap_or(message);

        self.schema_to_type(payload)
    }

    /// Resolve a message reference
    fn resolve_message_ref(&self, ref_str: &str) -> Result<Type, AsyncApiError> {
        // Extract name from reference
        let name = ref_str
            .rsplit('/')
            .next()
            .ok_or_else(|| AsyncApiError::InvalidRef(ref_str.to_string()))?;

        // Check if we have it in components
        if let Some(component) = self.components.get(ref_str) {
            // If it's a message, get its payload
            if let Some(payload) = component.get("payload") {
                return self.schema_to_type(payload);
            }
            // Otherwise treat as schema
            return self.schema_to_type(component);
        }

        // Return as Instance type (will be defined elsewhere)
        Ok(Type::Instance {
            name: name.to_string(),
            module: None,
            type_args: vec![],
        })
    }
}
