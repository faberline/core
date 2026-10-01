use anyhow::Result;

use crate::domain::connect::{cr_tokens_secret, select_token, Role, TOKEN_REGISTRY_SECRET_KEY};
use crate::infrastructure::connect::{bearer_secrets, kubectl_get_json, secret_data_bytes};

/// Resolve a CR's `spec.tokensSecret` (`None` when unset). `resource_kind`
/// is the CRD's kubectl resource name (e.g. `"lumen"`) — the CR-kind lookup
/// convention stays each adopter's own.
pub fn resolve_cr_tokens_secret(
    context: Option<&str>,
    namespace: &str,
    resource_kind: &str,
    cr: &str,
) -> Result<Option<String>> {
    let cr_json = kubectl_get_json(context, resource_kind, cr, namespace)?;
    Ok(cr_tokens_secret(&cr_json))
}

/// Resolve a usable bearer token without the caller decoding the
/// Secret/JSON by hand. Precedence: `explicit_token` (an already-resolved
/// flag/env value) wins; otherwise, when `namespace`/`secret` are both set,
/// fetch the Secret via kubectl, decode its `token-registry.json` key (the
/// same schema `lumen llm --topic auth` documents), and pick a token whose
/// role covers `role` for `collection` (or `*`). Returns `None` when no
/// token can be resolved (e.g. auth-disabled deployments).
pub fn resolve_token(
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
    let secret_json = kubectl_get_json(context, "secret", secret, namespace)?;
    let bytes = secret_data_bytes(&secret_json, TOKEN_REGISTRY_SECRET_KEY)?;
    let registry = bearer_secrets(&bytes)?;
    Ok(select_token(&registry, role, collection))
}
