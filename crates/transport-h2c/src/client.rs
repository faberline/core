use std::time::Duration;

/// Build a single-connection h2c client — the drop-in replacement for
/// `reqwest::Client::builder().http2_prior_knowledge().build()`.
pub fn h2c_client() -> reqwest::Result<reqwest::Client> {
    h2c_builder(None, None).build()
}

/// Like [`h2c_client`] with an optional per-request `timeout` and `user_agent`.
pub fn h2c_client_with(
    timeout: Option<Duration>,
    user_agent: Option<&str>,
) -> reqwest::Result<reqwest::Client> {
    h2c_builder(timeout, user_agent).build()
}

pub(crate) fn h2c_builder(
    timeout: Option<Duration>,
    user_agent: Option<&str>,
) -> reqwest::ClientBuilder {
    let mut b = reqwest::Client::builder().http2_prior_knowledge();
    if let Some(t) = timeout {
        b = b.timeout(t);
    }
    if let Some(ua) = user_agent {
        b = b.user_agent(ua.to_string());
    }
    b
}
