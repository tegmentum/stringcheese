//! [`LightBengaliStemmer`] — a light Bengali suffix stripper.
//!
//! # Scope
//!
//! Bengali morphology is agglutinative — nouns inflect for number,
//! definiteness, and case via a stack of postpositional suffixes;
//! verbs conjugate for tense / aspect / mood / person / honorific
//! level. No rule-based light stemmer can cover this in full; a
//! proper Bengali stemmer needs a lexicon and a morphological
//! analyzer. The community references (Ramanathan & Rao 2003 for
//! the Indic template, Loponen & Järvelin 2010 for statistical
//! stemming) both agree that a lightweight, IR-oriented pass over a
//! curated suffix table is the pragmatic starter.
//!
//! There is **no canonical Snowball Bengali algorithm**. Snowball's
//! catalogue lists Bengali as "planned" but ships no `bengali.sbl`.
//! This module ships a **deliberately conservative** rule-based
//! stemmer covering plural markers and the most-common case
//! endings. It is intended to be a starter — a downstream
//! `stringcheese-bn-morph` crate would ship the full analyzer.
//!
//! # Suffix table
//!
//! One pass, longest-match-wins, over the following (all Bengali):
//!
//! | Suffix   | Chars | Class                                     |
//! |----------|-------|-------------------------------------------|
//! | `গুলি`   | 4     | plural marker (formal)                    |
//! | `গুলো`   | 4     | plural marker (colloquial)                |
//! | `দের`   | 3     | plural genitive (of the ...)               |
//! | `রা`    | 2     | plural nominative (animate)                |
//! | `রে`    | 2     | accusative / oblique                       |
//! | `কে`    | 2     | accusative / dative marker                 |
//! | `তে`    | 2     | locative marker (in / at)                  |
//! | `য়`    | 1     | y-with-nukta ending (precomposed U+09DF)   |
//! | `র`    | 1     | genitive marker (of)                       |
//!
//! # What is deliberately **not** in the suffix table
//!
//! * **Free-standing postpositions (`এর`, `থেকে`, `দিয়ে`).**
//!   These are almost always written as *separate* orthographic
//!   words in modern Bengali text; they're handled by the stopword
//!   list, not by the stemmer.
//! * **Verb personal / tense endings.** Bengali verbs conjugate
//!   for six persons across half a dozen tenses and moods; stripping
//!   verb morphology safely needs a lexicon (verb stems can be
//!   irregular). The high-frequency copula and auxiliary forms are
//!   in the stopword list.
//! * **Standalone matras `ি`, `ু`, `া`.** These occur *inside*
//!   most Bengali words as ordinary dependent vowel signs; a trailing
//!   bare matra as an inflectional suffix is either historical or
//!   vanishingly rare in surface Bengali. Including them would
//!   over-strip more often than it would help.
//!
//! Each suffix is stored as a **[`Vec<char>`]** — no raw byte offsets —
//! and matched at the *scalar-slice tail* of the input's `Vec<char>`
//! form. Bengali scalars are 3 UTF-8 bytes each, so byte-level
//! arithmetic would silently corrupt boundaries.
//!
//! # Over-stripping safeguard
//!
//! If a strip would leave fewer than 2 *characters* (scalars), the
//! stemmer discards the strip and returns the input unchanged. This
//! guards against pathological inputs like a two-scalar suffix
//! standing alone.
//!
//! # Deliberate limitations (documented, not bugs)
//!
//! - **Free postpositions written as separate words are not seen
//!   by this stemmer.** `বইয়ের উপর` (on the book) is two
//!   whitespace-separated tokens; only `বইয়ের` reaches the
//!   stemmer, and its `-র` genitive marker is what gets stripped.
//! - **No consonant-alternation reversal.** Bengali stems can
//!   surface with alternating final consonants (e.g. across
//!   sandhi boundaries); this stemmer strips the suffix only.
//! - **No prefix stripping.** Bengali nominal prefixes are rare
//!   (mostly Sanskrit loans); leaving them intact keeps the stem
//!   recognizable.
//!
//! # Contract
//!
//! - **Deterministic.** For any input, two calls to `stem(w)`
//!   return equal outputs.
//! - **Converges under repeated application.** Each successful
//!   strip shortens the stem by at least one scalar and the
//!   suffix table has finite entries, so iterated application
//!   terminates.
//! - **Non-lengthening.** The output is never longer than the input
//!   (all rules are strict deletions).
//! - **Preserves input on no-match.** Returns [`Cow::Borrowed`]
//!   when no rule fires.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use stringcheese_lang::Stemmer;

/// Over-stripping guard: refuse to strip if the resulting stem would
/// have fewer than this many *characters* (scalars).
const MIN_STEM_CHARS: usize = 2;

/// Suffix table, sorted longest-first. Every entry is a
/// scalar-sequence (`&'static [char]`) rather than a byte string — the
/// stemmer matches on `Vec<char>` tails, never raw bytes.
///
/// Longest-match-wins is enforced by ordering the table with the
/// four-character entries first, then three, two, and one.
const SUFFIXES: &[&[char]] = &[
    // Four-scalar plural markers.
    &['গ', 'ু', 'ল', 'ি'], // গুলি — plural (formal)
    &['গ', 'ু', 'ল', 'ো'], // গুলো — plural (colloquial)
    // Three-scalar plural genitive.
    &['দ', 'ে', 'র'], // দের — plural genitive
    // Two-scalar case / plural markers.
    &['র', 'া'],  // রা — animate plural nominative
    &['র', 'ে'], // রে — accusative / oblique
    &['ক', 'ে'], // কে — accusative / dative
    &['ত', 'ে'], // তে — locative (in / at)
    // Single-scalar markers.
    &['\u{09DF}'], // য় — y-with-nukta (precomposed U+09DF)
    &['র'],        // র — genitive marker
];

/// The light Bengali suffix stripper.
///
/// A zero-sized value; construct as [`LightBengaliStemmer`] and reuse
/// across threads and calls.
///
/// See the [module-level docs](self) for the rule set and the
/// contract.
///
/// # Example
///
/// ```
/// use stringcheese_bn::LightBengaliStemmer;
/// use stringcheese_lang::Stemmer;
///
/// // Strip the plural marker -গুলি.
/// assert_eq!(LightBengaliStemmer.stem("বইগুলি"), "বই");
/// // Bare noun — no suffix, no-op.
/// assert_eq!(LightBengaliStemmer.stem("বাড়ি"), "বাড়ি");
/// // Strip the plural genitive -দের.
/// assert_eq!(LightBengaliStemmer.stem("ছেলেদের"), "ছেলে");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct LightBengaliStemmer;

impl LightBengaliStemmer {
    /// Stems `word` per the light Bengali rule set.
    ///
    /// Returns the stem as a [`Cow`]. If no rule fires, the returned
    /// `Cow` borrows the input.
    #[must_use]
    pub fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        // Assemble a Vec<char> for scalar-indexed arithmetic. Every
        // Bengali letter is 3 bytes in UTF-8, so any code that mixed
        // byte offsets with character-boundary logic would corrupt
        // boundaries — we work strictly in char space.
        let chars: Vec<char> = word.chars().collect();

        if chars.len() < MIN_STEM_CHARS {
            return Cow::Borrowed(word);
        }

        for &suffix in SUFFIXES {
            if suffix.len() > chars.len() {
                continue;
            }
            let stem_len = chars.len() - suffix.len();
            if chars[stem_len..] != *suffix {
                continue;
            }
            if stem_len < MIN_STEM_CHARS {
                // Over-strip guard fired — keep looking for a shorter
                // suffix (there may not be one, in which case the
                // input passes through unchanged).
                continue;
            }
            // Match! Rebuild the stem as a `String`.
            let stem: String = chars[..stem_len].iter().collect();
            return Cow::Owned(stem);
        }

        Cow::Borrowed(word)
    }
}

impl Stemmer for LightBengaliStemmer {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        Self::stem(self, word)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &str) -> String {
        LightBengaliStemmer.stem(w).into_owned()
    }

    // -------------------------------------------------------------
    // Plural markers.
    // -------------------------------------------------------------

    #[test]
    fn strips_plural_marker_guli() {
        // বইগুলি → বই (books, formal plural).
        assert_eq!(s("বইগুলি"), "বই");
    }

    #[test]
    fn strips_plural_marker_gulo() {
        // বইগুলো → বই (books, colloquial plural).
        assert_eq!(s("বইগুলো"), "বই");
    }

    #[test]
    fn strips_plural_genitive_der() {
        // ছেলেদের → ছেলে (of the boys).
        assert_eq!(s("ছেলেদের"), "ছেলে");
    }

    #[test]
    fn strips_animate_plural_ra() {
        // ছেলেরা → ছেলে (the boys — animate plural).
        assert_eq!(s("ছেলেরা"), "ছেলে");
    }

    // -------------------------------------------------------------
    // Case markers.
    // -------------------------------------------------------------

    #[test]
    fn strips_accusative_ke() {
        // ছেলেকে → ছেলে (to the boy).
        assert_eq!(s("ছেলেকে"), "ছেলে");
    }

    #[test]
    fn strips_locative_te() {
        // ঘরে would strip -রে, but let's use a non-collision case.
        // বাড়িতে → বাড়ি (at home) — strips -তে.
        assert_eq!(s("বাড়িতে"), "বাড়ি");
    }

    #[test]
    fn strips_genitive_r() {
        // ছেলের → ছেলে (of the boy).
        assert_eq!(s("ছেলের"), "ছেলে");
    }

    // -------------------------------------------------------------
    // Longest-match wins.
    // -------------------------------------------------------------

    #[test]
    fn longest_match_wins_der_over_r() {
        // ছেলেদের — দের beats bare র.
        assert_eq!(s("ছেলেদের"), "ছেলে");
    }

    #[test]
    fn longest_match_wins_guli_over_shorter() {
        // বইগুলি — গুলি beats bare -ি/-লি if those were in the table.
        assert_eq!(s("বইগুলি"), "বই");
    }

    // -------------------------------------------------------------
    // Contract: identity, idempotent, guarded.
    // -------------------------------------------------------------

    #[test]
    fn identity_on_no_match() {
        assert_eq!(s("বাংলা"), "বাংলা");
        assert_eq!(s(""), "");
        assert_eq!(s("hello"), "hello");
    }

    #[test]
    fn idempotent_second_pass_is_noop() {
        for w in [
            "বইগুলি",
            "বইগুলো",
            "ছেলেদের",
            "ছেলেরা",
            "ছেলেকে",
            "ছেলের",
            "বাড়িতে",
        ] {
            let once = LightBengaliStemmer.stem(w).into_owned();
            let twice = LightBengaliStemmer.stem(&once).into_owned();
            assert_eq!(once, twice, "stem not idempotent on {w:?}");
        }
    }

    #[test]
    fn over_strip_guard_refuses_short_stems() {
        // "কে" alone — 2 chars, stripping কে leaves 0 → guarded.
        assert_eq!(s("কে"), "কে");
        // "র" alone — 1 char, length short-circuit.
        assert_eq!(s("র"), "র");
    }

    #[test]
    fn output_never_longer_than_input() {
        for w in ["বাংলা", "বইগুলি", "ছেলেদের", "hello", "বাড়িতে"]
        {
            let out = LightBengaliStemmer.stem(w);
            assert!(out.len() <= w.len(), "stem grew on {w:?}: {out:?}");
        }
    }

    #[test]
    fn borrowed_when_no_match_owned_when_matched() {
        let borrowed = LightBengaliStemmer.stem("বাংলা");
        assert!(matches!(borrowed, Cow::Borrowed(_)));
        let owned = LightBengaliStemmer.stem("বইগুলি");
        assert!(matches!(owned, Cow::Owned(_)));
    }
}
