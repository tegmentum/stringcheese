//! Finnish language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Finnish`] value that carries the Finnish stopword
//! list, the [`FinnishSnowball`] stemmer, the whitespace-and-punctuation
//! [`FinnishTokenizer`], and a [`FinnishPhonex`] phonetic hookup.
//! Callers grab the singleton [`FINNISH`] `const` — no construction
//! ceremony required — and delegate through the
//! [`Language`](stringcheese_lang::Language) trait.
//!
//! # First Uralic (non-Indo-European) language pack
//!
//! Every previous `stringcheese-<lang>` pack has covered a language in
//! either the Indo-European family (English, German, French, Spanish,
//! Portuguese, Dutch, Polish, Russian, Ukrainian, Bulgarian, Serbian,
//! Czech, Slovak, Vietnamese-with-Sinitic-loanwords, Persian) or a
//! separate family that still fits the classic Latin / Cyrillic /
//! Arabic script tradition (Japanese, Chinese, Arabic, Hebrew,
//! Turkish). Finnish is the workspace's first **Uralic** language —
//! a family whose morphology, phonology, and vocabulary sit outside
//! the shared Indo-European inheritance. The concrete consequences
//! for a language pack:
//!
//! * **Agglutinative morphology.** Finnish stacks case, number,
//!   possessive, and clitic-particle suffixes on a stem — a single
//!   orthographic word can carry 5–10 morphemes (`talossanikin`
//!   "in my house also" = `talo` + `ssa` + `ni` + `kin`). The Snowball
//!   stemmer runs a fixed six-step cascade rather than a single
//!   unified suffix table; each step strips at most one morpheme
//!   from its own category.
//! * **Vowel harmony.** Finnish vowels split into two disjoint sets
//!   that never co-occur inside a native word: **back** vowels `a`,
//!   `o`, `u` and **front** vowels `ä`, `ö`, `y`. The vowels `e`
//!   and `i` are **neutral** — they appear with either set. Every
//!   harmony-sensitive suffix comes in a back / front pair
//!   (`-ssa` / `-ssä` inessive, `-lla` / `-llä` adessive, `-ko` /
//!   `-kö` question particle, …). The stemmer's suffix inventory
//!   lists both variants explicitly, so the literal-match check
//!   *is* the harmony check.
//! * **Fifteen grammatical cases.** Finnish inflects nouns for
//!   inessive, elative, illative, adessive, ablative, allative,
//!   essive, translative, partitive, genitive, accusative,
//!   nominative, instructive, comitative, and abessive. The stemmer
//!   covers the productive ones; the archaic instructive / comitative
//!   / abessive are best-effort. See [`snowball`].
//! * **Consonant gradation.** Certain stem-final consonants alternate
//!   between a strong grade (`kk`, `pp`, `tt`) and a weak grade (`k`,
//!   `p`, `t`) before some suffixes. The tidy-up step undoubles
//!   trailing geminates as a partial reversal; full gradation
//!   requires a lexicon (many stems don't gradate).
//! * **The letter `y`.** Finnish `y` is a **front rounded vowel** /y/
//!   (like German ü), *never* an English-style palatal glide /j/.
//!   Every vowel-class predicate treats `y` as a vowel — a critical
//!   detail for the region computation and the plural-strip
//!   predicate. Getting this wrong quietly breaks stemming for
//!   `syksy` "autumn", `kyllä` "yes", `hyvä` "good", and most of
//!   the front-vowel lexicon.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-fi` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Finnish stopword table
//! or the Snowball Finnish stemmer's code. Callers who need Finnish
//! add `stringcheese-fi = "0.1"` to their `Cargo.toml` explicitly.
//!
//! The implementation choices are deliberately opinionated:
//!
//! * **Snowball Finnish stemmer.** The algorithm documented at
//!   <https://snowballstem.org/algorithms/finnish/stemmer.html> —
//!   one of the longer Snowball entries because of the language's
//!   agglutinative morphology. The stemmer runs a six-step cascade:
//!   particles → possessives → cases → other derivational →
//!   plurals → tidy-up. See [`snowball`] for the algorithm's rules.
//! * **~170-word stopword list.** The intersection of NLTK's
//!   `finnish` list and the Snowball project's Finnish stopword
//!   collection. Covers personal / demonstrative / interrogative
//!   pronouns (with the highest-frequency case-inflected forms as
//!   separate entries, since NLTK's canonical Finnish list carries
//!   them explicitly), conjunctions, negation-verb forms, common
//!   postpositions and adverbs, and the high-frequency conjugations
//!   of *olla* "to be". See [`stopwords`].
//! * **PHONEX-Finnish phonetic encoder.** A light 4-character
//!   Soundex-shape encoder with Finnish-tuned preprocessing (long-
//!   vowel and long-consonant collapse, `ä → a`, `ö → o`, `å → o`
//!   fold, `y` treated as a vowel). Chosen small because Finnish
//!   orthography is already almost phonetic — the modern alphabet
//!   was standardized to be a near-1:1 phoneme mapping. See
//!   [`phonetic`].
//! * **Simple tokenizer.** Finnish has no clitic elision like French
//!   nor compound morpheme boundaries a tokenizer could see. Its
//!   orthography is whitespace-and-punctuation delimited, and the
//!   three Finnish special letters (`ä`, `ö`, `å`) are alphabetic
//!   under Unicode's classification. [`FinnishTokenizer`] is a
//!   transparent wrapper around
//!   [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer); the
//!   Snowball stemmer handles suffix stripping at the stem step.
//! * **Default Unicode case-fold.** Finnish has *no* locale-specific
//!   case-fold quirks — unlike Turkish (dotted / dotless `I`),
//!   Finnish's `ä ↔ Ä`, `ö ↔ Ö`, `å ↔ Å` mappings are all one-scalar
//!   Unicode defaults. The pack's [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//!   implementation nonetheless overrides the default trait method
//!   (which uses [`str::eq_ignore_ascii_case`]) to apply Unicode
//!   case-fold, so inputs like `SINÄ`, `MYÖS`, `ÄITI` match the
//!   lowercase stopword entries.
//!
//! # Deferred to a follow-up wave
//!
//! * **`stringcheese-et` (Estonian).** The other major Uralic
//!   language of the Baltic region. Estonian and Finnish share
//!   substantial morphological structure — case-heavy agglutination,
//!   vowel harmony (Estonian has lost much of it but the framework
//!   survives), consonant gradation. An Estonian pack would branch
//!   off this one with adapted suffix tables (Estonian's fourteen
//!   cases overlap the Finnish fifteen with different names and
//!   forms) and its own diacritic set (`ä ö õ ü`, no `å`).
//! * **Sami packs.** The Sami languages (Northern Sami, Lule Sami,
//!   Skolt Sami, Inari Sami, …) are the third Uralic branch present
//!   in Fennoscandia. Northern Sami (`se`) has the most speakers and
//!   the most standardized orthography (nine additional letters
//!   including `č`, `š`, `ž`, `đ`, `ŋ`, `ŧ`); each Sami language
//!   would want its own pack because the morphological rules and
//!   vocabulary diverge sharply.
//! * **Full-corpus reference cross-verification.** The Snowball
//!   project distributes `voc.txt` / `output.txt` reference files
//!   with tens of thousands of pairs; the shipped
//!   [`tests/snowball_reference.rs`](../../tests/snowball_reference.rs)
//!   test embeds a subset that exercises every step's happy path.
//! * **Lemmatization.** Reducing `menen → mennä`, `annoin → antaa`
//!   requires a lexicon (Finnish's productive consonant gradation
//!   and stem-vowel alternation are not surface-invertible without
//!   one); the shipped stemmer is a suffix-stripping algorithm only.
//! * **Lexicon-driven consonant-gradation reversal.** The shipped
//!   tidy-up step undoubles trailing geminates (`hakk → hak`), but
//!   the full alternation (`jalka` ↔ `jalan`, `käsi` ↔ `käden`,
//!   `nk` ↔ `ng`) requires a lexicon.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_fi::FINNISH;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(FINNISH.code(), "fi");
//! assert_eq!(FINNISH.name(), "Finnish");
//! assert!(FINNISH.is_stopword("ja"));
//! assert!(FINNISH.is_stopword("SINÄ"));  // Unicode case-fold.
//! assert!(!FINNISH.is_stopword("kirjasto"));
//! assert_eq!(FINNISH.stem("talossani"), "talo");
//!
//! let toks: Vec<&str> = FINNISH
//!     .tokenize("Hei, maailma! Helsinki on kaunis.")
//!     .collect();
//! assert_eq!(toks, ["Hei", "maailma", "Helsinki", "on", "kaunis"]);
//! ```
//!
//! # Module map
//!
//! - [`snowball`] — the [`FinnishSnowball`] stemmer.
//! - [`phonetic`] — [`FinnishPhonex`] plus the
//!   [`FinnishPhonexAdapter`] the
//!   [`Language`](stringcheese_lang::Language) trait hands back.
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`tokenizer`] — the [`FinnishTokenizer`] wrapper.
//! - The [`Finnish`] type and the [`FINNISH`] constant live in this
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
pub use phonetic::{FinnishPhonex, FinnishPhonexAdapter};
#[cfg(feature = "alloc")]
pub use snowball::FinnishSnowball;
pub use stopwords::STOPWORDS;
pub use tokenizer::FinnishTokenizer;

// -----------------------------------------------------------------------
// The Finnish language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::FinnishPhonexAdapter;
    use crate::snowball::FinnishSnowball;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::FinnishTokenizer;

    /// The Finnish language pack.
    ///
    /// Zero-sized; construct as [`Finnish`] and reuse the value freely
    /// across threads and calls, or grab the crate-level
    /// [`FINNISH`](crate::FINNISH) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Finnish;

    /// The static [`FinnishPhonexAdapter`] [`Finnish`] hands back from
    /// [`phonetic_encoder`](Language::phonetic_encoder).
    static PHONEX: FinnishPhonexAdapter = FinnishPhonexAdapter;

    impl Language for Finnish {
        fn code(&self) -> &'static str {
            "fi"
        }

        fn name(&self) -> &'static str {
            "Finnish"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        /// Unicode-case-fold stopword membership.
        ///
        /// Overrides the default trait implementation (which uses
        /// [`str::eq_ignore_ascii_case`]) so that stopwords carrying
        /// non-ASCII letters (`ä`, `ö`) match their uppercase forms.
        /// Finnish has no locale-specific case-fold quirks — the
        /// default Unicode fold is correct for every Finnish letter.
        fn is_stopword(&self, word: &str) -> bool {
            let lowered: alloc::string::String = word
                .chars()
                .flat_map(|c| c.to_lowercase().collect::<alloc::vec::Vec<_>>())
                .collect();
            STOPWORDS.iter().any(|s| *s == lowered)
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            FinnishSnowball.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(FinnishTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&PHONEX)
        }
    }

    /// The singleton [`Finnish`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Finnish`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other `stringcheese-<lang>`
    /// pack follows.
    pub const FINNISH: Finnish = Finnish;
}

#[cfg(feature = "alloc")]
pub use pack::{FINNISH, Finnish};

// Opt this pack into the shared `stringcheese_lang::registry` — a
// distributed slice populated at link time so callers picking a
// language by BCP-47 code at runtime
// (`stringcheese_lang::registry::language("fi")`) find Finnish without
// naming the crate. See `stringcheese_lang::registry` for the design
// and trade-offs.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(FINNISH);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-fi` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
