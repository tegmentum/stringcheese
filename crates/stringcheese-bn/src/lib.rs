//! Bengali language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Bengali`] value that carries a ~65-entry Bengali
//! stopword list, the [`LightBengaliStemmer`] suffix stripper, the
//! [`BengaliTokenizer`] whitespace-and-punctuation word splitter
//! (danda `।` aware), and a [`BengaliPhonex`] PHONEX-Bengali
//! (Soundex-shape 4-char) key computed over an ISO 15919 Bengali →
//! Latin transliteration ([`BengaliIso15919`]) as the phonetic
//! encoder. Callers grab the singleton [`BENGALI`] `const` — no
//! construction ceremony required — and delegate through the
//! [`Language`](stringcheese_lang::Language) trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-bn` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Bengali stopword table
//! or the ISO 15919 transliteration table. Callers who need Bengali
//! add `stringcheese-bn = "0.1"` to their `Cargo.toml` explicitly.
//!
//! # Second Brahmic-script pack
//!
//! Bengali is the **second Brahmic-script pack** in StringCheese
//! (after `stringcheese-hi`, which handles Devanagari). Bengali
//! script (U+0980..=U+09FF) is a sibling of Devanagari
//! (U+0900..=U+097F) — both are abugidas in the Brahmic family, both
//! carry inherent-schwa consonants overridden by dependent vowel
//! signs (matras / kars) and a virama (halant / hasant in Bengali;
//! `্` U+09CD), and both encode every letter in **3 UTF-8 bytes**
//! (the two blocks fall in UTF-8's 3-byte range). The pack reuses
//! the byte-safe processing model validated by `stringcheese-hi`:
//! all suffix / tokenizer / stemmer arithmetic runs on `Vec<char>`,
//! never raw byte offsets.
//!
//! ## The Bengali-specific invariants
//!
//! Bengali is a **left-to-right script** (no RTL surprises). The
//! things to remember are *storage width* and the *abugida script
//! model*:
//!
//! * **Every Bengali letter is 3 bytes in UTF-8.** The block
//!   U+0980..=U+09FF falls in UTF-8's 3-byte range
//!   (U+0800..=U+FFFF), so a word like `"আমি"` (3 characters:
//!   `আ` + `ম` + `ি`) is **9 bytes**. Cyrillic is 2 bytes per
//!   letter; Latin is 1. Any code that mixes byte offsets with
//!   character-boundary logic will silently corrupt token or suffix
//!   boundaries. This crate operates exclusively on `Vec<char>` and
//!   [`str::chars`] iteration — never raw byte offsets.
//! * **Independent vowels vs. dependent vowel signs.** Bengali
//!   distinguishes two forms of every vowel: an **independent** form
//!   used at the start of a word (`অ আ ই ঈ উ ঊ এ ঐ ও ঔ` U+0985..)
//!   and a **dependent** form (matra / kar: `া ি ী ু ূ ে ৈ ো ৌ`
//!   U+09BE..) used after a consonant.
//! * **Halant/hasant and consonant clusters.** Bengali is an
//!   **abugida**: every base consonant carries an implicit `a`
//!   (schwa) vowel unless a matra or a **halant** (`্` U+09CD)
//!   overrides it. `ক` alone represents `ka` (with schwa); `ক্`
//!   (`ক` + halant) represents bare `k`; `সত্য` (`স` + `ত` +
//!   halant + `য`) represents `satya` (dental `t` cluster). The
//!   [`phonetic::BengaliIso15919`] transliteration honors this by
//!   default.
//! * **Nukta (`়` U+09BC).** A combining mark that modifies base
//!   consonants to produce the three Bengali extension letters
//!   (`ড়` "ṛ" retroflex flap; `ঢ়` "ṛh" breathy retroflex flap;
//!   `য়` "ẏ" palatal glide). Nukta is **semantically meaningful** —
//!   `য` (dental "y") and `য়` (palatal glide "ẏ") are different
//!   phonemes — so the transliterator preserves the distinction
//!   (and handles both precomposed U+09DC/U+09DD/U+09DF and
//!   decomposed `base + ়` forms).
//! * **Khanda ta (`ৎ` U+09CE).** A special "half" form of `ত` used
//!   at word-final position to spell final /t/ without a schwa
//!   (e.g. `পৎ` "pat"). Never carries an inherent vowel.
//! * **Bengali digits (`০..৯` U+09E6..=U+09EF).** Bengali text uses
//!   either the Bengali digit block or ASCII `0..9`; the
//!   transliterator folds Bengali → ASCII.
//! * **Danda (`।` U+0964) — the Bengali "full stop".** Bengali
//!   inherits the Devanagari danda as its sentence terminator; the
//!   tokenizer treats it as a separator (see [`crate::tokenizer`]).
//!
//! The design choices are otherwise:
//!
//! * **Light Bengali suffix stripper as the stemmer baseline.**
//!   There is **no canonical Snowball Bengali algorithm**. Snowball's
//!   catalogue lists Bengali as "planned" but ships no `bengali.sbl`.
//!   The Ramanathan-Rao 2003 Indic template and the Loponen-Järvelin
//!   2010 statistical approach are the community references; this
//!   pack ships a deliberately conservative rule-based subset
//!   covering plural markers (`-গুলি -গুলো -দের -রা`) and the
//!   most-common case endings (`-কে -তে -র -রে -য়`). Documented as
//!   a **starter** — a follow-up `stringcheese-bn-morph` crate would
//!   ship the full analyzer. See [`crate::stemmer`].
//! * **ISO 15919 + PHONEX-Bengali two-stage phonetic encoder.** The
//!   scholarly Indic-script romanization ([`BengaliIso15919`]) is
//!   exposed as its own public API — useful in its own right for
//!   data-migration tools and IR display — and the language pack's
//!   [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
//!   returns a [`BengaliPhonex`] adapter that reduces the ISO 15919
//!   output to a 4-character Soundex-shape key. Adapter name:
//!   `"phonex-bn"`. This matches the shape of the other
//!   Latin-alphabet packs' phonetic hookups (Czech, French, Dutch,
//!   Portuguese, Spanish, Swedish, Norwegian, Finnish, Polish,
//!   Hungarian, Slovak, Vietnamese — all PHONEX-family). See
//!   [`crate::phonetic`].
//! * **Matra-aware tokenizer.** The default splitter walks on
//!   `char::is_alphanumeric`, which treats matras / halant /
//!   anusvara as separators and would shatter a Bengali word at
//!   every dependent vowel sign. The Bengali tokenizer extends the
//!   rule to include the full Bengali block U+0980..=U+09FF as word
//!   scalars. See [`crate::tokenizer`].
//! * **~65-word stopword list.** Personal / demonstrative /
//!   interrogative pronouns, postpositions, conjunctions, particles,
//!   high-frequency forms of the copula *হওয়া* ("to be") and
//!   auxiliary *করা* ("to do"), and common adverbs. See
//!   [`stopwords`].
//! * **Default Unicode collation.** Bengali sorts under CLDR's
//!   Bengali tailoring. This pack does not carry the CLDR tailoring
//!   data;
//!   [`Language::collator`](stringcheese_lang::Language::collator)
//!   returns `None`. Callers who need locale-aware Bengali
//!   collation should reach for `icu_collator`.
//!
//! # Deferred to a follow-up wave
//!
//! * **Assamese pack (`stringcheese-as`).** Assamese uses the
//!   Bengali script with two additional letters (`ৰ` U+09F0 "ra
//!   with middle diagonal", `ৱ` U+09F1 "wa"). The ISO 15919
//!   transliterator here would extend directly; the stopword list
//!   and stemmer would differ.
//! * **Manipuri (Bengali script variant), Rohingya, and other
//!   users of the Bengali script.** Each carries its own morphology
//!   and function-word inventory.
//! * **Full Snowball Bengali.** If a canonical `bengali.sbl` ever
//!   appears in the Snowball catalogue, ship the port under a
//!   Cargo feature (`snowball-bn`) alongside the current light
//!   stemmer.
//! * **Schwa-deletion.** The ISO 15919 encoder honors the
//!   inherent-schwa convention. Modern colloquial Bengali very
//!   actively drops the schwa in many word-final and medial
//!   positions; the deletion is context-dependent and
//!   lexicon-driven — implementing it needs a Bengali lexicon and
//!   is deferred to the morphology crate.
//! * **ITRANS / HK / SLP1 romanization adapters.** ISO 15919 is
//!   the scholarly baseline this pack ships with; additional
//!   romanization schemes are deferred.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_bn::BENGALI;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(BENGALI.code(), "bn");
//! assert_eq!(BENGALI.name(), "Bengali");
//! assert!(BENGALI.is_stopword("এবং"));
//! assert!(!BENGALI.is_stopword("বই"));
//! // The light stemmer strips the plural marker -গুলি.
//! assert_eq!(BENGALI.stem("বইগুলি"), "বই");
//!
//! let toks: Vec<&str> = BENGALI
//!     .tokenize("আমি বাংলা বলি।")
//!     .collect();
//! assert_eq!(toks, ["আমি", "বাংলা", "বলি"]);
//! ```
//!
//! # Module map
//!
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`stemmer`] — the [`LightBengaliStemmer`] suffix stripper.
//! - [`phonetic`] — the [`BengaliIso15919`] transliteration and the
//!   [`BengaliPhonex`] reduction; [`BengaliPhonexAdapter`] is the
//!   type [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
//!   hands back.
//! - [`tokenizer`] — the [`BengaliTokenizer`] whitespace-and-Bengali-
//!   punctuation splitter.
//! - The [`Bengali`] type and the [`BENGALI`] constant live in this
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
pub use phonetic::{BengaliIso15919, BengaliPhonex, BengaliPhonexAdapter};
#[cfg(feature = "alloc")]
pub use stemmer::LightBengaliStemmer;
pub use stopwords::STOPWORDS;
pub use tokenizer::BengaliTokenizer;

// -----------------------------------------------------------------------
// The Bengali language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::BengaliPhonexAdapter;
    use crate::stemmer::LightBengaliStemmer;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::BengaliTokenizer;

    /// The Bengali language pack.
    ///
    /// Zero-sized; construct as [`Bengali`] and reuse the value
    /// freely across threads and calls, or grab the crate-level
    /// [`BENGALI`](crate::BENGALI) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Bengali;

    /// The static [`BengaliPhonexAdapter`] [`Bengali`] hands back
    /// from [`phonetic_encoder`](Language::phonetic_encoder).
    static BENGALI_PHONEX: BengaliPhonexAdapter = BengaliPhonexAdapter;

    impl Language for Bengali {
        fn code(&self) -> &'static str {
            "bn"
        }

        fn name(&self) -> &'static str {
            "Bengali"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            LightBengaliStemmer.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(BengaliTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&BENGALI_PHONEX)
        }
    }

    /// The singleton [`Bengali`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Bengali`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other
    /// `stringcheese-<lang>` pack follows.
    pub const BENGALI: Bengali = Bengali;
}

#[cfg(feature = "alloc")]
pub use pack::{BENGALI, Bengali};

// Register into `stringcheese-lang::registry` so callers who look up
// languages dynamically (`registry::language("bn")`) find this pack.
// alloc-gated because the pack constant itself is alloc-gated.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(BENGALI);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-bn` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
