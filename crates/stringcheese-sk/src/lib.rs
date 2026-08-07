//! Slovak language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Slovak`] value that carries the Slovak stopword list,
//! the [`SlovakStemmer`] light suffix-stripping stemmer, the
//! whitespace-and-punctuation [`SlovakTokenizer`], and a
//! [`SlovakPhonex`] phonetic hookup. Callers grab the singleton
//! [`SLOVAK`] `const` — no construction ceremony required — and
//! delegate through the [`Language`](stringcheese_lang::Language)
//! trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-sk` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Slovak stopword table or
//! the light stemmer's code. Callers who need Slovak add
//! `stringcheese-sk = "0.1"` to their `Cargo.toml` explicitly.
//!
//! # No canonical Snowball Slovak
//!
//! Slovak, like Czech, is **not covered by an official Snowball
//! stemmer**. Community forks exist (variants of the Dolamic & Savoy
//! Czech light stemmer adapted for Slovak, hand-crafted Slovak
//! stemmers used in academic IR papers) but none has been adopted as
//! canonical the way Snowball Russian has.
//!
//! Given the absence of a canonical Snowball Slovak, this crate ships
//! a **light suffix-stripping stemmer** rather than a non-canonical
//! port. Same rationale as the Czech pack (see
//! [`stringcheese_cs`](https://docs.rs/stringcheese-cs)): shipping a
//! full-morphology algorithm whose reference behaviour is uncertain
//! would produce plausible-looking but subtly wrong stems; a light
//! stemmer with an explicit, hand-audited suffix table is easier to
//! reason about, test, and improve incrementally.
//!
//! # Slovak vs. Czech
//!
//! Slovak and Czech are mutually intelligible West Slavic languages
//! and share ~90% of their morphology; the two packs therefore share
//! the same overall shape. The differences this pack captures are:
//!
//! * **Slovak-specific letters** — Slovak has **ä** (an open-e vowel),
//!   **ĺ** and **ŕ** (long syllabic *l* and *r*), **ľ** (the palatal
//!   *l*, orthographically distinct from *l* in a way Czech does not
//!   mark), and **ô** (a diphthong orthographically /uo/). None of
//!   these appears in Czech.
//! * **Slovak lacks Czech's `ř` and `ě`.** Czech's fricative-trilled
//!   *ř* has no cognate in Slovak; Slovak's *r* covers both roles.
//!   Czech's palatalizing *ě* is spelled directly in Slovak (as *ie*,
//!   *e*, or *ia* depending on context).
//! * **Slovak lacks Czech's `ů` ring-over-u.** Slovak uses only *ú*
//!   for long *u*.
//! * **Slovak infinitive ends in `-ť`.** Czech's `-t` infinitive
//!   (`pracovat`, `dělat`, `mluvit`) is `-ť` in Slovak (`pracovať`,
//!   `robiť`, `hovoriť`). The stemmer's suffix table encodes this
//!   difference explicitly.
//!
//! # Slovak alphabet
//!
//! The Slovak alphabet extends the Latin block with **á, ä, č, ď, é,
//! í, ĺ, ľ, ň, ó, ô, ŕ, š, ť, ú, ý, ž** (each with an uppercase
//! counterpart). These fall into groups:
//!
//! * **Haček (caron) consonants** — `č`, `š`, `ž`, `ď`, `ť`, `ň`, `ľ`
//!   — orthographic marks for palatalized or affricated consonants.
//!   `ľ` is Slovak-specific and marks the palatal lateral. The
//!   phonetic module folds these to Latin placeholders for
//!   Soundex-shape classification.
//! * **Long vowels** — `á`, `é`, `í`, `ó`, `ú`, `ý`, plus the syllabic
//!   long consonants `ĺ` and `ŕ` (which behave as vowels in Slovak's
//!   vowel-consonant alternation rules). All fold to their short
//!   counterpart for stemming and encoding.
//! * **`ä` (e-diaeresis)** — Slovak-specific. Phonetically an
//!   open-front vowel /æ/, closer to *e* than to *a*; the phonetic
//!   module folds `ä → E`.
//! * **`ô` (o-circumflex)** — Slovak-specific. Orthographically marks
//!   the diphthong /uo/; the phonetic module folds `ô → O` (the base
//!   vowel it decorates), matching the long-vowel convention.
//!
//! All Slovak scalars in this list are UTF-8 multi-byte (2 bytes each
//! in U+0080..=U+07FF for `á/é/í/ó/ú/ý/ä/ô` and 2 bytes for the
//! extended Latin `č/š/ž/ď/ť/ň/ľ/ĺ/ŕ` in U+0100..=U+017F). All suffix
//! and region arithmetic in [`stemmer`] runs on `Vec<char>` indices —
//! never raw byte offsets — so no scalar is ever sliced apart.
//!
//! # Design choices
//!
//! * **Light suffix-stripping stemmer.** A single-pass longest-match
//!   pass over a hand-audited table of Slovak inflectional suffixes
//!   (noun / adjective / verb / possessive), guarded by an RV floor
//!   that mirrors the Snowball-family convention (region starts after
//!   the first vowel-followed-by-consonant). See [`stemmer`] for the
//!   rules and reference-pair coverage.
//! * **~240-word stopword list.** Personal / possessive /
//!   demonstrative pronouns, prepositions, conjunctions, particles,
//!   high-frequency forms of the copula *byť*, common adverbs, and
//!   quantifiers. See [`stopwords`].
//! * **PHONEX-Slovak phonetic encoder.** A 4-character Soundex-shape
//!   key with Slovak-tuned preprocessing (haček-fold including the
//!   Slovak-only `ľ → L`, long-vowel-fold including `ĺ`/`ŕ`, Slovak-
//!   specific `ä → E` and `ô → O`, silent `h`) over a Slovak-tuned
//!   classification table. Adapter name: `"phonex-sk"` — chosen for
//!   consistency with the other Latin-alphabet language packs
//!   (`phonex-nl`, `phonex-pt`, `phonex-es`, `phonex-fr`, `phonex-cs`).
//!   See [`phonetic`] for the mapping table and rationale.
//! * **Simple tokenizer.** Slovak is whitespace-and-punctuation
//!   delimited; every letter of the Slovak alphabet satisfies
//!   [`char::is_alphanumeric`]. [`SlovakTokenizer`] is a transparent
//!   wrapper around
//!   [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer).
//! * **Unicode case fold in `is_stopword`.** The default trait
//!   implementation uses [`str::eq_ignore_ascii_case`] which would
//!   miss uppercase Slovak-specific letters (`Č`, `Š`, `Ž`, `Á`, `É`,
//!   `Í`, `Ó`, `Ú`, `Ý`, `Ď`, `Ť`, `Ň`, `Ľ`, `Ĺ`, `Ŕ`, `Ä`, `Ô`).
//!   Slovak overrides
//!   [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//!   with a Unicode lowercase pass.
//! * **Default Unicode collation.**
//!   [`Language::collator`](stringcheese_lang::Language::collator)
//!   returns `None`. Slovak collation follows a locale-tailored order
//!   (`ch` sorts as a single digraph after `h`, hačeks sort adjacent
//!   to their bases); callers who need a locale-tailored collator
//!   should reach for `icu_collator`.
//!
//! # Deferred to a follow-up wave
//!
//! * **Canonical Snowball parity.** There is no canonical Snowball
//!   Slovak; if the Snowball project ever publishes one, this crate
//!   will adopt it under a new module (`snowball_canonical`) and keep
//!   the light stemmer as `stemmer_light` for callers who want the
//!   current behaviour.
//! * **Aggressive derivational stripping.** The current table sticks
//!   to inflectional suffixes. A parallel aggressive variant would
//!   strip derivational suffixes (`-osť`, `-stvo`, `-izmus`) and add
//!   a palatalization step.
//! * **Consonant alternation (`ruka → ruce`-style).** Slovak morphology
//!   applies velar / palatal alternations to the stem (`k / c / č`,
//!   `h / z / ž`, `ch / š`) in certain case-number cells, as Czech
//!   does. The light stemmer strips the suffix but does not reverse
//!   the alternation. A palatalization-aware pass is a follow-up.
//! * **Full-corpus cross-verification.** The reference-pair test
//!   embeds a hand-traced set that exercises each family of suffixes;
//!   full-corpus cross-verification would require a lexicon.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_sk::SLOVAK;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(SLOVAK.code(), "sk");
//! assert_eq!(SLOVAK.name(), "Slovak");
//! assert!(SLOVAK.is_stopword("a"));
//! assert!(SLOVAK.is_stopword("je"));
//! assert!(SLOVAK.is_stopword("NIE JE".split_whitespace().next().unwrap()));
//! assert!(!SLOVAK.is_stopword("kniha"));
//!
//! let toks: Vec<&str> = SLOVAK
//!     .tokenize("Mačka spí na koberci.")
//!     .collect();
//! assert_eq!(toks, ["Mačka", "spí", "na", "koberci"]);
//! ```
//!
//! # Module map
//!
//! - [`stemmer`] — the [`SlovakStemmer`] light suffix-stripping
//!   stemmer.
//! - [`phonetic`] — [`SlovakPhonex`] plus the [`SlovakPhonexAdapter`]
//!   the [`Language`](stringcheese_lang::Language) trait hands back.
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`tokenizer`] — the [`SlovakTokenizer`] wrapper.
//! - The [`Slovak`] type and the [`SLOVAK`] constant live in this
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

#[cfg(feature = "alloc")]
pub mod phonetic;
#[cfg(feature = "alloc")]
pub mod stemmer;
pub mod stopwords;
pub mod tokenizer;

#[cfg(all(test, feature = "std", not(target_family = "wasm")))]
mod properties;

#[cfg(feature = "alloc")]
pub use phonetic::{SlovakPhonex, SlovakPhonexAdapter};
#[cfg(feature = "alloc")]
pub use stemmer::SlovakStemmer;
pub use stopwords::STOPWORDS;
pub use tokenizer::SlovakTokenizer;

// -----------------------------------------------------------------------
// The Slovak language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;
    use alloc::string::String;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::SlovakPhonexAdapter;
    use crate::stemmer::SlovakStemmer;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::SlovakTokenizer;

    /// The Slovak language pack.
    ///
    /// Zero-sized; construct as [`Slovak`] and reuse the value freely
    /// across threads and calls, or grab the crate-level
    /// [`SLOVAK`](crate::SLOVAK) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Slovak;

    /// The static [`SlovakPhonexAdapter`] [`Slovak`] hands back from
    /// [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static PHONEX: SlovakPhonexAdapter = SlovakPhonexAdapter;

    /// Normalize a Slovak string for stopword comparison: lowercase
    /// under default Unicode rules. Handles the Slovak-specific letters
    /// `Č Ď Ň Š Ť Ž Á É Í Ó Ú Ý Ä Ĺ Ľ Ŕ Ô` correctly (each folds to
    /// its lowercase counterpart with no locale tailoring required).
    fn normalize_for_stopword(word: &str) -> String {
        let mut out = String::with_capacity(word.len());
        for c in word.chars() {
            for lc in c.to_lowercase() {
                out.push(lc);
            }
        }
        out
    }

    impl Language for Slovak {
        fn code(&self) -> &'static str {
            "sk"
        }

        fn name(&self) -> &'static str {
            "Slovak"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        /// Slovak-aware stopword membership.
        ///
        /// Overrides the default trait implementation (which uses
        /// [`str::eq_ignore_ascii_case`], missing every uppercase
        /// Slovak-specific letter like `Č`, `Š`, `Ž`, `Á`, `Í`, `Ú`,
        /// `Ý`, `Ď`, `Ť`, `Ň`, `Ľ`, `Ĺ`, `Ŕ`, `Ä`, `Ô`) with a
        /// Unicode lowercase pass — so `NIE` and `nie` both find `nie`
        /// in the stopword list.
        fn is_stopword(&self, word: &str) -> bool {
            let normalized = normalize_for_stopword(word);
            STOPWORDS.contains(&normalized.as_str())
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            SlovakStemmer.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(SlovakTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&PHONEX)
        }
    }

    /// The singleton [`Slovak`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Slovak`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other `stringcheese-<lang>`
    /// pack follows.
    pub const SLOVAK: Slovak = Slovak;
}

#[cfg(feature = "alloc")]
pub use pack::{SLOVAK, Slovak};

// Register into `stringcheese-lang::registry` so callers who look up
// languages dynamically (`registry::language("sk")`) find this pack.
// alloc-gated because the pack constant itself is alloc-gated.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(SLOVAK);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-sk` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
