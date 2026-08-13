//! Chinese (Simplified) language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Chinese`] value that carries a ~80-entry Chinese
//! stopword list, the [`ChineseTokenizer`] character-level tokenizer,
//! the [`ChineseStemmer`] identity stemmer, and a [`Pinyin`]
//! (tone-mark-stripped Hanyu Pinyin) phonetic encoder over a
//! ~1000-entry curated Han-character subset. Callers grab the
//! singleton [`CHINESE`] `const` — no construction ceremony required
//! — and delegate through the [`Language`](stringcheese_lang::Language)
//! trait.
//!
//! # Design commitment — **no dictionary**
//!
//! This pack does **NOT** perform dictionary-based word segmentation.
//! The gold-standard Chinese segmenters (jieba, thulac, pkuseg,
//! `HanLP`, LTP) all ship 100K-400K-entry lexica and megabyte-scale
//! model files; shipping any of that in a wasm-first / offline-first
//! language pack defeats the whole point of the crate's size budget.
//! Instead, the shipped [`ChineseTokenizer`] emits every CJK Han
//! scalar as its own token (character-level segmentation) and keeps
//! Latin / digit runs together — the same preprocessing philosophy
//! that BERT's multilingual tokenizer uses for Chinese.
//!
//! Concretely:
//!
//! - `你好` → `["你", "好"]` — every Han character is its own token.
//! - `hello` → `["hello"]` — Latin runs stay together.
//! - `2026年` → `["2026", "年"]` — digit run + one Han character.
//! - `中文hello123` → `["中", "文", "hello", "123"]`.
//!
//! Callers that need jieba-quality word segmentation should compose
//! a purpose-built segmenter on top of the
//! [`Language`](stringcheese_lang::Language) trait's tokenize
//! accessor — a hypothetical `stringcheese-zh-jieba` sibling crate
//! (deferred to a follow-up wave) would take that role while the
//! base `stringcheese-zh` crate stays wasm-friendly.
//!
//! # Simplified vs Traditional
//!
//! The pack targets **Simplified Chinese** (mainland China,
//! Singapore, Malaysia). Stopwords and pinyin table entries are
//! keyed on Simplified surface forms (`国` not `國`, `们` not `們`,
//! `简` not `簡`). Traditional characters (Taiwan / Hong Kong /
//! diaspora) do classify as `Han` and tokenize the same way, but
//! the stopword and pinyin lookups will typically miss.
//!
//! Traditional Chinese support requires either:
//!
//! - a Simplified↔Traditional converter (deferred: neither shipped
//!   in this crate nor in a sibling `stringcheese-zh-convert`), or
//! - a separate `stringcheese-zh-hant` pack with Traditional-facing
//!   stopword and pinyin tables (deferred).
//!
//! # Non-goals (deferred)
//!
//! - **Dictionary-driven word segmentation** (jieba-lite,
//!   thulac-lite, pkuseg-lite). Deferred to
//!   `stringcheese-zh-jieba`.
//! - **Simplified↔Traditional conversion.** Deferred.
//! - **Cantonese** (Yue, `yue`) — separate phonology, separate
//!   romanization (Jyutping, Yale). Deferred to
//!   `stringcheese-yue-jyutping`.
//! - **Wade-Giles / Yale / Tongyong Pinyin.** Alternate
//!   romanizations. Deferred.
//! - **Bopomofo (Zhuyin, 注音符号).** Taiwan's native phonetic
//!   script (U+3100..=U+312F). Not currently classified specially —
//!   Bopomofo scalars fall into `Other`. Deferred.
//! - **Tone-preserving pinyin.** The shipped encoder strips tones
//!   for stable equivalence-class keys; a `pinyin-zh-toned` sibling
//!   is a natural extension.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_zh::CHINESE;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(CHINESE.code(), "zh");
//! assert_eq!(CHINESE.name(), "Chinese");
//! assert!(CHINESE.is_stopword("的"));
//! assert!(CHINESE.is_stopword("我们"));
//! assert!(!CHINESE.is_stopword("北京"));
//!
//! // Chinese has no inflectional morphology — the stemmer is
//! // identity.
//! assert_eq!(CHINESE.stem("中国"), "中国");
//!
//! let toks: Vec<&str> = CHINESE.tokenize("中文hello123").collect();
//! assert_eq!(toks, ["中", "文", "hello", "123"]);
//! ```
//!
//! # Module map
//!
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`tokenizer`] — the [`ChineseTokenizer`] character-level
//!   splitter, plus the [`CharType`] enum and the [`classify`]
//!   classification function.
//! - [`stemmer`] — the [`ChineseStemmer`] identity stemmer.
//! - [`phonetic`] — the [`Pinyin`] Hanyu Pinyin (tone-stripped)
//!   encoder and the [`PinyinAdapter`] the
//!   [`Language`](stringcheese_lang::Language) trait hands back.
//! - The [`Chinese`] type and the [`CHINESE`] constant live in this
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

#[cfg(feature = "case-scud")]
pub mod case_data;
#[cfg(feature = "collation-scud")]
pub mod collation_data;
#[cfg(feature = "number-scud")]
pub mod number_data;
#[cfg(feature = "alloc")]
pub mod phonetic;
#[cfg(feature = "plural-scud")]
pub mod plural_data;
#[cfg(feature = "alloc")]
pub mod stemmer;
pub mod stopwords;
pub mod tokenizer;

#[cfg(all(test, feature = "std", not(target_family = "wasm")))]
mod properties;

#[cfg(feature = "alloc")]
pub use phonetic::{PINYIN_TABLE_LEN, Pinyin, PinyinAdapter};
#[cfg(feature = "alloc")]
pub use stemmer::ChineseStemmer;
pub use stopwords::STOPWORDS;
pub use tokenizer::{CharType, ChineseTokenizer, ChineseTokens, classify};

// -----------------------------------------------------------------------
// The Chinese language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::PinyinAdapter;
    use crate::stemmer::ChineseStemmer;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::ChineseTokenizer;

    /// The Chinese (Simplified) language pack.
    ///
    /// Zero-sized; construct as [`Chinese`] and reuse the value
    /// freely across threads and calls, or grab the crate-level
    /// [`CHINESE`](crate::CHINESE) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices (no dictionary, character-level tokenization,
    /// tone-stripped Pinyin) and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Chinese;

    /// The static Pinyin adapter [`Chinese`] hands back from
    /// [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static PINYIN: PinyinAdapter = PinyinAdapter;

    impl Language for Chinese {
        fn code(&self) -> &'static str {
            "zh"
        }

        fn name(&self) -> &'static str {
            "Chinese"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            ChineseStemmer.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(ChineseTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&PINYIN)
        }
    }

    /// The singleton [`Chinese`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Chinese`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other
    /// `stringcheese-<lang>` pack follows.
    pub const CHINESE: Chinese = Chinese;
}

#[cfg(feature = "alloc")]
pub use pack::{CHINESE, Chinese};

// Register into `stringcheese-lang::registry` so callers who look up
// languages dynamically (`registry::language("zh")`) find this pack.
// alloc-gated because the pack constant itself is alloc-gated.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(CHINESE);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-zh` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
