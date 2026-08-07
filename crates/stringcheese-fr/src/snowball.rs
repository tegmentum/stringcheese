//! The Snowball French stemmer.
//!
//! # Origin
//!
//! The Snowball project's French stemmer, originally due to Martin
//! Porter and documented at
//! <https://snowballstem.org/algorithms/french/stemmer.html>, is the
//! reference stemmer used across essentially every French IR pipeline
//! (Lucene's `FrenchLightStemmer` and `FrenchMinimalStemmer` are
//! deliberately weaker variants; Snowball French is the *reference*
//! implementation). This module ports the algorithm to Rust,
//! faithfully to the published spec.
//!
//! # Algorithm sketch
//!
//! 1. **Preprocess.** Lowercase; mark `u` after `q` and `u`/`i`/`y`
//!    between vowels as consonants by uppercasing them to `U`/`I`/`Y`
//!    (these become inert for the vowel classification).
//! 2. **Regions.** Compute `R1`, `R2`, and `RV` — three cut points
//!    the rules refer to. `R1` is after the first non-vowel following
//!    a vowel; `R2` is the same rule applied to the substring after
//!    `R1`; `RV` is a language-specific window (`par`/`col`/`tap` at
//!    word start start it at position 3; two initial vowels do the
//!    same; otherwise it's after the first vowel found at or after
//!    position 1).
//! 3. **Step 1 — standard suffix removal.** Long suffixes like
//!    `-ance`, `-iste`, `-ation`, `-logie`, `-usion`, `-ement`,
//!    `-eux`, `-aux`, `-euse`, `-issement`, `-amment`, `-emment`, and
//!    `-ment` — each with its own condition and each with several
//!    "if preceded by..." cascading rules.
//! 4. **Step 2a / 2b — verb suffix removal.** Two mutually
//!    conditional passes over inflected verb endings. Step 2a fires
//!    when step 1 did nothing or when step 1 removed a `-ment` /
//!    `-emment` / `-amment` suffix. Step 2b fires when step 2a made
//!    no change.
//! 5. **Step 3 — script cleanup.** Turn any remaining `Y` back into
//!    `i` and `ç` into `c`.
//! 6. **Step 4 — residual suffix cleanup.** Trailing `s`, `-ion`,
//!    `-ier`/`-ière`, `-e`, `-ë` — each with its own condition.
//! 7. **Step 5 — undouble.** Trailing `-enn`, `-onn`, `-ett`,
//!    `-ell`, `-eill` — drop the last letter.
//! 8. **Step 6 — un-accent.** Trailing `é` or `è` followed by at
//!    least one non-vowel is folded to `e`.
//!
//! At the end, any lingering marks (`U`, `I`, `Y` from step 1) are
//! lowered back to `u`, `i`, `y`.
//!
//! # Non-goals
//!
//! * **Lucene French-light / French-minimal.** Both are legitimate
//!   alternative French stemmers with narrower rule sets; the shipped
//!   pack picks the reference Snowball algorithm as its default. A
//!   caller who wants a lighter stem can implement their own
//!   `stringcheese_lang::Stemmer` and swap it in.
//! * **Full-vocabulary cross-verification.** The Snowball project
//!   distributes `voc.txt` / `output.txt` reference files with tens of
//!   thousands of pairs. The
//!   [`tests/snowball_reference.rs`](../../tests/snowball_reference.rs)
//!   test embeds a *subset* — enough to exercise every step's happy
//!   path and every documented cascading rule. Full-corpus
//!   cross-verification is a follow-up.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use stringcheese_lang::Stemmer;

/// The Snowball French stemmer.
///
/// A zero-sized unit value; construct as [`FrenchSnowball`] and reuse
/// the value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_fr::FrenchSnowball;
/// use stringcheese_lang::Stemmer;
///
/// assert_eq!(FrenchSnowball.stem("continue"), "continu");
/// assert_eq!(FrenchSnowball.stem("continuer"), "continu");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct FrenchSnowball;

impl FrenchSnowball {
    /// Stems `word` per the Snowball French algorithm.
    ///
    /// Returns the stem as a [`Cow`]. If the algorithm makes no change
    /// to a lowercase input, the returned `Cow` borrows the input.
    #[must_use]
    pub fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        // Words of length 0..=1 stem to themselves.
        if word.chars().count() <= 1 {
            return Cow::Borrowed(word);
        }

        // 1. Preprocess: lowercase and mark U/I/Y as consonants.
        let mut chars: Vec<char> = word.chars().flat_map(char::to_lowercase).collect();
        mark_consonants(&mut chars);

        // 2. Compute regions (char indices, not byte indices).
        let r1 = compute_r1(&chars);
        let r2 = compute_r2(&chars, r1);
        let rv = compute_rv(&chars);

        // 3. Step 1.
        let (chars_after_1, s1) = step_1(chars, r1, r2, rv);
        let mut chars = chars_after_1;

        // 4. Step 2a / 2b conditional dispatch.
        //
        // Per the Snowball spec: run step 2a iff step 1 did nothing OR
        // removed a ment-family suffix. If 2a runs and removes nothing,
        // fall through to 2b. If step 1 changed something that wasn't
        // ment-family, skip 2a and 2b entirely (proceed to step 3).
        if !s1.changed || s1.removed_ment_family {
            let (c, changed_2a) = step_2a(chars, rv);
            chars = c;
            if !changed_2a {
                chars = step_2b(chars, rv, r2);
            }
        }

        // 5. Step 3.
        step_3(&mut chars);

        // 6. Step 4.
        step_4(&mut chars, rv, r2);

        // 7. Step 5.
        step_5(&mut chars);

        // 8. Step 6.
        step_6(&mut chars);

        // 9. Un-mark: U/I/Y back to lowercase.
        for c in &mut chars {
            *c = match *c {
                'U' => 'u',
                'I' => 'i',
                'Y' => 'y',
                other => other,
            };
        }

        let out: String = chars.iter().collect();
        // Borrow-preservation: if the algorithm's output equals the
        // input verbatim (lowercase, no marks needed), keep the borrow.
        if out == word {
            Cow::Borrowed(word)
        } else {
            Cow::Owned(out)
        }
    }
}

impl Stemmer for FrenchSnowball {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        FrenchSnowball::stem(self, word)
    }
}

// ---------------------------------------------------------------------------
// Vowel classification & preprocessing.
// ---------------------------------------------------------------------------

/// French vowels (lowercase, base and accented forms). Uppercase `U`,
/// `I`, `Y` are *not* vowels — they're the consonant marks the
/// preprocessing step lays down.
#[inline]
fn is_vowel(c: char) -> bool {
    matches!(
        c,
        'a' | 'e'
            | 'i'
            | 'o'
            | 'u'
            | 'y'
            | 'â'
            | 'à'
            | 'ë'
            | 'é'
            | 'ê'
            | 'è'
            | 'ï'
            | 'î'
            | 'ô'
            | 'û'
            | 'ù'
    )
}

/// Mark `u` after `q`, and `u`/`i`/`y` between vowels, by uppercasing.
///
/// Also marks a leading `y` when the following character is a vowel
/// (`yeux`, `yaourt` — a `y` before a vowel is consonantal).
fn mark_consonants(chars: &mut [char]) {
    let n = chars.len();
    for i in 0..n {
        let prev = if i == 0 { None } else { Some(chars[i - 1]) };
        let next = if i + 1 < n { Some(chars[i + 1]) } else { None };
        match chars[i] {
            'u' if prev == Some('q') => chars[i] = 'U',
            'u' | 'i' => {
                if let (Some(p), Some(n_)) = (prev, next)
                    && is_vowel(p)
                    && is_vowel(n_)
                {
                    chars[i] = if chars[i] == 'u' { 'U' } else { 'I' };
                }
            }
            'y' => {
                // y is consonantal if flanked by a vowel on either side.
                let flanked_by_vowel = matches!(prev, Some(p) if is_vowel(p))
                    || matches!(next, Some(n_) if is_vowel(n_));
                if flanked_by_vowel {
                    chars[i] = 'Y';
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Regions R1, R2, RV — computed as char indices.
// ---------------------------------------------------------------------------

/// R1 = char index after the first non-vowel following a vowel
/// (or `chars.len()` if no such position exists).
fn compute_r1(chars: &[char]) -> usize {
    let n = chars.len();
    let mut i = 0;
    // Advance to the first vowel.
    while i < n && !is_vowel(chars[i]) {
        i += 1;
    }
    // Advance past the vowel run.
    while i < n && is_vowel(chars[i]) {
        i += 1;
    }
    // R1 starts here (one past the vowel-to-consonant transition), but
    // we still need to be past a consonant. Advance one step: the
    // canonical definition is "after the first non-vowel following a
    // vowel", so we're currently at the first non-vowel and R1 starts
    // *after* it.
    if i < n { i + 1 } else { n }
}

/// R2 = the same rule applied to `chars[R1..]`.
fn compute_r2(chars: &[char], r1: usize) -> usize {
    if r1 >= chars.len() {
        return chars.len();
    }
    let tail = &chars[r1..];
    r1 + compute_r1(tail)
}

/// RV = the French-specific region cut.
///
/// Per the Snowball French spec:
///
/// * If the word begins with `par`, `col`, or `tap` (ASCII, lowercase),
///   RV starts at position 3.
/// * Else if positions 0 and 1 are both vowels, RV starts *after the
///   first consonant found at position >= 2*.
/// * Else if position 1 is a consonant (position 0 anything), RV
///   starts *after the first vowel found at position >= 2*.
/// * Otherwise (position 0 consonant, position 1 vowel — the
///   "consonant-vowel case"), RV starts at position 3.
fn compute_rv(chars: &[char]) -> usize {
    let n = chars.len();
    // Special-case prefixes `par`, `col`, `tap`.
    if n >= 3 {
        let p = (chars[0], chars[1], chars[2]);
        if p == ('p', 'a', 'r') || p == ('c', 'o', 'l') || p == ('t', 'a', 'p') {
            return 3.min(n);
        }
    }
    // Word shorter than 3 letters: RV is the end of the word.
    if n < 2 {
        return n;
    }
    let c0 = chars[0];
    let c1 = chars[1];
    if is_vowel(c0) && is_vowel(c1) {
        // Both initial vowels: RV starts after the first consonant at
        // position >= 2.
        let mut i = 2;
        while i < n && is_vowel(chars[i]) {
            i += 1;
        }
        return (i + 1).min(n);
    }
    if !is_vowel(c1) {
        // Second letter is a consonant: RV starts after the first
        // vowel at position >= 2.
        let mut i = 2;
        while i < n && !is_vowel(chars[i]) {
            i += 1;
        }
        return (i + 1).min(n);
    }
    // Consonant-vowel case (c0 consonant, c1 vowel): RV at position 3.
    3.min(n)
}

// ---------------------------------------------------------------------------
// Suffix helpers.
// ---------------------------------------------------------------------------

/// Does `chars` end with the character-sequence `suffix`?
fn ends_with(chars: &[char], suffix: &[char]) -> bool {
    if suffix.len() > chars.len() {
        return false;
    }
    let start = chars.len() - suffix.len();
    chars[start..] == *suffix
}

/// Is the suffix of length `suf_len` at the end of `chars` in the
/// region beginning at char index `region_start`?
///
/// Equivalent to: `chars.len() - suf_len >= region_start`.
#[inline]
fn suffix_in(chars: &[char], suf_len: usize, region_start: usize) -> bool {
    chars.len().saturating_sub(suf_len) >= region_start
}

/// Find the longest suffix from `candidates` that `chars` ends with.
/// Returns the matched slice (or `None`).
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
// Step 1: standard suffix removal.
//
// The step returns a `Step1Outcome` recording whether anything changed
// and whether the change was one of the ment-family rules (which
// affects the step 2a / 2b dispatch).
// ---------------------------------------------------------------------------

/// Outcome of step 1.
struct Step1Outcome {
    changed: bool,
    removed_ment_family: bool,
}

#[allow(clippy::too_many_lines)] // step 1 is a fifteen-branch cascade
fn step_1(mut chars: Vec<char>, r1: usize, r2: usize, rv: usize) -> (Vec<char>, Step1Outcome) {
    // Assemble all step 1 suffix candidates as `&[&[char]]`.
    // Longest-match wins across the entire table.
    const CAND_A: &[&[char]] = &[
        &['a', 'n', 'c', 'e'],
        &['i', 'q', 'U', 'e'],
        &['i', 's', 'm', 'e'],
        &['a', 'b', 'l', 'e'],
        &['i', 's', 't', 'e'],
        &['e', 'u', 'x'],
        &['a', 'n', 'c', 'e', 's'],
        &['i', 'q', 'U', 'e', 's'],
        &['i', 's', 'm', 'e', 's'],
        &['a', 'b', 'l', 'e', 's'],
        &['i', 's', 't', 'e', 's'],
    ];
    const CAND_B: &[&[char]] = &[
        &['a', 't', 'r', 'i', 'c', 'e'],
        &['a', 't', 'e', 'u', 'r'],
        &['a', 't', 'i', 'o', 'n'],
        &['a', 't', 'r', 'i', 'c', 'e', 's'],
        &['a', 't', 'e', 'u', 'r', 's'],
        &['a', 't', 'i', 'o', 'n', 's'],
    ];
    const CAND_C: &[&[char]] = &[&['l', 'o', 'g', 'i', 'e'], &['l', 'o', 'g', 'i', 'e', 's']];
    const CAND_D: &[&[char]] = &[
        &['u', 's', 'i', 'o', 'n'],
        &['u', 't', 'i', 'o', 'n'],
        &['u', 's', 'i', 'o', 'n', 's'],
        &['u', 't', 'i', 'o', 'n', 's'],
    ];
    const CAND_E: &[&[char]] = &[&['e', 'n', 'c', 'e'], &['e', 'n', 'c', 'e', 's']];
    const CAND_F: &[&[char]] = &[&['e', 'm', 'e', 'n', 't'], &['e', 'm', 'e', 'n', 't', 's']];
    const CAND_G: &[&[char]] = &[&['i', 't', 'é'], &['i', 't', 'é', 's']];
    const CAND_H: &[&[char]] = &[
        &['i', 'f'],
        &['i', 'v', 'e'],
        &['i', 'f', 's'],
        &['i', 'v', 'e', 's'],
    ];
    const CAND_I: &[&[char]] = &[&['e', 'a', 'u', 'x']];
    const CAND_J: &[&[char]] = &[&['a', 'u', 'x']];
    const CAND_K: &[&[char]] = &[&['e', 'u', 's', 'e'], &['e', 'u', 's', 'e', 's']];
    const CAND_L: &[&[char]] = &[
        &['i', 's', 's', 'e', 'm', 'e', 'n', 't'],
        &['i', 's', 's', 'e', 'm', 'e', 'n', 't', 's'],
    ];
    const CAND_M: &[&[char]] = &[&['a', 'm', 'm', 'e', 'n', 't']];
    const CAND_N: &[&[char]] = &[&['e', 'm', 'm', 'e', 'n', 't']];
    const CAND_O: &[&[char]] = &[&['m', 'e', 'n', 't'], &['m', 'e', 'n', 't', 's']];

    // Find the single longest match across every group. Track which
    // group the winner belongs to so we can apply the group's action.
    #[derive(Copy, Clone)]
    enum Group {
        A,
        B,
        C,
        D,
        E,
        F,
        G,
        H,
        I,
        J,
        K,
        L,
        M,
        N,
        O,
    }
    let mut best_len: usize = 0;
    let mut best_group: Option<Group> = None;
    macro_rules! consider {
        ($cands:expr, $group:expr) => {
            if let Some(s) = longest_suffix(&chars, $cands)
                && s.len() > best_len
            {
                best_len = s.len();
                best_group = Some($group);
            }
        };
    }
    consider!(CAND_A, Group::A);
    consider!(CAND_B, Group::B);
    consider!(CAND_C, Group::C);
    consider!(CAND_D, Group::D);
    consider!(CAND_E, Group::E);
    consider!(CAND_F, Group::F);
    consider!(CAND_G, Group::G);
    consider!(CAND_H, Group::H);
    consider!(CAND_I, Group::I);
    consider!(CAND_J, Group::J);
    consider!(CAND_K, Group::K);
    consider!(CAND_L, Group::L);
    consider!(CAND_M, Group::M);
    consider!(CAND_N, Group::N);
    consider!(CAND_O, Group::O);

    let Some(group) = best_group else {
        return (
            chars,
            Step1Outcome {
                changed: false,
                removed_ment_family: false,
            },
        );
    };
    let stem_len = chars.len() - best_len;
    let mut changed = false;
    let mut removed_ment_family = false;

    match group {
        Group::A => {
            if suffix_in(&chars, best_len, r2) {
                chars.truncate(stem_len);
                changed = true;
            }
        }
        Group::B => {
            // Delete if in R2. If preceded by `ic`, delete if in R2,
            // else replace by `iqU`.
            if suffix_in(&chars, best_len, r2) {
                chars.truncate(stem_len);
                changed = true;
                if ends_with(&chars, &['i', 'c']) {
                    let ic_len = 2;
                    if suffix_in(&chars, ic_len, r2) {
                        chars.truncate(chars.len() - ic_len);
                    } else {
                        chars.truncate(chars.len() - ic_len);
                        chars.extend_from_slice(&['i', 'q', 'U']);
                    }
                }
            }
        }
        Group::C => {
            if suffix_in(&chars, best_len, r2) {
                chars.truncate(stem_len);
                chars.extend_from_slice(&['l', 'o', 'g']);
                changed = true;
            }
        }
        Group::D => {
            if suffix_in(&chars, best_len, r2) {
                chars.truncate(stem_len);
                chars.push('u');
                changed = true;
            }
        }
        Group::E => {
            if suffix_in(&chars, best_len, r2) {
                chars.truncate(stem_len);
                chars.extend_from_slice(&['e', 'n', 't']);
                changed = true;
            }
        }
        Group::F => {
            // -ement / -ements: delete if in RV.
            if suffix_in(&chars, best_len, rv) {
                chars.truncate(stem_len);
                changed = true;
                // Cascading precede-by rules.
                if ends_with(&chars, &['i', 'v']) && suffix_in(&chars, 2, r2) {
                    chars.truncate(chars.len() - 2);
                    if ends_with(&chars, &['a', 't']) && suffix_in(&chars, 2, r2) {
                        chars.truncate(chars.len() - 2);
                    }
                } else if ends_with(&chars, &['e', 'u', 's']) {
                    if suffix_in(&chars, 3, r2) {
                        chars.truncate(chars.len() - 3);
                    } else if suffix_in(&chars, 3, r1) {
                        chars.truncate(chars.len() - 3);
                        chars.extend_from_slice(&['e', 'u', 'x']);
                    }
                } else if (ends_with(&chars, &['a', 'b', 'l']) && suffix_in(&chars, 3, r2))
                    || (ends_with(&chars, &['i', 'q', 'U']) && suffix_in(&chars, 3, r2))
                {
                    chars.truncate(chars.len() - 3);
                } else if (ends_with(&chars, &['i', 'è', 'r']) && suffix_in(&chars, 3, rv))
                    || (ends_with(&chars, &['I', 'è', 'r']) && suffix_in(&chars, 3, rv))
                {
                    chars.truncate(chars.len() - 3);
                    chars.push('i');
                }
            }
        }
        Group::G => {
            // -ité / -ités: delete if in R2, then cascade.
            if suffix_in(&chars, best_len, r2) {
                chars.truncate(stem_len);
                changed = true;
                if ends_with(&chars, &['a', 'b', 'i', 'l']) {
                    if suffix_in(&chars, 4, r2) {
                        chars.truncate(chars.len() - 4);
                    } else {
                        chars.truncate(chars.len() - 4);
                        chars.extend_from_slice(&['a', 'b', 'l']);
                    }
                } else if ends_with(&chars, &['i', 'c']) {
                    if suffix_in(&chars, 2, r2) {
                        chars.truncate(chars.len() - 2);
                    } else {
                        chars.truncate(chars.len() - 2);
                        chars.extend_from_slice(&['i', 'q', 'U']);
                    }
                } else if ends_with(&chars, &['i', 'v']) && suffix_in(&chars, 2, r2) {
                    chars.truncate(chars.len() - 2);
                }
            }
        }
        Group::H => {
            // if / ive / ifs / ives.
            if suffix_in(&chars, best_len, r2) {
                chars.truncate(stem_len);
                changed = true;
                if ends_with(&chars, &['a', 't']) && suffix_in(&chars, 2, r2) {
                    chars.truncate(chars.len() - 2);
                    if ends_with(&chars, &['i', 'c']) {
                        if suffix_in(&chars, 2, r2) {
                            chars.truncate(chars.len() - 2);
                        } else {
                            chars.truncate(chars.len() - 2);
                            chars.extend_from_slice(&['i', 'q', 'U']);
                        }
                    }
                }
            }
        }
        Group::I => {
            // eaux -> eau (unconditional).
            chars.truncate(stem_len);
            chars.extend_from_slice(&['e', 'a', 'u']);
            changed = true;
        }
        Group::J => {
            // aux -> al if in R1.
            if suffix_in(&chars, best_len, r1) {
                chars.truncate(stem_len);
                chars.extend_from_slice(&['a', 'l']);
                changed = true;
            }
        }
        Group::K => {
            // euse / euses.
            if suffix_in(&chars, best_len, r2) {
                chars.truncate(stem_len);
                changed = true;
            } else if suffix_in(&chars, best_len, r1) {
                chars.truncate(stem_len);
                chars.extend_from_slice(&['e', 'u', 'x']);
                changed = true;
            }
        }
        Group::L => {
            // issement / issements: delete if in R1 and preceded by
            // non-vowel.
            if suffix_in(&chars, best_len, r1) && stem_len > 0 && !is_vowel(chars[stem_len - 1]) {
                chars.truncate(stem_len);
                changed = true;
            }
        }
        Group::M => {
            // amment -> ant if in RV.
            if suffix_in(&chars, best_len, rv) {
                chars.truncate(stem_len);
                chars.extend_from_slice(&['a', 'n', 't']);
                changed = true;
                removed_ment_family = true;
            }
        }
        Group::N => {
            // emment -> ent if in RV.
            if suffix_in(&chars, best_len, rv) {
                chars.truncate(stem_len);
                chars.extend_from_slice(&['e', 'n', 't']);
                changed = true;
                removed_ment_family = true;
            }
        }
        Group::O => {
            // ment / ments: delete if preceded by a vowel in RV.
            if stem_len > 0
                && is_vowel(chars[stem_len - 1])
                && suffix_in(&chars, best_len, rv)
                && stem_len > rv
            {
                chars.truncate(stem_len);
                changed = true;
                removed_ment_family = true;
            }
        }
    }

    (
        chars,
        Step1Outcome {
            changed,
            removed_ment_family,
        },
    )
}

// ---------------------------------------------------------------------------
// Step 2a: verb suffixes beginning with `i`.
// ---------------------------------------------------------------------------

fn step_2a(mut chars: Vec<char>, rv: usize) -> (Vec<char>, bool) {
    const CANDS: &[&[char]] = &[
        &['î', 'm', 'e', 's'],
        &['î', 't'],
        &['î', 't', 'e', 's'],
        &['i'],
        &['i', 'e'],
        &['i', 'e', 's'],
        &['i', 'r'],
        &['i', 'r', 'a'],
        &['i', 'r', 'a', 'i'],
        &['i', 'r', 'a', 'I', 'e', 'n', 't'],
        &['i', 'r', 'a', 'i', 's'],
        &['i', 'r', 'a', 'i', 't'],
        &['i', 'r', 'a', 's'],
        &['i', 'r', 'e', 'n', 't'],
        &['i', 'r', 'e', 'z'],
        &['i', 'r', 'i', 'e', 'z'],
        &['i', 'r', 'i', 'o', 'n', 's'],
        &['i', 'r', 'o', 'n', 's'],
        &['i', 'r', 'o', 'n', 't'],
        &['i', 's'],
        &['i', 's', 's', 'a', 'I', 'e', 'n', 't'],
        &['i', 's', 's', 'a', 'i', 's'],
        &['i', 's', 's', 'a', 'i', 't'],
        &['i', 's', 's', 'a', 'n', 't'],
        &['i', 's', 's', 'a', 'n', 't', 'e'],
        &['i', 's', 's', 'a', 'n', 't', 'e', 's'],
        &['i', 's', 's', 'a', 'n', 't', 's'],
        &['i', 's', 's', 'e'],
        &['i', 's', 's', 'e', 'n', 't'],
        &['i', 's', 's', 'e', 's'],
        &['i', 's', 's', 'e', 'z'],
        &['i', 's', 's', 'i', 'e', 'z'],
        &['i', 's', 's', 'i', 'o', 'n', 's'],
        &['i', 's', 's', 'o', 'n', 's'],
        &['i', 't'],
    ];
    let Some(s) = longest_suffix(&chars, CANDS) else {
        return (chars, false);
    };
    let sl = s.len();
    let stem_len = chars.len() - sl;
    // Must be in RV, preceded by a non-vowel.
    if !suffix_in(&chars, sl, rv) {
        return (chars, false);
    }
    if stem_len == 0 || is_vowel(chars[stem_len - 1]) {
        return (chars, false);
    }
    chars.truncate(stem_len);
    (chars, true)
}

// ---------------------------------------------------------------------------
// Step 2b: other verb suffixes.
// ---------------------------------------------------------------------------

fn step_2b(mut chars: Vec<char>, rv: usize, r2: usize) -> Vec<char> {
    // Three groups, longest match across all three.
    const GROUP_A: &[&[char]] = &[&['i', 'o', 'n', 's']];
    const GROUP_B: &[&[char]] = &[
        &['é'],
        &['é', 'e'],
        &['é', 'e', 's'],
        &['é', 's'],
        &['è', 'r', 'e', 'n', 't'],
        &['e', 'r'],
        &['e', 'r', 'a'],
        &['e', 'r', 'a', 'i'],
        &['e', 'r', 'a', 'I', 'e', 'n', 't'],
        &['e', 'r', 'a', 'i', 's'],
        &['e', 'r', 'a', 'i', 't'],
        &['e', 'r', 'a', 's'],
        &['e', 'r', 'e', 'z'],
        &['e', 'r', 'i', 'e', 'z'],
        &['e', 'r', 'i', 'o', 'n', 's'],
        &['e', 'r', 'o', 'n', 's'],
        &['e', 'r', 'o', 'n', 't'],
        &['e', 'z'],
        &['i', 'e', 'z'],
    ];
    const GROUP_C: &[&[char]] = &[
        &['â', 'm', 'e', 's'],
        &['â', 't'],
        &['â', 't', 'e', 's'],
        &['a'],
        &['a', 'i'],
        &['a', 'I', 'e', 'n', 't'],
        &['a', 'i', 's'],
        &['a', 'i', 't'],
        &['a', 'n', 't'],
        &['a', 'n', 't', 'e'],
        &['a', 'n', 't', 'e', 's'],
        &['a', 'n', 't', 's'],
        &['a', 's'],
        &['a', 's', 's', 'e'],
        &['a', 's', 's', 'e', 'n', 't'],
        &['a', 's', 's', 'e', 's'],
        &['a', 's', 's', 'i', 'e', 'z'],
        &['a', 's', 's', 'i', 'o', 'n', 's'],
    ];
    // Find longest across all three, track which.
    #[derive(Copy, Clone)]
    enum G {
        A,
        B,
        C,
    }
    let mut best_len = 0usize;
    let mut best_group: Option<G> = None;
    for (cands, g) in [(GROUP_A, G::A), (GROUP_B, G::B), (GROUP_C, G::C)] {
        if let Some(s) = longest_suffix(&chars, cands)
            && s.len() > best_len
        {
            best_len = s.len();
            best_group = Some(g);
        }
    }
    let Some(group) = best_group else {
        return chars;
    };
    let stem_len = chars.len() - best_len;
    match group {
        G::A => {
            // ions -> delete if in R2.
            if suffix_in(&chars, best_len, r2) {
                chars.truncate(stem_len);
            }
        }
        G::B => {
            if suffix_in(&chars, best_len, rv) {
                chars.truncate(stem_len);
            }
        }
        G::C => {
            if suffix_in(&chars, best_len, rv) {
                chars.truncate(stem_len);
                // If further preceded by e (in RV), delete e too.
                if let Some(&c) = chars.last()
                    && c == 'e'
                    && suffix_in(&chars, 1, rv)
                {
                    chars.pop();
                }
            }
        }
    }
    chars
}

// ---------------------------------------------------------------------------
// Step 3: Y -> i, ç -> c.
// ---------------------------------------------------------------------------

fn step_3(chars: &mut [char]) {
    // The spec: replace final Y with i or final ç with c.
    // "Final" here means the last character; but the traditional
    // reading (matching Snowball's reference implementations) is:
    // replace *any* remaining Y with i and *any* ç with c
    // (i.e., globally, not just the last one). We follow the global
    // reading — the U/I/Y marks laid down in step 1's preprocessing
    // must all be lowered.
    for c in chars.iter_mut() {
        *c = match *c {
            'Y' => 'i',
            'ç' => 'c',
            other => other,
        };
    }
}

// ---------------------------------------------------------------------------
// Step 4: residual suffix cleanup.
// ---------------------------------------------------------------------------

/// Step 4 candidate suffixes for longest-match residual cleanup.
const STEP_4_CANDS: &[&[char]] = &[
    &['i', 'o', 'n'],
    &['i', 'e', 'r'],
    &['i', 'è', 'r', 'e'],
    &['I', 'e', 'r'],
    &['I', 'è', 'r', 'e'],
    &['e'],
    &['ë'],
];

fn step_4(chars: &mut Vec<char>, rv: usize, r2: usize) {
    // (1) Trailing s not preceded by a i o u è s → delete.
    if let Some(&last) = chars.last()
        && last == 's'
        && chars.len() >= 2
    {
        let prev = chars[chars.len() - 2];
        if !matches!(prev, 'a' | 'i' | 'o' | 'u' | 'è' | 's') {
            chars.pop();
        }
    }
    // (2) Longest match from {ion, ier, ière, Ier, Ière, e, ë}.
    let cands = STEP_4_CANDS;
    let Some(s) = longest_suffix(chars, cands) else {
        return;
    };
    let sl = s.len();
    let stem_len = chars.len() - sl;
    // Rule keyed by which suffix matched.
    match s {
        &['i', 'o', 'n'] => {
            // Delete if in R2 and preceded by s or t.
            if suffix_in(chars, sl, r2) && stem_len > 0 && matches!(chars[stem_len - 1], 's' | 't')
            {
                chars.truncate(stem_len);
            }
        }
        &['i', 'e', 'r'] | &['i', 'è', 'r', 'e'] | &['I', 'e', 'r'] | &['I', 'è', 'r', 'e'] => {
            if suffix_in(chars, sl, rv) {
                chars.truncate(stem_len);
                chars.push('i');
            }
        }
        &['e'] => {
            if suffix_in(chars, sl, rv) {
                chars.truncate(stem_len);
            }
        }
        &['ë'] if stem_len >= 2 && chars[stem_len - 2] == 'g' && chars[stem_len - 1] == 'u' => {
            // Only strip ë if preceded by "gu" (i.e. "guë" pattern).
            chars.truncate(stem_len);
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Step 5: undouble.
// ---------------------------------------------------------------------------

fn step_5(chars: &mut Vec<char>) {
    for suffix in [
        &['e', 'n', 'n'][..],
        &['o', 'n', 'n'][..],
        &['e', 't', 't'][..],
        &['e', 'l', 'l'][..],
        &['e', 'i', 'l', 'l'][..],
    ] {
        if ends_with(chars, suffix) {
            chars.pop();
            return;
        }
    }
}

// ---------------------------------------------------------------------------
// Step 6: un-accent.
// ---------------------------------------------------------------------------

fn step_6(chars: &mut [char]) {
    // If word ends é or è followed by at least one non-vowel, replace
    // with e.
    // Find the last occurrence of é or è; check that all chars after
    // it are non-vowels and that there's at least one such char.
    let n = chars.len();
    if n < 2 {
        return;
    }
    // Search from the end backwards for é or è.
    for i in (0..n).rev() {
        let c = chars[i];
        if c == 'é' || c == 'è' {
            // Everything strictly after `i` must exist and be non-vowel.
            let tail = &chars[i + 1..];
            if !tail.is_empty() && tail.iter().all(|&x| !is_vowel(x)) {
                chars[i] = 'e';
            }
            return;
        }
        if is_vowel(c) {
            // Found a vowel that is not é/è — stop searching.
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &str) -> String {
        FrenchSnowball.stem(w).into_owned()
    }

    #[test]
    fn short_words_are_unchanged() {
        assert_eq!(s(""), "");
        assert_eq!(s("a"), "a");
        assert_eq!(s("le"), "le");
    }

    #[test]
    fn step_4_e_deletion() {
        // "continue" — nothing in steps 1-3 fires; step 4 deletes
        // trailing "e" if it's in RV.
        assert_eq!(s("continue"), "continu");
    }

    #[test]
    fn step_4_s_then_e() {
        // "continues" — trailing s (not preceded by a/i/o/u/è/s) is
        // stripped; then trailing e is stripped.
        assert_eq!(s("continues"), "continu");
    }

    #[test]
    fn step_2b_er_deletion() {
        // "continuer" — step 2b's "er" rule fires (in RV).
        assert_eq!(s("continuer"), "continu");
    }

    #[test]
    fn regions_match_paper_examples() {
        // "hivernale" (an example commonly used in Snowball docs):
        //   h,i,v,e,r,n,a,l,e
        //   R1 begins after 'v' (first non-vowel after a vowel) — so R1=3
        //   R2 begins after 'r' (first non-vowel after a vowel in R1) — so R2=5
        //   RV: word[0]='h' (consonant), word[1]='i' (vowel) — the
        //       "consonant-vowel case" — so RV starts at position 3.
        let chars: Vec<char> = "hivernale".chars().collect();
        assert_eq!(compute_r1(&chars), 3);
        assert_eq!(compute_r2(&chars, 3), 5);
        assert_eq!(compute_rv(&chars), 3);
    }

    #[test]
    fn rv_consonant_consonant_start() {
        // "grande" — g,r,a,n,d,e — starts consonant + consonant, so
        // RV is after the first vowel at position >= 2, i.e. after
        // 'a' at position 2 → RV = 3.
        let chars: Vec<char> = "grande".chars().collect();
        assert_eq!(compute_rv(&chars), 3);
    }

    #[test]
    fn rv_par_col_tap_exception() {
        let chars: Vec<char> = "parler".chars().collect();
        assert_eq!(compute_rv(&chars), 3);
        let chars: Vec<char> = "colline".chars().collect();
        assert_eq!(compute_rv(&chars), 3);
        let chars: Vec<char> = "tapis".chars().collect();
        assert_eq!(compute_rv(&chars), 3);
    }

    #[test]
    fn rv_two_initial_vowels() {
        let chars: Vec<char> = "aimer".chars().collect();
        assert_eq!(compute_rv(&chars), 3);
    }

    #[test]
    fn stem_is_idempotent_on_common_vocabulary() {
        for w in [
            "continu",
            "continue",
            "continues",
            "continuer",
            "parler",
            "parle",
            "mange",
            "mangent",
            "petit",
            "grande",
            "beauté",
        ] {
            let once = FrenchSnowball.stem(w).into_owned();
            let twice = FrenchSnowball.stem(&once).into_owned();
            assert_eq!(
                once, twice,
                "not idempotent on {w:?}: {once:?} -> {twice:?}"
            );
        }
    }
}
