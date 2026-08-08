//! Tamil language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Tamil`] value that carries a ~55-entry Tamil
//! stopword list, the [`LightTamilStemmer`] suffix stripper, the
//! [`TamilTokenizer`] whitespace-and-punctuation word splitter, and
//! a [`TamilPhonex`] PHONEX-Tamil (Soundex-shape 4-char) key computed
//! over an ISO 15919 Tamil → Latin transliteration
//! ([`TamilIso15919`]) as the phonetic encoder. Callers grab the
//! singleton [`TAMIL`] `const` — no construction ceremony required —
//! and delegate through the [`Language`](stringcheese_lang::Language)
//! trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-ta` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Tamil stopword table or
//! the ISO 15919 transliteration table. Callers who need Tamil add
//! `stringcheese-ta = "0.1"` to their `Cargo.toml` explicitly.
//!
//! # First Dravidian pack, third Brahmic-script pack
//!
//! Tamil is the **first Dravidian language** in StringCheese and the
//! **third Brahmic-script pack** (after `stringcheese-hi`, which
//! handles Devanagari, and `stringcheese-bn`, which handles Bengali).
//!
//! Every previous language pack in the workspace has been
//! Indo-European (Latin / Cyrillic / Devanagari / Bengali / Greek /
//! Arabic-script Iranian) or Uralic (Finnish / Estonian / Hungarian)
//! or from an isolated / small family (Japanese / Korean / Chinese /
//! Vietnamese / Turkic). Tamil is Dravidian — a family whose
//! grammar diverges from Indo-European in the same *systematic*
//! ways Uralic does (agglutinative morphology, extensive case
//! marking, no gender distinction on verbs, subject-object-verb
//! constituent order) but with an independent lexicon and phonology.
//!
//! The Tamil script (U+0B80..=U+0BFF) is a Brahmic-family abugida
//! like Devanagari and Bengali. Every Tamil letter is encoded in
//! **3 UTF-8 bytes** (the block falls in UTF-8's 3-byte range), so
//! the pack reuses the byte-safe processing model validated by
//! `stringcheese-hi` and `stringcheese-bn`: all suffix / tokenizer
//! / stemmer arithmetic runs on `Vec<char>`, never raw byte offsets.
//!
//! ## The Tamil-specific invariants
//!
//! Tamil is a **left-to-right script** (no RTL surprises). The
//! things to remember are *storage width*, the *abugida script
//! model*, and the *simpler consonant inventory* than Devanagari's:
//!
//! * **Every Tamil letter is 3 bytes in UTF-8.** The block
//!   U+0B80..=U+0BFF falls in UTF-8's 3-byte range
//!   (U+0800..=U+FFFF), so a word like `"நான்"` (4 characters:
//!   `ந` + `ா` + `ன` + `்`) is **12 bytes**. Cyrillic is 2 bytes
//!   per letter; Latin is 1. Any code that mixes byte offsets with
//!   character-boundary logic will silently corrupt token or suffix
//!   boundaries. This crate operates exclusively on `Vec<char>` and
//!   [`str::chars`] iteration — never raw byte offsets.
//! * **No conjunct consonants — every cluster uses an explicit
//!   pulli.** Unlike Devanagari, which stacks consonants into
//!   ligature-forming conjuncts and *visually elides* the virama
//!   inside a cluster, Tamil writes **every** consonant cluster
//!   with a visible pulli `்` U+0BCD. So `நன்றி` "thanks" is
//!   `ந + ன + ் + ற + ி` — the pulli on the middle `ன` is a
//!   genuine word-internal mark that the tokenizer must keep inside
//!   the word.
//! * **Single stop-consonant series.** Unlike Devanagari and
//!   Bengali, which distinguish voiced / voiceless / aspirated /
//!   unaspirated at each place of articulation (~20 stops), Tamil's
//!   native script has **one stop per place** (k, c, ṭ, t, p);
//!   voicing is determined by context at pronunciation time, not
//!   orthographically marked. Grantha loans (`ஜ ஷ ஸ ஹ`) add a few
//!   more letters for Sanskrit-vocabulary words.
//! * **Two nasals and two liquids more than Devanagari.** Tamil
//!   uniquely writes an alveolar nasal (`ன` "ṉ") separate from the
//!   dental / palatal nasal (`ந` "n"), an alveolar tap (`ற` "ṟ")
//!   separate from the dental / retroflex r (`ர` "r"), a retroflex
//!   approximant (`ழ` "ḻ" — the famous "ḻ" of "tamiḻ"), and a
//!   retroflex lateral (`ள` "ḷ"). Each collapses to its ASCII base
//!   in the PHONEX-Tamil reduction.
//! * **Independent vowels vs. dependent vowel signs.** Tamil
//!   distinguishes two forms of every vowel: an **independent**
//!   form used at the start of a word (`அ ஆ இ ஈ உ ஊ எ ஏ ஐ ஒ ஓ
//!   ஔ` U+0B85..) and a **dependent** form (matra: `ா ி ீ ு ூ ெ
//!   ே ை ொ ோ ௌ` U+0BBE..) used after a consonant.
//! * **Tamil digits (`௦..௯` U+0BE6..=U+0BEF).** Tamil text uses
//!   either the Tamil digit block or ASCII `0..9`; the
//!   transliterator folds Tamil → ASCII.
//! * **No danda.** Unlike Devanagari and Bengali, Tamil does **not**
//!   inherit the danda `।` as a sentence terminator. Tamil uses
//!   ASCII `.`, `?`, `!` for sentence-final punctuation. The
//!   tokenizer treats those as separators via the standard
//!   `!is_alphanumeric` rule.
//!
//! The design choices are otherwise:
//!
//! * **Longest-match-wins Tamil suffix stripper as the stemmer baseline.**
//!   Tamil is agglutinative — nouns stack case + plural + possessive
//!   suffixes; verbs stack tense + person + honorific endings — and
//!   no rule-based light stemmer can cover it in full. There is **no
//!   canonical Snowball Tamil algorithm**. This pack ships a
//!   **deliberately conservative** longest-match-wins stripper
//!   covering the plural marker, the eight canonical case suffixes,
//!   the six present-tense verb personal endings, the tense-marker
//!   infixes that also appear in surface stems, the two common
//!   interrogative particles, and the future-tense marker.
//!   Documented as a **starter** — a follow-up
//!   `stringcheese-ta-morph` crate would ship the full analyzer.
//!   See [`crate::stemmer`].
//! * **ISO 15919 + PHONEX-Tamil two-stage phonetic encoder.** The
//!   scholarly Indic-script romanization ([`TamilIso15919`]) is
//!   exposed as its own public API — useful in its own right for
//!   data-migration tools and IR display — and the language pack's
//!   [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
//!   returns a [`TamilPhonex`] adapter that reduces the ISO 15919
//!   output to a 4-character Soundex-shape key. Adapter name:
//!   `"phonex-ta"`. This matches the shape of the other
//!   Latin-alphabet packs' phonetic hookups. See
//!   [`crate::phonetic`].
//! * **Matra-aware tokenizer.** The default splitter walks on
//!   `char::is_alphanumeric`, which treats matras / pulli as
//!   separators and would shatter a Tamil word at every dependent
//!   vowel sign. The Tamil tokenizer extends the rule to include
//!   the full Tamil block U+0B80..=U+0BFF as word scalars. See
//!   [`crate::tokenizer`].
//! * **~55-word stopword list.** Personal / demonstrative /
//!   interrogative pronouns, postpositions, conjunctions, particles,
//!   high-frequency forms of the auxiliary *இரு-* ("to be") and
//!   *ஆகு-* ("to become"), and common adverbs. See [`stopwords`].
//! * **Default Unicode collation.** Tamil sorts under CLDR's Tamil
//!   tailoring. This pack does not carry the CLDR tailoring data;
//!   [`Language::collator`](stringcheese_lang::Language::collator)
//!   returns `None`. Callers who need locale-aware Tamil collation
//!   should reach for `icu_collator`.
//!
//! # Deferred to a follow-up wave
//!
//! * **Malayalam / Telugu / Kannada packs.** The three sister
//!   Dravidian languages, each with its own Brahmic script
//!   (U+0D00..=U+0D7F Malayalam, U+0C00..=U+0C7F Telugu, U+0C80..
//!   Kannada). Each has its own morphology and function-word
//!   inventory.
//! * **Full Tamil morphological analysis.** Past-tense infixes
//!   interacting with sandhi at the stem boundary, honorific
//!   marking, aspectual auxiliaries, and the numerous non-finite
//!   verb forms. Handled by the follow-up `stringcheese-ta-morph`
//!   crate.
//! * **Modern colloquial Tamil.** The stemmer targets the written
//!   (centamiḻ) register; spoken (koṭuntamiḻ) surface forms diverge
//!   in the shape of many function words.
//! * **ITRANS / HK / SLP1 romanization adapters.** ISO 15919 is
//!   the scholarly baseline this pack ships with; additional
//!   romanization schemes are deferred.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_ta::TAMIL;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(TAMIL.code(), "ta");
//! assert_eq!(TAMIL.name(), "Tamil");
//! assert!(TAMIL.is_stopword("மற்றும்"));
//! assert!(!TAMIL.is_stopword("தமிழ்"));
//! // The light stemmer strips the plural marker -கள்.
//! assert_eq!(TAMIL.stem("மாணவர்கள்"), "மாணவர்");
//!
//! let toks: Vec<&str> = TAMIL
//!     .tokenize("நான் தமிழ் பேசுகிறேன்.")
//!     .collect();
//! assert_eq!(toks, ["நான்", "தமிழ்", "பேசுகிறேன்"]);
//! ```
//!
//! # Module map
//!
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`stemmer`] — the [`LightTamilStemmer`] suffix stripper.
//! - [`phonetic`] — the [`TamilIso15919`] transliteration and the
//!   [`TamilPhonex`] reduction; [`TamilPhonexAdapter`] is the type
//!   [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
//!   hands back.
//! - [`tokenizer`] — the [`TamilTokenizer`] whitespace-and-punctuation
//!   splitter.
//! - The [`Tamil`] type and the [`TAMIL`] constant live in this
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
pub use phonetic::{TamilIso15919, TamilPhonex, TamilPhonexAdapter};
#[cfg(feature = "alloc")]
pub use stemmer::LightTamilStemmer;
pub use stopwords::STOPWORDS;
pub use tokenizer::TamilTokenizer;

// -----------------------------------------------------------------------
// The Tamil language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::TamilPhonexAdapter;
    use crate::stemmer::LightTamilStemmer;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::TamilTokenizer;

    /// The Tamil language pack.
    ///
    /// Zero-sized; construct as [`Tamil`] and reuse the value freely
    /// across threads and calls, or grab the crate-level
    /// [`TAMIL`](crate::TAMIL) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Tamil;

    /// The static [`TamilPhonexAdapter`] [`Tamil`] hands back from
    /// [`phonetic_encoder`](Language::phonetic_encoder).
    static TAMIL_PHONEX: TamilPhonexAdapter = TamilPhonexAdapter;

    impl Language for Tamil {
        fn code(&self) -> &'static str {
            "ta"
        }

        fn name(&self) -> &'static str {
            "Tamil"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            LightTamilStemmer.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(TamilTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&TAMIL_PHONEX)
        }
    }

    /// The singleton [`Tamil`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Tamil`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other
    /// `stringcheese-<lang>` pack follows.
    pub const TAMIL: Tamil = Tamil;
}

#[cfg(feature = "alloc")]
pub use pack::{TAMIL, Tamil};

// Register into `stringcheese-lang::registry` so callers who look up
// languages dynamically (`registry::language("ta")`) find this pack.
// alloc-gated because the pack constant itself is alloc-gated.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(TAMIL);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-ta` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
