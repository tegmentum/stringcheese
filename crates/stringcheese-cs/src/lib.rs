//! Czech language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Czech`] value that carries the Czech stopword list,
//! the [`CzechStemmer`] light suffix-stripping stemmer, the
//! whitespace-and-punctuation [`CzechTokenizer`], and a
//! [`CzechPhonex`] phonetic hookup. Callers grab the singleton
//! [`CZECH`] `const` — no construction ceremony required — and
//! delegate through the [`Language`](stringcheese_lang::Language)
//! trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-cs` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Czech stopword table or
//! the light stemmer's code. Callers who need Czech add
//! `stringcheese-cs = "0.1"` to their `Cargo.toml` explicitly.
//!
//! # No canonical Snowball Czech
//!
//! Unlike English, French, German, Spanish, Portuguese, Dutch, and
//! Russian — each of which has a Porter/Boulton canonical Snowball
//! algorithm shipped as `*.sbl` in the Snowball repository — **Czech
//! is not covered by an official Snowball stemmer**. Community forks
//! exist (Dolamic & Savoy's "light" and "aggressive" Czech stemmers,
//! the community `czech-stemmer` package that ports the Dolamic/Savoy
//! rules) but none has been adopted as canonical the way Snowball
//! Russian has.
//!
//! Given the absence of a canonical Snowball Czech, this crate ships
//! a **light suffix-stripping stemmer** rather than a non-canonical
//! port of a related-language algorithm. The rationale is the same as
//! for the Ukrainian pack: shipping a full-morphology algorithm whose
//! reference behaviour is uncertain would produce plausible-looking
//! but subtly wrong stems; a light stemmer with an explicit,
//! hand-audited suffix table is easier to reason about, test, and
//! improve incrementally.
//!
//! Downstream callers who need a full-morphology Czech lemmatizer
//! should reach for a dictionary-based tool (majka, MorfFlex-CZ, the
//! ÚFAL Czech tools); this crate's charter is dictionary-free suffix
//! stripping.
//!
//! # Czech-specific letters
//!
//! The Czech alphabet extends the Latin block with **á, č, ď, é, ě,
//! í, ň, ó, ř, š, ť, ú, ů, ý, ž** (each with an uppercase
//! counterpart). These fall into three groups:
//!
//! * **Haček (caron) consonants** — `č`, `š`, `ž`, `ř`, `ď`, `ť`, `ň`
//!   — orthographic marks for palatalized or affricated consonants.
//!   The phonetic module folds these to Latin placeholders for
//!   Soundex-shape classification.
//! * **Long vowels** — `á`, `é`, `í`, `ó`, `ú`, `ý`, plus `ů` (the
//!   ring-over-u, an alternative long-u spelling that alternates with
//!   `ú` by historical convention). All fold to their short
//!   counterpart for stemming and encoding.
//! * **`ě` (e-caron)** — a Czech-specific vowel that palatalizes the
//!   preceding consonant. Folds to `e` for the algorithms in this
//!   crate.
//!
//! All Czech scalars in this list are UTF-8 multi-byte (2 bytes each
//! in U+0080..=U+07FF for `á/é/í/ó/ú/ý` and 2 bytes for the extended
//! Latin `č/š/ž/ř/ď/ť/ň/ě/ů` in U+0100..=U+017F). All suffix and
//! region arithmetic in [`stemmer`] runs on `Vec<char>` indices —
//! never raw byte offsets — so no scalar is ever sliced apart.
//!
//! # Design choices
//!
//! * **Light suffix-stripping stemmer.** A single-pass longest-match
//!   pass over a hand-audited table of Czech inflectional suffixes
//!   (noun / adjective / verb / possessive), guarded by an RV floor
//!   that mirrors the Snowball-family convention (region starts after
//!   the first vowel-followed-by-consonant). See [`stemmer`] for the
//!   rules and reference-pair coverage.
//! * **~275-word stopword list.** Union of published Czech stopword
//!   collections (the Ranks NL Czech list, the ÚFAL PDT stopword set,
//!   NLTK-adjacent Slavic function-word inventories). Covers personal
//!   / possessive / demonstrative pronouns, prepositions,
//!   conjunctions, particles, high-frequency forms of the copula
//!   *být*, common adverbs, and quantifiers. See [`stopwords`].
//! * **PHONEX-Czech phonetic encoder.** A 4-character Soundex-shape
//!   key with Czech-tuned preprocessing (haček-fold, long-vowel-fold,
//!   silent `h`) over a Czech-tuned classification table. Adapter
//!   name: `"phonex-cs"` — chosen for consistency with the other
//!   Latin-alphabet language packs (`phonex-nl`, `phonex-pt`,
//!   `phonex-es`, `phonex-fr`). See [`phonetic`] for the mapping table
//!   and rationale.
//! * **Simple tokenizer.** Czech is whitespace-and-punctuation
//!   delimited; every letter of the Czech alphabet satisfies
//!   [`char::is_alphanumeric`]. [`CzechTokenizer`] is a transparent
//!   wrapper around
//!   [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer).
//! * **Unicode case fold in `is_stopword`.** The default trait
//!   implementation uses [`str::eq_ignore_ascii_case`] which would
//!   miss uppercase Czech-specific letters (`Č`, `Ě`, `Ř`, `Š`, `Ž`,
//!   `Á`, `É`, `Í`, `Ó`, `Ú`, `Ů`, `Ý`, `Ď`, `Ť`, `Ň`). Czech
//!   overrides
//!   [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//!   with a Unicode lowercase pass.
//! * **Default Unicode collation.**
//!   [`Language::collator`](stringcheese_lang::Language::collator)
//!   returns `None`. Czech collation follows a locale-tailored order
//!   (`ch` sorts as a single digraph after `h`, hačeks sort adjacent
//!   to their bases); callers who need a locale-tailored collator
//!   should reach for `icu_collator`.
//!
//! # Deferred to a follow-up wave
//!
//! * **Canonical Snowball parity.** There is no canonical Snowball
//!   Czech; if the Snowball project ever publishes one, this crate
//!   will adopt it under a new module (`snowball_canonical`) and keep
//!   the light stemmer as `stemmer_light` for callers who want the
//!   current behaviour.
//! * **Aggressive Dolamic-Savoy Czech stemmer.** The published
//!   "aggressive" variant strips derivational suffixes (`-ost`,
//!   `-ství`, `-ismus`) in addition to the inflectional ones and adds
//!   a palatalization step. Would ship as a second stemmer type.
//! * **Consonant alternation (`ruka → ruce`).** Czech morphology
//!   applies velar / palatal alternations to the stem (`k / c / č`,
//!   `h / z / ž`, `ch / š`) in certain case-number cells. The light
//!   stemmer strips the suffix but does not reverse the alternation
//!   — `ruce` (dat./loc. sg. of *ruka*) stems to `ruc` under the bare
//!   `-e` rule, not `ruk`. A palatalization-aware pass is a follow-up.
//! * **Slovak.** Slovak shares much of the Czech morphology and Latin
//!   alphabet but has its own function-word inventory, `-a/-o/-e`
//!   distribution, and additional letters (`ä`, `ĺ`, `ľ`, `ŕ`, `ô`).
//!   Deserves its own `stringcheese-sk` pack rather than a mode
//!   toggle.
//! * **Serbo-Croatian-adjacent packs (Croatian, Bosnian,
//!   Montenegrin).** The `stringcheese-sr` Serbian pack already ships;
//!   Croatian / Bosnian / Montenegrin share the dual-script base but
//!   diverge in vocabulary.
//! * **Full-corpus cross-verification.** The reference-pair test
//!   embeds a hand-traced set that exercises each family of suffixes;
//!   full-corpus cross-verification would require a lexicon.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_cs::CZECH;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(CZECH.code(), "cs");
//! assert_eq!(CZECH.name(), "Czech");
//! assert!(CZECH.is_stopword("a"));
//! assert!(CZECH.is_stopword("je"));
//! assert!(CZECH.is_stopword("NENÍ")); // Unicode case-fold: NENÍ → není.
//! assert!(!CZECH.is_stopword("kniha"));
//!
//! let toks: Vec<&str> = CZECH
//!     .tokenize("Kočka spí na koberci.")
//!     .collect();
//! assert_eq!(toks, ["Kočka", "spí", "na", "koberci"]);
//! ```
//!
//! # Module map
//!
//! - [`stemmer`] — the [`CzechStemmer`] light suffix-stripping stemmer.
//! - [`phonetic`] — [`CzechPhonex`] plus the [`CzechPhonexAdapter`]
//!   the [`Language`](stringcheese_lang::Language) trait hands back.
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`tokenizer`] — the [`CzechTokenizer`] wrapper.
//! - The [`Czech`] type and the [`CZECH`] constant live in this
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
pub use phonetic::{CzechPhonex, CzechPhonexAdapter};
#[cfg(feature = "alloc")]
pub use stemmer::CzechStemmer;
pub use stopwords::STOPWORDS;
pub use tokenizer::CzechTokenizer;

// -----------------------------------------------------------------------
// The Czech language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;
    use alloc::string::String;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::CzechPhonexAdapter;
    use crate::stemmer::CzechStemmer;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::CzechTokenizer;

    /// The Czech language pack.
    ///
    /// Zero-sized; construct as [`Czech`] and reuse the value freely
    /// across threads and calls, or grab the crate-level
    /// [`CZECH`](crate::CZECH) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Czech;

    /// The static [`CzechPhonexAdapter`] [`Czech`] hands back from
    /// [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static PHONEX: CzechPhonexAdapter = CzechPhonexAdapter;

    /// Normalize a Czech string for stopword comparison: lowercase
    /// under default Unicode rules. Handles the Czech-specific letters
    /// `Č Ď Ě Ň Ř Š Ť Ž Á É Í Ó Ú Ů Ý` correctly (each folds to its
    /// lowercase counterpart with no locale tailoring required).
    fn normalize_for_stopword(word: &str) -> String {
        let mut out = String::with_capacity(word.len());
        for c in word.chars() {
            for lc in c.to_lowercase() {
                out.push(lc);
            }
        }
        out
    }

    impl Language for Czech {
        fn code(&self) -> &'static str {
            "cs"
        }

        fn name(&self) -> &'static str {
            "Czech"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        /// Czech-aware stopword membership.
        ///
        /// Overrides the default trait implementation (which uses
        /// [`str::eq_ignore_ascii_case`], missing every uppercase
        /// Czech-specific letter like `Č`, `Ě`, `Ř`, `Š`, `Ž`, `Á`,
        /// `Í`, `Ú`, `Ů`, `Ý`, `Ď`, `Ť`, `Ň`) with a Unicode
        /// lowercase pass — so `NENÍ` and `není` both find `není` in
        /// the stopword list.
        fn is_stopword(&self, word: &str) -> bool {
            let normalized = normalize_for_stopword(word);
            STOPWORDS.contains(&normalized.as_str())
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            CzechStemmer.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(CzechTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&PHONEX)
        }
    }

    /// The singleton [`Czech`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Czech`] every time — the type is zero-sized, so the two forms
    /// are equivalent, but the constant is the intended entry point
    /// and matches the pattern every other `stringcheese-<lang>` pack
    /// follows.
    pub const CZECH: Czech = Czech;
}

#[cfg(feature = "alloc")]
pub use pack::{CZECH, Czech};

// Register into `stringcheese-lang::registry` so callers who look up
// languages dynamically (`registry::language("cs")`) find this pack.
// alloc-gated because the pack constant itself is alloc-gated.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(CZECH);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-cs` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
