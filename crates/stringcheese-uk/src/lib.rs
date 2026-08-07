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
//!   `х → kh`, `щ → shch`). The pack also opts into the cross-Slavic
//!   [`SlavicMetaphone`](stringcheese_phonetic::SlavicMetaphone)
//!   Metaphone-family sound-alike encoder from `stringcheese-phonetic`
//!   behind the `slavic-metaphone` Cargo feature — grab
//!   [`UKRAINIAN_WITH_SLAVIC_METAPHONE`] for a pack whose
//!   [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
//!   returns the shared Slavic-family sound-alike key instead of the
//!   default transliteration.
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
//! * **PHONEX-Slavic phonetic encoder.** The shipped transliteration
//!   is a *transliteration* (deterministic character-level mapping),
//!   not a sound-alike encoder. The `slavic-metaphone` feature adds
//!   the cross-Slavic
//!   [`SlavicMetaphone`](stringcheese_phonetic::SlavicMetaphone)
//!   Metaphone-family encoder as an alternate; a Ukrainian-specific
//!   PHONEX-Ukrainian variant is a further follow-up.
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

#[cfg(all(feature = "alloc", feature = "slavic-metaphone"))]
pub use phonetic::SlavicMetaphoneAdapter;
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

    #[cfg(feature = "slavic-metaphone")]
    use crate::phonetic::SlavicMetaphoneAdapter;
    use crate::phonetic::UkrainianGost779BAdapter;
    use crate::snowball::UkrainianSnowball;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::UkrainianTokenizer;

    /// Which phonetic encoder this [`Ukrainian`] instance uses.
    ///
    /// Modeled as an `enum` (rather than a `&'static dyn ...` reference)
    /// so the pack stays `Copy`, its constant constructors stay
    /// `const`-usable, and the shipped encoders stay a closed set. The
    /// [`SlavicMetaphone`](Self::SlavicMetaphone) variant is only
    /// available when the crate's `slavic-metaphone` Cargo feature is
    /// on; a build without that feature carries only the
    /// [`Gost779BUk`](Self::Gost779BUk) variant.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub enum UkrainianPhoneticChoice {
        /// The default Ukrainian GOST 7.79 System B Cyrillic → Latin
        /// transliteration adapter
        /// ([`UkrainianGost779BAdapter`](crate::phonetic::UkrainianGost779BAdapter)).
        #[default]
        Gost779BUk,
        /// The cross-Slavic
        /// [`SlavicMetaphone`](stringcheese_phonetic::SlavicMetaphone)
        /// Metaphone-family sound-alike encoder from
        /// `stringcheese-phonetic`, wrapped as
        /// [`SlavicMetaphoneAdapter`](crate::phonetic::SlavicMetaphoneAdapter).
        /// Only available when the crate's `slavic-metaphone` Cargo
        /// feature is on.
        #[cfg(feature = "slavic-metaphone")]
        SlavicMetaphone,
    }

    /// The Ukrainian language pack.
    ///
    /// Carries a phonetic-encoder choice — the default (used by the
    /// [`UKRAINIAN`](crate::UKRAINIAN) constant) is the Ukrainian
    /// GOST 7.79-B transliteration; callers who want the cross-Slavic
    /// [`SlavicMetaphone`](stringcheese_phonetic::SlavicMetaphone)
    /// sound-alike encoder grab
    /// [`UKRAINIAN_WITH_SLAVIC_METAPHONE`](crate::UKRAINIAN_WITH_SLAVIC_METAPHONE)
    /// (behind the `slavic-metaphone` feature) or compose their own
    /// via [`with_slavic_metaphone_encoder`](Ukrainian::with_slavic_metaphone_encoder).
    ///
    /// Two `Ukrainian` values with different encoder choices are
    /// otherwise identical — same stopwords, same stemmer, same
    /// tokenizer, same code/name. The distinguishing piece is cheap
    /// (a small enum), so `Ukrainian` remains cheap to copy and cheap
    /// to construct.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Ukrainian {
        phonetic_choice: UkrainianPhoneticChoice,
    }

    impl Ukrainian {
        /// Construct a `Ukrainian` pack with the default Ukrainian
        /// GOST 7.79-B transliteration phonetic encoder.
        #[must_use]
        pub const fn new() -> Self {
            Self {
                phonetic_choice: UkrainianPhoneticChoice::Gost779BUk,
            }
        }

        /// Return a `Ukrainian` pack whose
        /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
        /// hands back the default Ukrainian GOST 7.79-B
        /// transliteration adapter
        /// ([`UkrainianGost779BAdapter`](crate::phonetic::UkrainianGost779BAdapter)).
        ///
        /// This is the default; the method exists so a caller who
        /// composed a
        /// [`with_slavic_metaphone_encoder`](Self::with_slavic_metaphone_encoder)
        /// pack can undo that choice.
        #[must_use]
        pub const fn with_default_encoder(mut self) -> Self {
            self.phonetic_choice = UkrainianPhoneticChoice::Gost779BUk;
            self
        }

        /// Return a `Ukrainian` pack whose
        /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
        /// hands back the cross-Slavic
        /// [`SlavicMetaphone`](stringcheese_phonetic::SlavicMetaphone)
        /// Metaphone-family sound-alike encoder from
        /// `stringcheese-phonetic`, wrapped as
        /// [`SlavicMetaphoneAdapter`](crate::phonetic::SlavicMetaphoneAdapter).
        ///
        /// Reach for the [`UKRAINIAN_WITH_SLAVIC_METAPHONE`](crate::UKRAINIAN_WITH_SLAVIC_METAPHONE)
        /// constant if you want the default pack with the
        /// Slavic-Metaphone encoder; this builder method lets a caller
        /// compose the encoder choice with future non-default pack
        /// pieces.
        ///
        /// Only available when the crate's `slavic-metaphone` Cargo
        /// feature is enabled.
        #[cfg(feature = "slavic-metaphone")]
        #[must_use]
        pub const fn with_slavic_metaphone_encoder(mut self) -> Self {
            self.phonetic_choice = UkrainianPhoneticChoice::SlavicMetaphone;
            self
        }
    }

    /// The static [`UkrainianGost779BAdapter`] [`Ukrainian`] hands
    /// back from [`phonetic_encoder`](Language::phonetic_encoder) when
    /// the pack was built with the default choice.
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static GOST_779_B_UK: UkrainianGost779BAdapter = UkrainianGost779BAdapter;

    /// The static [`SlavicMetaphoneAdapter`] [`Ukrainian`] hands back
    /// from [`phonetic_encoder`](Language::phonetic_encoder) when the
    /// pack was built with
    /// [`UkrainianPhoneticChoice::SlavicMetaphone`].
    ///
    /// Kept as a `static` for the same reason as `GOST_779_B_UK`.
    #[cfg(feature = "slavic-metaphone")]
    static SLAVIC_METAPHONE: SlavicMetaphoneAdapter = SlavicMetaphoneAdapter;

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
            match self.phonetic_choice {
                UkrainianPhoneticChoice::Gost779BUk => Some(&GOST_779_B_UK),
                #[cfg(feature = "slavic-metaphone")]
                UkrainianPhoneticChoice::SlavicMetaphone => Some(&SLAVIC_METAPHONE),
            }
        }
    }

    /// The singleton [`Ukrainian`] language pack (default Ukrainian
    /// GOST 7.79-B transliteration phonetic encoder).
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Ukrainian`] every time — the two forms are equivalent, but
    /// the constant is the intended entry point and matches the
    /// pattern every other `stringcheese-<lang>` pack follows.
    ///
    /// See [`UKRAINIAN_WITH_SLAVIC_METAPHONE`](crate::UKRAINIAN_WITH_SLAVIC_METAPHONE)
    /// for the same pack backed by the cross-Slavic
    /// [`SlavicMetaphone`](stringcheese_phonetic::SlavicMetaphone)
    /// encoder.
    pub const UKRAINIAN: Ukrainian = Ukrainian::new();

    /// The singleton [`Ukrainian`] language pack whose
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// hands back the cross-Slavic
    /// [`SlavicMetaphone`](stringcheese_phonetic::SlavicMetaphone)
    /// Metaphone-family sound-alike encoder instead of the default
    /// Ukrainian GOST 7.79-B transliteration.
    ///
    /// Identical to [`UKRAINIAN`](crate::UKRAINIAN) in every respect
    /// except its phonetic encoder. Only available when the crate's
    /// `slavic-metaphone` Cargo feature is enabled — see the
    /// [`slavic_metaphone`](mod@stringcheese_phonetic::slavic_metaphone)
    /// module docs for the design trade-offs.
    #[cfg(feature = "slavic-metaphone")]
    pub const UKRAINIAN_WITH_SLAVIC_METAPHONE: Ukrainian =
        Ukrainian::new().with_slavic_metaphone_encoder();
}

#[cfg(all(feature = "alloc", feature = "slavic-metaphone"))]
pub use pack::UKRAINIAN_WITH_SLAVIC_METAPHONE;
#[cfg(feature = "alloc")]
pub use pack::{UKRAINIAN, Ukrainian, UkrainianPhoneticChoice};

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
