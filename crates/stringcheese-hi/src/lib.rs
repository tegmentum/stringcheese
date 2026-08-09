//! Hindi language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Hindi`] value that carries a ~130-entry Devanagari
//! stopword list, the [`LightHindiStemmer`] suffix stripper, the
//! [`HindiNormalizer`] digit-fold / nukta-strip normalizer, the
//! [`HindiTokenizer`] whitespace-and-Devanagari-punctuation word
//! splitter (danda `।` aware), and a [`HindiIast`] IAST
//! (International Alphabet of Sanskrit Transliteration)
//! Devanagari → Latin transliteration as the phonetic encoder.
//! Callers grab the singleton [`HINDI`] `const` — no construction
//! ceremony required — and delegate through the
//! [`Language`](stringcheese_lang::Language) trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-hi` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Hindi stopword table
//! or the IAST transliteration table. Callers who need Hindi add
//! `stringcheese-hi = "0.1"` to their `Cargo.toml` explicitly.
//!
//! # First Devanagari-script pack
//!
//! This is the first `stringcheese-<lang>` implementation for a
//! script written in Devanagari (U+0900..=U+097F). It exists to
//! validate the shape of the [`Language`](stringcheese_lang::Language)
//! trait on Devanagari, and to prove that StringCheese's
//! byte/character-sequence processing model handles Devanagari inputs
//! without any special-case machinery beyond the tokenizer's
//! matra-aware word rule.
//!
//! ## The Devanagari-specific invariants
//!
//! Devanagari is a **left-to-right script** (no RTL surprises). The
//! things to remember are *storage width* and the *abugida script
//! model*:
//!
//! * **Every Devanagari letter is 3 bytes in UTF-8.** The block
//!   U+0900..=U+097F falls in UTF-8's 3-byte range
//!   (U+0800..=U+FFFF), so a word like `"मैं"` (3 characters: `म` +
//!   `ै` + `ं`) is **9 bytes**. Cyrillic is 2 bytes per letter; Latin
//!   is 1. Any code that mixes byte offsets with character-boundary
//!   logic will silently corrupt token or suffix boundaries. This
//!   crate operates exclusively on `Vec<char>` and [`str::chars`]
//!   iteration — never raw byte offsets. Callers of
//!   [`Language::stem`](stringcheese_lang::Language::stem) and
//!   [`Language::tokenize`](stringcheese_lang::Language::tokenize)
//!   never see the char-vs-byte distinction because the returned
//!   [`Cow<str>`](alloc::borrow::Cow) and token slices are always
//!   valid UTF-8; downstream callers that do byte-level arithmetic
//!   on the outputs must remember the 3x expansion factor.
//! * **Independent vowels vs. dependent vowel signs.** Devanagari
//!   distinguishes two forms of every vowel: an **independent** form
//!   used at the start of a word (`अ आ इ ई उ ऊ ए ऐ ओ औ` U+0905..) and
//!   a **dependent** form (matra: `ा ि ी ु ू े ै ो ौ` U+093E..) used
//!   after a consonant. The two forms are distinct Unicode scalars;
//!   `कि` (`क` + matra `ि`) is *not* the same sequence as `कइ` (`क`
//!   + independent `इ`).
//! * **Virama and consonant clusters.** Devanagari is an **abugida**:
//!   every base consonant carries an implicit `a` (schwa) vowel
//!   unless a matra or a **virama** (`्` U+094D) overrides it. `क`
//!   alone represents `ka` (with schwa); `क्` (`क` + virama)
//!   represents bare `k`; `क्ष` (`क` + virama + `ष`) represents `kṣa`
//!   (the classical Sanskrit consonant cluster). The
//!   [`phonetic::HindiIast`] transliteration honors this by
//!   default — see [`crate::phonetic`] for the schwa-handling rules.
//! * **Nukta (`़` U+093C).** A combining mark that modifies base
//!   consonants to produce Perso-Arabic loans (`क़`, `ख़`, `ग़`, `ज़`,
//!   `ड़`, `ढ़`, `फ़`). Nukta is **semantically meaningful** — `ज`
//!   ("ja") and `ज़` ("za") are different phonemes — so the default
//!   normalizer preserves it. Callers who want casual-writing
//!   tolerance turn on
//!   [`normalize::HindiNormalizer::with_nukta_stripping`].
//! * **Devanagari digits (`०..९` U+0966..=U+096F).** Hindi text uses
//!   either the Devanagari digit block or ASCII `0..9`; the default
//!   normalizer preserves whichever form the input uses. An opt-in
//!   fold (see
//!   [`normalize::HindiNormalizer::with_devanagari_digit_folding`])
//!   collapses Devanagari to ASCII.
//! * **Danda (`।` U+0964) — the Devanagari full stop.** Sentence
//!   terminator in Hindi text; the tokenizer treats it as a
//!   separator (see [`crate::tokenizer`]).
//!
//! The design choices are otherwise:
//!
//! * **Light Hindi suffix stripper as the stemmer baseline.** There
//!   is **no canonical Snowball Hindi algorithm**. Snowball's
//!   catalogue lists Hindi as "planned" but ships no `hindi.sbl`.
//!   The Ramanathan-Rao 2003 "light" stemmer is the community
//!   reference; this pack ships a deliberately conservative subset
//!   covering gender / number markers, fused postpositions, and the
//!   most-common verb tense endings. Documented as a **starter** —
//!   a follow-up `stringcheese-hi-morph` crate would ship the full
//!   analyzer. See [`crate::stemmer`].
//! * **IAST transliteration phonetic encoder.** The scholarly
//!   Sanskrit / Hindi romanization, with long vowels marked by
//!   macrons (`ā ī ū`), retroflex consonants marked with under-dots
//!   (`ṭ ṭh ḍ ḍh ṇ`), sibilants marked with acute / dot (`ś ṣ`),
//!   and the velar / palatal nasals given their own marks (`ṅ ñ`).
//!   Base consonants carry the inherent schwa
//!   (`क` → `ka`, not just `k`); virama suppresses it. Adapter name:
//!   `"iast-hi"`. See [`crate::phonetic`].
//! * **Matra-aware tokenizer.** The default splitter walks on
//!   `char::is_alphanumeric`, which treats matras / virama /
//!   anusvara as separators and would shatter a Hindi word at every
//!   dependent vowel sign. The Hindi tokenizer extends the rule to
//!   include the full Devanagari block U+0900..=U+097F as word
//!   scalars, with the three Devanagari punctuation marks (`।` `॥`
//!   `॰`) as explicit separators. See [`crate::tokenizer`].
//! * **~130-word stopword list.** Personal / demonstrative /
//!   interrogative pronouns, postpositions, conjunctions, particles,
//!   high-frequency forms of the copula *होना* ("to be") and
//!   auxiliary *करना* ("to do"), and common adverbs. See
//!   [`stopwords`].
//! * **Devanagari-aware normalizer.** Two opt-in folds
//!   ([`HindiNormalizer::with_devanagari_digit_folding`] and
//!   [`HindiNormalizer::with_nukta_stripping`]); the default is the
//!   identity function so callers pay for exactly the folds they
//!   ask for. See [`mod@normalize`].
//! * **Default Unicode collation.** Hindi sorts under CLDR's Hindi
//!   tailoring. This pack does not carry the CLDR tailoring data;
//!   [`Language::collator`](stringcheese_lang::Language::collator)
//!   returns `None`. Callers who need locale-aware Hindi collation
//!   should reach for `icu_collator`.
//!
//! # Deferred to a follow-up wave
//!
//! * **Sanskrit pack (`stringcheese-sa`).** Sanskrit uses Devanagari
//!   (and other scripts historically) with much richer morphology
//!   than Hindi — full nominal declension, verbal conjugation, and
//!   sandhi (phonological joining across word boundaries). The IAST
//!   encoder here would be reused directly (IAST is *the* standard
//!   Sanskrit romanization); the stemmer and stopword list would be
//!   entirely different.
//! * **Marathi pack (`stringcheese-mr`).** Marathi uses Devanagari
//!   with its own additions (`ऱ ऴ`) and different morphology
//!   (agglutinative postpositions rather than Hindi's analytic
//!   compounds).
//! * **Nepali pack (`stringcheese-ne`).** Nepali also uses
//!   Devanagari with a distinct morphology and function-word
//!   inventory.
//! * **Devanagari-script variants and other Indic scripts.** Bengali
//!   (U+0980..=U+09FF), Gurmukhi (U+0A00..=U+0A7F), Gujarati
//!   (U+0A80..=U+0AFF), Oriya (U+0B00..=U+0B7F), Tamil
//!   (U+0B80..=U+0BFF), Telugu (U+0C00..=U+0C7F), Kannada
//!   (U+0C80..=U+0CFF), Malayalam (U+0D00..=U+0D7F) — every Indic
//!   script has its own abugida with the same schwa-handling
//!   shape as Devanagari; each deserves its own pack.
//! * **Full Snowball Hindi.** If a canonical `hindi.sbl` ever
//!   appears in the Snowball catalogue, ship the port under a
//!   Cargo feature (`snowball-hi`) alongside the current light
//!   stemmer.
//! * **Schwa-deletion.** The IAST encoder honors the inherent-schwa
//!   convention (Sanskrit-style). Modern colloquial Hindi drops the
//!   schwa in many word-final and some medial positions; the
//!   deletion is context-dependent and lexicon-driven — implementing
//!   it needs a Hindi lexicon and is deferred to the morphology
//!   crate.
//! * **Aksharamukha / indic-transliteration adapter.** Additional
//!   ITRANS / HK / SLP1 / ISO 15919 romanization schemes are
//!   deferred; IAST is the scholarly baseline this pack ships with.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_hi::HINDI;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(HINDI.code(), "hi");
//! assert_eq!(HINDI.name(), "Hindi");
//! assert!(HINDI.is_stopword("और"));
//! assert!(!HINDI.is_stopword("किताब"));
//! // The light stemmer strips the m. sg. marker -ा.
//! assert_eq!(HINDI.stem("लड़का"), "लड़क");
//!
//! let toks: Vec<&str> = HINDI
//!     .tokenize("मैं किताब पढ़ता हूँ।")
//!     .collect();
//! assert_eq!(toks, ["मैं", "किताब", "पढ़ता", "हूँ"]);
//! ```
//!
//! # Module map
//!
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`mod@normalize`] — the [`HindiNormalizer`] and the
//!   [`normalize()`] free function.
//! - [`stemmer`] — the [`LightHindiStemmer`] suffix stripper.
//! - [`phonetic`] — the [`HindiIast`] transliteration and the
//!   [`HindiIastAdapter`] the
//!   [`Language`](stringcheese_lang::Language) trait hands back.
//! - [`tokenizer`] — the [`HindiTokenizer`] whitespace-and-Devanagari-
//!   punctuation splitter.
//! - The [`Hindi`] type and the [`HINDI`] constant live in this
//!   crate's root.

#![cfg_attr(not(feature = "std"), no_std)]
// `deny(unsafe_code)` rather than `forbid(unsafe_code)` because the
// `stringcheese_lang::register_language!` macro expands to a linkme
// `#[distributed_slice]` static that emits `#[unsafe(link_section = "...")]`
// (Rust 2024 form) — `forbid` cannot be relaxed by inner attributes and
// would break the build. The macro emits an explicit
// `#[allow(unsafe_code)]` at the sole registration site; the rest of
// this crate is still lint-enforced no-`unsafe`. Same pattern as the
// other language packs.
#![deny(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

// Generated by `stringcheese-lang-gen` at build time from
// `rules/hi.toml`. Exposes `STOPWORDS: &[&str]` and
// `pub static CAPABILITIES: stringcheese_lang::LanguageCapabilities`.
mod generated {
    include!(concat!(env!("OUT_DIR"), "/generated.rs"));
}
pub use generated::CAPABILITIES;

#[cfg(feature = "alloc")]
pub mod normalize;
#[cfg(feature = "alloc")]
pub mod phonetic;
#[cfg(feature = "alloc")]
pub mod stemmer;
pub mod stopwords;
pub mod tokenizer;

#[cfg(all(test, feature = "std", not(target_family = "wasm")))]
mod properties;

#[cfg(feature = "alloc")]
pub use normalize::{HindiNormalizer, normalize};
#[cfg(feature = "alloc")]
pub use phonetic::{HindiIast, HindiIastAdapter};
#[cfg(feature = "alloc")]
pub use stemmer::LightHindiStemmer;
pub use stopwords::STOPWORDS;
pub use tokenizer::HindiTokenizer;

// -----------------------------------------------------------------------
// The Hindi language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::HindiIastAdapter;
    use crate::stemmer::LightHindiStemmer;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::HindiTokenizer;

    /// The Hindi language pack.
    ///
    /// Zero-sized; construct as [`Hindi`] and reuse the value freely
    /// across threads and calls, or grab the crate-level
    /// [`HINDI`](crate::HINDI) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Hindi;

    /// The static [`HindiIastAdapter`] [`Hindi`] hands back from
    /// [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static HINDI_IAST: HindiIastAdapter = HindiIastAdapter;

    impl Language for Hindi {
        fn code(&self) -> &'static str {
            "hi"
        }

        fn name(&self) -> &'static str {
            "Hindi"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            LightHindiStemmer.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(HindiTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&HINDI_IAST)
        }
    }

    /// The singleton [`Hindi`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Hindi`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other `stringcheese-<lang>`
    /// pack follows.
    pub const HINDI: Hindi = Hindi;
}

#[cfg(feature = "alloc")]
pub use pack::{HINDI, Hindi};

// Register into `stringcheese-lang::registry` so callers who look up
// languages dynamically (`registry::language("hi")`) find this pack.
// alloc-gated because the pack constant itself is alloc-gated.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(HINDI);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-hi` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
