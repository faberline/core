//! The HTTP client behind the remote ports: the GitHub API, courier, a
//! node's status endpoints and release downloads.

use crate::domain::remote::RemoteError;

/// A `reqwest` client with the tool's user agent. It implements every HTTP
/// port.
pub(crate) struct HttpClient {
    pub(in crate::infrastructure) client: reqwest::Client,
}

impl HttpClient {
    /// Build a client that sends `user_agent`.
    pub(crate) fn new(user_agent: String) -> Result<Self, RemoteError> {
        let client = reqwest::Client::builder()
            .user_agent(user_agent)
            .build()
            .map_err(|e| RemoteError::Client(Box::new(e)))?;
        Ok(Self { client })
    }
}
