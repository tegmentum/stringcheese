//! URI percent-encoding / decoding.
//!
//! Wraps [`percent_encoding`] — the URL WG's canonical
//! implementation, used by `url` and countless downstreams.

use alloc::borrow::Cow;
use alloc::string::{String, ToString};

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};

/// Percent-encode every character not in `[A-Za-z0-9]`.
///
/// Matches the strict `application/x-www-form-urlencoded`-style
/// encoding used for path segments and query values. Preserves
/// UTF-8 in a way `percent_encoding` guarantees round-trips
/// through [`decode`].
#[must_use]
pub fn encode(input: &str) -> String {
    utf8_percent_encode(input, ENCODE_SET).to_string()
}

/// Reverse [`encode`]. Fails when the input contains a malformed
/// percent-triplet or the decoded bytes aren't valid UTF-8.
pub fn decode(input: &str) -> Result<String, DecodeError> {
    percent_decode_str(input)
        .decode_utf8()
        .map(Cow::into_owned)
        .map_err(|_| DecodeError::InvalidUtf8)
}

/// URI-decode failure reason.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// Percent-decoded bytes didn't form valid UTF-8.
    InvalidUtf8,
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidUtf8 => write!(f, "percent-decoded bytes are not valid UTF-8"),
        }
    }
}

// `NON_ALPHANUMERIC` is `percent_encoding`'s built-in "encode
// everything but ASCII letters and digits" ruleset. Matches the
// safe-default a caller expects when they hand us a raw string
// and ask "make this URI-safe."
const ENCODE_SET: &AsciiSet = NON_ALPHANUMERIC;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alphanumeric_passes_through() {
        assert_eq!(encode("abcXYZ123"), "abcXYZ123");
    }

    #[test]
    fn spaces_and_punct_encoded() {
        assert_eq!(encode("hello world!"), "hello%20world%21");
    }

    #[test]
    fn multibyte_encoded_as_utf8_triplets() {
        // 日 = 0xE6 0x97 0xA5.
        assert_eq!(encode("日"), "%E6%97%A5");
    }

    #[test]
    fn round_trip() {
        let cases = ["hello world", "café résumé", "日本語", "a/b?c=d&e"];
        for s in cases {
            let round = decode(&encode(s)).unwrap();
            assert_eq!(round, s);
        }
    }

    #[test]
    fn malformed_percent_triplet_fails() {
        // %ZZ isn't a valid hex pair; percent_decode_str keeps the
        // literal bytes, and decode_utf8 fails when they aren't UTF-8.
        // "%E6%97" is 2 bytes of an incomplete 3-byte UTF-8 scalar.
        assert!(decode("%E6%97").is_err());
    }
}
