//! HTTP calls to the service under test. Transport failures are retried; HTTP
//! error statuses are not.

use reqwest::Method;
use serde_json::Value;
use std::time::Duration;

const TRANSPORT_RETRIES: u32 = 5;
const RETRY_DELAY: Duration = Duration::from_millis(200);
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// A client for one base URL.
#[derive(Debug, Clone)]
pub struct Api {
    /// The base URL without a trailing `/`.
    pub base: String,
}

impl Api {
    /// A client for `base`; trailing `/` characters are removed.
    pub fn new(base: &str) -> Self {
        Self {
            base: base.trim_end_matches('/').to_string(),
        }
    }

    /// A client for the URL in `E2E_API_BASE`.
    pub fn from_env() -> Self {
        Self::new(&super::env::required("E2E_API_BASE"))
    }

    /// Sends one request. Retries only a send that got no HTTP response.
    ///
    /// # Panics
    ///
    /// Panics when every attempt fails, when the status is 4xx or 5xx, or
    /// when the body is not JSON.
    pub async fn call(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
        timeout: Duration,
    ) -> Value {
        let url = format!("{}{}", self.base, path);
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_else(|e| panic!("http client: {e}"));
        let attempts = 1 + TRANSPORT_RETRIES;
        let mut last_error = String::new();
        for attempt in 0..attempts {
            if attempt > 0 {
                tokio::time::sleep(RETRY_DELAY).await;
            }
            let mut request = client.request(method.clone(), &url);
            if let Some(json) = body {
                request = request.json(json);
            }
            match request.send().await {
                Ok(response) => {
                    let status = response.status();
                    let text = response
                        .text()
                        .await
                        .unwrap_or_else(|e| panic!("{method} {url} body: {e}"));
                    if status.is_client_error() || status.is_server_error() {
                        panic!("{method} {url} returned {}: {text}", status.as_u16());
                    }
                    if text.is_empty() {
                        return Value::Null;
                    }
                    return serde_json::from_str(&text)
                        .unwrap_or_else(|e| panic!("invalid JSON from {url}: {e}"));
                }
                Err(e) => last_error = e.to_string(),
            }
        }
        panic!("{method} {url} failed after {attempts} attempts: {last_error}");
    }

    /// GET `path`.
    pub async fn get(&self, path: &str) -> Value {
        self.call(Method::GET, path, None, DEFAULT_TIMEOUT).await
    }

    /// POST `body` to `path`.
    pub async fn post(&self, path: &str, body: &Value) -> Value {
        self.call(Method::POST, path, Some(body), DEFAULT_TIMEOUT)
            .await
    }

    /// PUT `body` to `path`.
    pub async fn put(&self, path: &str, body: &Value) -> Value {
        self.call(Method::PUT, path, Some(body), DEFAULT_TIMEOUT)
            .await
    }

    /// True only when a GET of `path` gets a 2xx answer within 5 seconds.
    /// It never panics and never retries.
    pub async fn answers(&self, path: &str) -> bool {
        let url = format!("{}{}", self.base, path);
        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
        {
            Ok(c) => c,
            Err(_) => return false,
        };
        match client.get(&url).send().await {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::Method;
    use serde_json::json;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc::Sender;
    use std::time::Duration;

    const OK_200: &str = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}";

    /// One entry per accepted connection. `None` drops it at once; `Some(raw)`
    /// reads the request, sends `raw`, then drops the connection.
    fn serve(responses: Vec<Option<&'static str>>) -> String {
        serve_with(responses, None)
    }

    fn serve_with(responses: Vec<Option<&'static str>>, seen: Option<Sender<String>>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        std::thread::spawn(move || {
            for response in responses {
                let (mut stream, _) = listener.accept().expect("accept");
                let Some(raw) = response else { continue };
                let mut request = Vec::new();
                let mut buf = [0u8; 1024];
                while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    let n = stream.read(&mut buf).expect("read");
                    if n == 0 {
                        break;
                    }
                    request.extend_from_slice(&buf[..n]);
                }
                if let Some(tx) = &seen {
                    let _ = tx.send(String::from_utf8_lossy(&request).into_owned());
                }
                let _ = stream.write_all(raw.as_bytes());
            }
        });
        format!("http://127.0.0.1:{port}")
    }

    #[tokio::test]
    async fn call_retries_a_transport_failure_then_succeeds() {
        let base = serve(vec![None, Some(OK_200)]);
        let value = Api::new(&base)
            .call(Method::GET, "/x", None, Duration::from_secs(5))
            .await;
        assert_eq!(value, json!({"ok": true}));
    }

    #[tokio::test]
    #[should_panic(expected = "returned 500")]
    async fn call_panics_on_a_500_and_does_not_retry() {
        let base = serve(vec![Some(
            "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 4\r\nConnection: close\r\n\r\nboom",
        )]);
        Api::new(&base).get("/x").await;
    }

    #[tokio::test]
    #[should_panic(expected = "failed after 6 attempts")]
    async fn call_panics_after_the_retry_budget() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        drop(listener);
        Api::new(&format!("http://127.0.0.1:{port}"))
            .call(Method::GET, "/x", None, Duration::from_secs(1))
            .await;
    }

    #[test]
    fn new_trims_the_trailing_slash() {
        assert_eq!(Api::new("http://h/").base, "http://h");
    }

    #[tokio::test]
    async fn answers_is_true_for_a_2xx_and_false_for_a_closed_port() {
        let base = serve(vec![Some(OK_200)]);
        assert!(Api::new(&base).answers("/x").await);
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        drop(listener);
        assert!(
            !Api::new(&format!("http://127.0.0.1:{port}"))
                .answers("/x")
                .await
        );
    }

    #[tokio::test]
    async fn post_sends_the_json_body() {
        let (tx, rx) = std::sync::mpsc::channel();
        let base = serve_with(vec![Some(OK_200)], Some(tx));
        Api::new(&base).post("/p", &json!({"k": 1})).await;
        let request = rx.recv().expect("request");
        assert!(request.starts_with("POST /p "), "{request}");
        assert!(request.contains("{\"k\":1}"), "{request}");
    }
}
