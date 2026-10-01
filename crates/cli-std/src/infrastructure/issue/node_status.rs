#[cfg(feature = "online")]
pub(crate) async fn fetch_node_status(client: &reqwest::Client, url: &str) -> String {
    let base = url.trim_end_matches('/');
    match client
        .get(format!("{base}/version"))
        .send()
        .await
        .and_then(|r| r.error_for_status())
    {
        Ok(resp) => {
            let body = resp.text().await.unwrap_or_default();
            let health = client
                .get(format!("{base}/healthz"))
                .send()
                .await
                .map(|r| r.status().as_u16().to_string())
                .unwrap_or_else(|_| "?".to_string());
            format!("{base} → version={} healthz={health}", body.trim())
        }
        Err(_) => format!("unreachable ({base})"),
    }
}
