use std::collections::HashMap;

use axum::http::HeaderMap;

use crate::{
    load_registry, load_registry_file, load_registry_files, Registry, RegistrySource, Role,
    RoleMapPrincipal, StaticRoleMapVerifier, TokenClaims,
};
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
