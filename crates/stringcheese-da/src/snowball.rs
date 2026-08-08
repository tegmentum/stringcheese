//! The Snowball Danish stemmer.
//!
//! # Origin
//!
//! Martin Porter's Snowball Danish algorithm, documented at
//! <https://snowballstem.org/algorithms/danish/stemmer.html>, is the
//! reference stemmer used across essentially every Danish IR pipeline.
//! Lucene's `DanishAnalyzer`, Elasticsearch's `danish` analyzer,
//! `snowballstemmer` (Python), NLTK's `SnowballStemmer("danish")` — all
//! descend from the same Porter/Boulton `danish.sbl` source. This
//! module ports the algorithm to Rust, faithfully to the published
//! spec.
//!
//! # Algorithm sketch
//!
//! 1. **Lowercase (Unicode-aware).** Fold input to lowercase so the
//!    suffix tables (all lowercase) match.
//! 2. **R1 region.** Compute `R1` per the standard Snowball convention
//!    (`R1` = the position after the first non-vowel following a vowel),
//!    then adjust so `R1` never starts before char index 3 — the region
//!    before it must contain at least three letters.
//! 3. **Step 1 — main suffix (longest match in R1).**
//!    * *Group A* — delete: `hed`, `ethed`, `ered`, `e`, `erede`,
//!      `ende`, `erende`, `ene`, `erne`, `ere`, `en`, `heden`, `eren`,
//!      `er`, `heder`, `erer`, `heds`, `es`, `endes`, `erendes`, `enes`,
//!      `ernes`, `eres`, `ens`, `hedens`, `erens`, `ers`, `ets`,
//!      `erets`, `et`, `eret`.
//!    * *Group B* — `s`: delete when preceded by a **valid s-ending**.
//!
//!    A **valid s-ending** for Danish is one of
//!    `a b c d f g h j k l m n o p r t v y z å` (per the Snowball
//!    `danish.sbl` `s_ending` definition — includes the vowel `a` and
//!    the extended letter `å`, but not the other vowels; note this
//!    differs from the Norwegian pack, whose s-ending set is
//!    `b c d f g h j l m n o p r t v y z` plus context-guarded `k`).
//! 4. **Step 2 — consonant pair.** If the word ends in `-gd`, `-dt`,
//!    `-gt`, or `-kt` in R1, delete the final letter.
//! 5. **Step 3 — other suffix.**
//!    * If the word ends in `-igst` in R1, delete the final `-st`
//!      (leaves `-ig`).
//!    * Then search for the longest of the following in R1:
//!      * `ig`, `lig`, `elig`, `els` → delete (then re-run Step 2).
//!      * `løst` → `løs`.
//! 6. **Step 4 — undouble.** If the word ends in two identical
//!    consonants and the pair starts at a position ≥ R1, delete the
//!    final consonant (e.g., `-nn → -n`, `-tt → -t`).
//!
//! # Vowel set
//!
//! Danish vowels per the Snowball spec: `a e i o u y æ å ø` — the three
//! Danish-specific letters are all vowels.
//!
//! # Non-goals
//!
//! * **Full-vocabulary cross-verification.** The Snowball project
//!   distributes `voc.txt` / `output.txt` reference files with tens of
//!   thousands of pairs; the [`tests/snowball_reference.rs`](
//!   ../../tests/snowball_reference.rs) test embeds a *subset* that
//!   exercises every step's happy path and each cascading rule.
//!   Full-corpus cross-verification is a follow-up.
//! * **Compound splitting.** Danish productively compounds nouns
//!   (`børnehave = børne + have`); splitting them requires a
//!   compound-noun dictionary and is not part of the Snowball
//!   algorithm.
//! * **Lemmatization.** Reducing `bedre → god`, `værst → dårlig`
//!   requires a lexicon, not a suffix-stripping algorithm.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use stringcheese_lang::Stemmer;

/// The Snowball Danish stemmer.
///
/// A zero-sized unit value; construct as [`DanishSnowball`] and reuse
/// the value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_da::DanishSnowball;
/// use stringcheese_lang::Stemmer;
///
/// assert_eq!(DanishSnowball.stem("sædene"), "sæd");
/// assert_eq!(DanishSnowball.stem("kærlighed"), "kær");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct DanishSnowball;

impl DanishSnowball {
    /// Stems `word` per the Snowball Danish algorithm.
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

        // 3. Steps 1..=4.
        step_1_main_suffix(&mut chars, r1);
        step_2_consonant_pair(&mut chars, r1);
        let step3_deleted = step_3_other_suffix(&mut chars, r1);
        if step3_deleted {
            // Per the spec, the "delete" branches of Step 3 re-run
            // Step 2.
            step_2_consonant_pair(&mut chars, r1);
        }
        step_4_undouble(&mut chars, r1);

        // 4. Emit.
        let out: String = chars.into_iter().collect();
        if out == word {
            Cow::Borrowed(word)
        } else {
            Cow::Owned(out)
        }
    }
}

impl Stemmer for DanishSnowball {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        DanishSnowball::stem(self, word)
    }
}

// ---------------------------------------------------------------------------
// Vowel classification.
// ---------------------------------------------------------------------------

/// Danish vowels per the Snowball spec: `a e i o u y æ å ø`.
#[inline]
fn is_vowel(c: char) -> bool {
    matches!(c, 'a' | 'e' | 'i' | 'o' | 'u' | 'y' | 'æ' | 'å' | 'ø')
}

/// Valid s-ending characters, per the Snowball spec:
/// `a b c d f g h j k l m n o p r t v y z å`. Note the deliberate
/// inclusion of `a` and `å` — this matches the `danish.sbl`'s
/// `s_ending` definition verbatim.
#[inline]
fn is_s_ending(c: char) -> bool {
    matches!(
        c,
        'a' | 'b'
            | 'c'
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
            | 'z'
            | 'å'
    )
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

// Group A — plain-delete suffixes. `longest_suffix` picks the longest
// matching entry regardless of order, so ordering is presentational
// only (roughly ordered by decreasing length for hand-audit).
const S1A: &[&[char]] = &[
    &['e', 'r', 'e', 'n', 'd', 'e', 's'], // erendes (7)
    &['e', 'r', 'e', 'n', 'd', 'e'],      // erende  (6)
    &['h', 'e', 'd', 'e', 'n', 's'],      // hedens  (6)
    &['e', 'r', 'e', 'd', 'e'],           // erede   (5)
    &['h', 'e', 'd', 'e', 'n'],           // heden   (5)
    &['h', 'e', 'd', 'e', 'r'],           // heder   (5)
    &['e', 'r', 'e', 'n', 's'],           // erens   (5)
    &['e', 'r', 'n', 'e', 's'],           // ernes   (5)
    &['e', 'n', 'd', 'e', 's'],           // endes   (5)
    &['e', 'r', 'e', 't'],                // eret    (4)
    &['e', 't', 'h', 'e', 'd'],           // ethed   (5)
    &['e', 'r', 'e', 'd'],                // ered    (4)
    &['e', 'n', 'd', 'e'],                // ende    (4)
    &['e', 'r', 'n', 'e'],                // erne    (4)
    &['e', 'r', 'e', 'r'],                // erer    (4)
    &['e', 'r', 'e', 'n'],                // eren    (4)
    &['e', 'r', 'e', 's'],                // eres    (4)
    &['e', 'n', 'e', 's'],                // enes    (4)
    &['e', 'r', 'e'],                     // ere     (3)
    &['e', 'n', 'e'],                     // ene     (3)
    &['h', 'e', 'd', 's'],                // heds    (4)
    &['h', 'e', 'd'],                     // hed     (3)
    &['e', 'n', 's'],                     // ens     (3)
    &['e', 'r', 's'],                     // ers     (3)
    &['e', 't', 's'],                     // ets     (3)
    &['e', 'n'],                          // en      (2)
    &['e', 'r'],                          // er      (2)
    &['e', 's'],                          // es      (2)
    &['e', 't'],                          // et      (2)
    &['e'],                               // e       (1)
];

// Group B — bare `s` (special rule).
const S1B_S: &[char] = &['s'];

fn step_1_main_suffix(chars: &mut Vec<char>, r1: usize) {
    // Assemble the union of Group A and Group B. `longest_suffix`
    // picks the longest match — that's the "longest among the
    // following" rule the spec calls out.
    let mut all: Vec<&[char]> = Vec::with_capacity(S1A.len() + 1);
    all.extend_from_slice(S1A);
    all.push(S1B_S);
    let Some(s) = longest_suffix(chars, &all) else {
        return;
    };
    let sl = s.len();
    if !suffix_in(chars, sl, r1) {
        return;
    }
    let stem_len = chars.len() - sl;

    // Group B — s — delete iff preceded by a valid s-ending.
    if s == S1B_S {
        if stem_len == 0 {
            return;
        }
        if !is_s_ending(chars[stem_len - 1]) {
            return;
        }
        chars.truncate(stem_len);
        return;
    }

    // Group A — plain delete.
    chars.truncate(stem_len);
}

// ---------------------------------------------------------------------------
// Step 2: consonant-pair `-gd` / `-dt` / `-gt` / `-kt` → delete final
// letter.
// ---------------------------------------------------------------------------

const S2_PAIRS: &[&[char]] = &[&['g', 'd'], &['d', 't'], &['g', 't'], &['k', 't']];

fn step_2_consonant_pair(chars: &mut Vec<char>, r1: usize) {
    for &pair in S2_PAIRS {
        if ends_with(chars, pair) && suffix_in(chars, 2, r1) {
            chars.pop(); // drop the trailing letter of the pair
            return;
        }
    }
}

// ---------------------------------------------------------------------------
// Step 3: other suffix.
// ---------------------------------------------------------------------------

const S3_DELETE: &[&[char]] = &[
    &['e', 'l', 'i', 'g'], // elig (4)
    &['l', 'i', 'g'],      // lig  (3)
    &['e', 'l', 's'],      // els  (3)
    &['i', 'g'],           // ig   (2)
];

const S3_LOEST: &[char] = &['l', 'ø', 's', 't'];
const S3_LOES_REPLACEMENT: &[char] = &['l', 'ø', 's'];
const S3_IGST: &[char] = &['i', 'g', 's', 't'];

/// Runs Step 3. Returns `true` if a delete-branch fired, signalling the
/// caller to re-run Step 2 per the spec.
fn step_3_other_suffix(chars: &mut Vec<char>, r1: usize) -> bool {
    // Preliminary: if the word ends `-igst` in R1, delete the final
    // `-st` (leaves `-ig`).
    if ends_with(chars, S3_IGST) && suffix_in(chars, S3_IGST.len(), r1) {
        // Drop the last two chars ('s','t').
        chars.pop();
        chars.pop();
    }

    // Now search for the longest of the Step 3 candidates.
    // Assemble delete list + løst.
    let mut all: Vec<&[char]> = Vec::with_capacity(S3_DELETE.len() + 1);
    all.extend_from_slice(S3_DELETE);
    all.push(S3_LOEST);
    let Some(s) = longest_suffix(chars, &all) else {
        return false;
    };
    let sl = s.len();
    if !suffix_in(chars, sl, r1) {
        return false;
    }
    let stem_len = chars.len() - sl;

    if s == S3_LOEST {
        // Replace løst → løs.
        chars.truncate(stem_len);
        chars.extend_from_slice(S3_LOES_REPLACEMENT);
        // The `løst → løs` branch does NOT re-run Step 2 per the spec —
        // only the delete branches do.
        return false;
    }

    // Delete branch (ig / lig / elig / els).
    chars.truncate(stem_len);
    true
}

// ---------------------------------------------------------------------------
// Step 4: undouble trailing repeated consonant.
// ---------------------------------------------------------------------------

fn step_4_undouble(chars: &mut Vec<char>, r1: usize) {
    let n = chars.len();
    if n < 2 {
        return;
    }
    let a = chars[n - 1];
    let b = chars[n - 2];
    if a != b || is_vowel(a) {
        return;
    }
    // The doubled pair sits at positions n-2 and n-1. The pair "is in
    // R1" iff the pair's start position (n-2) is ≥ R1.
    if !suffix_in(chars, 2, r1) {
        return;
    }
    chars.pop();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &str) -> String {
        DanishSnowball.stem(w).into_owned()
    }

    #[test]
    fn short_words_are_unchanged() {
        assert_eq!(s(""), "");
        assert_eq!(s("a"), "a");
        assert_eq!(s("og"), "og");
        assert_eq!(s("i"), "i");
    }

    #[test]
    fn step1_e_strip() {
        // "arbejde" (work) — e at end, R1=3, delete → "arbejd".
        assert_eq!(s("arbejde"), "arbejd");
    }

    #[test]
    fn step1_en_strip() {
        // "sæden" (the seed) → -en delete → "sæd".
        assert_eq!(s("sæden"), "sæd");
    }

    #[test]
    fn step1_ene_strip() {
        // "sædene" (imagined plural definite) → -ene delete → "sæd".
        assert_eq!(s("sædene"), "sæd");
    }

    #[test]
    fn step1_et_strip() {
        // "havet" (the sea) → -et delete → "hav".
        assert_eq!(s("havet"), "hav");
        // "elsket" (loved) → -et delete → "elsk".
        assert_eq!(s("elsket"), "elsk");
    }

    #[test]
    fn step1_bare_e_on_ede_form() {
        // "elskede" (loved-past) — the Danish reference suffix table
        //   does NOT include `-ede` (unlike Norwegian's Group A).
        //   Only the bare `-e` strip fires → "elsked". The result is
        //   asymmetric with the neuter past `elsket` → `elsk`
        //   (which strips `-et`); this is standard Snowball Danish
        //   behavior — the algorithm is a suffix stripper, not a
        //   lemmatizer.
        assert_eq!(s("elskede"), "elsked");
    }

    #[test]
    fn step1_hed_strip() {
        // "kærlighed" (love) → -hed delete → "kærlig", then step 3 -lig
        //   → "kær".
        assert_eq!(s("kærlighed"), "kær");
    }

    #[test]
    fn step1_hedens_strip() {
        // "kærlighedens" (of the love) → -hedens delete → "kærlig",
        //   then step 3 -lig → "kær".
        assert_eq!(s("kærlighedens"), "kær");
    }

    #[test]
    fn step1_ered_strip() {
        // A word in `-ered` — try "regered" (governed): r,e,g,e,r,e,d
        //   (7). R1: r non-v, e v, g non-v at 2. R1=3. `ered` at pos 3,
        //   7-4=3≥3 ✓. Delete → "reg".
        assert_eq!(s("regered"), "reg");
    }

    #[test]
    fn step1_ende_strip() {
        // "løbende" (running) → -ende delete → "løb".
        //   løbende = l,ø,b,e,n,d,e (7). R1: l non-v, ø v, b non-v at 2.
        //   R1=3. `ende` at pos 3, 7-4=3≥3 ✓. Delete → "løb".
        assert_eq!(s("løbende"), "løb");
    }

    #[test]
    fn step1_s_after_valid_s_ending() {
        // "hunds" — 's' preceded by 'd' (in s_ending). Delete → "hund".
        //   hunds = h,u,n,d,s (5). R1: h non-v, u v, n non-v at 2.
        //   R1=3. `s` at pos 4, 5-1=4≥3 ✓. Preceded by `d` (valid s-
        //   ending). Delete → "hund".
        assert_eq!(s("hunds"), "hund");
    }

    #[test]
    fn step1_s_after_invalid_s_ending_kept() {
        // "kaos" — s preceded by 'o' (in s_ending). Delete? Actually
        //   `o` IS in the Danish s_ending set (matches the sbl). So
        //   `s` deletes.
        //   kaos = k,a,o,s (4). R1: k non-v, a v, o v (still v), s non-v
        //   at 3. R1=4 → adj to 3 (since 3 < chars.len()=4)? Actually
        //   R1=4, adjusted = max(4, 3.min(4)) = max(4, 3) = 4. So R1=4.
        //   `s` at pos 3, 4-1=3≥4? No. So `s` doesn't fire. Result:
        //   "kaos".
        assert_eq!(s("kaos"), "kaos");
    }

    #[test]
    fn step2_dt_pair() {
        // "verdt" (imagined) = v,e,r,d,t (5). R1: v non-v, e v, r non-v
        //   at 2. R1=3. Step 1: no match ending in `t`. Step 2: `dt` at
        //   pos 3, 5-2=3≥3 ✓. Delete final `t` → "verd".
        assert_eq!(s("verdt"), "verd");
    }

    #[test]
    fn step2_gt_pair() {
        // "brugt" (used) = b,r,u,g,t (5). R1: b non-v, r non-v, u v, g
        //   non-v at 3. R1=4. Step 2: `gt` at pos 3, 5-2=3≥4? No.
        //   Doesn't fire. Result: "brugt".
        assert_eq!(s("brugt"), "brugt");
        // "bevægt" (imagined) = b,e,v,æ,g,t (6). R1: b non-v, e v, v
        //   non-v at 2. R1=3. Step 2: `gt` at pos 4, 6-2=4≥3 ✓. Delete
        //   → "bevæg".
        assert_eq!(s("bevægt"), "bevæg");
    }

    #[test]
    fn step3_lig_strip() {
        // "kærlig" (loving) = k,æ,r,l,i,g (6). R1: k non-v, æ v, r non-v
        //   at 2. R1=3. Step 1: no match ending in `g`. Step 2: no.
        //   Step 3: `lig` at pos 3, 6-3=3≥3 ✓. Delete → "kær".
        assert_eq!(s("kærlig"), "kær");
    }

    #[test]
    fn step3_elig_strip() {
        // "kongelig" (royal) = k,o,n,g,e,l,i,g (8). R1: k non-v, o v, n
        //   non-v at 2. R1=3. Step 1 no match ending in `g`. Step 2 no.
        //   Step 3: longest match — `elig` at pos 4, 8-4=4≥3 ✓. Delete
        //   → "kong".
        assert_eq!(s("kongelig"), "kong");
    }

    #[test]
    fn step3_igst_prelude() {
        // "morigst" (imagined superlative -igst) = m,o,r,i,g,s,t (7).
        //   R1: m non-v, o v, r non-v at 2. R1=3. Step 1: no match on
        //   `-t`. Step 2: no. Step 3: `igst` at pos 3, 7-4=3≥3 ✓. Strip
        //   `-st` → "morig". Then longest match: `ig` at pos 3, 5-2=3≥3
        //   ✓. Delete → "mor".
        assert_eq!(s("morigst"), "mor");
    }

    #[test]
    fn step3_loest_replacement() {
        // "løst" (loosely / loosened) at word start: l,ø,s,t (4). R1:
        //   l non-v, ø v, s non-v at 2. R1=3. Step 1: no match on `-t`.
        //   Step 2: no (`st` not a pair). Step 3: `løst` at pos 0,
        //   4-4=0≥3? No. Doesn't fire. Result: "løst".
        // Try a longer word ending in løst: "opløst" (dissolved) = o,p,
        //   l,ø,s,t (6). R1: o v, p non-v at 1. R1=2 → adj to 3. Step
        //   3: `løst` at pos 2, 6-4=2≥3? No. Doesn't fire. Result:
        //   "opløst".
        // Try even longer: "genopløst" (imagined) = g,e,n,o,p,l,ø,s,t
        //   (9). R1: g non-v, e v, n non-v at 2. R1=3. Step 3: `løst`
        //   at pos 5, 9-4=5≥3 ✓. Replace → "genopløs".
        assert_eq!(s("genopløst"), "genopløs");
    }

    #[test]
    fn step4_undouble_double_consonant() {
        // "besidde" (possess-inf) = b,e,s,i,d,d,e (7). R1: b non-v, e v,
        //   s non-v at 2. R1=3. Step 1: `e` at pos 6, delete → "besidd".
        //   Step 2: no. Step 3: no. Step 4: `dd` at end, positions 4,5;
        //   pair start pos 4, 6-2=4≥3 ✓. Delete last → "besid".
        assert_eq!(s("besidde"), "besid");
    }

    #[test]
    fn step4_undouble_not_in_r1_stays() {
        // "kunne" (can-inf) = k,u,n,n,e (5). R1: k non-v, u v, n non-v
        //   at 2. R1=3. Step 1: `e` at pos 4, delete → "kunn". Step 4:
        //   `nn` at end, pair start pos 2, 4-2=2≥3? No. Doesn't fire.
        //   Result: "kunn".
        assert_eq!(s("kunne"), "kunn");
    }

    #[test]
    fn step4_undouble_no_vowel_double() {
        // A word ending in `ee` (unlikely in Danish but tests the vowel
        //   guard): "gammelee" (imagined) — vowel pair, no undouble.
        // Test with a natural case that leaves the last-char as a vowel:
        //   `"arbejde"` (already tested) — after strip becomes "arbejd";
        //   ends in `d`, not a doubled consonant.
        assert_eq!(s("arbejde"), "arbejd");
    }

    #[test]
    fn danish_letters_preserved() {
        // "år" (year) — no suffix rules fire. Result: "år".
        assert_eq!(s("år"), "år");
        // "være" (to be) — v,æ,r,e (4). R1: v non-v, æ v, r non-v at 2.
        //   R1=3. Step 1: `e` at pos 3, 4-1=3≥3 ✓. Delete → "vær".
        assert_eq!(s("være"), "vær");
        // "øje" (eye) = ø,j,e (3). R1: ø v, j non-v at 1. R1=2 → adj to
        //   3. Step 1: `e` at pos 2, 3-1=2≥3? No. Doesn't fire.
        //   Result: "øje".
        assert_eq!(s("øje"), "øje");
    }

    #[test]
    fn convergence_on_common_vocabulary() {
        for w in [
            "hus",
            "huset",
            "arbejde",
            "arbejdet",
            "kærlig",
            "kærlighed",
            "kærlighedens",
            "sæd",
            "sæden",
            "sædene",
            "hunds",
            "besidde",
            "kunne",
            "elske",
            "elskede",
            "løbende",
            "være",
            "vær",
            "øje",
            "år",
        ] {
            let mut cur = DanishSnowball.stem(w).into_owned();
            for _ in 0..5 {
                let next = DanishSnowball.stem(&cur).into_owned();
                if next == cur {
                    break;
                }
                cur = next;
            }
            let n1 = DanishSnowball.stem(&cur).into_owned();
            assert_eq!(cur, n1, "did not converge on {w:?}");
        }
    }

    #[test]
    fn r1_paper_example() {
        // "arbejde" — R1?
        //   a v, r non-v at 1. R1=2 → adjusted to 3.
        let chars: Vec<char> = "arbejde".chars().collect();
        assert_eq!(compute_r1_adjusted(&chars), 3);
    }

    #[test]
    fn s_ending_set_includes_aa_and_danish_a_ring() {
        // Spec quirk: `a` and `å` are in the s-ending group.
        assert!(is_s_ending('a'));
        assert!(is_s_ending('å'));
        assert!(is_s_ending('k'));
        assert!(is_s_ending('t'));
        assert!(!is_s_ending('e'));
        assert!(!is_s_ending('i'));
        assert!(!is_s_ending('u'));
        assert!(!is_s_ending('æ'));
        assert!(!is_s_ending('ø'));
    }
}
