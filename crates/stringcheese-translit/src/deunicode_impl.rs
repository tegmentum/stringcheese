//! [`deunicode`]-backed general-purpose transliterator.

use alloc::string::{String, ToString};

use crate::Transliterator;

/// General-purpose transliterator: every non-ASCII scalar gets a
/// deunicode-approximate ASCII rendering.
///
/// Cheap to construct (stateless); the deunicode table is a
/// static.
#[derive(Copy, Clone, Debug, Default)]
pub struct DeunicodeTransliterator;

impl DeunicodeTransliterator {
    /// Construct.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Transliterator for DeunicodeTransliterator {
    /// Transliterate `input` to an ASCII approximation.
    ///
    /// ## Fast path
    ///
    /// Bytes strictly `< 0x7F` are identity under `deunicode`:
    /// every printable-ASCII / C0-control scalar maps to itself
    /// in the substitution table (`\x7F` DEL is the one
    /// exception — deunicode's table drops it, matching its own
    /// internal "skip ASCII prefix" cutoff of `c < 0x7F`; see
    /// `deunicode-1.6.2/src/lib.rs` line 108). Gate on that same
    /// condition and, when it holds, clone the input directly and
    /// skip the per-scalar substitution walk.
    ///
    /// The gate is a two-step scan:
    ///
    /// 1. [`str::is_ascii`] first — SIMD-accelerated in std,
    ///    short-circuits on the first byte `>= 0x80`. Bounds the
    ///    cost on non-ASCII input to the position of the first
    ///    non-ASCII byte (a few bytes for prose starting with an
    ///    accented word, a handful for CJK-first content).
    /// 2. A `contains(&0x7F)` scan to catch the DEL edge case.
    ///    On any input that reaches this step (already known
    ///    ASCII), DEL is essentially never present in real text,
    ///    so the scan burns through the buffer at memory
    ///    bandwidth and returns `false`.
    ///
    /// Mixed and pure non-ASCII inputs fall through to
    /// [`deunicode::deunicode`] unchanged. Same pattern as
    /// `stringcheese-stats::Ratios::of` (see commit `59cb9d4`).
    fn transliterate(&self, input: &str) -> String {
        let bytes = input.as_bytes();
        if bytes.is_ascii() && !bytes.contains(&0x7F) {
            return input.to_string();
        }
        deunicode::deunicode(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_passes_through() {
        let t = DeunicodeTransliterator::new();
        assert_eq!(t.transliterate("hello"), "hello");
    }

    #[test]
    fn accents_stripped() {
        let t = DeunicodeTransliterator::new();
        assert_eq!(t.transliterate("Café résumé"), "Cafe resume");
    }

    #[test]
    fn cjk_gets_a_rendering() {
        // deunicode gives some transliteration for every scalar;
        // the specific form isn't part of our contract, but the
        // output must be non-empty ASCII.
        let t = DeunicodeTransliterator::new();
        let out = t.transliterate("日本語");
        assert!(!out.is_empty());
        assert!(out.is_ascii());
    }

    /// Differential test: on every input the fast-path result
    /// must equal a direct [`deunicode::deunicode`] call.
    /// Guards the invariant the fast-path gate relies on — that
    /// deunicode is the identity on scalars `< 0x7F`, so
    /// bypassing the substitution walk on inputs that contain
    /// only those bytes cannot change the observable result.
    ///
    /// The full ASCII sweep also covers the `\x7F` edge case:
    /// deunicode's table maps DEL to the empty string, and this
    /// test enforces the fast-path falls back to deunicode when
    /// any DEL byte is present (rather than silently passing it
    /// through).
    #[test]
    fn ascii_fast_path_matches_deunicode() {
        // Every ASCII byte enumerated once — covers control
        // codes, printables, punctuation, digits, letters, and
        // the DEL (0x7F) edge case together.
        let all_ascii: String = (0u8..128).map(|b| b as char).collect();
        assert_eq!(
            DeunicodeTransliterator::new().transliterate(&all_ascii),
            deunicode::deunicode(&all_ascii),
            "fast path diverged from deunicode on the full ASCII sweep"
        );

        // Representative prose / mixed-class inputs in addition
        // to the exhaustive sweep — cheap belt & braces. The
        // last two entries exercise the DEL fallback and the
        // non-ASCII fallback respectively so both `bail-out`
        // branches are covered.
        for s in [
            "",
            "hello",
            "hi, world",
            "12345",
            "a\x07b",
            "The quick brown fox jumps over the lazy dog.\n",
            "\t\r\n ",
            "path/to/file.rs",
            "before\x7Fafter",
            "Café résumé",
        ] {
            assert_eq!(
                DeunicodeTransliterator::new().transliterate(s),
                deunicode::deunicode(s),
                "fast path diverged on {s:?}"
            );
        }
    }
}
