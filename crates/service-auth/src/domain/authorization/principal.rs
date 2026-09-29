use super::role::{Role, TokenClaims};

const WILDCARD_RESOURCE: &str = "*";

/// The resolved principal for a request: `Open` (auth disabled, no bearer
/// presented) or an authenticated token's claims.
#[derive(Debug, Clone)]
pub enum RoleMapPrincipal {
    /// Auth is disabled and no bearer was presented. Treated as full admin
    /// in development; production should run with auth required.
    Open,
    Token(TokenClaims),
}

impl RoleMapPrincipal {
    /// The per-resource authorization check a handler runs after
    /// authentication: `Open` always passes (dev mode); a token must carry a
    /// claim on `resource` (or the wildcard `*`) that covers `needed`.
    pub fn ensure(&self, resource: &str, needed: Role) -> Result<(), RoleMapDenied> {
        match self {
            RoleMapPrincipal::Open => Ok(()),
            RoleMapPrincipal::Token(claims) => {
                let have = claims
                    .roles
                    .get(resource)
                    .or_else(|| claims.roles.get(WILDCARD_RESOURCE));
                match have {
                    Some(r) if r.covers(needed) => Ok(()),
                    _ => Err(RoleMapDenied {
                        subject: claims.subject.clone(),
                        needed,
                        resource: resource.to_string(),
                    }),
                }
            }
        }
    }

    pub fn subject(&self) -> Option<&str> {
        match self {
            RoleMapPrincipal::Open => None,
            RoleMapPrincipal::Token(c) => Some(c.subject.as_str()),
        }
    }
}

/// Why [`RoleMapPrincipal::ensure`] rejected a request: a valid principal
/// lacking `needed` on `resource`. Structured (not pre-rendered) so a
/// service can build its own message / audit log the way lumen does.
#[derive(Debug, Clone)]
pub struct RoleMapDenied {
    pub subject: String,
    pub needed: Role,
    pub resource: String,
}
