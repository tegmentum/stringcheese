//! Georgian language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Georgian`] value that carries the Georgian stopword
//! list, the [`GeorgianStemmer`] longest-match suffix stripper, the
//! whitespace-and-punctuation [`GeorgianTokenizer`], and a
//! [`GeorgianPhonex`] PHONEX-Georgian phonetic hookup computed over
//! an ISO 9984 Georgian -> Latin transliteration. Callers grab the
//! singleton [`GEORGIAN`] `const` — no construction ceremony required
//! — and delegate through the
//! [`Language`](stringcheese_lang::Language) trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-ka` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Georgian stopword table
//! or the Georgian stemmer's code. Callers who need Georgian add
//! `stringcheese-ka = "0.1"` to their `Cargo.toml` explicitly.
//!
//! # First Kartvelian pack, first Georgian-script pack
//!
//! Georgian is a **Kartvelian** language — the first Kartvelian pack
//! in StringCheese. Kartvelian is a small language family (Georgian,
//! Mingrelian, Laz, Svan) native to the Caucasus, **unrelated** to
//! every language family the workspace has covered so far (Indo-
//! European, Uralic, Turkic, Semitic, Sino-Tibetan, Japonic, Koreanic,
//! Austronesian, Austroasiatic). It is also the first Georgian-script
//! pack, ordinarily written in **Mkhedruli** (U+10D0..=U+10FF), a
//! script that:
//!
//! * Encodes every scalar as **3 UTF-8 bytes** (the Mkhedruli block
//!   U+10D0..=U+10FF and the Mtavruli block U+1C90..=U+1CBF both fall
//!   in UTF-8's 3-byte range U+0800..=U+FFFF). All tokenizer /
//!   stemmer / phonex arithmetic runs on `Vec<char>` or
//!   [`str::chars`] iteration — never raw byte offsets — because byte
//!   arithmetic would silently corrupt scalar boundaries.
//! * Is **unicase** in normal Modern-Georgian usage. Historically
//!   Mkhedruli was the sole modern script; Mtavruli (Unicode 11, 2018)
//!   is a capitalized style occasionally used for headings and
//!   emphasis. Every modern Mkhedruli scalar (U+10D0..=U+10FF) is
//!   paired with its Mtavruli counterpart (U+1C90..=U+1CBF, offset
//!   `+0x0BC0`), so Rust's default [`char::to_lowercase`] folds
//!   Mtavruli input to Mkhedruli. Old Georgian used the separate
//!   Asomtavruli (U+10A0..=U+10CF, "capital") and Nuskhuri
//!   (U+2D00..=U+2D2F, "lowercase") scripts; those are handled at
//!   the phonex level (the mapping recognizes their Mkhedruli
//!   equivalents) and pass through the tokenizer as alphabetic
//!   scalars.
//! * Uses ASCII whitespace and ASCII punctuation between orthographic
//!   words (like Greek and Latin, unlike Chinese / Japanese / Thai);
//!   Georgian also has a paragraph separator `჻` U+10FB that Unicode
//!   classifies as punctuation and the default splitter honours.
//!
//! # Georgian morphology
//!
//! Georgian is **agglutinative-fusional** with a rich morphological
//! surface:
//!
//! * **Seven nominal cases:** nominative `-ი`, dative-accusative
//!   `-ს`, ergative `-მა`, genitive `-ის`, instrumental `-ით`,
//!   adverbial `-ად`, and vocative (unmarked). The ergative appears
//!   only on the *subject of a transitive verb in the aorist* — one
//!   of Kartvelian's diagnostic features.
//! * **Plural markers:** contemporary `-ები`, archaic `-ნი` and
//!   `-თა` (still used in formal / literary writing).
//! * **Agglutinated postpositions:** `-ში` "in", `-ზე` "on", `-თან`
//!   "at", `-გან` "from", `-კენ` "toward", `-თვის` "for", stacked
//!   *after* the case marker (e.g. `წიგნებში` "in the books" is
//!   `წიგნ` + plural `-ებ` + postposition `-ში`).
//! * **Polypersonal verbs.** A finite verb agrees with *both*
//!   subject and object; person is marked by a fixed set of
//!   subject / object prefixes (`ვ-` 1sg subject, `მ-` 1sg object,
//!   `გ-` 2sg object, etc.). Tense / aspect / mood — the "screeve"
//!   system — combine as suffixes and stem alternations. See
//!   [`crate::stemmer`] for the subset the shipped stemmer handles.
//!
//! # Design choices
//!
//! * **Longest-match suffix stemmer.** A curated suffix table
//!   covering all seven case endings, the contemporary and archaic
//!   plural markers, the five common agglutinated postpositions, the
//!   plural + case / postposition compounds (`-ებით`, `-ებმა`,
//!   `-ების`, `-ებში`, `-ებზე`, `-ებთან`, `-ებგან`, `-ებკენ`), and
//!   the highest-frequency verb personal / tense endings (`-ვდი`,
//!   `-ავს`, `-იან`, `-ობდი`, `-ებდი`). Longest-match wins so
//!   `-ისთვის` (6 chars) beats bare `-ის` (2 chars). Every strip is
//!   guarded by a **2-scalar minimum stem length**. See
//!   [`crate::stemmer`].
//! * **~65-entry stopword list.** Personal / demonstrative /
//!   interrogative pronouns, the high-frequency forms of the copula
//!   `არის`, conjunctions, negators / affirmatives, quantifiers, and
//!   common adverbs. Stored in Mkhedruli lowercase. See
//!   [`stopwords`].
//! * **PHONEX-Georgian phonetic hookup.** A 4-character Soundex-shape
//!   key computed over an **ISO 9984** Georgian -> Latin
//!   transliteration. The six ejective consonants (`კ`, `პ`, `ტ`,
//!   `წ`, `ჭ`, `ყ`) transliterate with an ISO 9984 apostrophe
//!   (`k'`, `p'`, `t'`, `ts'`, `ch'`, `q'`) that the phonex step
//!   drops — so ejective / aspirate pairs (`კ`/`ქ`, `პ`/`ფ`,
//!   `ტ`/`თ`, `წ`/`ც`, `ჭ`/`ჩ`) fold to the same key by design.
//!   Adapter name: `"phonex-ka"`. See [`crate::phonetic`].
//! * **Thin tokenizer.** A transparent wrapper around
//!   [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer);
//!   Mkhedruli and Mtavruli scalars are alphabetic under Unicode's
//!   classification and stay word-internal naturally. See
//!   [`crate::tokenizer`].
//! * **Georgian-aware case fold.** Uses Rust's default
//!   [`char::to_lowercase`] to fold Mtavruli (U+1C90..=U+1CBF) to
//!   Mkhedruli (U+10D0..=U+10FF). The `is_stopword` override applies
//!   the fold before comparison so Mtavruli-cased queries match the
//!   Mkhedruli stopword list.
//! * **Default Unicode collation.** Georgian sorts under CLDR's
//!   Georgian tailoring. This pack does not carry the CLDR tailoring
//!   data; [`Language::collator`](stringcheese_lang::Language::collator)
//!   returns `None`. Callers who need locale-aware Georgian collation
//!   should reach for `icu_collator`.
//!
//! # Deferred to a follow-up wave
//!
//! * **Old Georgian (`stringcheese-oka`).** Old Georgian used the
//!   Asomtavruli (uppercase, U+10A0..=U+10CF) and Nuskhuri (lowercase,
//!   U+2D00..=U+2D2F) scripts, richer inflection, and a partly
//!   different vocabulary (the vocative `-ო`, additional aorist and
//!   optative forms, distinct pronoun paradigms). A dedicated
//!   `stringcheese-oka` sibling would ship the two-script normalizer
//!   and an Old-Georgian-aware suffix cascade.
//! * **Mingrelian / Laz / Svan siblings.** The other Kartvelian
//!   languages (Mingrelian, Laz, Svan) share phonology and much
//!   morphology with Georgian but are distinct languages with their
//!   own inventories and case systems. Their packs would live as
//!   `stringcheese-xmf` (Mingrelian), `stringcheese-lzz` (Laz), and
//!   `stringcheese-sva` (Svan) once demand arises.
//! * **Verb-preverb stripping.** Georgian verbs take direction /
//!   aspect preverbs (`მო-`, `წა-`, `გა-`, `და-`, `მი-`, `ა-`, etc.)
//!   which shift the meaning of the underlying root. Stripping them
//!   safely needs a lexicon; the shipped stemmer only handles
//!   suffixes.
//! * **Screeve / tense-alternation reversal.** Different tenses of
//!   the same verb take different vowel patterns in the stem
//!   (present `ვწერ` vs. aorist `დავწერე`); a real lemmatizer would
//!   fold these. Out of scope for a suffix-stripper.
//! * **BGN/PCGN romanization adapter.** BGN/PCGN 1981 diverges from
//!   ISO 9984 on the ejective / aspirated distinction. Would want
//!   its own adapter for name-matching interop.
//! * **Georgian National transliteration (2002) adapter.** The
//!   Georgian government's 2002 romanization is close to ISO 9984
//!   but not identical. A dedicated adapter is deferred.
//! * **Full canonical Georgian stemmer parity.** No published
//!   Snowball Georgian algorithm exists; a real IR-grade stemmer
//!   would need corpus-driven suffix inventory refinement and
//!   context-sensitive over-strip guards.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_ka::GEORGIAN;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(GEORGIAN.code(), "ka");
//! assert_eq!(GEORGIAN.name(), "Georgian");
//! assert!(GEORGIAN.is_stopword("და"));
//! assert!(GEORGIAN.is_stopword("არის"));
//! assert!(!GEORGIAN.is_stopword("წიგნი"));
//!
//! // Longest-match suffix stripping.
//! assert_eq!(GEORGIAN.stem("წიგნები"), "წიგნ");
//! assert_eq!(GEORGIAN.stem("წიგნის"), "წიგნ");
//! assert_eq!(GEORGIAN.stem("სახლში"), "სახლ");
//!
//! let toks: Vec<&str> = GEORGIAN
//!     .tokenize("გამარჯობა, მსოფლიო! თბილისი — დედაქალაქი.")
//!     .collect();
//! assert_eq!(toks, ["გამარჯობა", "მსოფლიო", "თბილისი", "დედაქალაქი"]);
//! ```
//!
//! # Module map
//!
//! - [`stemmer`] — the [`GeorgianStemmer`] longest-match suffix
//!   stripper.
//! - [`phonetic`] — [`GeorgianPhonex`] plus the [`GeorgianPhonexAdapter`]
//!   the [`Language`](stringcheese_lang::Language) trait hands back.
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`tokenizer`] — the [`GeorgianTokenizer`] wrapper.
//! - The [`Georgian`] type and the [`GEORGIAN`] constant live in this
//!   crate's root.

#![cfg_attr(not(feature = "std"), no_std)]
// `deny(unsafe_code)` rather than `forbid(unsafe_code)` because the
// `stringcheese_lang::register_language!` macro expands to a linkme
// `#[distributed_slice]` static that emits `#[unsafe(link_section =
// "...")]` (Rust 2024 form) — `forbid` cannot be relaxed by inner
// attributes and would break the build. The macro emits an explicit
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
pub use phonetic::{GeorgianPhonex, GeorgianPhonexAdapter, mkhedruli_to_iso9984};
#[cfg(feature = "alloc")]
pub use stemmer::GeorgianStemmer;
pub use stopwords::STOPWORDS;
pub use tokenizer::GeorgianTokenizer;

// -----------------------------------------------------------------------
// The Georgian language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;
    use alloc::string::String;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::GeorgianPhonexAdapter;
    use crate::stemmer::GeorgianStemmer;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::GeorgianTokenizer;

    /// The Georgian language pack.
    ///
    /// Zero-sized; construct as [`Georgian`] and reuse the value freely
    /// across threads and calls, or grab the crate-level
    /// [`GEORGIAN`](crate::GEORGIAN) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation choices
    /// (longest-match suffix stemmer, ISO 9984 -> PHONEX phonetic key)
    /// and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Georgian;

    /// The static [`GeorgianPhonexAdapter`] [`Georgian`] hands back
    /// from [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static PHONEX_KA: GeorgianPhonexAdapter = GeorgianPhonexAdapter;

    /// Normalize a Georgian string for stopword comparison: fold
    /// Mtavruli (U+1C90..=U+1CBF) to Mkhedruli via Rust's default
    /// Unicode lowercase.
    ///
    /// Every entry in [`STOPWORDS`] is stored in Mkhedruli lowercase;
    /// a query like `"ᲓᲐ"` (Mtavruli "and") needs to fold to `"და"`
    /// before the scan can match.
    fn normalize_for_stopword(word: &str) -> String {
        word.chars().flat_map(char::to_lowercase).collect()
    }

    impl Language for Georgian {
        fn code(&self) -> &'static str {
            "ka"
        }

        fn name(&self) -> &'static str {
            "Georgian"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        /// Georgian-aware stopword membership.
        ///
        /// Overrides the default trait implementation (which uses
        /// [`str::eq_ignore_ascii_case`], missing every Mtavruli input)
        /// with a Unicode case-fold that maps Mtavruli to Mkhedruli
        /// under Unicode 11+.
        fn is_stopword(&self, word: &str) -> bool {
            let normalized = normalize_for_stopword(word);
            STOPWORDS.contains(&normalized.as_str())
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            GeorgianStemmer.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(GeorgianTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&PHONEX_KA)
        }
    }

    /// The singleton [`Georgian`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Georgian`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other `stringcheese-<lang>`
    /// pack follows.
    pub const GEORGIAN: Georgian = Georgian;
}

#[cfg(feature = "alloc")]
pub use pack::{GEORGIAN, Georgian};

// Register into `stringcheese-lang::registry` so callers who look up
// languages dynamically (`registry::language("ka")`) find this pack.
// alloc-gated because the pack constant itself is alloc-gated.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(GEORGIAN);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-ka` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
