//! [`LanguagePhoneticEncoder`] — object-safe phonetic-encoder adapter.
//!
//! [`stringcheese_phonetic::PhoneticEncoder`] carries an associated
//! [`Key`](stringcheese_phonetic::PhoneticEncoder::Key) type so single-key
//! algorithms (Soundex, NYSIIS) can return `String` while multi-key
//! algorithms (Double Metaphone) can return a small struct with the
//! primary and alternate keys. That flexibility is exactly what the
//! phonetic crate needs — and exactly what makes the trait
//! non-object-safe.
//!
//! The [`Language`](crate::Language) trait, however, has to hand back
//! a phonetic encoder through a trait object because a `dyn Language`
//! must not care whether the underlying algorithm produces one key or
//! two. This module defines a small **object-safe** wrapper trait
//! whose `encode` method normalizes any concrete phonetic encoder into
//! a `(primary, Option<alternate>)` return type; the [`SoundexAdapter`]
//! and [`NysiisAdapter`] wrappers demonstrate how a `stringcheese-<lang>`
//! pack plugs a phonetic-crate encoder into the language trait's
//! surface.
//!
//! # Deviation from the sketched trait
//!
//! The design sketch for [`Language::phonetic_encoder`](crate::Language::phonetic_encoder)
//! showed the return type as `Option<&dyn PhoneticEncoder>`. That
//! signature is not compilable — [`stringcheese_phonetic::PhoneticEncoder`]
//! carries an associated type, so `dyn PhoneticEncoder` is only well-formed
//! when the type is pinned (`dyn PhoneticEncoder<Key = String>`), and
//! pinning it eliminates the polymorphism the sketch wanted in the
//! first place. Using this crate's [`LanguagePhoneticEncoder`] wrapper
//! preserves the polymorphism while remaining object-safe.

use alloc::string::String;

use stringcheese_phonetic::{
    DoubleMetaphone, DoubleMetaphoneKey, Nysiis, PhoneticEncoder, Soundex,
};

/// Object-safe phonetic-encoder surface exposed through a
/// [`Language`](crate::Language) trait object.
///
/// Every implementation normalizes its underlying algorithm's output
/// into a `(primary, Option<alternate>)` pair. A single-key algorithm
/// (Soundex, NYSIIS) always yields `Some((key, None))`; a multi-key
/// algorithm (Double Metaphone) yields `Some((primary, Some(alt)))`
/// when the alternate differs from the primary. Any encoder returns
/// `None` when the input has no usable phonetic key (for example,
/// input with no ASCII-letter content under an ASCII-only algorithm).
///
/// Language packs build one of these by wrapping a
/// [`stringcheese_phonetic`] encoder: see [`SoundexAdapter`],
/// [`NysiisAdapter`], and [`DoubleMetaphoneAdapter`].
pub trait LanguagePhoneticEncoder: Send + Sync {
    /// Encodes `word` into its phonetic key(s).
    ///
    /// Returns `Some((primary, alternate))` on success; `alternate`
    /// is `None` for single-key algorithms. Returns `None` if the
    /// algorithm cannot produce a key (empty input, no letters, etc.).
    fn encode(&self, word: &str) -> Option<(String, Option<String>)>;

    /// A short, human-readable name for the underlying algorithm
    /// (`"soundex"`, `"nysiis"`, `"double-metaphone"`, …).
    ///
    /// Used only for diagnostics — the authoritative identifier is the
    /// underlying encoder's
    /// [`AlgorithmDescriptor`](stringcheese_core::AlgorithmDescriptor).
    fn name(&self) -> &'static str;
}

/// Adapter that exposes [`stringcheese_phonetic::Soundex`] through the
/// [`LanguagePhoneticEncoder`] trait.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SoundexAdapter;

impl LanguagePhoneticEncoder for SoundexAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        let key = Soundex.encode(word);
        if key.is_empty() {
            None
        } else {
            Some((key, None))
        }
    }

    fn name(&self) -> &'static str {
        "soundex"
    }
}

/// Adapter that exposes [`stringcheese_phonetic::Nysiis`] through the
/// [`LanguagePhoneticEncoder`] trait.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct NysiisAdapter;

impl LanguagePhoneticEncoder for NysiisAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        let key = Nysiis.encode(word);
        if key.is_empty() {
            None
        } else {
            Some((key, None))
        }
    }

    fn name(&self) -> &'static str {
        "nysiis"
    }
}

/// Adapter that exposes [`stringcheese_phonetic::DoubleMetaphone`]
/// through the [`LanguagePhoneticEncoder`] trait.
///
/// The adapter carries the encoder value (Double Metaphone has a
/// `PrimaryOnly` / `Full` variant on the encoder itself) so callers
/// can pick the variant they want.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct DoubleMetaphoneAdapter(pub DoubleMetaphone);

impl LanguagePhoneticEncoder for DoubleMetaphoneAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        let DoubleMetaphoneKey { primary, alternate } = self.0.encode(word);
        if primary.is_empty() {
            None
        } else {
            // Suppress an alternate that equals the primary (or is
            // empty) — the return-type contract says "alternate that
            // differs from the primary".
            let alt = alternate.filter(|a| !a.is_empty() && *a != primary);
            Some((primary, alt))
        }
    }

    fn name(&self) -> &'static str {
        "double-metaphone"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soundex_adapter_encodes_smith() {
        let sx = SoundexAdapter;
        assert_eq!(
            sx.encode("SMITH"),
            Some((String::from("S530"), None)),
            "Soundex(SMITH) should be S530 with no alternate"
        );
    }

    #[test]
    fn soundex_adapter_returns_none_for_empty() {
        let sx = SoundexAdapter;
        assert_eq!(sx.encode(""), None);
        assert_eq!(sx.encode("---"), None);
    }

    #[test]
    fn nysiis_adapter_returns_some() {
        let n = NysiisAdapter;
        // Just check the shape; NYSIIS reference values live in the
        // phonetic crate's own test suite.
        let out = n.encode("SMITH");
        assert!(out.is_some());
        let (primary, alt) = out.unwrap();
        assert!(!primary.is_empty());
        assert!(alt.is_none());
    }

    #[test]
    fn adapters_expose_a_name() {
        assert_eq!(SoundexAdapter.name(), "soundex");
        assert_eq!(NysiisAdapter.name(), "nysiis");
    }
}
