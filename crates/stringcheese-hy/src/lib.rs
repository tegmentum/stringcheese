//! Armenian (Eastern Armenian) language pack for the StringCheese
//! toolkit.
//!
//! A zero-sized [`Armenian`] value that carries the Armenian stopword
//! list, the [`ArmenianStemmer`] longest-match suffix stripper, the
//! whitespace-and-punctuation [`ArmenianTokenizer`], and an
//! [`ArmenianPhonex`] Soundex-shape phonetic hookup. Callers grab
//! the singleton [`ARMENIAN`] `const` — no construction ceremony
//! required — and delegate through the
//! [`Language`](stringcheese_lang::Language) trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-hy` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Armenian stopword table
//! or the Armenian stemmer's code. Callers who need Armenian add
//! `stringcheese-hy = "0.1"` to their `Cargo.toml` explicitly.
//!
//! # First Armenian-script pack
//!
//! This is the first `stringcheese-<lang>` implementation for a
//! script written in the Armenian alphabet. It exists to validate
//! the shape of the [`Language`](stringcheese_lang::Language) trait
//! on the Armenian block, and to prove that StringCheese's
//! byte/character-sequence processing model handles Armenian-script
//! inputs without any special-case machinery.
//!
//! ## Armenian, an Indo-European isolate branch
//!
//! Armenian is an **Indo-European isolate branch** — a family of one
//! within Indo-European, in the same "single-member branch"
//! typological company as **Greek** (`stringcheese-el`) and Albanian.
//! It is not Slavic (unlike `stringcheese-ru` / `-uk` / `-be` /
//! `-bg` / `-mk` / `-sr`), not Germanic (unlike `stringcheese-en` /
//! `-de` / `-nl` / `-sv` / …), not Romance (unlike `stringcheese-es`
//! / `-fr` / `-it` / …), and not Iranian (unlike `stringcheese-fa`),
//! though contact with Iranian languages has left a large loanword
//! layer.
//!
//! ## Armenian alphabet
//!
//! The Armenian alphabet has **39 letters** (36 letters of the
//! original 5th-century inventory devised by Mesrop Mashtots, plus
//! `օ` and `ֆ` added in the 12th century, plus the ligature `և`).
//! It is **case-sensitive** — every letter has a distinct upper
//! (`Ա-Ֆ`) and lower (`ա-ֆ`) form:
//!
//! ```text
//! Ա Բ Գ Դ Ե Զ Է Ը Թ Ժ Ի Լ Խ Ծ Կ Հ Ձ Ղ Ճ Մ Յ Ն Շ Ո Չ Պ Ջ Ռ Ս Վ Տ Ր Ց Ւ Փ Ք Օ Ֆ
//! ա բ գ դ ե զ է ը թ ժ ի լ խ ծ կ հ ձ ղ ճ մ յ ն շ ո չ պ ջ ռ ս վ տ ր ց ւ փ ք օ ֆ
//! ```
//!
//! Rust's default [`char::to_lowercase`] fold handles the case
//! mapping correctly (no locale-specific quirks — Armenian is not
//! Turkish).
//!
//! ## Armenian-specific invariants
//!
//! Armenian is a **left-to-right script**. Unlike Arabic or Hebrew
//! (the RTL packs), there are no display-order surprises. Two
//! things to remember:
//!
//! * **Every letter is 2 bytes in UTF-8.** The Armenian block sits
//!   entirely inside U+0530..=U+058F, which falls in UTF-8's
//!   2-byte range (U+0080..=U+07FF). A word like `"Երևան"`
//!   (5 characters — the `և` ligature is a single scalar) is
//!   10 bytes. Any code that mixes byte offsets with
//!   character-boundary logic will silently corrupt token or
//!   suffix boundaries. This crate operates exclusively on
//!   `Vec<char>` and [`str::chars`] iteration — never raw byte
//!   offsets.
//! * **Armenian-specific punctuation.** Armenian has its own
//!   punctuation set separate from ASCII: `։` (U+0589 full stop),
//!   `՝` (U+055D comma), `՞` (U+055E question mark), `՜` (U+055C
//!   exclamation mark), and `֊` (U+058A hyphen). All are
//!   classified as Unicode punctuation (`Po` / `Pd`) so the
//!   default tokenizer splits on them correctly.
//! * **Ech-yiwn ligature.** Armenian has a single-scalar ligature
//!   `և` (U+0587, small ligature ech-yiwn) that spells the
//!   conjunction "and" as a single character. Classified as a
//!   lowercase letter (`Ll`); the pack normalizes the two-letter
//!   `եւ` spelling to `և` at every entry point so both spellings
//!   stem, tokenize, and encode identically.
//! * **The `ու` digraph.** Armenian writes the vowel /u/ as
//!   `ո` (o) + `ւ` (w) — a **two-scalar digraph**. The phonetic
//!   encoder recognizes this pattern and folds it to a single
//!   Latin `U`.
//!
//! # Design choices
//!
//! * **Longest-match suffix stripper stemmer.** Armenian has no
//!   widely-published Snowball algorithm. The shipped stemmer is a
//!   hand-audited longest-match suffix stripper that iterates to
//!   convergence, covering the seven Eastern Armenian case suffixes
//!   (nominative-definite `-ը` / `-ն`, genitive `-ի`, dative `-ին`,
//!   ablative `-ից`, instrumental `-ով`, locative `-ում`), the two
//!   plural markers (`-եր` monosyllabic base, `-ներ` polysyllabic
//!   base), their combinations (`-ների`, `-ներով`, `-ներում`,
//!   `-ներից`, `-ներին`, `-երի`, `-երով`, `-երում`, `-երից`,
//!   `-երին`), and the aorist personal endings (`-եցի` 1sg, `-եցիր`
//!   2sg, `-եց` 3sg, `-եցինք` 1pl, `-եցիք` 2pl, `-եցին` 3pl). A
//!   min-stem-length-of-2 guard blocks over-stripping of short base
//!   words. See [`stemmer`] for the algorithm.
//! * **~55-entry stopword list.** Common Eastern Armenian pronouns,
//!   demonstratives, interrogatives, conjunctions, prepositions,
//!   particles, the copula (`եմ` / `ես` / `է` / `ենք` / `եք` / `են`),
//!   the negator (`չէ` / `չեմ` / …), and high-frequency adverbs /
//!   quantifiers. Entries are stored lowercase; the `is_stopword`
//!   override applies the case fold before comparison. See
//!   [`stopwords`].
//! * **PHONEX-Armenian phonetic encoder.** A 4-character Soundex-shape
//!   key computed over a Hübschmann-Meillet-inspired Armenian → Latin
//!   fold that collapses aspiration contrasts (labial `պ / փ / բ → P`,
//!   dental `տ / թ / դ → T`, velar `կ / ք / գ → K`, dental affricate
//!   `ծ / ց / ձ → C`, palato-alveolar affricate `ճ / չ / ջ → J`) and
//!   recognizes the `ու → U` digraph and the `և → EV` ligature.
//!   Adapter name: `"phonex-hy"`. See [`phonetic`].
//! * **Simple tokenizer.** Armenian orthography uses ASCII spaces
//!   between orthographic words, and every letter of the modern
//!   Armenian alphabet satisfies [`char::is_alphanumeric`]. Armenian
//!   punctuation (`։ ՝ ՞ ՜ ֊`) is classified as Unicode punctuation
//!   and splits under the default splitter, so [`ArmenianTokenizer`]
//!   is a transparent wrapper around
//!   [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer). See
//!   [`tokenizer`].
//! * **Armenian-aware case fold.** Uses Rust's default
//!   [`char::to_lowercase`]. The `is_stopword` override applies the
//!   same fold at the call site so uppercase queries match the
//!   lowercase stopword list.
//! * **Default Unicode collation.** Armenian sorts under CLDR's
//!   Armenian tailoring. This pack does not carry the CLDR tailoring
//!   data;
//!   [`Language::collator`](stringcheese_lang::Language::collator)
//!   returns `None`. Callers who need locale-aware Armenian collation
//!   should reach for `icu_collator`.
//!
//! # Deferred to a follow-up wave
//!
//! * **Western Armenian (`stringcheese-hyw`).** Western Armenian
//!   (spoken across the Armenian diaspora) has a distinct phonology
//!   (the plain / voiced stop distinction of Eastern Armenian
//!   collapses — Western reads `բ` as /pʰ/ where Eastern reads it as
//!   /b/) and some morphological divergence (present tense uses
//!   `կը` + finite form). A dedicated `stringcheese-hyw` sibling
//!   with a Western-aware stopword list and stemmer would deserve
//!   its own pack.
//! * **Classical Armenian / Grabar (`stringcheese-xcl`).** The
//!   5th-century literary language preserved in the New Testament
//!   translations uses a much richer morphology (7 cases with
//!   distinct singular / plural forms, an aorist / imperfect /
//!   perfect distinction, participles that inflect for case). A
//!   dedicated `stringcheese-xcl` (ISO 639-3 for Classical Armenian)
//!   sibling with a Grabar-aware suffix cascade would deserve its
//!   own pack.
//! * **Lexicon-driven lemmatization.** Reducing surface forms like
//!   `գնացի → գնալ` (I went → to go) needs a verb-conjugation
//!   lexicon, not a suffix-stripping algorithm. Deferred.
//! * **ISO 9985 / BGN-PCGN transliteration adapters.** ISO 9985 is
//!   the ISO standard for scholarly Armenian romanization (uses
//!   diacritics like `ë ə́ ǰ č̣`); BGN/PCGN is the US/UK geographic
//!   names board romanization used on maps. Both are readable
//!   transliterations rather than phonetic keys; future
//!   `stringcheese-hy-iso9985` / `stringcheese-hy-bgnpcgn` siblings
//!   could expose them alongside the PHONEX-shape default.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_hy::ARMENIAN;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(ARMENIAN.code(), "hy");
//! assert_eq!(ARMENIAN.name(), "Armenian");
//! assert!(ARMENIAN.is_stopword("և"));
//! assert!(ARMENIAN.is_stopword("ԵՒ"));   // Armenian case-fold: ԵՒ → եւ → և.
//! assert!(ARMENIAN.is_stopword("եւ"));   // Two-letter spelling normalizes to `և`.
//! assert!(!ARMENIAN.is_stopword("մայր"));
//! assert_eq!(ARMENIAN.stem("մայրը"), "մայր"); // definite article strips.
//! assert_eq!(ARMENIAN.stem("գրքի"), "գրք");   // genitive strips.
//!
//! let toks: Vec<&str> = ARMENIAN
//!     .tokenize("Բարև, աշխարհ։ Երևանը՝ մայրաքաղաքն է։")
//!     .collect();
//! assert_eq!(
//!     toks,
//!     ["Բարև", "աշխարհ", "Երևանը", "մայրաքաղաքն", "է"]
//! );
//! ```
//!
//! # Module map
//!
//! - [`stemmer`] — the [`ArmenianStemmer`] longest-match suffix
//!   stripper.
//! - [`phonetic`] — [`ArmenianPhonex`] plus the
//!   [`ArmenianPhonexAdapter`] the
//!   [`Language`](stringcheese_lang::Language) trait hands back.
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`tokenizer`] — the [`ArmenianTokenizer`] wrapper.
//! - The [`Armenian`] type and the [`ARMENIAN`] constant live in
//!   this crate's root.

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
pub use phonetic::{ArmenianPhonex, ArmenianPhonexAdapter, armenian_to_latin};
#[cfg(feature = "alloc")]
pub use stemmer::ArmenianStemmer;
pub use stopwords::STOPWORDS;
pub use tokenizer::ArmenianTokenizer;

// -----------------------------------------------------------------------
// The Armenian language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;
    use alloc::string::String;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::ArmenianPhonexAdapter;
    use crate::stemmer::ArmenianStemmer;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::ArmenianTokenizer;

    /// The Armenian language pack.
    ///
    /// Zero-sized; construct as [`Armenian`] and reuse the value
    /// freely across threads and calls, or grab the crate-level
    /// [`ARMENIAN`](crate::ARMENIAN) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Armenian;

    /// The static [`ArmenianPhonexAdapter`] [`Armenian`] hands back
    /// from [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static PHONEX_HY: ArmenianPhonexAdapter = ArmenianPhonexAdapter;

    /// Normalize an Armenian string for stopword comparison:
    /// lowercase under default Unicode rules and normalize the
    /// `եւ → և` two-letter spelling to the ligature.
    ///
    /// The pack stores stopwords in lowercase form (with `և` where
    /// applicable); a query like `"ԵՎ"` needs to lowercase to `եւ`
    /// and then normalize to `և` before the scan can match.
    fn normalize_for_stopword(word: &str) -> String {
        let lowered: String = word.chars().flat_map(char::to_lowercase).collect();
        lowered.replace("եւ", "և")
    }

    impl Language for Armenian {
        fn code(&self) -> &'static str {
            "hy"
        }

        fn name(&self) -> &'static str {
            "Armenian"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        /// Armenian-aware stopword membership.
        ///
        /// Overrides the default trait implementation (which uses
        /// [`str::eq_ignore_ascii_case`], missing every Armenian
        /// input) with a Unicode lowercase pass plus a `եւ → և`
        /// spelling normalization — so `ԵՎ`, `Եւ`, `եւ`, and `և`
        /// all find `և` in the stopword list.
        fn is_stopword(&self, word: &str) -> bool {
            let normalized = normalize_for_stopword(word);
            STOPWORDS.contains(&normalized.as_str())
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            ArmenianStemmer.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(ArmenianTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&PHONEX_HY)
        }
    }

    /// The singleton [`Armenian`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Armenian`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other
    /// `stringcheese-<lang>` pack follows.
    pub const ARMENIAN: Armenian = Armenian;
}

#[cfg(feature = "alloc")]
pub use pack::{ARMENIAN, Armenian};

// Register into `stringcheese-lang::registry` so callers who look up
// languages dynamically (`registry::language("hy")`) find this pack.
// alloc-gated because the pack constant itself is alloc-gated.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(ARMENIAN);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-hy` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
