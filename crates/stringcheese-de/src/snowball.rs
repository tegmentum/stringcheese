//! The Snowball German stemmer.
//!
//! # Origin
//!
//! Martin Porter and Richard Boulton's Snowball stemmer for German is
//! the standard suffix-stripping algorithm used in German IR work. The
//! algorithm is documented in Snowball's language algorithm archive:
//! <https://snowballstem.org/algorithms/german/stemmer.html>. This
//! module implements the algorithm as stated on that page, working
//! directly on Unicode `char` sequences so umlauts (`ä`, `ö`, `ü`) and
//! `ß` behave correctly without any encoding gymnastics.
//!
//! # Algorithm sketch
//!
//! Every word is preprocessed by:
//!
//! 1. Lowercasing (Unicode-aware).
//! 2. Replacing `ß` with `ss`.
//! 3. Putting `u` and `i` between two vowels into upper case (`U`, `I`),
//!    which makes them behave as consonants during region computation
//!    and suffix stripping.
//!
//! The regions **R1** and **R2** are then set up per the standard
//! Snowball convention (R1 = the region after the first non-vowel
//! following a vowel; R2 = the same, computed inside R1), with the
//! German-specific adjustment that R1 must start at position 3 or later
//! — the region before it contains at least 3 letters.
//!
//! Three suffix-stripping steps are then applied in order:
//!
//! * **Step 1** removes plural / genitive endings (`em`, `en`, `ern`,
//!   `er`, `es`, `e`) that lie inside R1, plus the terminal `s` when
//!   preceded by a valid s-ending letter (`b d f g h k l m n r t`).
//! * **Step 2** removes verb / adjective endings (`en`, `er`, `est`)
//!   inside R1, plus the terminal `st` when preceded by a valid
//!   st-ending letter (`b d f g h k l m n t`) with at least 3 letters
//!   before it.
//! * **Step 3** removes derivational suffixes (`end`, `ung`, `ig`, `ik`,
//!   `isch`, `lich`, `heit`, `keit`) inside R2, with a small cascade of
//!   secondary cleanups (an inner `ig` after `end`/`ung`; an inner
//!   `er`/`en` after `lich`/`heit`; an inner `lich`/`ig` after `keit`).
//!
//! Post-processing turns the marked `U` / `I` back into their lowercase
//! form and folds the umlauts into their base vowels (`ä → a`, `ö → o`,
//! `ü → u`).
//!
//! # Non-goals
//!
//! * **Compound-word splitting.** German famously builds long compound
//!   nouns (`Donaudampfschifffahrtsgesellschaft`). Splitting them
//!   requires a compound-noun dictionary and is not part of the
//!   Snowball algorithm; a caller who needs compound-aware stemming
//!   should split first and stem the parts.
//! * **Umlaut restoration.** The algorithm folds `ä → a`, `ö → o`,
//!   `ü → u` at the end. The stemmer does not attempt to preserve the
//!   original spelling of the stem — `Häuser` and `Haus` collapse to
//!   the same `haus` stem, which is the intended behaviour for IR.
//! * **Case preservation.** The algorithm operates on lowercase input;
//!   the returned stem is lowercase.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use stringcheese_lang::Stemmer;

/// The Snowball German stemmer.
///
/// A zero-sized unit value; construct as `SnowballDe` and reuse the
/// value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_de::SnowballDe;
/// use stringcheese_lang::Stemmer;
///
/// assert_eq!(SnowballDe.stem("Häuser"), "haus");
/// assert_eq!(SnowballDe.stem("haben"), "hab");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SnowballDe;

impl SnowballDe {
    /// Stems `word` per the Snowball German algorithm.
    ///
    /// Returns a lowercase stem, with umlauts folded and `ß` normalized
    /// to `ss`. If `word` was already in the algorithm's normal form and
    /// no rules fired, the returned [`Cow`] borrows the input.
    #[must_use]
    pub fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        if word.is_empty() {
            return Cow::Borrowed(word);
        }

        // Preprocess: lowercase (Unicode-aware) and replace ß with ss.
        let lowered: String = word.chars().flat_map(char::to_lowercase).collect();
        let normalized = if lowered.contains('ß') {
            lowered.replace('ß', "ss")
        } else {
            lowered
        };

        // Convert to a Vec<char> so we can index by scalar position —
        // German umlauts and the ß-expansion behave uniformly under a
        // char-level indexing scheme, and the Snowball algorithm is
        // authored in terms of "letters" (scalars), not bytes.
        let mut chars: Vec<char> = normalized.chars().collect();

        // Words shorter than 3 chars are never modified by the German
        // stemmer (R1 adjustment already implies no rule can fire).
        if chars.len() < 3 {
            return finalize(word, &normalized, chars);
        }

        // Mark u/i between vowels as consonants (U/I).
        mark_ui(&mut chars);

        // Set up R1 and R2 per the Snowball convention, with the German
        // adjustment (R1 has at least 3 letters before it).
        let (r1, r2) = regions(&chars);

        // Apply the three suffix-stripping steps in order.
        step1(&mut chars, r1);
        step2(&mut chars, r1);
        step3(&mut chars, r1, r2);

        // Post-process: unmark U/I, fold umlauts.
        for c in &mut chars {
            *c = match *c {
                'I' => 'i',
                'ä' => 'a',
                'ö' => 'o',
                'U' | 'ü' => 'u',
                c => c,
            };
        }

        finalize(word, &normalized, chars)
    }
}

impl Stemmer for SnowballDe {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        SnowballDe::stem(self, word)
    }
}

/// Assemble the final `Cow`. If the collapsed stem equals the original
/// input byte-for-byte, we can hand back the borrow.
fn finalize<'s>(original: &'s str, _normalized: &str, chars: Vec<char>) -> Cow<'s, str> {
    let stem: String = chars.into_iter().collect();
    if stem == original {
        Cow::Borrowed(original)
    } else {
        Cow::Owned(stem)
    }
}

// ---------------------------------------------------------------------------
// Preprocessing: mark u/i between two vowels as consonants (U/I).
// ---------------------------------------------------------------------------

fn mark_ui(chars: &mut [char]) {
    let n = chars.len();
    if n < 3 {
        return;
    }
    for i in 1..n - 1 {
        let c = chars[i];
        if (c == 'u' || c == 'i') && is_vowel(chars[i - 1]) && is_vowel(chars[i + 1]) {
            chars[i] = if c == 'u' { 'U' } else { 'I' };
        }
    }
}

/// German vowels for the Snowball algorithm. Note that `U` and `I` (the
/// marked forms produced by [`mark_ui`]) are intentionally *not* vowels
/// — that is the whole point of the marking pass.
#[inline]
fn is_vowel(c: char) -> bool {
    matches!(c, 'a' | 'e' | 'i' | 'o' | 'u' | 'y' | 'ä' | 'ö' | 'ü')
}

/// Letters that can validly precede a stripped terminal `s` (Step 1b).
#[inline]
fn is_s_ending(c: char) -> bool {
    matches!(
        c,
        'b' | 'd' | 'f' | 'g' | 'h' | 'k' | 'l' | 'm' | 'n' | 'r' | 't'
    )
}

/// Letters that can validly precede a stripped terminal `st` (Step 2b).
/// Same as [`is_s_ending`] minus `r`.
#[inline]
fn is_st_ending(c: char) -> bool {
    matches!(c, 'b' | 'd' | 'f' | 'g' | 'h' | 'k' | 'l' | 'm' | 'n' | 't')
}

// ---------------------------------------------------------------------------
// Region computation: R1 and R2 per the standard Snowball convention,
// with the German adjustment.
// ---------------------------------------------------------------------------

fn regions(chars: &[char]) -> (usize, usize) {
    let n = chars.len();

    // R1: skip any initial run of consonants, then a run of vowels,
    // and the position one past the following non-vowel.
    let r1 = region_start_after(chars, 0);
    // R2: same, starting from R1.
    let r2 = region_start_after(chars, r1);

    // German adjustment: R1 must start at position 3 or later.
    let r1 = r1.max(3.min(n));

    (r1, r2)
}

/// Standard Snowball region start: skip [start..) forward past a
/// vowel-run then past the following consonant, and return the index
/// one past that consonant. If no vowel/consonant pair is found, return
/// the word length.
fn region_start_after(chars: &[char], start: usize) -> usize {
    let n = chars.len();
    let mut i = start;
    // Scan to the first vowel.
    while i < n && !is_vowel(chars[i]) {
        i += 1;
    }
    // Scan past the vowel run.
    while i < n && is_vowel(chars[i]) {
        i += 1;
    }
    // Position immediately after the non-vowel that follows.
    if i < n { i + 1 } else { n }
}

// ---------------------------------------------------------------------------
// Step 1: remove plural / genitive endings.
//
//   (a) em, en, ern, er, es, e   → delete if in R1
//   (b) s (after a valid s-ending) → delete if in R1
// ---------------------------------------------------------------------------

fn step1(chars: &mut Vec<char>, r1: usize) {
    // Sort suffixes longest-first so the first ending_with match wins.
    const GROUP_A: &[&[char]] = &[
        &['e', 'r', 'n'],
        &['e', 'm'],
        &['e', 'n'],
        &['e', 'r'],
        &['e', 's'],
        &['e'],
    ];

    for suf in GROUP_A {
        if ends_with(chars, suf) {
            let start = chars.len() - suf.len();
            if start >= r1 {
                chars.truncate(start);
            }
            return;
        }
    }

    // (b) 's' preceded by a valid s-ending. Only meaningful when the
    // penultimate letter exists and is a valid s-ending.
    if ends_with(chars, &['s']) && chars.len() >= 2 {
        let prev = chars[chars.len() - 2];
        if is_s_ending(prev) {
            let start = chars.len() - 1;
            if start >= r1 {
                chars.truncate(start);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Step 2: remove verb / adjective endings.
//
//   (a) en, er, est           → delete if in R1
//   (b) st (after a valid st-ending, itself preceded by at least 3
//       letters) → delete if in R1
// ---------------------------------------------------------------------------

fn step2(chars: &mut Vec<char>, r1: usize) {
    const GROUP_A: &[&[char]] = &[&['e', 's', 't'], &['e', 'n'], &['e', 'r']];

    for suf in GROUP_A {
        if ends_with(chars, suf) {
            let start = chars.len() - suf.len();
            if start >= r1 {
                chars.truncate(start);
            }
            return;
        }
    }

    // (b) 'st' after a valid st-ending, itself preceded by at least 3
    // letters. Position of the st-ending letter is chars.len() - 3;
    // "at least 3 letters before it" means chars.len() - 3 >= 3, i.e.
    // chars.len() >= 6.
    if chars.len() >= 6 && ends_with(chars, &['s', 't']) {
        let ending_pos = chars.len() - 3;
        if is_st_ending(chars[ending_pos]) {
            let start = chars.len() - 2;
            if start >= r1 {
                chars.truncate(start);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Step 3: remove derivational suffixes.
// ---------------------------------------------------------------------------

fn step3(chars: &mut Vec<char>, r1: usize, r2: usize) {
    // Longest-first search across every step-3 suffix.
    const CANDIDATES: &[&[char]] = &[
        &['i', 's', 'c', 'h'],
        &['l', 'i', 'c', 'h'],
        &['h', 'e', 'i', 't'],
        &['k', 'e', 'i', 't'],
        &['e', 'n', 'd'],
        &['u', 'n', 'g'],
        &['i', 'g'],
        &['i', 'k'],
    ];

    let mut matched: Option<&[char]> = None;
    for suf in CANDIDATES {
        if ends_with(chars, suf) {
            matched = Some(suf);
            break;
        }
    }
    let Some(suf) = matched else { return };
    let start = chars.len() - suf.len();

    // Dispatch on which suffix matched. Pattern-matching directly on
    // `[char]` slice literals would need `const_slice_from_raw_parts`
    // gymnastics on stable, so a small helper equality check is
    // clearer here.
    if (is_slice_eq(suf, &['e', 'n', 'd']) || is_slice_eq(suf, &['u', 'n', 'g'])) && start >= r2 {
        chars.truncate(start);
        // Follow-up: strip an inner `ig`.
        if ends_with(chars, &['i', 'g']) {
            let ig_start = chars.len() - 2;
            if ig_start >= r2 && (ig_start == 0 || chars[ig_start - 1] != 'e') {
                chars.truncate(ig_start);
            }
        }
    } else if (is_slice_eq(suf, &['i', 'g'])
        || is_slice_eq(suf, &['i', 'k'])
        || is_slice_eq(suf, &['i', 's', 'c', 'h']))
        && start >= r2
        && (start == 0 || chars[start - 1] != 'e')
    {
        chars.truncate(start);
    } else if (is_slice_eq(suf, &['l', 'i', 'c', 'h']) || is_slice_eq(suf, &['h', 'e', 'i', 't']))
        && start >= r2
    {
        chars.truncate(start);
        // Follow-up: strip a trailing `er` or `en` (longest match)
        // if it's in R1.
        for tail in [&['e', 'r'][..], &['e', 'n'][..]] {
            if ends_with(chars, tail) {
                let t_start = chars.len() - tail.len();
                if t_start >= r1 {
                    chars.truncate(t_start);
                }
                break;
            }
        }
    } else if is_slice_eq(suf, &['k', 'e', 'i', 't']) && start >= r2 {
        chars.truncate(start);
        // Follow-up: strip a trailing `lich` (longest) or `ig` if in R2.
        for tail in [&['l', 'i', 'c', 'h'][..], &['i', 'g'][..]] {
            if ends_with(chars, tail) {
                let t_start = chars.len() - tail.len();
                if t_start >= r2 {
                    chars.truncate(t_start);
                }
                break;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Small helpers.
// ---------------------------------------------------------------------------

#[inline]
fn ends_with(chars: &[char], suffix: &[char]) -> bool {
    chars.len() >= suffix.len() && &chars[chars.len() - suffix.len()..] == suffix
}

#[inline]
fn is_slice_eq(a: &[char], b: &[char]) -> bool {
    a == b
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &str) -> String {
        SnowballDe.stem(w).into_owned()
    }

    #[test]
    fn empty_and_short_words_are_unchanged() {
        assert_eq!(s(""), "");
        assert_eq!(s("a"), "a");
        assert_eq!(s("in"), "in");
    }

    #[test]
    fn lowercases_and_normalizes_ss() {
        // ß → ss preprocessing.
        assert_eq!(s("Straße"), s("strasse"));
        assert_eq!(s("STRASSE"), s("strasse"));
    }

    #[test]
    fn umlauts_are_folded() {
        // Post-processing folds ä ö ü.
        assert_eq!(s("Häuser"), "haus");
    }

    #[test]
    fn plural_stripping() {
        // Step 1(a): -en, -er, -es, -e in R1.
        assert_eq!(s("Kinder"), "kind");
        assert_eq!(s("Tage"), "tag");
    }

    #[test]
    fn verb_stripping() {
        // Common infinitives -en.
        assert_eq!(s("haben"), "hab");
        assert_eq!(s("geben"), "geb");
    }

    #[test]
    fn derivational_ung_and_ig_cascade() {
        // Step 3: -ung deletes and the leftover -ig also deletes.
        // "Beleidigung" → delete "ung" → "beleidig" → delete "ig" → "beleid".
        assert_eq!(s("Beleidigung"), "beleid");
    }

    #[test]
    fn heit_cascade() {
        // Step 3: -heit deletes in R2; no leftover -er/-en in this case.
        assert_eq!(s("Gesundheit"), "gesund");
        // NB: "Freiheit" is famously *not* stripped by Snowball — the
        // adjacent `ei` vowel cluster pushes R2 past the -heit suffix
        // (R1 = 5 covers `ei h`; R2 = 8 covers only the final `t`), so
        // the suffix is not in R2 and step 3 doesn't fire. This is the
        // reference algorithm's behaviour; we assert it explicitly.
        assert_eq!(s("Freiheit"), "freiheit");
    }

    #[test]
    fn stem_is_lowercase_for_ascii_input() {
        for w in ["Kinder", "Haben", "GEBEN", "TAG"] {
            let out = s(w);
            for ch in out.chars() {
                assert!(
                    !ch.is_ascii_uppercase(),
                    "stem {out:?} of {w:?} contains ASCII uppercase"
                );
            }
        }
    }

    #[test]
    fn measure_regions_for_sample_words() {
        // "häuser" → h ä u s e r (6 chars).
        // vowel-run then consonant: ä (1), u (2 still vowel), s (3 consonant).
        // R1 starts one past s → 4.
        let chars: Vec<char> = "häuser".chars().collect();
        let (r1, _) = regions(&chars);
        assert_eq!(r1, 4);
    }

    #[test]
    fn s_ending_check_prevents_over_stripping() {
        // 's' after a vowel is not stripped by step 1(b). "haus" ends
        // in 's' preceded by 'u' (vowel), so 's' is NOT a valid
        // s-ending → no strip in step 1(b).
        assert_eq!(s("haus"), "haus");
    }
}
