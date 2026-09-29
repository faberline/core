use base64::Engine;

use super::hmac::{ct_eq, hex, hmac_sha256};
use super::scope::Scope;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::URL_SAFE_NO_PAD;

/// Sign a scope into a token: `b64url(json) "." hex(hmac)`.
pub fn sign(secret: &[u8], scope: &Scope) -> String {
    let payload = B64.encode(serde_json::to_vec(scope).expect("encode scope"));
    let sig = hmac_sha256(secret, payload.as_bytes());
    format!("{payload}.{}", hex(&sig))
}

/// Verify a token's signature (constant-time) and expiry (`now`, unix secs);
/// return its [`Scope`] if valid.
pub fn verify(secret: &[u8], token: &str, now: u64) -> Option<Scope> {
    let (payload, sig) = token.split_once('.')?;
    if !ct_eq(&hex(&hmac_sha256(secret, payload.as_bytes())), sig) {
        return None;
    }
    let scope: Scope = serde_json::from_slice(&B64.decode(payload).ok()?).ok()?;
    (scope.exp >= now).then_some(scope)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope() -> Scope {
        Scope {
            r: "run:a:in".into(),
            w: "run:a:result".into(),
            exp: 1000,
        }
    }

    #[test]
    fn sign_then_verify_roundtrips() {
        let t = sign(b"secret", &scope());
        assert_eq!(verify(b"secret", &t, 999), Some(scope()));
    }

    #[test]
    fn rejects_tamper_wrong_key_and_expiry() {
        let t = sign(b"secret", &scope());
        assert!(verify(b"WRONG", &t, 999).is_none(), "wrong secret");
        assert!(
            verify(b"secret", &t, 1001).is_none(),
            "expired (exp=1000, now=1001)"
        );
        let tampered = format!("{}.deadbeef", t.split_once('.').unwrap().0);
        assert!(
            verify(b"secret", &tampered, 999).is_none(),
            "tampered signature"
        );
        assert!(verify(b"secret", "no-dot", 999).is_none(), "malformed");
    }
}
