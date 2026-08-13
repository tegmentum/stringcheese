//! Japanese language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Japanese`] value that carries a ~120-entry Japanese
//! stopword list, the [`JapaneseTokenizer`] character-type-based
//! tokenizer, the [`JapaneseStemmer`] polite-and-plural simplifier, and
//! a [`KunreiRomaji`] (ISO 3602) romanization as the phonetic encoder.
//! Callers grab the singleton [`JAPANESE`] `const` — no construction
//! ceremony required — and delegate through the
//! [`Language`](stringcheese_lang::Language) trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-ja` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Japanese stopword table
//! or the character-classification cascade. Callers who need Japanese
//! add `stringcheese-ja = "0.1"` to their `Cargo.toml` explicitly.
//!
//! # First non-Latin-script pack
//!
//! This is the first `stringcheese-<lang>` implementation for a script
//! without word-boundary spaces. Its purpose is to validate the
//! [`Language`](stringcheese_lang::Language) trait's shape on that
//! script while making three deliberate concessions to the offline-first
//! commitment:
//!
//! * **No dictionary.** The gold standard for Japanese tokenization is
//!   dictionary-driven morphological analysis (`MeCab`, kuromoji, sudachi,
//!   vibrato — all ship `IPADic`-scale lexica, ~100 MB on disk). Shipping
//!   a 100 MB static asset defeats the whole point of a wasm-first
//!   language pack. Instead, this crate ships a
//!   **character-type-based tokenizer** that splits at every
//!   script/type transition, with one exception: a Kanji run absorbs
//!   the trailing Hiragana okurigana of a verb or adjective stem. See
//!   [`JapaneseTokenizer`] for the details.
//! * **Kunrei-shiki over Hepburn.** The pack's phonetic encoder is
//!   Kunrei-shiki (訓令式, ISO 3602). It differs from Hepburn on the
//!   shibboleth moras (`し → si` not `shi`, `つ → tu` not `tsu`,
//!   `じゃ → zya` not `ja`); Kunrei's consonant regularity makes the
//!   output a better *equivalence-class key* for lookup, which is what
//!   a phonetic encoder wants. Hepburn is a legitimate alternate and
//!   is called out as a follow-up in the roadmap.
//! * **Minimal stemmer.** Japanese doesn't have inflection-heavy
//!   morphology like English or French — verb conjugation is
//!   paradigm-driven and requires a morphological analyzer to strip
//!   correctly. The shipped [`JapaneseStemmer`] is a **token
//!   simplifier**, not a true stemmer: it strips the plural markers
//!   `-たち` / `-達` / `-ら` / `-ども` and the polite auxiliaries
//!   `-ます` / `-です` (with their past and negative forms). Nothing
//!   more. See [`stemmer`] for the (deliberately short) rule set.
//!
//! # Alternate encoders and normalization utilities
//!
//! * **Hepburn romanization.** A parallel encoder that emits the
//!   English-facing spellings (`shi`, `chi`, `tsu`, `fu`, `ja`, macron
//!   long vowels, …). See [`hepburn`] for the implementation and
//!   [`JAPANESE_WITH_HEPBURN`] for the [`Language`](stringcheese_lang::Language)-adapted
//!   pack variant.
//! * **Kana normalization.** Half-width katakana widening, dakuten
//!   canonicalization, katakana ↔ hiragana folding, and full-width
//!   ASCII → half-width folding, composed by the [`KanaNormalizer`]
//!   builder. See [`normalize`].
//!
//! # Deferred to a follow-up wave
//!
//! * **Word-boundary macrons.** The Hepburn encoder combines every
//!   consecutive `おう` / `おお` / `うう` / `ええ` / `ああ` into a
//!   macron long vowel, but at a morpheme boundary the two vowels
//!   should stay separate. Detecting the boundary needs a dictionary.
//! * **Nihon-shiki.** A third romanization sitting between Kunrei and
//!   the traditional strict form. Not shipped.
//! * **Kuromoji-lite morphological tokenizer.** A crate-external
//!   sibling (`stringcheese-ja-morph`?) with a dictionary loader would
//!   let a caller opt into gold-standard tokenization while the base
//!   `stringcheese-ja` crate stays wasm-friendly.
//! * **Verb-conjugation stripping.** Removing `-る` / `-た` / `-て` /
//!   `-ば` from verb forms needs paradigm knowledge (godan vs. ichidan)
//!   that a suffix-stripper cannot provide.
//! * **Full Unicode NFKC integration.** The [`KanaNormalizer`] passes
//!   cover the Japanese-facing subset most callers want; the rest of
//!   NFKC (compatibility decompositions, casefolding, canonical
//!   composition of every combining sequence) is out of scope.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_ja::JAPANESE;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(JAPANESE.code(), "ja");
//! assert_eq!(JAPANESE.name(), "Japanese");
//! assert!(JAPANESE.is_stopword("は"));
//! assert!(!JAPANESE.is_stopword("桜"));
//! assert_eq!(JAPANESE.stem("私たち"), "私");
//!
//! let toks: Vec<&str> = JAPANESE
//!     .tokenize("彼はJavaScriptを勉強しています")
//!     .collect();
//! assert_eq!(toks, ["彼は", "JavaScript", "を", "勉強しています"]);
//! ```
//!
//! # Module map
//!
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`tokenizer`] — the [`JapaneseTokenizer`] character-type-based
//!   splitter, plus the [`CharType`] enum and the [`classify`]
//!   classification function.
//! - [`romaji`] — the [`KunreiRomaji`] (ISO 3602) romanizer and the
//!   [`KunreiRomajiAdapter`] the [`Language`](stringcheese_lang::Language)
//!   trait hands back by default.
//! - [`hepburn`] — the [`HepburnRomaji`] (Modified Hepburn) romanizer
//!   and its [`HepburnRomajiAdapter`], the alternate encoder the
//!   [`JAPANESE_WITH_HEPBURN`] pack variant hands back.
//! - [`normalize`] — the [`KanaNormalizer`] builder and the
//!   half-width / dakuten / katakana ↔ hiragana / full-width ASCII
//!   passes it composes.
//! - [`stemmer`] — the [`JapaneseStemmer`] polite-and-plural
//!   simplifier.
//! - The [`Japanese`] / [`JapaneseWithHepburn`] types and the
//!   [`JAPANESE`] / [`JAPANESE_WITH_HEPBURN`] constants live in this
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

#[cfg(feature = "datetime-scud")]
pub mod datetime_data;
#[cfg(feature = "alloc")]
pub mod hepburn;
#[cfg(feature = "alloc")]
pub mod normalize;
#[cfg(feature = "number-scud")]
pub mod number_data;
#[cfg(feature = "plural-scud")]
pub mod plural_data;
#[cfg(feature = "alloc")]
pub mod romaji;
#[cfg(feature = "alloc")]
pub mod stemmer;
pub mod stopwords;
pub mod tokenizer;

#[cfg(all(test, feature = "std", not(target_family = "wasm")))]
mod properties;

#[cfg(feature = "alloc")]
pub use hepburn::{HepburnRomaji, HepburnRomajiAdapter, to_hepburn};
#[cfg(feature = "alloc")]
pub use normalize::KanaNormalizer;
#[cfg(feature = "alloc")]
pub use romaji::{KunreiRomaji, KunreiRomajiAdapter};
#[cfg(feature = "alloc")]
pub use stemmer::JapaneseStemmer;
pub use stopwords::STOPWORDS;
pub use tokenizer::{CharType, JapaneseTokenizer, JapaneseTokens, classify};

// -----------------------------------------------------------------------
// The Japanese language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::hepburn::HepburnRomajiAdapter;
    use crate::romaji::KunreiRomajiAdapter;
    use crate::stemmer::JapaneseStemmer;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::JapaneseTokenizer;

    /// The Japanese language pack.
    ///
    /// Zero-sized; construct as [`Japanese`] and reuse the value freely
    /// across threads and calls, or grab the crate-level
    /// [`JAPANESE`](crate::JAPANESE) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Japanese;

    /// The Japanese language pack, alternate variant whose phonetic
    /// encoder is [`HepburnRomajiAdapter`] instead of the Kunrei-shiki
    /// default.
    ///
    /// Identical to [`Japanese`] in every other respect (stopwords,
    /// stemmer, tokenizer). Callers who prefer the English-facing
    /// Hepburn spellings for their phonetic keys grab
    /// [`JAPANESE_WITH_HEPBURN`](crate::JAPANESE_WITH_HEPBURN), or
    /// construct one via [`Japanese::with_hepburn_encoder`].
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct JapaneseWithHepburn;

    /// The static Kunrei-shiki romanizer adapter [`Japanese`] hands
    /// back from [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static KUNREI: KunreiRomajiAdapter = KunreiRomajiAdapter;

    /// The static Hepburn romanizer adapter [`JapaneseWithHepburn`]
    /// hands back from [`phonetic_encoder`](Language::phonetic_encoder).
    static HEPBURN: HepburnRomajiAdapter = HepburnRomajiAdapter;

    impl Japanese {
        /// A `const` factory that returns the sibling
        /// [`JapaneseWithHepburn`] pack — the Japanese pack whose
        /// phonetic encoder is Modified Hepburn instead of Kunrei-shiki.
        ///
        /// # Example
        ///
        /// ```
        /// use stringcheese_ja::Japanese;
        /// use stringcheese_lang::Language;
        ///
        /// let pack = Japanese::with_hepburn_encoder();
        /// let enc = pack.phonetic_encoder().expect("hepburn encoder is present");
        /// assert_eq!(enc.name(), "hepburn-romaji");
        /// ```
        #[must_use]
        pub const fn with_hepburn_encoder() -> JapaneseWithHepburn {
            JapaneseWithHepburn
        }
    }

    impl Language for Japanese {
        fn code(&self) -> &'static str {
            "ja"
        }

        fn name(&self) -> &'static str {
            "Japanese"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            JapaneseStemmer.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(JapaneseTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&KUNREI)
        }
    }

    impl Language for JapaneseWithHepburn {
        fn code(&self) -> &'static str {
            "ja"
        }

        fn name(&self) -> &'static str {
            "Japanese"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            JapaneseStemmer.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(JapaneseTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&HEPBURN)
        }
    }

    /// The singleton [`Japanese`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Japanese`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other `stringcheese-<lang>`
    /// pack follows.
    pub const JAPANESE: Japanese = Japanese;

    /// The singleton [`JapaneseWithHepburn`] language pack — Kunrei's
    /// sibling whose phonetic encoder emits Modified Hepburn keys.
    ///
    /// Callers who want the English-facing spellings (`sushi`,
    /// `Tokyo`, `Fuji`, `matcha`) as the phonetic key grab this
    /// constant; everyone else stays with [`JAPANESE`], which is
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)-set
    /// to Kunrei-shiki (ISO 3602) by default.
    pub const JAPANESE_WITH_HEPBURN: JapaneseWithHepburn = JapaneseWithHepburn;
}

#[cfg(feature = "alloc")]
pub use pack::{JAPANESE, JAPANESE_WITH_HEPBURN, Japanese, JapaneseWithHepburn};

// Register into `stringcheese-lang::registry` so callers who look up
// languages dynamically (`registry::language("ja")`) find this pack.
// alloc-gated because the pack constant itself is alloc-gated.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(JAPANESE);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-ja` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
