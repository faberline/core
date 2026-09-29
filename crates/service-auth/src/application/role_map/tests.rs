use std::collections::HashMap;

use axum::http::HeaderMap;

use crate::role_map::*;
use crate::{AuthError, Verifier};

fn token(roles: &[(&str, Role)]) -> TokenClaims {
    TokenClaims {
        subject: "tester".into(),
        roles: roles
            .iter()
            .map(|(r, role)| (r.to_string(), *role))
            .collect(),
    }
}

mod loading;
mod namespaces;
mod principals;
mod sources;
