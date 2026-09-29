/// A browser-openable pre-filled `issues/new` URL (title + body + labels
/// percent-encoded). Labels are comma-joined into the `labels` query param so
/// the convention's `app:<name>` tag survives the no-token fallback path.
pub fn prefilled_url(repo: &str, title: &str, body: &str, labels: &[String]) -> String {
    let mut url = format!(
        "https://github.com/{repo}/issues/new?title={}&body={}",
        percent_encode_query(title),
        percent_encode_query(body),
    );
    if !labels.is_empty() {
        url.push_str(&format!(
            "&labels={}",
            percent_encode_query(&labels.join(","))
        ));
    }
    url
}

fn percent_encode_query(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub(crate) fn issue_url(repo: &str, number: u64) -> String {
    format!("https://github.com/{repo}/issues/{number}")
}

/// `GET https://api.github.com/search/issues?q=&per_page=` — the pre-existing
/// direct-GitHub search URL, unchanged, now named so `search()`'s fallback
/// branch is testable in isolation from the courier branch above it.
#[cfg(feature = "online")]
pub(crate) fn github_search_url(q: &str, limit: u32) -> String {
    format!(
        "https://api.github.com/search/issues?q={}&per_page={limit}",
        percent_encode_query(q),
    )
}

/// `GET https://api.github.com/repos/{repo}/issues/{number}` — the
/// pre-existing direct-GitHub view URL, unchanged, now named so `view()`'s
/// fallback branch is testable in isolation from the courier branch above it.
#[cfg(feature = "online")]
pub(crate) fn github_view_url(repo: &str, number: u64) -> String {
    format!("https://api.github.com/repos/{repo}/issues/{number}")
}

/// `GET {courier}/v1/issues/{owner}/{name}?state=&q=&limit=` — courier's
/// search endpoint URL. Pure and unit-tested so proxy-mode request routing
/// is verifiable without network I/O; `search()`'s courier branch calls this
/// directly (single source of truth).
#[cfg(feature = "online")]
pub(crate) fn courier_search_url(
    courier_url: &str,
    owner: &str,
    name: &str,
    state: &str,
    q: &str,
    limit: u32,
) -> String {
    format!(
        "{}/v1/issues/{owner}/{name}?state={}&q={}&limit={limit}",
        courier_url.trim_end_matches('/'),
        percent_encode_query(state),
        percent_encode_query(q),
    )
}

/// `GET {courier}/v1/issues/{owner}/{name}/{number}` — courier's view
/// endpoint URL. Pure and unit-tested; `view()`'s courier branch calls this
/// directly.
#[cfg(feature = "online")]
pub(crate) fn courier_view_url(courier_url: &str, owner: &str, name: &str, number: u64) -> String {
    format!(
        "{}/v1/issues/{owner}/{name}/{number}",
        courier_url.trim_end_matches('/')
    )
}

/// `POST {courier}/v1/issues/{owner}/{name}` — courier's create endpoint URL.
/// Pure and unit-tested; `create()`'s courier branch calls this directly.
#[cfg(feature = "online")]
pub(crate) fn courier_create_url(courier_url: &str, owner: &str, name: &str) -> String {
    format!(
        "{}/v1/issues/{owner}/{name}",
        courier_url.trim_end_matches('/')
    )
}

/// `POST {courier}/v1/issues/{owner}/{name}/{number}/comments` — courier's
/// comment endpoint URL. Pure and unit-tested; `comment()`'s courier branch
/// calls this directly.
#[cfg(feature = "online")]
pub(crate) fn courier_comment_url(
    courier_url: &str,
    owner: &str,
    name: &str,
    number: u64,
) -> String {
    format!(
        "{}/v1/issues/{owner}/{name}/{number}/comments",
        courier_url.trim_end_matches('/')
    )
}

#[cfg(all(test, feature = "online"))]
mod courier_routing_tests;
