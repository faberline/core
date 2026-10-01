use sha2::{Digest, Sha256};

/// The lowercase hex SHA-256 of `bytes`: a page's key and reference hash,
/// and an archived object's `sha256`.
pub(crate) fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut text = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut text, "{byte:02x}").expect("write to String cannot fail");
    }
    text
}
