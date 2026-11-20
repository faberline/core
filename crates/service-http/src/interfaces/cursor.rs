//! Opaque cursor codec: URL-safe base64 without padding.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;

/// The cursor text is not valid URL-safe base64 without padding.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("cursor is not valid URL-safe base64")]
pub struct CursorDecodeError;

/// Encode caller bytes as an opaque cursor string.
pub fn encode_cursor(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Decode an opaque cursor string back to the caller bytes.
pub fn decode_cursor(text: &str) -> Result<Vec<u8>, CursorDecodeError> {
    URL_SAFE_NO_PAD
        .decode(text)
        .map_err(|_| CursorDecodeError)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let all: Vec<u8> = (0u8..=255).collect();
        for input in [&b""[..], b"abc", &[0xfb, 0xff], all.as_slice()] {
            assert_eq!(decode_cursor(&encode_cursor(input)), Ok(input.to_vec()));
        }
    }

    #[test]
    fn encoding_is_url_safe_without_padding() {
        assert_eq!(encode_cursor(&[0xfb, 0xff]), "-_8");
        assert_eq!(encode_cursor(b"a"), "YQ");
    }

    #[test]
    fn bad_input_is_an_error_not_a_panic() {
        assert_eq!(decode_cursor("!!!"), Err(CursorDecodeError));
        assert_eq!(decode_cursor("a"), Err(CursorDecodeError));
        assert_eq!(decode_cursor("YQ=="), Err(CursorDecodeError));
        assert_eq!(decode_cursor("a b"), Err(CursorDecodeError));
    }
}
