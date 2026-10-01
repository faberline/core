use super::resolve_github_token_from;

#[test]
fn gh_token_takes_precedence() {
    let got = resolve_github_token_from(
        |v| match v {
            "GH_TOKEN" => Some("from-gh-env".to_string()),
            "GITHUB_TOKEN" => Some("from-github".to_string()),
            _ => None,
        },
        || Some("from-gh-cli".to_string()),
    );
    assert_eq!(got.as_deref(), Some("from-gh-env"));
}

#[test]
fn github_token_is_second() {
    let got = resolve_github_token_from(
        |v| (v == "GITHUB_TOKEN").then(|| "from-github".to_string()),
        || Some("from-gh-cli".to_string()),
    );
    assert_eq!(got.as_deref(), Some("from-github"));
}

#[test]
fn falls_back_to_gh_cli_when_env_absent_or_blank() {
    // Blank env values are skipped, so the gh CLI store is consulted.
    let got = resolve_github_token_from(
        |v| (v == "GH_TOKEN").then(|| "   ".to_string()),
        || Some("from-gh-cli".to_string()),
    );
    assert_eq!(got.as_deref(), Some("from-gh-cli"));
}

#[test]
fn none_when_no_credential_anywhere() {
    let got = resolve_github_token_from(|_| None, || None);
    assert_eq!(got, None);
}
