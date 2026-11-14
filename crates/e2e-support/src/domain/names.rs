/// RFC 1123 DNS label: 1 to 63 chars, only `a-z`, `0-9`, `-`; starts and ends with `a-z` or `0-9`.
pub fn is_dns_label(s: &str) -> bool {
    let bytes = s.as_bytes();
    let alnum = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit();
    (1..=63).contains(&bytes.len())
        && bytes.iter().all(|&b| alnum(b) || b == b'-')
        && bytes.first().is_some_and(|&b| alnum(b))
        && bytes.last().is_some_and(|&b| alnum(b))
}

/// Exactly `len` chars, each in `0-9a-f` (lower case only).
pub fn is_lower_hex(s: &str, len: usize) -> bool {
    s.len() == len
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dns_label_accepts_valid_labels() {
        let long = "x".repeat(63);
        for s in ["a", "a-b1", long.as_str()] {
            assert!(is_dns_label(s), "{s}");
        }
    }

    #[test]
    fn dns_label_rejects_invalid_labels() {
        let long = "x".repeat(64);
        for s in ["", "-a", "a-", "A", "a_b", long.as_str()] {
            assert!(!is_dns_label(s), "{s}");
        }
    }

    #[test]
    fn lower_hex_checks_length_and_case() {
        assert!(is_lower_hex("0f9a", 4));
        assert!(!is_lower_hex("0F9a", 4));
        assert!(!is_lower_hex("0f9", 4));
        assert!(!is_lower_hex("0g9a", 4));
    }
}
