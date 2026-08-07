//! Polish language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Polish`] value that carries the Polish stopword list,
//! the [`PolishSnowball`] light suffix-stripping stemmer, the
//! whitespace-and-punctuation [`PolishTokenizer`], and a
//! [`PolishPhonex`] phonetic hookup. Callers grab the singleton
//! [`POLISH`] `const` — no construction ceremony required — and
//! delegate through the [`Language`](stringcheese_lang::Language) trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-pl` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Polish stopword table or
//! the stemmer's code. Callers who need Polish add
//! `stringcheese-pl = "0.1"` to their `Cargo.toml` explicitly.
//!
//! # Polish alphabet and orthography
//!
//! Polish is written in the Latin script with **nine additional
//! diacritic-carrying letters**:
//!
//! * **`ą` (U+0105) / `Ą` (U+0104)** — the nasal vowel /ɔ̃/. Falls to
//!   `a` in the PHONEX phonetic key (nasalisation is a feature the
//!   Soundex-shape encoder collapses), and is treated as a vowel by
//!   the stemmer's RV region calculation.
//! * **`ć` (U+0107) / `Ć` (U+0106)** — the palatalised /tɕ/ affricate.
//!   Encodes to the sibilant class `C` in PHONEX (grouped with `cz`
//!   and `c`).
//! * **`ę` (U+0119) / `Ę` (U+0118)** — the nasal vowel /ɛ̃/. Falls to
//!   `e` in the PHONEX phonetic key. Vowel in the stemmer.
//! * **`ł` (U+0142) / `Ł` (U+0141)** — historically /ɫ/, now /w/ in
//!   most dialects. Encoded as `L` in PHONEX (grouped with `l`).
//! * **`ń` (U+0144) / `Ń` (U+0143)** — the palatalised /ɲ/ nasal.
//!   Encoded as `N` (grouped with `n`) in PHONEX.
//! * **`ó` (U+00F3) / `Ó` (U+00D3)** — historically /oː/, now
//!   phonetically **identical to `u` /u/** in modern Polish. The
//!   PHONEX preprocessor **conflates `ó` with `u`** so that
//!   `Kraków` and `krakuw` produce the same key. Vowel in the stemmer.
//! * **`ś` (U+015B) / `Ś` (U+015A)** — the palatalised /ɕ/ fricative.
//!   Encoded as `S` (grouped with `s`) in PHONEX.
//! * **`ź` (U+017A) / `Ź` (U+0179)** — the palatalised /ʑ/ fricative.
//!   Encoded as `Z` in PHONEX; **merges with `ż`** in the phonetic key
//!   because most Polish sibilant-index heuristics unify the two.
//! * **`ż` (U+017C) / `Ż` (U+017B)** — the retroflex /ʐ/ fricative.
//!   Encoded as `Z` in PHONEX; merges with `ź`.
//!
//! **Polish digraphs.** `sz` (/ʂ/), `cz` (/tʂ/), `rz` (/ʐ/, historically
//! /rʲ/), `ch` (/x/), `dz` (/dz/), `dź` (/dʑ/), `dż` (/dʐ/). The PHONEX
//! preprocessor handles `sz → S`, `cz → C`, `rz → R` (which then
//! encodes to class `6`), and folds `ch → K` (velar). The stemmer does
//! not decompose digraphs — every operation is on `char`s so `sz` is
//! two `char`s in the input but every suffix table entry is spelled
//! character-by-character.
//!
//! # Design choices
//!
//! * **Light suffix-stripping stemmer in the Snowball style.** Polish
//!   is one of the Slavic languages **without a widely-adopted
//!   canonical Snowball algorithm** — the Snowball project has
//!   experimental Polish sources, and the community-standard
//!   `stempel` (Egothor) requires a large trained transducer that is
//!   out of scope for a per-crate offline pack. This module ships a
//!   **light suffix stripper** modeled on the Ukrainian pack's
//!   approach: compute an RV region, then apply a single globally-
//!   longest-match pass over a unified table of nominal / adjectival /
//!   verbal / adverbial inflectional suffixes drawn from Polish
//!   morphology tables. Callers who need a lexicon-based full
//!   lemmatizer should reach for `morfologik-stemming` or `pymorfeusz`.
//!   See [`snowball`] for the algorithm and rule coverage.
//! * **~280-word stopword list.** Union of published Polish stopword
//!   collections (the Snowball / stopwords-iso Polish list plus the
//!   full paradigms of `być` / `mieć` / `móc` / `chcieć`). Covers
//!   personal / possessive / demonstrative pronouns, prepositions,
//!   conjunctions, particles, and the common auxiliary-verb forms.
//!   See [`stopwords`].
//! * **PHONEX-Polish phonetic encoder.** A Soundex-shape 4-character
//!   encoder with Polish-tuned preprocessing (`sz → S`, `cz → C`,
//!   `rz → R`, `ch → K`, `ó → U`, nasal `ą/ę → a/e`, `ż/ź` merge to
//!   `Z`, `ń → N`, `ć → C`, `ś → S`, `ł → L`) and the standard
//!   Soundex classification table adapted for Polish (sibilant class
//!   `S/Z/C`). Adapter name: `"phonex-pl"`. See [`phonetic`] for
//!   the algorithm.
//! * **Simple tokenizer.** Polish is whitespace-and-punctuation
//!   delimited; every accented letter (`ą ć ę ł ń ó ś ź ż` and their
//!   uppercase forms) is alphabetic under Unicode's classification, so
//!   the workspace's default
//!   [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer) already
//!   handles Polish correctly and [`PolishTokenizer`] is a transparent
//!   wrapper around it (a courtesy for callers who match the
//!   language-pack pattern).
//! * **Unicode case fold.** Polish case-folding is well-behaved under
//!   Rust's default [`char::to_lowercase`] — `Ą → ą`, `Ć → ć`,
//!   `Ż → ż`, etc. all work under default rules with no locale
//!   tailoring. The pack overrides
//!   [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//!   to lowercase under this rule so uppercase Polish queries match
//!   the plain lowercase stopword list.
//! * **Default Unicode collation.**
//!   [`Language::collator`](stringcheese_lang::Language::collator)
//!   returns `None`. Polish collation follows Polish-tailored root
//!   ordering (`a < ą < b < c < ć < …`) which requires a
//!   locale-specific tailoring; callers who need it should reach for
//!   `icu_collator` via a [`stringcheese_lang::Collator`] impl of
//!   their own.
//!
//! # Deferred to a follow-up wave
//!
//! * **Canonical Snowball parity.** If the Snowball project ever
//!   publishes a stable canonical `polish.sbl` with a shipped
//!   `voc.txt` / `output.txt` reference pair, this crate will adopt
//!   it under a new module (`snowball_canonical`) and keep the light
//!   stemmer as `snowball_light` for callers who want the current
//!   behaviour.
//! * **Full-corpus cross-verification.** The reference-pair test
//!   embeds a hand-traced subset that exercises each family of
//!   suffixes; full-corpus cross-verification against a Polish
//!   lexicon is a follow-up.
//! * **Morfologik / Stempel dictionary lemmatization.** Reducing
//!   `lepszy → dobry`, `poszedł → iść` needs a lexicon; the shipped
//!   stemmer is a suffix-stripper only.
//! * **Polish-tailored collator.** Would compose the ICU CLDR Polish
//!   tailoring (`a < ą`, `c < ć`, `l < ł`, `n < ń`, `o < ó`,
//!   `s < ś`, `z < ź < ż`). Out of scope for the initial drop.
//! * **Métaphone Polish** — a variable-length parallel encoder with
//!   better discrimination; heavier to reference-test.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_pl::POLISH;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(POLISH.code(), "pl");
//! assert_eq!(POLISH.name(), "Polish");
//! assert!(POLISH.is_stopword("i"));
//! assert!(POLISH.is_stopword("NIE")); // Unicode case-fold.
//! assert!(!POLISH.is_stopword("książka"));
//!
//! let toks: Vec<&str> = POLISH
//!     .tokenize("Kot śpi na macie.")
//!     .collect();
//! assert_eq!(toks, ["Kot", "śpi", "na", "macie"]);
//! ```
//!
//! # Module map
//!
//! - [`snowball`] — the [`PolishSnowball`] light stemmer.
//! - [`phonetic`] — [`PolishPhonex`] plus the [`PolishPhonexAdapter`]
//!   the [`Language`](stringcheese_lang::Language) trait hands back.
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`tokenizer`] — the [`PolishTokenizer`] wrapper.
//! - The [`Polish`] type and the [`POLISH`] constant live in this
//!   crate's root.

#![cfg_attr(not(feature = "std"), no_std)]
// `deny` rather than `forbid` because the `stringcheese_lang::
// register_language!` invocation below expands to a `linkme`-backed
// static whose implementation is `unsafe`-tagged (safe in practice
// — that's linkme's whole design — but flagged by the
// `unsafe_code` lint). The macro emits an explicit
// `#[allow(unsafe_code)]` at the sole registration site; the rest
// of this crate is still lint-enforced no-`unsafe`.
#![deny(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
pub mod phonetic;
#[cfg(feature = "alloc")]
pub mod snowball;
pub mod stopwords;
pub mod tokenizer;

#[cfg(all(test, feature = "std", not(target_family = "wasm")))]
mod properties;

#[cfg(feature = "alloc")]
pub use phonetic::{PolishPhonex, PolishPhonexAdapter};
#[cfg(feature = "alloc")]
pub use snowball::PolishSnowball;
pub use stopwords::STOPWORDS;
pub use tokenizer::PolishTokenizer;

// -----------------------------------------------------------------------
// The Polish language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;
    use alloc::string::String;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::PolishPhonexAdapter;
    use crate::snowball::PolishSnowball;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::PolishTokenizer;

    /// The Polish language pack.
    ///
    /// Zero-sized; construct as [`Polish`] and reuse the value freely
    /// across threads and calls, or grab the crate-level
    /// [`POLISH`](crate::POLISH) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Polish;

    /// The static [`PolishPhonexAdapter`] [`Polish`] hands back from
    /// [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static PHONEX: PolishPhonexAdapter = PolishPhonexAdapter;

    /// Normalize a Polish string for stopword comparison: lowercase
    /// under default Unicode rules. Polish case-fold is well-behaved
    /// under default Unicode rules (`Ą → ą`, `Ć → ć`, `Ż → ż`, etc.) —
    /// no locale tailoring required.
    fn normalize_for_stopword(word: &str) -> String {
        let mut out = String::with_capacity(word.len());
        for c in word.chars() {
            for lc in c.to_lowercase() {
                out.push(lc);
            }
        }
        out
    }

    impl Language for Polish {
        fn code(&self) -> &'static str {
            "pl"
        }

        fn name(&self) -> &'static str {
            "Polish"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        /// Unicode-aware stopword membership.
        ///
        /// Overrides the default trait implementation (which uses
        /// [`str::eq_ignore_ascii_case`], missing the case-fold for
        /// Polish's diacritic-carrying uppercase letters `Ą Ć Ę Ł Ń Ó
        /// Ś Ź Ż`) with a Unicode lowercase pass — so `NIE` and `nie`
        /// both find `nie`, and `ŻE` and `że` both find `że`, in the
        /// stopword list.
        fn is_stopword(&self, word: &str) -> bool {
            let normalized = normalize_for_stopword(word);
            STOPWORDS.contains(&normalized.as_str())
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            PolishSnowball.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(PolishTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&PHONEX)
        }
    }

    /// The singleton [`Polish`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Polish`] every time — the type is zero-sized, so the two forms
    /// are equivalent, but the constant is the intended entry point
    /// and matches the pattern every other `stringcheese-<lang>` pack
    /// follows.
    pub const POLISH: Polish = Polish;
}

#[cfg(feature = "alloc")]
pub use pack::{POLISH, Polish};

// Opt this pack into the shared `stringcheese_lang::registry` — a
// distributed slice populated at link time so callers picking a
// language by BCP-47 code at runtime
// (`stringcheese_lang::registry::language("pl")`) find Polish without
// naming the crate. See `stringcheese_lang::registry` for the design
// and trade-offs.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(POLISH);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-pl` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
