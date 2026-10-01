/// Resolve the courier proxy URL from `$AXIOM_COURIER_URL`. Returns `None`
/// when unset or blank (mirrors `resolve_github_token()`'s blank-counts-as-
/// unset convention). When `Some`, `issue::{search,view,create,comment}`
/// route through courier's `/v1/issues/...` endpoints instead of calling
/// `api.github.com` directly.
#[cfg(feature = "online")]
pub(crate) fn resolve_courier_url() -> Option<String> {
    resolve_courier_url_from(|var| std::env::var(var).ok())
}

/// Pure resolution (env lookup injected for testing): trim `$AXIOM_COURIER_URL`,
/// `None` when unset or blank.
#[cfg(feature = "online")]
fn resolve_courier_url_from(env: impl Fn(&str) -> Option<String>) -> Option<String> {
    env("AXIOM_COURIER_URL").and_then(|v| {
        let v = v.trim().to_string();
        (!v.is_empty()).then_some(v)
    })
}

/// Resolve the courier proxy bearer token from `$AXIOM_COURIER_TOKEN`.
/// Returns `None` when unset or blank. Sent as `Authorization: Bearer
/// <token>` to courier -- this is the courier-issued client credential, not
/// a personal GitHub token.
#[cfg(feature = "online")]
pub(crate) fn resolve_courier_token() -> Option<String> {
    resolve_courier_token_from(|var| std::env::var(var).ok())
}

/// Pure resolution (env lookup injected for testing): trim `$AXIOM_COURIER_TOKEN`,
/// `None` when unset or blank.
#[cfg(feature = "online")]
fn resolve_courier_token_from(env: impl Fn(&str) -> Option<String>) -> Option<String> {
    env("AXIOM_COURIER_TOKEN").and_then(|v| {
        let v = v.trim().to_string();
        (!v.is_empty()).then_some(v)
    })
}

#[cfg(all(test, feature = "online"))]
mod courier_tests;
