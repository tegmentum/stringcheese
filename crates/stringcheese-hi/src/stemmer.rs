//! [`LightHindiStemmer`] — a light Hindi suffix stripper.
//!
//! # Scope
//!
//! Hindi morphology is substantial: nouns inflect for gender / number
//! / case (direct vs. oblique); adjectives agree with their head noun;
//! verbs conjugate for tense / aspect / mood / number / gender /
//! honorific level and combine with a rich set of auxiliaries; and
//! postpositions attach to oblique-case nouns as *separate*
//! orthographic words (`कमरे के अंदर` — three words). No rule-based
//! light stemmer can cover this in full; a proper Hindi stemmer needs
//! a lexicon and a morphological analyzer.
//!
//! There is **no canonical Snowball Hindi algorithm**. Snowball's
//! catalogue lists Hindi as "planned" but ships no `hindi.sbl`. The
//! Ramanathan-Rao 2003 "light" stemmer is the community reference; it
//! strips ~65 common suffixes in longest-match-first order.
//!
//! This module ships a **deliberately conservative** subset of the
//! Ramanathan-Rao rules: gender / number markers, postpositions (when
//! fused rather than free-standing), and the most-common verb tense
//! endings. It is intended to be a starter — a downstream
//! `stringcheese-hi-morph` crate would ship the full analyzer.
//!
//! # Suffix table
//!
//! One pass, longest-match-wins, over the following (all Devanagari):
//!
//! | Suffix | Chars | Class                                      |
//! |--------|-------|--------------------------------------------|
//! | `ों`   | 2     | gen/num oblique plural marker (matra + ं)  |
//! | `ें`   | 2     | gen/num plural marker (matra + ं)          |
//! | `ता`   | 2     | habitual m. sg.                            |
//! | `ती`   | 2     | habitual f. sg.                            |
//! | `ते`   | 2     | habitual m. pl.                            |
//! | `या`   | 2     | perfective m. sg.                          |
//! | `ा`    | 1     | m. sg. direct                              |
//! | `ी`    | 1     | f. sg. direct                              |
//! | `े`    | 1     | m. pl. or oblique                          |
//!
//! # What is deliberately **not** in the suffix table
//!
//! * **Fused postpositions (`-का -की -के -ने -को -से -में -पर`).**
//!   Hindi postpositions are almost always written as *separate*
//!   orthographic words in modern text (`लड़के का` "of the boy",
//!   two tokens). The rare fused forms would collide with common
//!   nominal endings — stripping `का` from `लड़का` (boy, m. sg.)
//!   would wrongly produce `लड़` instead of `लड़क`, and stripping
//!   `के` from `बच्चे` (children, m. pl.) would wrongly leave
//!   `बच्` instead of `बच्च`. The safer default is to leave those
//!   entries out and let the tokenizer's whitespace split handle
//!   the free-standing postpositions.
//! * **Standalone matras `ि` and `ु`.** These occur *inside* many
//!   Hindi words as ordinary dependent vowel signs (`सिख` "Sikh",
//!   `गुरु` "guru"); a trailing `ि` or `ु` as an inflectional suffix
//!   is either historical or vanishingly rare in modern surface
//!   forms. Including them would over-strip more often than it
//!   would help.
//! * **Independent-vowel verb endings `ई`, `ए`.** These appear at
//!   morpheme boundaries only when the preceding syllable ends in
//!   a bare consonant, which happens rarely in surface Hindi
//!   orthography (surface `-ई` in perfectives is usually written
//!   as a matra `ी` after the last consonant). Leaving them out
//!   also guarantees the stemmer's idempotence contract — a stem
//!   that ends in a bare vowel scalar is not further reduced.
//!
//! Each suffix is stored as a **[`Vec<char>`]** — no raw byte offsets —
//! and matched at the *scalar-slice tail* of the input's `Vec<char>`
//! form. Devanagari scalars are 3 UTF-8 bytes each, so byte-level
//! arithmetic would silently corrupt boundaries.
//!
//! # Over-stripping safeguard
//!
//! If a strip would leave fewer than 2 *characters* (scalars), the
//! stemmer discards the strip and returns the input unchanged. This
//! guards against pathological inputs like a two-scalar postposition
//! standing alone.
//!
//! # Deliberate limitations (documented, not bugs)
//!
//! - **Postpositions written as separate words are not seen by this
//!   stemmer.** `कमरे के अंदर` is three whitespace-separated tokens;
//!   only `कमरे` reaches the stemmer, and its `-े` oblique marker is
//!   what gets stripped. The postposition entries above cover only
//!   the *fused* forms that some corpora attach directly.
//! - **No consonant-alternation reversal.** Hindi verb stems can
//!   surface with alternating final consonants (`खाना → खा` "to
//!   eat"); this stemmer strips the suffix only.
//! - **No prefix stripping.** Hindi nominal prefixes are rare and
//!   mostly Sanskrit / Arabic loans; leaving them intact keeps the
//!   stem recognizable.
//! - **Perfective forms may over-strip.** `किया` (did) has both a
//!   perfective `-या` ending *and* a hypothetical noun sense
//!   `किया → कि` — the stemmer strips greedily and may lose the noun
//!   root in edge cases. This is the price of a lexicon-free
//!   stemmer; downstream IR pipelines usually accept the trade-off.
//!
//! # Contract
//!
//! - **Deterministic.** For any input, two calls to `stem(w)` return
//!   equal outputs.
//! - **Converges under repeated application.** The single-pass API
//!   is not strictly idempotent — stripping a plural matra
//!   (`ों → ँ`) can occasionally leave a bare single-char matra
//!   that another pass would strip again — but the process always
//!   terminates because each successful strip shortens the stem by
//!   at least one scalar. See the crate's property-test module for
//!   the machine-checked bound.
//! - **Non-lengthening.** The output is never longer than the input
//!   (all rules are strict deletions).
//! - **Preserves input on no-match.** Returns [`Cow::Borrowed`] when
//!   no rule fires.

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
/// three-character entries first, then two-character, then
/// single-character.
const SUFFIXES: &[&[char]] = &[
    // Two-scalar gender/number oblique markers.
    &['ो', 'ं'], // ों — oblique plural (matra o + anusvara)
    &['े', 'ं'],  // ें — plural (matra e + anusvara)
    // Two-scalar verb tense markers.
    &['त', 'ा'], // ता — habitual m. sg.
    &['त', 'ी'], // ती — habitual f. sg.
    &['त', 'े'],  // ते — habitual m. pl.
    &['य', 'ा'], // या — perfective m. sg.
    // Single-scalar gender / number markers.
    &['ा'], // matra ā (m. sg.)
    &['ी'], // matra ī (f. sg.)
    &['े'],  // matra e (m. pl. / oblique)
];

/// The light Hindi suffix stripper.
///
/// A zero-sized value; construct as [`LightHindiStemmer`] and reuse
/// across threads and calls.
///
/// See the [module-level docs](self) for the rule set and the
/// contract.
///
/// # Example
///
/// ```
/// use stringcheese_hi::LightHindiStemmer;
/// use stringcheese_lang::Stemmer;
///
/// // Strip the habitual m. sg. suffix -ता.
/// assert_eq!(LightHindiStemmer.stem("जाता"), "जा");
/// // Bare noun — no suffix, no-op.
/// assert_eq!(LightHindiStemmer.stem("किताब"), "किताब");
/// // Oblique plural — strip -ों.
/// assert_eq!(LightHindiStemmer.stem("लड़कों"), "लड़क");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct LightHindiStemmer;

impl LightHindiStemmer {
    /// Stems `word` per the light Hindi rule set.
    ///
    /// Returns the stem as a [`Cow`]. If no rule fires, the returned
    /// `Cow` borrows the input.
    #[must_use]
    pub fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        // Assemble a Vec<char> for scalar-indexed arithmetic. Every
        // Devanagari letter is 3 bytes in UTF-8, so any code that
        // mixed byte offsets with character-boundary logic would
        // corrupt boundaries — we work strictly in char space.
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

impl Stemmer for LightHindiStemmer {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        Self::stem(self, word)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &str) -> String {
        LightHindiStemmer.stem(w).into_owned()
    }

    // -------------------------------------------------------------
    // Gender / number markers.
    // -------------------------------------------------------------

    #[test]
    fn strips_masculine_singular_marker() {
        // लड़का = ल + ड + ़ + क + ा  → strip ा → ल + ड + ़ + क.
        assert_eq!(s("लड़का"), "लड़क");
    }

    #[test]
    fn strips_feminine_singular_marker() {
        // लड़की → लड़क (strip ी).
        assert_eq!(s("लड़की"), "लड़क");
    }

    #[test]
    fn strips_masculine_plural_marker() {
        // लड़के → लड़क (strip े).
        assert_eq!(s("लड़के"), "लड़क");
    }

    #[test]
    fn strips_oblique_plural_marker() {
        // लड़कों → लड़क (strip ों, longest match beats bare े).
        assert_eq!(s("लड़कों"), "लड़क");
    }

    // -------------------------------------------------------------
    // Verb tense markers.
    // -------------------------------------------------------------

    #[test]
    fn strips_habitual_masculine_singular() {
        // जाता → जा (strip ता).
        assert_eq!(s("जाता"), "जा");
    }

    #[test]
    fn strips_habitual_feminine_singular() {
        // जाती → जा (strip ती).
        assert_eq!(s("जाती"), "जा");
    }

    #[test]
    fn strips_habitual_masculine_plural() {
        // जाते → जा (strip ते).
        assert_eq!(s("जाते"), "जा");
    }

    #[test]
    fn strips_perfective_masculine_singular() {
        // "आया" = आ + य + ा (3 independent-vowel scalars). Longest
        // matching suffix from the tail is या [य,ा]; the stem "आ"
        // would be 1 char and the guard rolls it back; the fallback
        // ा [ा] match then leaves "आय" (2 chars), which meets the
        // guard.
        assert_eq!(s("आया"), "आय");
        // "खाया" = ख + ा + य + ा (4 scalars). या matches, stem "खा"
        // (2 chars) meets the guard, no fallback needed.
        assert_eq!(s("खाया"), "खा");
    }

    // -------------------------------------------------------------
    // Longest-match wins.
    // -------------------------------------------------------------

    #[test]
    fn longest_match_wins_ons_over_bare_e() {
        // "बच्चों" (children, oblique) — ों beats bare े.
        assert_eq!(s("बच्चों"), "बच्च");
    }

    #[test]
    fn longest_match_wins_taan_over_bare_aa() {
        // "बच्चा" (child, m. sg. direct) — ा wins (there's no तै match).
        assert_eq!(s("बच्चा"), "बच्च");
    }

    // -------------------------------------------------------------
    // Contract: identity, idempotent, guarded.
    // -------------------------------------------------------------

    #[test]
    fn identity_on_no_match() {
        assert_eq!(s("किताब"), "किताब");
        assert_eq!(s(""), "");
        assert_eq!(s("hello"), "hello");
    }

    #[test]
    fn idempotent_second_pass_is_noop() {
        for w in [
            "लड़का",
            "लड़की",
            "लड़के",
            "लड़कों",
            "जाता",
            "जाती",
            "खाया",
            "बच्चों",
        ] {
            let once = LightHindiStemmer.stem(w).into_owned();
            let twice = LightHindiStemmer.stem(&once).into_owned();
            assert_eq!(once, twice, "stem not idempotent on {w:?}");
        }
    }

    #[test]
    fn over_strip_guard_refuses_short_stems() {
        // "का" alone — 2 chars, stripping का leaves 0 → guarded.
        assert_eq!(s("का"), "का");
        // "ा" alone — 1 char, length short-circuit.
        assert_eq!(s("ा"), "ा");
    }

    #[test]
    fn output_never_longer_than_input() {
        for w in ["लड़का", "जाता", "किताब", "hello", "खाया", "लड़कों"]
        {
            let out = LightHindiStemmer.stem(w);
            assert!(out.len() <= w.len(), "stem grew on {w:?}: {out:?}");
        }
    }

    #[test]
    fn borrowed_when_no_match_owned_when_matched() {
        let borrowed = LightHindiStemmer.stem("किताब");
        assert!(matches!(borrowed, Cow::Borrowed(_)));
        let owned = LightHindiStemmer.stem("जाता");
        assert!(matches!(owned, Cow::Owned(_)));
    }
}
