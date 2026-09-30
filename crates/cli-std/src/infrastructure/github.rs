#[cfg(feature = "online")]
use super::http::HttpClient;
#[cfg(feature = "online")]
use crate::domain::release::ReleaseSource;
#[cfg(feature = "online")]
use crate::domain::remote::RemoteError;

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

/// `GET` against the GitHub API, with the token from
/// [`resolve_github_token`] when there is one.
#[cfg(feature = "online")]
pub(in crate::infrastructure) async fn github_get(
    client: &reqwest::Client,
    url: &str,
) -> Result<reqwest::Response, RemoteError> {
    let mut req = client
        .get(url)
        .header("Accept", "application/vnd.github+json");
    if let Some(token) = resolve_github_token() {
        req = req.bearer_auth(token);
    }
    req.send()
        .await
        .map_err(|e| RemoteError::Send {
            method: "GET",
            target: url.to_string(),
            source: Box::new(e),
        })?
        .error_for_status()
        .map_err(|e| RemoteError::Status {
            service: "GitHub API",
            url: url.to_string(),
            source: Box::new(e),
        })
}

/// Parse a JSON response body; `what` names the body in the error.
#[cfg(feature = "online")]
pub(in crate::infrastructure) async fn parse_json(
    resp: reqwest::Response,
    what: &'static str,
) -> Result<serde_json::Value, RemoteError> {
    resp.json().await.map_err(|e| RemoteError::Parse {
        what,
        source: Box::new(e),
    })
}

/// `GET` a download URL, failing on an error status.
#[cfg(feature = "online")]
async fn download(client: &reqwest::Client, url: &str) -> Result<reqwest::Response, RemoteError> {
    let failed = |e: reqwest::Error| RemoteError::Download {
        url: url.to_string(),
        source: Box::new(e),
    };
    client
        .get(url)
        .send()
        .await
        .map_err(failed)?
        .error_for_status()
        .map_err(failed)
}

#[cfg(feature = "online")]
impl ReleaseSource for HttpClient {
    async fn releases(&self, repo: &str) -> Result<serde_json::Value, RemoteError> {
        let url = format!("https://api.github.com/repos/{repo}/releases?per_page=100");
        parse_json(github_get(&self.client, &url).await?, "releases").await
    }

    async fn release(&self, repo: &str, tag: &str) -> Result<serde_json::Value, RemoteError> {
        let url = format!("https://api.github.com/repos/{repo}/releases/tags/{tag}");
        parse_json(github_get(&self.client, &url).await?, "release").await
    }

    async fn download_bytes(&self, url: &str) -> Result<Vec<u8>, RemoteError> {
        let resp = download(&self.client, url).await?;
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| RemoteError::Body(Box::new(e)))?;
        Ok(bytes.to_vec())
    }

    async fn download_text(&self, url: &str) -> Result<String, RemoteError> {
        let resp = download(&self.client, url).await?;
        resp.text()
            .await
            .map_err(|e| RemoteError::Body(Box::new(e)))
    }
}

#[cfg(all(test, feature = "online"))]
mod token_tests;
