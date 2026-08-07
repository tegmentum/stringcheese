//! Serbian language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Serbian`] value that carries the dual-script Serbian
//! stopword tables, the [`SerbianSnowball`] stemmer, the whitespace-
//! and-punctuation [`SerbianTokenizer`], and a [`SerbianLatin`]
//! Cyrillic-to-Latin phonetic hookup. Callers grab the singleton
//! [`SERBIAN`] `const` — no construction ceremony required — and
//! delegate through the [`Language`](stringcheese_lang::Language)
//! trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-sr` — language packs are per-crate,
//! per-language dependencies. Callers who need Serbian add
//! `stringcheese-sr = "0.1"` to their `Cargo.toml` explicitly.
//!
//! # First dual-script pack
//!
//! Serbian is the first `stringcheese-<lang>` implementation for a
//! language written in **two scripts** — Vukovica (Cyrillic,
//! `а б в г д ђ е ж з и ј к л љ м н њ о п р с т ћ у ф х ц ч џ ш`) and
//! Gaj's Latin (`a b v g d đ e ž z i j k l lj m n nj o p r s t ć u f h
//! c č dž š`). Both scripts are equally official; both are equally
//! valid input to every pack function.
//!
//! ## The dual-script invariants
//!
//! * **Both scripts work in `is_stopword`.** The pack ships two
//!   parallel stopword lists ([`STOPWORDS_CYR`] and [`STOPWORDS_LAT`])
//!   and the [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//!   override dispatches on the script of the input.
//! * **Both scripts work in `stem`.** The stemmer normalizes Cyrillic
//!   input to Latin via [`scripts::to_latin`], runs a single Latin
//!   suffix table, and transliterates the stem back to Cyrillic if
//!   the input was Cyrillic — so Cyrillic in, Cyrillic out; Latin
//!   in, Latin out. See [`snowball`] for the algorithm and the
//!   rationale for option (a) (normalize-to-Latin) versus option (b)
//!   (dual suffix tables).
//! * **Both scripts work in `tokenize`.** Every letter of both
//!   alphabets satisfies [`char::is_alphanumeric`], so the default
//!   splitter handles both scripts (and mixed-script input) with no
//!   special-case machinery.
//! * **The phonetic key unifies the scripts.** The
//!   [`SerbianLatin`] encoder returns the lowercase Latin form of any
//!   input — Cyrillic gets transliterated, Latin gets lowercased —
//!   so a phonetic index built on the pack conflates records filed
//!   under either script under the same key. Adapter name:
//!   `"sr-latin"`. The pack also opts into the cross-Slavic
//!   [`SlavicMetaphone`](stringcheese_phonetic::SlavicMetaphone)
//!   Metaphone-family sound-alike encoder from `stringcheese-phonetic`
//!   behind the `slavic-metaphone` Cargo feature — grab
//!   [`SERBIAN_WITH_SLAVIC_METAPHONE`] for a pack whose
//!   [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
//!   returns the shared Slavic-family sound-alike key instead of the
//!   default `SerbianLatin` transliteration.
//!
//! ## The bijective transliteration
//!
//! Serbian orthography is designed for a bijective script conversion:
//! every Cyrillic letter has a single Latin counterpart and vice
//! versa. The three Latin digraphs `lj`, `nj`, `dž` collapse to
//! single Cyrillic scalars `љ`, `њ`, `џ`; every other pair is
//! single-letter. See [`scripts`] for the round-trip helpers and the
//! "well-formed input" caveat (a handful of loanwords have letter-
//! boundary `n+j` sequences that collapse to the digraph on the
//! Cyrillic side, matching standard Serbian orthography).
//!
//! # Ekavian vs. ijekavian
//!
//! Serbian standard admits two pronunciations of the reflex of
//! Common Slavic `*ě` — ekavian `vek` / `век` versus ijekavian `vijek`
//! / `вијек`. This pack treats them **as distinct opaque forms** for
//! stemming and phonetic-key purposes: `vek` and `vijek` stem to
//! themselves and produce different keys. The stopword list carries
//! **both variants** where a stopword differs between the two
//! pronunciations (`gde` / `gdje`, `uvek` / `uvijek`). A follow-up
//! wave could add an explicit ijekavian → ekavian fold under a
//! separate adapter name.
//!
//! # Deferred to a follow-up wave
//!
//! * **Croatian, Bosnian, Montenegrin.** These share the dual-script
//!   base with Serbian but diverge in vocabulary (function words,
//!   morphology quirks) and orthographic preference (Croatian is
//!   Latin-only in practice; Montenegrin adds `с́` / `з́` to the
//!   alphabet). Each deserves its own `stringcheese-hr` / `-bs` /
//!   `-cnr` pack.
//! * **Byte-perfect Snowball parity.** The shipped stemmer is a
//!   compact Snowball-family light stemmer; the full `serbian.sbl`
//!   port and `voc.txt` / `output.txt` cross-verification is a
//!   follow-up.
//! * **Consonant alternations.** `k → č`, `g → ž`, `h → š`, the
//!   sibilant palatalization series — Serbian morphological
//!   alternations aren't modelled by the light stemmer.
//! * **CLDR-tailored collation.** The pack does not carry Serbian
//!   collation data; callers who need locale-aware sorting should
//!   reach for `icu_collator`.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_sr::SERBIAN;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(SERBIAN.code(), "sr");
//! assert_eq!(SERBIAN.name(), "Serbian");
//!
//! // Stopwords in both scripts.
//! assert!(SERBIAN.is_stopword("и"));      // Cyrillic
//! assert!(SERBIAN.is_stopword("i"));      // Latin
//! assert!(SERBIAN.is_stopword("НЕ"));     // Cyrillic case-fold
//! assert!(SERBIAN.is_stopword("Ne"));     // Latin case-fold
//!
//! // Stemming in both scripts.
//! assert_eq!(SERBIAN.stem("lepa"), "lep");
//! assert_eq!(SERBIAN.stem("лепа"), "леп");
//!
//! // Tokenization is script-agnostic.
//! let toks: Vec<&str> = SERBIAN
//!     .tokenize("Београд је главни град.")
//!     .collect();
//! assert_eq!(toks, ["Београд", "је", "главни", "град"]);
//! ```
//!
//! # Module map
//!
//! - [`scripts`] — the bijective [`to_latin`](scripts::to_latin) and
//!   [`to_cyrillic`](scripts::to_cyrillic) transliteration helpers.
//! - [`snowball`] — the [`SerbianSnowball`] stemmer.
//! - [`phonetic`] — [`SerbianLatin`] plus the
//!   [`SerbianLatinAdapter`] the
//!   [`Language`](stringcheese_lang::Language) trait hands back.
//! - [`stopwords`] — the [`STOPWORDS_CYR`] and [`STOPWORDS_LAT`]
//!   lists plus the [`STOPWORDS_ALL`] combined view.
//! - [`tokenizer`] — the [`SerbianTokenizer`] wrapper.
//! - The [`Serbian`] type and the [`SERBIAN`] constant live in this
//!   crate's root.

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
pub mod scripts;
#[cfg(feature = "alloc")]
pub mod snowball;
pub mod stopwords;
pub mod tokenizer;

#[cfg(all(test, feature = "std", not(target_family = "wasm")))]
mod properties;

#[cfg(all(feature = "alloc", feature = "slavic-metaphone"))]
pub use phonetic::SlavicMetaphoneAdapter;
#[cfg(feature = "alloc")]
pub use phonetic::{SerbianLatin, SerbianLatinAdapter};
#[cfg(feature = "alloc")]
pub use snowball::SerbianSnowball;
pub use stopwords::{STOPWORDS_ALL, STOPWORDS_CYR, STOPWORDS_LAT};
pub use tokenizer::SerbianTokenizer;

// -----------------------------------------------------------------------
// The Serbian language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;
    use alloc::string::String;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::SerbianLatinAdapter;
    #[cfg(feature = "slavic-metaphone")]
    use crate::phonetic::SlavicMetaphoneAdapter;
    use crate::scripts::contains_cyrillic;
    use crate::snowball::SerbianSnowball;
    use crate::stopwords::{STOPWORDS_ALL, STOPWORDS_CYR, STOPWORDS_LAT};
    use crate::tokenizer::SerbianTokenizer;

    /// Which phonetic encoder this [`Serbian`] instance uses.
    ///
    /// Modeled as an `enum` (rather than a `&'static dyn ...` reference)
    /// so the pack stays `Copy`, its constant constructors stay
    /// `const`-usable, and the shipped encoders stay a closed set. The
    /// [`SlavicMetaphone`](Self::SlavicMetaphone) variant is only
    /// available when the crate's `slavic-metaphone` Cargo feature is
    /// on; a build without that feature carries only the
    /// [`SerbianLatin`](Self::SerbianLatin) variant.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub enum SerbianPhoneticChoice {
        /// The default Cyrillic → Latin `SerbianLatin`
        /// transliteration adapter
        /// ([`SerbianLatinAdapter`](crate::phonetic::SerbianLatinAdapter)).
        #[default]
        SerbianLatin,
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

    /// The Serbian language pack.
    ///
    /// Carries a phonetic-encoder choice — the default (used by the
    /// [`SERBIAN`](crate::SERBIAN) constant) is the `SerbianLatin`
    /// Cyrillic-to-Latin transliteration; callers who want the
    /// cross-Slavic
    /// [`SlavicMetaphone`](stringcheese_phonetic::SlavicMetaphone)
    /// sound-alike encoder grab
    /// [`SERBIAN_WITH_SLAVIC_METAPHONE`](crate::SERBIAN_WITH_SLAVIC_METAPHONE)
    /// (behind the `slavic-metaphone` feature) or compose their own
    /// via [`with_slavic_metaphone_encoder`](Serbian::with_slavic_metaphone_encoder).
    ///
    /// Two `Serbian` values with different encoder choices are
    /// otherwise identical — same stopwords, same stemmer, same
    /// tokenizer, same code/name, same dual-script commitments. The
    /// distinguishing piece is cheap (a small enum), so `Serbian`
    /// remains cheap to copy and cheap to construct.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the dual-script commitments.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Serbian {
        phonetic_choice: SerbianPhoneticChoice,
    }

    impl Serbian {
        /// Construct a `Serbian` pack with the default `SerbianLatin`
        /// Cyrillic-to-Latin transliteration phonetic encoder.
        #[must_use]
        pub const fn new() -> Self {
            Self {
                phonetic_choice: SerbianPhoneticChoice::SerbianLatin,
            }
        }

        /// Return a `Serbian` pack whose
        /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
        /// hands back the default `SerbianLatin` Cyrillic-to-Latin
        /// transliteration adapter
        /// ([`SerbianLatinAdapter`](crate::phonetic::SerbianLatinAdapter)).
        ///
        /// This is the default; the method exists so a caller who
        /// composed a
        /// [`with_slavic_metaphone_encoder`](Self::with_slavic_metaphone_encoder)
        /// pack can undo that choice.
        #[must_use]
        pub const fn with_default_encoder(mut self) -> Self {
            self.phonetic_choice = SerbianPhoneticChoice::SerbianLatin;
            self
        }

        /// Return a `Serbian` pack whose
        /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
        /// hands back the cross-Slavic
        /// [`SlavicMetaphone`](stringcheese_phonetic::SlavicMetaphone)
        /// Metaphone-family sound-alike encoder from
        /// `stringcheese-phonetic`, wrapped as
        /// [`SlavicMetaphoneAdapter`](crate::phonetic::SlavicMetaphoneAdapter).
        ///
        /// Reach for the [`SERBIAN_WITH_SLAVIC_METAPHONE`](crate::SERBIAN_WITH_SLAVIC_METAPHONE)
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
            self.phonetic_choice = SerbianPhoneticChoice::SlavicMetaphone;
            self
        }
    }

    /// The static [`SerbianLatinAdapter`] [`Serbian`] hands back from
    /// [`phonetic_encoder`](Language::phonetic_encoder) when the pack
    /// was built with the default choice.
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static SR_LATIN: SerbianLatinAdapter = SerbianLatinAdapter;

    /// The static [`SlavicMetaphoneAdapter`] [`Serbian`] hands back
    /// from [`phonetic_encoder`](Language::phonetic_encoder) when the
    /// pack was built with
    /// [`SerbianPhoneticChoice::SlavicMetaphone`].
    ///
    /// Kept as a `static` for the same reason as `SR_LATIN`.
    #[cfg(feature = "slavic-metaphone")]
    static SLAVIC_METAPHONE: SlavicMetaphoneAdapter = SlavicMetaphoneAdapter;

    /// Lowercase `word` under default Unicode rules.
    fn lowercase(word: &str) -> String {
        word.chars().flat_map(char::to_lowercase).collect()
    }

    impl Language for Serbian {
        fn code(&self) -> &'static str {
            "sr"
        }

        fn name(&self) -> &'static str {
            "Serbian"
        }

        /// The combined stopword slice (both scripts).
        ///
        /// Callers that inspect the raw stopword table see every
        /// form. The [`is_stopword`](Language::is_stopword) override
        /// dispatches on the input's script and consults the
        /// per-script list directly, so a lookup does not scan the
        /// whole combined slice.
        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS_ALL
        }

        /// Dual-script stopword membership.
        ///
        /// Lowercases the input under default Unicode rules, then
        /// scans the Cyrillic-script list if the input contains any
        /// Cyrillic-block scalar, and the Latin-script list
        /// otherwise. A caller who passes a Latin-script word gets
        /// it looked up against the Latin list, and vice versa —
        /// which is exactly the dual-script contract the pack
        /// promises.
        fn is_stopword(&self, word: &str) -> bool {
            let normalized = lowercase(word);
            let list = if contains_cyrillic(&normalized) {
                STOPWORDS_CYR
            } else {
                STOPWORDS_LAT
            };
            list.contains(&normalized.as_str())
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            SerbianSnowball.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(SerbianTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            match self.phonetic_choice {
                SerbianPhoneticChoice::SerbianLatin => Some(&SR_LATIN),
                #[cfg(feature = "slavic-metaphone")]
                SerbianPhoneticChoice::SlavicMetaphone => Some(&SLAVIC_METAPHONE),
            }
        }
    }

    /// The singleton [`Serbian`] language pack (default `SerbianLatin`
    /// Cyrillic-to-Latin transliteration phonetic encoder).
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Serbian`] every time — the two forms are equivalent, but the
    /// constant is the intended entry point and matches the pattern
    /// every other `stringcheese-<lang>` pack follows.
    ///
    /// See [`SERBIAN_WITH_SLAVIC_METAPHONE`](crate::SERBIAN_WITH_SLAVIC_METAPHONE)
    /// for the same pack backed by the cross-Slavic
    /// [`SlavicMetaphone`](stringcheese_phonetic::SlavicMetaphone)
    /// encoder.
    pub const SERBIAN: Serbian = Serbian::new();

    /// The singleton [`Serbian`] language pack whose
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// hands back the cross-Slavic
    /// [`SlavicMetaphone`](stringcheese_phonetic::SlavicMetaphone)
    /// Metaphone-family sound-alike encoder instead of the default
    /// `SerbianLatin` Cyrillic-to-Latin transliteration.
    ///
    /// Identical to [`SERBIAN`](crate::SERBIAN) in every respect
    /// except its phonetic encoder. Only available when the crate's
    /// `slavic-metaphone` Cargo feature is enabled — see the
    /// [`slavic_metaphone`](mod@stringcheese_phonetic::slavic_metaphone)
    /// module docs for the design trade-offs.
    #[cfg(feature = "slavic-metaphone")]
    pub const SERBIAN_WITH_SLAVIC_METAPHONE: Serbian =
        Serbian::new().with_slavic_metaphone_encoder();
}

#[cfg(all(feature = "alloc", feature = "slavic-metaphone"))]
pub use pack::SERBIAN_WITH_SLAVIC_METAPHONE;
#[cfg(feature = "alloc")]
pub use pack::{SERBIAN, Serbian, SerbianPhoneticChoice};

// Register into `stringcheese-lang::registry` so callers who look up
// languages dynamically (`registry::language("sr")`) find this pack.
// alloc-gated because the pack constant itself is alloc-gated.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(SERBIAN);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-sr` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
