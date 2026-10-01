use sha2::{Digest, Sha256};

/// A SHA-256 digest of a bearer token, used as a cache key.
///
/// The digest is the key precisely so that a memory dump, a debugger, or a
/// `Debug` print of the cache cannot yield a usable credential.
pub(crate) type TokenDigest = [u8; 32];

pub(crate) fn digest(token: &str) -> TokenDigest {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hasher.finalize().into()
}

/// A short, stable, non-reversible handle on a token, for correlating audit
/// lines about the same caller. Six bytes of SHA-256 — enough to correlate,
/// far too little to be a credential.
pub fn fingerprint(token: &str) -> String {
    digest(token)[..6]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
