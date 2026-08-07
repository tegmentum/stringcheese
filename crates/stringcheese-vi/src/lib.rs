//! Vietnamese language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Vietnamese`] value that carries the Vietnamese
//! stopword list, the [`VietnameseNormalizer`] NFC / tone-mark /
//! full-diacritic configurable normalizer, the [`VietnameseStemmer`]
//! identity-style canonicalizer (Vietnamese is an analytic language
//! with no inflectional morphology to strip), the
//! [`VietnameseTokenizer`] whitespace-and-punctuation splitter, and
//! a [`VietnamesePhonex`] Soundex-shape phonetic hookup. Callers
//! grab the singleton [`VIETNAMESE`] `const` — no construction
//! ceremony required — and delegate through the
//! [`Language`](stringcheese_lang::Language) trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-vi` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Vietnamese stopword
//! table or the normalizer's diacritic-folding tables. Callers who
//! need Vietnamese add `stringcheese-vi = "0.1"` to their
//! `Cargo.toml` explicitly.
//!
//! # Vietnamese orthography — Latin script + two-layer diacritics
//!
//! Vietnamese is written in the Latin script (chữ Quốc ngữ, "national
//! script"), but the surface form carries a **two-layer diacritic
//! system** that no other Latin-scripted language in this workspace
//! uses:
//!
//! ## Layer 1: **letter modifiers** (part of the letter's identity)
//!
//! These diacritics create *distinct letters* in the Vietnamese
//! alphabet — they are not tone marks and are not stripped by the
//! tone-mark-strip flag:
//!
//! * **`ă` (U+0103) / `Ă` (U+0102)** — a-breve, a distinct vowel from
//!   plain `a`. Represents /a/ (short) vs. plain `a` /aː/ (long).
//! * **`â` (U+00E2) / `Â` (U+00C2)** — a-circumflex, distinct vowel.
//!   Represents /ə/.
//! * **`đ` (U+0111) / `Đ` (U+0110)** — d-with-stroke, distinct
//!   *consonant* from plain `d`. Represents /ɗ/ (implosive) vs.
//!   plain `d` /z/ or /j/ depending on dialect.
//! * **`ê` (U+00EA) / `Ê` (U+00CA)** — e-circumflex, distinct vowel.
//! * **`ô` (U+00F4) / `Ô` (U+00D4)** — o-circumflex, distinct vowel.
//! * **`ơ` (U+01A1) / `Ơ` (U+01A0)** — o-with-horn, distinct vowel.
//! * **`ư` (U+01B0) / `Ư` (U+01AF)** — u-with-horn, distinct vowel.
//!
//! Six vowels with letter-modifier diacritics and one consonant
//! (`đ`). These are **not tone marks** — Vietnamese speakers treat
//! them as distinct alphabet entries.
//!
//! ## Layer 2: **tone marks** (five marks, applied on top of any vowel)
//!
//! Vietnamese has **six phonemic tones**; the unmarked vowel carries
//! the level (ngang) tone, and five diacritics mark the other five:
//!
//! | Tone name (VN)  | Diacritic         | Example      | Combining code |
//! |-----------------|-------------------|--------------|----------------|
//! | huyền (falling) | grave `à`         | `à ằ ầ è ề ì ò ồ ờ ù ừ ỳ` | U+0300 |
//! | sắc (rising)    | acute `á`         | `á ắ ấ é ế í ó ố ớ ú ứ ý` | U+0301 |
//! | hỏi (dipping)   | hook-above `ả`    | `ả ẳ ẩ ẻ ể ỉ ỏ ổ ở ủ ử ỷ` | U+0309 |
//! | ngã (creaky-r)  | tilde `ã`         | `ã ẵ ẫ ẽ ễ ĩ õ ỗ ỡ ũ ữ ỹ` | U+0303 |
//! | nặng (heavy)    | dot-below `ạ`     | `ạ ặ ậ ẹ ệ ị ọ ộ ợ ụ ự ỵ` | U+0323 |
//!
//! A vowel can carry **at most one tone mark** (five marks total,
//! plus the unmarked level tone). But a vowel can carry **both** a
//! letter-modifier diacritic (layer 1) and a tone mark (layer 2)
//! simultaneously — for example `ẳ` is `ă` (a-breve) plus the
//! hook-above tone. In NFD form that's three scalars: `a` + combining
//! breve U+0306 + combining hook-above U+0309. In NFC form it is a
//! single precomposed scalar `ẳ` (U+1EB3).
//!
//! ## Why "tone marks" and "letter modifiers" are distinct concepts
//!
//! The distinction is **linguistically real**, not a convenience the
//! pack invented:
//!
//! * **Letter modifiers** change the *segmental phoneme* — `a` and
//!   `ă` are different vowels; `d` and `đ` are different consonants.
//!   Removing them changes the word: `ban` (friend / peers) and
//!   `băn` (Sino-Vietnamese "table") are different words.
//! * **Tone marks** encode *suprasegmental* pitch contour. Removing
//!   them collapses six syllables to one written form, matching how
//!   most Vietnamese IR pipelines treat toneless orthography for
//!   fuzzy search. Removing a tone mark from `bàn` (grave, "table")
//!   yields `ban`, which happens to also be a word — the space
//!   collapses.
//!
//! The [`VietnameseNormalizer::with_strip_tone_marks`] flag strips
//! only the tone marks and preserves the letter modifiers: `ằ → ă`,
//! `ầ → â`, `ộ → ô`, `ự → ư`. The
//! [`VietnameseNormalizer::with_strip_all_diacritics`] flag strips
//! everything and yields plain ASCII: `ằ → a`, `đ → d`, `ự → u`. The
//! two flags are independent; enabling both is redundant but
//! well-defined (the full-strip subsumes the tone-strip).
//!
//! # NFC vs. NFD — why NFC is the default
//!
//! Every Vietnamese vowel-plus-diacritic combination has both a
//! **precomposed** NFC scalar and a **decomposed** NFD sequence:
//!
//! * NFC: `ệ` = U+1EC7 (one scalar, three UTF-8 bytes)
//! * NFD: `ệ` = `e` + U+0302 (circumflex) + U+0323 (dot-below) — three
//!   scalars, five UTF-8 bytes.
//!
//! The web overwhelmingly delivers Vietnamese text in **NFC** — every
//! major browser normalizes form input to NFC before submitting; every
//! Vietnamese input method (Telex, VNI, VIQR) produces NFC output;
//! every Vietnamese dictionary and corpus stores NFC. **The pack
//! defaults to NFC** because that matches what pipelines actually see
//! in the wild.
//!
//! The [`VietnameseNormalizer::with_nfc`] flag is on by default;
//! turning it off preserves whatever composition the caller passed in
//! (useful for callers whose input has *already* been normalized
//! upstream and who want to avoid the second pass). NFD is not offered
//! as a default because operating on NFD would force every downstream
//! rule (tone-mark strip, full-diacritic strip, the PHONEX
//! preprocessor, the tokenizer's per-scalar checks) to walk combining
//! marks — that is fine on paper but wasteful when the input is
//! already NFC.
//!
//! # Design choices
//!
//! * **Configurable normalizer with three orthogonal knobs.** NFC
//!   canonicalization (on by default, lossless), tone-mark stripping
//!   (off by default; reduces the six-tone syllable inventory to one),
//!   and full-diacritic stripping (off by default; folds every
//!   Vietnamese character to plain ASCII). See [`mod@normalize`].
//! * **~180-word stopword list.** Union of published Vietnamese
//!   stopword collections (personal / possessive / demonstrative
//!   pronouns, prepositions, conjunctions, classifiers, particles,
//!   auxiliary-verb forms of `là` / `có` / `được`, and the most-common
//!   discourse markers). See [`stopwords`].
//! * **Identity-style stemmer.** Vietnamese is an **analytic**
//!   language — words do not inflect for tense, number, case, or
//!   gender. There is no meaningful morphological stemming. The
//!   [`VietnameseStemmer`] is documented as a *canonicalizer*: it
//!   returns the input in NFC form (so `ệ` in decomposed form and
//!   `ệ` in precomposed form stem to the same canonical key) and does
//!   nothing else. See [`stemmer`].
//! * **PHONEX-Vietnamese phonetic encoder.** A Soundex-shape 4-char
//!   encoder with Vietnamese-tuned preprocessing (`ng → N`,
//!   `nh → N`, `ph → F`, `kh → K`, `tr → T`, `ch → X`, all
//!   diacritics stripped first). Adapter name: `"phonex-vi"`. See
//!   [`phonetic`].
//! * **Simple tokenizer.** Vietnamese is a space-separated language
//!   (unlike Chinese / Japanese / Thai) — a single syllable is a
//!   single orthographic word (`học sinh` "student" is two words in
//!   the orthography even though it is one semantic unit). The
//!   default
//!   [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer) handles
//!   Vietnamese correctly; [`VietnameseTokenizer`] is a thin wrapper
//!   for pack-pattern consistency. See [`tokenizer`].
//! * **Unicode case fold.** Vietnamese case-folding is well-behaved
//!   under Rust's default [`char::to_lowercase`] — `Ă → ă`, `Đ → đ`,
//!   `Ơ → ơ`, `Ệ → ệ` all work under default rules with no locale
//!   tailoring. The pack overrides
//!   [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//!   to lowercase under this rule so uppercase Vietnamese queries
//!   match the plain lowercase stopword list.
//!
//! # Deferred to a follow-up wave
//!
//! * **Multi-syllable word segmentation.** Vietnamese semantic units
//!   often span multiple orthographic syllables (`học sinh` "student",
//!   `máy tính` "computer"); joining them requires a dictionary
//!   (`VnCoreNLP`, `underthesea`). Out of scope for the offline pack.
//! * **Compound-word lemmatization.** Related — reducing `học sinh`
//!   to a canonical lemma needs a lexicon.
//! * **Regional-variant handling.** Northern (Hanoi), Central (Huế),
//!   and Southern (Saigon) dialects have distinct pronunciations that
//!   the phonetic encoder does not distinguish; a follow-up
//!   `stringcheese-vi-south` pack could ship dialect-specific PHONEX
//!   rules.
//! * **Métaphone Vietnamese** — a variable-length parallel encoder
//!   with better discrimination; heavier to reference-test.
//! * **Vietnamese-tailored collator.** Would compose the ICU CLDR
//!   Vietnamese tailoring (`a < ă < â < b < c < …`; each vowel
//!   collated by tone-mark order). Out of scope for the initial drop.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_lang::Language;
//! use stringcheese_vi::VIETNAMESE;
//!
//! assert_eq!(VIETNAMESE.code(), "vi");
//! assert_eq!(VIETNAMESE.name(), "Vietnamese");
//! assert!(VIETNAMESE.is_stopword("và"));
//! assert!(VIETNAMESE.is_stopword("VÀ")); // Unicode case-fold.
//! assert!(!VIETNAMESE.is_stopword("sách"));
//!
//! let toks: Vec<&str> = VIETNAMESE
//!     .tokenize("Học sinh đọc sách.")
//!     .collect();
//! assert_eq!(toks, ["Học", "sinh", "đọc", "sách"]);
//! ```
//!
//! # Module map
//!
//! - [`mod@normalize`] — the [`VietnameseNormalizer`] and the
//!   [`normalize()`] free function.
//! - [`phonetic`] — [`VietnamesePhonex`] plus the
//!   [`VietnamesePhonexAdapter`] the
//!   [`Language`](stringcheese_lang::Language) trait hands back.
//! - [`stemmer`] — the [`VietnameseStemmer`] identity canonicalizer.
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`tokenizer`] — the [`VietnameseTokenizer`] wrapper.
//! - The [`Vietnamese`] type and the [`VIETNAMESE`] constant live in
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
pub mod normalize;
#[cfg(feature = "alloc")]
pub mod phonetic;
#[cfg(feature = "alloc")]
pub mod stemmer;
pub mod stopwords;
pub mod tokenizer;

#[cfg(all(test, feature = "std", not(target_family = "wasm")))]
mod properties;

#[cfg(feature = "alloc")]
pub use normalize::{VietnameseNormalizer, normalize};
#[cfg(feature = "alloc")]
pub use phonetic::{VietnamesePhonex, VietnamesePhonexAdapter};
#[cfg(feature = "alloc")]
pub use stemmer::VietnameseStemmer;
pub use stopwords::STOPWORDS;
pub use tokenizer::VietnameseTokenizer;

// -----------------------------------------------------------------------
// The Vietnamese language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;
    use alloc::string::String;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::VietnamesePhonexAdapter;
    use crate::stemmer::VietnameseStemmer;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::VietnameseTokenizer;

    /// The Vietnamese language pack.
    ///
    /// Zero-sized; construct as [`Vietnamese`] and reuse the value
    /// freely across threads and calls, or grab the crate-level
    /// [`VIETNAMESE`](crate::VIETNAMESE) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Vietnamese;

    /// The static [`VietnamesePhonexAdapter`] [`Vietnamese`] hands
    /// back from [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static PHONEX: VietnamesePhonexAdapter = VietnamesePhonexAdapter;

    /// Normalize a Vietnamese string for stopword comparison:
    /// lowercase under default Unicode rules. Vietnamese case-fold is
    /// well-behaved under default Unicode rules (`Ă → ă`, `Đ → đ`,
    /// `Ệ → ệ`, etc.) — no locale tailoring required.
    fn normalize_for_stopword(word: &str) -> String {
        let mut out = String::with_capacity(word.len());
        for c in word.chars() {
            for lc in c.to_lowercase() {
                out.push(lc);
            }
        }
        out
    }

    impl Language for Vietnamese {
        fn code(&self) -> &'static str {
            "vi"
        }

        fn name(&self) -> &'static str {
            "Vietnamese"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        /// Unicode-aware stopword membership.
        ///
        /// Overrides the default trait implementation (which uses
        /// [`str::eq_ignore_ascii_case`], missing the case-fold for
        /// Vietnamese's diacritic-carrying uppercase letters) with a
        /// Unicode lowercase pass — so `VÀ` and `và` both find `và`,
        /// and `ĐƯỢC` and `được` both find `được`, in the stopword
        /// list.
        fn is_stopword(&self, word: &str) -> bool {
            let normalized = normalize_for_stopword(word);
            STOPWORDS.contains(&normalized.as_str())
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            VietnameseStemmer.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(VietnameseTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&PHONEX)
        }
    }

    /// The singleton [`Vietnamese`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Vietnamese`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other `stringcheese-<lang>`
    /// pack follows.
    pub const VIETNAMESE: Vietnamese = Vietnamese;
}

#[cfg(feature = "alloc")]
pub use pack::{VIETNAMESE, Vietnamese};

// Opt this pack into the shared `stringcheese_lang::registry` — a
// distributed slice populated at link time so callers picking a
// language by BCP-47 code at runtime
// (`stringcheese_lang::registry::language("vi")`) find Vietnamese
// without naming the crate. See `stringcheese_lang::registry` for the
// design and trade-offs.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(VIETNAMESE);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-vi` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
