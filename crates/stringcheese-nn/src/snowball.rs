//! The Snowball Norwegian stemmer, applied to Nynorsk input.
//!
//! # Origin
//!
//! Martin Porter's Snowball Norwegian algorithm, documented at
//! <https://snowballstem.org/algorithms/norwegian/stemmer.html>, is
//! the reference stemmer used across essentially every Norwegian IR
//! pipeline. The upstream algorithm is a **single** stemmer that
//! covers both Bokmål and Nynorsk — the two standards share
//! substantial nominal / adjectival / verbal morphology, and the
//! suffix system the stemmer operates on (`-en` / `-et` / `-ene` /
//! `-heter` / `-ede` / `-ande` / `-ende` / …) is common to both. This
//! module ports the algorithm to Rust, faithfully to the published
//! spec.
//!
//! The Bokmål sibling ([`stringcheese-no::NorwegianSnowball`](
//! https://docs.rs/stringcheese-no)) ports the same algorithm; the
//! two implementations are algorithmically identical by design. The
//! separate type here means the pack namespaces are consistent
//! (`NynorskSnowball` in `stringcheese-nn`, `NorwegianSnowball` in
//! `stringcheese-no`) and the reference tests can be co-located with
//! each pack.
//!
//! # Nynorsk-specific coverage notes
//!
//! * **`-a` feminine definite** (`jenta` "the girl") is used across
//!   both standards and is captured by the Group A `-a` strip. Nynorsk
//!   uses `-a` more consistently than Bokmål (which permits both
//!   `-en` and `-a` for feminine definites), so this strip fires
//!   proportionally more often on Nynorsk text.
//! * **`-ande` present-participle / gerund** (`krevande` "demanding")
//!   is the Nynorsk canonical form (Bokmål prefers `-ende`); both
//!   sit in the Group A delete list.
//! * **`-ast` superlative** (`høgast` "highest") is the Nynorsk
//!   canonical form (Bokmål: `-est`); `-ast` is in Group A.
//! * **Irregular verbs.** A handful of Nynorsk verbs (`gå` / `gjekk`
//!   / `gått`, `sjå` / `såg` / `sett`) are irregular in ways the
//!   suffix stripper cannot capture; reducing those requires a
//!   lexicon and is out of scope.
//!
//! # Algorithm sketch
//!
//! 1. **Lowercase (Unicode-aware).** Fold input to lowercase so the
//!    suffix tables (all lowercase) match.
//! 2. **R1 region.** Compute `R1` per the standard Snowball convention
//!    (`R1` = the region after the first non-vowel following a vowel,
//!    or the end-of-word null region if none exists), then adjust so
//!    `R1` never starts before char index 3 — the region before it
//!    must contain at least three letters.
//! 3. **Step 1 — main suffix (longest match in R1).**
//!    * *Group A* — delete: `a`, `e`, `ede`, `ande`, `ende`, `ane`,
//!      `ene`, `hetene`, `en`, `heten`, `ar`, `er`, `heter`, `as`,
//!      `es`, `edes`, `endes`, `enes`, `hetenes`, `ens`, `hetens`,
//!      `ers`, `ets`, `et`, `het`, `ast`.
//!    * *Group B* — `s`: delete when preceded by a **valid s-ending**.
//!    * *Group C* — `erte` / `ert`: replace with `er`.
//!
//!    A **valid s-ending** is one of `b c d f g h j l m n o p r t v y
//!    z`, or `k` provided the character before it is a non-vowel. Note
//!    the deliberate inclusion of `o` and `y` in the s-ending set —
//!    this matches the upstream Snowball spec verbatim; `y` is
//!    ambiguously vocalic / consonantal in Norwegian and both roles
//!    can precede a plural `-s`.
//! 4. **Step 2 — consonant-pair.** If the word ends in `dt` or `vt`
//!    in R1, delete the final `t`.
//! 5. **Step 3 — derivational suffix (longest match in R1).** Delete
//!    if found: `leg`, `eleg`, `ig`, `eig`, `lig`, `elig`, `els`,
//!    `lov`, `elov`, `slov`, `hetslov`.
//!
//! # Vowel set
//!
//! Norwegian vowels per the Snowball spec: `a e i o u y æ å ø` — the
//! three Norwegian-specific letters are all vowels.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use stringcheese_lang::Stemmer;

/// The Snowball Norwegian stemmer, packaged for Nynorsk.
///
/// A zero-sized unit value; construct as [`NynorskSnowball`] and reuse
/// the value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_nn::NynorskSnowball;
/// use stringcheese_lang::Stemmer;
///
/// assert_eq!(NynorskSnowball.stem("bilane"), "bil");
/// assert_eq!(NynorskSnowball.stem("husets"), "hus");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct NynorskSnowball;

impl NynorskSnowball {
    /// Stems `word` per the Snowball Norwegian algorithm.
    ///
    /// Returns the stem as a [`Cow`]. If the algorithm makes no change
    /// to a lowercase input, the returned `Cow` borrows the input.
    #[must_use]
    pub fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        if word.chars().count() <= 1 {
            return Cow::Borrowed(word);
        }

        // 1. Lowercase (Unicode-aware).
        let mut chars: Vec<char> = word.chars().flat_map(char::to_lowercase).collect();

        // 2. R1 region, adjusted so R1 ≥ 3.
        let r1 = compute_r1_adjusted(&chars);

        // 3. Steps 1..=3.
        step_1_main_suffix(&mut chars, r1);
        step_2_consonant_pair(&mut chars, r1);
        step_3_other_suffix(&mut chars, r1);

        // 4. Emit.
        let out: String = chars.into_iter().collect();
        if out == word {
            Cow::Borrowed(word)
        } else {
            Cow::Owned(out)
        }
    }
}

impl Stemmer for NynorskSnowball {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        NynorskSnowball::stem(self, word)
    }
}

// ---------------------------------------------------------------------------
// Vowel classification.
// ---------------------------------------------------------------------------

/// Norwegian vowels per the Snowball spec: `a e i o u y æ å ø`.
#[inline]
fn is_vowel(c: char) -> bool {
    matches!(c, 'a' | 'e' | 'i' | 'o' | 'u' | 'y' | 'æ' | 'å' | 'ø')
}

/// Valid s-ending characters, per the Snowball spec:
/// `b c d f g h j l m n o p r t v y z`. `k` is a valid s-ending when
/// the character before it is a non-vowel — that case is handled
/// separately by [`is_valid_s_ending_at`].
#[inline]
fn is_plain_s_ending(c: char) -> bool {
    matches!(
        c,
        'b' | 'c'
            | 'd'
            | 'f'
            | 'g'
            | 'h'
            | 'j'
            | 'l'
            | 'm'
            | 'n'
            | 'o'
            | 'p'
            | 'r'
            | 't'
            | 'v'
            | 'y'
            | 'z'
    )
}

/// True if `chars[idx]` is a valid s-ending in context: either a plain
/// s-ending letter, or a `k` preceded by a non-vowel.
fn is_valid_s_ending_at(chars: &[char], idx: usize) -> bool {
    let c = chars[idx];
    if is_plain_s_ending(c) {
        return true;
    }
    if c == 'k' {
        if idx == 0 {
            // `k` at start with nothing before it: the spec's
            // "non-vowel before k" cannot be satisfied. Treat as
            // invalid — matches the Snowball reference behaviour
            // where the `gopast v` prelude never lets the s-strip
            // fire on such short inputs anyway.
            return false;
        }
        return !is_vowel(chars[idx - 1]);
    }
    false
}

// ---------------------------------------------------------------------------
// Region R1 (adjusted so it never starts before char 3).
// ---------------------------------------------------------------------------

fn compute_r1_adjusted(chars: &[char]) -> usize {
    let r1 = compute_r1(chars);
    // The region before R1 must contain at least 3 characters — i.e.,
    // R1 itself starts at char index >= 3.
    r1.max(3.min(chars.len()))
}

fn compute_r1(chars: &[char]) -> usize {
    let n = chars.len();
    let mut i = 0;
    while i < n && !is_vowel(chars[i]) {
        i += 1;
    }
    while i < n && is_vowel(chars[i]) {
        i += 1;
    }
    if i < n { i + 1 } else { n }
}

// ---------------------------------------------------------------------------
// Suffix helpers.
// ---------------------------------------------------------------------------

fn ends_with(chars: &[char], suffix: &[char]) -> bool {
    if suffix.len() > chars.len() {
        return false;
    }
    let start = chars.len() - suffix.len();
    chars[start..] == *suffix
}

#[inline]
fn suffix_in(chars: &[char], suf_len: usize, region_start: usize) -> bool {
    chars.len().saturating_sub(suf_len) >= region_start
}

fn longest_suffix<'a>(chars: &[char], candidates: &[&'a [char]]) -> Option<&'a [char]> {
    let mut best: Option<&[char]> = None;
    for &s in candidates {
        if ends_with(chars, s) && best.is_none_or(|b| s.len() > b.len()) {
            best = Some(s);
        }
    }
    best
}

// ---------------------------------------------------------------------------
// Step 1: main suffix.
// ---------------------------------------------------------------------------

// Group A — plain-delete suffixes.
const S1A: &[&[char]] = &[
    &['h', 'e', 't', 'e', 'n', 'e', 's'], // hetenes (7)
    &['h', 'e', 't', 'e', 'n', 'e'],      // hetene  (6)
    &['h', 'e', 't', 'e', 'n', 's'],      // hetens  (6)
    &['h', 'e', 't', 'e', 'n'],           // heten   (5)
    &['h', 'e', 't', 'e', 'r'],           // heter   (5)
    &['a', 'n', 'd', 'e'],                // ande    (4)
    &['e', 'n', 'd', 'e'],                // ende    (4)
    &['e', 'd', 'e', 's'],                // edes    (4)
    &['e', 'n', 'd', 'e', 's'],           // endes   (5)
    &['e', 'n', 'e', 's'],                // enes    (4)
    &['h', 'e', 't'],                     // het     (3)
    &['a', 's', 't'],                     // ast     (3)
    &['e', 'd', 'e'],                     // ede     (3)
    &['a', 'n', 'e'],                     // ane     (3)
    &['e', 'n', 'e'],                     // ene     (3)
    &['e', 'n', 's'],                     // ens     (3)
    &['e', 'r', 's'],                     // ers     (3)
    &['e', 't', 's'],                     // ets     (3)
    &['e', 'n'],                          // en      (2)
    &['a', 'r'],                          // ar      (2)
    &['e', 'r'],                          // er      (2)
    &['a', 's'],                          // as      (2)
    &['e', 's'],                          // es      (2)
    &['e', 't'],                          // et      (2)
    &['a'],                               // a       (1)
    &['e'],                               // e       (1)
];

// Group B — bare `s` (special rule).
const S1B_S: &[char] = &['s'];

// Group C — `erte` / `ert` → `er`.
const S1C_ERTE: &[char] = &['e', 'r', 't', 'e'];
const S1C_ERT: &[char] = &['e', 'r', 't'];

fn step_1_main_suffix(chars: &mut Vec<char>, r1: usize) {
    // Assemble the union of the three groups. `longest_suffix` picks
    // the longest match — that's the "longest among the following"
    // rule the spec calls out.
    let mut all: Vec<&[char]> = Vec::with_capacity(S1A.len() + 3);
    all.extend_from_slice(S1A);
    all.push(S1B_S);
    all.push(S1C_ERTE);
    all.push(S1C_ERT);
    let Some(s) = longest_suffix(chars, &all) else {
        return;
    };
    let sl = s.len();
    if !suffix_in(chars, sl, r1) {
        return;
    }
    let stem_len = chars.len() - sl;

    // Group C — erte / ert → er.
    if s == S1C_ERTE || s == S1C_ERT {
        chars.truncate(stem_len);
        chars.push('e');
        chars.push('r');
        return;
    }

    // Group B — s — delete iff preceded by a valid s-ending.
    if s == S1B_S {
        if stem_len == 0 {
            return;
        }
        if !is_valid_s_ending_at(chars, stem_len - 1) {
            return;
        }
        chars.truncate(stem_len);
        return;
    }

    // Group A — plain delete.
    chars.truncate(stem_len);
}

// ---------------------------------------------------------------------------
// Step 2: consonant-pair `-dt` / `-vt` → delete final `t`.
// ---------------------------------------------------------------------------

fn step_2_consonant_pair(chars: &mut Vec<char>, r1: usize) {
    let dt: &[char] = &['d', 't'];
    let vt: &[char] = &['v', 't'];
    if (ends_with(chars, dt) || ends_with(chars, vt)) && suffix_in(chars, 2, r1) {
        chars.pop(); // drop the trailing `t`
    }
}

// ---------------------------------------------------------------------------
// Step 3: derivational suffix.
// ---------------------------------------------------------------------------

const S3: &[&[char]] = &[
    &['h', 'e', 't', 's', 'l', 'o', 'v'], // hetslov (7)
    &['e', 'l', 'e', 'g'],                // eleg    (4)
    &['e', 'l', 'i', 'g'],                // elig    (4)
    &['e', 'l', 'o', 'v'],                // elov    (4)
    &['s', 'l', 'o', 'v'],                // slov    (4)
    &['l', 'e', 'g'],                     // leg     (3)
    &['e', 'i', 'g'],                     // eig     (3)
    &['l', 'i', 'g'],                     // lig     (3)
    &['e', 'l', 's'],                     // els     (3)
    &['l', 'o', 'v'],                     // lov     (3)
    &['i', 'g'],                          // ig      (2)
];

fn step_3_other_suffix(chars: &mut Vec<char>, r1: usize) {
    let Some(s) = longest_suffix(chars, S3) else {
        return;
    };
    let sl = s.len();
    if !suffix_in(chars, sl, r1) {
        return;
    }
    let stem_len = chars.len() - sl;
    chars.truncate(stem_len);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &str) -> String {
        NynorskSnowball.stem(w).into_owned()
    }

    #[test]
    fn short_words_are_unchanged() {
        assert_eq!(s(""), "");
        assert_eq!(s("a"), "a");
        assert_eq!(s("og"), "og");
        assert_eq!(s("i"), "i");
    }

    #[test]
    fn step1_group_a_plain_delete_nynorsk_a_plural() {
        // Nynorsk canonical plural definite `-ane` (Bokmål prefers
        // `-ene`, both are in Group A). `bilane` (the cars) → `bil`.
        //   bilane = b i l a n e (6 chars). R1: b non-v, i v, l non-v
        //     at 2. R1 = 3. `ane` at pos 3. 3 >= 3. Delete → `bil`.
        assert_eq!(s("bilane"), "bil");
    }

    #[test]
    fn step1_ene_plural() {
        // `guttene` (the boys) — shared form; `-ene` deletes → `gutt`.
        assert_eq!(s("guttene"), "gutt");
    }

    #[test]
    fn step1_et_neuter_definite() {
        // `huset` (the house) → `-et` deletes → `hus`.
        assert_eq!(s("huset"), "hus");
    }

    #[test]
    fn step1_heter_replace() {
        // `sannheter` (truths) — `-heter` (5) deletes → `sann`.
        assert_eq!(s("sannheter"), "sann");
    }

    #[test]
    fn step1_s_plural_after_consonant_k() {
        // `parks` — s preceded by `k` preceded by non-vowel `r`.
        // Valid s-ending. Delete → `park`.
        assert_eq!(s("parks"), "park");
    }

    #[test]
    fn step1_erte_ert_replace_with_er() {
        // Imagined forms — verify the rewrite fires when in R1.
        assert_eq!(s("hoppert"), "hopper");
        assert_eq!(s("hopperte"), "hopper");
    }

    #[test]
    fn step1_ast_superlative_nynorsk() {
        // Nynorsk canonical superlative `-ast`. `høgast` (highest) →
        //   h ø g a s t (6). R1: h non-v, ø v, g non-v at 2. R1 = 3.
        //   `ast` at pos 3. 3 >= 3. Delete → `høg`.
        assert_eq!(s("høgast"), "høg");
    }

    #[test]
    fn step1_ande_gerund_nynorsk() {
        // Nynorsk canonical present-participle / gerund `-ande`.
        //   `krevande` = k r e v a n d e (8). R1: k non-v, r non-v,
        //     e v, v non-v at 3. R1 = 4. `ande` at pos 4. 4 >= 4.
        //     Delete → `krev`.
        assert_eq!(s("krevande"), "krev");
    }

    #[test]
    fn step2_dt_pair_deletes_trailing_t() {
        // `verdt` = v e r d t (5). R1 = 3. `dt` at pos 3. In R1.
        //   Delete `t` → `verd`.
        assert_eq!(s("verdt"), "verd");
        // `godt` (short) → not in R1 → unchanged.
        assert_eq!(s("godt"), "godt");
    }

    #[test]
    fn step3_ig_strip() {
        // `snerkig` — step 3 `ig` strips → `snerk`.
        assert_eq!(s("snerkig"), "snerk");
    }

    #[test]
    fn step3_lig_strip() {
        // `hyggelig` — step 3 `elig` strips → `hygg`.
        assert_eq!(s("hyggelig"), "hygg");
    }

    #[test]
    fn norwegian_letters_preserved() {
        assert_eq!(s("hår"), "hår");
        // `vera` (Nynorsk be-inf) — v e r a (4). R1: v non-v, e v, r
        //   non-v at 2. R1 = 3. Step 1 `a` at pos 3. 3 >= 3. Delete →
        //   `ver`.
        assert_eq!(s("vera"), "ver");
        // `øye` (eye) — 3 chars, no rules fire past R1 protection.
        assert_eq!(s("øye"), "øye");
    }

    #[test]
    fn stem_is_convergent_on_common_vocabulary() {
        for w in [
            "bil",
            "bilane",
            "hus",
            "huset",
            "gutt",
            "guttene",
            "sann",
            "sannhet",
            "sannheter",
            "sannhetens",
            "park",
            "parks",
            "verdt",
            "godt",
            "hyggelig",
            "vera",
            "vore",
            "hår",
            "øye",
            "krevande",
            "høgast",
        ] {
            let mut cur = NynorskSnowball.stem(w).into_owned();
            for _ in 0..5 {
                let next = NynorskSnowball.stem(&cur).into_owned();
                if next == cur {
                    break;
                }
                cur = next;
            }
            let n1 = NynorskSnowball.stem(&cur).into_owned();
            assert_eq!(cur, n1, "did not converge on {w:?}");
        }
    }

    #[test]
    fn regions_paper_example_snakke() {
        // "snakke" — s, n, a, k, k, e
        //   R1: s non-v, n non-v, a v, k non-v at 3. R1 = 4.
        //     Adjusted max(4, 3) = 4.
        let chars: Vec<char> = "snakke".chars().collect();
        assert_eq!(compute_r1_adjusted(&chars), 4);
    }

    #[test]
    fn s_ending_group_includes_o_and_y() {
        // Spec quirk: `o` and `y` are in the s-ending group.
        assert_eq!(s("fjellos"), "fjello");
        // `landas` → group-A `as` fires first: delete → `land`.
        assert_eq!(s("landas"), "land");
    }
}
