use std::collections::HashMap;

use axum::http::HeaderMap;

use crate::application::http::{bearer_token, AuthError, Verifier};
use crate::domain::authorization::{RoleMapPrincipal, TokenClaims};

/// A [`Verifier`] over a static, config-driven token→claims registry — the
/// archetype's `<SVC>_AUTH=off|required` shape.
#[derive(Debug, Clone)]
pub struct StaticRoleMapVerifier {
    required: bool,
    tokens: HashMap<String, TokenClaims>,
}

impl StaticRoleMapVerifier {
    pub fn new(required: bool, tokens: HashMap<String, TokenClaims>) -> Self {
        Self { required, tokens }
    }

    /// Open/dev verifier: auth disabled, no tokens.
    pub fn open() -> Self {
        Self::new(false, HashMap::new())
    }

    fn lookup(&self, token: &str) -> Option<&TokenClaims> {
        self.tokens.get(token)
    }
}

impl Verifier for StaticRoleMapVerifier {
    type Principal = RoleMapPrincipal;

    fn authenticate(&self, headers: &HeaderMap) -> Result<RoleMapPrincipal, AuthError> {
        match (self.required, bearer_token(headers)) {
            (false, None) => Ok(RoleMapPrincipal::Open),
            (_, Some(t)) => self
                .lookup(t)
                .cloned()
                .map(RoleMapPrincipal::Token)
                .ok_or(AuthError::Unauthenticated),
            (true, None) => Err(AuthError::Unauthenticated),
        }
    }

    fn required(&self) -> bool {
        self.required
    }
}
