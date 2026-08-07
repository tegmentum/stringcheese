//! Bulgarian language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Bulgarian`] value that carries the Bulgarian stopword
//! list, the [`BulgarianSnowball`] stemmer (with the definite-article
//! step Bulgarian requires), the whitespace-and-punctuation
//! [`BulgarianTokenizer`], and a [`BulgarianGost779B`] transliteration
//! phonetic hookup. Callers grab the singleton [`BULGARIAN`] `const` —
//! no construction ceremony required — and delegate through the
//! [`Language`](stringcheese_lang::Language) trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-bg` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Bulgarian stopword table
//! or the Snowball Bulgarian stemmer's code. Callers who need
//! Bulgarian add `stringcheese-bg = "0.1"` to their `Cargo.toml`
//! explicitly.
//!
//! # Third Cyrillic-script pack
//!
//! This is the third `stringcheese-<lang>` implementation for a script
//! written in Cyrillic — Russian shipped in wave 7 and Ukrainian in
//! wave 8, both establishing the shape (`Vec<char>` suffix arithmetic,
//! `is_stopword` override with Unicode case-fold, deterministic
//! ASCII-only transliteration). Bulgarian follows that shape with two
//! Bulgarian-specific twists:
//!
//! ## Bulgarian alphabet — 30 letters
//!
//! Bulgarian uses a 30-letter subset of the Cyrillic block:
//!
//! ```text
//! а б в г д е ж з и й к л м н о п р с т у ф х ц ч ш щ ъ ь ю я
//! ```
//!
//! Compared with the modern Russian alphabet (33 letters), Bulgarian
//! **omits three Russian-only letters** and **repurposes one**:
//!
//! * **`ё` (U+0451)** — absent from Bulgarian. Bulgarian orthography
//!   never marks the diaeresis on `е`, so there is no `ё → е` fold in
//!   the stemmer or `is_stopword` implementation (contrast with
//!   `stringcheese-ru`, which does apply that fold).
//! * **`ы` (U+044B)** — absent from Bulgarian. Bulgarian has no
//!   high-back-unrounded vowel of that shape.
//! * **`э` (U+044D)** — absent from Bulgarian. Bulgarian's plain `е`
//!   (U+0435) covers /ɛ/.
//! * **`ъ` (U+044A) is a full vowel**, not a hard sign. In Russian,
//!   `ъ` is a purely orthographic hard-sign glyph carrying no phonetic
//!   content; in Bulgarian, `ъ` is the mid-central vowel /ɤ/ (the
//!   schwa-like sound in `България` — "Bulgaria" — itself). This
//!   changes the vowel set the Snowball region calculation walks over
//!   (Bulgarian counts `ъ` as a vowel; Russian does not) and it changes
//!   the transliteration rendering (`ъ → a` in Bulgarian, `ъ → ''` in
//!   Russian GOST-B).
//! * **`щ`** — pronounced differently. Russian `щ` is a long soft `ʃː`;
//!   Bulgarian `щ` is a cluster `ʃt`. The two transliteration adapters
//!   render this divergence: Russian GOST-B is `щ → shh`; Bulgarian
//!   GOST-B is `щ → sht`.
//!
//! ## Bulgarian definite article — the signature morphological feature
//!
//! Bulgarian is **the** analytic Slavic language: it dropped nominal
//! case declension, but in exchange it retained (and expanded) a
//! **postposed definite article** that attaches to the noun as a
//! suffix rather than standing alone as a separate word the way
//! English's `the` does. The article agrees with the noun's gender
//! and number:
//!
//! | Gender / number | Full article | Short article | Example |
//! |-----------------|--------------|---------------|---------|
//! | Masculine | `-ът` (subject) | `-а` (object) | `градът` (the city) / `града` |
//! | Masculine (soft) | `-ят` | `-я` | `учителят` (the teacher) |
//! | Feminine | `-та` | — | `книгата` (the book) |
//! | Neuter | `-то` | — | `детето` (the child) |
//! | Plural | `-те` | — | `децата` (the children — after -а plural), `столовете` (the tables) |
//! | Plural (after -а) | `-та` | — | `имената` (the names) |
//!
//! A retrieval pipeline that treats `книгата` and `книга` as different
//! surface forms will fail to link `book`-scoped documents that happen
//! to render the noun with a definite article. The [`snowball`]
//! stemmer therefore runs the article-stripping step **first**, before
//! any other suffix cascade, so that `книгата → книг` and
//! `човекът → човек` collapse to the same stem as `книга` and
//! `човек`.
//!
//! ## Cyrillic-specific invariants (same as Russian / Ukrainian)
//!
//! * **Every letter is 2 bytes in UTF-8.** Bulgarian's alphabet sits
//!   entirely inside U+0400..=U+045F, which falls in UTF-8's 2-byte
//!   range (U+0080..=U+07FF). A word like `"стол"` (4 characters) is
//!   8 bytes. All suffix and region arithmetic runs on `Vec<char>` in
//!   [`snowball`], never raw byte offsets, so no scalar is ever sliced
//!   apart. Callers of
//!   [`Language::stem`](stringcheese_lang::Language::stem) and
//!   [`Language::tokenize`](stringcheese_lang::Language::tokenize) never
//!   see the char-vs-byte distinction because the returned
//!   [`Cow<str>`](alloc::borrow::Cow) and token slices are always
//!   valid UTF-8; downstream callers that do byte-level arithmetic on
//!   the outputs must remember the 2x expansion factor.
//! * **No Turkic-fold concerns.** Bulgarian case-folding is
//!   well-behaved under Rust's default [`char::to_lowercase`]:
//!   `А → а`, `Ъ → ъ`, `Я → я`. There is no locale tailoring the way
//!   Turkish requires for the dotted / dotless-I distinction.
//! * **UTF-8 code-point processing order is left-to-right.** No RTL
//!   concerns; the tokenizer emits tokens in reading order.
//!
//! # Design choices
//!
//! * **Snowball Bulgarian stemmer.** Based on the Nakov (2003)
//!   algorithm documented at
//!   <https://snowballstem.org/algorithms/bulgarian/stemmer.html>. The
//!   algorithm is a four-step cascade: **remove the definite article
//!   (first)**, then remove plural markers, then verb / participle
//!   endings, then a palatal-alternation / general-suffix pass.
//!   Bulgarian's vowel set for the R1 region calculation includes
//!   `ъ` (unlike Russian). See [`snowball`] for the rules.
//! * **~236-word stopword list.** Common Bulgarian pronouns,
//!   demonstratives, interrogatives, conjunctions, prepositions,
//!   particles, high-frequency forms of the copula *съм*, and
//!   quantifiers. Where the definite article turns a noun into a
//!   function-word-ish form (`всичко` — "everything", `нищо` —
//!   "nothing"), the definite form is carried alongside the bare form.
//!   See [`stopwords`].
//! * **GOST 7.79-B transliteration, Bulgarian adaptation.** A
//!   deterministic, ASCII-only Cyrillic → Latin mapping tailored to
//!   the Bulgarian letter set and pronunciation. Adapter name:
//!   `"gost-7.79-b-bg"` — the `-bg` disambiguates it from the Russian
//!   `"gost-7.79-b"` and the Ukrainian `"gost-7.79-b-uk"` (the three
//!   encoders differ on `ъ`, `щ`, `х`, `ц`, and cover overlapping but
//!   distinct letter sets). See [`phonetic`] for the mapping table and
//!   the rationale.
//! * **Simple tokenizer.** Bulgarian orthography uses ASCII spaces
//!   between orthographic words, and every letter of the Bulgarian
//!   alphabet satisfies [`char::is_alphanumeric`], so the default
//!   splitter handles Bulgarian word segmentation correctly out of the
//!   box. [`BulgarianTokenizer`] is a transparent wrapper around
//!   [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer).
//! * **Cyrillic-aware case fold.** Uses Rust's default
//!   [`char::to_lowercase`] — no Turkic-style tailoring, no `ё → е`
//!   precomputation (Bulgarian has no `ё`). The `is_stopword`
//!   override lowercases under this rule so uppercase Cyrillic queries
//!   match the plain stopword list.
//! * **Default Unicode collation.**
//!   [`Language::collator`](stringcheese_lang::Language::collator)
//!   returns `None`. Callers who need locale-aware Bulgarian
//!   collation should reach for `icu_collator`.
//!
//! # Deferred to a follow-up wave
//!
//! * **Macedonian pack** (`stringcheese-mk`). Macedonian is
//!   linguistically the closest sibling to Bulgarian — both are
//!   analytic South Slavic languages with postposed definite articles
//!   — but Macedonian carries additional letters (`ѓ`, `ќ`, `ѕ`, `ј`,
//!   `љ`, `њ`, `џ`) and its definite article system has three-way
//!   proximal / medial / distal distinctions (`-от` / `-ов` / `-он`
//!   for masculine, etc.) that Bulgarian does not. Would deserve its
//!   own crate.
//! * **Old Church Slavonic pack** (`stringcheese-cu`). Would need
//!   handling for the extended historical Cyrillic block (`ѣ`, `ѫ`,
//!   `ѧ`, `ѩ`, `ѭ`, `ѱ`, `ѳ`, `ѵ`) and an entirely different
//!   morphological analyzer — OCS retained the full Common Slavic
//!   case system and dual number.
//! * **Church Slavic Snowball variant.** Would be a modern-Bulgarian
//!   stemmer adjusted for OCS orthography — probably a niche addition
//!   on top of a full OCS pack.
//! * **Belarusian pack** (`stringcheese-be`). East Slavic sibling to
//!   Russian and Ukrainian; would carry `ў` (U+045E) and follow the
//!   Russian-style suffix patterns.
//! * **ISO 9 System A transliteration alongside GOST 7.79-B.** Would
//!   want it under a separate adapter for library-catalog interop.
//! * **Slavic-Metaphone.** A sound-alike phonetic encoder that spans
//!   Bulgarian, Russian, Ukrainian, and Serbian would complement the
//!   shipped transliteration for cross-Slavic record linkage.
//! * **Full-vocabulary cross-verification.** The shipped
//!   reference-pair test embeds a hand-traced subset; a full corpus
//!   cross-check would want a Bulgarian voc.txt/output.txt pair.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_bg::BULGARIAN;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(BULGARIAN.code(), "bg");
//! assert_eq!(BULGARIAN.name(), "Bulgarian");
//! assert!(BULGARIAN.is_stopword("и"));
//! assert!(BULGARIAN.is_stopword("НЕ"));   // Cyrillic case-fold: НЕ → не.
//! assert!(!BULGARIAN.is_stopword("книга"));
//!
//! // The signature Bulgarian move: definite-article stripping.
//! // `книгата` ("the book") stems to the same form as `книга` ("book").
//! assert_eq!(BULGARIAN.stem("книгата"), BULGARIAN.stem("книга"));
//! assert_eq!(BULGARIAN.stem("човекът"), "човек");
//!
//! let toks: Vec<&str> = BULGARIAN
//!     .tokenize("Здравей, свят! София — столица.")
//!     .collect();
//! assert_eq!(toks, ["Здравей", "свят", "София", "столица"]);
//! ```
//!
//! # Module map
//!
//! - [`snowball`] — the [`BulgarianSnowball`] stemmer.
//! - [`phonetic`] — [`BulgarianGost779B`] plus the
//!   [`BulgarianGost779BAdapter`] the
//!   [`Language`](stringcheese_lang::Language) trait hands back.
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`tokenizer`] — the [`BulgarianTokenizer`] wrapper.
//! - The [`Bulgarian`] type and the [`BULGARIAN`] constant live in
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
pub use phonetic::{BulgarianGost779B, BulgarianGost779BAdapter};
#[cfg(feature = "alloc")]
pub use snowball::BulgarianSnowball;
pub use stopwords::STOPWORDS;
pub use tokenizer::BulgarianTokenizer;

// -----------------------------------------------------------------------
// The Bulgarian language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;
    use alloc::string::String;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::BulgarianGost779BAdapter;
    use crate::snowball::BulgarianSnowball;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::BulgarianTokenizer;

    /// The Bulgarian language pack.
    ///
    /// Zero-sized; construct as [`Bulgarian`] and reuse the value
    /// freely across threads and calls, or grab the crate-level
    /// [`BULGARIAN`](crate::BULGARIAN) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Bulgarian;

    /// The static [`BulgarianGost779BAdapter`] [`Bulgarian`] hands
    /// back from [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static GOST_779_B_BG: BulgarianGost779BAdapter = BulgarianGost779BAdapter;

    /// Normalize a Cyrillic string for stopword comparison: lowercase
    /// under default Unicode rules. Unlike the Russian pack, no
    /// `ё → е` fold is applied — Bulgarian does not use `ё`.
    fn normalize_for_stopword(word: &str) -> String {
        let mut out = String::with_capacity(word.len());
        for c in word.chars() {
            for lc in c.to_lowercase() {
                out.push(lc);
            }
        }
        out
    }

    impl Language for Bulgarian {
        fn code(&self) -> &'static str {
            "bg"
        }

        fn name(&self) -> &'static str {
            "Bulgarian"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        /// Cyrillic-aware stopword membership.
        ///
        /// Overrides the default trait implementation (which uses
        /// [`str::eq_ignore_ascii_case`], missing every uppercase
        /// Cyrillic input) with a Unicode lowercase pass — so `НЕ`
        /// and `не` both find `не` in the stopword list.
        fn is_stopword(&self, word: &str) -> bool {
            let normalized = normalize_for_stopword(word);
            STOPWORDS.contains(&normalized.as_str())
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            BulgarianSnowball.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(BulgarianTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&GOST_779_B_BG)
        }
    }

    /// The singleton [`Bulgarian`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Bulgarian`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other `stringcheese-<lang>`
    /// pack follows.
    pub const BULGARIAN: Bulgarian = Bulgarian;
}

#[cfg(feature = "alloc")]
pub use pack::{BULGARIAN, Bulgarian};

// Register into `stringcheese-lang::registry` so callers who look up
// languages dynamically (`registry::language("bg")`) find this pack.
// alloc-gated because the pack constant itself is alloc-gated.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(BULGARIAN);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-bg` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
