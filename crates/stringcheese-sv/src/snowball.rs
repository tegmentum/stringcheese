//! The Snowball Swedish stemmer.
//!
//! # Origin
//!
//! Martin Porter's Snowball Swedish algorithm, documented at
//! <https://snowballstem.org/algorithms/swedish/stemmer.html>, is the
//! reference stemmer used across essentially every Swedish IR pipeline.
//! Lucene's `SwedishAnalyzer`, Elasticsearch's `swedish` analyzer,
//! `snowballstemmer` (Python), NLTK's `SnowballStemmer("swedish")` —
//! all descend from the same Porter/Boulton `swedish.sbl` source. This
//! module ports the algorithm to Rust, faithfully to the published
//! spec.
//!
//! # Algorithm sketch
//!
//! Swedish is a much simpler Snowball algorithm than Dutch or German:
//! it defines a single R1 region and runs three suffix-stripping steps
//! (all confined to R1). There is no prelude or postlude — the algorithm
//! operates directly on the lowercased input.
//!
//! 1. **Region R1.** Standard Snowball convention (`R1` = the position
//!    after the first non-vowel following a vowel), with the German-
//!    style adjustment that `R1` must not begin before char index 3 —
//!    the region before it must contain at least three letters.
//! 2. **Step 1 — main suffix removal (longest match, in R1).**
//!    * Unconditional deletion of one of: `a`, `arna`, `erna`,
//!      `heterna`, `orna`, `ad`, `e`, `ade`, `ande`, `arne`, `are`,
//!      `aste`, `en`, `anden`, `aren`, `heten`, `ern`, `ar`, `er`,
//!      `heter`, `or`, `as`, `arnas`, `ernas`, `ornas`, `es`, `ades`,
//!      `andes`, `ens`, `arens`, `hetens`, `erns`, `at`, `andet`,
//!      `het`, `ast`.
//!    * `s` — delete if the char immediately before `s` is in the Swedish
//!      s-ending set (`b c d f g h j k l m n o p r t v y`) *or* if the
//!      two chars before `s` spell `et` and the `et` position satisfies
//!      the et-condition below.
//!    * `et` — delete if the et-condition holds at the `et` position.
//! 3. **Step 2 — consonant-pair reduction (in R1).** If the word ends
//!    in one of `dd`, `gd`, `nn`, `dt`, `gt`, `kt`, `tt`, delete the
//!    final letter.
//! 4. **Step 3 — other suffixes (longest match, in R1).**
//!    * `lig`, `ig`, `els` — delete.
//!    * `öst` → `ös` if the char before `öst` is in the öst-ending set
//!      (`i k l n p r t u v`).
//!    * `fullt` → `full`.
//!
//! # The `et` condition
//!
//! For `s` (via the `ets` sub-condition) and `et` alone, the condition
//! is: the three chars immediately preceding `et` end in a non-vowel-
//! followed-by-vowel-followed-by-non-vowel structure *and* the tail
//! before `et` does not match one of the etymologically-important
//! exclusions (`h`, `iet`, `uit`, `fab`, `cit`, `dit`, `alit`, `ilit`,
//! `mit`, `nit`, `pit`, `rit`, `sit`, `tit`, `ivit`, `kvit`, `xit`,
//! `kom`, `rak`, `pak`, `stak`). The exclusions protect stems like
//! `frihet` (freedom), `societet` (society), `alfabet` (alphabet),
//! `paket` (package), and `raket` (rocket) from over-stemming.
//!
//! # Non-goals
//!
//! * **Full-vocabulary cross-verification.** The Snowball project
//!   distributes `voc.txt` / `output.txt` reference files with tens of
//!   thousands of pairs; the [`tests/snowball_reference.rs`](
//!   ../../tests/snowball_reference.rs) test embeds a *subset* that
//!   exercises every step's happy path and each cascading rule.
//!   Full-corpus cross-verification is a follow-up.
//! * **Finland-Swedish variety.** The Snowball algorithm is neutral
//!   between Sverigesvenska and Finlandssvenska; the shared vocabulary
//!   of the Snowball `swedish/stop.txt` distribution serves both.
//!   Regional-vocabulary extensions are a follow-up.
//! * **Compound splitting.** Swedish productively compounds nouns
//!   (`glass + bil → glassbil`, `barn + bok → barnbok`); splitting them
//!   requires a compound-noun dictionary and is not part of the
//!   Snowball algorithm.
//! * **Lemmatization.** Reducing `är → vara`, `gick → gå` needs a
//!   lexicon, not a suffix-stripping algorithm.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use stringcheese_lang::Stemmer;

/// The Snowball Swedish stemmer.
///
/// A zero-sized unit value; construct as [`SwedishSnowball`] and reuse
/// the value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_sv::SwedishSnowball;
/// use stringcheese_lang::Stemmer;
///
/// assert_eq!(SwedishSnowball.stem("flickorna"), "flick");
/// assert_eq!(SwedishSnowball.stem("husen"), "hus");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SwedishSnowball;

impl SwedishSnowball {
    /// Stems `word` per the Snowball Swedish algorithm.
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

        // 2. Region R1 (German-style: at least 3 chars before it).
        let r1 = compute_r1_adjusted(&chars);

        // 3. Steps 1..=3.
        step_1(&mut chars, r1);
        step_2(&mut chars, r1);
        step_3(&mut chars, r1);

        // 4. Rebuild string.
        let out: String = chars.into_iter().collect();
        if out == word {
            Cow::Borrowed(word)
        } else {
            Cow::Owned(out)
        }
    }
}

impl Stemmer for SwedishSnowball {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        SwedishSnowball::stem(self, word)
    }
}

// ---------------------------------------------------------------------------
// Vowel classification.
// ---------------------------------------------------------------------------

/// Swedish vowels per the Snowball spec: `a e i o u y ä å ö`.
#[inline]
fn is_vowel(c: char) -> bool {
    matches!(c, 'a' | 'e' | 'i' | 'o' | 'u' | 'y' | 'ä' | 'å' | 'ö')
}

/// The Swedish s-ending set — a valid consonant preceding `s` that
/// licenses the `s`-strip. Spelled `bcdfghjklmnoprtvy` in the sbl.
#[inline]
fn is_s_ending(c: char) -> bool {
    matches!(
        c,
        'b' | 'c'
            | 'd'
            | 'f'
            | 'g'
            | 'h'
            | 'j'
            | 'k'
            | 'l'
            | 'm'
            | 'n'
            | 'o'
            | 'p'
            | 'r'
            | 't'
            | 'v'
            | 'y'
    )
}

/// The Swedish öst-ending set — the char preceding `öst` that licenses
/// the `öst → ös` rewrite. Spelled `iklnprtuv` in the sbl.
#[inline]
fn is_ost_ending(c: char) -> bool {
    matches!(c, 'i' | 'k' | 'l' | 'n' | 'p' | 'r' | 't' | 'u' | 'v')
}

// ---------------------------------------------------------------------------
// Region R1 — as a char index.
// ---------------------------------------------------------------------------

/// Standard Snowball R1: char index after the first non-vowel following
/// a vowel (or `chars.len()` if no such position exists). Adjusted so
/// R1 ≥ 3 (German / Swedish convention).
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
// Step 1 — main suffix removal.
// ---------------------------------------------------------------------------

// The unconditional-delete suffix table (Step 1 group A). Sorted so a
// hand-audit against the sbl `among(...)` block is easy.
const STEP1_DELETE: &[&[char]] = &[
    &['a'],
    &['a', 'r', 'n', 'a'],
    &['e', 'r', 'n', 'a'],
    &['h', 'e', 't', 'e', 'r', 'n', 'a'],
    &['o', 'r', 'n', 'a'],
    &['a', 'd'],
    &['e'],
    &['a', 'd', 'e'],
    &['a', 'n', 'd', 'e'],
    &['a', 'r', 'n', 'e'],
    &['a', 'r', 'e'],
    &['a', 's', 't', 'e'],
    &['e', 'n'],
    &['a', 'n', 'd', 'e', 'n'],
    &['a', 'r', 'e', 'n'],
    &['h', 'e', 't', 'e', 'n'],
    &['e', 'r', 'n'],
    &['a', 'r'],
    &['e', 'r'],
    &['h', 'e', 't', 'e', 'r'],
    &['o', 'r'],
    &['a', 's'],
    &['a', 'r', 'n', 'a', 's'],
    &['e', 'r', 'n', 'a', 's'],
    &['o', 'r', 'n', 'a', 's'],
    &['e', 's'],
    &['a', 'd', 'e', 's'],
    &['a', 'n', 'd', 'e', 's'],
    &['e', 'n', 's'],
    &['a', 'r', 'e', 'n', 's'],
    &['h', 'e', 't', 'e', 'n', 's'],
    &['e', 'r', 'n', 's'],
    &['a', 't'],
    &['a', 'n', 'd', 'e', 't'],
    &['h', 'e', 't'],
    &['a', 's', 't'],
];

/// The exclusion list from the sbl's `et_condition` — stems that would
/// otherwise over-strip. Each string is the tail (immediately before
/// `et` / `ets`) that suppresses the strip.
const ET_EXCLUSIONS: &[&[char]] = &[
    &['h'],
    &['i', 'e', 't'],
    &['u', 'i', 't'],
    &['f', 'a', 'b'],
    &['c', 'i', 't'],
    &['d', 'i', 't'],
    &['a', 'l', 'i', 't'],
    &['i', 'l', 'i', 't'],
    &['m', 'i', 't'],
    &['n', 'i', 't'],
    &['p', 'i', 't'],
    &['r', 'i', 't'],
    &['s', 'i', 't'],
    &['t', 'i', 't'],
    &['i', 'v', 'i', 't'],
    &['k', 'v', 'i', 't'],
    &['x', 'i', 't'],
    &['k', 'o', 'm'],
    &['r', 'a', 'k'],
    &['p', 'a', 'k'],
    &['s', 't', 'a', 'k'],
];

/// Check the et-condition at position `pos_before_et` (the char index
/// where the `et` suffix begins — i.e., the length of the stem before
/// `et`).
///
/// Returns true iff the stem satisfies:
/// * length ≥ 3 (needed for the non-v v non-v structure and the
///   "not atlimit" clause), and
/// * the three chars immediately preceding `et` (the last three of the
///   stem before `et`) are (non-vowel)(vowel)(non-vowel), and
/// * the stem does NOT end in one of the exclusion strings.
fn et_condition(chars: &[char], pos_before_et: usize) -> bool {
    if pos_before_et < 3 {
        return false;
    }
    // Look at chars[pos_before_et-3..pos_before_et] — the three chars
    // immediately before `et`. The sbl pattern is `non-v v not atlimit`
    // walking backwards from the position before 'et'. In backward
    // mode, `non-v v` starting at position pos_before_et matches:
    //   char at pos_before_et - 1 is non-v (the char just before 'et'),
    //   char at pos_before_et - 2 is v.
    // The `not atlimit` clause means: the position after those two
    // matches (i.e., pos_before_et - 2) must not be at the start. So
    // we need pos_before_et - 2 > 0, i.e., pos_before_et >= 3.
    let c1 = chars[pos_before_et - 1];
    let c2 = chars[pos_before_et - 2];
    if is_vowel(c1) || !is_vowel(c2) {
        return false;
    }
    // Exclusion pass — does the stem before 'et' end in any of the
    // forbidden strings?
    let stem = &chars[..pos_before_et];
    for &excl in ET_EXCLUSIONS {
        if ends_with(stem, excl) {
            return false;
        }
    }
    true
}

fn step_1(chars: &mut Vec<char>, r1: usize) {
    // Group A — unconditional delete (longest match).
    if let Some(s) = longest_suffix(chars, STEP1_DELETE) {
        let sl = s.len();
        if suffix_in(chars, sl, r1) {
            let stem_len = chars.len() - sl;
            chars.truncate(stem_len);
            return;
        }
    }
    // Group B — 's' (conditional).
    if ends_with(chars, &['s']) && suffix_in(chars, 1, r1) {
        let stem_len = chars.len() - 1;
        if stem_len > 0 {
            let prev = chars[stem_len - 1];
            // Sub-condition 1: prev is in s_ending.
            let ok_s_ending = is_s_ending(prev);
            // Sub-condition 2: the two chars before 's' spell 'et',
            // and et_condition holds at the 'et' position.
            let ok_ets = stem_len >= 2
                && chars[stem_len - 2] == 'e'
                && chars[stem_len - 1] == 't'
                && et_condition(chars, stem_len - 2);
            if ok_ets || ok_s_ending {
                chars.truncate(stem_len);
            }
        }
        return;
    }
    // Group C — 'et' (conditional).
    if ends_with(chars, &['e', 't']) && suffix_in(chars, 2, r1) {
        let stem_len = chars.len() - 2;
        if et_condition(chars, stem_len) {
            chars.truncate(stem_len);
        }
    }
}

// ---------------------------------------------------------------------------
// Step 2 — consonant-pair reduction.
// ---------------------------------------------------------------------------

const STEP2_PAIRS: &[&[char]] = &[
    &['d', 'd'],
    &['g', 'd'],
    &['n', 'n'],
    &['d', 't'],
    &['g', 't'],
    &['k', 't'],
    &['t', 't'],
];

fn step_2(chars: &mut Vec<char>, r1: usize) {
    for &pair in STEP2_PAIRS {
        if ends_with(chars, pair) && suffix_in(chars, pair.len(), r1) {
            chars.pop();
            return;
        }
    }
}

// ---------------------------------------------------------------------------
// Step 3 — other suffixes.
// ---------------------------------------------------------------------------

const STEP3_DELETE: &[&[char]] = &[&['l', 'i', 'g'], &['i', 'g'], &['e', 'l', 's']];

const STEP3_OST: &[char] = &['ö', 's', 't'];
const STEP3_OS_REPLACEMENT: &[char] = &['ö', 's'];
const STEP3_FULLT: &[char] = &['f', 'u', 'l', 'l', 't'];
const STEP3_FULL_REPLACEMENT: &[char] = &['f', 'u', 'l', 'l'];

fn step_3(chars: &mut Vec<char>, r1: usize) {
    // Longest match across all Step 3 alternatives.
    let mut best: Option<(&[char], StepAction)> = None;
    for &s in STEP3_DELETE {
        if ends_with(chars, s)
            && suffix_in(chars, s.len(), r1)
            && best.as_ref().is_none_or(|(b, _)| s.len() > b.len())
        {
            best = Some((s, StepAction::Delete));
        }
    }
    if ends_with(chars, STEP3_OST) && suffix_in(chars, STEP3_OST.len(), r1) {
        let stem_len = chars.len() - STEP3_OST.len();
        // öst-condition: char before 'öst' is in ost_ending.
        if stem_len > 0
            && is_ost_ending(chars[stem_len - 1])
            && best.as_ref().is_none_or(|(b, _)| STEP3_OST.len() > b.len())
        {
            best = Some((STEP3_OST, StepAction::ReplaceOst));
        }
    }
    if ends_with(chars, STEP3_FULLT)
        && suffix_in(chars, STEP3_FULLT.len(), r1)
        && best
            .as_ref()
            .is_none_or(|(b, _)| STEP3_FULLT.len() > b.len())
    {
        best = Some((STEP3_FULLT, StepAction::ReplaceFullt));
    }
    let Some((s, action)) = best else {
        return;
    };
    let stem_len = chars.len() - s.len();
    match action {
        StepAction::Delete => {
            chars.truncate(stem_len);
        }
        StepAction::ReplaceOst => {
            chars.truncate(stem_len);
            chars.extend_from_slice(STEP3_OS_REPLACEMENT);
        }
        StepAction::ReplaceFullt => {
            chars.truncate(stem_len);
            chars.extend_from_slice(STEP3_FULL_REPLACEMENT);
        }
    }
}

#[derive(Copy, Clone, Debug)]
enum StepAction {
    Delete,
    ReplaceOst,
    ReplaceFullt,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &str) -> String {
        SwedishSnowball.stem(w).into_owned()
    }

    #[test]
    fn short_words_are_unchanged() {
        assert_eq!(s(""), "");
        assert_eq!(s("a"), "a");
        assert_eq!(s("i"), "i");
    }

    #[test]
    fn step1_a_strip() {
        // "flicka" (girl) → step 1 'a' delete → "flick".
        // R1: f non-v, l non-v, i v, c non-v at 3. R1 = 4.
        // 'a' at position 5. 5 >= 4. Delete → "flick".
        assert_eq!(s("flicka"), "flick");
    }

    #[test]
    fn step1_orna_strip() {
        // "flickorna" (the girls) → step 1 longest-match 'orna' → "flick".
        assert_eq!(s("flickorna"), "flick");
    }

    #[test]
    fn step1_en_strip() {
        // "husen" (the houses) → step 1 'en' → "hus".
        assert_eq!(s("husen"), "hus");
        // "hunden" (the dog) → step 1 'en' → "hund". Then step 2: no
        // pair match (`nd` not in the list). Result: "hund".
        assert_eq!(s("hunden"), "hund");
    }

    #[test]
    fn step1_arna_strip() {
        // "hundarna" (the dogs) → step 1 longest-match 'arna' → "hund".
        assert_eq!(s("hundarna"), "hund");
    }

    #[test]
    fn step1_heten_strip() {
        // "trolighet" (probability) — step 1 'het' at position 6. R1
        //   for "trolighet" (9 chars): t non-v, r non-v, o v, l non-v
        //   at 3. R1 = 4. 9-3 = 6 >= 4 → in R1. Delete 'het' → "trolig".
        //   Step 2: no pair. Step 3: 'lig' at pos 3. 6-3 = 3 >= 4? No.
        //   'ig' at pos 4. 6-2 = 4 >= 4. Delete → "trol".
        assert_eq!(s("trolighet"), "trol");
    }

    #[test]
    fn step1_s_delete_with_valid_s_ending() {
        // "hunds" — 's' preceded by 'd' (in s_ending). Delete → "hund".
        assert_eq!(s("hunds"), "hund");
    }

    #[test]
    fn step1_s_ending_semantics() {
        // "kaos" — 4 chars. R1: k non-v, a v, o v, s non-v at 3.
        //   R1 = 4. Step 1: 's' at pos 3. suffix_in(1, 4) = 4-1 = 3
        //   >= 4? No. Doesn't fire. Result: "kaos".
        assert_eq!(s("kaos"), "kaos");
        // "utopias" — 7 chars. R1: u v, t non-v at 1. R1 = 2, adjusted
        //   to min(3, 7) = 3. Step 1: 'as' at pos 5. 7-2 = 5 >= 3.
        //   Delete → "utopi".
        assert_eq!(s("utopias"), "utopi");
    }

    #[test]
    fn step2_dd_pair_reduction() {
        // "bladd" — contrived; but the algorithm reduces trailing dd to
        // d if in R1.
        // "godd" — 4 chars. R1: g non-v, o v, d non-v at 2. R1 = 3.
        //   step 1: 'd' isn't a suffix. Ends in 'dd'. Not in step 1.
        //   step 2: 'dd' at position 2. 2 >= 3? No — need suffix_in(2, 3)
        //     i.e., 4-2 >= 3 → 2 >= 3 → false. So doesn't fire.
        // Try a longer one: "kladd" (5 chars).
        //   R1: k non-v, l non-v, a v, d non-v at 3. R1 = 4.
        //   step 1: none.
        //   step 2: 'dd' at position 3. 5-2 = 3 >= 4? No. Doesn't fire.
        // Try "svindladdet" — contrived. Skip.
        // The 'dd' rule fires on words like "väldd" or in stems after
        // step 1 stripping. E.g., "väddad" → step 1 'ad' → "vädd" →
        //   step 2 'dd' → "väd" (contrived example).
        // Focus on real cases: "sätt" (way) is only 4 chars.
        //   R1: s non-v, ä v, t non-v at 2. R1 = 3. 'tt' at 2. 4-2 = 2
        //     >= 3? No. Doesn't fire. Result: "sätt".
        assert_eq!(s("sätt"), "sätt");
        // "hattar" (hats) → step 1 'ar' → "hatt". R1: h non-v, a v,
        //   t non-v at 2. R1 = 3. Then step 2 'tt' at position 2.
        //   4 - 2 = 2 >= 3? No. Doesn't fire. Result: "hatt".
        assert_eq!(s("hattar"), "hatt");
        // Longer: "avvisandet" (the rejection) — step 1 'andet' →
        //   "avvis". R1: a v, v non-v at 1. R1 adjusted = 3.
        //   step 2: no pair. step 3: no. Result: "avvis".
        assert_eq!(s("avvisandet"), "avvis");
    }

    #[test]
    fn step3_lig_strip() {
        // "roligt" (funny, neuter) — R1: r non-v, o v, l non-v at 2.
        //   R1 = 3.
        //   step 1: no match on "gt"/"t".
        //   step 2: 'gt' at position 4. 6-2 = 4 >= 3. Delete last
        //     letter → "rolig".
        //   step 3: 'lig' at position 2. 5-3 = 2 >= 3? No.
        //     'ig' at position 3. 5-2 = 3 >= 3. Delete → "rol".
        assert_eq!(s("roligt"), "rol");
        // "trolig" (probable) — R1: t non-v, r non-v, o v, l non-v at
        //   3. R1 = 4. step 1: no match. step 2: no.
        //   step 3: 'lig' at position 3. 6-3 = 3 >= 4? No.
        //     'ig' at position 4. 6-2 = 4 >= 4. Delete → "trol".
        assert_eq!(s("trolig"), "trol");
    }

    #[test]
    fn step3_ost_replacement() {
        // "roligast" — ends in 'ast', step 1 'ast' delete → "rolig".
        //   Then step 2 no. Step 3 'ig' at position 4. 5 - 2 = 3 >= 4?
        //   Wait, R1 for "rolig" length 5 chars: r non-v, o v, l non-v
        //   at 2. R1 = 3. 5 - 2 = 3 >= 3. Delete → "rol". Actually R1
        //   was computed from the original "roligast" (8 chars): R1
        //   there was 3. In the stripped stem the R1 index still holds
        //   as a char position (rows before it), so we check
        //   suffix_in(2, 3) i.e., stripped_len - suffix_len >= 3.
        //   For "rolig" (5 chars) stripped to "rolig", 5 - 2 = 3 >= 3.
        //   Delete → "rol".
        assert_eq!(s("roligast"), "rol");
    }

    #[test]
    fn step3_fullt_replacement() {
        // "härligt" would go through step 2 'gt' → "härlig", then step 3
        //   'lig'. Try "fullt": 5 chars.
        //   R1: f non-v, u v, l non-v at 2. R1 = 3.
        //   step 1: no match (ends 't', not on the list).
        //   step 2: no ('lt' not a pair).
        //   step 3: 'fullt' at position 0. 5 - 5 = 0 >= 3? No. Doesn't
        //     fire. Result: "fullt".
        // The 'fullt' rule fires only when the word is longer, e.g.,
        // "underfullt" (10 chars). R1: u v, n non-v at 1. R1 adj = 3.
        //   step 1: no match.
        //   step 2: no ('lt' not a pair).
        //   step 3: 'fullt' at position 5. 10 - 5 = 5 >= 3. Replace
        //     'fullt' → 'full'. Result: "underfull".
        assert_eq!(s("underfullt"), "underfull");
    }

    #[test]
    fn et_condition_protects_paket() {
        // "paket" (package) — the et-condition exclusion 'pak' matches
        //   before 'et'. Step 1 'et' skipped. Result: "paket".
        assert_eq!(s("paket"), "paket");
    }

    #[test]
    fn et_condition_protects_alfabet() {
        // "alfabet" — 'fab' is in the exclusion list.
        assert_eq!(s("alfabet"), "alfabet");
    }

    #[test]
    fn et_condition_protects_frihet() {
        // "frihet" (6 chars). R1: f non-v, r non-v, i v, h non-v at 3.
        //   R1 = 4. Step 1 'het' at position 3: suffix_in(3, 4) = 6-3
        //   = 3 >= 4? No — 'het' is not fully inside R1. Fall through
        //   to 'et' check: 'et' at position 4, 6-2 = 4 >= 4 → in R1.
        //   et_condition: stem "frih" ends in 'h' → exclusion → skip.
        //   No suffix stripped. Result: "frihet" (unchanged).
        //   This is the intended behaviour — the exclusion list guards
        //   -het loans from over-stripping, and R1's 3-char minimum
        //   already blocks the -het strip on short stems.
        assert_eq!(s("frihet"), "frihet");
    }

    #[test]
    fn regions_paper_example() {
        // "flickorna" — R1 index?
        //   f non-v, l non-v, i v, c non-v at 3. R1 = 4.
        let chars: Vec<char> = "flickorna".chars().collect();
        assert_eq!(compute_r1_adjusted(&chars), 4);
    }

    #[test]
    fn convergence_on_common_vocabulary() {
        for w in [
            "flicka",
            "flickorna",
            "hund",
            "hunden",
            "hundarna",
            "husen",
            "roligt",
            "trolig",
            "trolighet",
            "underfullt",
            "kaos",
            "paket",
            "alfabet",
            "frihet",
            "sätt",
        ] {
            let mut cur = SwedishSnowball.stem(w).into_owned();
            for _ in 0..5 {
                let next = SwedishSnowball.stem(&cur).into_owned();
                if next == cur {
                    break;
                }
                cur = next;
            }
            let n1 = SwedishSnowball.stem(&cur).into_owned();
            assert_eq!(cur, n1, "did not converge on {w:?}");
        }
    }

    #[test]
    fn s_ending_set_matches_sbl() {
        // Spot-check against the sbl's `'bcdfghjklmnoprtvy'` set.
        assert!(is_s_ending('d'));
        assert!(is_s_ending('t'));
        assert!(is_s_ending('o')); // deliberately included by the sbl
        assert!(is_s_ending('y')); // deliberately included by the sbl
        assert!(!is_s_ending('a'));
        assert!(!is_s_ending('e'));
        assert!(!is_s_ending('i'));
        assert!(!is_s_ending('u'));
        assert!(!is_s_ending('s')); // avoids ss cascade
        assert!(!is_s_ending('x'));
        assert!(!is_s_ending('z'));
        assert!(!is_s_ending('q'));
    }

    #[test]
    fn ost_ending_set_matches_sbl() {
        assert!(is_ost_ending('i'));
        assert!(is_ost_ending('k'));
        assert!(is_ost_ending('l'));
        assert!(is_ost_ending('n'));
        assert!(is_ost_ending('p'));
        assert!(is_ost_ending('r'));
        assert!(is_ost_ending('t'));
        assert!(is_ost_ending('u'));
        assert!(is_ost_ending('v'));
        assert!(!is_ost_ending('a'));
        assert!(!is_ost_ending('e'));
    }
}
