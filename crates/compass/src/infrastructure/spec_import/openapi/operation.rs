//! Path items, operations, parameters, request bodies and responses.

use super::{OpenApiError, OpenApiParser};
use crate::domain::spec::ir::{
    EndpointDef, HttpMethod, ParamDef, QueryParam, RequestBody, ResponseDef, RestApiSpec,
};
use crate::type_inference::Type;
use serde_json::Value;

impl OpenApiParser {
    /// Parse a path item (all operations for a path)
    pub(super) fn parse_path_item(
        &self,
        path: &str,
        item: &Value,
        spec: &mut RestApiSpec,
    ) -> Result<(), OpenApiError> {
        // Shared parameters for all operations in this path
        let shared_params = item
            .get("parameters")
            .and_then(|v| v.as_array())
            .map(|arr| self.parse_parameters(arr))
            .unwrap_or_default();

        let methods = [
            ("get", HttpMethod::Get),
            ("post", HttpMethod::Post),
            ("put", HttpMethod::Put),
            ("patch", HttpMethod::Patch),
            ("delete", HttpMethod::Delete),
            ("head", HttpMethod::Head),
            ("options", HttpMethod::Options),
        ];

        for (method_str, method) in methods {
            if let Some(op) = item.get(method_str) {
                let endpoint = self.parse_operation(path, method, op, &shared_params)?;
                spec.endpoints.push(endpoint);
            }
        }

        Ok(())
    }

    /// Parse an operation
    fn parse_operation(
        &self,
        path: &str,
        method: HttpMethod,
        op: &Value,
        shared_params: &[ParsedParam],
    ) -> Result<EndpointDef, OpenApiError> {
        let operation_id = op
            .get("operationId")
            .and_then(|v| v.as_str())
            .map(String::from);
        let summary = op.get("summary").and_then(|v| v.as_str()).map(String::from);
        let description = op
            .get("description")
            .and_then(|v| v.as_str())
            .map(String::from);

        let tags: Vec<String> = op
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        // Parse operation-specific parameters
        let op_params = op
            .get("parameters")
            .and_then(|v| v.as_array())
            .map(|arr| self.parse_parameters(arr))
            .unwrap_or_default();

        // Combine shared and operation params
        let all_params: Vec<_> = shared_params.iter().chain(op_params.iter()).collect();

        // Separate path and query params
        let mut path_params = Vec::new();
        let mut query_params = Vec::new();

        for param in all_params {
            match param.location.as_str() {
                "path" => path_params.push(ParamDef {
                    name: param.name.clone(),
                    ty: param.ty.clone(),
                    default: param.default.clone(),
                }),
                "query" => query_params.push(QueryParam {
                    name: param.name.clone(),
                    ty: param.ty.clone(),
                    required: param.required,
                    description: param.description.clone(),
                    default: param.default.clone(),
                }),
                _ => {}
            }
        }

        // Parse request body
        let request_body = op
            .get("requestBody")
            .map(|rb| self.parse_request_body(rb))
            .transpose()?;

        // Parse responses
        let responses = self.parse_responses(op.get("responses"));

        // Security
        let security: Vec<String> = op
            .get("security")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_object())
                    .flat_map(|obj| obj.keys().cloned())
                    .collect()
            })
            .unwrap_or_default();

        let deprecated = op
            .get("deprecated")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        Ok(EndpointDef {
            path: path.to_string(),
            method,
            operation_id,
            summary,
            description,
            tags,
            path_params,
            query_params,
            request_body,
            responses,
            security,
            deprecated,
        })
    }

    /// Parse parameters
    fn parse_parameters(&self, params: &[Value]) -> Vec<ParsedParam> {
        params
            .iter()
            .filter_map(|p| self.parse_parameter(p))
            .collect()
    }

    /// Parse a single parameter
    fn parse_parameter(&self, param: &Value) -> Option<ParsedParam> {
        let name = param.get("name")?.as_str()?.to_string();
        let location = param.get("in")?.as_str()?.to_string();
        let required = param
            .get("required")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let description = param
            .get("description")
            .and_then(|v| v.as_str())
            .map(String::from);

        let schema = param.get("schema").unwrap_or(&Value::Null);
        let ty = self.schema_to_type(schema);

        let default = param.pointer("/schema/default").map(|v| v.to_string());

        Some(ParsedParam {
            name,
            location,
            ty,
            required,
            description,
            default,
        })
    }

    /// Parse request body
    fn parse_request_body(&self, rb: &Value) -> Result<RequestBody, OpenApiError> {
        let required = rb
            .get("required")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let description = rb
            .get("description")
            .and_then(|v| v.as_str())
            .map(String::from);

        // Get the first content type (usually application/json)
        let content = rb.get("content").and_then(|v| v.as_object());

        let (content_type, schema) = if let Some(c) = content {
            if let Some((ct, media)) = c.iter().next() {
                let schema = media.get("schema").unwrap_or(&Value::Null);
                (ct.clone(), self.schema_to_type(schema))
            } else {
                ("application/json".to_string(), Type::Any)
            }
        } else {
            ("application/json".to_string(), Type::Any)
        };

        Ok(RequestBody {
            content_type,
            schema,
            required,
            description,
        })
    }

    /// Parse responses
    fn parse_responses(&self, responses: Option<&Value>) -> Vec<ResponseDef> {
        let Some(responses) = responses.and_then(|v| v.as_object()) else {
            return vec![];
        };

        responses
            .iter()
            .filter_map(|(status, resp)| {
                let status_code: u16 = status.parse().ok()?;
                let description = resp
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Response")
                    .to_string();

                let content = resp.get("content").and_then(|v| v.as_object());
                let (content_type, schema) = if let Some(c) = content {
                    if let Some((ct, media)) = c.iter().next() {
                        let schema = media.get("schema").map(|s| self.schema_to_type(s));
                        (Some(ct.clone()), schema)
                    } else {
                        (None, None)
                    }
                } else {
                    (None, None)
                };

                Some(ResponseDef {
                    status_code,
                    description,
                    schema,
                    content_type,
                })
            })
            .collect()
    }
}

/// Parsed parameter (intermediate)
#[derive(Debug)]
struct ParsedParam {
    name: String,
    location: String,
    ty: Type,
    required: bool,
    description: Option<String>,
    default: Option<String>,
}
