//! Thai language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Thai`] value that carries a ~55-entry Thai stopword
//! list, the [`ThaiTokenizer`] syllable-cluster tokenizer, the
//! [`ThaiStemmer`] near-identity stemmer (Thai is analytic — the
//! stemmer only strips a small closed set of nominalizer / agent
//! prefixes and folds word-level reduplication), and a PHONEX-Thai
//! ([`ThaiPhonex`]) phonetic encoder computed over a Royal-Thai-
//! Romanization-family consonant fold. Callers grab the singleton
//! [`THAI`] `const` — no construction ceremony required — and
//! delegate through the [`Language`](stringcheese_lang::Language)
//! trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-th` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Thai stopword table or
//! the Thai script's classification cascade. Callers who need Thai
//! add `stringcheese-th = "0.1"` to their `Cargo.toml` explicitly.
//!
//! # First Thai-script pack
//!
//! Thai is the **first Thai-script pack** in StringCheese. Its
//! purpose is to validate the
//! [`Language`](stringcheese_lang::Language) trait's shape on a
//! script that:
//!
//! * Encodes every scalar as **3 UTF-8 bytes** (the Thai block
//!   U+0E00..=U+0E7F falls in UTF-8's 3-byte range U+0800..=U+FFFF).
//!   All tokenizer / stemmer / phonex arithmetic runs on `Vec<char>`
//!   or [`str::chars`] iteration — never raw byte offsets — because
//!   byte arithmetic would silently corrupt scalar boundaries.
//! * Is written **without spaces between words**, like Chinese and
//!   Japanese. Space *is* used in Thai typography, but as a
//!   phrase / sentence separator, not a word separator. This is
//!   the central complication: a word tokenizer for Thai needs a
//!   dictionary and typically a statistical or neural model
//!   (ICU's Thai break iterator, `PyThaiNLP`'s `newmm`, `attacut`,
//!   `deepcut`). The base pack ships **syllable-cluster
//!   segmentation** instead — deterministic, dictionary-free, and
//!   dramatically coarser than word-level — see
//!   [`crate::tokenizer`] for the exact rules.
//! * Marks vowels with pre-vowels (typed **before** the consonant
//!   they modify: `เ แ โ ใ ไ` U+0E40..=U+0E44), above / below /
//!   after vowel marks, and tone marks that combine with the
//!   preceding cluster. The tokenizer models this so that a cluster
//!   like `เป็น` is a single unit (pre-vowel + consonant + vowel
//!   mark), not five separate "characters" as a naive splitter
//!   might treat them.
//! * Has **no case**, **no plural inflection**, and **no verb tense
//!   inflection** — morphology is extremely simple, closer to
//!   Chinese than Sanskrit. The stemmer is therefore nearly
//!   identity; the deferred cases are documented at
//!   [`crate::stemmer`].
//!
//! # Design choices
//!
//! * **Syllable-cluster tokenizer (no dictionary).** The base
//!   [`ThaiTokenizer`] emits Thai syllable clusters (a leading
//!   consonant, optionally preceded by a pre-vowel, followed by
//!   any combining vowel / tone / sign marks). This is coarser
//!   than a dictionary-driven word segmenter: `ไทย` "Thai" is
//!   split into two clusters (`ไท` and `ย`) under the naive rule,
//!   because a dictionary is needed to see that all three scalars
//!   belong to the same word. Full word segmentation is
//!   **deferred** — a future `stringcheese-th-newmm` (or
//!   `stringcheese-th-dict`) sibling would take that role while
//!   the base pack stays wasm-friendly. See [`crate::tokenizer`].
//! * **Near-identity stemmer.** Thai is analytic (no case, no
//!   number, no gender, no tense inflection), so the stemmer is
//!   essentially identity. Two narrow rules do fire:
//!   (a) strip the nominalizer / agent prefixes `การ- ความ- ผู้-
//!   นัก- เครื่อง-` where the residual stem has ≥ 2 scalars, and
//!   (b) fold exact word-level reduplication (`XX → X`), which
//!   Thai uses for plural / intensity. Everything else passes
//!   through. See [`crate::stemmer`].
//! * **~55-entry stopword list.** Personal / demonstrative /
//!   interrogative pronouns, conjunctions, prepositions,
//!   copula / auxiliary / negator / TAM markers, motion light
//!   verbs, sentence-final particles, and high-frequency
//!   quantifiers. See [`stopwords`].
//! * **PHONEX-Thai phonetic hookup.** A 4-character Soundex-shape
//!   key computed over a Royal-Thai-Romanization-family consonant
//!   fold: tone marks and vowel marks drop, each Thai consonant
//!   maps to an ASCII family letter (all velars `ก ข ฃ ค ฅ ฆ → K`;
//!   all sibilants `ซ ศ ษ ส → S`; all labials `ผ พ ภ ป → P`; etc.),
//!   then the standard Soundex-shape 4-char reduction runs over
//!   the folded Latin. Adapter name: `"phonex-th"`. See
//!   [`crate::phonetic`].
//! * **Default Unicode collation.** Thai sorts under CLDR's Thai
//!   tailoring; this pack does not carry the CLDR tailoring data.
//!   [`Language::collator`](stringcheese_lang::Language::collator)
//!   returns `None`. Callers who need locale-aware Thai collation
//!   should reach for `icu_collator`.
//!
//! # Deferred to a follow-up wave
//!
//! * **True word segmentation.** Requires a dictionary and
//!   typically a statistical / neural model (see the tokenizer
//!   module docs for the state of the art). A future
//!   `stringcheese-th-newmm` sibling would ship the ICU-flavoured
//!   or PyThaiNLP-flavoured segmenter.
//! * **Sara-am decomposition and complex non-spacing marks.**
//!   Unicode's UAX #29 Thai grapheme-cluster boundary has
//!   additional rules for `ำ` U+0E33 (sara am, historically
//!   decomposable to `nikhahit + sara aa`) and other historical
//!   compositions. The base tokenizer approximates the subset
//!   that dominates modern Thai; a caller who needs UAX #29
//!   compliance should compose a grapheme segmenter over the
//!   tokens.
//! * **Tone preservation in the phonetic encoder.** Thai has five
//!   phonemic tones; PHONEX collapses across tone by design (a
//!   phonetic key is an equivalence-class representative). A
//!   tone-preserving encoder is a natural sibling.
//! * **RTGS-faithful romanization** (`stringcheese-th-rtgs`) — a
//!   readable Latin transcription rather than a phonetic key.
//! * **ISO 11940 transliteration.** The scholarly / library
//!   one-to-one Thai-script → Latin-with-diacritics transliteration.
//! * **Lanna (Northern Thai), Isan, and Southern-Thai script
//!   variants.** All deferred.
//! * **Khmer, Lao, Burmese siblings.** Southeast Asian scripts
//!   that share the Brahmic-family cluster model but have their
//!   own vocabularies and phonologies. All deferred.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_th::THAI;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(THAI.code(), "th");
//! assert_eq!(THAI.name(), "Thai");
//! assert!(THAI.is_stopword("และ"));
//! assert!(THAI.is_stopword("ไม่"));
//! assert!(!THAI.is_stopword("ประเทศ"));
//!
//! // The stemmer strips the nominalizer prefix การ-.
//! assert_eq!(THAI.stem("การเรียน"), "เรียน");
//! // Content words pass through unchanged.
//! assert_eq!(THAI.stem("สวัสดี"), "สวัสดี");
//!
//! // Syllable-cluster tokenization — coarser than word-level.
//! let toks: Vec<&str> = THAI.tokenize("hello ไทย 2026").collect();
//! assert_eq!(toks, ["hello", "ไท", "ย", "2026"]);
//! ```
//!
//! # Module map
//!
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`tokenizer`] — the [`ThaiTokenizer`] syllable-cluster
//!   splitter, plus the [`CharType`] enum and the [`classify`]
//!   classification function.
//! - [`stemmer`] — the [`ThaiStemmer`] near-identity stemmer.
//! - [`phonetic`] — the [`ThaiPhonex`] PHONEX-Thai encoder and its
//!   [`ThaiPhonexAdapter`] — the type
//!   [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
//!   hands back.
//! - The [`Thai`] type and the [`THAI`] constant live in this
//!   crate's root.

#![cfg_attr(not(feature = "std"), no_std)]
// `deny(unsafe_code)` rather than `forbid(unsafe_code)` because the
// `stringcheese_lang::register_language!` macro expands to a linkme
// `#[distributed_slice]` static that emits `#[unsafe(link_section =
// "...")]` (Rust 2024 form) — `forbid` cannot be relaxed by inner
// attributes and would break the build. The macro emits an explicit
// `#[allow(unsafe_code)]` at the sole registration site; the rest of
// this crate is still lint-enforced no-`unsafe`. Same pattern as the
// other language packs.
#![deny(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
pub mod phonetic;
#[cfg(feature = "alloc")]
pub mod stemmer;
pub mod stopwords;
pub mod tokenizer;

#[cfg(all(test, feature = "std", not(target_family = "wasm")))]
mod properties;

#[cfg(feature = "alloc")]
pub use phonetic::{ThaiPhonex, ThaiPhonexAdapter, consonant_family};
#[cfg(feature = "alloc")]
pub use stemmer::ThaiStemmer;
pub use stopwords::STOPWORDS;
pub use tokenizer::{CharType, ThaiTokenizer, ThaiTokens, classify};

// -----------------------------------------------------------------------
// The Thai language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::ThaiPhonexAdapter;
    use crate::stemmer::ThaiStemmer;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::ThaiTokenizer;

    /// The Thai language pack.
    ///
    /// Zero-sized; construct as [`Thai`] and reuse the value freely
    /// across threads and calls, or grab the crate-level
    /// [`THAI`](crate::THAI) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices (syllable-cluster tokenization, near-identity
    /// stemmer, PHONEX-Thai phonetic key) and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Thai;

    /// The static [`ThaiPhonexAdapter`] [`Thai`] hands back from
    /// [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static THAI_PHONEX: ThaiPhonexAdapter = ThaiPhonexAdapter;

    impl Language for Thai {
        fn code(&self) -> &'static str {
            "th"
        }

        fn name(&self) -> &'static str {
            "Thai"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            ThaiStemmer.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(ThaiTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&THAI_PHONEX)
        }
    }

    /// The singleton [`Thai`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Thai`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other
    /// `stringcheese-<lang>` pack follows.
    pub const THAI: Thai = Thai;
}

#[cfg(feature = "alloc")]
pub use pack::{THAI, Thai};

// Register into `stringcheese-lang::registry` so callers who look up
// languages dynamically (`registry::language("th")`) find this pack.
// alloc-gated because the pack constant itself is alloc-gated.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(THAI);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-th` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
