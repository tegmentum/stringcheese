//! Greek language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Greek`] value that carries the Greek stopword list,
//! the [`GreekSnowball`] stemmer, the whitespace-and-punctuation
//! [`GreekTokenizer`], and a [`GreekIso843`] transliteration phonetic
//! hookup. Callers grab the singleton [`GREEK`] `const` — no
//! construction ceremony required — and delegate through the
//! [`Language`](stringcheese_lang::Language) trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-el` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Greek stopword table or
//! the Greek stemmer's code. Callers who need Greek add
//! `stringcheese-el = "0.1"` to their `Cargo.toml` explicitly.
//!
//! # First Greek-script pack
//!
//! This is the first `stringcheese-<lang>` implementation for a script
//! written in Greek. It exists to validate the shape of the
//! [`Language`](stringcheese_lang::Language) trait on the Greek
//! alphabet, and to prove that StringCheese's byte/character-sequence
//! processing model handles Greek-script inputs without any
//! special-case machinery.
//!
//! ## Modern Greek alphabet
//!
//! Modern Greek uses **24 letters**:
//!
//! ```text
//! α β γ δ ε ζ η θ ι κ λ μ ν ξ ο π ρ σ/ς τ υ φ χ ψ ω
//! ```
//!
//! Sigma has two positional variants — **final** `ς` (U+03C2) at the
//! end of a word and **non-final** `σ` (U+03C3) elsewhere. The two
//! are the same letter; only the glyph differs. This crate folds
//! `ς → σ` at every entry point (stemmer, phonetic encoder, stopword
//! lookup) so callers get consistent behavior regardless of which
//! spelling their input carries.
//!
//! On top of the 24 base letters the modern *monotonic* orthography
//! places an acute tonos accent on the stressed vowel of every
//! polysyllabic word:
//!
//! ```text
//! ά έ ή ί ό ύ ώ
//! ```
//!
//! and a diaeresis to mark a vowel as syllabically distinct from a
//! preceding vowel that would otherwise form a diphthong:
//!
//! ```text
//! ϊ ϋ    (and the tonos+dialytika composites ΐ ΰ)
//! ```
//!
//! All three (tonos, dialytika, and their composite) fold to the base
//! vowel before any stemming or transliteration runs.
//!
//! ## The Greek-specific invariants
//!
//! Greek is a **left-to-right script**. Unlike Arabic or Hebrew (the
//! RTL packs), there are no display-order surprises; a Rust source
//! file containing `"Αθήνα"` reads A-th-i-n-a in both logical and
//! display order.
//!
//! The one thing to remember is *storage width*:
//!
//! * **Every letter is 2 bytes in UTF-8.** The modern Greek alphabet
//!   sits entirely inside U+0370..=U+03FF, which falls in UTF-8's
//!   2-byte range (U+0080..=U+07FF). A word like `"κόσμος"` (6
//!   characters) is 12 bytes. Any code that mixes byte offsets with
//!   character-boundary logic will silently corrupt token or suffix
//!   boundaries. This crate operates exclusively on `Vec<char>` and
//!   [`str::chars`] iteration — never raw byte offsets. Callers of
//!   [`Language::stem`](stringcheese_lang::Language::stem) and
//!   [`Language::tokenize`](stringcheese_lang::Language::tokenize)
//!   never see the char-vs-byte distinction because the returned
//!   [`Cow<str>`](alloc::borrow::Cow) and token slices are always
//!   valid UTF-8; downstream callers that do byte-level arithmetic on
//!   the outputs must remember the 2x expansion factor.
//! * **No Turkic-fold concerns.** Greek case-folding is well-behaved
//!   under Rust's default [`char::to_lowercase`]: `Α → α`, `Θ → θ`,
//!   `Ω → ω`. Rust follows the *default* Unicode mapping for capital
//!   sigma (`Σ → σ`), not the conditional-on-word-boundary mapping
//!   that would produce final `ς` in position. The pack's `ς → σ`
//!   fold handles the positional variant on input.
//! * **UTF-8 code-point processing order is left-to-right.** No RTL
//!   concerns; the tokenizer emits tokens in reading order.
//!
//! # Design choices
//!
//! * **Snowball-family Greek stemmer.** Modeled after Ntais (2006) as
//!   published at <https://snowballstem.org/algorithms/greek/stemmer.html>,
//!   with a hand-audited subset of the suffix inventory. Preprocessing
//!   folds accents (`ά → α`, etc.) and final sigma (`ς → σ`) before
//!   any suffix work; the suffix cascade runs three passes (long
//!   compound endings, medium nominal / verbal endings, bare final
//!   vowel) with a min-stem-length-3 guard on every strip. See
//!   [`snowball`] for the algorithm and the deferred-parity note.
//! * **~220-entry stopword list (~205 unique).** Common Greek pronouns,
//!   demonstratives, interrogatives, conjunctions, prepositions,
//!   particles, the high-frequency forms of the copula *είμαι* and
//!   the auxiliary *έχω*, and quantifiers. Entries are stored
//!   accent-stripped with non-final sigma; the `is_stopword` override
//!   applies the same fold before comparison so accented or
//!   final-sigma queries match. See [`stopwords`].
//! * **ISO 843 transliteration phonetic encoder.** A deterministic,
//!   ASCII-only Greek → Latin mapping. Not a Soundex-family
//!   sound-alike encoder, but a stable equivalence class the phonetic
//!   subsystem accepts and downstream indexes can consume. Adapter
//!   name: `"iso-843-el"`. See [`phonetic`].
//! * **Simple tokenizer.** Greek orthography uses ASCII spaces
//!   between orthographic words, and every letter of the modern Greek
//!   alphabet satisfies [`char::is_alphanumeric`], so the default
//!   splitter handles Greek word segmentation correctly out of the
//!   box. [`GreekTokenizer`] is a transparent wrapper around
//!   [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer).
//! * **Greek-aware case fold.** Uses Rust's default
//!   [`char::to_lowercase`] plus an accent-fold plus a final-sigma
//!   fold. The `is_stopword` override applies the same normalization
//!   at the call site so accented or final-sigma queries match the
//!   accent-stripped, non-final-sigma stopword list.
//! * **Default Unicode collation.** Greek sorts under CLDR's Greek
//!   tailoring. This pack does not carry the CLDR tailoring data;
//!   [`Language::collator`](stringcheese_lang::Language::collator)
//!   returns `None`. Callers who need locale-aware Greek collation
//!   should reach for `icu_collator`.
//!
//! # Deferred to a follow-up wave
//!
//! * **Ancient Greek (`stringcheese-grc`).** Ancient Greek used a
//!   *polytonic* orthography — three accents (acute, grave,
//!   circumflex), two breathings (rough, smooth), and the iota
//!   subscript / adscript — plus a much richer morphology (five
//!   cases, three numbers including dual, three voices, six moods,
//!   participles that inflect for case). A dedicated
//!   `stringcheese-grc` sibling with a polytonic normalizer and an
//!   Ancient-Greek-aware suffix cascade would deserve its own pack.
//! * **Katharevousa.** The archaic / puristic form of Modern Greek
//!   used until 1976 preserved many Ancient-Greek forms that Demotic
//!   Greek does not (`ήτο`, `όστις`, etc.). Katharevousa handling is
//!   out of scope for this pack.
//! * **Polytonic Greek text.** Modern Greek written with polytonic
//!   accents (still occasionally seen in ecclesiastical texts, some
//!   literary reprints, and older scholarly editions) passes through
//!   this pack's accent-fold unchanged — the fold only recognizes the
//!   monotonic tonos and dialytika. A future polytonic normalizer
//!   could either NFD-decompose and strip the full range of Greek
//!   combining marks, or ship an expanded fold table.
//! * **Coptic sibling (`stringcheese-cop`).** The Coptic script
//!   (U+2C80..=U+2CFF, plus historical letters in the Greek block)
//!   descends from Greek and shares many letters; a dedicated Coptic
//!   pack would want its own stopword list, morphological analyzer,
//!   and transliteration.
//! * **Full canonical Snowball parity.** The shipped stemmer covers
//!   the high-frequency nominal / adjectival / verbal endings; the
//!   full canonical Snowball spec's context-sensitive tail (word-list
//!   exceptions, palatal-alternation reversal) is a follow-up.
//! * **ELOT 743 Greek-transliteration adapter.** The Greek passport
//!   authority's transliteration diverges from ISO 843 on `γκ → g` /
//!   `μπ → b` context-sensitive rules. Would want it under a separate
//!   adapter for name-matching interop.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_el::GREEK;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(GREEK.code(), "el");
//! assert_eq!(GREEK.name(), "Greek");
//! assert!(GREEK.is_stopword("και"));
//! assert!(GREEK.is_stopword("ΚΑΙ"));      // Greek case-fold: ΚΑΙ → και.
//! assert!(GREEK.is_stopword("είμαι"));    // Accent-fold: είμαι → ειμαι.
//! assert!(!GREEK.is_stopword("κόσμος"));
//! assert_eq!(GREEK.stem("καλός"), "καλ");
//! assert_eq!(GREEK.stem("κόσμος"), "κοσμ");
//!
//! let toks: Vec<&str> = GREEK
//!     .tokenize("Καλημέρα, κόσμε! Αθήνα — πρωτεύουσα.")
//!     .collect();
//! assert_eq!(toks, ["Καλημέρα", "κόσμε", "Αθήνα", "πρωτεύουσα"]);
//! ```
//!
//! # Module map
//!
//! - [`snowball`] — the [`GreekSnowball`] stemmer.
//! - [`phonetic`] — [`GreekIso843`] plus the [`GreekIso843Adapter`]
//!   the [`Language`](stringcheese_lang::Language) trait hands back.
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`tokenizer`] — the [`GreekTokenizer`] wrapper.
//! - The [`Greek`] type and the [`GREEK`] constant live in this
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
pub mod snowball;
pub mod stopwords;
pub mod tokenizer;

#[cfg(all(test, feature = "std", not(target_family = "wasm")))]
mod properties;

#[cfg(feature = "alloc")]
pub use phonetic::{GreekIso843, GreekIso843Adapter};
#[cfg(feature = "alloc")]
pub use snowball::GreekSnowball;
pub use stopwords::STOPWORDS;
pub use tokenizer::GreekTokenizer;

// -----------------------------------------------------------------------
// The Greek language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;
    use alloc::string::String;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::{GreekIso843Adapter, fold_accents};
    use crate::snowball::GreekSnowball;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::GreekTokenizer;

    /// The Greek language pack.
    ///
    /// Zero-sized; construct as [`Greek`] and reuse the value freely
    /// across threads and calls, or grab the crate-level
    /// [`GREEK`](crate::GREEK) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Greek;

    /// The static [`GreekIso843Adapter`] [`Greek`] hands back from
    /// [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static ISO_843_EL: GreekIso843Adapter = GreekIso843Adapter;

    /// Normalize a Greek string for stopword comparison: lowercase
    /// under default Unicode rules, fold accents (`ά → α`, etc.), and
    /// fold the positional final sigma (`ς → σ`).
    ///
    /// The pack stores stopwords in their accent-stripped
    /// non-final-sigma form; a query like `"ΕΊΜΑΙ"` needs to lowercase,
    /// strip the accent, and land on `"ειμαι"` before the scan can
    /// match. A query like `"κόσμος"` needs the same normalization
    /// (accent fold + `ς → σ`).
    fn normalize_for_stopword(word: &str) -> String {
        let mut out = String::with_capacity(word.len());
        for c in word.chars() {
            for lc in c.to_lowercase() {
                let folded = fold_accents(lc);
                let folded = if folded == 'ς' { 'σ' } else { folded };
                out.push(folded);
            }
        }
        out
    }

    impl Language for Greek {
        fn code(&self) -> &'static str {
            "el"
        }

        fn name(&self) -> &'static str {
            "Greek"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        /// Greek-aware stopword membership.
        ///
        /// Overrides the default trait implementation (which uses
        /// [`str::eq_ignore_ascii_case`], missing every uppercase
        /// Greek input) with a Unicode lowercase pass, an accent-fold,
        /// and a final-sigma fold — so `ΕΊΜΑΙ`, `Είμαι`, and `είμαι`
        /// all find the accent-stripped `ειμαι` in the stopword list.
        fn is_stopword(&self, word: &str) -> bool {
            let normalized = normalize_for_stopword(word);
            STOPWORDS.contains(&normalized.as_str())
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            GreekSnowball.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(GreekTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&ISO_843_EL)
        }
    }

    /// The singleton [`Greek`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Greek`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other `stringcheese-<lang>`
    /// pack follows.
    pub const GREEK: Greek = Greek;
}

#[cfg(feature = "alloc")]
pub use pack::{GREEK, Greek};

// Register into `stringcheese-lang::registry` so callers who look up
// languages dynamically (`registry::language("el")`) find this pack.
// alloc-gated because the pack constant itself is alloc-gated.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(GREEK);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-el` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
