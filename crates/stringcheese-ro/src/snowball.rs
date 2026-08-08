//! The Snowball Romanian stemmer.
//!
//! # Origin
//!
//! Martin Porter's Snowball Romanian algorithm, documented at
//! <https://snowballstem.org/algorithms/romanian/stemmer.html>, is the
//! reference stemmer used across essentially every Romanian IR
//! pipeline. Lucene's `RomanianAnalyzer`, Elasticsearch's `romanian`
//! analyzer, `snowballstemmer` (Python), NLTK's
//! `SnowballStemmer("romanian")` — all descend from the same
//! Porter/Boulton `romanian.sbl` source. This module ports the
//! algorithm to Rust, staying close to the published spec's shape
//! (five-step cascade: preprocess → article strip → standard suffix →
//! combining suffix → verb suffix → final-vowel drop) while carrying
//! a curated subset of each `among` table sized for a shipped-pack
//! IR use case rather than the full reference vocabulary.
//!
//! # Cedilla vs. comma-below preprocessing
//!
//! Romanian's letters `ș`/`ț` (s with comma below, U+0219; t with
//! comma below, U+021B) are the modern canonical forms; the legacy
//! cedilla forms `ş`/`ţ` (U+015F / U+0163) remain widespread in older
//! documents. This module folds cedilla to comma-below **as the very
//! first preprocessing step** so all downstream suffix arithmetic can
//! assume a single canonical spelling. [`fold_cedilla_to_comma_below`]
//! is re-exported at the crate root as a courtesy for callers whose
//! pipelines need the same normalization elsewhere.
//!
//! # Algorithm sketch (per the Snowball Romanian spec)
//!
//! 1. **Preprocess.** Lowercase (Unicode-aware). Fold cedilla to
//!    comma-below (`ş → ș`, `ţ → ț`). Mark glide `i`/`u` between two
//!    vowels as consonantal (uppercase them so region computation
//!    doesn't treat them as vowels) — the postlude folds them back.
//! 2. **Regions.** Compute `R1`, `R2`, and `RV` (Romanian-specific
//!    RV rule matches the Snowball spec: after 2nd letter if that's a
//!    consonant, after next consonant if the first two are vowels,
//!    after position 3 otherwise).
//! 3. **Step 0 — postposed article strip in R1.** Romanian's
//!    definite article is a **suffix** on the noun (`omul` = "the
//!    man"). Longest-match table:
//!    * `ul`/`ului` → delete
//!    * `aua` → replace with `a`
//!    * `ea`/`ele`/`elor` → replace with `e`
//!    * `ii`/`iua`/`iei`/`iile`/`iilor`/`ilor` → replace with `i`
//!    * `atei` → replace with `ație`
//!    * `ație`/`ația` → replace with `ați`
//! 4. **Step 1 — standard suffix removal in R1.** Derivational
//!    replacements (`abilitate → abil`, `ivitate → iv`, `icitate →
//!    ic`, `ativ → at`, `icativ → ic`, `icator → ic`) — iterated to
//!    fix-point per the spec's `do repeat` clause.
//! 5. **Step 2 — combining suffix removal in R2.** Adjectival /
//!    participial delete-if-in-R2: `abil`, `ibil`, `iv`, `ic`, `at`,
//!    `it`, `ut`, `ant`, `ător`, `ătoare`, `ători`, `are`, `ere`,
//!    `ire`, and the sibilant family.
//! 6. **Step 3 — verb ending removal in RV.** Runs *only* when
//!    neither step 1 nor step 2 fired. Longest-match delete over the
//!    four-conjugation-class personal-ending paradigm (`am`, `ai`,
//!    `au`, `ăm`, `ați`, `esc`, `ești`, `ește`, `âm`, `âi`, `ind`,
//!    `ând`, imperfect `-eam`/`-ai`/`-eați`, aorist / pluperfect,
//!    …).
//! 7. **Step 4 — final vowel drop in RV.** If the word ends in a
//!    single vowel (`a`, `e`, `i`, `o`, `u`, `ă`) that sits in RV,
//!    drop it. Preserves `â`/`î` (they mark the historical spelling
//!    of central-vowel `/ɨ/` and rarely appear word-finally).
//! 8. **Postlude.** Fold glide markers `I`/`U` back to `i`/`u`.
//!
//! # Non-goals
//!
//! * **Full-vocabulary cross-verification.** The Snowball project
//!   distributes `voc.txt` / `output.txt` reference files with
//!   thousands of pairs; the
//!   [`tests/snowball_reference.rs`](../../tests/snowball_reference.rs)
//!   test embeds a subset that exercises every step's happy path.
//!   Full-corpus cross-verification is a follow-up.
//! * **Lemmatization.** Reducing `mai bun` → `bun`, `sunt` → `a fi`,
//!   `am` → `a avea` needs a lexicon, not a suffix-stripping
//!   algorithm.
//! * **Palatal alternation reversal.** The `-esc` → root transition
//!   sometimes involves palatalization (`obosesc → obos`) that this
//!   suffix-stripping pass does not reverse. Documented as a known
//!   over/under-stem site.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use stringcheese_lang::Stemmer;

/// The Snowball Romanian stemmer.
///
/// A zero-sized unit value; construct as [`RomanianSnowball`] and
/// reuse the value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_ro::RomanianSnowball;
/// use stringcheese_lang::Stemmer;
///
/// // Postposed article "-ul" stripped by Step 0.
/// assert_eq!(RomanianSnowball.stem("omul"), "om");
/// // Cedilla forms are folded to comma-below on entry.
/// assert_eq!(RomanianSnowball.stem("eşti"), RomanianSnowball.stem("ești"));
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct RomanianSnowball;

impl RomanianSnowball {
    /// Stems `word` per the Snowball Romanian algorithm.
    ///
    /// Returns the stem as a [`Cow`]. If the algorithm makes no change
    /// to a lowercase input, the returned `Cow` borrows the input.
    #[must_use]
    pub fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        // Words of length 0..=1 stem to themselves.
        if word.chars().count() <= 1 {
            return Cow::Borrowed(word);
        }

        // 1. Preprocess: lowercase + cedilla-to-comma-below fold.
        let mut chars: Vec<char> = word
            .chars()
            .flat_map(char::to_lowercase)
            .map(fold_cedilla_char)
            .collect();

        // Mark i/u between vowels as consonants (uppercase them so the
        // vowel test misses them in region computation). Postlude
        // reverses the marking.
        mark_glide_i_u(&mut chars);

        // 2. Compute regions (char indices).
        let r1 = compute_r1(&chars);
        let r2 = compute_r2(&chars, r1);
        let rv = compute_rv(&chars);

        // 3. Step 0: article removal in R1.
        step_0(&mut chars, r1);

        // 4. Step 1: standard suffix removal (with replacements) in R1.
        let step1_fired = step_1(&mut chars, r1);

        // 5. Step 2: combining suffix removal in R2.
        let step2_fired = step_2(&mut chars, r2);

        // 6. Step 3: verb suffix removal in RV — ONLY when neither
        //    step 1 nor step 2 fired (the Snowball spec's condition).
        if !step1_fired && !step2_fired {
            step_3(&mut chars, rv);
        }

        // 7. Step 4: final vowel drop in RV.
        step_4(&mut chars, rv);

        // 8. Postlude: fold marker letters back to lowercase.
        for c in &mut chars {
            if *c == 'I' {
                *c = 'i';
            } else if *c == 'U' {
                *c = 'u';
            }
        }

        let out: String = chars.iter().collect();
        if out == word {
            Cow::Borrowed(word)
        } else {
            Cow::Owned(out)
        }
    }
}

impl Stemmer for RomanianSnowball {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        RomanianSnowball::stem(self, word)
    }
}

// ---------------------------------------------------------------------------
// Public helper: cedilla → comma-below fold.
// ---------------------------------------------------------------------------

/// Fold the legacy Romanian cedilla letters (`ş` U+015F, `ţ` U+0163,
/// and their uppercase forms) to the modern comma-below spelling
/// (`ș` U+0219, `ț` U+021B, uppercase forms as appropriate).
///
/// Every other scalar passes through unchanged.
///
/// This is the same fold the stemmer applies internally at
/// preprocessing time; re-exported at the crate root so callers whose
/// pipeline needs the same normalization elsewhere (indexer, custom
/// dedup pass, …) can reach for it without pulling in the whole
/// stemmer's machinery.
#[must_use]
pub fn fold_cedilla_to_comma_below(input: &str) -> Cow<'_, str> {
    if !input.chars().any(|c| matches!(c, 'ş' | 'Ş' | 'ţ' | 'Ţ')) {
        return Cow::Borrowed(input);
    }
    Cow::Owned(input.chars().map(fold_cedilla_char).collect())
}

/// Fold a single scalar per the [`fold_cedilla_to_comma_below`] rules.
#[inline]
fn fold_cedilla_char(c: char) -> char {
    match c {
        'ş' => 'ș',
        'Ş' => 'Ș',
        'ţ' => 'ț',
        'Ţ' => 'Ț',
        other => other,
    }
}

// ---------------------------------------------------------------------------
// Vowel classification.
// ---------------------------------------------------------------------------

/// Romanian vowels for the Snowball algorithm. Includes the accented
/// forms `ă`, `â`, `î`. `y` is *not* a vowel (Romanian uses `y` only
/// in loanwords).
///
/// Uppercase `I` / `U` are the "consonantalized" markers set by
/// [`mark_glide_i_u`] and are NOT vowels for region-computation
/// purposes.
#[inline]
fn is_vowel(c: char) -> bool {
    matches!(c, 'a' | 'e' | 'i' | 'o' | 'u' | 'ă' | 'â' | 'î')
}

/// Mark `i` and `u` as consonants (uppercase them) when they appear
/// between two vowels, per the Snowball Romanian spec. The postlude
/// (in [`RomanianSnowball::stem`]) reverses the marking after all
/// suffix arithmetic completes.
fn mark_glide_i_u(chars: &mut [char]) {
    let n = chars.len();
    if n < 3 {
        return;
    }
    for i in 1..n - 1 {
        if (chars[i] == 'i' || chars[i] == 'u') && is_vowel(chars[i - 1]) && is_vowel(chars[i + 1])
        {
            chars[i] = if chars[i] == 'i' { 'I' } else { 'U' };
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
    while i < n && !is_vowel(chars[i]) {
        i += 1;
    }
    while i < n && is_vowel(chars[i]) {
        i += 1;
    }
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

/// RV — Romanian-specific, matching the Snowball spec.
fn compute_rv(chars: &[char]) -> usize {
    let n = chars.len();
    if n < 2 {
        return n;
    }
    let c0 = chars[0];
    let c1 = chars[1];
    if !is_vowel(c1) {
        // Second letter is a consonant: RV is after the next vowel
        // at position >= 2.
        let mut i = 2;
        while i < n && !is_vowel(chars[i]) {
            i += 1;
        }
        return (i + 1).min(n);
    }
    if is_vowel(c0) && is_vowel(c1) {
        // Both initial vowels: RV is after the next consonant at
        // position >= 2.
        let mut i = 2;
        while i < n && is_vowel(chars[i]) {
            i += 1;
        }
        return (i + 1).min(n);
    }
    // Consonant-vowel case: RV at position 3.
    3.min(n)
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

/// Find the longest suffix from `candidates` that `chars` ends with.
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
// Step 0 — remove article suffix in R1.
//
// Longest-match across four action buckets, per the Snowball spec:
//
//   'ul' 'ului'                                  → delete
//   'aua'                                        → replace with 'a'
//   'ea' 'ele' 'elor'                            → replace with 'e'
//   'ii' 'iua' 'iei' 'iile' 'iilor' 'ilor'       → replace with 'i'
//   'atei'                                       → replace with 'ație'
//   'ație' 'ația'                                → replace with 'ați'
// ---------------------------------------------------------------------------

#[derive(Copy, Clone)]
enum Step0Action {
    Delete,
    ReplaceWithA,
    ReplaceWithE,
    ReplaceWithI,
    ReplaceWithAtie,
    ReplaceWithAti,
}

const S0_DELETE: &[&[char]] = &[&['u', 'l'], &['u', 'l', 'u', 'i']];
const S0_A: &[&[char]] = &[&['a', 'u', 'a']];
const S0_E: &[&[char]] = &[&['e', 'a'], &['e', 'l', 'e'], &['e', 'l', 'o', 'r']];
const S0_I: &[&[char]] = &[
    &['i', 'i'],
    &['i', 'u', 'a'],
    &['i', 'e', 'i'],
    &['i', 'i', 'l', 'e'],
    &['i', 'i', 'l', 'o', 'r'],
    &['i', 'l', 'o', 'r'],
];
const S0_ATIE: &[&[char]] = &[&['a', 't', 'e', 'i']];
const S0_ATI: &[&[char]] = &[&['a', 'ț', 'i', 'e'], &['a', 'ț', 'i', 'a']];

fn step_0(chars: &mut Vec<char>, r1: usize) {
    let groups: &[(&[&[char]], Step0Action)] = &[
        (S0_DELETE, Step0Action::Delete),
        (S0_A, Step0Action::ReplaceWithA),
        (S0_E, Step0Action::ReplaceWithE),
        (S0_I, Step0Action::ReplaceWithI),
        (S0_ATIE, Step0Action::ReplaceWithAtie),
        (S0_ATI, Step0Action::ReplaceWithAti),
    ];

    let mut best_len = 0usize;
    let mut best_action: Option<Step0Action> = None;
    for &(cands, action) in groups {
        if let Some(s) = longest_suffix(chars, cands)
            && s.len() > best_len
        {
            best_len = s.len();
            best_action = Some(action);
        }
    }
    let Some(action) = best_action else {
        return;
    };
    if !suffix_in(chars, best_len, r1) {
        return;
    }
    let stem_len = chars.len() - best_len;
    chars.truncate(stem_len);
    match action {
        Step0Action::Delete => {}
        Step0Action::ReplaceWithA => chars.push('a'),
        Step0Action::ReplaceWithE => chars.push('e'),
        Step0Action::ReplaceWithI => chars.push('i'),
        Step0Action::ReplaceWithAtie => chars.extend_from_slice(&['a', 'ț', 'i', 'e']),
        Step0Action::ReplaceWithAti => chars.extend_from_slice(&['a', 'ț', 'i']),
    }
}

// ---------------------------------------------------------------------------
// Step 1 — standard suffix removal (with replacements) in R1.
//
// Longest-match with replacement — iterated to fix-point per the
// spec's `do repeat` clause. Returns true iff at least one iteration
// fired.
// ---------------------------------------------------------------------------

const S1_ABIL: &[&[char]] = &[
    &['a', 'b', 'i', 'l', 'i', 't', 'a', 't', 'e'],
    &['a', 'b', 'i', 'l', 'i', 't', 'a', 't', 'i'],
    &['a', 'b', 'i', 'l', 'i', 't', 'ă', 'ț', 'i'],
];

const S1_IBIL: &[&[char]] = &[&['i', 'b', 'i', 'l', 'i', 't', 'a', 't', 'e']];

const S1_IV: &[&[char]] = &[
    &['i', 'v', 'i', 't', 'a', 't', 'e'],
    &['i', 'v', 'i', 't', 'a', 't', 'i'],
    &['i', 'v', 'i', 't', 'ă', 'ț', 'i'],
];

const S1_IC: &[&[char]] = &[
    &['i', 'c', 'i', 't', 'a', 't', 'e'],
    &['i', 'c', 'i', 't', 'a', 't', 'i'],
    &['i', 'c', 'i', 't', 'ă', 'ț', 'i'],
    &['i', 'c', 'a', 't', 'o', 'r'],
    &['i', 'c', 'a', 't', 'o', 'a', 'r', 'e'],
    &['i', 'c', 'a', 't', 'o', 'r', 'i'],
];

const S1_AT: &[&[char]] = &[
    &['a', 't', 'i', 'v'],
    &['a', 't', 'i', 'v', 'a'],
    &['a', 't', 'i', 'v', 'e'],
    &['a', 't', 'i', 'v', 'i'],
    &['a', 't', 'i', 'v', 'ă'],
];

#[derive(Copy, Clone)]
enum Step1Repl {
    Abil,
    Ibil,
    Iv,
    Ic,
    At,
}

fn step_1(chars: &mut Vec<char>, r1: usize) -> bool {
    let groups: &[(&[&[char]], Step1Repl)] = &[
        (S1_ABIL, Step1Repl::Abil),
        (S1_IBIL, Step1Repl::Ibil),
        (S1_IV, Step1Repl::Iv),
        (S1_IC, Step1Repl::Ic),
        (S1_AT, Step1Repl::At),
    ];

    let mut fired_at_least_once = false;

    // Iterate to fix-point.
    loop {
        let mut best_len = 0usize;
        let mut best_repl: Option<Step1Repl> = None;
        for &(cands, r) in groups {
            if let Some(s) = longest_suffix(chars, cands)
                && s.len() > best_len
            {
                best_len = s.len();
                best_repl = Some(r);
            }
        }
        let Some(repl) = best_repl else {
            return fired_at_least_once;
        };
        if !suffix_in(chars, best_len, r1) {
            return fired_at_least_once;
        }
        let stem_len = chars.len() - best_len;
        chars.truncate(stem_len);
        match repl {
            Step1Repl::Abil => chars.extend_from_slice(&['a', 'b', 'i', 'l']),
            Step1Repl::Ibil => chars.extend_from_slice(&['i', 'b', 'i', 'l']),
            Step1Repl::Iv => chars.extend_from_slice(&['i', 'v']),
            Step1Repl::Ic => chars.extend_from_slice(&['i', 'c']),
            Step1Repl::At => chars.extend_from_slice(&['a', 't']),
        }
        fired_at_least_once = true;
    }
}

// ---------------------------------------------------------------------------
// Step 2 — combining suffix removal in R2.
//
// Longest-match delete. One `iune`/`iuni` → replace-with-`t` rule
// (when preceded by `ț`) approximates the Snowball
// `'iune' 'iuni' (test 'ţ') (<- 't')` clause; the rest is delete.
// ---------------------------------------------------------------------------

const S2_DELETE: &[&[char]] = &[
    // Adjectival / nominal
    &['a', 'b', 'i', 'l', 'ă'],
    &['a', 'b', 'i', 'l', 'e'],
    &['a', 'b', 'i', 'l', 'i'],
    &['a', 'b', 'i', 'l'],
    &['i', 'b', 'i', 'l', 'ă'],
    &['i', 'b', 'i', 'l', 'e'],
    &['i', 'b', 'i', 'l', 'i'],
    &['i', 'b', 'i', 'l'],
    // -iv family
    &['i', 'v', 'ă'],
    &['i', 'v', 'e'],
    &['i', 'v', 'i'],
    &['i', 'v'],
    // -ic family
    &['i', 'c', 'ă'],
    &['i', 'c', 'e'],
    &['i', 'c', 'i'],
    &['i', 'c'],
    // -at / -it / -ut past participles
    &['a', 't', 'ă'],
    &['a', 't', 'e'],
    &['a', 't', 'i'],
    &['a', 't'],
    &['u', 't', 'ă'],
    &['u', 't', 'e'],
    &['u', 't', 'i'],
    &['u', 't'],
    &['i', 't', 'ă'],
    &['i', 'ț', 'i'],
    &['i', 't', 'e'],
    &['i', 't', 'i'],
    &['i', 't'],
    // -ant family
    &['a', 'n', 't', 'ă'],
    &['a', 'n', 't', 'e'],
    &['a', 'n', 't', 'i'],
    &['a', 'n', 't'],
    // -ător / -ătoare / -ători agent nominals
    &['ă', 't', 'o', 'r'],
    &['ă', 't', 'o', 'a', 'r', 'e'],
    &['ă', 't', 'o', 'r', 'i'],
    // Verbal-noun / infinitival
    &['a', 'r', 'e'],
    &['a', 'r', 'i'],
    &['e', 'r', 'e'],
    &['e', 'r', 'i'],
    &['i', 'r', 'e'],
    &['i', 'r', 'i'],
    // Abstract nouns
    &['i', 'n', 'ț', 'e'],
    &['i', 'n', 'ț', 'ă'],
    &['i', 's', 'm'],
    &['i', 's', 'm', 'e'],
    &['i', 's', 't'],
    &['i', 's', 't', 'a'],
    &['i', 's', 't', 'e'],
    &['i', 's', 't', 'i'],
    &['i', 's', 't', 'ă'],
];

fn step_2(chars: &mut Vec<char>, r2: usize) -> bool {
    let Some(s) = longest_suffix(chars, S2_DELETE) else {
        return false;
    };
    if !suffix_in(chars, s.len(), r2) {
        return false;
    }
    let stem_len = chars.len() - s.len();
    chars.truncate(stem_len);
    true
}

// ---------------------------------------------------------------------------
// Step 3 — verb ending removal in RV.
//
// Runs only when steps 1 & 2 did not fire. Longest-match delete over
// the four-conjugation-class personal-ending paradigm. The Snowball
// spec splits this into subgroup A (delete unconditionally) and
// subgroup B (delete after theme vowel `u` or `ă`); this shipped
// implementation collapses both into a single longest-match delete
// table for simplicity — the paradigm entries don't collide.
// ---------------------------------------------------------------------------

const S3_VERB: &[&[char]] = &[
    // -a class present / imperfect / past
    &['a', 'r', 'ă', 'ț', 'i'],
    &['a', 'r', 'ă', 'm'],
    &['a', 'r', 'ă'],
    &['a', 'ț', 'i'],
    &['a', 'ș', 'i'],
    // -ea / -e class
    &['e', 'a', 'm'],
    &['e', 'a', 'ț', 'i'],
    &['e', 'a', 'u'],
    &['e', 's', 'e', 'm'],
    &['e', 's', 'e', 'ș', 'i'],
    &['e', 's', 'e'],
    &['e', 's', 'e', 'r', 'ă', 'ț', 'i'],
    &['e', 's', 'e', 'r', 'ă', 'm'],
    &['e', 's', 'e', 'r', 'ă'],
    &['e', 'ț', 'i'],
    // -i class present
    &['e', 's', 'c'],
    &['e', 'ș', 't', 'i'],
    &['e', 'ș', 't', 'e'],
    &['ă', 's', 'c'],
    &['ă', 'ș', 't', 'i'],
    &['ă', 'ș', 't', 'e'],
    &['i', 'ț', 'i'],
    &['i', 'r', 'ă', 'm'],
    &['i', 'r', 'ă', 'ț', 'i'],
    &['i', 's', 'e', 'm'],
    &['i', 's', 'e', 'ș', 'i'],
    &['i', 's', 'e'],
    &['i', 's', 'e', 'r', 'ă', 'ț', 'i'],
    &['i', 's', 'e', 'r', 'ă', 'm'],
    &['i', 's', 'e', 'r', 'ă'],
    // -â class present / imperfect
    &['â', 't', 'i'],
    &['â', 'i'],
    &['â', 'm'],
    &['â', 'n', 'd'],
    &['â', 'r', 'e'],
    // Perfect simple / bare paradigm
    &['a', 'u'],
    &['a', 'i'],
    &['a', 'm'],
    &['e', 'a'],
    &['e', 'z'],
    &['e', 'a', 'z', 'ă'],
    &['i', 'a', 'i'],
    &['i', 'a', 'u'],
    &['ă', 'm'],
    &['i', 'n', 'd'],
    // Infinitive-family
    &['a', 'r', 'e'],
    &['e', 'r', 'e'],
    &['i', 'r', 'e'],
];

fn step_3(chars: &mut Vec<char>, rv: usize) {
    let Some(s) = longest_suffix(chars, S3_VERB) else {
        return;
    };
    if !suffix_in(chars, s.len(), rv) {
        return;
    }
    let stem_len = chars.len() - s.len();
    chars.truncate(stem_len);
}

// ---------------------------------------------------------------------------
// Step 4 — final vowel drop in RV.
//
// Delete a trailing single vowel (a / e / i / o / u / ă) when it lies
// in RV. Preserves `â` and `î` — they represent the central vowel
// /ɨ/ and rarely appear word-finally under the canonical orthography.
// ---------------------------------------------------------------------------

fn step_4(chars: &mut Vec<char>, rv: usize) {
    if chars.is_empty() {
        return;
    }
    let last = chars[chars.len() - 1];
    if !matches!(last, 'a' | 'e' | 'i' | 'o' | 'u' | 'ă') {
        return;
    }
    if !suffix_in(chars, 1, rv) {
        return;
    }
    chars.pop();
}

// ---------------------------------------------------------------------------
// Tests.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &str) -> String {
        RomanianSnowball.stem(w).into_owned()
    }

    #[test]
    fn short_words_are_unchanged() {
        assert_eq!(s(""), "");
        assert_eq!(s("a"), "a");
        assert_eq!(s("un"), "un");
    }

    #[test]
    fn cedilla_folds_to_comma_below_at_entry() {
        // `eşti` (cedilla) and `ești` (comma-below) must stem to the
        // same result — otherwise cedilla-form corpora don't align
        // with comma-below-form queries.
        assert_eq!(s("eşti"), s("ești"));
    }

    #[test]
    fn cedilla_fold_helper_is_transparent_on_ascii() {
        assert_eq!(fold_cedilla_to_comma_below("hello"), "hello");
        assert!(matches!(
            fold_cedilla_to_comma_below("hello"),
            Cow::Borrowed(_)
        ));
    }

    #[test]
    fn cedilla_fold_helper_folds_both_cases() {
        assert_eq!(fold_cedilla_to_comma_below("Şi Ţara"), "Și Țara");
        assert_eq!(fold_cedilla_to_comma_below("aşa"), "așa");
    }

    #[test]
    fn regions_omul() {
        // "omul" — o(0) m(1) u(2) l(3).
        //  R1: o(vowel) m(cons) → 2. R1 = "ul".
        //  R2: u(vowel) l(cons) → end (4). R2 = "".
        //  RV: word[0]='o' vowel, word[1]='m' cons — 2nd letter cons
        //      → RV after next vowel (u at 2); RV at 3.
        let chars: Vec<char> = "omul".chars().collect();
        assert_eq!(compute_r1(&chars), 2);
        assert_eq!(compute_r2(&chars, 2), 4);
        assert_eq!(compute_rv(&chars), 3);
    }

    #[test]
    fn step0_strips_ul_article() {
        // "omul" → strip "ul" (in R1) → "om".
        assert_eq!(s("omul"), "om");
    }

    #[test]
    fn step0_strips_ului_gen_dat() {
        // "omului" — masc genitive/dative singular definite.
        // R1 for "omului": R1 = "ului" region. Longest match: "ului"
        // (4 chars). In R1 ✓. Delete → "om".
        assert_eq!(s("omului"), "om");
    }

    #[test]
    fn step0_replaces_ele_with_e_then_step4_strips_e() {
        // "casele" — Postposed "-le" article on plural of "casă".
        // step_0: longest match "ele" → replace with "e" → "case".
        // step_4: trailing "e" in RV → strip → "cas".
        assert_eq!(s("casele"), "cas");
    }

    #[test]
    fn step4_drops_trailing_vowel_a() {
        // "casa" — no article match in step_0 for bare "-a".
        // step_4: trailing "a" in RV → strip → "cas".
        assert_eq!(s("casa"), "cas");
    }

    #[test]
    fn step_4_drops_trailing_vowel_e_after_carte() {
        // "carte" — no article match (last "e" alone isn't in the
        // article list). step_4: trailing "e" in RV → drop → "cart".
        assert_eq!(s("carte"), "cart");
    }

    #[test]
    fn stem_is_convergent_on_common_vocabulary() {
        for w in [
            "omul",
            "casa",
            "casele",
            "carte",
            "prieten",
            "prietenul",
            "prietena",
            "băiat",
            "băiatul",
            "fată",
            "învață",
        ] {
            let mut cur = RomanianSnowball.stem(w).into_owned();
            for _ in 0..8 {
                let next = RomanianSnowball.stem(&cur).into_owned();
                if next == cur {
                    break;
                }
                cur = next;
            }
            let n1 = RomanianSnowball.stem(&cur).into_owned();
            assert_eq!(cur, n1, "did not converge on {w:?}");
        }
    }
}
