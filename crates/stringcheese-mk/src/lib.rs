//! Macedonian language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Macedonian`] value that carries the Macedonian
//! stopword list, the [`MacedonianStemmer`] (with the signature
//! three-way postposed-article step Macedonian requires), the
//! whitespace-and-punctuation [`MacedonianTokenizer`], and a
//! [`MacedonianPhonex`] Soundex-shape phonetic encoder. Callers grab
//! the singleton [`MACEDONIAN`] `const` — no construction ceremony
//! required — and delegate through the
//! [`Language`](stringcheese_lang::Language) trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-mk` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Macedonian stopword table
//! or the stemmer's code. Callers who need Macedonian add
//! `stringcheese-mk = "0.1"` to their `Cargo.toml` explicitly.
//!
//! # Fourth Cyrillic-script pack
//!
//! This is the fourth `stringcheese-<lang>` implementation for a script
//! written in Cyrillic — Russian shipped in wave 7, Ukrainian in wave 8,
//! Bulgarian and Serbian later — all establishing the shape
//! (`Vec<char>` suffix arithmetic, `is_stopword` override with Unicode
//! case-fold, deterministic Cyrillic-friendly phonetic key). Macedonian
//! is Bulgarian's closest linguistic sibling (both are analytic South
//! Slavic languages with postposed definite articles) and this pack
//! follows the Bulgarian shape with three Macedonian-specific twists:
//!
//! ## Macedonian alphabet — 31 letters, seven Macedonian-specific
//!
//! Macedonian uses a 31-letter subset of the Cyrillic block that
//! includes **seven letters no East Slavic pack carries**:
//!
//! ```text
//! а б в г д ѓ е ж з ѕ и ј к л љ м н њ о п р с т ќ у ф х ц ч џ ш
//! ```
//!
//! * **`ѓ` (U+0453)** — the voiced palatal stop /ɟ/. A distinctly
//!   Macedonian letter (Serbian writes the same sound as `ђ` U+0452).
//! * **`ќ` (U+045C)** — the voiceless palatal stop /c/. Macedonian's
//!   counterpart to Serbian `ћ` U+045B.
//! * **`љ` (U+0459)** — the palatal lateral approximant /ʎ/. Shared
//!   with Serbian; not present in Bulgarian / Russian / Ukrainian.
//! * **`њ` (U+045A)** — the palatal nasal /ɲ/. Shared with Serbian.
//! * **`џ` (U+045F)** — the voiced postalveolar affricate /dʒ/. Shared
//!   with Serbian.
//! * **`ѕ` (U+0455)** — the voiced alveolar affricate /dz/. Distinctly
//!   Macedonian; Serbian and Bulgarian both write /dz/ as a `дз`
//!   sequence.
//! * **`ј` (U+0458)** — the palatal approximant /j/. Shared with
//!   Serbian; Bulgarian writes /j/ as `й`, and Russian writes it that
//!   way too.
//!
//! Compared with Bulgarian, Macedonian **omits** `й` (U+0439), `щ`
//! (U+0449), `ъ` (U+044A), `ь` (U+044C), `ю` (U+044E), `я` (U+044F).
//! Compared with Russian, Macedonian also omits `ё`, `ы`, `э`. The
//! stopword, stemmer, and PHONEX tables therefore never encounter
//! those letters.
//!
//! ## Macedonian's three-way definite article — the signature feature
//!
//! Macedonian, like Bulgarian, is **analytic**: it dropped nominal case
//! declension but retained a **postposed definite article** that
//! attaches to the noun as a suffix. Where Bulgarian has one series
//! (`-ът / -та / -то / -те` plus long-adjective variants), Macedonian
//! has **three**, differing in speaker-to-referent proximity:
//!
//! | Proximity | Masculine | Feminine | Neuter | Plural |
//! |-----------|-----------|----------|--------|--------|
//! | Proximal (near / this-here) | `-ов` | `-ва` | `-во` | `-ве` |
//! | Medial (neutral, the default) | `-от` | `-та` | `-то` | `-те` |
//! | Distal (far / that-yonder) | `-он` | `-на` | `-но` | `-не` |
//!
//! `градот` = "the city" (medial, neutral); `градов` = "this city
//! here" (proximal); `градон` = "that city yonder" (distal). All three
//! must collapse to the same stem as `град` for an IR pipeline that
//! wants `city`-scoped documents to match. The [`stemmer`] handles the
//! collapse; the article-stripping step runs **first**, before any
//! other suffix cascade.
//!
//! ## Cyrillic-specific invariants (same as Russian / Ukrainian /
//! ## Bulgarian / Serbian)
//!
//! * **Every letter is 2 bytes in UTF-8.** Macedonian's alphabet sits
//!   entirely inside U+0400..=U+045F, which falls in UTF-8's 2-byte
//!   range (U+0080..=U+07FF). A word like `"град"` (4 characters) is
//!   8 bytes. All suffix and region arithmetic runs on `Vec<char>` in
//!   [`stemmer`], never raw byte offsets, so no scalar is ever sliced
//!   apart.
//! * **No Turkic-fold concerns.** Macedonian case-folding is
//!   well-behaved under Rust's default [`char::to_lowercase`]:
//!   `А → а`, `Ѓ → ѓ`, `Ќ → ќ`, `Љ → љ`, `Њ → њ`, `Џ → џ`, `Ѕ → ѕ`,
//!   `Ј → ј`. There is no locale tailoring the way Turkish requires.
//! * **UTF-8 code-point processing order is left-to-right.** No RTL
//!   concerns; the tokenizer emits tokens in reading order.
//!
//! # Design choices
//!
//! * **Rule-based Macedonian stemmer.** There is no canonical Snowball
//!   Macedonian; this crate ships a hand-rolled stemmer in the shape of
//!   the Bulgarian Snowball (Nakov 2003) algorithm, adapted for
//!   Macedonian's three-way article system and letter set. Four-step
//!   cascade: strip definite article first (all twelve forms), then
//!   plural markers (`-ови`, `-еви`, `-ња`), then verb personal
//!   endings (`-ам`, `-аш`, `-ат`, `-ме`, `-те`, `-ав`), then a bare-
//!   vowel final-strip (`-а`, `-и`, `-о`, `-у`). See [`stemmer`].
//! * **~155-word stopword list.** Common Macedonian pronouns
//!   (including the proximity-triple demonstratives `овој / тој / оној`
//!   with their gender / number agreement), interrogatives,
//!   conjunctions, prepositions, particles, high-frequency forms of
//!   the copula *сум*, and quantifiers. See [`stopwords`].
//! * **PHONEX-mk Soundex-shape encoder.** A 4-character
//!   `<letter><digit><digit><digit>` key with Macedonian-tuned
//!   classification — the seven Macedonian-specific letters fold to
//!   their nearest Slavic-Soundex class (`ѓ`/`ќ`/`ј` → guttural class
//!   2, `љ` → lateral class 4, `њ` → nasal class 5, `ѕ`/`џ` → sibilant
//!   class 7). Adapter name: `"phonex-mk"`. See [`phonetic`].
//! * **Simple tokenizer.** Macedonian orthography uses ASCII spaces
//!   between orthographic words, and every letter of the Macedonian
//!   alphabet satisfies [`char::is_alphanumeric`], so the default
//!   splitter handles Macedonian word segmentation correctly.
//!   [`MacedonianTokenizer`] is a transparent wrapper around
//!   [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer).
//! * **Cyrillic-aware case fold.** Uses Rust's default
//!   [`char::to_lowercase`] — no Turkic-style tailoring. The
//!   `is_stopword` override lowercases under this rule so uppercase
//!   Cyrillic queries match the plain stopword list.
//! * **Default Unicode collation.**
//!   [`Language::collator`](stringcheese_lang::Language::collator)
//!   returns `None`. Callers who need locale-aware Macedonian
//!   collation should reach for `icu_collator`.
//!
//! # Deferred to a follow-up wave
//!
//! * **GOST 7.79-B Macedonian adaptation.** A parallel Cyrillic → Latin
//!   transliteration adapter for library-catalog interop. Would ship
//!   alongside the PHONEX encoder as an alternative.
//! * **Slavic-Metaphone Macedonian.** A variable-length sound-alike
//!   encoder spanning Bulgarian / Russian / Ukrainian / Serbian /
//!   Macedonian for cross-Slavic record linkage.
//! * **Palatal-alternation reversal** (`к`/`ц`, `г`/`з`, `х`/`с`).
//!   Same non-goal as Bulgarian's pack — reversing these without a
//!   lexicon over-restores for words where the alternation is
//!   fossilized.
//! * **Full-vocabulary cross-verification.** The shipped reference-
//!   pair test embeds a hand-traced subset; a full corpus cross-check
//!   would want a Macedonian voc.txt/output.txt pair.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_mk::MACEDONIAN;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(MACEDONIAN.code(), "mk");
//! assert_eq!(MACEDONIAN.name(), "Macedonian");
//! assert!(MACEDONIAN.is_stopword("и"));
//! assert!(MACEDONIAN.is_stopword("НЕ"));   // Cyrillic case-fold: НЕ → не.
//! assert!(!MACEDONIAN.is_stopword("книга"));
//!
//! // The signature Macedonian move: three-way definite-article stripping.
//! // `градот` (medial), `градов` (proximal), `градон` (distal) all
//! // collapse to the same stem as `град`.
//! assert_eq!(MACEDONIAN.stem("градот"), MACEDONIAN.stem("град"));
//! assert_eq!(MACEDONIAN.stem("градов"), MACEDONIAN.stem("град"));
//! assert_eq!(MACEDONIAN.stem("градон"), MACEDONIAN.stem("град"));
//!
//! let toks: Vec<&str> = MACEDONIAN
//!     .tokenize("Здраво, свет! Скопје — главен град.")
//!     .collect();
//! assert_eq!(toks, ["Здраво", "свет", "Скопје", "главен", "град"]);
//! ```
//!
//! # Module map
//!
//! - [`stemmer`] — the [`MacedonianStemmer`] rule-based stemmer.
//! - [`phonetic`] — [`MacedonianPhonex`] plus the
//!   [`MacedonianPhonexAdapter`] the
//!   [`Language`](stringcheese_lang::Language) trait hands back.
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`tokenizer`] — the [`MacedonianTokenizer`] wrapper.
//! - The [`Macedonian`] type and the [`MACEDONIAN`] constant live in
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
pub use phonetic::{MacedonianPhonex, MacedonianPhonexAdapter};
#[cfg(feature = "alloc")]
pub use stemmer::MacedonianStemmer;
pub use stopwords::STOPWORDS;
pub use tokenizer::MacedonianTokenizer;

// -----------------------------------------------------------------------
// The Macedonian language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;
    use alloc::string::String;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::MacedonianPhonexAdapter;
    use crate::stemmer::MacedonianStemmer;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::MacedonianTokenizer;

    /// The Macedonian language pack.
    ///
    /// Zero-sized; construct as [`Macedonian`] and reuse the value
    /// freely across threads and calls, or grab the crate-level
    /// [`MACEDONIAN`](crate::MACEDONIAN) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Macedonian;

    /// The static [`MacedonianPhonexAdapter`] [`Macedonian`] hands
    /// back from [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static PHONEX_MK: MacedonianPhonexAdapter = MacedonianPhonexAdapter;

    /// Normalize a Cyrillic string for stopword comparison: lowercase
    /// under default Unicode rules.
    fn normalize_for_stopword(word: &str) -> String {
        let mut out = String::with_capacity(word.len());
        for c in word.chars() {
            for lc in c.to_lowercase() {
                out.push(lc);
            }
        }
        out
    }

    impl Language for Macedonian {
        fn code(&self) -> &'static str {
            "mk"
        }

        fn name(&self) -> &'static str {
            "Macedonian"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        /// Cyrillic-aware stopword membership.
        ///
        /// Overrides the default trait implementation (which uses
        /// [`str::eq_ignore_ascii_case`], missing every uppercase
        /// Cyrillic input) with a Unicode lowercase pass — so `НЕ`
        /// and `не` both find `не` in the stopword list.
        fn is_stopword(&self, word: &str) -> bool {
            let normalized = normalize_for_stopword(word);
            STOPWORDS.contains(&normalized.as_str())
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            MacedonianStemmer.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(MacedonianTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&PHONEX_MK)
        }
    }

    /// The singleton [`Macedonian`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Macedonian`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other `stringcheese-<lang>`
    /// pack follows.
    pub const MACEDONIAN: Macedonian = Macedonian;
}

#[cfg(feature = "alloc")]
pub use pack::{MACEDONIAN, Macedonian};

// Register into `stringcheese-lang::registry` so callers who look up
// languages dynamically (`registry::language("mk")`) find this pack.
// alloc-gated because the pack constant itself is alloc-gated.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(MACEDONIAN);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-mk` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
