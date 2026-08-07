//! Persian (Farsi) language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Persian`] value that carries a ~160-entry Persian
//! stopword list, the [`LightPersianStemmer`] suffix stripper, the
//! [`PersianNormalizer`] Arabic-yeh / Arabic-kaf / ZWNJ / tatweel /
//! digit-folding normalizer, the [`PersianTokenizer`] ZWNJ-aware word
//! splitter, and a [`PersianBuckwalter`] transliteration as the phonetic
//! encoder. Callers grab the singleton [`PERSIAN`] `const` — no
//! construction ceremony required — and delegate through the
//! [`Language`](stringcheese_lang::Language) trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-fa` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Persian stopword table
//! or the Persian-Buckwalter mapping. Callers who need Persian add
//! `stringcheese-fa = "0.1"` to their `Cargo.toml` explicitly.
//!
//! # Persian vs. Arabic — same script, different orthography
//!
//! Persian is written in the Arabic script but is *not* an Arabic
//! variety — it is an Indo-European language that adopted the script
//! after the 7th-century Arab conquest of Persia. The orthographic
//! differences the pack cares about:
//!
//! - **Four extra consonants.** Persian phonology has four sounds
//!   Arabic does not — `پ` (U+067E, `p`), `چ` (U+0686, `č`), `ژ`
//!   (U+0698, `ž`), and `گ` (U+06AF, `g`). These letters have no
//!   Arabic counterpart; they carry Persian-only phonetic content.
//! - **Different-shaped versions of Arabic letters.** Persian uses
//!   `ک` (U+06A9) for kaf where Arabic writes `ك` (U+0643), and `ی`
//!   (U+06CC) for yeh where Arabic writes `ي` (U+064A). Text on the
//!   web mixes the two forms constantly; the normalizer folds Arabic
//!   forms to Persian by default.
//! - **Extended Arabic-Indic digits.** Persian text uses `۰..۹`
//!   (U+06F0..=U+06F9), *not* the Eastern Arabic-Indic block `٠..٩`
//!   (U+0660..=U+0669) that Arabic uses. The normalizer offers an
//!   opt-in fold to Western `0..9`.
//! - **Zero-width non-joiner (ZWNJ, U+200C).** Persian compound words
//!   (`می‌روم` "I go", `کتاب‌ها` "books") are written with a ZWNJ
//!   between the joining morphemes, preventing them from ligating
//!   while keeping them a single orthographic word. The ZWNJ is
//!   semantically meaningful — stripping it collapses words that
//!   should stay compound — so the tokenizer treats ZWNJ as
//!   *word-internal* by default. An opt-in normalizer flag strips it
//!   for callers who want compound halves separated for search.
//! - **Ezafeh (`ِ` U+0650).** Persian nominal chains attach nouns via a
//!   ezafeh vowel that is usually *unwritten* in the surface form and
//!   only appears in fully-vocalized text. The pack does not detect
//!   ezafeh; it is deferred to a follow-up. (See "Deferred" below.)
//! - **Tatweel (`ـ` U+0640).** The Arabic decorative letter-stretcher
//!   is also used in Persian typography for justification; the
//!   normalizer strips it by default (it carries no semantic content).
//!
//! # RTL note — same as Arabic
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
//! logical-order UTF-8 and this pack processes it as-is.
//!
//! # Design decisions
//!
//! * **Light suffix stripper as the stemmer baseline.** The Persian
//!   morphology surface is much larger than what a light rule-based
//!   stemmer can cover (verb conjugation, ezafeh, complex compound
//!   verbs), but a small suffix stripper that removes the plural
//!   `-ها` / `-های`, the comparative `-تر` / superlative `-ترین`, and
//!   the possessive clitics `-ام / -ای / -اش / -مان / -تان / -شان`
//!   captures the most-common IR wins without the cost of a full
//!   morphological analyzer. Documented as a **starter** — a
//!   follow-up `stringcheese-fa-morph` crate would ship the richer
//!   analyzer.
//! * **Persian-Buckwalter transliteration.** Buckwalter's original
//!   1990s ASCII mapping covered the 32 Arabic scalars; Persian adds
//!   four consonants (`پ چ ژ گ`) plus its two distinct-shape letters
//!   (`ک ی`). The pack ships a Persian-extended Buckwalter table:
//!   the 28 Arabic letters keep their classical Buckwalter codes; the
//!   four Persian additions get lowercase Latin approximations
//!   (`p`, `c`, `J`, `g`); the Persian `ک` / `ی` collapse to the
//!   same codes as their Arabic counterparts (`k` / `y`) so a query
//!   typed with either variant matches. Adapter name:
//!   `"persian-buckwalter"`.
//! * **ZWNJ is word-internal by default.** Compound words like
//!   `می‌روم` (imperfective marker `می` + `روم` "I go") are a single
//!   orthographic word by convention; the tokenizer respects that.
//!   Callers who want compound halves emitted as separate tokens turn
//!   on [`PersianNormalizer::with_strip_zwnj`] before tokenizing.
//! * **Persian normalizer.** Folds Arabic yeh / kaf to Persian by
//!   default (the most common orthographic mismatch on the web),
//!   strips tatweel by default, offers opt-in Extended-Arabic-Indic
//!   → Western digit folding, opt-in ZWNJ stripping, and opt-in
//!   heh-yeh (`ۀ` U+06C0) decomposition.
//!
//! # Deferred to a follow-up wave
//!
//! * **Ezafeh detection.** The nominal-chain ezafeh vowel is usually
//!   unwritten in the surface form; detecting *where* it belongs
//!   requires POS tagging (is this noun the head of a chain or a
//!   modifier?) plus a lexicon. Full ezafeh detection is deferred to
//!   a downstream `stringcheese-fa-morph` crate.
//! * **Verb morphology.** Persian verbs have a rich prefix + stem +
//!   suffix system (`می-` durative, `ب-` subjunctive, subject / tense
//!   agreement suffixes). The light stemmer handles nominal suffixes
//!   only. Verb-form stripping is deferred to the morphology crate.
//! * **Compound-verb decomposition.** Persian expresses many verbs as
//!   `noun + auxiliary` (`کار کردن` "to work", literally "to make
//!   work"); recognizing and lemmatizing these is a lexicon problem
//!   and out of scope here.
//! * **Dari and Tajik varieties.** Dari (Afghan Persian) uses the
//!   Arabic script and shares most orthography with Iranian Persian
//!   but has a distinct pronunciation and some lexical divergence;
//!   Tajik (Tajikistani Persian) uses Cyrillic script entirely.
//!   These belong in their own `stringcheese-fa-af` /
//!   `stringcheese-tg` packs.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_fa::PERSIAN;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(PERSIAN.code(), "fa");
//! assert_eq!(PERSIAN.name(), "Persian");
//! assert!(PERSIAN.is_stopword("در"));
//! assert!(!PERSIAN.is_stopword("کتاب"));
//! // The plural suffix is stripped by the light stemmer.
//! assert_eq!(PERSIAN.stem("کتاب‌ها"), "کتاب");
//!
//! let toks: Vec<&str> = PERSIAN
//!     .tokenize("محمد کتاب می‌خواند")
//!     .collect();
//! assert_eq!(toks, ["محمد", "کتاب", "می\u{200C}خواند"]);
//! ```
//!
//! # Module map
//!
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`mod@normalize`] — the [`PersianNormalizer`] and the
//!   [`normalize()`] free function.
//! - [`stemmer`] — the [`LightPersianStemmer`] suffix stripper.
//! - [`phonetic`] — the [`PersianBuckwalter`] transliteration and the
//!   [`PersianBuckwalterAdapter`] the
//!   [`Language`](stringcheese_lang::Language) trait hands back.
//! - [`tokenizer`] — the [`PersianTokenizer`] whitespace-based
//!   splitter that treats ZWNJ as word-internal.
//! - The [`Persian`] type and the [`PERSIAN`] constant live in this
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
pub use normalize::{PersianNormalizer, normalize};
#[cfg(feature = "alloc")]
pub use phonetic::{PersianBuckwalter, PersianBuckwalterAdapter};
#[cfg(feature = "alloc")]
pub use stemmer::LightPersianStemmer;
pub use stopwords::STOPWORDS;
pub use tokenizer::PersianTokenizer;

// -----------------------------------------------------------------------
// The Persian language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::PersianBuckwalterAdapter;
    use crate::stemmer::LightPersianStemmer;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::PersianTokenizer;

    /// The Persian language pack.
    ///
    /// Zero-sized; construct as [`Persian`] and reuse the value freely
    /// across threads and calls, or grab the crate-level
    /// [`PERSIAN`](crate::PERSIAN) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Persian;

    /// The static [`PersianBuckwalterAdapter`] [`Persian`] hands back
    /// from [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static PERSIAN_BUCKWALTER: PersianBuckwalterAdapter = PersianBuckwalterAdapter;

    impl Language for Persian {
        fn code(&self) -> &'static str {
            "fa"
        }

        fn name(&self) -> &'static str {
            "Persian"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            LightPersianStemmer.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(PersianTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&PERSIAN_BUCKWALTER)
        }
    }

    /// The singleton [`Persian`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Persian`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other `stringcheese-<lang>`
    /// pack follows.
    pub const PERSIAN: Persian = Persian;
}

#[cfg(feature = "alloc")]
pub use pack::{PERSIAN, Persian};

// Register into `stringcheese-lang::registry` so callers who look up
// languages dynamically (`registry::language("fa")`) find this pack.
// alloc-gated because the pack constant itself is alloc-gated.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(PERSIAN);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-fa` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
