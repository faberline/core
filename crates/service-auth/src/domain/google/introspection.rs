//! The access-token introspection port and the answer it returns.

use std::future::Future;
use std::pin::Pin;

use serde::{Deserialize, Deserializer};

/// What Google's introspection endpoint reports about an access token.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct IntrospectedToken {
    #[serde(default)]
    pub email: Option<String>,
    /// Google sends this as the **string** `"true"`, not a JSON boolean.
    #[serde(default, deserialize_with = "lenient_bool")]
    pub email_verified: bool,
    /// Seconds of remaining life. Also sent as a string.
    #[serde(default, deserialize_with = "lenient_u64")]
    pub expires_in: u64,
}

/// Resolves an opaque access token by asking Google.
///
/// The method is written in the shape `#[async_trait]` expands to, so an
/// implementation may still be written as `#[async_trait] impl
/// AccessTokenIntrospection for …` with an `async fn introspect`.
pub trait AccessTokenIntrospection: Send + Sync {
    /// `Ok(Some(_))` — Google answered and the token is live.
    /// `Ok(None)` — Google answered and the token is not valid.
    /// `Err(_)` — Google did not answer. These three are different outcomes
    /// and the type keeps them different.
    fn introspect<'life0, 'life1, 'async_trait>(
        &'life0 self,
        token: &'life1 str,
    ) -> Pin<
        Box<dyn Future<Output = Result<Option<IntrospectedToken>, String>> + Send + 'async_trait>,
    >
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait;
}

pub(crate) fn lenient_bool<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum BoolOrString {
        Bool(bool),
        Str(String),
    }
    Ok(match BoolOrString::deserialize(deserializer)? {
        BoolOrString::Bool(value) => value,
        BoolOrString::Str(value) => matches!(value.trim(), "true" | "True" | "TRUE" | "1"),
    })
}

fn lenient_u64<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum NumOrString {
        Num(u64),
        Str(String),
    }
    Ok(match NumOrString::deserialize(deserializer)? {
        NumOrString::Num(value) => value,
        NumOrString::Str(value) => value.trim().parse().unwrap_or(0),
    })
}
