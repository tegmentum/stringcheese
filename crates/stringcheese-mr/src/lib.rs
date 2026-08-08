//! Marathi language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Marathi`] value that carries a ~55-entry Devanagari
//! stopword list, the [`LightMarathiStemmer`] iterated suffix
//! stripper, the [`MarathiTokenizer`] whitespace-and-Devanagari-
//! punctuation word splitter (danda `।` aware), and a
//! [`MarathiPhonex`] PHONEX-Marathi (Soundex-shape 4-char) key
//! computed over an ISO 15919 Devanagari → Latin transliteration
//! ([`MarathiIso15919`]) as the phonetic encoder. Callers grab the
//! singleton [`MARATHI`] `const` — no construction ceremony
//! required — and delegate through the
//! [`Language`](stringcheese_lang::Language) trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-mr` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Marathi stopword table
//! or the ISO 15919 transliteration table. Callers who need Marathi
//! add `stringcheese-mr = "0.1"` to their `Cargo.toml` explicitly.
//!
//! # Second Devanagari-script pack
//!
//! Marathi is the **second Devanagari-script pack** in StringCheese
//! (after `stringcheese-hi`, which is the first). Both languages use
//! the Devanagari block (U+0900..=U+097F, 3 UTF-8 bytes per scalar),
//! but Marathi differs from Hindi in three linguistically-meaningful
//! ways that this pack reflects:
//!
//! * **Marathi retains the Old Indo-Aryan neuter gender.** Hindi has
//!   only masculine and feminine; Marathi has all three, so verb and
//!   adjective agreement is three-way rather than two-way. The
//!   stopword list carries the neuter forms of demonstratives (`हे`
//!   n. sg. alongside `हा` m. sg. / `ही` f. sg.).
//! * **Marathi's case marking is agglutinative.** Hindi's
//!   postpositions are almost always written as separate words
//!   (`घर को` "to the house" — two tokens); Marathi's case suffixes
//!   attach directly to the noun stem (`घराला` — one token). This
//!   means the Marathi stemmer is *substantially* more useful than
//!   Hindi's — Marathi words genuinely inflect at the surface
//!   spelling level, so the stemmer's suffix table can strip case
//!   markers (`-ला`, `-चा`, `-ने`, `-त`, `-हून`, `-शी`) that in
//!   Hindi would arrive as separate tokens that the tokenizer has
//!   already split off.
//! * **Marathi retains letters Standard Modern Hindi lacks.**
//!   `ळ` (U+0933, retroflex L, /ɭ/) and `ऱ` (U+0931, R with lower
//!   diagonal) are Marathi phonemes with no counterpart in the
//!   Hindi mapping. The tokenizer captures them via the general
//!   Devanagari-block rule; the phonetic encoder maps them to
//!   `ḷ` / `ṟ` in ISO 15919, folding both to their ASCII bases
//!   (`L` / `R`) in the phonex-key output.
//!
//! ## The Devanagari-specific invariants (inherited from `stringcheese-hi`)
//!
//! Devanagari is a **left-to-right script** (no RTL surprises). The
//! things to remember are *storage width* and the *abugida script
//! model*:
//!
//! * **Every Devanagari letter is 3 bytes in UTF-8.** The block
//!   U+0900..=U+097F falls in UTF-8's 3-byte range
//!   (U+0800..=U+FFFF), so a word like `"पुस्तक"` (6 characters:
//!   `प` + `ु` + `स` + `्` + `त` + `क`) is **18 bytes**. Cyrillic
//!   is 2 bytes per letter; Latin is 1. Any code that mixes byte
//!   offsets with character-boundary logic will silently corrupt
//!   token or suffix boundaries. This crate operates exclusively on
//!   `Vec<char>` and [`str::chars`] iteration — never raw byte
//!   offsets. Callers of
//!   [`Language::stem`](stringcheese_lang::Language::stem) and
//!   [`Language::tokenize`](stringcheese_lang::Language::tokenize)
//!   never see the char-vs-byte distinction because the returned
//!   [`Cow<str>`](alloc::borrow::Cow) and token slices are always
//!   valid UTF-8; downstream callers that do byte-level arithmetic
//!   on the outputs must remember the 3x expansion factor.
//! * **Independent vowels vs. dependent vowel signs.** Devanagari
//!   distinguishes two forms of every vowel: an **independent** form
//!   used at the start of a word (`अ आ इ ई उ ऊ ए ऐ ओ औ` U+0905..) and
//!   a **dependent** form (matra: `ा ि ी ु ू े ै ो ौ` U+093E..) used
//!   after a consonant.
//! * **Virama and consonant clusters.** Devanagari is an **abugida**:
//!   every base consonant carries an implicit `a` (schwa) vowel
//!   unless a matra or a **virama** (`्` U+094D) overrides it.
//! * **Nukta (`़` U+093C).** A combining mark that modifies base
//!   consonants to produce Perso-Arabic loans. Both precomposed
//!   nukta letters (`क़`, `ख़`, `ग़`, `ज़`, `ड़`, `ढ़`, `फ़`) and
//!   decomposed `base + ़` sequences are handled by the ISO 15919
//!   encoder.
//! * **Devanagari digits (`०..९` U+0966..=U+096F).** Marathi text
//!   uses either the Devanagari digit block or ASCII `0..9`; the
//!   transliterator folds Devanagari → ASCII.
//! * **Danda (`।` U+0964) — the Devanagari full stop.** Marathi text
//!   ends sentences with the danda (same convention as Hindi); the
//!   tokenizer treats it as a separator (see [`crate::tokenizer`]).
//!
//! The design choices are otherwise:
//!
//! * **Light Marathi suffix stripper as the stemmer baseline.** There
//!   is **no canonical Snowball Marathi algorithm** (Marathi is not
//!   in the Snowball catalogue). The module ships a deliberately
//!   conservative rule-based subset covering the agglutinative case
//!   markers (`-ला/-ना` accusative/dative, `-चा/-ची/-चे/-च्या`
//!   genitive with head-noun agreement, `-ने/-नी` instrumental/
//!   ergative, `-त/-मध्ये` locative, `-ऊन/-हून` ablative,
//!   `-शी/-सह` sociative), the two-form plural markers, and the
//!   most-common present-habitual and aorist verb personal endings,
//!   with a 2-scalar min-stem guard. The internal loop **iterates
//!   to convergence** so a stacked suffix like the oblique-plural +
//!   case-marker combination in `घरांना` (houses-to) reduces to
//!   `घर` in a single external call. Documented as a **starter** —
//!   a follow-up `stringcheese-mr-morph` crate would ship the full
//!   analyzer. See [`crate::stemmer`].
//! * **ISO 15919 + PHONEX-Marathi two-stage phonetic encoder.** The
//!   scholarly Indic-script romanization ([`MarathiIso15919`]) is
//!   exposed as its own public API — useful in its own right for
//!   data-migration tools and IR display — and the language pack's
//!   [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
//!   returns a [`MarathiPhonex`] adapter that reduces the ISO 15919
//!   output to a 4-character Soundex-shape key. Adapter name:
//!   `"phonex-mr"`. This matches the shape of the Bengali pack
//!   (`phonex-bn`) and every other Latin-alphabet pack's phonetic
//!   hookup. See [`crate::phonetic`].
//! * **Matra-aware tokenizer.** The default splitter walks on
//!   `char::is_alphanumeric`, which treats matras / virama /
//!   anusvara as separators and would shatter a Marathi word at
//!   every dependent vowel sign. The Marathi tokenizer extends the
//!   rule to include the full Devanagari block U+0900..=U+097F as
//!   word scalars, with the three Devanagari punctuation marks
//!   (`।` `॥` `॰`) as explicit separators. See [`crate::tokenizer`].
//! * **~55-word stopword list.** Personal / demonstrative /
//!   interrogative pronouns (including the neuter demonstratives
//!   Hindi lacks and the clusivity-preserving 1pl `आम्ही` / `आपण`
//!   distinction), the *independent* postpositions (Marathi's
//!   agglutinative case markers are handled by the stemmer, not the
//!   stopword list), conjunctions, particles, high-frequency forms
//!   of the copula *असणे* ("to be") and auxiliary *करणे* ("to do"),
//!   and common adverbs. See [`stopwords`].
//! * **Default Unicode collation.** Marathi sorts under CLDR's
//!   Marathi tailoring. This pack does not carry the CLDR tailoring
//!   data;
//!   [`Language::collator`](stringcheese_lang::Language::collator)
//!   returns `None`. Callers who need locale-aware Marathi collation
//!   should reach for `icu_collator`.
//!
//! # Deferred to a follow-up wave
//!
//! * **Konkani pack (`stringcheese-kok`).** Konkani is a sister
//!   Indo-Aryan language of Maharashtra / Goa, written in Devanagari
//!   (and other scripts historically) with distinct morphology and
//!   function-word inventory.
//! * **Full Snowball Marathi.** If a canonical `marathi.sbl` ever
//!   appears in the Snowball catalogue, ship the port under a Cargo
//!   feature (`snowball-mr`) alongside the current light stemmer.
//! * **Schwa-deletion.** The ISO 15919 encoder honors the
//!   inherent-schwa convention. Modern colloquial Marathi drops the
//!   schwa in many word-final and some medial positions; the
//!   deletion is context-dependent and lexicon-driven — implementing
//!   it needs a Marathi lexicon and is deferred to the morphology
//!   crate.
//! * **ITRANS / HK / SLP1 romanization adapters.** ISO 15919 is the
//!   scholarly baseline this pack ships with; additional romanization
//!   schemes are deferred.
//! * **Marathi-specific normalizer.** Callers who need
//!   Devanagari-digit folding or nukta stripping should reach for the
//!   `stringcheese-hi::HindiNormalizer` — the folds apply
//!   unchanged to Marathi text.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_mr::MARATHI;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(MARATHI.code(), "mr");
//! assert_eq!(MARATHI.name(), "Marathi");
//! assert!(MARATHI.is_stopword("आणि"));
//! assert!(!MARATHI.is_stopword("पुस्तक"));
//! // The stemmer strips agglutinative case markers and iterates to
//! // convergence.
//! assert_eq!(MARATHI.stem("घराला"), "घर");
//!
//! let toks: Vec<&str> = MARATHI
//!     .tokenize("मी मराठी बोलतो।")
//!     .collect();
//! assert_eq!(toks, ["मी", "मराठी", "बोलतो"]);
//! ```
//!
//! # Module map
//!
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`stemmer`] — the [`LightMarathiStemmer`] iterated suffix
//!   stripper.
//! - [`phonetic`] — the [`MarathiIso15919`] transliteration and the
//!   [`MarathiPhonex`] reduction; [`MarathiPhonexAdapter`] is the
//!   type
//!   [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
//!   hands back.
//! - [`tokenizer`] — the [`MarathiTokenizer`] whitespace-and-
//!   Devanagari-punctuation splitter.
//! - The [`Marathi`] type and the [`MARATHI`] constant live in this
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
pub mod phonetic;
#[cfg(feature = "alloc")]
pub mod stemmer;
pub mod stopwords;
pub mod tokenizer;

#[cfg(all(test, feature = "std", not(target_family = "wasm")))]
mod properties;

#[cfg(feature = "alloc")]
pub use phonetic::{MarathiIso15919, MarathiPhonex, MarathiPhonexAdapter};
#[cfg(feature = "alloc")]
pub use stemmer::LightMarathiStemmer;
pub use stopwords::STOPWORDS;
pub use tokenizer::MarathiTokenizer;

// -----------------------------------------------------------------------
// The Marathi language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::MarathiPhonexAdapter;
    use crate::stemmer::LightMarathiStemmer;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::MarathiTokenizer;

    /// The Marathi language pack.
    ///
    /// Zero-sized; construct as [`Marathi`] and reuse the value
    /// freely across threads and calls, or grab the crate-level
    /// [`MARATHI`](crate::MARATHI) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Marathi;

    /// The static [`MarathiPhonexAdapter`] [`Marathi`] hands back
    /// from [`phonetic_encoder`](Language::phonetic_encoder).
    static MARATHI_PHONEX: MarathiPhonexAdapter = MarathiPhonexAdapter;

    impl Language for Marathi {
        fn code(&self) -> &'static str {
            "mr"
        }

        fn name(&self) -> &'static str {
            "Marathi"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            LightMarathiStemmer.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(MarathiTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&MARATHI_PHONEX)
        }
    }

    /// The singleton [`Marathi`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Marathi`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other
    /// `stringcheese-<lang>` pack follows.
    pub const MARATHI: Marathi = Marathi;
}

#[cfg(feature = "alloc")]
pub use pack::{MARATHI, Marathi};

// Register into `stringcheese-lang::registry` so callers who look up
// languages dynamically (`registry::language("mr")`) find this pack.
// alloc-gated because the pack constant itself is alloc-gated.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(MARATHI);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-mr` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
