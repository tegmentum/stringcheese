//! Ukrainian language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Ukrainian`] value that carries the Ukrainian
//! stopword list, the [`UkrainianSnowball`] light suffix-stripping
//! stemmer, the apostrophe-aware [`UkrainianTokenizer`], and a
//! [`UkrainianGost779B`] transliteration phonetic hookup. Callers grab
//! the singleton [`UKRAINIAN`] `const` — no construction ceremony
//! required — and delegate through the
//! [`Language`](stringcheese_lang::Language) trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-uk` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Ukrainian stopword
//! table or the light stemmer's code. Callers who need Ukrainian add
//! `stringcheese-uk = "0.1"` to their `Cargo.toml` explicitly.
//!
//! # Second Cyrillic-script pack
//!
//! This is the second `stringcheese-<lang>` implementation for a
//! script written in Cyrillic — Russian shipped in wave 7 and set the
//! shape (`Vec<char>` suffix arithmetic, `is_stopword` override with
//! Unicode case-fold, deterministic ASCII-only transliteration).
//! Ukrainian follows the same shape with three
//! Ukrainian-specific differences:
//!
//! ## Extended Cyrillic letter set — Ukrainian carries what Russian omits
//!
//! Ukrainian's alphabet extends the Cyrillic block with four letters
//! that Russian's modern inventory does not use:
//!
//! * **`ґ` (U+0491) / `Ґ` (U+0490)** — the voiced velar stop /ɡ/.
//!   Ukrainian distinguishes it from `г` (U+0433), a voiced glottal
//!   fricative /ɦ/. Russian collapses both into `г`, so Russian GOST
//!   7.79-B maps `г → g`; the Ukrainian pack instead maps `г → h` and
//!   `ґ → g`. This is the most consequential Ukrainian-vs-Russian
//!   divergence in the transliteration table.
//! * **`є` (U+0454) / `Є` (U+0404)** — the iotated /je/ vowel;
//!   Ukrainian's counterpart to Russian's `е` in some positions.
//!   Ukrainian carries a separate `е` (U+0435) that renders /e/
//!   without the /j/ onset — the two are distinct letters.
//! * **`і` (U+0456) / `І` (U+0406)** — the /i/ vowel. Ukrainian's
//!   counterpart to Russian's `и` (which Ukrainian also uses, but
//!   only for /ɪ/, a lax high-front vowel — not the /i/ of Russian
//!   `и`).
//! * **`ї` (U+0457) / `Ї` (U+0407)** — the iotated /ji/ vowel;
//!   Ukrainian-only.
//!
//! Ukrainian **does not** use these Russian letters:
//!
//! * **`ъ` (U+044A, hard sign)** — absent from Ukrainian; the
//!   equivalent role (marking a hard consonant before a following
//!   iotated vowel) is played by the **ASCII apostrophe `'`
//!   (U+0027)** in words like `сім'я` (family), `п'ять` (five). See
//!   [`tokenizer`] for how the apostrophe is preserved.
//! * **`ы` (U+044B)** — absent from Ukrainian. Ukrainian uses `и`
//!   for the /ɪ/ sound; the two are distinct letters that just look
//!   similar in some cursive scripts.
//! * **`ё` (U+0451)** — absent from Ukrainian. Ukrainian does not
//!   use the diaeresis-drop orthography Russian does; there is no
//!   `ё → е` fold in the stemmer or `is_stopword` implementation.
//! * **`э` (U+044D)** — absent from Ukrainian; Ukrainian's `е` plays
//!   this role.
//!
//! ## The Cyrillic-specific invariants (unchanged from Russian)
//!
//! * **Every letter is 2 bytes in UTF-8.** The Ukrainian alphabet
//!   sits inside U+0400..=U+045F plus `Ґ`/`ґ` at U+0490/U+0491, all
//!   of which fall in the UTF-8 2-byte range (U+0080..=U+07FF). A
//!   word like `"стіл"` (4 characters) is 8 bytes. All suffix and
//!   region arithmetic in [`snowball`] runs on `Vec<char>`, never
//!   raw byte offsets, so no scalar is ever sliced apart. Callers of
//!   [`Language::stem`](stringcheese_lang::Language::stem) and
//!   [`Language::tokenize`](stringcheese_lang::Language::tokenize) never
//!   see the char-vs-byte distinction because the returned
//!   [`Cow<str>`](alloc::borrow::Cow) and token slices are always
//!   valid UTF-8; downstream callers that do byte-level arithmetic on
//!   the outputs must remember the 2x expansion factor.
//! * **No Turkic-fold concerns.** Ukrainian case-folding is
//!   well-behaved under Rust's default [`char::to_lowercase`]:
//!   `А → а`, `Ґ → ґ`, `Є → є`, `І → і`, `Ї → ї`, `Я → я`. There is
//!   no locale tailoring the way Turkish requires for the dotted /
//!   dotless-I distinction.
//! * **UTF-8 code-point processing order is left-to-right.** No RTL
//!   concerns; the tokenizer emits tokens in reading order.
//!
//! # Design choices
//!
//! * **Light suffix-stripping stemmer.** Unlike Russian, French,
//!   German, Spanish, and every other language with a canonical
//!   Snowball algorithm, Ukrainian is **not covered by an official
//!   Snowball stemmer** — the Snowball repository has no
//!   `ukrainian.sbl`. This crate ships a **light suffix-stripping
//!   stemmer** with an explicit scope (strip common inflectional
//!   suffixes in a single longest-match pass, guarded by an RV floor)
//!   rather than a non-canonical port of the Russian algorithm.
//!   Rationale: a Russian port with the letter set adjusted would
//!   inherit Russian assumptions about perfective-gerund contexts,
//!   `нн` participle undoublement, and `ость` derivational endings
//!   that either do not apply to Ukrainian or apply differently. See
//!   [`snowball`] for the rules and reference-pair coverage.
//! * **~220-word stopword list.** Union of published Ukrainian
//!   collections (Solariz's `ukrainian-stopwords`, community Snowball
//!   Ukrainian lists, NLTK-adjacent Slavic function-word
//!   inventories). Covers pronouns, demonstratives, interrogatives,
//!   conjunctions, prepositions, particles, high-frequency forms of
//!   the copula *бути*, and quantifiers. See [`stopwords`].
//! * **GOST 7.79-B transliteration phonetic encoder, Ukrainian
//!   adaptation.** A deterministic, ASCII-only Cyrillic → Latin
//!   mapping tailored to the Ukrainian letter set. Adapter name:
//!   `"gost-7.79-b-uk"` — the `-uk` disambiguates it from the
//!   Russian `"gost-7.79-b"` (the two encoders differ on `г`, `ґ`,
//!   `х`, and `щ` and cover disjoint letter sets). See [`phonetic`]
//!   for the mapping table and the rationale behind the Ukrainian
//!   choices (`г → h`, `ґ → g`, `є → ye`, `ї → yi`, `и → y`,
//!   `х → kh`, `щ → shch`).
//! * **Apostrophe-aware tokenizer.** Ukrainian orthography uses the
//!   ASCII apostrophe **`'` (U+0027)** as a **word-internal**
//!   separator marking the boundary between a hard consonant and a
//!   following iotated vowel (`сім'я`, `п'ять`, `об'єкт`). The
//!   pack's [`UkrainianTokenizer`] promotes the apostrophe to a
//!   word-internal character when it sits between two alphanumeric
//!   scalars, while treating every other separator (whitespace,
//!   Unicode punctuation, hyphens, quotes) the same way as
//!   [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer).
//! * **Cyrillic-aware case fold.** Uses Rust's default
//!   [`char::to_lowercase`] — no Turkic-style tailoring, no `ё → е`
//!   precomputation (Ukrainian has no `ё`). The `is_stopword`
//!   override lowercases under this rule so uppercase Cyrillic
//!   queries match the plain stopword list.
//! * **Default Unicode collation.**
//!   [`Language::collator`](stringcheese_lang::Language::collator)
//!   returns `None`. Callers who need locale-aware Ukrainian
//!   collation should reach for `icu_collator`.
//!
//! # Deferred to a follow-up wave
//!
//! * **Canonical Snowball parity.** There is no canonical Snowball
//!   Ukrainian; if the Snowball project ever publishes one, this
//!   crate will adopt it under a new module (`snowball_canonical`)
//!   and keep the light stemmer as `snowball_light` for callers who
//!   want the current behaviour.
//! * **Verb-aspect stripping.** Ukrainian's perfective/imperfective
//!   aspect prefixes (`про-`, `на-`, `з-`, …) are content-carrying
//!   and are not stripped by the light stemmer. A prefix-aware
//!   variant is a follow-up.
//! * **Belarusian, Serbian, Bulgarian, Macedonian.** Each deserves
//!   its own `stringcheese-<lang>` pack — different function-word
//!   inventories, different morphology, different subset of the
//!   extended Cyrillic block (Belarusian `ў`, Serbian `љ њ ђ ћ џ`,
//!   Macedonian `ѓ ќ ѕ`).
//! * **Slavic-Metaphone / PHONEX-Slavic phonetic encoder.** The
//!   shipped transliteration is a *transliteration* (deterministic
//!   character-level mapping), not a *sound-alike* encoder. A
//!   Slavic-tuned Metaphone that spans both Russian and Ukrainian
//!   would complement it for cross-Slavic record linkage.
//! * **ISO 9 System A transliteration alongside GOST 7.79-B.** Would
//!   want it under a separate adapter for library-catalog interop.
//! * **Ukrainian government 2010 transliteration.** The official
//!   standard for passport rendering (`Пилипенко → Pylypenko`); it
//!   differs from GOST 7.79-B in the treatment of the soft sign
//!   (dropped vs `'`), `й` (dropped word-finally after `и`), and the
//!   digraph conventions. Would be a second adapter.
//! * **Typographic apostrophe (U+2019) recognition.** The tokenizer
//!   currently promotes only the ASCII apostrophe (U+0027).
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_uk::UKRAINIAN;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(UKRAINIAN.code(), "uk");
//! assert_eq!(UKRAINIAN.name(), "Ukrainian");
//! assert!(UKRAINIAN.is_stopword("і"));
//! assert!(UKRAINIAN.is_stopword("НЕ"));   // Cyrillic case-fold: НЕ → не.
//! assert!(!UKRAINIAN.is_stopword("книга"));
//! assert_eq!(UKRAINIAN.stem("красивий"), "красив");
//! assert_eq!(UKRAINIAN.stem("столи"), "стол");
//!
//! let toks: Vec<&str> = UKRAINIAN
//!     .tokenize("Привіт, сім'я! Київ — столиця.")
//!     .collect();
//! assert_eq!(toks, ["Привіт", "сім'я", "Київ", "столиця"]);
//! ```
//!
//! # Module map
//!
//! - [`snowball`] — the [`UkrainianSnowball`] light stemmer.
//! - [`phonetic`] — [`UkrainianGost779B`] plus the
//!   [`UkrainianGost779BAdapter`] the
//!   [`Language`](stringcheese_lang::Language) trait hands back.
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`tokenizer`] — the [`UkrainianTokenizer`] with apostrophe
//!   preservation.
//! - The [`Ukrainian`] type and the [`UKRAINIAN`] constant live in
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
pub mod snowball;
pub mod stopwords;
pub mod tokenizer;

#[cfg(all(test, feature = "std", not(target_family = "wasm")))]
mod properties;

#[cfg(feature = "alloc")]
pub use phonetic::{UkrainianGost779B, UkrainianGost779BAdapter};
#[cfg(feature = "alloc")]
pub use snowball::UkrainianSnowball;
pub use stopwords::STOPWORDS;
pub use tokenizer::UkrainianTokenizer;

// -----------------------------------------------------------------------
// The Ukrainian language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;
    use alloc::string::String;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::UkrainianGost779BAdapter;
    use crate::snowball::UkrainianSnowball;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::UkrainianTokenizer;

    /// The Ukrainian language pack.
    ///
    /// Zero-sized; construct as [`Ukrainian`] and reuse the value
    /// freely across threads and calls, or grab the crate-level
    /// [`UKRAINIAN`](crate::UKRAINIAN) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Ukrainian;

    /// The static [`UkrainianGost779BAdapter`] [`Ukrainian`] hands
    /// back from [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static GOST_779_B_UK: UkrainianGost779BAdapter = UkrainianGost779BAdapter;

    /// Normalize a Cyrillic string for stopword comparison: lowercase
    /// under default Unicode rules. Unlike the Russian pack, no
    /// `ё → е` fold is applied — Ukrainian does not use `ё`.
    fn normalize_for_stopword(word: &str) -> String {
        let mut out = String::with_capacity(word.len());
        for c in word.chars() {
            for lc in c.to_lowercase() {
                out.push(lc);
            }
        }
        out
    }

    impl Language for Ukrainian {
        fn code(&self) -> &'static str {
            "uk"
        }

        fn name(&self) -> &'static str {
            "Ukrainian"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        /// Cyrillic-aware stopword membership.
        ///
        /// Overrides the default trait implementation (which uses
        /// [`str::eq_ignore_ascii_case`], missing every uppercase
        /// Cyrillic input) with a Unicode lowercase pass — so `І` and
        /// `і` both find `і` in the stopword list.
        fn is_stopword(&self, word: &str) -> bool {
            let normalized = normalize_for_stopword(word);
            STOPWORDS.contains(&normalized.as_str())
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            UkrainianSnowball.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(UkrainianTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&GOST_779_B_UK)
        }
    }

    /// The singleton [`Ukrainian`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Ukrainian`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other `stringcheese-<lang>`
    /// pack follows.
    pub const UKRAINIAN: Ukrainian = Ukrainian;
}

#[cfg(feature = "alloc")]
pub use pack::{UKRAINIAN, Ukrainian};

// Register into `stringcheese-lang::registry` so callers who look up
// languages dynamically (`registry::language("uk")`) find this pack.
// alloc-gated because the pack constant itself is alloc-gated.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(UKRAINIAN);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-uk` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
