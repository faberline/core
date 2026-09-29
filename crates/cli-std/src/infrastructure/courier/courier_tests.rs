use super::{resolve_courier_token_from, resolve_courier_url_from};

#[test]
fn resolve_courier_url_returns_some_when_env_set() {
    let got = resolve_courier_url_from(|v| {
        (v == "AXIOM_COURIER_URL").then(|| "  https://courier.internal  ".to_string())
    });
    assert_eq!(got.as_deref(), Some("https://courier.internal"));
}

#[test]
fn resolve_courier_url_returns_none_when_env_unset_or_blank() {
    assert_eq!(resolve_courier_url_from(|_| None), None);
    let blank = resolve_courier_url_from(|v| (v == "AXIOM_COURIER_URL").then(|| "   ".to_string()));
    assert_eq!(blank, None);
}

#[test]
fn resolve_courier_token_returns_some_when_env_set() {
    let got = resolve_courier_token_from(|v| {
        (v == "AXIOM_COURIER_TOKEN").then(|| " secret-token ".to_string())
    });
    assert_eq!(got.as_deref(), Some("secret-token"));
}

#[test]
fn resolve_courier_token_returns_none_when_env_unset_or_blank() {
    assert_eq!(resolve_courier_token_from(|_| None), None);
    let blank =
        resolve_courier_token_from(|v| (v == "AXIOM_COURIER_TOKEN").then(|| "".to_string()));
    assert_eq!(blank, None);
}
