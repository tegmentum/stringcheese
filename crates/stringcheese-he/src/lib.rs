//! Hebrew language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Hebrew`] value that carries a ~130-entry Hebrew
//! stopword list, the [`LightHebrewStemmer`] prefix/suffix stripper,
//! the [`HebrewNormalizer`] niqqud / cantillation / final-form folder,
//! the [`HebrewTokenizer`] maqaf-aware word splitter, and a simplified
//! [`Iso259`] ASCII transliteration as the phonetic encoder. Callers
//! grab the singleton [`HEBREW`] `const` — no construction ceremony
//! required — and delegate through the
//! [`Language`](stringcheese_lang::Language) trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-he` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Hebrew stopword table or
//! the simplified ISO 259 mapping. Callers who need Hebrew add
//! `stringcheese-he = "0.1"` to their `Cargo.toml` explicitly.
//!
//! # Second right-to-left-script pack (after Arabic)
//!
//! This is the *second* `stringcheese-<lang>` implementation for a
//! script written right-to-left, and the second Semitic pack after
//! `stringcheese-ar`. Arabic validated the shape of the
//! [`Language`](stringcheese_lang::Language) trait on RTL text; the
//! Hebrew pack re-uses the same processing model — byte/character
//! sequences in logical UTF-8 order — but against a distinct script
//! (Hebrew, U+0590..=U+05FF) with its own orthographic rules (final
//! letter forms, niqqud below-and-above vowel points, te'amim
//! cantillation marks, and the maqaf compound-word joiner).
//!
//! ## RTL is a display concern
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
//! logical-order UTF-8 (which is what
//! `String::from`-of-a-`&str` naturally gives them) and this pack
//! processes it as-is.
//!
//! # Hebrew orthography, briefly
//!
//! - **22 base consonants.** `א ב ג ד ה ו ז ח ט י כ ל מ נ ס ע פ צ ק ר ש ת`
//!   (U+05D0..=U+05EA, minus the five final-form slots). Every letter
//!   is `char::is_alphanumeric` under Unicode.
//! - **5 final forms.** `ך ם ן ף ץ` (U+05DA, U+05DD, U+05DF, U+05E3,
//!   U+05E5) — the terminal-position variants of kaf, mem, nun, pe,
//!   and tsadi. They carry no lexical distinction from their base
//!   forms; the phonetic encoder always folds them, and the
//!   normalizer's [`HebrewNormalizer::with_final_form_folding`] flag
//!   folds them on the surface form (off by default — final forms are
//!   semantically meaningful position markers that a display-parity
//!   pipeline should preserve).
//! - **Niqqud (vowel points).** Ten combining marks below or above
//!   letters (U+05B0..=U+05BC plus U+05BF, U+05C1, U+05C2, U+05C4,
//!   U+05C5, U+05C7). Optional in modern Hebrew — newswire, web text,
//!   and most digital corpora omit them; the normalizer strips them by
//!   default.
//! - **Te'amim (cantillation marks).** ~30 combining marks in
//!   U+0591..=U+05AF used to annotate Biblical Hebrew for chant. Never
//!   appear in modern text; the normalizer strips them by default.
//! - **Maqaf (`־` U+05BE).** A Hebrew hyphen that joins compound words
//!   (`בית־ספר` "school", literally "house-of-book"). The tokenizer
//!   treats it as *word-internal* by default so compound words stay
//!   one token; the [`HebrewNormalizer::with_strip_hebrew_punctuation`]
//!   flag strips it (along with geresh and gershayim) for callers who
//!   want compound halves separated.
//! - **Geresh (`׳` U+05F3) and gershayim (`״` U+05F4).** Punctuation
//!   used inside abbreviations (`ד״ר` "Dr.") and acronyms; the
//!   normalizer's punctuation flag strips them.
//!
//! ## Design decisions
//!
//! * **Light suffix-and-prefix stemmer.** Hebrew morphology is
//!   root-and-pattern (like Arabic) — a full stemmer needs a
//!   large-scale lexicon (`HebMorph`, `MILA`, etc.). The pack ships a
//!   *light* stemmer that strips the common single-letter prefixes
//!   (`ה` "the", `ו` "and", `ב` "in", `כ` "like", `ל` "to", `מ`
//!   "from", `ש` "that") and the common plural / possessive / past-
//!   tense suffixes (`-ים`, `-ות`, `-ה`, `-י`, `-ך`, `-ו`, `-נו`,
//!   `-כם`, `-הם`, `-תי`, `-ת`, `-תם`). Good-enough-for-IR; full
//!   root extraction is deferred to a downstream `stringcheese-he-morph`
//!   crate. See [`LightHebrewStemmer`] for the algorithm.
//! * **Simplified ISO 259 transliteration.** ISO 259 is the standard
//!   Latin-script transliteration for Hebrew. This pack ships a
//!   *simplified* single-character ASCII variant that trades the
//!   diacritics of the full ISO 259-2 (e.g. `ẖ`, `ṭ`, `ṣ`, `š`) for
//!   ASCII-only single characters (`H`, `T`, `c`, `$`) — same design
//!   choice Buckwalter made for Arabic. Bijective on the letter set
//!   (five final forms fold to their base forms), ASCII-only, plays
//!   well with byte-oriented indexes. Not a Soundex-style sound-alike
//!   encoder; a true `HebrewSoundex` or `PhonexHe` could be added in a
//!   follow-up. See [`Iso259`] for the full table.
//! * **Hebrew normalizer.** Strips niqqud and cantillation marks by
//!   default, and offers opt-in final-form folding and opt-in Hebrew-
//!   punctuation stripping (maqaf, geresh, gershayim). See
//!   [`HebrewNormalizer`] for the rule set.
//! * **Maqaf-aware tokenizer.** The default
//!   [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer) would
//!   split on the maqaf (U+05BE is Unicode punctuation), breaking
//!   compound words. [`HebrewTokenizer`] extends the classifier to
//!   treat maqaf as word-internal so `בית־ספר` stays one token.
//!
//! # Deferred to a follow-up wave
//!
//! * **Root-and-pattern morphological analysis.** Extracting the 3- or
//!   4-letter consonantal root needs template matching against a large
//!   table of derivational patterns plus a lexicon (`HebMorph`, `MILA`).
//!   The light stemmer is the well-established "good enough for IR"
//!   baseline; root extraction belongs in a downstream
//!   `stringcheese-he-morph` crate.
//! * **Verb-conjugation morphology.** Hebrew's seven-binyan verb
//!   system (`pa'al`, `nif'al`, `pi'el`, `pu'al`, `hif'il`, `huf'al`,
//!   `hitpa'el`) has stem-changing morphology the light stemmer does
//!   not attempt to reverse.
//! * **Yiddish (`yi`), Ladino (`lad`), Judeo-Aramaic (`jrb` / `tmr`).**
//!   These languages write with the Hebrew script but have distinct
//!   grammars, vocabulary, and function-word inventories. They belong
//!   in their own `stringcheese-yi` / `stringcheese-lad` /
//!   `stringcheese-jrb` packs.
//! * **True phonetic encoder.** The simplified ISO 259 mapping is a
//!   deterministic *transliteration*, not a sound-alike encoder. A
//!   `HebrewSoundex` or Metaphone-style encoder that collapses
//!   consonantal-skeleton homophones would be a useful alternate.
//! * **Biblical vs. Modern Hebrew tuning.** The stopword list and
//!   stemmer target Modern Hebrew. A `stringcheese-he-biblical` pack
//!   would extend niqqud handling, add construct-state suffix rules,
//!   and carry a Biblical-specific stopword list.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_he::HEBREW;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(HEBREW.code(), "he");
//! assert_eq!(HEBREW.name(), "Hebrew");
//! assert!(HEBREW.is_stopword("של"));
//! assert!(!HEBREW.is_stopword("ספר"));
//! // The definite-article prefix is stripped by the light stemmer.
//! assert_eq!(HEBREW.stem("הספר"), "ספר");
//!
//! let toks: Vec<&str> = HEBREW
//!     .tokenize("אני אוהב לקרוא ספרים")
//!     .collect();
//! assert_eq!(toks, ["אני", "אוהב", "לקרוא", "ספרים"]);
//! ```
//!
//! # Module map
//!
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`mod@normalize`] — the [`HebrewNormalizer`] and the
//!   [`normalize()`] free function.
//! - [`stemmer`] — the [`LightHebrewStemmer`] prefix/suffix stripper.
//! - [`phonetic`] — the simplified [`Iso259`] transliteration and the
//!   [`Iso259Adapter`] the
//!   [`Language`](stringcheese_lang::Language) trait hands back.
//! - [`tokenizer`] — the [`HebrewTokenizer`] maqaf-aware splitter.
//! - The [`Hebrew`] type and the [`HEBREW`] constant live in this
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
pub use normalize::{HebrewNormalizer, normalize};
#[cfg(feature = "alloc")]
pub use phonetic::{Iso259, Iso259Adapter};
#[cfg(feature = "alloc")]
pub use stemmer::LightHebrewStemmer;
pub use stopwords::STOPWORDS;
pub use tokenizer::HebrewTokenizer;

// -----------------------------------------------------------------------
// The Hebrew language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::Iso259Adapter;
    use crate::stemmer::LightHebrewStemmer;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::HebrewTokenizer;

    /// The Hebrew language pack.
    ///
    /// Zero-sized; construct as [`Hebrew`] and reuse the value freely
    /// across threads and calls, or grab the crate-level
    /// [`HEBREW`](crate::HEBREW) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Hebrew;

    /// The static [`Iso259Adapter`] [`Hebrew`] hands back from
    /// [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static ISO259: Iso259Adapter = Iso259Adapter;

    impl Language for Hebrew {
        fn code(&self) -> &'static str {
            "he"
        }

        fn name(&self) -> &'static str {
            "Hebrew"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            LightHebrewStemmer.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(HebrewTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&ISO259)
        }
    }

    /// The singleton [`Hebrew`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Hebrew`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other `stringcheese-<lang>`
    /// pack follows.
    pub const HEBREW: Hebrew = Hebrew;
}

#[cfg(feature = "alloc")]
pub use pack::{HEBREW, Hebrew};

// Register into `stringcheese-lang::registry` so callers who look up
// languages dynamically (`registry::language("he")`) find this pack.
// alloc-gated because the pack constant itself is alloc-gated.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(HEBREW);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-he` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
