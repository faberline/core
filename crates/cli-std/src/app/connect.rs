//! `connect`'s token resolution, wired to the `kubectl` binary.

use anyhow::Result;

use crate::application::connect as use_case;
use crate::domain::connect::Role;
use crate::infrastructure::connect::KubectlCli;

/// Resolve a CR's `spec.tokensSecret` (`None` when unset). `resource_kind`
/// is the CRD's kubectl resource name (e.g. `"lumen"`) — the CR-kind lookup
/// convention stays each adopter's own.
pub fn resolve_cr_tokens_secret(
    context: Option<&str>,
    namespace: &str,
    resource_kind: &str,
    cr: &str,
) -> Result<Option<String>> {
    use_case::resolve_cr_tokens_secret(&KubectlCli, context, namespace, resource_kind, cr)
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
    use_case::resolve_token(
        &KubectlCli,
        explicit_token,
        context,
        namespace,
        secret,
        role,
        collection,
    )
}
