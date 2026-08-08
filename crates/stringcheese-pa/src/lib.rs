//! Punjabi (Eastern, Gurmukhi script) language pack for the
//! StringCheese toolkit.
//!
//! A zero-sized [`Punjabi`] value that carries a ~55-entry Gurmukhi
//! stopword list, the [`LightPunjabiStemmer`] suffix stripper, the
//! [`PunjabiTokenizer`] whitespace-and-punctuation word splitter
//! (Devanagari-inherited danda `।` aware), and a [`PunjabiPhonex`]
//! PHONEX-Punjabi (Soundex-shape 4-char) key computed over an ISO
//! 15919 Gurmukhi → Latin transliteration ([`PunjabiIso15919`]) with
//! a Punjabi-specific tone-collapse pre-pass. Callers grab the
//! singleton [`PUNJABI`] `const` — no construction ceremony required —
//! and delegate through the [`Language`](stringcheese_lang::Language)
//! trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-pa` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Punjabi stopword table
//! or the ISO 15919 transliteration table. Callers who need Punjabi
//! add `stringcheese-pa = "0.1"` to their `Cargo.toml` explicitly.
//!
//! # First Gurmukhi-script pack; fifth Brahmic-script pack
//!
//! Punjabi is the **first Gurmukhi-script pack** in StringCheese and
//! the **fifth Brahmic-script pack** — after `stringcheese-hi`
//! (Devanagari), `stringcheese-bn` (Bengali), `stringcheese-ta`
//! (Tamil), and `stringcheese-ml` (Malayalam) when it lands. The
//! Gurmukhi script (U+0A00..=U+0A7F) is an abugida in the Brahmic
//! family. Every Gurmukhi letter is encoded in **3 UTF-8 bytes** (the
//! block falls in UTF-8's 3-byte range), so the pack reuses the
//! byte-safe processing model validated by the earlier Brahmic packs:
//! all suffix / tokenizer / stemmer arithmetic runs on `Vec<char>`,
//! never raw byte offsets.
//!
//! Only Eastern Punjabi (Gurmukhi) is shipped in this pack. Western
//! Punjabi is written in a Perso-Arabic derivative called Shahmukhi;
//! a companion `stringcheese-pa-arab` pack (BCP-47 `pa-Arab`) is
//! deferred.
//!
//! ## The Punjabi-specific invariants
//!
//! Punjabi is a **left-to-right script** (no RTL surprises). The
//! things to remember are *storage width*, the *abugida script
//! model*, and the *tonal phonology*:
//!
//! * **Every Gurmukhi letter is 3 bytes in UTF-8.** The block
//!   U+0A00..=U+0A7F falls in UTF-8's 3-byte range
//!   (U+0800..=U+FFFF), so a word like `"ਪੰਜਾਬ"` (5 characters:
//!   `ਪ` + `ੰ` + `ਜ` + `ਾ` + `ਬ`) is **15 bytes**. Cyrillic is 2
//!   bytes per letter; Latin is 1. Any code that mixes byte offsets
//!   with character-boundary logic will silently corrupt token or
//!   suffix boundaries. This crate operates exclusively on
//!   `Vec<char>` and [`str::chars`] iteration — never raw byte
//!   offsets.
//! * **Independent vowels vs. dependent vowel signs.** Gurmukhi
//!   distinguishes two forms of every vowel: an **independent** form
//!   used at the start of a word (`ਅ ਆ ਇ ਈ ਉ ਊ ਏ ਐ ਓ ਔ` U+0A05..)
//!   and a **dependent** form (matra: `ਾ ਿ ੀ ੁ ੂ ੇ ੈ ੋ ੌ`
//!   U+0A3E..) used after a consonant.
//! * **Virama / halant and consonant clusters.** Gurmukhi is an
//!   **abugida**: every base consonant carries an implicit `a`
//!   (schwa) vowel unless a matra or a **virama** (`੍` U+0A4D)
//!   overrides it. Consonant clusters are usually written with
//!   subjoined half-form letters (visible in the font); the encoded
//!   sequence uses base + virama + base.
//! * **Punjabi is a tonal language.** The historical Sanskrit-inherited
//!   voiced-aspirate letters `ਘ` / `ਝ` / `ਢ` / `ਧ` / `ਭ` have **lost
//!   their voicing and aspiration** in modern Punjabi. What remains
//!   is a *tone contour* on the adjacent vowel — low tone when the
//!   historical aspirate begins the syllable, high tone when it
//!   ends. The letter shapes are still on Sikh signs and in
//!   religious texts, but the phone is now the same as the
//!   corresponding voiceless-unaspirated stop (`k`/`c`/`ṭ`/`t`/`p`).
//!   The [`PunjabiIso15919`] transliterator preserves the ISO 15919
//!   spellings `gh`/`jh`/`ḍh`/`dh`/`bh`; the [`PunjabiPhonex`]
//!   reduction applies a tone-collapse pre-pass that folds them to
//!   the voiceless-unaspirated form so tone-marked and unmarked
//!   spellings share a phonex key.
//! * **Addak (`ੱ` U+0A71) geminates the following consonant.**
//!   `ਪੱਕਾ` (pakkā, "ripe") — `ਪ + ੱ + ਕ + ਾ` — encodes as
//!   `p` + inherent schwa + gemination-marker + `k` + `ā`,
//!   transcribed `pakkā`.
//! * **Tippi (`ੰ` U+0A70) and bindi (`ਂ` U+0A02) nasalize a
//!   vowel.** Tippi is anusvara-like (transliterated `ṁ`); bindi is
//!   chandrabindu-like (transliterated `m̐`). Both fold to `M` in
//!   the phonex reduction.
//! * **Nukta (`਼` U+0A3C).** A combining mark that produces
//!   Perso-Arabic loans: `ਖ਼` (x), `ਗ਼` (ġ), `ਜ਼` (z), `ਫ਼`
//!   (f); the native Punjabi retroflex flap `ੜ` (ṛ) is likewise
//!   encoded as a nukta-precomposed form (`ਡ + ਼`). The
//!   transliterator handles both precomposed forms and decomposed
//!   base + `਼` sequences.
//! * **Gurmukhi digits (`੦..੯` U+0A66..=U+0A6F).** Punjabi text uses
//!   either the Gurmukhi digit block or ASCII `0..9`; the
//!   transliterator folds Gurmukhi → ASCII.
//! * **Danda (`।` U+0964) — inherited from Devanagari.** Gurmukhi
//!   inherits the Devanagari danda as its sentence terminator in
//!   traditional typography; modern newspaper Punjabi mixes ASCII
//!   `.` widely too. The double danda `॥` U+0965 marks end of
//!   verse in Sikh religious texts (notably the Guru Granth Sahib).
//!   The tokenizer treats both as separators (see [`crate::tokenizer`]).
//!
//! The design choices are otherwise:
//!
//! * **Light Punjabi suffix stripper as the stemmer baseline.**
//!   There is **no canonical Snowball Punjabi algorithm** — Snowball
//!   does not list Punjabi. The community references are
//!   Kumar & Josan (2010) *A stemming algorithm for Punjabi
//!   language* and Gupta (2013)'s successor. This pack ships a
//!   deliberately conservative rule-based subset covering case
//!   markers (`-ੇ` obl sg, `-ੀ` fem sg), plural markers (`-ਾਂ`,
//!   `-ਆਂ`, `-ੀਆਂ`, `-ਿਆਂ`), imperfective participles
//!   (`-ਦਾ`/`-ਦੀ`/`-ਦੇ`), and perfective/aorist endings
//!   (`-ਿਆ` 3sg-m, `-ੀ` 3sg-f, `-ੇ` 3pl-m, `-ੀਆਂ` 3pl-f). Tippi,
//!   bindi, and addak are never stripped alone. Documented as a
//!   **starter** — a follow-up `stringcheese-pa-morph` crate would
//!   ship the full analyzer. See [`crate::stemmer`].
//! * **ISO 15919 + PHONEX-Punjabi two-stage phonetic encoder** with
//!   a tone-collapse pre-pass between the stages. The scholarly
//!   Indic-script romanization ([`PunjabiIso15919`]) is exposed as
//!   its own public API — useful in its own right for data-migration
//!   tools and IR display — and the language pack's
//!   [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
//!   returns a [`PunjabiPhonex`] adapter that first collapses
//!   tone-bearing letters to their voiceless-unaspirated counterparts
//!   and then reduces the result to a 4-character Soundex-shape key.
//!   Adapter name: `"phonex-pa"`. See [`crate::phonetic`].
//! * **Matra-aware tokenizer.** The default splitter walks on
//!   `char::is_alphanumeric`, which treats matras / virama / tippi /
//!   bindi / addak / nukta as separators and would shatter a
//!   Punjabi word at every dependent mark. The Punjabi tokenizer
//!   extends the rule to include the full Gurmukhi block
//!   U+0A00..=U+0A7F as word scalars. See [`crate::tokenizer`].
//! * **~55-word stopword list.** Personal / demonstrative /
//!   interrogative pronouns, postpositions, conjunctions, particles,
//!   high-frequency forms of the copula *ਹੋਣਾ* ("to be"), and
//!   common adverbs. See [`stopwords`].
//! * **Default Unicode collation.** Punjabi sorts under CLDR's
//!   Punjabi tailoring. This pack does not carry the CLDR tailoring
//!   data; [`Language::collator`](stringcheese_lang::Language::collator)
//!   returns `None`. Callers who need locale-aware Punjabi collation
//!   should reach for `icu_collator`.
//!
//! # Deferred to a follow-up wave
//!
//! * **Shahmukhi (Perso-Arabic) Punjabi pack (`stringcheese-pa-arab`,
//!   BCP-47 `pa-Arab`).** Western Punjabi is written in Shahmukhi;
//!   the function-word inventory overlaps but the surface spellings
//!   are entirely different, and the RTL script model differs
//!   completely.
//! * **Regional dialect surface forms.** The stopword list targets
//!   the written Majhi (standard) register; Doabi, Malwai, Puadhi,
//!   Pothohari surface forms diverge in the shape of many function
//!   words.
//! * **Full Snowball Punjabi.** If a canonical `punjabi.sbl` ever
//!   appears in the Snowball catalogue, ship the port under a
//!   Cargo feature (`snowball-pa`) alongside the current light
//!   stemmer.
//! * **Explicit tone-marking in transliteration.** The current
//!   [`PunjabiIso15919`] preserves the ISO 15919 spelling of the
//!   tone letters (`gh`/`jh`/`ḍh`/`dh`/`bh`); an alternate
//!   `tone-preserving` encoder that emits explicit low- / high-tone
//!   marks on adjacent vowels is deferred.
//! * **ITRANS / HK / SLP1 romanization adapters.** ISO 15919 is
//!   the scholarly baseline this pack ships with; additional
//!   romanization schemes are deferred.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_pa::PUNJABI;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(PUNJABI.code(), "pa");
//! assert_eq!(PUNJABI.name(), "Punjabi");
//! assert!(PUNJABI.is_stopword("ਅਤੇ"));
//! assert!(!PUNJABI.is_stopword("ਪੰਜਾਬ"));
//! // The light stemmer strips the plural marker -ਾਂ.
//! assert_eq!(PUNJABI.stem("ਘਰਾਂ"), "ਘਰ");
//!
//! let toks: Vec<&str> = PUNJABI
//!     .tokenize("ਮੈਂ ਪੰਜਾਬੀ ਬੋਲਦਾ ਹਾਂ।")
//!     .collect();
//! assert_eq!(toks, ["ਮੈਂ", "ਪੰਜਾਬੀ", "ਬੋਲਦਾ", "ਹਾਂ"]);
//! ```
//!
//! # Module map
//!
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`stemmer`] — the [`LightPunjabiStemmer`] suffix stripper.
//! - [`phonetic`] — the [`PunjabiIso15919`] transliteration and the
//!   [`PunjabiPhonex`] reduction; [`PunjabiPhonexAdapter`] is the
//!   type [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
//!   hands back.
//! - [`tokenizer`] — the [`PunjabiTokenizer`] whitespace-and-Gurmukhi-
//!   punctuation splitter.
//! - The [`Punjabi`] type and the [`PUNJABI`] constant live in this
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
pub use phonetic::{PunjabiIso15919, PunjabiPhonex, PunjabiPhonexAdapter};
#[cfg(feature = "alloc")]
pub use stemmer::LightPunjabiStemmer;
pub use stopwords::STOPWORDS;
pub use tokenizer::PunjabiTokenizer;

// -----------------------------------------------------------------------
// The Punjabi language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::PunjabiPhonexAdapter;
    use crate::stemmer::LightPunjabiStemmer;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::PunjabiTokenizer;

    /// The Punjabi language pack.
    ///
    /// Zero-sized; construct as [`Punjabi`] and reuse the value freely
    /// across threads and calls, or grab the crate-level
    /// [`PUNJABI`](crate::PUNJABI) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Punjabi;

    /// The static [`PunjabiPhonexAdapter`] [`Punjabi`] hands back
    /// from [`phonetic_encoder`](Language::phonetic_encoder).
    static PUNJABI_PHONEX: PunjabiPhonexAdapter = PunjabiPhonexAdapter;

    impl Language for Punjabi {
        fn code(&self) -> &'static str {
            "pa"
        }

        fn name(&self) -> &'static str {
            "Punjabi"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            LightPunjabiStemmer.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(PunjabiTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&PUNJABI_PHONEX)
        }
    }

    /// The singleton [`Punjabi`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Punjabi`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other
    /// `stringcheese-<lang>` pack follows.
    pub const PUNJABI: Punjabi = Punjabi;
}

#[cfg(feature = "alloc")]
pub use pack::{PUNJABI, Punjabi};

// Register into `stringcheese-lang::registry` so callers who look up
// languages dynamically (`registry::language("pa")`) find this pack.
// alloc-gated because the pack constant itself is alloc-gated.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(PUNJABI);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-pa` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
