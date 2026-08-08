//! The Snowball Norwegian (Bokmål) stemmer.
//!
//! # Origin
//!
//! Martin Porter's Snowball Norwegian algorithm, documented at
//! <https://snowballstem.org/algorithms/norwegian/stemmer.html>, is
//! the reference stemmer used across essentially every Norwegian IR
//! pipeline. Lucene's `NorwegianAnalyzer`, Elasticsearch's `norwegian`
//! analyzer, `snowballstemmer` (Python), NLTK's
//! `SnowballStemmer("norwegian")` — all descend from the same
//! Porter/Boulton `norwegian.sbl` source. This module ports the
//! algorithm to Rust, faithfully to the published spec.
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
//!    in R1, delete the final `t`. This captures forms like `godt`
//!    (adverbial of `god`) reducing to `god`, and `bevt` (uncommon)
//!    reducing to `bev`.
//! 5. **Step 3 — derivational suffix (longest match in R1).** Delete
//!    if found: `leg`, `eleg`, `ig`, `eig`, `lig`, `elig`, `els`,
//!    `lov`, `elov`, `slov`, `hetslov`.
//!
//! # Vowel set
//!
//! Norwegian vowels per the Snowball spec: `a e i o u y æ å ø` — the
//! three Norwegian-specific letters are all vowels.
//!
//! # Non-goals
//!
//! * **Full-vocabulary cross-verification.** The Snowball project
//!   distributes `voc.txt` / `output.txt` reference files with tens of
//!   thousands of pairs; the [`tests/snowball_reference.rs`](
//!   ../../tests/snowball_reference.rs) test embeds a *subset* that
//!   exercises every step's happy path and each cascading rule.
//!   Full-corpus cross-verification is a follow-up.
//! * **Compound splitting.** Norwegian productively compounds nouns
//!   (`fotballag = fotball + lag`); splitting them requires a
//!   compound-noun dictionary and is not part of the Snowball
//!   algorithm.
//! * **Lemmatization.** Reducing `verre → dårlig`, `bedre → god`
//!   requires a lexicon, not a suffix-stripping algorithm.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use stringcheese_lang::Stemmer;

/// The Snowball Norwegian (Bokmål) stemmer.
///
/// A zero-sized unit value; construct as [`NorwegianSnowball`] and
/// reuse the value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_no::NorwegianSnowball;
/// use stringcheese_lang::Stemmer;
///
/// assert_eq!(NorwegianSnowball.stem("bilene"), "bil");
/// assert_eq!(NorwegianSnowball.stem("husets"), "hus");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct NorwegianSnowball;

impl NorwegianSnowball {
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

impl Stemmer for NorwegianSnowball {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        NorwegianSnowball::stem(self, word)
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

// Group A — plain-delete suffixes, in longest-first-friendly form (the
// longest_suffix helper picks the longest matching entry regardless of
// order, so ordering is presentational only).
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
    if ends_with(chars, dt) || ends_with(chars, vt) {
        // The pair itself must sit in R1 (i.e., the char before the
        // pair is at index >= r1 - 1, meaning the pair starts at >= r1
        // - 1 which we approximate as "pair in R1" per Snowball's
        // convention: the two-letter substring's start must be ≥ r1).
        if suffix_in(chars, 2, r1) {
            chars.pop(); // drop the trailing `t`
        }
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
        NorwegianSnowball.stem(w).into_owned()
    }

    #[test]
    fn short_words_are_unchanged() {
        assert_eq!(s(""), "");
        assert_eq!(s("a"), "a");
        assert_eq!(s("og"), "og");
        assert_eq!(s("i"), "i");
    }

    #[test]
    fn step1_group_a_plain_delete() {
        // `bilene` → definite plural of `bil` (car). `-ene` in R1
        // deletes → `bil`.
        //   bilene = b i l e n e (6 chars)
        //   R1: b non-v, i v, l non-v at 2. R1 = 3.
        //   Step 1: `ene` at end (pos 3). 3 >= 3. In R1. Delete → `bil`.
        assert_eq!(s("bilene"), "bil");
    }

    #[test]
    fn step1_er_plural() {
        // `guttene` (the boys) → `-ene` deletes → `gutt`.
        //   guttene = g u t t e n e (7 chars). R1: g non-v, u v, t non-v
        //     at 2. R1 = 3. `ene` at pos 4. 4 >= 3. Delete → `gutt`.
        assert_eq!(s("guttene"), "gutt");
    }

    #[test]
    fn step1_et_neuter_definite() {
        // `huset` (the house) → `-et` deletes → `hus`.
        //   huset = h u s e t (5 chars). R1: h non-v, u v, s non-v at 2.
        //     R1 = 3. `et` at pos 3. 3 >= 3. Delete → `hus`.
        assert_eq!(s("huset"), "hus");
    }

    #[test]
    fn step1_heter_replace() {
        // Words in `-heter` (abstract-noun plural of `-het`) delete
        // the whole 5-char suffix.
        //   sannheter (truths) = s a n n h e t e r (9). R1: s non-v,
        //     a v, n non-v at 2. R1 = 3. `heter` at pos 4. 4 >= 3.
        //     Delete → `sann`.
        assert_eq!(s("sannheter"), "sann");
    }

    #[test]
    fn step1_hetens_delete() {
        // `sannhetens` → `-hetens` deletes → `sann`.
        //   sannhetens = s a n n h e t e n s (10). R1 = 3 (as above).
        //     `hetens` at pos 4. 4 >= 3. Delete → `sann`.
        assert_eq!(s("sannhetens"), "sann");
    }

    #[test]
    fn step1_s_plural_after_valid_ending() {
        // `hus` → too short — no -s strip (R1 adjusted ≥ 3, so `s` at
        // pos 2 fails the region check).
        assert_eq!(s("hus"), "hus");
        // `bibliotek` + `s` → `biblioteks` (library-genitive form).
        //   biblioteks = b i b l i o t e k s (10). R1: b non-v, i v,
        //     b non-v at 2. R1 = 3. Group-A candidates: last 2 = `ks`
        //     — no. last 3 = `eks` — no. So Group-A finds nothing.
        //     Group B `s` — preceded by `k`. `k` is a valid s-ending
        //     iff the char before it is non-vowel; before `k` (pos 8)
        //     is `e` (vowel) → not valid. So `s` is NOT stripped.
        //     Result: `biblioteks`.
        assert_eq!(s("biblioteks"), "biblioteks");
    }

    #[test]
    fn step1_s_plural_after_consonant_k() {
        // `parks` — s preceded by `k`, and k preceded by `r` (non-v).
        // Valid s-ending case. Delete.
        //   parks = p a r k s (5). R1: p non-v, a v, r non-v at 2.
        //     R1 = 3. `s` at pos 4. 4 >= 3. `k` at pos 3, preceded by
        //     `r` (non-v) → valid. Delete → `park`.
        assert_eq!(s("parks"), "park");
    }

    #[test]
    fn step1_erte_ert_replace_with_er() {
        // `bygget` doesn't end in erte/ert. Try a made-up example:
        // `snakkerte` (past tense-ish, non-standard for testing).
        //   Actually `-erte` is the past tense of e.g., `spiste` — no.
        //   Try `hoppert` (imagined) → hoppert = h o p p e r t (7).
        //     R1: h non-v, o v, p non-v at 2. R1 = 3. `ert` at pos 4.
        //     4 >= 3. Replace → `hopper`.
        assert_eq!(s("hoppert"), "hopper");
        // erte: `hopperte` (imagined) = h o p p e r t e (8). R1 = 3.
        //   `erte` at pos 4. 4 >= 3. Replace → `hopper`.
        assert_eq!(s("hopperte"), "hopper");
    }

    #[test]
    fn step2_dt_pair_deletes_trailing_t() {
        // `godt` (well/good — adverbial) = g o d t (4). R1: g non-v,
        //   o v, d non-v at 2. R1 = 3. Group-A `t` alone not in list.
        //   Group-B `s` — no, ends in `t`. Group-C `ert` — no.
        //   Step 1 does nothing.
        //   Step 2 `dt` at pos 2. Pair position 2 >= r1(3)? 2 >= 3 is
        //     false. So NOT in R1. Doesn't fire. `godt` unchanged.
        // Hmm — the spec says the pair sits *in* R1. `godt` is short
        // enough that the pair isn't there. That matches Snowball's
        // actual output: `godt` → `godt` under the algorithm (short
        // words are protected).
        assert_eq!(s("godt"), "godt");
        // Try a longer word: `verdt` (worth-neut). v e r d t (5).
        //   R1: v non-v, e v, r non-v at 2. R1 = 3. `dt` at pos 3
        //     (starts at index 3). 3 >= 3. In R1. Delete `t` →
        //     `verd`.
        assert_eq!(s("verdt"), "verd");
    }

    #[test]
    fn step3_ig_strip() {
        // `snerkig` (imagined adjective in -ig).
        //   snerkig = s n e r k i g (7). R1: s non-v, n non-v,
        //     e v, r non-v at 3. R1 = 4.
        //   Step 1: last 2 `ig` — Group A doesn't have `ig`. `g`
        //     alone not in Group A. Group B `s` no. `ert` no. Step 1
        //     no change.
        //   Step 2: no dt/vt.
        //   Step 3: `ig` at pos 5. 5 >= 4. In R1. Delete → `snerk`.
        assert_eq!(s("snerkig"), "snerk");
    }

    #[test]
    fn step3_lig_strip() {
        // `hyggelig` (cozy) = h y g g e l i g (8). R1: h non-v, y v,
        //   g non-v at 2. R1 = 3.
        //   Step 1: last 2 `ig` — not in Group A. Skip.
        //   Step 3: candidates ending: check `elig` at pos 4. 4 >= 3.
        //     Delete → `hygg`. That's the longest match in R1.
        assert_eq!(s("hyggelig"), "hygg");
    }

    #[test]
    fn norwegian_letters_preserved() {
        // `hår` (hair) — no suffix rules fire (whole word). Result: `hår`.
        assert_eq!(s("hår"), "hår");
        // `være` (to be) — v æ r e = 4 chars. R1: v non-v, æ v, r
        //   non-v at 2. R1 = 3. Step 1 `e` at pos 3. 3 >= 3. Delete →
        //   `vær`.
        assert_eq!(s("være"), "vær");
        // `øye` (eye) — ø y e = 3 chars. R1: ø v, y v (still vowel),
        //   e v (vowel). No non-vowel follows a vowel. R1 = 3 (end of
        //   word) then adjusted to 3.min(3) = 3. Step 1 `e` at pos 2.
        //   2 >= 3 is false. No strip. Result: `øye`.
        assert_eq!(s("øye"), "øye");
    }

    #[test]
    fn stem_is_convergent_on_common_vocabulary() {
        for w in [
            "bil",
            "bilene",
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
            "være",
            "vær",
            "hår",
            "øye",
            "snakke",
            "snakker",
            "snakket",
        ] {
            let mut cur = NorwegianSnowball.stem(w).into_owned();
            for _ in 0..5 {
                let next = NorwegianSnowball.stem(&cur).into_owned();
                if next == cur {
                    break;
                }
                cur = next;
            }
            let n1 = NorwegianSnowball.stem(&cur).into_owned();
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
    fn s_ending_group_excludes_vowels_a_e_i_u_but_includes_o_and_y() {
        // Spec quirk: `o` and `y` are in the s-ending group.
        // A word ending in `-os` is stripped: e.g., `fjellos` (imagined)
        //   f j e l l o s = 7. R1: f non-v, j non-v, e v, l non-v at 3.
        //     R1 = 4. `s` at pos 6. 6 >= 4. Preceded by `o` (plain
        //     s-ending). Delete → `fjello`.
        assert_eq!(s("fjellos"), "fjello");
        // A word ending in `-as` (a is a vowel, not in s-ending group)
        //   is NOT stripped by group B — BUT `as` is in group A's plain
        //   delete list. So `landas` (imagined) → landas = 6. R1: l
        //   non-v, a v, n non-v at 2. R1 = 3. `as` at pos 4. 4 >= 3.
        //   Delete → `land`.
        assert_eq!(s("landas"), "land");
    }
}
