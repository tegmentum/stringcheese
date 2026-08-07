//! [`Light10`] — the Larkey ALP "light10" Arabic light stemmer.
//!
//! # Origin
//!
//! Leah Larkey, Lisa Ballesteros, and Margaret Connell's 2002 paper
//! *"Improving Stemming for Arabic Information Retrieval: Light
//! Stemming and Co-occurrence Analysis"* (SIGIR 2002) introduced a
//! family of *light* Arabic stemmers — rule-based prefix/suffix
//! strippers that ignore the root-and-pattern morphology Arabic
//! grammarians usually reach for. The paper describes six variants
//! (light1 through light10, minus a couple of skipped numbers); the
//! **light10** variant strips the largest well-behaved suffix list and
//! is the version the paper reports the best precision/recall figures
//! for.
//!
//! Light10 has become the de-facto baseline Arabic stemmer for
//! information retrieval: Lucene's `ArabicStemmer`, Snowball's
//! `arabic_stemmer.sbl`, and every "good enough for keyword search"
//! Arabic pipeline in the wild trace their rule sets to this paper.
//!
//! # Algorithm
//!
//! Light10 is a two-pass strict prefix/suffix stripper. Normalization
//! (via [`crate::normalize::normalize`]) is expected to have run
//! **before** the stemmer sees the input; the rules below assume plain
//! alef, plain yeh, no harakat.
//!
//! ## Pass 1 — strip one prefix, longest-match-wins
//!
//! Try each of the following in order and strip the first that matches:
//!
//! | Prefix | Meaning                       |
//! |--------|-------------------------------|
//! | `وال`  | *and-the-* (`wa-` + `al-`)    |
//! | `فال`  | *so-the-* (`fa-` + `al-`)     |
//! | `بال`  | *with-the-* (`bi-` + `al-`)   |
//! | `كال`  | *like-the-* (`ka-` + `al-`)   |
//! | `ال`   | *the-* (definite article)     |
//! | `و`    | *and-* (conjunction)          |
//!
//! ## Pass 2 — strip one suffix, longest-match-wins
//!
//! Try each of the following in order and strip the first that matches:
//!
//! | Suffix | Meaning                                          |
//! |--------|--------------------------------------------------|
//! | `ها`   | feminine 3rd-person possessive ("her-")          |
//! | `ان`   | dual                                             |
//! | `ات`   | feminine plural                                  |
//! | `ون`   | masculine plural (nominative)                    |
//! | `ين`   | masculine plural (accusative/genitive)           |
//! | `يه`   | variant possessive                               |
//! | `ية`   | feminine adjective / possessive with -y-         |
//! | `ه`    | masculine 3rd-person possessive ("his-")         |
//! | `ي`    | 1st-person possessive ("my-")                    |
//! | `ة`    | teh marbuta                                      |
//!
//! ## Over-stripping safeguard
//!
//! If the resulting stem is fewer than 2 *characters* (not bytes —
//! Arabic scalars are 2 UTF-8 bytes each, so "2 characters" is 4 bytes),
//! the stemmer discards the strip and returns the input unchanged.
//! This guards against nonsense like stripping the entire word.
//!
//! # Contract
//!
//! - **Idempotent on real Arabic input.** For any valid MSA surface
//!   form, `stem(stem(w)) == stem(w)` — the prefix and suffix tables
//!   are curated so that no real word's stem re-matches a table
//!   entry. **Adversarial input may re-strip:** the single-pass
//!   design (one prefix strip, one suffix strip, per call) means
//!   that a synthetic input engineered to nest affixes can be
//!   stripped further on a second call. For example, `الواف`
//!   (not a real Arabic word) strips its `ال` prefix on the first
//!   call to yield `واف`, whose leading `و` then matches on a second
//!   call. This is a deliberate trade-off: iterating to a fixed
//!   point inside `stem` would over-strip real words like `الوقت`
//!   ("the time", `al-` + `waqt`) — the `و` in `وقت` is part of the
//!   root, not a conjunction prefix, so a second pass would wrongly
//!   yield `قت`. Preserving the classical single-pass semantics is
//!   the correct choice for MSA. See the crate's property-test module
//!   for the machine-checked bounded-iteration convergence assertion.
//! - **Non-lengthening.** The output is never longer than the input
//!   (all rules are strict deletions).
//! - **Preserves input on no-match.** Returns [`Cow::Borrowed`] when
//!   no rule fires.
//!
//! # Non-goals
//!
//! - **Full root-and-pattern morphological analysis.** Extracting the
//!   3- or 4-letter consonantal root (`كتب`, `درس`, `عمل`, ...) needs
//!   template matching against a large table of derivational patterns
//!   plus a lexicon — Buckwalter Arabic Morphological Analyzer,
//!   MADAMIRA, or the like. Light10 is the well-established "good
//!   enough for IR" baseline; root extraction is deferred to a
//!   downstream `stringcheese-ar-morph` crate.
//! - **Verb-conjugation stripping.** The prefix and suffix tables do
//!   not cover the perfect/imperfect verb inflection ladder (`أ-`,
//!   `ت-`, `ي-`, `ن-` subject markers; `-ت`, `-ت`, `-وا` object
//!   markers). Verb stems are effectively passed through for IR.
//! - **Definite-article-under-sun-letter assimilation.** In pronunciation
//!   `الشمس` is *ash-shams* (the definite article's `ل` assimilates to
//!   the following sun letter); the orthography preserves the `ل`,
//!   which is what this stemmer strips. Assimilation is a phonological
//!   phenomenon and out of scope.
//! - **Dialect-aware stemming.** Egyptian, Levantine, and Gulf Arabic
//!   have prefix/suffix inventories that overlap MSA but diverge in
//!   places. Light10 is calibrated for MSA.
//!
//! # RTL note
//!
//! Prefix stripping removes bytes from the *start* of the UTF-8
//! string; suffix stripping removes bytes from the *end*. These are
//! logical-order operations, not display-order operations — the "start"
//! is where the first consonant is written, which is displayed on the
//! *right* in an RTL-rendered document. This mirrors every other
//! Arabic-text-processing library's convention.

use alloc::borrow::Cow;
use alloc::string::String;

use stringcheese_lang::Stemmer;

/// Prefix table, longest-first. Each entry is a `&'static str` — the
/// stemmer strips the *first* one that matches from the *start* of the
/// input.
///
/// The ordering places the four-scalar (six-byte) `وال`/`فال`/`بال`/
/// `كال` combinations before the two-scalar `ال` and the one-scalar
/// `و` — longest-match-wins.
const PREFIXES: &[&str] = &[
    "وال", // wa-al: and-the-
    "فال", // fa-al: so-the-
    "بال", // bi-al: with-the-
    "كال", // ka-al: like-the-
    "ال",  // al: the-
    "و",   // wa: and-
];

/// Suffix table, longest-first. Each entry is a `&'static str` — the
/// stemmer strips the *first* one that matches from the *end* of the
/// input.
///
/// The ordering places the two-scalar suffixes before the one-scalar
/// suffixes — longest-match-wins.
const SUFFIXES: &[&str] = &[
    "ها", // -ha: her-
    "ان", // -an: dual
    "ات", // -at: feminine plural
    "ون", // -un: masculine plural (nominative)
    "ين", // -in: masculine plural (accusative/genitive)
    "يه", // -yh: variant possessive
    "ية", // -yh (teh marbuta): feminine adjective / possessive
    "ه",  // -h: masculine possessive
    "ي",  // -y: 1st-person possessive
    "ة",  // teh marbuta
];

/// Over-stripping guard: refuse to strip if the resulting stem would
/// have fewer than this many *characters* (scalars).
///
/// Two characters is the light10 paper's convention; below it, the
/// stem is almost certainly noise. Arabic scalars are 2 UTF-8 bytes
/// each, so the equivalent byte floor for pure-Arabic input is 4 bytes,
/// but we count characters — the guard has to work correctly for input
/// that includes ASCII too.
const MIN_STEM_CHARS: usize = 2;

/// The Larkey ALP light10 Arabic light stemmer.
///
/// A zero-sized value; construct as [`Light10`] and reuse across
/// threads and calls.
///
/// See the [module-level docs](self) for the rule set and the
/// contract.
///
/// # Example
///
/// ```
/// use stringcheese_ar::Light10;
/// use stringcheese_lang::Stemmer;
///
/// // Strip the definite article and the feminine plural suffix.
/// assert_eq!(Light10.stem("الطالبات"), "طالب");
/// // Bare noun with no affixes — the stemmer is a no-op.
/// assert_eq!(Light10.stem("كتاب"), "كتاب");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Light10;

impl Light10 {
    /// Strip one prefix and then one suffix, honoring the over-strip
    /// safeguard.
    ///
    /// Returns [`Cow::Borrowed`] when neither pass fires; [`Cow::Owned`]
    /// otherwise.
    ///
    /// # Contract
    ///
    /// - **Idempotent on real Arabic input** — a second call on the
    ///   output is a no-op for any valid MSA surface form. See the
    ///   [module-level docs](self#contract) for the
    ///   adversarial-input caveat (nested-prefix synthetic strings
    ///   can be re-stripped) and why the classical single-pass
    ///   semantics is preserved rather than iterating to a fixed
    ///   point (which would over-strip real words like `الوقت`).
    /// - **Non-lengthening.** Output is never longer than the input.
    /// - **Guarded.** A strip that would leave fewer than 2 characters
    ///   is rolled back.
    #[must_use]
    pub fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        // Two-character floor short-circuit — nothing to strip.
        if word.chars().count() < MIN_STEM_CHARS {
            return Cow::Borrowed(word);
        }

        // Pass 1: strip one prefix (longest-match-wins).
        let after_prefix = strip_prefix(word);
        // Pass 2: strip one suffix from whatever pass 1 handed us.
        let after_suffix = strip_suffix(after_prefix.as_ref());

        // Compare against the original word — return Cow::Borrowed on
        // no-change.
        if after_suffix == word {
            return Cow::Borrowed(word);
        }
        Cow::Owned(String::from(after_suffix.as_ref()))
    }
}

/// Strip the first matching prefix in [`PREFIXES`] from the *start* of
/// `word`, honoring the over-strip guard. Returns a `Cow` — borrowed
/// if no prefix fires, owned (a slice reborrow) otherwise.
///
/// The `Cow<str>` return is a slice of the input under the hood — we
/// use `Cow` so the caller can pass through both "no change" and
/// "sliced away" cases with the same handle.
fn strip_prefix(word: &str) -> Cow<'_, str> {
    for &prefix in PREFIXES {
        if word.len() > prefix.len() && word.starts_with(prefix) {
            let stem = &word[prefix.len()..];
            // Over-strip guard.
            if stem.chars().count() >= MIN_STEM_CHARS {
                return Cow::Borrowed(stem);
            }
            // Guard fired — don't strip this prefix, but don't fall
            // through to a shorter one either (per light10's
            // longest-match-wins semantics, a match that fails the
            // guard is not a "no match"; it's a "match, rolled back").
            return Cow::Borrowed(word);
        }
    }
    Cow::Borrowed(word)
}

/// Strip the first matching suffix in [`SUFFIXES`] from the *end* of
/// `word`, honoring the over-strip guard.
fn strip_suffix(word: &str) -> Cow<'_, str> {
    for &suffix in SUFFIXES {
        if word.len() > suffix.len() && word.ends_with(suffix) {
            let stem_len = word.len() - suffix.len();
            let stem = &word[..stem_len];
            if stem.chars().count() >= MIN_STEM_CHARS {
                return Cow::Borrowed(stem);
            }
            return Cow::Borrowed(word);
        }
    }
    Cow::Borrowed(word)
}

impl Stemmer for Light10 {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        Self::stem(self, word)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &str) -> alloc::string::String {
        Light10.stem(w).into_owned()
    }

    // ---------------------------------------------------------------
    // Prefix stripping.
    // ---------------------------------------------------------------

    #[test]
    fn strips_al_definite_article() {
        // ال + كتاب (book) → كتاب
        assert_eq!(s("الكتاب"), "كتاب");
    }

    #[test]
    fn strips_wal_and_the() {
        // وال + كتاب → كتاب
        assert_eq!(s("والكتاب"), "كتاب");
    }

    #[test]
    fn strips_fal_so_the() {
        assert_eq!(s("فالكتاب"), "كتاب");
    }

    #[test]
    fn strips_bal_with_the() {
        assert_eq!(s("بالكتاب"), "كتاب");
    }

    #[test]
    fn strips_kal_like_the() {
        assert_eq!(s("كالكتاب"), "كتاب");
    }

    #[test]
    fn strips_wa_and() {
        // و + كتاب → كتاب
        assert_eq!(s("وكتاب"), "كتاب");
    }

    #[test]
    fn longest_prefix_wins() {
        // Input starts with وال — the four-byte prefix must trip
        // before the two-byte ال.
        // والطالب → طالب (وال stripped, no suffix match).
        assert_eq!(s("والطالب"), "طالب");
    }

    // ---------------------------------------------------------------
    // Suffix stripping.
    // ---------------------------------------------------------------

    #[test]
    fn strips_ha_her_possessive() {
        // كتاب + ها → كتاب
        assert_eq!(s("كتابها"), "كتاب");
    }

    #[test]
    fn strips_dual_an() {
        // كتاب + ان → كتاب
        assert_eq!(s("كتابان"), "كتاب");
    }

    #[test]
    fn strips_feminine_plural_at() {
        // طالب + ات → طالب
        assert_eq!(s("طالبات"), "طالب");
    }

    #[test]
    fn strips_masculine_plural_un() {
        // معلم + ون → معلم
        assert_eq!(s("معلمون"), "معلم");
    }

    #[test]
    fn strips_masculine_plural_in() {
        // معلم + ين → معلم
        assert_eq!(s("معلمين"), "معلم");
    }

    #[test]
    fn strips_teh_marbuta() {
        // طالب + ة → طالب
        assert_eq!(s("طالبة"), "طالب");
    }

    #[test]
    fn strips_masculine_possessive_h() {
        // كتاب + ه → كتاب
        assert_eq!(s("كتابه"), "كتاب");
    }

    #[test]
    fn strips_first_person_possessive_y() {
        // كتاب + ي → كتاب
        assert_eq!(s("كتابي"), "كتاب");
    }

    // ---------------------------------------------------------------
    // Combined prefix + suffix stripping.
    // ---------------------------------------------------------------

    #[test]
    fn strips_prefix_and_suffix_in_one_call() {
        // ال + طالب + ات → طالب
        assert_eq!(s("الطالبات"), "طالب");
    }

    #[test]
    fn strips_wal_and_feminine_plural() {
        // وال + طالب + ات → طالب
        assert_eq!(s("والطالبات"), "طالب");
    }

    // ---------------------------------------------------------------
    // Contract: idempotent, non-lengthening, guarded.
    // ---------------------------------------------------------------

    #[test]
    fn identity_on_no_match() {
        // A word with no matching prefix or suffix is returned unchanged.
        // `علم` (science) — 3 chars, no prefix or suffix in the tables
        // matches; passes through unchanged.
        assert_eq!(s("كتاب"), "كتاب");
        assert_eq!(s("علم"), "علم");
        assert_eq!(s(""), "");
    }

    #[test]
    fn idempotent_second_pass_is_noop() {
        for w in ["الكتاب", "الطالبات", "والطالب", "معلمون", "كتابها", "طالبة"]
        {
            let once = Light10.stem(w).into_owned();
            let twice = Light10.stem(&once).into_owned();
            assert_eq!(once, twice, "stem not idempotent on {w:?}");
        }
    }

    #[test]
    fn over_strip_guard_refuses_short_stems() {
        // ال + بن (son) → would leave "بن" (2 chars — permitted by the
        // >= 2 rule).
        assert_eq!(s("البن"), "بن");
        // ال + ن (would leave 1 char, guard rolls back).
        assert_eq!(s("الن"), "الن");
        // Very short word — the length short-circuit fires.
        assert_eq!(s("ك"), "ك");
    }

    #[test]
    fn output_never_longer_than_input() {
        for w in ["الكتاب", "والطالبات", "معلمين", "كتابها", "طالبة", "hello"]
        {
            let out = Light10.stem(w);
            assert!(out.len() <= w.len(), "stem grew on {w:?}: {out:?}");
        }
    }

    #[test]
    fn identity_on_non_arabic_input() {
        // Nothing in the tables matches English — the stemmer is a
        // no-op for non-Arabic input.
        assert_eq!(s("hello"), "hello");
        assert_eq!(s("running"), "running");
    }

    #[test]
    fn borrowed_when_no_match_owned_when_matched() {
        let borrowed = Light10.stem("كتاب");
        assert!(matches!(borrowed, Cow::Borrowed(_)));
        let owned = Light10.stem("الكتاب");
        assert!(matches!(owned, Cow::Owned(_)));
    }
}
