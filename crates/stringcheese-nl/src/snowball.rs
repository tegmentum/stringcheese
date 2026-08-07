//! The Snowball Dutch stemmer.
//!
//! # Origin
//!
//! Martin Porter's Snowball Dutch algorithm, documented at
//! <https://snowballstem.org/algorithms/dutch/stemmer.html>, is the
//! reference stemmer used across essentially every Dutch IR pipeline.
//! Lucene's `DutchAnalyzer`, Elasticsearch's `dutch` analyzer,
//! `snowballstemmer` (Python), NLTK's `SnowballStemmer("dutch")` — all
//! descend from the same Porter/Boulton `dutch.sbl` source. This module
//! ports the algorithm to Rust, faithfully to the published spec.
//!
//! # Algorithm sketch
//!
//! 1. **Prelude — fold diacritics, mark glide vowels.** Dutch
//!    orthography carries a diaeresis on syllable-splitters (`ë`, `ï`,
//!    `ö`, `ü`) and an acute on stressed loans (`á`, `é`, `í`, `ó`,
//!    `ú`); the stemmer folds each of them to its plain base vowel.
//!    The grave `è` — used on `frisée`-family borrowings — is kept
//!    as-is because it is included in the Snowball vowel set.
//!    Then, per the spec's *put initial `y`, `y` after a vowel, and
//!    `i` between vowels into upper case*, we mark those letters as
//!    `Y` / `I` so they behave as consonants during region and
//!    suffix analysis. The postlude restores them to lowercase.
//! 2. **Regions.** Compute `R1` and `R2`. Standard Snowball convention
//!    (`R1` = the region after the first non-vowel following a vowel),
//!    with the Dutch-specific adjustment that `R1` must not begin
//!    before char index 3 — the region before it must contain at
//!    least three letters.
//! 3. **Step 1 — standard suffix removal (longest match).**
//!    * `heden` (in R1) — replace with `heid`.
//!    * `en` / `ene` (in R1, preceded by a valid en-ending, not
//!      preceded by `gem`) — delete, then undouble.
//!    * `s` / `se` (in R1, preceded by a valid s-ending) — delete.
//!
//!    A **valid en-ending** is any non-vowel; the additional
//!    "not `gem`" guard suppresses the strip on words like
//!    `gemeenten` where the plural marker is fused with the derivational
//!    prefix. A **valid s-ending** is any non-vowel other than `j`.
//! 4. **Step 2 — e-ending.** Delete a trailing `e` if it lies in R1 and
//!    is preceded by a non-vowel; set the `e_removed` flag; then
//!    undouble.
//! 5. **Step 3(a) — `-heid`.** Delete a trailing `heid` if it lies in
//!    R2 and is not preceded by `c`, then re-run the en-ending rule on
//!    the shortened stem.
//! 6. **Step 3(b) — derivational suffixes (longest match).**
//!    * `end` / `ing` (in R2) — delete, then either delete a further
//!      `ig` (in R2 and not preceded by `e`) or undouble.
//!    * `ig` (in R2, not preceded by `e`) — delete.
//!    * `lijk` (in R2) — delete, then apply the Step 2 e-ending rule.
//!    * `baar` (in R2) — delete.
//!    * `bar` (in R2 and `e_removed`) — delete.
//! 7. **Step 4 — undouble short-vowel `-aa/-ee/-oo/-uu`.** If the
//!    stem ends in `aa`, `ee`, `oo`, or `uu` and the character before
//!    the doubled pair is a non-vowel, delete one of the vowels. In
//!    practice this rule fires rarely — most stems end in a consonant
//!    — but it captures Dutch's short-vowel doubling convention when
//!    it does happen.
//! 8. **Postlude — restore `I` / `Y`.** Fold marked `I` back to `i`
//!    and `Y` back to `y`.
//!
//! # Consonant-cluster preservation (`-cht`)
//!
//! The stem-end cluster `-cht` (as in `nacht`, `vlecht`, `slecht`) is
//! preserved by the algorithm without a special rule: the undouble step
//! only touches `kk` / `dd` / `tt`, and `cht` matches none of the
//! Step 3(b) derivational suffixes. Test cases in
//! `tests/snowball_reference.rs` confirm the behaviour on both nominal
//! (`nacht → nacht`) and verbal (`vlechten → vlecht`) forms.
//!
//! # Non-goals
//!
//! * **Full-vocabulary cross-verification.** The Snowball project
//!   distributes `voc.txt` / `output.txt` reference files with tens of
//!   thousands of pairs; the [`tests/snowball_reference.rs`](
//!   ../../tests/snowball_reference.rs) test embeds a *subset* that
//!   exercises every step's happy path and each cascading rule.
//!   Full-corpus cross-verification is a follow-up.
//! * **Compound splitting.** Dutch productively compounds nouns
//!   (`boekwinkel`, `stadhuis`); splitting them requires a compound-
//!   noun dictionary and is not part of the Snowball algorithm.
//! * **Lemmatization.** Reducing `beter → goed`, `ben → zijn` needs a
//!   lexicon, not a suffix-stripping algorithm.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use stringcheese_lang::Stemmer;

/// The Snowball Dutch stemmer.
///
/// A zero-sized unit value; construct as [`DutchSnowball`] and reuse
/// the value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_nl::DutchSnowball;
/// use stringcheese_lang::Stemmer;
///
/// assert_eq!(DutchSnowball.stem("lopen"), "lop");
/// assert_eq!(DutchSnowball.stem("boeken"), "boek");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct DutchSnowball;

impl DutchSnowball {
    /// Stems `word` per the Snowball Dutch algorithm.
    ///
    /// Returns the stem as a [`Cow`]. If the algorithm makes no change
    /// to a lowercase input, the returned `Cow` borrows the input.
    #[must_use]
    pub fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        if word.chars().count() <= 1 {
            return Cow::Borrowed(word);
        }

        // 1. Lowercase (Unicode-aware).
        let lower: Vec<char> = word.chars().flat_map(char::to_lowercase).collect();

        // 2. Prelude: fold diacritics and mark glide vowels.
        let mut chars = prelude(&lower);

        // 3. Regions.
        let r1 = compute_r1_adjusted(&chars);
        let r2 = compute_r2(&chars, r1);

        // 4. Steps 1..=4.
        let mut e_removed = false;
        step_1(&mut chars, r1);
        step_2(&mut chars, r1, &mut e_removed);
        step_3a(&mut chars, r1, r2);
        step_3b(&mut chars, r1, r2, e_removed);
        step_4(&mut chars);

        // 5. Postlude: restore I/Y.
        let mut out = String::with_capacity(chars.len());
        for c in chars {
            match c {
                'I' => out.push('i'),
                'Y' => out.push('y'),
                other => out.push(other),
            }
        }

        if out == word {
            Cow::Borrowed(word)
        } else {
            Cow::Owned(out)
        }
    }
}

impl Stemmer for DutchSnowball {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        DutchSnowball::stem(self, word)
    }
}

// ---------------------------------------------------------------------------
// Vowel classification.
// ---------------------------------------------------------------------------

/// Dutch vowels per the Snowball spec. Includes `è` (used on French
/// borrowings after the diaeresis / acute fold). The marked `I` and `Y`
/// (uppercase) are *not* vowels — that's the whole point of the
/// prelude marking pass.
#[inline]
fn is_vowel(c: char) -> bool {
    matches!(c, 'a' | 'e' | 'i' | 'o' | 'u' | 'y' | 'è')
}

// ---------------------------------------------------------------------------
// Prelude: diacritic fold + glide-vowel marking.
// ---------------------------------------------------------------------------

/// Fold Dutch diacritics and mark initial `y` / `y` after vowel / `i`
/// between vowels as `Y` / `I`. Returns a fresh char buffer.
fn prelude(lower: &[char]) -> Vec<char> {
    let mut out: Vec<char> = Vec::with_capacity(lower.len());
    for &c in lower {
        // Fold the diaeresis (`ä ë ï ö ü`) and the acute (`á é í ó ú`)
        // to their base vowels. `è` is kept as-is — it's part of the
        // Snowball Dutch vowel set. Grouped-by-target so
        // `clippy::match_same_arms` doesn't flag the collapse.
        let folded = match c {
            'ä' | 'á' => 'a',
            'ë' | 'é' => 'e',
            'ï' | 'í' => 'i',
            'ö' | 'ó' => 'o',
            'ü' | 'ú' => 'u',
            other => other,
        };
        out.push(folded);
    }
    // Mark initial `y`, `y` after a vowel, and `i` between vowels.
    let n = out.len();
    for i in 0..n {
        let c = out[i];
        if c == 'y' {
            let prev_is_vowel = i > 0 && is_vowel(out[i - 1]);
            if i == 0 || prev_is_vowel {
                out[i] = 'Y';
            }
        } else if c == 'i' {
            let prev_is_vowel = i > 0 && is_vowel(out[i - 1]);
            let next_is_vowel = i + 1 < n && is_vowel(out[i + 1]);
            if prev_is_vowel && next_is_vowel {
                out[i] = 'I';
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Regions R1 and R2 — as char indices.
// ---------------------------------------------------------------------------

/// R1 = char index after the first non-vowel following a vowel
/// (or `chars.len()` if no such position exists). Adjusted so R1 ≥ 3.
fn compute_r1_adjusted(chars: &[char]) -> usize {
    let r1 = compute_r1(chars);
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

/// R2 = the same rule applied to `chars[R1..]`.
fn compute_r2(chars: &[char], r1: usize) -> usize {
    if r1 >= chars.len() {
        return chars.len();
    }
    let tail = &chars[r1..];
    r1 + compute_r1(tail)
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

/// A valid Dutch s-ending: a non-vowel other than `j`.
///
/// The `I` / `Y` marker letters are non-vowels, so they qualify.
#[inline]
fn is_valid_s_ending(c: char) -> bool {
    !is_vowel(c) && c != 'j'
}

/// A valid Dutch en-ending: any non-vowel.
#[inline]
fn is_valid_en_ending(c: char) -> bool {
    !is_vowel(c)
}

/// True if the three characters immediately preceding the suffix at
/// `chars[..stem_len]` spell `gem` (the guard on the `-en` strip).
fn preceded_by_gem(chars: &[char], stem_len: usize) -> bool {
    stem_len >= 3
        && chars[stem_len - 3] == 'g'
        && chars[stem_len - 2] == 'e'
        && chars[stem_len - 1] == 'm'
}

/// Undouble a trailing `kk` / `dd` / `tt` down to a single letter.
fn undouble(chars: &mut Vec<char>) {
    let n = chars.len();
    if n < 2 {
        return;
    }
    let last = chars[n - 1];
    let prev = chars[n - 2];
    if last == prev && matches!(last, 'k' | 'd' | 't') {
        chars.pop();
    }
}

// ---------------------------------------------------------------------------
// Step 1: heden / en / ene / s / se.
// ---------------------------------------------------------------------------

const S1_HEDEN: &[char] = &['h', 'e', 'd', 'e', 'n'];
const S1_ENE: &[char] = &['e', 'n', 'e'];
const S1_EN: &[char] = &['e', 'n'];
const S1_SE: &[char] = &['s', 'e'];
const S1_S: &[char] = &['s'];

fn step_1(chars: &mut Vec<char>, r1: usize) {
    let candidates: &[&[char]] = &[S1_HEDEN, S1_ENE, S1_EN, S1_SE, S1_S];
    let Some(s) = longest_suffix(chars, candidates) else {
        return;
    };
    let sl = s.len();
    if !suffix_in(chars, sl, r1) {
        return;
    }
    let stem_len = chars.len() - sl;

    if s == S1_HEDEN {
        chars.truncate(stem_len);
        chars.extend_from_slice(&['h', 'e', 'i', 'd']);
        return;
    }

    if s == S1_EN || s == S1_ENE {
        // valid en-ending: non-vowel; not preceded by `gem`.
        if stem_len == 0 {
            return;
        }
        if !is_valid_en_ending(chars[stem_len - 1]) {
            return;
        }
        if preceded_by_gem(chars, stem_len) {
            return;
        }
        chars.truncate(stem_len);
        undouble(chars);
        return;
    }

    if s == S1_S || s == S1_SE {
        if stem_len == 0 {
            return;
        }
        if !is_valid_s_ending(chars[stem_len - 1]) {
            return;
        }
        chars.truncate(stem_len);
    }
}

// ---------------------------------------------------------------------------
// Step 2: e-ending.
// ---------------------------------------------------------------------------

fn step_2(chars: &mut Vec<char>, r1: usize, e_removed: &mut bool) {
    if !ends_with(chars, &['e']) {
        return;
    }
    if !suffix_in(chars, 1, r1) {
        return;
    }
    let stem_len = chars.len() - 1;
    if stem_len == 0 {
        return;
    }
    if is_vowel(chars[stem_len - 1]) {
        return;
    }
    chars.truncate(stem_len);
    *e_removed = true;
    undouble(chars);
}

// ---------------------------------------------------------------------------
// Step 3a: heid.
// ---------------------------------------------------------------------------

const HEID: &[char] = &['h', 'e', 'i', 'd'];

fn step_3a(chars: &mut Vec<char>, r1: usize, r2: usize) {
    if !ends_with(chars, HEID) {
        return;
    }
    let sl = HEID.len();
    if !suffix_in(chars, sl, r2) {
        return;
    }
    let stem_len = chars.len() - sl;
    if stem_len > 0 && chars[stem_len - 1] == 'c' {
        return;
    }
    chars.truncate(stem_len);
    // Re-run the en-ending sub-rule.
    if ends_with(chars, S1_EN) && suffix_in(chars, S1_EN.len(), r1) {
        let stem_len2 = chars.len() - S1_EN.len();
        if stem_len2 > 0
            && is_valid_en_ending(chars[stem_len2 - 1])
            && !preceded_by_gem(chars, stem_len2)
        {
            chars.truncate(stem_len2);
            undouble(chars);
        }
    }
}

// ---------------------------------------------------------------------------
// Step 3b: end / ing / ig / lijk / baar / bar.
// ---------------------------------------------------------------------------

const S3B_END: &[char] = &['e', 'n', 'd'];
const S3B_ING: &[char] = &['i', 'n', 'g'];
const S3B_IG: &[char] = &['i', 'g'];
const S3B_LIJK: &[char] = &['l', 'i', 'j', 'k'];
const S3B_BAAR: &[char] = &['b', 'a', 'a', 'r'];
const S3B_BAR: &[char] = &['b', 'a', 'r'];

fn step_3b(chars: &mut Vec<char>, r1: usize, r2: usize, e_removed: bool) {
    let candidates: &[&[char]] = &[S3B_END, S3B_ING, S3B_IG, S3B_LIJK, S3B_BAAR, S3B_BAR];
    let Some(s) = longest_suffix(chars, candidates) else {
        return;
    };
    let sl = s.len();
    if !suffix_in(chars, sl, r2) {
        return;
    }
    let stem_len = chars.len() - sl;

    if s == S3B_END || s == S3B_ING {
        chars.truncate(stem_len);
        // Try `ig` (in R2, not preceded by `e`) — else undouble.
        let n = chars.len();
        if n >= 2
            && chars[n - 2] == 'i'
            && chars[n - 1] == 'g'
            && suffix_in(chars, 2, r2)
            && !(n >= 3 && chars[n - 3] == 'e')
        {
            chars.truncate(n - 2);
        } else {
            undouble(chars);
        }
        return;
    }

    if s == S3B_IG {
        // Delete if not preceded by `e`.
        if stem_len > 0 && chars[stem_len - 1] == 'e' {
            return;
        }
        chars.truncate(stem_len);
        return;
    }

    if s == S3B_LIJK {
        chars.truncate(stem_len);
        // Apply Step 2 (e-ending) on the shortened stem. The
        // `e_removed` flag is a follow-on read-only signal for the
        // `bar` branch, which we've already dispatched past.
        let mut sink = false;
        step_2(chars, r1, &mut sink);
        return;
    }

    if s == S3B_BAAR {
        chars.truncate(stem_len);
        return;
    }

    if s == S3B_BAR {
        if !e_removed {
            return;
        }
        chars.truncate(stem_len);
    }
}

// ---------------------------------------------------------------------------
// Step 4: undouble the trailing short-vowel double aa / ee / oo / uu.
// ---------------------------------------------------------------------------

fn step_4(chars: &mut Vec<char>) {
    let n = chars.len();
    if n < 3 {
        return;
    }
    let last = chars[n - 1];
    let prev = chars[n - 2];
    let two_back = chars[n - 3];
    if last != prev {
        return;
    }
    if !matches!(last, 'a' | 'e' | 'o' | 'u') {
        return;
    }
    if is_vowel(two_back) {
        return;
    }
    chars.pop();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &str) -> String {
        DutchSnowball.stem(w).into_owned()
    }

    #[test]
    fn short_words_are_unchanged() {
        assert_eq!(s(""), "");
        assert_eq!(s("a"), "a");
        assert_eq!(s("de"), "de");
    }

    #[test]
    fn step1_plain_en_strip() {
        assert_eq!(s("boeken"), "boek");
        assert_eq!(s("dagen"), "dag");
        assert_eq!(s("lopen"), "lop");
    }

    #[test]
    fn step1_s_strip() {
        // `honden` → step 1: en, non-v `d` preceded, delete → `hond`.
        assert_eq!(s("honden"), "hond");
        // `boeks` — `s` preceded by `k` (valid s-ending). Delete → `boek`.
        assert_eq!(s("boeks"), "boek");
    }

    #[test]
    fn step1_heden_replace() {
        // `snelheden` → step 1 replaces `heden` with `heid` → `snelheid`.
        // R1 for `snelheden` = 4 (`n` at 3 in `snel..`, +1). `heden` at
        // position 4, in R1. Replace. Step 3a: `heid` at position 4;
        // R2 = 8 (past word). Not in R2 → no more strip. Result:
        // `snelheid`.
        assert_eq!(s("snelheden"), "snelheid");
    }

    #[test]
    fn step2_e_ending() {
        // `zaakte` → step 2: `e` preceded by `t` (non-v) → delete → `zaakt`.
        // No undouble (kt not in kk/dd/tt).
        assert_eq!(s("zaakte"), "zaakt");
    }

    #[test]
    fn step3a_heid_strip() {
        // `moeilijkheid` → step 3a: `heid` in R2 not preceded by c → delete.
        // Then step 3b `lijk`... let's trace.
        // moeilijkheid = m o e i l i j k h e i d (12 chars).
        // Prelude: `i` between vowels? Position 3: preceded by `e` (v),
        // followed by `l` (non-v). Not between vowels. Not marked.
        // Position 5: preceded by `l` (non-v), followed by `j` (non-v).
        // Not marked. Position 10: preceded by `e` (v), followed by `d`
        // (non-v). Not marked.
        // R1: m non-v, o v, e v, i v, l non-v at 4. R1 = 5. Adjusted ≥ 3 → 5.
        // R2 tail from 5 = `ijkheid` (7 chars). i v, j non-v at 1. R2 tail
        //   = 2. R2 = 5+2 = 7.
        // Step 1: no match on `d` end (not en/ene/s/se/heden last).
        //   Actually `heden`? chars end in `heid` not `heden`. So no.
        // Step 2: `e` at end? No, ends in `d`.
        // Step 3a: `heid` at position 8. 8 >= R2(7). Preceded by `k`
        //   (not c). Delete → `moeilijk` (8 chars). Then try en:
        //   ends in `k` not `en`. Skip.
        // Step 3b: `lijk` at end position 4. 4 >= R2(7)? No. Doesn't fire.
        // Step 4: undouble aa/ee/oo/uu? no.
        // Result: `moeilijk`.
        assert_eq!(s("moeilijkheid"), "moeilijk");
    }

    #[test]
    fn step3b_ing_strip() {
        // `wandeling` (walk) → step 3b: `ing` in R2, delete → `wandel`.
        // wandeling = w a n d e l i n g (9 chars).
        // R1: w non-v, a v, n non-v at 2. R1 = 3.
        // R2 tail from 3 = `deling`. d non-v, e v, l non-v at 2. R2 tail
        //   = 3. R2 = 3+3 = 6.
        // Step 1: `ing` isn't in step 1. But `en` isn't in tail. Actually
        //   the last two chars are `ng` not `en`. No match.
        // Step 3b: `ing` at position 6. 6 >= 6. In R2. Delete → `wandel`.
        //   Try `ig` at end: `wandel` ends in `l`, not `ig`. Undouble:
        //   last two `el`, not kk/dd/tt. Result: `wandel`.
        assert_eq!(s("wandeling"), "wandel");
    }

    #[test]
    fn step3b_ig_strip() {
        // `machtig` (mighty) → step 3b: `ig` at end. R2 for `machtig`
        // (7 chars): R1 = m non-v, a v, c non-v at 2. R1 = 3. R2 tail
        //   from 3 = `htig`. h non-v, t non-v, i v at 2 (actually h non-v,
        //   t non-v, i v at 2, g non-v at 3), R2 tail = 4. R2 = 3+4 = 7.
        //   `ig` at position 5. 5 >= 7? No. Doesn't fire in R2.
        // Hmm, so short `machtig` doesn't reduce. Try a longer one.
        // `belangrijkheid` → but we tested that. Try `voorlopig`.
        // voorlopig = v o o r l o p i g (9 chars).
        //   R1: v non-v, o v, o v (still vowels), r non-v at 3. R1 = 4.
        //     Adjusted ≥ 3 → 4.
        //   R2 tail from 4 = `lopig`. l non-v, o v, p non-v at 2, R2 tail
        //     = 3. R2 = 4+3 = 7.
        //   Step 3b: `ig` at position 7. 7 >= 7. In R2. Not preceded by
        //     `e` (preceded by `p`). Delete → `voorlop`.
        // But `voorlopig` I'd hope for `voorlop`. That works.
        assert_eq!(s("voorlopig"), "voorlop");
    }

    #[test]
    fn step3b_lijk_strip() {
        // `natuurlijk` (naturally) → step 3b: `lijk` in R2. Delete →
        // `natuur`. Then step 2 e-ending: `natuur` ends in `r` not `e`.
        //   R1: n non-v, a v, t non-v at 2. R1 = 3.
        //   R2 tail from 3 = `uurlijk`. u v, u v, r non-v at 2. R2 tail
        //     = 3. R2 = 3+3 = 6.
        //   `lijk` at position 6, 6 >= 6. Delete → `natuur`.
        //   Step 4: `natuur` last 2 = `ur`. Not doubled. No change.
        //   Result: `natuur`.
        assert_eq!(s("natuurlijk"), "natuur");
    }

    #[test]
    fn step3b_baar_strip() {
        // `houdbaar` (durable) → step 3b: `baar` in R2. Delete → `houd`.
        //   R1: h non-v, o v, u v (vowels), d non-v at 3. R1 = 4. Adjusted
        //     ≥ 3 → 4.
        //   R2 tail from 4 = `baar`. b non-v, a v, a v, r non-v at 3.
        //     R2 tail = 4. R2 = 4+4 = 8. `baar` at position 4. 4 >= 8?
        //     No. Doesn't fire.
        // Try `duurzaam` → doesn't end in `baar`.
        // Try `betaalbaar`: b e t a a l b a a r (10 chars).
        //   R1: b non-v, e v, t non-v at 2. R1 = 3.
        //   R2 tail from 3 = `aalbaar`. a v, a v, l non-v at 2. R2 tail
        //     = 3. R2 = 3+3 = 6.
        //   `baar` at position 6. 6 >= 6. Delete → `betaal`.
        //   Step 4: ends in `al`, not doubled. Result: `betaal`.
        assert_eq!(s("betaalbaar"), "betaal");
    }

    #[test]
    fn cht_cluster_preserved() {
        // `nacht` (night) — no reductions apply. Result: `nacht`.
        assert_eq!(s("nacht"), "nacht");
        // `vlechten` (to braid) — step 1 en delete → `vlecht`. Result: `vlecht`.
        assert_eq!(s("vlechten"), "vlecht");
        // `slechte` → step 2 e delete preceded by t → `slecht`. Result: `slecht`.
        assert_eq!(s("slechte"), "slecht");
    }

    #[test]
    fn ij_digraph_preserved_in_stem() {
        // `mijn` → no ij marking (i preceded by m non-v). Word ends in n,
        // no `en` suffix (it's the whole word). Result: `mijn`.
        assert_eq!(s("mijn"), "mijn");
        // `zij` → 1 short 3 chars — same, no rules fire. Result: `zij`.
        assert_eq!(s("zij"), "zij");
    }

    #[test]
    fn regions_paper_example_lopen() {
        // "lopen" — l, o, p, e, n
        //  R1: l non-v, o v, p non-v at 2. R1 = 3 (adjusted).
        //  R2: tail from 3 = "en". e v, n non-v at 1. R2 tail = 2.
        //       R2 = 3+2 = 5.
        let chars: Vec<char> = "lopen".chars().collect();
        assert_eq!(compute_r1_adjusted(&chars), 3);
        assert_eq!(compute_r2(&chars, 3), 5);
    }

    #[test]
    fn stem_is_convergent_on_common_vocabulary() {
        for w in [
            "boek",
            "boeken",
            "lopen",
            "loop",
            "dagen",
            "dag",
            "huisje",
            "huisjes",
            "houden",
            "vlechten",
            "nacht",
            "kinderen",
            "wandeling",
            "moeilijk",
            "moeilijkheid",
            "natuurlijk",
            "betaalbaar",
        ] {
            let mut cur = DutchSnowball.stem(w).into_owned();
            for _ in 0..5 {
                let next = DutchSnowball.stem(&cur).into_owned();
                if next == cur {
                    break;
                }
                cur = next;
            }
            let n1 = DutchSnowball.stem(&cur).into_owned();
            assert_eq!(cur, n1, "did not converge on {w:?}");
        }
    }

    #[test]
    fn gem_guard_suppresses_en_strip() {
        // A word ending in `gemen` — the guard suppresses the -en strip.
        // Not a real Dutch word, but the algorithm's rule is what we're
        // testing. `gemen` = g e m e n.
        //   R1 adjusted ≥ 3. R1 without adj: g non-v, e v, m non-v at 2.
        //     R1 = 3. Adjusted = 3.
        //   Step 1: `en` at end. In R1 (position 3 >= 3). Preceded by
        //     `m` (non-v valid en-ending). But 3 chars before end
        //     `gem` → guard fires, no delete. Result: `gemen`.
        assert_eq!(s("gemen"), "gemen");
    }

    #[test]
    fn y_after_vowel_is_marked_and_restored() {
        // `partij` → no `y`, this doesn't test. Try `bijoux`? Loanword.
        // Try `bay` (short Dutch/English loan). `bay` → b a y.
        //   Prelude: `y` at 2 preceded by `a` (v). Mark Y. → b a Y.
        //   R1: b non-v, a v, Y non-v at 2. R1 = 3.
        //   No suffix rules fire (word too short, no en/e/s at end).
        //   Actually `s` doesn't apply. Result after postlude: `bay`.
        assert_eq!(s("bay"), "bay");
    }

    #[test]
    fn i_between_vowels_is_marked_and_restored() {
        // `keien` → k e i e n (5 chars).
        //   Prelude: `i` at 2 between `e` (v) and `e` (v). Mark → keIen.
        //   R1: k non-v, e v, I non-v at 2. R1 = 3.
        //   Step 1: `en` at end (position 3). 3 >= 3. Preceded by `I`
        //     (non-v, valid en-ending). Not preceded by `gem`. Delete
        //     → keI. Undouble? No. Result after postlude: `kei`.
        assert_eq!(s("keien"), "kei");
    }
}
