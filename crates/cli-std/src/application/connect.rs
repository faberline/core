use anyhow::Result;

use crate::domain::connect::kubectl::Kubectl;
use crate::domain::connect::{
    bearer_secrets, cr_tokens_secret, select_token, Role, TOKEN_REGISTRY_SECRET_KEY,
};

/// Resolve a CR's `spec.tokensSecret` (`None` when unset), reading the CR
/// through `kubectl`.
pub(crate) fn resolve_cr_tokens_secret(
    kubectl: &impl Kubectl,
    context: Option<&str>,
    namespace: &str,
    resource_kind: &str,
    cr: &str,
) -> Result<Option<String>> {
    let cr_json = kubectl.get_json(context, resource_kind, cr, namespace)?;
    Ok(cr_tokens_secret(&cr_json))
}

/// Resolve a usable bearer token, reading the token-registry Secret through
/// `kubectl`. `explicit_token` wins; without both `namespace` and `secret`
/// there is nothing to read and the answer is `None`.
pub(crate) fn resolve_token(
    kubectl: &impl Kubectl,
    explicit_token: Option<&str>,
    context: Option<&str>,
    namespace: Option<&str>,
    secret: Option<&str>,
    role: Role,
    collection: Option<&str>,
) -> Result<Option<String>> {
    if let Some(token) = explicit_token {
        return Ok(Some(token.to_string()));
    }
    let (Some(namespace), Some(secret)) = (namespace, secret) else {
        return Ok(None);
    };
    let bytes = kubectl.secret_data(context, namespace, secret, TOKEN_REGISTRY_SECRET_KEY)?;
    let registry = bearer_secrets(&bytes)?;
    Ok(select_token(&registry, role, collection))
}
