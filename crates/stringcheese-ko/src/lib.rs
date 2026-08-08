//! Korean language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Korean`] value that carries a ~60-entry Korean
//! stopword list, the [`KoreanTokenizer`] whitespace-and-punctuation
//! splitter, the [`KoreanStemmer`] particle stripper, and a
//! [`KoreanPhonex`] Revised-Romanization + Soundex-family phonetic
//! encoder. Callers grab the singleton [`KOREAN`] `const` — no
//! construction ceremony required — and delegate through the
//! [`Language`](stringcheese_lang::Language) trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-ko` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Korean stopword table,
//! the jamo decomposition tables, or the Revised Romanization
//! machinery. Callers who need Korean add `stringcheese-ko = "0.1"`
//! to their `Cargo.toml` explicitly.
//!
//! # First Hangul-script pack
//!
//! This is the first `stringcheese-<lang>` implementation for text
//! written in **Hangul** — a featural alphabet whose letters
//! ("jamos") are packed into visually-fused syllable blocks. Hangul
//! is unusual among world scripts:
//!
//! * **Precomposed syllables.** Modern Korean text is stored as
//!   precomposed Hangul syllables in the range U+AC00..=U+D7A3
//!   (exactly 11172 code points). Each syllable is deterministically
//!   the composition of a *choseong* (initial consonant, "L"), a
//!   *jungseong* (medial vowel, "V"), and an optional *jongseong*
//!   (final consonant, "T") jamo. See [`jamo`] for the closed-form
//!   decomposition and composition formulas from Unicode 3.12.
//! * **Space-delimited words.** Unlike Japanese or Chinese, Korean
//!   uses whitespace between words. A plain
//!   [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer) is very
//!   nearly the right shape; [`KoreanTokenizer`] adds Korean-facing
//!   punctuation handling.
//! * **Agglutinative morphology.** Verbs and nouns accrete a stack of
//!   suffixes / particles: `학교에서도` "at school too" fuses `학교`
//!   with `-에서` (locative) and `-도` (focus). The
//!   [`KoreanStemmer`] iteratively strips a small closed set of the
//!   most common noun-attached case particles, leaving verb / adjective
//!   conjugation stripping to a dictionary-driven follow-up.
//!
//! ## The Hangul-specific invariants
//!
//! Hangul is a **left-to-right script**. There are no display-order
//! surprises; a Rust source file containing `"한국"` reads
//! h-a-n-g-u-k in both logical and display order.
//!
//! The two things to remember are *storage width* and *jamo model*:
//!
//! * **Every precomposed syllable is 3 bytes in UTF-8.** The
//!   U+AC00..=U+D7A3 block sits inside UTF-8's 3-byte range
//!   (U+0800..=U+FFFF), so a 6-syllable word like `"안녕하세요"` is 18
//!   bytes. Any code that mixes byte offsets with character-boundary
//!   logic will silently corrupt token or syllable boundaries. This
//!   crate operates via [`str::chars`] and `char` values wherever a
//!   Hangul-aware operation is needed — never raw byte offsets.
//! * **Conjoining jamos vs. compatibility jamos.** The
//!   U+1100..=U+11FF block carries the *conjoining* jamos used by
//!   Unicode's canonical decomposition and by the [`jamo`] module.
//!   The U+3130..=U+318F *Hangul Compatibility Jamo* block is a
//!   legacy compatibility mapping used only for glyph rendering; it
//!   plays no role in text processing. Callers that ingest
//!   compatibility jamos should NFKC-decompose before handing text to
//!   the pack.
//!
//! # Design choices
//!
//! * **Algorithmic jamo decomposition.** [`jamo::decompose_syllable`]
//!   and [`jamo::compose_jamo`] implement the closed-form formulas
//!   from Unicode 3.12 — no lookup tables. The `jamo_decompose`
//!   integration test enumerates all 11172 precomposed syllables and
//!   round-trips them.
//! * **Revised Romanization (RR) as the phonetic intermediate.** The
//!   phonetic encoder is a two-step algorithm: decompose to jamos,
//!   emit the RR Latin form per the National Institute of the Korean
//!   Language's 2000 tables, then reduce to a 4-character Soundex-
//!   family key with Korean-tuned consonant grouping. See
//!   [`phonetic`] for the tables and the reducer.
//! * **~60 stopwords.** Common Korean demonstratives, interrogatives,
//!   conjunctions, common adverbs, and the dictionary forms of the
//!   copula and high-frequency verbs. Case particles (`은/는/이/가/…`)
//!   are deliberately not carried as stopwords — they attach at the
//!   syllable end and are stripped by the stemmer.
//! * **Coarse suffix-stripping stemmer.** Iteratively strips the
//!   closed set of common noun-attached case particles from the
//!   syllable end. Verb / adjective conjugation stripping needs a
//!   paradigm-aware morphological analyzer and is deferred.
//! * **Whitespace-and-punctuation tokenizer.** Korean uses ASCII
//!   spaces between orthographic words; the [`KoreanTokenizer`]
//!   splits on whitespace and Korean-facing punctuation while keeping
//!   Latin / digit characters glued to any adjacent Hangul (matching
//!   how Korean readers group borrowed English terms).
//!
//! # Deferred to a follow-up wave
//!
//! * **Verb / adjective conjugation stripping.** Korean verb endings
//!   (`-습니다`, `-어요`, `-았`, `-겠`, `-어서`, `-니까`, …) fuse
//!   with stems whose surface form varies with the following vowel
//!   (`먹` "eat" + `-어요` → `먹어요`; `가` "go" + `-아요` → `가요`
//!   after vowel elision). Recognizing the elision needs a
//!   paradigm-aware analyzer — deferred to a `stringcheese-ko-morph`
//!   sibling with a dictionary loader.
//! * **Contextual RR rules.** Full RR is context-sensitive
//!   (assimilation, palatalization, liaison across syllable
//!   boundaries). This encoder emits the surface-form jamo-by-jamo
//!   romanization; a follow-up could implement the assimilation
//!   rules for higher romanization fidelity.
//! * **McCune-Reischauer alternate encoder.** The pre-2000 romanization
//!   still used in North Korea and Western academic literature. A
//!   `KoreanWithMcCuneReischauer` variant would slot in the same
//!   place the Japanese pack's Hepburn variant does.
//! * **Hangul Compatibility Jamo normalization.** Ingesting text that
//!   uses U+3130..=U+318F requires an NFKC pass; adding a native
//!   normalizer here (rather than requiring callers to run
//!   `unicode_normalization` first) is a follow-up.
//! * **Compound-word splitting.** `대한민국` "Republic of Korea" is one
//!   orthographic word; splitting it into `대한` + `민국` needs a
//!   compound dictionary. Not shipped.
//! * **Korean-tailored collator.** Korean sorts by jamo order (initial
//!   consonant, then medial vowel, then final consonant) — the
//!   default Unicode code-point ordering already produces the correct
//!   Korean sort for pure Hangul input because the precomposed
//!   syllable block is laid out in that order. Cross-script sorting
//!   (mixed Hangul + Latin) is a caller concern; no collator ships.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_ko::KOREAN;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(KOREAN.code(), "ko");
//! assert_eq!(KOREAN.name(), "Korean");
//! assert!(KOREAN.is_stopword("그리고"));
//! assert!(!KOREAN.is_stopword("한국"));
//! assert_eq!(KOREAN.stem("학교에서도"), "학교");
//!
//! let toks: Vec<&str> = KOREAN
//!     .tokenize("나는 학교에 갑니다.")
//!     .collect();
//! assert_eq!(toks, ["나는", "학교에", "갑니다"]);
//! ```
//!
//! # Module map
//!
//! - [`jamo`] — algorithmic Hangul syllable ↔ jamo decomposition /
//!   composition ([`jamo::decompose_syllable`],
//!   [`jamo::compose_jamo`]).
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`tokenizer`] — the [`KoreanTokenizer`] splitter.
//! - [`stemmer`] — the [`KoreanStemmer`] particle stripper.
//! - [`phonetic`] — [`phonetic::revised_romanization`] plus the
//!   [`KoreanPhonex`] reducer and the
//!   [`KoreanPhonexAdapter`] the
//!   [`Language`](stringcheese_lang::Language) trait hands back.
//! - The [`Korean`] type and the [`KOREAN`] constant live in this
//!   crate's root.

#![cfg_attr(not(feature = "std"), no_std)]
// `deny` rather than `forbid` because the `stringcheese_lang::
// register_language!` invocation below expands to a `linkme`-backed
// static whose implementation is `unsafe`-tagged (safe in practice
// — that's linkme's whole design — but flagged by the
// `unsafe_code` lint). The macro emits an explicit
// `#[allow(unsafe_code)]` at the sole registration site; the rest
// of this crate is still lint-enforced no-`unsafe`. Same pattern as
// the other language packs.
#![deny(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod jamo;
#[cfg(feature = "alloc")]
pub mod phonetic;
#[cfg(feature = "alloc")]
pub mod stemmer;
pub mod stopwords;
pub mod tokenizer;

#[cfg(all(test, feature = "std", not(target_family = "wasm")))]
mod properties;

#[cfg(feature = "alloc")]
pub use phonetic::{KoreanPhonex, KoreanPhonexAdapter};
#[cfg(feature = "alloc")]
pub use stemmer::KoreanStemmer;
pub use stopwords::STOPWORDS;
pub use tokenizer::{KoreanTokenizer, KoreanTokens};

// -----------------------------------------------------------------------
// The Korean language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::KoreanPhonexAdapter;
    use crate::stemmer::KoreanStemmer;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::KoreanTokenizer;

    /// The Korean language pack.
    ///
    /// Zero-sized; construct as [`Korean`] and reuse the value freely
    /// across threads and calls, or grab the crate-level
    /// [`KOREAN`](crate::KOREAN) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Korean;

    /// The static [`KoreanPhonexAdapter`] [`Korean`] hands back from
    /// [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static PHONEX_KO: KoreanPhonexAdapter = KoreanPhonexAdapter;

    impl Language for Korean {
        fn code(&self) -> &'static str {
            "ko"
        }

        fn name(&self) -> &'static str {
            "Korean"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            KoreanStemmer.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(KoreanTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&PHONEX_KO)
        }
    }

    /// The singleton [`Korean`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Korean`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other `stringcheese-<lang>`
    /// pack follows.
    pub const KOREAN: Korean = Korean;
}

#[cfg(feature = "alloc")]
pub use pack::{KOREAN, Korean};

// Register into `stringcheese-lang::registry` so callers who look up
// languages dynamically (`registry::language("ko")`) find this pack.
// alloc-gated because the pack constant itself is alloc-gated.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(KOREAN);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-ko` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
