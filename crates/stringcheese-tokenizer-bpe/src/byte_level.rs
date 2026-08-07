//! Byte-level byte-to-Unicode mapping — the "`bytes_char()`" table.
//!
//! # Rationale
//!
//! Byte-level BPE tokenizers (GPT-2, Llama-2/3, and any Hugging Face
//! `tokenizer.json` that carries a `ByteLevel` pre-tokenizer) sit on a
//! canonical bijection between raw bytes `0..=255` and printable Unicode
//! code points. Before running the BPE merge loop, every input byte is
//! remapped through this table, so the whole vocabulary and every merge
//! rule can be written and stored as printable UTF-8. The `Ġ` glyph
//! that shows up in `Ġhello` is byte `0x20` (ASCII space) after
//! remapping — the low 32 bytes plus a handful of "unprintable" Latin-1
//! bytes get lifted into U+0100..U+0143 to keep the vocabulary all
//! plain-text.
//!
//! Because the mapping is a bijection over 256 elements the inverse is
//! also a simple table; decoding a byte-level tokenizer's output is
//! literally "join the pieces, look each char up in the reverse table,
//! interpret the recovered byte string as UTF-8." This module ships
//! both tables plus the string-in / string-out helpers
//! [`encode_bytes`] and [`decode_chars`] that the crate uses at the
//! seams between the byte-level layer and the plain BPE core.
//!
//! # The mapping itself
//!
//! Faithful port of Hugging Face's `tokenizers/src/pre_tokenizers/
//! byte_level.rs::bytes_char()`, which in turn came from Radford et
//! al.'s `bpe_tokenizer.py` in the original GPT-2 release:
//!
//! * Printable ASCII (`0x21..=0x7E`), the Latin-1 supplement's
//!   printable ranges (`0xA1..=0xAC` and `0xAE..=0xFF`) — 188 bytes
//!   total — pass through unchanged.
//! * The remaining 68 "unprintable" bytes (`0x00..=0x20`, `0x7F..=0xA0`,
//!   `0xAD`) are lifted, in ascending order, to code points
//!   `U+0100..=U+0143`. Byte `0x20` (space) therefore lands at
//!   `U+0120` (`Ġ`); byte `0x00` at `U+0100` (`Ā`); byte `0xAD` at
//!   `U+0143` (`Ń`); and so on.
//!
//! The inverse table [`CHARS_TO_BYTES`] is dense over
//! `U+0000..=U+0143` — 324 entries — and returns `None` for any code
//! point outside that range. Every `ByteLevel`-encoded character
//! produced by [`encode_bytes`] lands inside the range, so decode is
//! infallible on well-formed input.
//!
//! # Round-trip
//!
//! `decode_chars(encode_bytes(s))` returns `s.as_bytes().to_vec()` for
//! every UTF-8 string `s`. The reverse also holds when the input to
//! `decode_chars` came from `encode_bytes` — chars outside the
//! bijection are passed through as their raw UTF-8 bytes, so callers
//! who feed the decoder a string that was *not* byte-level encoded
//! still get a byte-string back (rather than a panic), but the round
//! trip is no longer defined.

use alloc::string::String;
use alloc::vec::Vec;

/// Number of code points spanned by the `ByteLevel` mapping range.
///
/// Every code point that can legally appear in a `ByteLevel`-encoded
/// string fits below `U+0144`: printable ASCII (0..=127), the printable
/// Latin-1 supplement (up to `U+00FF`), and the 68 lifted bytes at
/// `U+0100..=U+0143`. The inverse table [`CHARS_TO_BYTES`] is sized to
/// match.
pub const CHARS_TO_BYTES_LEN: usize = 0x144;

const MAPS: ([char; 256], [Option<u8>; CHARS_TO_BYTES_LEN]) = compute_maps();

/// Forward table: byte value → printable Unicode char.
///
/// See the module-level docs for the derivation. The array is indexed by
/// the raw byte, so `BYTES_TO_CHARS[0x20]` is `'Ġ'` (`U+0120`) and
/// `BYTES_TO_CHARS[b'A' as usize]` is `'A'`.
pub const BYTES_TO_CHARS: [char; 256] = MAPS.0;

/// Reverse table: code point → byte value, sparse in the low 0x144
/// range.
///
/// Indexed by the code point value cast to `usize`; returns `None` for
/// code points that are not part of the `ByteLevel` mapping. Every char
/// yielded by [`encode_bytes`] hits `Some(_)` here — the two tables
/// together are a genuine bijection over 256 elements.
pub const CHARS_TO_BYTES: [Option<u8>; CHARS_TO_BYTES_LEN] = MAPS.1;

// Every cast inside `compute_maps` is bounded by the `while b < 256`
// loop guard, so the `u32 -> u8` and `u32 -> usize` casts are exact.
// Clippy can't see the loop guard, hence the explicit allow.
#[allow(clippy::cast_possible_truncation)]
const fn compute_maps() -> ([char; 256], [Option<u8>; CHARS_TO_BYTES_LEN]) {
    let mut bytes_to_chars: [char; 256] = ['\0'; 256];
    let mut chars_to_bytes: [Option<u8>; CHARS_TO_BYTES_LEN] = [None; CHARS_TO_BYTES_LEN];

    // "Printable" bytes stay as themselves; unprintable bytes get lifted
    // to U+0100..U+0143 in ascending byte order. `n` counts how many
    // unprintable bytes we've seen so far. The loop variable is `u32`
    // (rather than `u8` or `usize`) so the printable-range comparisons
    // below stay in one type — a `u8` loop would wrap on `b += 1`
    // after `b == 255`, and clippy correctly refuses `usize as u32`
    // as a truncation risk on wide-pointer targets.
    let mut n: u32 = 0;
    let mut b: u32 = 0;
    while b < 256 {
        let printable =
            (b >= 0x21 && b <= 0x7E) || (b >= 0xA1 && b <= 0xAC) || (b >= 0xAE && b <= 0xFF);
        let cp = if printable {
            b
        } else {
            let c = 256 + n;
            n += 1;
            c
        };
        // `cp` is always <= 0x143 by construction (188 printable bytes
        // stay in 0x21..=0xFF, 68 unprintable lift to 0x100..=0x143), so
        // `char::from_u32` never returns `None` here. Panic on the
        // impossible branch to keep the table honest.
        let Some(ch) = char::from_u32(cp) else {
            panic!("ByteLevel mapping produced an invalid Unicode scalar");
        };
        // `b` is always < 256 by the loop guard so both casts are
        // exact; `cp` is <= 0x143 which is well below `usize::MAX`.
        bytes_to_chars[b as usize] = ch;
        chars_to_bytes[cp as usize] = Some(b as u8);
        b += 1;
    }

    (bytes_to_chars, chars_to_bytes)
}

/// Encode a UTF-8 string as a `ByteLevel`-remapped UTF-8 string.
///
/// Every UTF-8 byte of `text` is looked up in [`BYTES_TO_CHARS`] and
/// pushed to the output as a Unicode char. The output is therefore
/// always valid UTF-8 and every char lands in the `ByteLevel`-mapped
/// range, so [`decode_chars`] round-trips it back to the original byte
/// string.
///
/// # Examples
///
/// ```
/// use stringcheese_tokenizer_bpe::byte_level::encode_bytes;
///
/// // ASCII stays ASCII where the byte value is in the printable range.
/// assert_eq!(encode_bytes("Hello"), "Hello");
/// // The space byte (0x20) becomes `Ġ` (U+0120).
/// assert_eq!(encode_bytes(" world"), "Ġworld");
/// // Multi-byte UTF-8 characters are encoded byte-for-byte: `é` is
/// // 0xC3 0xA9 in UTF-8, which the mapping turns into two separate
/// // printable-Latin-1 chars `Ã` and `©` — the canonical shape
/// // GPT-2 / Llama vocabularies use for accented characters.
/// assert_eq!(encode_bytes("café"), "cafÃ©");
/// ```
#[must_use]
pub fn encode_bytes(text: &str) -> String {
    // Every mapped char is <= 2 UTF-8 bytes (all mapped code points are
    // below U+0800), so 2*len is a tight upper bound.
    let mut out = String::with_capacity(text.len().saturating_mul(2));
    for &b in text.as_bytes() {
        out.push(BYTES_TO_CHARS[b as usize]);
    }
    out
}

/// Decode a `ByteLevel`-encoded UTF-8 string back to its raw bytes.
///
/// For each `char` of `text`:
///
/// * If the char is in the `ByteLevel` range and mapped in
///   [`CHARS_TO_BYTES`], push its byte value.
/// * Otherwise push the char's own UTF-8 encoding — the decoder is
///   forgiving so callers who accidentally feed it plain text don't
///   see a panic; the round-trip guarantee is lost but nothing crashes.
///
/// # Examples
///
/// ```
/// use stringcheese_tokenizer_bpe::byte_level::{decode_chars, encode_bytes};
///
/// let raw = " Hello, world!";
/// let encoded = encode_bytes(raw);
/// assert_eq!(&encoded, "ĠHello,Ġworld!");
/// assert_eq!(decode_chars(&encoded), raw.as_bytes());
/// ```
#[must_use]
pub fn decode_chars(text: &str) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::with_capacity(text.len());
    for c in text.chars() {
        let cp = c as u32;
        let idx = cp as usize;
        if idx < CHARS_TO_BYTES_LEN {
            if let Some(b) = CHARS_TO_BYTES[idx] {
                out.push(b);
                continue;
            }
        }
        // Fallback: keep the raw UTF-8 bytes so the function is
        // total. This branch is only reachable when the caller feeds
        // in a string that was not produced by `encode_bytes`.
        let mut buf = [0u8; 4];
        let s = c.encode_utf8(&mut buf);
        out.extend_from_slice(s.as_bytes());
    }
    out
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    /// Reference implementation, transcribed one-for-one from Hugging
    /// Face's `tokenizers/src/pre_tokenizers/byte_level.rs::bytes_char()`
    /// (which itself came from GPT-2's `bpe_tokenizer.py`). Kept as an
    /// oracle so the crate's constant tables are always checkable
    /// against the canonical formulation without hand-typed magic
    /// numbers.
    fn reference_bytes_char() -> Vec<(u8, char)> {
        let mut bs: Vec<u32> = Vec::new();
        for b in b'!'..=b'~' {
            bs.push(u32::from(b));
        }
        for b in 0xA1u32..=0xAC {
            bs.push(b);
        }
        for b in 0xAEu32..=0xFF {
            bs.push(b);
        }
        let mut cs: Vec<u32> = bs.clone();
        let mut n: u32 = 0;
        for b in 0u32..256 {
            if !bs.contains(&b) {
                bs.push(b);
                cs.push(256 + n);
                n += 1;
            }
        }
        bs.iter()
            .zip(cs.iter())
            .map(|(&b, &c)| {
                // `b` is `0..256` by construction — the `bs`
                // initialiser only ever appends values in that range.
                let b8 = u8::try_from(b).expect("byte value out of range");
                (b8, char::from_u32(c).unwrap())
            })
            .collect()
    }

    #[test]
    fn forward_table_matches_hf_reference() {
        for (b, expected) in reference_bytes_char() {
            assert_eq!(
                BYTES_TO_CHARS[b as usize], expected,
                "byte {b:#04x} maps to {:?} in the constant, expected {expected:?}",
                BYTES_TO_CHARS[b as usize]
            );
        }
    }

    #[test]
    fn inverse_table_is_the_true_inverse() {
        for b in 0u8..=255 {
            let ch = BYTES_TO_CHARS[b as usize];
            let cp = ch as usize;
            assert!(cp < CHARS_TO_BYTES_LEN, "char {ch:?} outside table range");
            assert_eq!(
                CHARS_TO_BYTES[cp],
                Some(b),
                "inverse of {ch:?} was {:?}, expected Some({b})",
                CHARS_TO_BYTES[cp]
            );
        }
    }

    #[test]
    fn all_forward_chars_are_distinct() {
        // Bijection: 256 distinct output chars for 256 input bytes.
        let mut seen: Vec<char> = BYTES_TO_CHARS.to_vec();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), 256, "forward table is not a bijection");
    }

    #[test]
    fn inverse_table_has_exactly_256_entries() {
        let some_count = CHARS_TO_BYTES.iter().filter(|e| e.is_some()).count();
        assert_eq!(
            some_count, 256,
            "inverse table should carry exactly one Some per byte"
        );
    }

    #[test]
    fn known_landmarks() {
        // The canonical `Ġ` glyph — byte 0x20 (space) → U+0120.
        assert_eq!(BYTES_TO_CHARS[0x20], '\u{0120}');
        assert_eq!(BYTES_TO_CHARS[0x20], 'Ġ');
        // Byte 0x00 → U+0100 (`Ā`) — the first lifted byte.
        assert_eq!(BYTES_TO_CHARS[0x00], '\u{0100}');
        // Byte 0xAD → U+0143 (`Ń`) — the last lifted byte.
        assert_eq!(BYTES_TO_CHARS[0xAD], '\u{0143}');
        // Byte 0x7F (DEL) → U+0121 (`ġ`) — right after the printable
        // ASCII range.
        assert_eq!(BYTES_TO_CHARS[0x7F], '\u{0121}');
        // Byte 0x80 → U+0122.
        assert_eq!(BYTES_TO_CHARS[0x80], '\u{0122}');
        // Byte 0xA0 (NBSP) → U+0142 (`ł`).
        assert_eq!(BYTES_TO_CHARS[0xA0], '\u{0142}');
        // Printable ASCII passes through.
        assert_eq!(BYTES_TO_CHARS[b'A' as usize], 'A');
        assert_eq!(BYTES_TO_CHARS[b'!' as usize], '!');
        // Printable Latin-1.
        assert_eq!(BYTES_TO_CHARS[0xA1], '¡');
        assert_eq!(BYTES_TO_CHARS[0xFF], 'ÿ');
    }

    #[test]
    fn encode_hello_world_shape() {
        // The task's canonical example: a leading-space GPT-2 chunk
        // becomes a Ġ-prefixed token surface string.
        assert_eq!(encode_bytes(" Hello"), "ĠHello");
        assert_eq!(encode_bytes(" world"), "Ġworld");
        // The whole thing concatenated (no chunking here — the regex
        // pre-tokenizer would separate the two chunks elsewhere).
        assert_eq!(encode_bytes(" Hello world"), "ĠHelloĠworld");
    }

    #[test]
    fn encode_decode_round_trips_every_byte() {
        // A single string containing every byte 0..=255 in order.
        // The output must be valid UTF-8 (encode_bytes says so) and
        // decode_chars must give the original byte sequence back.
        let raw: Vec<u8> = (0u8..=255).collect();
        // Note: `raw` is not itself valid UTF-8, so we can't hand it to
        // `encode_bytes` directly. Instead, chunk into 1-byte pseudo
        // strings via unsafe... no — instead, exercise the tables
        // directly for the by-byte contract, and then round-trip a
        // UTF-8 string separately for the API-level contract.
        for &b in &raw {
            let ch = BYTES_TO_CHARS[b as usize];
            let idx = ch as usize;
            assert_eq!(CHARS_TO_BYTES[idx], Some(b));
        }
    }

    #[test]
    fn encode_decode_round_trips_utf8_strings() {
        for &s in &[
            "",
            "a",
            "hello",
            " hello",
            "Hello, world!",
            "café résumé",
            "  leading spaces",
            "trailing spaces  ",
            "line1\nline2",
            "日本語",
            "\u{1F600}",
        ] {
            let enc = encode_bytes(s);
            let dec = decode_chars(&enc);
            assert_eq!(dec, s.as_bytes(), "round-trip broke on {s:?}");
        }
    }

    #[test]
    fn encoded_string_is_valid_utf8() {
        // Encode a mix of ASCII whitespace, printable, and Latin-1
        // bytes; verify the result is a well-formed String and its
        // char count matches the input's byte count (bijection).
        let s = "Hello,\tworld!\n";
        let enc = encode_bytes(s);
        assert!(enc.is_char_boundary(0));
        assert!(enc.is_char_boundary(enc.len()));
        assert_eq!(enc.chars().count(), s.len());
    }

    #[test]
    fn decode_passthrough_for_unknown_chars() {
        // A char that is NOT in the `ByteLevel` table (a random CJK
        // code point) is passed through as its UTF-8 bytes. This is
        // the documented "forgiving decoder" behaviour.
        let out = decode_chars("日");
        assert_eq!(out, "日".as_bytes());
    }
}
