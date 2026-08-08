//! Norwegian (Nynorsk) language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Nynorsk`] value that carries the Nynorsk stopword
//! list, the [`NynorskSnowball`] stemmer, the whitespace-and-
//! punctuation [`NynorskTokenizer`], and a [`NynorskPhonex`] phonetic
//! hookup. Callers grab the singleton [`NYNORSK`] `const` — no
//! construction ceremony required — and delegate through the
//! [`Language`](stringcheese_lang::Language) trait.
//!
//! # Nynorsk-only
//!
//! Norway has two official written standards:
//!
//! * **Bokmål** ("book tongue") — descended from written Danish, the
//!   more common of the two by usage (~85–90% of writers). Covered by
//!   the sibling [`stringcheese-no`](https://crates.io/crates/stringcheese-no)
//!   crate, which registers the BCP-47 tag `"nb"`.
//! * **Nynorsk** ("new Norwegian") — synthesized in the 19th century
//!   by Ivar Aasen from western Norwegian dialects; used by ~10–15% of
//!   writers and mandated for a share of state broadcasting, school
//!   materials, and official documents. This pack targets Nynorsk.
//!
//! The BCP-47 tag registered by this pack is **`"nn"`** — the
//! IANA-registered code for Norwegian Nynorsk. Like the Bokmål sibling,
//! this pack does *not* register the macrolanguage tag `"no"` — the
//! two standards live in separate BCP-47 buckets so callers can pick
//! them independently.
//!
//! # Shared Snowball stemmer
//!
//! The [`Snowball` project](https://snowballstem.org/algorithms/norwegian/stemmer.html)
//! ships a single Norwegian stemmer (`norwegian.sbl`) whose author
//! describes it as suitable for **both Bokmål and Nynorsk**. The suffix
//! inventory (`-en` / `-et` / `-ene` / `-heter` / `-ede` / `-ande` /
//! `-ende` / …) covers the shared Bokmål-Nynorsk paradigm because the
//! two standards share substantial nominal, adjectival, and verbal
//! morphology; the differences are mostly lexical (`ikkje` vs. `ikke`,
//! `eg` vs. `jeg`) and orthographic (a-infinitives `å skriva` /
//! `å arbeida` in Nynorsk versus e-infinitives `å skrive` /
//! `å arbeide` in Bokmål) rather than in the suffix system itself. The
//! [`NynorskSnowball`] port here is byte-identical to the Bokmål
//! sibling's [`stringcheese-no`'s `NorwegianSnowball`](
//! https://docs.rs/stringcheese-no) — the same algorithm carried under
//! a Nynorsk-flavoured name for consistency with the pack pattern.
//!
//! # Norwegian-specific letters
//!
//! The Norwegian alphabet extends the 26-letter Latin base with three
//! letters at the end: `æ` (`ash`), `ø` (`o-slash`), and `å` (`a-ring`).
//! All three occur in high-frequency Nynorsk vocabulary (`vera` "to be",
//! `ønske` "to wish", `få` "get"). The Snowball vowel set includes all
//! three, and the PHONEX preprocessor folds them for phonetic key
//! collapse (see [`phonetic`]).
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-nn` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Nynorsk stopword table
//! or the Snowball stemmer's code. Callers who need Nynorsk add
//! `stringcheese-nn = "0.1"` to their `Cargo.toml` explicitly.
//!
//! The implementation choices are deliberately opinionated:
//!
//! * **Snowball Norwegian stemmer.** Martin Porter's Snowball
//!   Norwegian algorithm, documented at
//!   <https://snowballstem.org/algorithms/norwegian/stemmer.html>.
//!   The upstream algorithm covers Nynorsk; this crate ports the same
//!   three-step cascade documented in [`snowball`].
//! * **~100-word Nynorsk-tuned stopword list.** Drawn from the classic
//!   Nynorsk function-word inventory (pronouns `eg` / `du` / `ho` /
//!   `han` / `me` / `de` / `dei`; articles `ein` / `ei` / `eit`;
//!   negation `ikkje`; interrogatives `kva` / `kvifor` / `korleis` /
//!   `kvar` / `kven`; adverbs `mykje` / `difor` / `so` / `no`;
//!   copula/auxiliary paradigms `vera` / `har` / `vil` / `kan` /
//!   `skal` / `må` / `blir`). Where Nynorsk and Bokmål share a
//!   function word (most prepositions and conjunctions), the shared
//!   form is included.
//! * **PHONEX-Norwegian phonetic encoder.** A Soundex-shaped
//!   4-character encoder with Norwegian-tuned preprocessing (`skj →
//!   S`, `sk` before front vowels → `S`, `kj → C`, `k` before front
//!   vowels → `C`, `ch → S`, plus the `å → o` / `æ → e` / `ø → e`
//!   letter folds) and the standard PHONEX classification table. See
//!   [`phonetic`] for the algorithm. Nynorsk shares the same
//!   phonological cluster set as Bokmål, so the preprocessing rules
//!   apply verbatim; the adapter name is `"phonex-nn"` so a caller
//!   picking the encoder by name can distinguish the pack.
//! * **Simple tokenizer.** Nynorsk, like Bokmål, is whitespace-and-
//!   punctuation delimited and requires no elision-splitting pass —
//!   [`NynorskTokenizer`] is a transparent wrapper around
//!   [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer).
//! * **Default Unicode collation.**
//!   [`Language::collator`](stringcheese_lang::Language::collator)
//!   returns `None`. Norwegian collation (both standards) places `æ`,
//!   `ø`, `å` after `z` (in that order) — the standard Unicode root
//!   does not; callers who need a locale-tailored collator should
//!   reach for `icu_collator` via a [`stringcheese_lang::Collator`]
//!   impl of their own.
//!
//! # Deferred to a follow-up wave
//!
//! * **Danish (`stringcheese-da`) and Icelandic (`stringcheese-is`).**
//!   The other North Germanic Latin-script languages; Danish shares
//!   `æ ø å` with Norwegian but has its own Snowball stemmer, and
//!   Icelandic has additional letters `þ ð` and a much richer
//!   inflectional morphology that would need its own algorithm.
//! * **Full-corpus cross-verification.** The Snowball project
//!   distributes `voc.txt` / `output.txt` reference files with tens
//!   of thousands of pairs; the
//!   [`tests/snowball_reference.rs`](../../tests/snowball_reference.rs)
//!   test embeds a *subset* that exercises every step's happy path
//!   and each cascading rule. Full-corpus cross-verification is a
//!   follow-up.
//! * **Métaphone Norwegian.** A parallel encoder with a variable-
//!   length key; better for record-linkage precision, but heavier to
//!   reference-test and out of scope for the initial drop.
//! * **Compound-noun splitting.** Nynorsk, like Bokmål and German,
//!   productively compounds nouns (`fotball + lag → fotballag`).
//!   Splitting them needs a compound-noun dictionary and is not part
//!   of the Snowball algorithm.
//! * **Nynorsk-only lexicon-driven lemmatization.** A handful of
//!   Nynorsk verbs are irregular in ways the suffix stripper cannot
//!   capture (`gjeng` / `gjekk` / `gått` for `gå`, `sjå` / `såg` /
//!   `sett` for `sjå`); reducing them requires a lexicon.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_nn::NYNORSK;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(NYNORSK.code(), "nn");
//! assert_eq!(NYNORSK.name(), "Norwegian Nynorsk");
//! assert!(NYNORSK.is_stopword("og"));
//! assert!(NYNORSK.is_stopword("ikkje"));
//! assert!(NYNORSK.is_stopword("kva"));
//! assert!(!NYNORSK.is_stopword("fisk"));
//!
//! let toks: Vec<&str> = NYNORSK
//!     .tokenize("Katten søv på matta.")
//!     .collect();
//! assert_eq!(toks, ["Katten", "søv", "på", "matta"]);
//! ```
//!
//! # Module map
//!
//! - [`snowball`] — the [`NynorskSnowball`] stemmer.
//! - [`phonetic`] — [`NynorskPhonex`] plus the
//!   [`NynorskPhonexAdapter`] the
//!   [`Language`](stringcheese_lang::Language) trait hands back.
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`tokenizer`] — the [`NynorskTokenizer`] wrapper.
//! - The [`Nynorsk`] type and the [`NYNORSK`] constant live in this
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
pub use phonetic::{NynorskPhonex, NynorskPhonexAdapter};
#[cfg(feature = "alloc")]
pub use snowball::NynorskSnowball;
pub use stopwords::STOPWORDS;
pub use tokenizer::NynorskTokenizer;

// -----------------------------------------------------------------------
// The Nynorsk language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::NynorskPhonexAdapter;
    use crate::snowball::NynorskSnowball;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::NynorskTokenizer;

    /// The Norwegian (Nynorsk) language pack.
    ///
    /// Zero-sized; construct as [`Nynorsk`] and reuse the value freely
    /// across threads and calls, or grab the crate-level
    /// [`NYNORSK`](crate::NYNORSK) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Nynorsk;

    /// The static [`NynorskPhonexAdapter`] [`Nynorsk`] hands back
    /// from [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static PHONEX: NynorskPhonexAdapter = NynorskPhonexAdapter;

    impl Language for Nynorsk {
        fn code(&self) -> &'static str {
            "nn"
        }

        fn name(&self) -> &'static str {
            "Norwegian Nynorsk"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            NynorskSnowball.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(NynorskTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&PHONEX)
        }
    }

    /// The singleton [`Nynorsk`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Nynorsk`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other `stringcheese-<lang>`
    /// pack follows.
    pub const NYNORSK: Nynorsk = Nynorsk;
}

#[cfg(feature = "alloc")]
pub use pack::{NYNORSK, Nynorsk};

// Opt this pack into the shared `stringcheese_lang::registry` — a
// distributed slice populated at link time so callers picking a
// language by BCP-47 code at runtime
// (`stringcheese_lang::registry::language("nn")`) find Norwegian
// Nynorsk without naming the crate. See `stringcheese_lang::registry`
// for the design and trade-offs.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(NYNORSK);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-nn` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
