//! Indonesian (Bahasa Indonesia) language pack for the StringCheese
//! toolkit.
//!
//! A zero-sized [`Indonesian`] value that carries the Indonesian
//! stopword list, the [`IndonesianStemmer`] (a simplified
//! Nazief-Adriani confix stripper), the whitespace-and-punctuation
//! [`IndonesianTokenizer`], and an [`IndonesianPhonex`] phonetic
//! hookup. Callers grab the singleton [`INDONESIAN`] `const` — no
//! construction ceremony required — and delegate through the
//! [`Language`](stringcheese_lang::Language) trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-id` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Indonesian stopword
//! table or the Nazief-Adriani stemmer's code. Callers who need
//! Indonesian add `stringcheese-id = "0.1"` to their `Cargo.toml`
//! explicitly.
//!
//! # Indonesian — the first Malayo-Polynesian pack
//!
//! Indonesian (Bahasa Indonesia) belongs to the **Malayo-Polynesian**
//! branch of the **Austronesian** language family — no other pack in
//! this workspace shares that family. Every previously shipped pack
//! is Indo-European (English, German, French, Portuguese, Russian,
//! Hindi, Persian, …), Sino-Tibetan (Chinese), Japonic (Japanese),
//! Koreanic (Korean), Semitic (Arabic, Hebrew), Uralic (Finnish,
//! Estonian, Hungarian), Turkic (Turkish), or Austroasiatic
//! (Vietnamese). Indonesian's morphology reflects a Malayo-Polynesian
//! character:
//!
//! * **Rich derivational affixation, zero inflection.** Verbs don't
//!   conjugate for tense, aspect, mood, person, or number. Nouns
//!   don't decline for case, number, or gender. Adjectives don't
//!   agree. Every grammatical relation is expressed by word order or
//!   function words (`akan` future, `sudah` perfective, `sedang`
//!   progressive, `para` / `banyak` plurality, `dia` / `mereka`
//!   person). The morphological chain that *does* exist is entirely
//!   derivational — prefixes, suffixes, and circumfixes that change
//!   part-of-speech or add nuance.
//! * **Nasal-assimilation on `me-` and `pe-`.** Two productive
//!   prefixes (`me-` verbal, `pe-` agent nominalizer) assimilate to
//!   the initial consonant of the root and often elide it:
//!   `me-` + `pilih` "choose" → `memilih` (`p` elided);
//!   `me-` + `tulis` "write" → `menulis` (`t` elided);
//!   `me-` + `kirim` "send" → `mengirim` (`k` elided);
//!   `me-` + `sapu` "sweep" → `menyapu` (`s` elided).
//!   Reversing this assimilation in the stemmer requires a
//!   consonant-restoration table.
//! * **ASCII-only orthography with four native digraphs.** The
//!   modern Ejaan Yang Disempurnakan spells Indonesian in the
//!   26-letter Latin alphabet with **no diacritics** — the only
//!   compositional units above the letter are the digraphs `ny` /ɲ/,
//!   `ng` /ŋ/, `sy` /ʃ/, `kh` /x/. This makes tokenization trivial
//!   (whitespace-and-punctuation delimits every word) and case fold
//!   simple (`str::eq_ignore_ascii_case` is the whole rule).
//!
//! # Design choices
//!
//! * **Simplified Nazief-Adriani stemmer.** The Nazief and Adriani
//!   (1996) confix-stripping algorithm is the canonical reference
//!   for Indonesian IR stemmers. The reference algorithm consults a
//!   root-word dictionary at every strip step; this crate ships the
//!   algorithm's **rule structure without the dictionary lookup** —
//!   strip decisions are made purely from the surface form under a
//!   3-character length floor and Nazief-Adriani's
//!   consonant-restoration rules. Five ordered steps: stopword
//!   short-circuit → particle suffix → possessive suffix →
//!   derivational suffix → derivational prefix (with `me-`/`pe-`
//!   consonant restoration). See [`stemmer`].
//! * **~90-word stopword list.** The intersection of published
//!   Indonesian stopword collections (Tala 2003 companion list, the
//!   Sastrawi library's `id-stopwords`, the Snowball-community
//!   candidate list). Coverage: coordinating and subordinating
//!   conjunctions, prepositions, personal / demonstrative /
//!   interrogative pronouns, copular / existential / auxiliary
//!   verbs, common adverbs, negations, and numerals up to
//!   `sepuluh`. See [`stopwords`].
//! * **PHONEX-Indonesian phonetic encoder.** A light Soundex-shape
//!   4-character encoder with Indonesian-tuned preprocessing:
//!   digraph rewrites `ny → N`, `ng → G`, `sy → S`, `kh → K`; silent
//!   `H` dropped after the digraph pass. Adapter name `"phonex-id"`.
//!   See [`phonetic`].
//! * **Simple tokenizer.** Indonesian is delimiter-clean and uses
//!   ASCII letters exclusively; the default
//!   [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer) suffices.
//!   [`IndonesianTokenizer`] is a transparent wrapper for
//!   pack-pattern consistency. Reduplication (`buku-buku` "books")
//!   splits at the hyphen and the halves stem to the same base
//!   individually. See [`tokenizer`].
//! * **ASCII case fold.** Indonesian orthography is exactly the
//!   26-letter Latin alphabet — no diacritics, no locale-specific
//!   case-fold rules. The default
//!   [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//!   (which uses [`str::eq_ignore_ascii_case`]) is correct without
//!   an override.
//! * **Default Unicode collation.** Indonesian sorts under the
//!   default Latin alphabet order — no CLDR tailoring is needed.
//!   [`Language::collator`](stringcheese_lang::Language::collator)
//!   returns `None`.
//!
//! # Deferred to a follow-up wave
//!
//! * **Dictionary-backed root confirmation.** The reference
//!   Nazief-Adriani algorithm accepts a strip only when the residue
//!   is a valid root; the Sastrawi library ships a 30 000-word
//!   dictionary. Adding one would eliminate a class of over-strips
//!   (`pergi` "go" → `perg` under the shipped rules is one such
//!   case) at the cost of a static-data payload.
//! * **Malay sibling (`stringcheese-ms`).** Malaysian Malay and
//!   Indonesian share ~80 % of core vocabulary and identical
//!   morphology; a Malay pack would fork the stopword list and
//!   possibly the stemmer's few Indonesian-specific spellings, but
//!   the algorithm and phonetic encoder would carry over unchanged.
//! * **Métaphone-shaped variable-length phonetic encoder.** A
//!   parallel variable-length key would improve record-linkage
//!   precision; heavier to reference-test and out of scope for the
//!   initial drop.
//! * **Colloquial / SMS register.** Common informal spellings (`gw`
//!   for `saya`, `lu` for `kamu`, `bgt` for `banget`) are not in
//!   the stopword list; downstream applications with informal
//!   corpora should carry their own list.
//! * **Reduplication canonicalization.** `buku-buku` "books"
//!   currently tokenizes to two halves that stem to `buku`
//!   individually; a follow-up could recombine them.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_id::INDONESIAN;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(INDONESIAN.code(), "id");
//! assert_eq!(INDONESIAN.name(), "Indonesian");
//! assert!(INDONESIAN.is_stopword("dan"));
//! assert!(INDONESIAN.is_stopword("YANG"));  // ASCII case-fold.
//! assert!(!INDONESIAN.is_stopword("buku"));
//! assert_eq!(INDONESIAN.stem("membaca"), "baca");
//! assert_eq!(INDONESIAN.stem("memilih"), "pilih");
//! assert_eq!(INDONESIAN.stem("makanan"), "makan");
//!
//! let toks: Vec<&str> = INDONESIAN
//!     .tokenize("Saya membaca buku di rumah.")
//!     .collect();
//! assert_eq!(toks, ["Saya", "membaca", "buku", "di", "rumah"]);
//! ```
//!
//! # Module map
//!
//! - [`stemmer`] — the [`IndonesianStemmer`] Nazief-Adriani stripper.
//! - [`phonetic`] — [`IndonesianPhonex`] plus the
//!   [`IndonesianPhonexAdapter`] the
//!   [`Language`](stringcheese_lang::Language) trait hands back.
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`tokenizer`] — the [`IndonesianTokenizer`] wrapper.
//! - The [`Indonesian`] type and the [`INDONESIAN`] constant live in
//!   this crate's root.

#![cfg_attr(not(feature = "std"), no_std)]
// `deny` rather than `forbid` because the `stringcheese_lang::
// register_language!` invocation below expands to a `linkme`-backed
// static whose implementation is `unsafe`-tagged (safe in practice
// — that's linkme's whole design — but flagged by the
// `unsafe_code` lint). The macro emits an explicit
// `#[allow(unsafe_code)]` at the sole registration site; the rest
// of this crate is still lint-enforced no-`unsafe`.
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
pub use phonetic::{IndonesianPhonex, IndonesianPhonexAdapter};
#[cfg(feature = "alloc")]
pub use stemmer::IndonesianStemmer;
pub use stopwords::STOPWORDS;
pub use tokenizer::IndonesianTokenizer;

// -----------------------------------------------------------------------
// The Indonesian language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::IndonesianPhonexAdapter;
    use crate::stemmer::IndonesianStemmer;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::IndonesianTokenizer;

    /// The Indonesian language pack.
    ///
    /// Zero-sized; construct as [`Indonesian`] and reuse the value
    /// freely across threads and calls, or grab the crate-level
    /// [`INDONESIAN`](crate::INDONESIAN) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Indonesian;

    /// The static [`IndonesianPhonexAdapter`] [`Indonesian`] hands
    /// back from [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static PHONEX: IndonesianPhonexAdapter = IndonesianPhonexAdapter;

    impl Language for Indonesian {
        fn code(&self) -> &'static str {
            "id"
        }

        fn name(&self) -> &'static str {
            "Indonesian"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        // No `is_stopword` override — Indonesian orthography is
        // strictly ASCII, so the default trait method (which uses
        // `str::eq_ignore_ascii_case`) is correct.

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            IndonesianStemmer.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(IndonesianTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&PHONEX)
        }
    }

    /// The singleton [`Indonesian`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Indonesian`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other `stringcheese-<lang>`
    /// pack follows.
    pub const INDONESIAN: Indonesian = Indonesian;
}

#[cfg(feature = "alloc")]
pub use pack::{INDONESIAN, Indonesian};

// Opt this pack into the shared `stringcheese_lang::registry` — a
// distributed slice populated at link time so callers picking a
// language by BCP-47 code at runtime
// (`stringcheese_lang::registry::language("id")`) find Indonesian
// without naming the crate. See `stringcheese_lang::registry` for the
// design and trade-offs.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(INDONESIAN);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-id` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
