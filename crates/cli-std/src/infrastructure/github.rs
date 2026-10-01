/// Resolve a GitHub token the way `gh` itself does, in order: `$GH_TOKEN`,
/// then `$GITHUB_TOKEN`, then the `gh` CLI credential store (`gh auth token`,
/// which reads the OS keyring / `hosts.yml`). Returns `None` when no credential
/// is available. This makes the standard CLI ops "just work" for anyone already
/// authenticated via `gh`, which does not export a `GITHUB_TOKEN` env var.
#[cfg(feature = "online")]
pub(crate) fn resolve_github_token() -> Option<String> {
    resolve_github_token_from(|var| std::env::var(var).ok(), gh_auth_token)
}

/// Pure resolution order (env lookup + `gh` fallback injected for testing):
/// first non-empty of `$GH_TOKEN`, `$GITHUB_TOKEN`, then `gh()`.
#[cfg(feature = "online")]
fn resolve_github_token_from(
    env: impl Fn(&str) -> Option<String>,
    gh: impl Fn() -> Option<String>,
) -> Option<String> {
    for var in ["GH_TOKEN", "GITHUB_TOKEN"] {
        if let Some(token) = env(var) {
            let token = token.trim().to_string();
            if !token.is_empty() {
                return Some(token);
            }
        }
    }
    gh()
}

/// Shell out to `gh auth token` — the de-facto GitHub auth on developer
/// machines (token in the OS keyring). Returns `None` if `gh` is missing,
/// unauthenticated, or prints nothing.
#[cfg(feature = "online")]
fn gh_auth_token() -> Option<String> {
    let output = std::process::Command::new("gh")
        .args(["auth", "token"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!token.is_empty()).then_some(token)
}

#[cfg(feature = "online")]
pub(crate) async fn github_get(
    client: &reqwest::Client,
    url: &str,
) -> anyhow::Result<reqwest::Response> {
    use anyhow::Context;
    let mut req = client
        .get(url)
        .header("Accept", "application/vnd.github+json");
    if let Some(token) = resolve_github_token() {
        req = req.bearer_auth(token);
    }
    req.send()
        .await
        .with_context(|| format!("GET {url}"))?
        .error_for_status()
        .with_context(|| format!("GitHub API error for {url}"))
}

#[cfg(feature = "online")]
pub(crate) async fn download_bytes(client: &reqwest::Client, url: &str) -> anyhow::Result<Vec<u8>> {
    use anyhow::Context;
    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("download {url}"))?
        .error_for_status()
        .with_context(|| format!("download {url}"))?;
    Ok(resp.bytes().await.context("read download body")?.to_vec())
}

#[cfg(feature = "online")]
pub(crate) async fn download_text(client: &reqwest::Client, url: &str) -> anyhow::Result<String> {
    use anyhow::Context;
    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("download {url}"))?
        .error_for_status()
        .with_context(|| format!("download {url}"))?;
    resp.text().await.context("read download body")
}

#[cfg(all(test, feature = "online"))]
mod token_tests;
