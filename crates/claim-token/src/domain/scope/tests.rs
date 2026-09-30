use super::Scope;
use crate::domain::{sign, verify};

const SCOPE_JSON: &str = r#"{"r":"run:a:in","w":"run:a:result","exp":1000}"#;

/// `sign(b"secret", &scope())`: base64url(SCOPE_JSON) "." hex(hmac-sha256).
const TOKEN: &str = "eyJyIjoicnVuOmE6aW4iLCJ3IjoicnVuOmE6cmVzdWx0IiwiZXhwIjoxMDAwfQ.44d16019b5d2e7ec4b36ac63689fe8eab5a275ba40ab444f97856b01c52ca6e4";

fn scope() -> Scope {
    Scope {
        r: "run:a:in".into(),
        w: "run:a:result".into(),
        exp: 1000,
    }
}

#[test]
fn scope_json_is_pinned() {
    assert_eq!(serde_json::to_string(&scope()).unwrap(), SCOPE_JSON);
    assert_eq!(serde_json::from_str::<Scope>(SCOPE_JSON).unwrap(), scope());
}

#[test]
fn signed_token_bytes_are_pinned() {
    assert_eq!(sign(b"secret", &scope()), TOKEN);
    assert_eq!(verify(b"secret", TOKEN, 1000), Some(scope()));
}
