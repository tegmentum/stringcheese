//! Amharic language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Amharic`] value that carries a ~55-entry Ge'ez-script
//! stopword list, the [`LightAmharicStemmer`] rule-based suffix
//! stripper (definite article, plural, possessive, and object
//! suffixes), the [`AmharicTokenizer`] Ge'ez-aware whitespace-and-
//! punctuation word splitter, the [`crate::geez`] Ge'ez syllable
//! decompose/compose helpers, and a two-stage [`AmharicPhonex`]
//! (Ge'ez → BGN/PCGN-style Latin → Soundex-shape 4-char reduction)
//! phonetic key as the phonetic encoder. Callers grab the singleton
//! [`AMHARIC`] `const` — no construction ceremony required — and
//! delegate through the
//! [`Language`](stringcheese_lang::Language) trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-am` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Amharic stopword table
//! or the Ge'ez syllable-decomposition math. Callers who need
//! Amharic add `stringcheese-am = "0.1"` to their `Cargo.toml`
//! explicitly.
//!
//! # First Ge'ez-script pack, Semitic sibling of Arabic and Hebrew
//!
//! Amharic is:
//!
//! * The **first Ge'ez-script pack** in StringCheese. Ge'ez (also
//!   called *Ethiopic*) is an **abugida / syllabary** — each scalar
//!   represents a **consonant + vowel** combination. Unlike
//!   Devanagari or Bengali, where a base consonant plus a
//!   dependent-vowel matra form *two* scalars, Ge'ez packs each
//!   consonant + vowel *pair* into **one** scalar. So `ኪ` (Amharic
//!   "ki") is a single scalar, not two.
//! * The **first Ethiopian / Eritrean language pack**. Amharic is
//!   the working language of Ethiopia. Sibling packs for Tigrinya
//!   (`ti`), Tigre (`tig`), and liturgical Ge'ez (`gez`) are
//!   deferred to follow-up waves.
//! * A **Semitic sibling of Arabic (`stringcheese-ar`) and Hebrew
//!   (`stringcheese-he`)**. All three share the root-and-pattern
//!   morphology model — 3- or 4-consonant roots interleaved with
//!   vowel patterns — but write in three different scripts (Arabic
//!   abjad, Hebrew abjad, Ge'ez abugida). This pack, like its
//!   Semitic siblings, ships a *light* stemmer that peels off the
//!   productive surface morphology (definite article, plural,
//!   possessive clitics, object clitics); full root-and-pattern
//!   morphological analysis is deferred to a downstream
//!   `stringcheese-am-morph` crate.
//!
//! ## Ge'ez / Ethiopic script layout
//!
//! The **main Ge'ez block** occupies **U+1200..=U+137F** — 384
//! scalars laid out as **48 rows × 8 columns**. Each row is a
//! *consonant family*; each column is a *vowel order*:
//!
//! | Column | Amharic name | Vowel   | Example (from ሀ = h family) |
//! |--------|--------------|---------|-----------------------------|
//! | 0      | ግዕዝ         | ə / ä   | ሀ  (hä)                     |
//! | 1      | ካዕብ         | u       | ሁ  (hu)                     |
//! | 2      | ሣልስ         | i       | ሂ  (hi)                     |
//! | 3      | ራብዕ         | a       | ሃ  (ha)                     |
//! | 4      | ሓምስ         | e       | ሄ  (he)                     |
//! | 5      | ሳድስ         | ɨ / ∅   | ህ  (hɨ — often no vowel)    |
//! | 6      | ሳብዕ         | o       | ሆ  (ho)                     |
//! | 7      | (labialized) | wa / oa | ሇ  (variable / often absent) |
//!
//! Column 5 (ሳድስ) is the **6th order** — historically a short high
//! central vowel /ɨ/ that in Amharic frequently reduces to no vowel
//! at all. Column 7 (labialized) is present for some consonant
//! families and reserved / absent for others.
//!
//! The [`crate::geez`] module ships the arithmetic:
//! [`decompose`](crate::geez::decompose) turns any main-block scalar
//! into its `(family_head, order)` pair, and
//! [`compose`](crate::geez::compose) is the inverse. These
//! primitives are used by the phonetic encoder and are exposed as
//! public API in their own right.
//!
//! ## Ge'ez punctuation
//!
//! Amharic uses:
//!
//! * **`፡` (U+1361 ETHIOPIC WORDSPACE)** — the traditional Ge'ez
//!   word separator (used like ASCII space).
//! * **`።` (U+1362 ETHIOPIC FULL STOP)** — the sentence terminator.
//! * **`፣ ፤ ፥ ፦ ፧ ፨`** — comma, semicolon, colon, preface colon,
//!   question mark, paragraph separator (U+1363..=U+1368).
//!
//! The [`AmharicTokenizer`] treats all of these as separators;
//! every Ge'ez syllable stays word-internal.
//!
//! ## Design decisions
//!
//! * **Byte-length is 3 for every Ge'ez main-block scalar.** The
//!   block U+1200..=U+137F falls in UTF-8's 3-byte range
//!   (U+0800..=U+FFFF). This pack, like the Bengali and Devanagari
//!   packs, walks `Vec<char>` for all suffix and stemmer arithmetic
//!   — byte offsets would silently corrupt 3-byte scalars.
//! * **Rule-based Amharic suffix stripper.** Amharic has a
//!   productive surface morphology on top of the root-and-pattern
//!   Semitic core — definite article, plural, possessive clitics,
//!   object clitics. The [`LightAmharicStemmer`] iterates to
//!   convergence over a suffix table with a 2-scalar min-stem
//!   guard, stripping stacked suffixes deterministically. Full
//!   root-and-pattern reversal is deferred; the light stemmer is
//!   the well-established IR baseline.
//! * **Ge'ez → BGN/PCGN-style Latin → Soundex-shape 4-char
//!   PHONEX-Amharic phonetic hookup.** The two-stage design
//!   ([`AmharicBgnPcgn`] then [`AmharicPhonex`]) matches the shape
//!   of the Bengali / Hindi / Tamil Indic packs. Adapter name:
//!   `"phonex-am"`.
//! * **~55-word stopword list.** Personal / demonstrative /
//!   interrogative pronouns, prepositions, conjunctions, negation
//!   particles, high-frequency copula surface forms, and common
//!   adverbs.
//! * **Default Unicode collation.** Ge'ez sorts under CLDR's
//!   Ethiopic tailoring. This pack does not carry the CLDR
//!   tailoring data;
//!   [`Language::collator`](stringcheese_lang::Language::collator)
//!   returns `None`. Callers who need locale-aware Amharic
//!   collation should reach for `icu_collator`.
//!
//! # Deferred to a follow-up wave
//!
//! * **Tigrinya (`stringcheese-ti`), Tigre (`stringcheese-tig`),
//!   Ge'ez liturgical (`stringcheese-gez`).** All three use the
//!   Ge'ez script but have distinct vocabularies, function-word
//!   inventories, and morphology.
//! * **Root-and-pattern morphological analysis.** Extracting the
//!   3- or 4-consonant Semitic root needs template matching plus a
//!   lexicon (`HornMorpho`, `AmMorpho`). Deferred to a downstream
//!   `stringcheese-am-morph` crate.
//! * **Verb morphology.** Amharic verbs carry subject prefixes,
//!   object suffixes, tense / aspect / mood markers, and the
//!   root-and-pattern stem alternations. This light stemmer strips
//!   the *nominal* surface morphology only.
//! * **ISO 9985 alternate romanization.** The pack ships a
//!   BGN/PCGN-style ASCII transliteration; an ISO 9985 adapter
//!   (with proper diacritics `ḥ ḫ ṣ š ṭ`) would be a useful
//!   follow-up alongside the current PHONEX encoder.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_am::AMHARIC;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(AMHARIC.code(), "am");
//! assert_eq!(AMHARIC.name(), "Amharic");
//! assert!(AMHARIC.is_stopword("እና"));
//! assert!(!AMHARIC.is_stopword("አማርኛ"));
//! // The light stemmer strips the plural marker -ኦች.
//! assert_eq!(AMHARIC.stem("ልጅኦች"), "ልጅ");
//!
//! let toks: Vec<&str> = AMHARIC
//!     .tokenize("እኔ አማርኛ እወዳለሁ።")
//!     .collect();
//! assert_eq!(toks, ["እኔ", "አማርኛ", "እወዳለሁ"]);
//! ```
//!
//! # Module map
//!
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`stemmer`] — the [`LightAmharicStemmer`] rule-based suffix
//!   stripper.
//! - [`phonetic`] — the [`AmharicBgnPcgn`] transliteration and the
//!   [`AmharicPhonex`] reduction; [`AmharicPhonexAdapter`] is the
//!   type
//!   [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
//!   hands back.
//! - [`tokenizer`] — the [`AmharicTokenizer`] whitespace-and-Ge'ez-
//!   punctuation splitter.
//! - [`geez`] — the Ge'ez syllable [`decompose`](geez::decompose) /
//!   [`compose`](geez::compose) helpers.
//! - The [`Amharic`] type and the [`AMHARIC`] constant live in this
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

pub mod geez;
#[cfg(feature = "alloc")]
pub mod phonetic;
#[cfg(feature = "alloc")]
pub mod stemmer;
pub mod stopwords;
pub mod tokenizer;

#[cfg(all(test, feature = "std", not(target_family = "wasm")))]
mod properties;

#[cfg(feature = "alloc")]
pub use phonetic::{AmharicBgnPcgn, AmharicPhonex, AmharicPhonexAdapter};
#[cfg(feature = "alloc")]
pub use stemmer::LightAmharicStemmer;
pub use stopwords::STOPWORDS;
pub use tokenizer::AmharicTokenizer;

// -----------------------------------------------------------------------
// The Amharic language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::AmharicPhonexAdapter;
    use crate::stemmer::LightAmharicStemmer;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::AmharicTokenizer;

    /// The Amharic language pack.
    ///
    /// Zero-sized; construct as [`Amharic`] and reuse the value
    /// freely across threads and calls, or grab the crate-level
    /// [`AMHARIC`](crate::AMHARIC) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Amharic;

    /// The static [`AmharicPhonexAdapter`] [`Amharic`] hands back
    /// from [`phonetic_encoder`](Language::phonetic_encoder).
    static AMHARIC_PHONEX: AmharicPhonexAdapter = AmharicPhonexAdapter;

    impl Language for Amharic {
        fn code(&self) -> &'static str {
            "am"
        }

        fn name(&self) -> &'static str {
            "Amharic"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            LightAmharicStemmer.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(AmharicTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&AMHARIC_PHONEX)
        }
    }

    /// The singleton [`Amharic`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Amharic`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other
    /// `stringcheese-<lang>` pack follows.
    pub const AMHARIC: Amharic = Amharic;
}

#[cfg(feature = "alloc")]
pub use pack::{AMHARIC, Amharic};

// Register into `stringcheese-lang::registry` so callers who look up
// languages dynamically (`registry::language("am")`) find this pack.
// alloc-gated because the pack constant itself is alloc-gated.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(AMHARIC);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-am` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
