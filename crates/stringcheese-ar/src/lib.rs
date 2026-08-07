//! Arabic language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Arabic`] value that carries a ~150-entry Arabic
//! stopword list, the [`Light10`] Larkey ALP light stemmer, the
//! [`ArabicNormalizer`] diacritic-and-variant folder, the
//! [`ArabicTokenizer`] whitespace-based splitter, and a [`Buckwalter`]
//! transliteration as the phonetic encoder. Callers grab the singleton
//! [`ARABIC`] `const` — no construction ceremony required — and
//! delegate through the [`Language`](stringcheese_lang::Language)
//! trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-ar` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Arabic stopword table
//! or the Buckwalter mapping. Callers who need Arabic add
//! `stringcheese-ar = "0.1"` to their `Cargo.toml` explicitly.
//!
//! # First right-to-left-script pack
//!
//! This is the first `stringcheese-<lang>` implementation for a script
//! written right-to-left. It exists to validate the shape of the
//! [`Language`](stringcheese_lang::Language) trait on RTL text with
//! root-and-pattern morphology, and to prove that StringCheese's
//! byte/character-sequence processing model handles RTL scripts
//! without any special-case machinery.
//!
//! ## RTL is a display concern
//!
//! Every StringCheese subsystem — this pack included — processes
//! strings as **byte/character sequences in logical (UTF-8) order**.
//! That order is *first-consonant-first* regardless of how the string
//! is displayed. Rust source files, `str::eq_ignore_ascii_case`,
//! `str::starts_with`, `str::ends_with`, `char::is_alphanumeric`, and
//! every StringCheese algorithm operate on this logical order.
//! Right-to-left rendering is a **display-layer** concern (handled by
//! a bidi algorithm in the terminal, browser, or GUI toolkit); it
//! never affects what the byte comparison sees. Callers pass in
//! logical-order UTF-8 (which is what
//! `String::from`-of-a-`&str` naturally gives them) and this pack
//! processes it as-is.
//!
//! ## Design decisions
//!
//! * **Larkey ALP light10 stemmer.** Leah Larkey, Lisa Ballesteros,
//!   and Margaret Connell's 2002 SIGIR paper introduced a family of
//!   "light" Arabic stemmers — rule-based prefix/suffix strippers
//!   that ignore the root-and-pattern morphology Arabic grammarians
//!   usually reach for. Light10 is the largest and most-used variant;
//!   Lucene's `ArabicStemmer` and Snowball's `arabic_stemmer.sbl`
//!   both trace their rule sets to it. See [`Light10`] for the
//!   algorithm.
//! * **Buckwalter transliteration.** A deterministic, ASCII-only,
//!   *reversible* mapping from Arabic scalars to ASCII characters.
//!   Not a phonetic encoder in the classical sense (Soundex-like
//!   sound-alike matching), but a stable equivalence-class key that
//!   works well for byte-oriented indexes. See [`Buckwalter`] for
//!   the full mapping.
//! * **Arabic normalizer.** Strips harakat (short-vowel diacritics),
//!   folds alef variants (`أ إ آ → ا`), folds `ى → ي`, and offers
//!   opt-in teh-marbuta → heh folding as a builder flag. See
//!   [`ArabicNormalizer`] for the (short) rule set.
//! * **`SimpleTokenizer` wrapper.** Modern Standard Arabic uses ASCII
//!   spaces between orthographic words, and every Arabic letter is
//!   `char::is_alphanumeric`, so the default
//!   [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer) handles
//!   Arabic word segmentation correctly out of the box. This pack
//!   ships an [`ArabicTokenizer`] named type that mirrors the
//!   per-pack tokenizer convention but delegates.
//!
//! # Deferred to a follow-up wave
//!
//! * **Root-and-pattern morphological analysis.** Extracting the 3/4-
//!   letter consonantal root (`كتب` "write", `درس` "study", ...) needs
//!   template matching against a large table of derivational patterns
//!   plus a lexicon (Buckwalter Arabic Morphological Analyzer,
//!   MADAMIRA, Farasa). Light10 is the well-established "good enough
//!   for IR" baseline; root extraction is deferred to a downstream
//!   `stringcheese-ar-morph` crate.
//! * **Dialect coverage.** Egyptian (`مش`, `دلوقتي`, `يعني`), Levantine
//!   (`شو`, `هلق`), and Gulf (`شلون`, `وين`) function words and
//!   morphology are absent — this pack targets Modern Standard Arabic
//!   only. Dialect packs (`stringcheese-ar-eg`, `stringcheese-ar-lv`,
//!   `stringcheese-ar-gulf`) are a follow-up.
//! * **Farsi / Urdu / Pashto variants.** These languages use the
//!   Arabic script but have distinct grammars and function-word
//!   inventories. They belong in their own `stringcheese-fa` /
//!   `stringcheese-ur` / `stringcheese-ps` packs.
//! * **True phonetic encoder.** Buckwalter is a *reversible*
//!   transliteration, not a sound-alike encoder. An `AraSoundex` or
//!   ISRI-style phonetic encoder that collapses homophones would be
//!   a useful alternate.
//! * **Digit normalization.** Arabic uses both Eastern Arabic digits
//!   (`٠١٢٣٤٥٦٧٨٩`) and Western Arabic digits (`0123456789`); a
//!   normalizer variant that folds between them would help
//!   numeric-search use-cases.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_ar::ARABIC;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(ARABIC.code(), "ar");
//! assert_eq!(ARABIC.name(), "Arabic");
//! assert!(ARABIC.is_stopword("في"));
//! assert!(!ARABIC.is_stopword("كتاب"));
//! // The definite-article prefix is stripped by Light10.
//! assert_eq!(ARABIC.stem("الكتاب"), "كتاب");
//!
//! let toks: Vec<&str> = ARABIC
//!     .tokenize("محمد يحب القراءة")
//!     .collect();
//! assert_eq!(toks, ["محمد", "يحب", "القراءة"]);
//! ```
//!
//! # Module map
//!
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`mod@normalize`] — the [`ArabicNormalizer`] and the
//!   [`normalize()`] free function.
//! - [`stemmer`] — the [`Light10`] Larkey ALP light stemmer.
//! - [`phonetic`] — the [`Buckwalter`] transliteration and the
//!   [`BuckwalterAdapter`] the
//!   [`Language`](stringcheese_lang::Language) trait hands back.
//! - [`tokenizer`] — the [`ArabicTokenizer`] whitespace-based
//!   splitter (a thin wrapper over the default
//!   [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer)).
//! - The [`Arabic`] type and the [`ARABIC`] constant live in this
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
pub use normalize::{ArabicNormalizer, normalize};
#[cfg(feature = "alloc")]
pub use phonetic::{Buckwalter, BuckwalterAdapter};
#[cfg(feature = "alloc")]
pub use stemmer::Light10;
pub use stopwords::STOPWORDS;
pub use tokenizer::ArabicTokenizer;

// -----------------------------------------------------------------------
// The Arabic language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::BuckwalterAdapter;
    use crate::stemmer::Light10;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::ArabicTokenizer;

    /// The Arabic language pack.
    ///
    /// Zero-sized; construct as [`Arabic`] and reuse the value freely
    /// across threads and calls, or grab the crate-level
    /// [`ARABIC`](crate::ARABIC) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Arabic;

    /// The static [`BuckwalterAdapter`] [`Arabic`] hands back from
    /// [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static BUCKWALTER: BuckwalterAdapter = BuckwalterAdapter;

    impl Language for Arabic {
        fn code(&self) -> &'static str {
            "ar"
        }

        fn name(&self) -> &'static str {
            "Arabic"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            Light10.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(ArabicTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&BUCKWALTER)
        }
    }

    /// The singleton [`Arabic`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Arabic`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other `stringcheese-<lang>`
    /// pack follows.
    pub const ARABIC: Arabic = Arabic;
}

#[cfg(feature = "alloc")]
pub use pack::{ARABIC, Arabic};

// Register into `stringcheese-lang::registry` so callers who look up
// languages dynamically (`registry::language("ar")`) find this pack.
// alloc-gated because the pack constant itself is alloc-gated.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(ARABIC);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-ar` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
