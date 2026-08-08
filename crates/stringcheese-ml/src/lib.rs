//! Malayalam language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Malayalam`] value that carries a ~55-entry
//! Malayalam stopword list, the [`LightMalayalamStemmer`] suffix
//! stripper, the [`MalayalamTokenizer`] whitespace-and-punctuation
//! word splitter, and a [`MalayalamPhonex`] PHONEX-Malayalam
//! (Soundex-shape 4-char) key computed over an ISO 15919
//! Malayalam → Latin transliteration ([`MalayalamIso15919`]) as the
//! phonetic encoder. Callers grab the singleton [`MALAYALAM`]
//! `const` — no construction ceremony required — and delegate
//! through the [`Language`](stringcheese_lang::Language) trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-ml` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Malayalam stopword
//! table or the ISO 15919 transliteration table. Callers who need
//! Malayalam add `stringcheese-ml = "0.1"` to their `Cargo.toml`
//! explicitly.
//!
//! # Second Dravidian, fourth Brahmic-script pack
//!
//! Malayalam is the **second Dravidian language** in StringCheese
//! (sibling of Tamil at `stringcheese-ta`) and the **fourth
//! Brahmic-script pack** (after `stringcheese-hi` for Devanagari,
//! `stringcheese-bn` for Bengali, and `stringcheese-ta` for Tamil).
//! It is the first Malayalam-script pack.
//!
//! Malayalam script (U+0D00..=U+0D7F) is a Brahmic-family abugida
//! like Devanagari, Bengali, and Tamil. Every Malayalam letter is
//! encoded in **3 UTF-8 bytes** (the block falls in UTF-8's 3-byte
//! range), so the pack reuses the byte-safe processing model
//! validated by the earlier Brahmic packs: all suffix / tokenizer /
//! stemmer arithmetic runs on `Vec<char>`, never raw byte offsets.
//!
//! ## The Malayalam-specific invariants
//!
//! Malayalam is a **left-to-right script** (no RTL surprises). The
//! things to remember are *storage width*, the *abugida script
//! model*, the *presence of conjunct consonants* (unlike Tamil, but
//! like Devanagari and Bengali), and the *chillu letters* (unique
//! among Brahmic scripts):
//!
//! * **Every Malayalam letter is 3 bytes in UTF-8.** The block
//!   U+0D00..=U+0D7F falls in UTF-8's 3-byte range
//!   (U+0800..=U+FFFF), so a word like `"ഞാൻ"` (3 characters:
//!   `ഞ` + `ാ` + `ൻ`) is **9 bytes**. Cyrillic is 2 bytes per
//!   letter; Latin is 1. Any code that mixes byte offsets with
//!   character-boundary logic will silently corrupt token or suffix
//!   boundaries. This crate operates exclusively on `Vec<char>` and
//!   [`str::chars`] iteration — never raw byte offsets.
//! * **Conjunct consonants formed via virama** (`്` U+0D4D). Unlike
//!   Tamil, which writes every consonant cluster with a visible
//!   pulli and lacks true conjuncts, Malayalam *does* form
//!   ligature-style conjunct consonants when a base consonant is
//!   followed by virama + another consonant. Some clusters render
//!   as ligatures (`ത്യ`, `ന്ത`), some as visible virama sequences
//!   (`സത്യം`). The transliterator treats them uniformly — virama
//!   suppresses the schwa on the preceding consonant regardless of
//!   glyph shape.
//! * **Chillu letters (U+0D7A..=U+0D7F) — Malayalam-only.** Six
//!   atomic "pure consonant" scalars — `ൺ` (chillu ṇ, from ണ),
//!   `ൻ` (chillu n, from ന), `ർ` (chillu r, from ര), `ൽ` (chillu
//!   l, from ല), `ൾ` (chillu ḷ, from ള), `ൿ` (chillu k, from ക) —
//!   that represent a word-final consonant without any following
//!   vowel. Words like `ഞാൻ` "I", `അവർ` "they", `അവൾ` "she" end
//!   in a bare chillu. The tokenizer keeps them inside their
//!   word; the stemmer takes care **not** to strip a standalone
//!   chillu as if it were a suffix (its suffix table lists only
//!   multi-scalar chillu-carrying endings like `-ിൽ`, `-ന്റെ`,
//!   `-ന്`, `-കൾ`, `-മാർ`); the phonetic encoder emits the base
//!   consonant with no schwa.
//! * **Independent vowels vs. dependent vowel signs.** Malayalam
//!   distinguishes two forms of every vowel: an **independent**
//!   form used at the start of a word (`അ ആ ഇ ഈ ഉ ഊ ഋ എ ഏ ഐ ഒ ഓ
//!   ഔ` U+0D05..) and a **dependent** form (matra: `ാ ി ീ ു ൂ ൃ
//!   െ േ ൈ ൊ ോ ൌ` U+0D3E..) used after a consonant.
//! * **Malayalam digits (`൦..൯` U+0D66..=U+0D6F).** Malayalam text
//!   uses either the Malayalam digit block or ASCII `0..9`; the
//!   transliterator folds Malayalam → ASCII.
//! * **No danda.** Like Tamil and unlike Devanagari / Bengali,
//!   modern Malayalam does **not** use the danda `।` as a sentence
//!   terminator. Malayalam uses ASCII `.`, `?`, `!` for
//!   sentence-final punctuation. The tokenizer treats those as
//!   separators via the standard `!is_alphanumeric` rule.
//! * **Zero-width joiner / non-joiner (U+200D / U+200C).**
//!   Malayalam uses these to control conjunct formation in typing
//!   input. Both sit outside the Malayalam block and both fail
//!   `is_alphanumeric`, so the tokenizer treats them as separators.
//!   Callers who want to preserve them can pre-normalize; the
//!   default tokenizer's behavior is deliberately conservative.
//!
//! The design choices are otherwise:
//!
//! * **Longest-match-wins Malayalam suffix stripper as the stemmer
//!   baseline.** Malayalam is agglutinative — nouns stack case +
//!   plural + possessive suffixes; verbs stack tense + aspect +
//!   adjectival endings — and no rule-based light stemmer can
//!   cover it in full. There is **no canonical Snowball Malayalam
//!   algorithm**. This pack ships a **deliberately conservative**
//!   longest-match-wins stripper covering the six canonical case
//!   suffixes (accusative `-നെ`; genitive `-ന്റെ` / `-ുടെ`;
//!   dative `-ന്`; locative `-ിൽ`; instrumental `-ാൽ` /
//!   `-ിനാൽ`; sociative `-ോട്`), the two plural markers (`-കൾ`
//!   inanimate; `-മാർ` animate; plus the `ങ്ങ`-linked variant
//!   `-ങ്ങൾ`), the highest-frequency verb tense / aspect and
//!   adjectival endings (`-ുന്നു` present; `-ുന്ന`
//!   present-adjectival; `-ിയ` past-adjectival; `-ുക`
//!   infinitive), the emphatic particles (`-ും` "also" / future;
//!   `-ോ` interrogative; `-ല്ലേ` tag question), all matched on
//!   `Vec<char>` with a 2-scalar min-stem guard, and with
//!   **bare chillu-letter word endings deliberately left in
//!   place**. Documented as a **starter** — a follow-up
//!   `stringcheese-ml-morph` crate would ship the full analyzer.
//!   See [`crate::stemmer`].
//! * **ISO 15919 + PHONEX-Malayalam two-stage phonetic encoder.**
//!   The scholarly Indic-script romanization
//!   ([`MalayalamIso15919`]) is exposed as its own public API —
//!   useful in its own right for data-migration tools and IR
//!   display — and the language pack's
//!   [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
//!   returns a [`MalayalamPhonex`] adapter that reduces the ISO
//!   15919 output to a 4-character Soundex-shape key. Adapter
//!   name: `"phonex-ml"`. This matches the shape of the other
//!   Latin-alphabet packs' phonetic hookups. See
//!   [`crate::phonetic`].
//! * **Matra-and-chillu-aware tokenizer.** The default splitter
//!   walks on `char::is_alphanumeric`, which treats matras /
//!   virama / combining marks as separators and would shatter a
//!   Malayalam word at every dependent vowel sign. The Malayalam
//!   tokenizer extends the rule to include the full Malayalam
//!   block U+0D00..=U+0D7F as word scalars, so matras, virama,
//!   anusvara, visarga, and the six chillu letters all stay
//!   word-internal. See [`crate::tokenizer`].
//! * **~55-word stopword list.** Personal / demonstrative /
//!   interrogative pronouns, postpositions, conjunctions, negation
//!   and affirmation particles, high-frequency forms of the copula
//!   *ആണ്* / *ഉണ്ട്* ("to be"), and common adverbs. See
//!   [`stopwords`].
//! * **Default Unicode collation.** Malayalam sorts under CLDR's
//!   Malayalam tailoring. This pack does not carry the CLDR
//!   tailoring data;
//!   [`Language::collator`](stringcheese_lang::Language::collator)
//!   returns `None`. Callers who need locale-aware Malayalam
//!   collation should reach for `icu_collator`.
//!
//! # Deferred to a follow-up wave
//!
//! * **Telugu / Kannada packs.** The two remaining Dravidian
//!   languages, each with its own Brahmic script (U+0C00..=U+0C7F
//!   Telugu, U+0C80..=U+0CFF Kannada). Each has its own morphology
//!   and function-word inventory.
//! * **Full Malayalam morphological analysis.** Sandhi-driven
//!   consonant alternations at stem boundaries, honorific marking,
//!   aspectual auxiliaries, and the numerous non-finite verb
//!   forms. Handled by the follow-up `stringcheese-ml-morph`
//!   crate.
//! * **Modern colloquial Malayalam.** The stemmer targets the
//!   written register; spoken surface forms diverge in the shape
//!   of many function words.
//! * **ITRANS / HK / SLP1 romanization adapters.** ISO 15919 is
//!   the scholarly baseline this pack ships with; additional
//!   romanization schemes are deferred.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_ml::MALAYALAM;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(MALAYALAM.code(), "ml");
//! assert_eq!(MALAYALAM.name(), "Malayalam");
//! assert!(MALAYALAM.is_stopword("ഒപ്പം"));
//! assert!(!MALAYALAM.is_stopword("മലയാളം"));
//! // The light stemmer strips the plural marker -ങ്ങൾ.
//! assert_eq!(MALAYALAM.stem("പുസ്തകങ്ങൾ"), "പുസ്തക");
//!
//! let toks: Vec<&str> = MALAYALAM
//!     .tokenize("ഞാൻ മലയാളം സംസാരിക്കുന്നു.")
//!     .collect();
//! assert_eq!(toks, ["ഞാൻ", "മലയാളം", "സംസാരിക്കുന്നു"]);
//! ```
//!
//! # Module map
//!
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`stemmer`] — the [`LightMalayalamStemmer`] suffix stripper.
//! - [`phonetic`] — the [`MalayalamIso15919`] transliteration and
//!   the [`MalayalamPhonex`] reduction; [`MalayalamPhonexAdapter`]
//!   is the type
//!   [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
//!   hands back.
//! - [`tokenizer`] — the [`MalayalamTokenizer`]
//!   whitespace-and-punctuation splitter.
//! - The [`Malayalam`] type and the [`MALAYALAM`] constant live in
//!   this crate's root.

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
pub use phonetic::{MalayalamIso15919, MalayalamPhonex, MalayalamPhonexAdapter};
#[cfg(feature = "alloc")]
pub use stemmer::LightMalayalamStemmer;
pub use stopwords::STOPWORDS;
pub use tokenizer::MalayalamTokenizer;

// -----------------------------------------------------------------------
// The Malayalam language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::MalayalamPhonexAdapter;
    use crate::stemmer::LightMalayalamStemmer;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::MalayalamTokenizer;

    /// The Malayalam language pack.
    ///
    /// Zero-sized; construct as [`Malayalam`] and reuse the value
    /// freely across threads and calls, or grab the crate-level
    /// [`MALAYALAM`](crate::MALAYALAM) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Malayalam;

    /// The static [`MalayalamPhonexAdapter`] [`Malayalam`] hands back
    /// from [`phonetic_encoder`](Language::phonetic_encoder).
    static MALAYALAM_PHONEX: MalayalamPhonexAdapter = MalayalamPhonexAdapter;

    impl Language for Malayalam {
        fn code(&self) -> &'static str {
            "ml"
        }

        fn name(&self) -> &'static str {
            "Malayalam"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            LightMalayalamStemmer.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(MalayalamTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&MALAYALAM_PHONEX)
        }
    }

    /// The singleton [`Malayalam`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Malayalam`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other
    /// `stringcheese-<lang>` pack follows.
    pub const MALAYALAM: Malayalam = Malayalam;
}

#[cfg(feature = "alloc")]
pub use pack::{MALAYALAM, Malayalam};

// Register into `stringcheese-lang::registry` so callers who look up
// languages dynamically (`registry::language("ml")`) find this pack.
// alloc-gated because the pack constant itself is alloc-gated.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(MALAYALAM);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-ml` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
