//! [`ChineseStemmer`] — an **identity** stemmer.
//!
//! # Why identity?
//!
//! Chinese is a fully **analytic** language: it has no inflectional
//! morphology — no case, no gender, no number agreement, no tense,
//! no conjugation. A noun is the same string whether it's singular
//! or plural, subject or object; a verb is the same string in past,
//! present, or future context (grammatical time is expressed by
//! adverbs and aspect particles, not by verb suffixes).
//!
//! Concretely:
//!
//! - `我` = "I" / "me" (same character regardless of syntactic role).
//! - `买` = "buy" / "bought" / "will buy" / "buying" (tense and aspect
//!   are marked separately: `买了` = "bought", `在买` = "am buying",
//!   `会买` = "will buy").
//! - `狗` = "dog" / "dogs" (plurality is optional and marked with
//!   context, quantifiers, or the plural marker `们` on animate
//!   nouns).
//!
//! There is nothing for a suffix-stripping stemmer to strip. Attempting
//! to strip characters (say, `们` off `我们` "we" to leave `我` "I")
//! would sometimes be the right call (`我们` genuinely does mean "we"
//! and reducing it to `我` for IR is defensible) and sometimes be
//! wrong (a bigram like `他们` "they" reduces to `他` "he/him", which
//! silently drops the plural meaning; and `们` also appears
//! non-pluralwise in idiomatic multi-character words).
//!
//! Because the base pack ships no dictionary — and stripping is only
//! safe with a dictionary that can tell "plural marker suffix" apart
//! from "non-suffix identical characters" — the stemmer is the
//! **identity function**. It returns the input verbatim.
//!
//! # Contract
//!
//! - **Identity.** `stem(w) == w` for every input.
//! - **Idempotent.** Trivially: `stem(stem(w)) == stem(w)`.
//! - **Non-lengthening.** Trivially: `len(stem(w)) == len(w)`.
//! - **Borrows the input.** Returns [`Cow::Borrowed`] every time — no
//!   allocation.
//!
//! # Deferred
//!
//! - **Plural-marker stripping (`们`).** With a dictionary and a
//!   part-of-speech tagger, `我们`/`你们`/`他们`/`她们` could reduce
//!   to `我`/`你`/`他`/`她`, and `孩子们` "children" to `孩子`
//!   "child". Both call sites live in a hypothetical
//!   `stringcheese-zh-jieba` (deferred) — the base pack stays
//!   dictionary-free.
//! - **Reduplication normalization.** Verbs like `看看` "have a look"
//!   or adjectives like `高高兴兴` "very happy" use reduplication for
//!   aspectual / emphatic marking. Collapsing these back to their base
//!   form (`看`, `高兴`) needs pattern-matching that a suffix-stripper
//!   cannot provide safely.

use alloc::borrow::Cow;

use stringcheese_lang::Stemmer;

/// The Chinese identity stemmer.
///
/// A zero-sized value; construct as [`ChineseStemmer`] and reuse the
/// value freely across threads and calls.
///
/// See the [module-level docs](self) for the rationale (Chinese is
/// analytic — no inflection to strip).
///
/// # Example
///
/// ```
/// use stringcheese_zh::ChineseStemmer;
/// use stringcheese_lang::Stemmer;
///
/// // Every input passes through unchanged.
/// assert_eq!(ChineseStemmer.stem("我"), "我");
/// assert_eq!(ChineseStemmer.stem("我们"), "我们");
/// assert_eq!(ChineseStemmer.stem("中国"), "中国");
/// assert_eq!(ChineseStemmer.stem("hello"), "hello");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ChineseStemmer;

impl ChineseStemmer {
    /// Return `word` unchanged.
    ///
    /// The stem of a Chinese word is the word itself — see the
    /// [module-level docs](self) for the rationale.
    #[must_use]
    pub fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        Cow::Borrowed(word)
    }
}

impl Stemmer for ChineseStemmer {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        Self::stem(self, word)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_on_han_input() {
        assert_eq!(ChineseStemmer.stem("我"), "我");
        assert_eq!(ChineseStemmer.stem("中国"), "中国");
        assert_eq!(ChineseStemmer.stem("我们"), "我们");
    }

    #[test]
    fn identity_on_latin_input() {
        assert_eq!(ChineseStemmer.stem("hello"), "hello");
    }

    #[test]
    fn identity_on_empty_input() {
        assert_eq!(ChineseStemmer.stem(""), "");
    }

    #[test]
    fn output_always_borrowed() {
        // The identity stemmer never allocates.
        for w in ["我", "中国", "我们", "hello", "", "北京上海"] {
            let out = ChineseStemmer.stem(w);
            assert!(
                matches!(out, Cow::Borrowed(_)),
                "expected Cow::Borrowed for {w:?}, got Cow::Owned"
            );
        }
    }

    #[test]
    fn idempotent() {
        for w in ["我们", "中国", "hello"] {
            let once = ChineseStemmer.stem(w).into_owned();
            let twice = ChineseStemmer.stem(&once).into_owned();
            assert_eq!(once, twice);
        }
    }

    #[test]
    fn output_never_longer_than_input() {
        for w in ["我", "中国", "我们", "hello", ""] {
            let out = ChineseStemmer.stem(w);
            assert_eq!(out.len(), w.len(), "identity stemmer must preserve len");
        }
    }
}
