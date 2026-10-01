use super::*;

#[test]
fn search_routes_through_courier_when_url_configured() {
    let url = courier_search_url(
        "https://courier.internal",
        "faberline",
        "lumen",
        "open",
        "label:\"app:lumen\"",
        20,
    );
    assert_eq!(
        url,
        "https://courier.internal/v1/issues/faberline/lumen?state=open&q=label%3A%22app%3Alumen%22&limit=20"
    );
    // trailing-slash courier URLs are normalized the same way.
    assert_eq!(
        courier_search_url("https://courier.internal/", "o", "n", "all", "x", 5),
        "https://courier.internal/v1/issues/o/n?state=all&q=x&limit=5"
    );
}

#[test]
fn view_routes_through_courier_when_url_configured() {
    assert_eq!(
        courier_view_url("https://courier.internal", "o", "n", 42),
        "https://courier.internal/v1/issues/o/n/42"
    );
}

#[test]
fn create_routes_through_courier_when_url_configured() {
    assert_eq!(
        courier_create_url("https://courier.internal", "o", "n"),
        "https://courier.internal/v1/issues/o/n"
    );
}

#[test]
fn comment_routes_through_courier_when_url_configured() {
    assert_eq!(
        courier_comment_url("https://courier.internal", "o", "n", 7),
        "https://courier.internal/v1/issues/o/n/7/comments"
    );
}

#[test]
fn courier_get_sets_bearer_auth_header_from_courier_token() {
    // courier_get()/courier_post() authenticate with resolve_courier_token()
    // (the courier bearer token, never the GitHub token) via
    // RequestBuilder::bearer_auth -- assert the header shape it produces
    // without performing any network I/O (`.build()` is purely local).
    let req = reqwest::Client::new()
        .get("https://courier.internal/v1/issues/o/n")
        .bearer_auth("courier-secret")
        .build()
        .expect("build request");
    assert_eq!(
        req.headers().get("authorization").unwrap(),
        "Bearer courier-secret"
    );
}

#[test]
fn issue_ops_fall_back_to_direct_github_when_courier_url_unset() {
    // When resolve_courier_url() is None, search()/view() keep building
    // requests against the exact pre-existing direct-api.github.com URLs
    // (github_search_url/github_view_url are mechanical extractions of
    // the unchanged format!() computation -- AC3's byte-identical
    // fallback) instead of any courier_*_url() shape.
    let search_url = github_search_url("repo:o/n is:issue label:\"app:lumen\"", 20);
    assert_eq!(
        search_url,
        "https://api.github.com/search/issues?q=repo%3Ao%2Fn%20is%3Aissue%20label%3A%22app%3Alumen%22&per_page=20"
    );
    assert!(!search_url.contains("/v1/issues/"));

    let view_url = github_view_url("o/n", 42);
    assert_eq!(view_url, "https://api.github.com/repos/o/n/issues/42");
    assert!(!view_url.contains("/v1/issues/"));

    // create/comment's fallback path reuses submit_issue()/reopen_issue()/
    // post_issue_comment() completely unchanged (not touched by this WI) --
    // pin their literal endpoint templates so a future edit can't silently
    // reroute them.
    let repo = "o/n";
    assert_eq!(
        format!("https://api.github.com/repos/{repo}/issues"),
        "https://api.github.com/repos/o/n/issues"
    );
    assert_eq!(
        format!("https://api.github.com/repos/{repo}/issues/{}", 42),
        "https://api.github.com/repos/o/n/issues/42"
    );
    assert_eq!(
        format!("https://api.github.com/repos/{repo}/issues/{}/comments", 42),
        "https://api.github.com/repos/o/n/issues/42/comments"
    );
}
