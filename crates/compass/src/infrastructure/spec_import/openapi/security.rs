//! Security scheme parsing.

use super::OpenApiParser;
use crate::domain::spec::ir::{SecurityScheme, SecuritySchemeType};
use serde_json::Value;

impl OpenApiParser {
    /// Parse security scheme
    pub(super) fn parse_security_scheme(
        &self,
        name: &str,
        scheme: &Value,
    ) -> Option<SecurityScheme> {
        let scheme_type = scheme.get("type")?.as_str()?;

        let parsed_type = match scheme_type {
            "apiKey" => {
                let key_name = scheme.get("name")?.as_str()?.to_string();
                let location = scheme.get("in")?.as_str()?;
                SecuritySchemeType::ApiKey {
                    in_header: location == "header",
                    key_name,
                }
            }
            "http" => {
                let auth_scheme = scheme.get("scheme")?.as_str()?.to_string();
                let bearer_format = scheme
                    .get("bearerFormat")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                SecuritySchemeType::Http {
                    scheme: auth_scheme,
                    bearer_format,
                }
            }
            "oauth2" => {
                let flows: Vec<String> = scheme
                    .get("flows")
                    .and_then(|v| v.as_object())
                    .map(|obj| obj.keys().cloned().collect())
                    .unwrap_or_default();
                SecuritySchemeType::OAuth2 { flows }
            }
            "openIdConnect" => {
                let url = scheme.get("openIdConnectUrl")?.as_str()?.to_string();
                SecuritySchemeType::OpenIdConnect { url }
            }
            _ => return None,
        };

        Some(SecurityScheme {
            name: name.to_string(),
            scheme_type: parsed_type,
            description: scheme
                .get("description")
                .and_then(|v| v.as_str())
                .map(String::from),
        })
    }
}
