use anyhow::{bail, Context, Result};

/// Fetch exact snapshot bytes from the standard `GET /admin/backup` endpoint.
///
/// Services retain the domain-specific snapshot encoding and restore policy.
/// This helper owns the common Bearer request, non-success diagnostic, and
/// byte-preserving response read used by service backup CLIs and CronJobs.
pub async fn fetch_admin_snapshot(base_url: &str, token: Option<&str>) -> Result<Vec<u8>> {
    let url = format!("{}/admin/backup", base_url.trim_end_matches('/'));
    let client = reqwest::Client::new();
    let mut request = client.get(&url);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request.send().await.with_context(|| format!("GET {url}"))?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        bail!("GET {url} returned {status}: {body}");
    }
    let payload = response
        .bytes()
        .await
        .with_context(|| format!("read response body from {url}"))?;
    Ok(payload.to_vec())
}
