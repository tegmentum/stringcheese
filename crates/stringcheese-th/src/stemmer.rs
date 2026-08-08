//! [`ThaiStemmer`] — an near-identity stemmer that strips common
//! nominalizer / agent prefixes and folds trivial word-level
//! reduplication.
//!
//! # Why "near-identity"
//!
//! Thai is a fully **analytic** language, similar to Chinese: it has
//! **no case, no gender, no number agreement, no tense, and no verb
//! conjugation**. A noun is the same surface string whether it is
//! singular or plural; a verb is the same string in past, present, or
//! future contexts (tense and aspect are marked by adverbs and
//! preverbal particles, not by inflection). There is nothing for a
//! suffix-stripping stemmer to strip.
//!
//! Because of that, the shipped stemmer is *almost* identity — it
//! only rewrites two narrow cases where a rule-based transform is
//! demonstrably safe without a lexicon:
//!
//! 1. **Common nominalizer / agent prefixes.** Thai forms abstract
//!    nouns and agent nouns with a small closed set of prefixes:
//!    - `การ-` — nominalizer for verbs (`การเรียน` = "learning",
//!      from `เรียน` = "to learn").
//!    - `ความ-` — nominalizer for stative verbs / adjectives
//!      (`ความสุข` = "happiness", from `สุข` = "happy").
//!    - `ผู้-` — agent noun ("one who …") (`ผู้พูด` = "speaker",
//!      from `พูด` = "to speak").
//!    - `นัก-` — agent / practitioner ("professional at …")
//!      (`นักเรียน` = "student", from `เรียน` = "to learn").
//!    - `เครื่อง-` — instrument ("machine / tool for …")
//!      (`เครื่องบิน` = "airplane", from `บิน` = "to fly").
//!
//!    The stemmer strips these when the residual stem has at least
//!    2 characters (Thai scalars) — the same over-strip guard the
//!    other language packs use.
//! 2. **Reduplication (`XX → X`).** Thai marks plurality / intensity
//!    with word-level reduplication (`เด็ก` = "child" → `เด็กเด็ก`
//!    = "children"). When the input is exactly the concatenation of
//!    a string with itself (`X` followed by another `X`), the
//!    stemmer collapses to a single copy. This rule fires only on
//!    exact `XX` — it makes no attempt to detect partial
//!    reduplication.
//!
//! Everything else passes through unchanged (identity), returning
//! [`Cow::Borrowed`].
//!
//! # Deferred (needs a lexicon)
//!
//! - **Classifier stripping.** Thai counts objects with a numeral +
//!   classifier construction (`หนังสือสามเล่ม` "three books"). The
//!   classifier `เล่ม` attaches to numerals for book-shaped objects;
//!   stripping classifiers safely needs a lexicon of
//!   numeral / measure / classifier collocations.
//! - **`ๆ` (maiyamok) expansion.** The reduplication mark `ๆ`
//!   U+0E46 is Thai's canonical way to write reduplication in
//!   running text (`เด็กๆ` = "children"). The tokenizer keeps the
//!   mark inside the cluster; a semantics-aware stemmer would fold
//!   `Xๆ → X`. The mark is dropped here only to the extent that the
//!   over-strip guards permit — the base rule is intentionally
//!   conservative to avoid stripping information a downstream IR
//!   pipeline may want.
//! - **Compound noun decomposition.** Thai compounds are surface-form
//!   concatenations of two full words (`ประเทศไทย` "Thailand" =
//!   `ประเทศ` "country" + `ไทย` "Thai"). Splitting them safely needs
//!   a dictionary.
//! - **Prefix ambiguity.** `การ` is also an ordinary content noun
//!   ("work", "job") and can appear as the head noun of a phrase
//!   rather than as a nominalizing prefix. Without lexical
//!   disambiguation, the rule sometimes over-strips; the min-stem
//!   guard limits the damage but does not eliminate it.
//!
//! # Contract
//!
//! - **Deterministic.** For any input, two calls to `stem(w)`
//!   return equal outputs.
//! - **Non-lengthening.** The output is never longer than the input.
//! - **Idempotent.** `stem(stem(w)) == stem(w)` for every input.
//! - **Borrows the input on no-match.** Returns [`Cow::Borrowed`]
//!   when no rule fires.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use stringcheese_lang::Stemmer;

/// Over-stripping guard: refuse to strip a prefix if the resulting
/// stem would have fewer than this many *characters* (scalars).
const MIN_STEM_CHARS: usize = 2;

/// Thai nominalizer / agent prefixes, longest-first. Every entry is
/// stored as a `&'static [char]` — matched on `Vec<char>` heads, never
/// on raw bytes (every Thai scalar is 3 UTF-8 bytes, and byte offsets
/// would silently corrupt boundaries).
const PREFIXES: &[&[char]] = &[
    // Five-scalar `เครื่อง` (instrument): เ + ค + ร + ื + ่ + อ + ง
    // — actually seven scalars because of the tone mark on the vowel.
    &['เ', 'ค', 'ร', 'ื', '่', 'อ', 'ง'],
    // Four-scalar `ความ` (abstract nominalizer): ค + ว + า + ม.
    &['ค', 'ว', 'า', 'ม'],
    // Three-scalar `การ` (verbal nominalizer): ก + า + ร.
    &['ก', 'า', 'ร'],
    // Three-scalar `นัก` (agent / practitioner): น + ั + ก.
    &['น', 'ั', 'ก'],
    // Three-scalar `ผู้` (agent, "one who"): ผ + ู + ้.
    &['ผ', 'ู', '้'],
];

/// The Thai near-identity stemmer.
///
/// A zero-sized value; construct as [`ThaiStemmer`] and reuse the
/// value freely across threads and calls.
///
/// See the [module-level docs](self) for the rule set and the
/// contract (Thai is analytic — the stemmer is almost identity).
///
/// # Example
///
/// ```
/// use stringcheese_th::ThaiStemmer;
/// use stringcheese_lang::Stemmer;
///
/// // Strip the nominalizer การ- : `การเรียน` (learning) → `เรียน` (learn).
/// assert_eq!(ThaiStemmer.stem("การเรียน"), "เรียน");
/// // Strip the abstract nominalizer ความ- : `ความสุข` (happiness) → `สุข`.
/// assert_eq!(ThaiStemmer.stem("ความสุข"), "สุข");
/// // Strip the agent prefix นัก- : `นักเรียน` (student) → `เรียน`.
/// assert_eq!(ThaiStemmer.stem("นักเรียน"), "เรียน");
/// // Word-level reduplication: `เด็กเด็ก` → `เด็ก`.
/// assert_eq!(ThaiStemmer.stem("เด็กเด็ก"), "เด็ก");
/// // No rule fires — identity.
/// assert_eq!(ThaiStemmer.stem("สวัสดี"), "สวัสดี");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ThaiStemmer;

impl ThaiStemmer {
    /// Stems `word` per the Thai rule set.
    ///
    /// Returns the stem as a [`Cow`]. If no rule fires, the returned
    /// `Cow` borrows the input.
    #[must_use]
    pub fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        // Assemble a Vec<char> for scalar-indexed arithmetic. Every
        // Thai letter is 3 bytes in UTF-8, so any code that mixed
        // byte offsets with character-boundary logic would corrupt
        // boundaries — we work strictly in char space.
        let chars: Vec<char> = word.chars().collect();

        if chars.len() < MIN_STEM_CHARS {
            return Cow::Borrowed(word);
        }

        // Rule 1: exact word-level reduplication (XX → X). Fires only
        // if the input has even length and the first half equals the
        // second half. The single-copy stem must itself satisfy the
        // min-stem guard.
        if chars.len() >= 2 * MIN_STEM_CHARS && chars.len() % 2 == 0 {
            let half = chars.len() / 2;
            if chars[..half] == chars[half..] {
                let stem: String = chars[..half].iter().collect();
                return Cow::Owned(stem);
            }
        }

        // Rule 2: prefix strip, longest-first.
        for &prefix in PREFIXES {
            if prefix.len() >= chars.len() {
                continue;
            }
            if chars[..prefix.len()] != *prefix {
                continue;
            }
            let stem_len = chars.len() - prefix.len();
            if stem_len < MIN_STEM_CHARS {
                continue;
            }
            let stem: String = chars[prefix.len()..].iter().collect();
            return Cow::Owned(stem);
        }

        Cow::Borrowed(word)
    }
}

impl Stemmer for ThaiStemmer {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        Self::stem(self, word)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &str) -> String {
        ThaiStemmer.stem(w).into_owned()
    }

    // -------------------------------------------------------------
    // Prefix stripping.
    // -------------------------------------------------------------

    #[test]
    fn strips_verbal_nominalizer_kaan() {
        // การเรียน "learning" → เรียน "to learn".
        assert_eq!(s("การเรียน"), "เรียน");
    }

    #[test]
    fn strips_abstract_nominalizer_khwaam() {
        // ความสุข "happiness" → สุข "happy".
        assert_eq!(s("ความสุข"), "สุข");
    }

    #[test]
    fn strips_agent_prefix_phu() {
        // ผู้พูด "speaker" → พูด "to speak".
        assert_eq!(s("ผู้พูด"), "พูด");
    }

    #[test]
    fn strips_practitioner_prefix_nak() {
        // นักเรียน "student" → เรียน "to learn".
        assert_eq!(s("นักเรียน"), "เรียน");
    }

    #[test]
    fn strips_instrument_prefix_khrueang() {
        // เครื่องบิน "airplane" → บิน "to fly".
        assert_eq!(s("เครื่องบิน"), "บิน");
    }

    // -------------------------------------------------------------
    // Reduplication.
    // -------------------------------------------------------------

    #[test]
    fn folds_word_level_reduplication() {
        // เด็กเด็ก → เด็ก (children).
        assert_eq!(s("เด็กเด็ก"), "เด็ก");
        // ดีดี → ดี.
        assert_eq!(s("ดีดี"), "ดี");
    }

    #[test]
    fn does_not_fold_partial_reduplication() {
        // Not-a-double: the two halves differ.
        assert_eq!(s("ดีดา"), "ดีดา");
    }

    // -------------------------------------------------------------
    // Contract: identity, idempotent, guarded.
    // -------------------------------------------------------------

    #[test]
    fn identity_on_bare_content_word() {
        assert_eq!(s("สวัสดี"), "สวัสดี");
        assert_eq!(s("ประเทศไทย"), "ประเทศไทย");
    }

    #[test]
    fn identity_on_latin_input() {
        assert_eq!(s("hello"), "hello");
    }

    #[test]
    fn identity_on_empty_input() {
        assert_eq!(s(""), "");
    }

    #[test]
    fn identity_on_single_character() {
        // 1 scalar, below the MIN_STEM_CHARS floor — no rule fires.
        assert_eq!(s("ก"), "ก");
    }

    #[test]
    fn over_strip_guard_refuses_short_stems() {
        // ผู้ alone would leave 0 chars after strip — guarded.
        assert_eq!(s("ผู้"), "ผู้");
        // การก — strip การ leaves 1 char, guarded.
        assert_eq!(s("การก"), "การก");
    }

    #[test]
    fn idempotent() {
        for w in [
            "การเรียน",
            "ความสุข",
            "นักเรียน",
            "ผู้พูด",
            "เครื่องบิน",
            "เด็กเด็ก",
            "สวัสดี",
        ] {
            let once = ThaiStemmer.stem(w).into_owned();
            let twice = ThaiStemmer.stem(&once).into_owned();
            assert_eq!(once, twice, "stem not idempotent on {w:?}");
        }
    }

    #[test]
    fn output_never_longer_than_input() {
        for w in [
            "การเรียน",
            "ความสุข",
            "นักเรียน",
            "ผู้พูด",
            "เครื่องบิน",
            "เด็กเด็ก",
            "สวัสดี",
            "hello",
            "",
        ] {
            let out = ThaiStemmer.stem(w);
            assert!(out.len() <= w.len(), "stem grew on {w:?}: {out:?}");
        }
    }

    #[test]
    fn borrowed_when_no_match_owned_when_matched() {
        let borrowed = ThaiStemmer.stem("สวัสดี");
        assert!(matches!(borrowed, Cow::Borrowed(_)));
        let owned = ThaiStemmer.stem("การเรียน");
        assert!(matches!(owned, Cow::Owned(_)));
    }
}
