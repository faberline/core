//! Product-neutral templates for security checks on a service's HTTP API: how
//! it answers a missing credential, a credential of the wrong role and a valid
//! credential. The base URL, the path and the credentials are parameters.

use std::time::Duration;

/// GET `path` on `base`, with `token` as a bearer credential when given.
/// Returns the full URL and the status code. Never retries.
///
/// # Panics
///
/// Panics when the HTTP client cannot be built or the request fails.
async fn get_status(base: &str, path: &str, token: Option<&str>) -> (String, u16) {
    let url = format!("{}{}", base.trim_end_matches('/'), path);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap_or_else(|e| panic!("http client: {e}"));
    let mut request = client.get(&url);
    if let Some(token) = token {
        request = request.header("Authorization", format!("Bearer {token}"));
    }
    let response = request
        .send()
        .await
        .unwrap_or_else(|e| panic!("security: GET {url} failed: {e}"));
    let status = response.status().as_u16();
    (url, status)
}

/// Checks that GET `path` without a credential is refused with 401.
///
/// # Panics
///
/// Panics unless the status is 401, or when the request fails.
pub async fn assert_rejects_missing_credential(base: &str, path: &str) {
    let (url, status) = get_status(base, path, None).await;
    if status != 401 {
        panic!("security: GET {url} without a credential returned {status}, expected 401");
    }
}

/// Checks that GET `path` with a valid credential of a role that must not
/// reach it is refused with 403.
///
/// # Panics
///
/// Panics unless the status is 403, or when the request fails.
pub async fn assert_rejects_wrong_role(base: &str, path: &str, token: &str) {
    let (url, status) = get_status(base, path, Some(token)).await;
    if status != 403 {
        panic!("security: GET {url} with a wrong role returned {status}, expected 403");
    }
}

/// Checks that GET `path` with a valid credential is accepted (2xx).
///
/// # Panics
///
/// Panics unless the status is 2xx, or when the request fails.
pub async fn assert_accepts_credential(base: &str, path: &str, token: &str) {
    let (url, status) = get_status(base, path, Some(token)).await;
    if !(200..300).contains(&status) {
        panic!("security: GET {url} with a valid credential returned {status}, expected 2xx");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// Serves forever on a local port. `answer` gets the `Authorization`
    /// header value (`None` when the request has none) and returns the status
    /// code of the response.
    fn serve(answer: fn(Option<&str>) -> u16) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut request = Vec::new();
                let mut buf = [0u8; 1024];
                while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    match stream.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => request.extend_from_slice(&buf[..n]),
                    }
                }
                let head = String::from_utf8_lossy(&request).into_owned();
                let authorization = head.lines().find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.trim()
                        .eq_ignore_ascii_case("authorization")
                        .then(|| value.trim())
                });
                let code = answer(authorization);
                let reason = match code {
                    200 => "OK",
                    401 => "Unauthorized",
                    403 => "Forbidden",
                    _ => "Status",
                };
                let response = format!(
                    "HTTP/1.1 {code} {reason}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        format!("http://127.0.0.1:{port}")
    }

    /// Answers like a correct service: 401 with no credential, 403 for the
    /// `viewer` role, 200 for the `admin` role.
    fn correct(authorization: Option<&str>) -> u16 {
        match authorization {
            None => 401,
            Some("Bearer viewer") => 403,
            Some("Bearer admin") => 200,
            Some(_) => 401,
        }
    }

    fn accept_all(_: Option<&str>) -> u16 {
        200
    }

    fn reject_all(_: Option<&str>) -> u16 {
        401
    }

    #[tokio::test]
    async fn correct_server_passes_all_three_templates() {
        let base = serve(correct);
        assert_rejects_missing_credential(&base, "/x").await;
        assert_rejects_wrong_role(&base, "/x", "viewer").await;
        assert_accepts_credential(&base, "/x", "admin").await;
    }

    #[tokio::test]
    #[should_panic(expected = "without a credential returned 200, expected 401")]
    async fn accepts_a_missing_credential_fails_the_template() {
        let base = serve(accept_all);
        assert_rejects_missing_credential(&base, "/x").await;
    }

    #[tokio::test]
    #[should_panic(expected = "with a wrong role returned 200, expected 403")]
    async fn accepts_a_wrong_role_fails_the_template() {
        let base = serve(accept_all);
        assert_rejects_wrong_role(&base, "/x", "viewer").await;
    }

    #[tokio::test]
    #[should_panic(expected = "with a valid credential returned 401, expected 2xx")]
    async fn rejecting_a_valid_credential_fails_the_template() {
        let base = serve(reject_all);
        assert_accepts_credential(&base, "/x", "admin").await;
    }
}
