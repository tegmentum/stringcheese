//! Belarusian language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Belarusian`] value that carries the Belarusian
//! stopword list, the [`BelarusianStemmer`] light suffix-stripping
//! stemmer, the apostrophe-aware [`BelarusianTokenizer`], and a
//! [`BelarusianPhonex`] Soundex-shape phonetic hookup. Callers grab
//! the singleton [`BELARUSIAN`] `const` — no construction ceremony
//! required — and delegate through the
//! [`Language`](stringcheese_lang::Language) trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-be` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Belarusian stopword
//! table or the light stemmer's code. Callers who need Belarusian add
//! `stringcheese-be = "0.1"` to their `Cargo.toml` explicitly.
//!
//! # Third Cyrillic-script pack
//!
//! This is the third `stringcheese-<lang>` implementation for a script
//! written in Cyrillic — Russian shipped first (setting the
//! `Vec<char>` suffix-arithmetic shape), then Ukrainian (extending the
//! shape with apostrophe-aware tokenization and Ukrainian-adapted
//! GOST-B), and now Belarusian.
//!
//! ## Belarusian-specific letter set
//!
//! Belarusian's alphabet takes a subset of Russian's letters and adds
//! two Belarusian-only graphemes:
//!
//! * **`ў` (U+045E) / `Ў` (U+040E)** — the **short u**, a labial
//!   glide /w/ that appears after vowels (`праўда`, `аўтар`). It has
//!   no Russian or Ukrainian counterpart; in the PHONEX-Belarusian
//!   encoder [`phonetic`] it folds to `W` and joins the labial class
//!   (`B`, `P`, `F`, `V`) — consonant class 1 — because functionally
//!   it is a labial glide, not a vowel.
//! * **`і` (U+0456) / `І` (U+0406)** — shared with Ukrainian; used
//!   for the /i/ vowel in place of Russian's `и`.
//!
//! Belarusian **does not** use these Russian letters:
//!
//! * **`и` (U+0438)** — replaced by `і`.
//! * **`щ` (U+0449)** — Belarusian's equivalent phoneme is written as
//!   the digraph `шч` (`яшчэ`, `шчотка`).
//! * **`ъ` (U+044A, hard sign)** — replaced by the **ASCII apostrophe
//!   `'` (U+0027)** in words like `сям'я` (family), `аб'ект` (object),
//!   `пад'езд` (entrance). See [`tokenizer`] for how the apostrophe
//!   is preserved as a word-internal character.
//!
//! Belarusian **does** use these letters that Ukrainian does not:
//!
//! * **`ё` (U+0451)** — the iotated /o/ vowel. Belarusian uses it
//!   consistently (unlike Russian's optional diaeresis-drop). No fold
//!   to `е` is applied.
//! * **`ы` (U+044B)** — Belarusian carries `ы` alongside `і`; the two
//!   are distinct vowels.
//! * **`э` (U+044D)** — a full Belarusian vowel (`гэта`, `яшчэ`).
//!
//! ## Belarusian-specific consonant digraphs
//!
//! Belarusian orthography treats **`дж`** and **`дз`** as digraphs —
//! two Cyrillic scalars that render single phonemes (voiced
//! postalveolar affricate /d͡ʒ/ and voiced alveolar affricate /d͡z/).
//! The [`phonetic`] encoder rewrites both digraphs to single ASCII
//! placeholder letters (`J` for `дж`, `Z` for `дз`) before the
//! Soundex-shape encoding pass, so a digraph counts as one letter for
//! the duplicate-collapse rule. The stemmer treats them as ordinary
//! character sequences — Belarusian inflectional suffixes never span
//! into a digraph.
//!
//! ## The Cyrillic-specific invariants (unchanged from Russian /
//! Ukrainian)
//!
//! * **Every letter is 2 bytes in UTF-8.** All suffix and region
//!   arithmetic in [`stemmer`] runs on `Vec<char>`, never raw byte
//!   offsets, so no scalar is ever sliced apart. Callers of
//!   [`Language::stem`](stringcheese_lang::Language::stem) and
//!   [`Language::tokenize`](stringcheese_lang::Language::tokenize)
//!   never see the char-vs-byte distinction because the returned
//!   [`Cow<str>`](alloc::borrow::Cow) and token slices are always
//!   valid UTF-8; downstream callers that do byte-level arithmetic on
//!   the outputs must remember the 2x expansion factor.
//! * **No Turkic-fold concerns.** Belarusian case-folding is
//!   well-behaved under Rust's default [`char::to_lowercase`]:
//!   `А → а`, `Ў → ў`, `І → і`, `Я → я`. There is no locale tailoring
//!   the way Turkish requires for the dotted / dotless-I distinction.
//! * **UTF-8 code-point processing order is left-to-right.** No RTL
//!   concerns; the tokenizer emits tokens in reading order.
//!
//! # Design choices
//!
//! * **Light suffix-stripping stemmer.** Like Ukrainian, Belarusian is
//!   **not covered by an official Snowball stemmer** — the Snowball
//!   repository has no `belarusian.sbl`. This crate ships a **light
//!   suffix-stripping stemmer** with an explicit scope (strip common
//!   noun / adjective / verb suffixes in a single longest-match pass,
//!   guarded by an RV floor and a theme-vowel context guard for
//!   past-tense endings) rather than a non-canonical port of the
//!   Russian algorithm. Rationale: a Russian port with the letter set
//!   adjusted would inherit Russian assumptions about
//!   perfective-gerund contexts, `нн` participle undoublement, and
//!   `ость` derivational endings that either do not apply to
//!   Belarusian or apply differently. See [`stemmer`] for the rules
//!   and reference-pair coverage.
//! * **~85-word stopword list.** Union of published Belarusian
//!   collections (Wikipedia be corpora function-word extractions,
//!   community pymorphy-adjacent Slavic function-word inventories).
//!   Covers pronouns, demonstratives, interrogatives, conjunctions,
//!   prepositions, particles, high-frequency forms of the copula
//!   *быць*, and common adverbs. See [`stopwords`].
//! * **PHONEX-Belarusian phonetic encoder.** A 4-character
//!   Soundex-shape key with Belarusian-tuned preprocessing (digraph
//!   rewrites `дж → J`, `дз → Z`; short-u `ў → W` in the labial
//!   class) over a Slavic-Cyrillic classification table. Adapter
//!   name: `"phonex-be"`. See [`phonetic`] for the mapping table and
//!   rationale.
//! * **Apostrophe-aware tokenizer.** Belarusian orthography uses the
//!   ASCII apostrophe **`'` (U+0027)** as a **word-internal**
//!   separator marking the boundary between a hard consonant and a
//!   following iotated vowel (`сям'я`, `аб'ект`, `пад'езд`). The
//!   pack's [`BelarusianTokenizer`] promotes the apostrophe to a
//!   word-internal character when it sits between two alphanumeric
//!   scalars, while treating every other separator (whitespace,
//!   Unicode punctuation, hyphens, quotes) the same way as
//!   [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer).
//! * **Cyrillic-aware case fold.** Uses Rust's default
//!   [`char::to_lowercase`] — no Turkic-style tailoring. The
//!   `is_stopword` override lowercases under this rule so uppercase
//!   Cyrillic queries match the plain stopword list.
//! * **Default Unicode collation.**
//!   [`Language::collator`](stringcheese_lang::Language::collator)
//!   returns `None`. Callers who need locale-aware Belarusian
//!   collation should reach for `icu_collator`.
//!
//! # Deferred to a follow-up wave
//!
//! * **Canonical Snowball parity.** There is no canonical Snowball
//!   Belarusian; if the Snowball project ever publishes one, this
//!   crate will adopt it under a new module (`snowball_canonical`)
//!   and keep the light stemmer as `stemmer_light` for callers who
//!   want the current behaviour.
//! * **Verb-aspect stripping.** Belarusian's perfective/imperfective
//!   aspect prefixes (`па-`, `на-`, `за-`, `пра-`, …) are
//!   content-carrying and are not stripped by the light stemmer. A
//!   prefix-aware variant is a follow-up.
//! * **Slavic-Metaphone alternate encoder.** Shipping the
//!   [`SlavicMetaphone`](stringcheese_phonetic::SlavicMetaphone)
//!   cross-Slavic encoder behind a Cargo feature (as Russian,
//!   Ukrainian, and Serbian do) is deferred.
//! * **GOST 7.79-B transliteration alongside PHONEX.** A Belarusian-
//!   tuned deterministic Cyrillic → Latin mapping under a separate
//!   adapter for library-catalog interop.
//! * **Taraškievič / Narkamaŭka orthography toggle.** The two
//!   Belarusian orthographies differ on soft-sign placement
//!   (`сьвет` vs `свет`); a normalizer under a Cargo feature is a
//!   follow-up.
//! * **Typographic apostrophe (U+2019) recognition.** The tokenizer
//!   currently promotes only the ASCII apostrophe (U+0027).
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_be::BELARUSIAN;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(BELARUSIAN.code(), "be");
//! assert_eq!(BELARUSIAN.name(), "Belarusian");
//! assert!(BELARUSIAN.is_stopword("і"));
//! assert!(BELARUSIAN.is_stopword("НЕ"));   // Cyrillic case-fold: НЕ → не.
//! assert!(!BELARUSIAN.is_stopword("кніга"));
//! assert_eq!(BELARUSIAN.stem("красівы"), "красів");
//! assert_eq!(BELARUSIAN.stem("сталы"), "стал");
//!
//! let toks: Vec<&str> = BELARUSIAN
//!     .tokenize("Прывітанне, сям'я! Мінск — сталіца.")
//!     .collect();
//! assert_eq!(toks, ["Прывітанне", "сям'я", "Мінск", "сталіца"]);
//! ```
//!
//! # Module map
//!
//! - [`stemmer`] — the [`BelarusianStemmer`] light stemmer.
//! - [`phonetic`] — [`BelarusianPhonex`] plus the
//!   [`BelarusianPhonexAdapter`] the
//!   [`Language`](stringcheese_lang::Language) trait hands back.
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`tokenizer`] — the [`BelarusianTokenizer`] with apostrophe
//!   preservation.
//! - The [`Belarusian`] type and the [`BELARUSIAN`] constant live in
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
pub mod stemmer;
pub mod stopwords;
pub mod tokenizer;

#[cfg(all(test, feature = "std", not(target_family = "wasm")))]
mod properties;

#[cfg(feature = "alloc")]
pub use phonetic::{BelarusianPhonex, BelarusianPhonexAdapter};
#[cfg(feature = "alloc")]
pub use stemmer::BelarusianStemmer;
pub use stopwords::STOPWORDS;
pub use tokenizer::BelarusianTokenizer;

// -----------------------------------------------------------------------
// The Belarusian language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;
    use alloc::string::String;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::BelarusianPhonexAdapter;
    use crate::stemmer::BelarusianStemmer;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::BelarusianTokenizer;

    /// The Belarusian language pack.
    ///
    /// Zero-sized; construct as [`Belarusian`] and reuse the value
    /// freely across threads and calls, or grab the crate-level
    /// [`BELARUSIAN`](crate::BELARUSIAN) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Belarusian;

    /// The static [`BelarusianPhonexAdapter`] [`Belarusian`] hands
    /// back from [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static PHONEX: BelarusianPhonexAdapter = BelarusianPhonexAdapter;

    /// Normalize a Belarusian string for stopword comparison:
    /// lowercase under default Unicode rules. Unlike the Russian pack,
    /// no `ё → е` fold is applied — Belarusian carries `ё` as a
    /// distinct vowel.
    fn normalize_for_stopword(word: &str) -> String {
        let mut out = String::with_capacity(word.len());
        for c in word.chars() {
            for lc in c.to_lowercase() {
                out.push(lc);
            }
        }
        out
    }

    impl Language for Belarusian {
        fn code(&self) -> &'static str {
            "be"
        }

        fn name(&self) -> &'static str {
            "Belarusian"
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
            BelarusianStemmer.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(BelarusianTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&PHONEX)
        }
    }

    /// The singleton [`Belarusian`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Belarusian`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other `stringcheese-<lang>`
    /// pack follows.
    pub const BELARUSIAN: Belarusian = Belarusian;
}

#[cfg(feature = "alloc")]
pub use pack::{BELARUSIAN, Belarusian};

// Register into `stringcheese-lang::registry` so callers who look up
// languages dynamically (`registry::language("be")`) find this pack.
// alloc-gated because the pack constant itself is alloc-gated.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(BELARUSIAN);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-be` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
