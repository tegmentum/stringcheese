//! The Porter2 (Snowball English) stemmer.
//!
//! # Origin
//!
//! Martin Porter's revised English stemmer, first published in 2001 as
//! part of the Snowball stemmer generator project (see
//! <https://snowballstem.org/algorithms/english/stemmer.html>). Porter2
//! is the reference "English 2" algorithm every subsequent Snowball
//! port and every practical IR system either uses directly or documents
//! its variation against.
//!
//! # What Porter2 changes from Porter (1980)
//!
//! Porter2 keeps the five-step suffix-stripping shape of the original
//! algorithm but corrects several defects and adds handling for edge
//! cases the 1980 paper leaves implementation-dependent:
//!
//! * **Explicit exception table.** Words like `sky`, `dying`, `lying`,
//!   `tying`, `skis`, `skies`, `idly`, `ugly`, `early`, `only`,
//!   `singly`, `gently`, and the invariants `news`, `howe`, `atlas`,
//!   `cosmos`, `bias`, `andes` are handled up front.
//! * **R1 and R2 regions.** Porter2 replaces Porter's measure `m` with
//!   two region markers — R1 is the region after the first non-vowel
//!   following a vowel, and R2 is the region after the first non-vowel
//!   following a vowel in R1. All rule conditions are expressed as
//!   "in R1" or "in R2".
//! * **Special-prefix R1 rule.** Words beginning with `gener`,
//!   `commun`, `arsen`, `past`, `univers`, `later`, `emerg`, `organ`,
//!   or `inter` have R1 set to the remainder of the word. This
//!   prevents Porter2 from over-stemming `generate`, `communism`,
//!   `arsenal`, `university`, etc.
//! * **Y-vowel handling.** Initial `y` (or `y` after a vowel) is
//!   internally uppercased to `Y` and treated as a consonant for the
//!   duration of the algorithm; the postlude lowercases back.
//! * **New Step 1a s-rule.** `s` is deleted only if the preceding
//!   word part contains a vowel not immediately before the `s` (so
//!   `gas` and `this` retain the `s`; `gaps` and `kiwis` lose it).
//! * **Special Step 1b `ing`→`ie` case.** Words like `dying`, `lying`,
//!   `tying`, `vying` produce `die`, `lie`, `tie`, `vie` rather than
//!   Porter's `dy`, `ly`, `ty`, `vy`.
//! * **Post-Step-1a invariants.** `inn`, `out`, `cann`, `herr`, `earr`,
//!   `even` at the start of a word (as stems of `inning`, `outing`,
//!   `canning`, `herring`, `earring`, `evening`) skip Step 1b's
//!   `ing` deletion.
//! * **`proceed`, `exceed`, `succeed`** are recognized in Step 1b's
//!   `eed` rule so that `proceeding` doesn't erroneously stem the
//!   final `eed`.
//! * **Additional Step 2/3 suffixes.** `ogist`, `fulli`, `lessli`,
//!   `li` (with a valid-`li` predecessor), and several others.
//!
//! # When to pick Porter over Porter2 (or vice versa)
//!
//! * **Porter (1980)** — you're reproducing a reference implementation
//!   from an older paper or corpus, need bit-for-bit compatibility
//!   with the classic algorithm, or your consumers already index
//!   against Porter stems.
//! * **Porter2 (this module)** — you're building a fresh IR pipeline,
//!   want the corrected `-y` / small-word / `proceed` / `dying`
//!   behaviour, and have no legacy compatibility constraint.
//!
//! # Non-goals
//!
//! * **Case preservation.** Porter2 operates on lowercase ASCII. Input
//!   is lowercased at the entry point; output is lowercase.
//! * **Non-ASCII input.** Non-ASCII characters are passed through
//!   unchanged and treated as consonants (not vowels); the stem this
//!   produces is not meaningful for non-English input.

use alloc::borrow::Cow;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use stringcheese_lang::Stemmer;

/// The Porter2 (Snowball English) stemmer.
///
/// A zero-sized unit value; construct as `Porter2` and reuse the value
/// freely across threads and calls, or grab the crate-level
/// [`PORTER2_STEMMER`](crate::PORTER2_STEMMER) constant.
///
/// See the [module-level docs](self) for the algorithm and the
/// differences from Porter (1980).
///
/// # Example
///
/// ```
/// use stringcheese_en::Porter2;
/// use stringcheese_lang::Stemmer;
///
/// assert_eq!(Porter2.stem("caresses"), "caress");
/// assert_eq!(Porter2.stem("ponies"), "poni");
/// assert_eq!(Porter2.stem("running"), "run");
/// // Porter2's corrected `-ying` handling:
/// assert_eq!(Porter2.stem("dying"), "die");
/// assert_eq!(Porter2.stem("lying"), "lie");
/// // Porter2 preserves invariants where Porter would over-stem:
/// assert_eq!(Porter2.stem("sky"), "sky");
/// assert_eq!(Porter2.stem("news"), "news");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Porter2;

impl Porter2 {
    /// Stems `word` per the Porter2 (Snowball) English algorithm.
    ///
    /// Returns a lowercase ASCII stem. If `word` is already lowercase
    /// ASCII and the algorithm leaves it unchanged, the returned
    /// [`Cow`] borrows the input.
    ///
    /// # Panics
    ///
    /// Never panics on any `&str` input — the internal working buffer
    /// only ever contains ASCII bytes, so the final `String::from_utf8`
    /// is infallible. The `expect` inside is a compile-time assertion,
    /// not a runtime one.
    #[must_use]
    pub fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        // Fast path: an empty input is its own stem.
        if word.is_empty() {
            return Cow::Borrowed(word);
        }

        // Normalize to lowercase ASCII. If the input is already
        // lowercase ASCII and the algorithm leaves it unchanged, we
        // can return the borrow.
        let (buf, borrowed) = normalize(word);

        // Exception table (checked before length gate — some
        // exceptions are 3 letters long).
        if let Some(rep) = exception1(&buf) {
            return Cow::Owned(rep.to_string());
        }

        // Words of length <= 2 are never modified.
        if buf.len() <= 2 {
            return borrowed.unwrap_or(Cow::Owned(buf));
        }

        // Snowball: `not hop 3` — words with fewer than 3 chars are
        // returned unchanged. (We've already returned above for
        // len <= 2; this is the len == 3+ path.)
        let mut bytes: Vec<u8> = buf.into_bytes();

        prelude(&mut bytes);
        let (p1, p2) = mark_regions(&bytes);
        step_1a(&mut bytes);
        step_1b(&mut bytes, p1);
        step_1c(&mut bytes);
        step_2(&mut bytes, p1);
        step_3(&mut bytes, p1, p2);
        step_4(&mut bytes, p2);
        step_5(&mut bytes, p1, p2);
        postlude(&mut bytes);

        // SAFETY: prelude/steps only manipulate ASCII bytes, so the
        // buffer is always valid UTF-8.
        let out = String::from_utf8(bytes).expect("Porter2 keeps ASCII");
        match borrowed {
            Some(orig) if orig == out => orig,
            _ => Cow::Owned(out),
        }
    }
}

impl Stemmer for Porter2 {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        Porter2::stem(self, word)
    }
}

/// Normalizes the input to lowercase ASCII in an owned `String`.
///
/// Non-ASCII bytes are passed through unchanged. The second return
/// value is `Some(Cow::Borrowed(word))` when the input was already
/// lowercase ASCII — this lets `Porter2::stem` preserve the borrow
/// when the algorithm makes no further changes.
fn normalize(word: &str) -> (String, Option<Cow<'_, str>>) {
    if word.bytes().all(|b| b.is_ascii_lowercase()) {
        (word.to_string(), Some(Cow::Borrowed(word)))
    } else {
        (word.to_ascii_lowercase(), None)
    }
}

// ---------------------------------------------------------------------------
// Vowel / consonant predicates.
//
// Porter2's vowel set is `a, e, i, o, u, y`. An uppercase `Y` inserted
// by [`prelude`] is *not* a vowel — that's the whole point of the
// substitution.
// ---------------------------------------------------------------------------

#[inline]
fn is_vowel(b: u8) -> bool {
    matches!(b, b'a' | b'e' | b'i' | b'o' | b'u' | b'y')
}

/// The Snowball `v_WXY` grouping: vowel or `w`, `x`, `Y`.
#[inline]
fn is_v_or_wxy(b: u8) -> bool {
    is_vowel(b) || matches!(b, b'w' | b'x' | b'Y')
}

/// The Snowball `valid_LI` grouping: `c, d, e, g, h, k, m, n, r, t`.
#[inline]
fn is_valid_li(b: u8) -> bool {
    matches!(
        b,
        b'c' | b'd' | b'e' | b'g' | b'h' | b'k' | b'm' | b'n' | b'r' | b't'
    )
}

/// The Snowball doubles: `bb, dd, ff, gg, mm, nn, pp, rr, tt`.
#[inline]
fn is_double_byte(b: u8) -> bool {
    matches!(
        b,
        b'b' | b'd' | b'f' | b'g' | b'm' | b'n' | b'p' | b'r' | b't'
    )
}

// ---------------------------------------------------------------------------
// Prelude:
//   * Strip a leading apostrophe.
//   * Uppercase leading `y` to `Y`.
//   * Uppercase `y` immediately following a vowel to `Y`.
// ---------------------------------------------------------------------------

pub(crate) fn prelude(w: &mut Vec<u8>) {
    // Strip leading apostrophe if present.
    if w.first() == Some(&b'\'') {
        w.remove(0);
    }
    if w.is_empty() {
        return;
    }
    // Uppercase leading y.
    if w[0] == b'y' {
        w[0] = b'Y';
    }
    // Uppercase every `y` preceded by a lowercase vowel. Note that a
    // preceding `Y` (already uppercased) does *not* count as a vowel
    // per the Snowball spec — the `v` grouping is `aeiouy`, and the
    // capitalised `Y` is deliberately outside it.
    for i in 1..w.len() {
        if w[i] == b'y' && is_vowel(w[i - 1]) {
            w[i] = b'Y';
        }
    }
}

fn postlude(w: &mut [u8]) {
    for b in w.iter_mut() {
        if *b == b'Y' {
            *b = b'y';
        }
    }
}

// ---------------------------------------------------------------------------
// R1 / R2 region markers.
//
// R1 is the region after the first non-vowel following a vowel, or is
// the null region at the end of the word if there is no such non-vowel.
// R2 is the region after the first non-vowel following a vowel in R1,
// or the null region at the end of the word.
//
// Special: if the word begins with one of the listed prefixes, R1 is
// set to the remainder of the word (skipping the usual computation for
// R1 only — R2 is still computed the usual way starting from R1).
// ---------------------------------------------------------------------------

const SPECIAL_PREFIXES: &[&[u8]] = &[
    b"gener", b"commun", b"arsen", b"past", b"univers", b"later", b"emerg", b"organ", b"inter",
];

pub(crate) fn mark_regions(w: &[u8]) -> (usize, usize) {
    let p1 = SPECIAL_PREFIXES
        .iter()
        .find(|p| w.starts_with(p))
        .map_or_else(|| region_after(w, 0), |p| p.len());
    let p2 = region_after(w, p1);
    (p1, p2)
}

/// Returns the position after the first non-vowel that follows a vowel,
/// scanning from `start`. If no such non-vowel exists, returns `w.len()`.
fn region_after(w: &[u8], start: usize) -> usize {
    let n = w.len();
    let mut i = start;
    // Advance until we find a vowel.
    while i < n && !is_vowel(w[i]) {
        i += 1;
    }
    // Advance until we find a non-vowel.
    while i < n && is_vowel(w[i]) {
        i += 1;
    }
    // If we're at end, no non-vowel followed the vowel-run: null region.
    if i >= n {
        return n;
    }
    // We're at the first non-vowel after a vowel; the region starts one
    // past it.
    i + 1
}

// ---------------------------------------------------------------------------
// Exception table (Snowball `exception1`).
//
// Words that either map to a specific stem or are invariant. Checked
// once, at the very start of processing, before any regions are
// computed. If a word hits the exception table, we return the mapped
// value immediately without running the rest of the algorithm.
// ---------------------------------------------------------------------------

fn exception1(w: &str) -> Option<&'static str> {
    match w {
        // Special mappings.
        "skis" => Some("ski"),
        "idly" => Some("idl"),
        "gently" => Some("gentl"),
        "ugly" => Some("ugli"),
        "early" => Some("earli"),
        "only" => Some("onli"),
        "singly" => Some("singl"),
        // Invariants (mapped to themselves). `skies` -> `sky` also
        // lands here because Snowball explicitly rewrites it to `sky`.
        "skies" | "sky" => Some("sky"),
        "news" => Some("news"),
        "howe" => Some("howe"),
        "atlas" => Some("atlas"),
        "cosmos" => Some("cosmos"),
        "bias" => Some("bias"),
        "andes" => Some("andes"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Step 0 / Step 1a.
//
// Snowball folds a "Step 0" apostrophe strip into Step 1a: the leading
// `try (['] or ['s] or ['s'] delete)` runs first, then the main Step 1a
// suffix rules.
// ---------------------------------------------------------------------------

fn step_1a(w: &mut Vec<u8>) {
    // Step 0: strip trailing 's, 's', or ' (longest match).
    if w.ends_with(b"'s'") {
        w.truncate(w.len() - 3);
    } else if w.ends_with(b"'s") {
        w.truncate(w.len() - 2);
    } else if w.ends_with(b"'") {
        w.truncate(w.len() - 1);
    }

    if w.ends_with(b"sses") {
        // sses -> ss
        w.truncate(w.len() - 2);
    } else if w.ends_with(b"ied") || w.ends_with(b"ies") {
        // Replace by `i` if preceded by more than one letter, else `ie`.
        let stem_len = w.len() - 3;
        if stem_len > 1 {
            w.truncate(stem_len);
            w.push(b'i');
        } else {
            w.truncate(stem_len);
            w.extend_from_slice(b"ie");
        }
    } else if w.ends_with(b"us") || w.ends_with(b"ss") {
        // do nothing
    } else if w.ends_with(b"s") {
        // Delete `s` if the preceding word part contains a vowel not
        // immediately before the `s`. In byte terms: some byte at
        // position 0..=n-3 is a vowel.
        let n = w.len();
        if n >= 3 && w[..n - 2].iter().any(|&b| is_vowel(b)) {
            w.pop();
        }
    }
}

// ---------------------------------------------------------------------------
// Step 1b.
//
// Handles the past-tense / progressive suffixes -eed(ly), -ed(ly),
// -ing(ly), plus the special `-ying` -> `-ie` mapping and the
// invariant list for -ing endings (inn, out, cann, herr, earr, even).
// ---------------------------------------------------------------------------

fn step_1b(w: &mut Vec<u8>, p1: usize) {
    // -eed / -eedly: replace by ee if in R1, except for the proc/exc/
    // succ family (proceed/exceed/succeed and their -ly forms).
    if let Some(suf_len) = ends_with_any(w, &[b"eedly", b"eed"]) {
        let stem_len = w.len() - suf_len;
        // In R1?
        if stem_len < p1 {
            return;
        }
        // Exception: stem ends in proc/exc/succ AND that stem is the
        // whole word (i.e., the word is proceed/proceedly/exceed/
        // exceedly/succeed/succeedly).
        let stem = &w[..stem_len];
        for exc in [b"proc" as &[u8], b"exc", b"succ"] {
            if stem == exc {
                return;
            }
        }
        w.truncate(stem_len);
        w.extend_from_slice(b"ee");
        return;
    }

    // Match -ingly/-edly/-ing/-ed for the general handling.
    let Some(suf_len) = ends_with_any(w, &[b"ingly", b"edly", b"ing", b"ed"]) else {
        return;
    };
    let stem_len = w.len() - suf_len;
    let stem = &w[..stem_len];

    // -ing / -ingly special cases (only when the matched suffix is
    // -ing or -ingly, i.e., ends in 'g').
    if w[w.len() - 1] == b'g' {
        // dying/lying/tying/vying: stem is a single non-vowel followed
        // by `y` (stem is exactly 2 chars: non-v then y).
        if stem_len == 2 && !is_vowel(stem[0]) && stem[1] == b'y' {
            // Replace `ying` (or `yingly`) with `ie`.
            w.truncate(stem_len - 1);
            w.extend_from_slice(b"ie");
            return;
        }
        // Invariant stems for -ing: inn, out, cann, herr, earr, even.
        // The stem must be exactly one of these and must span the
        // whole word (i.e., "at limit").
        for inv in [b"inn" as &[u8], b"out", b"cann", b"herr", b"earr", b"even"] {
            if stem == inv {
                return;
            }
        }
    }

    // General handling: the stem must contain a vowel.
    if !stem.iter().any(|&b| is_vowel(b)) {
        return;
    }
    // Delete the suffix.
    w.truncate(stem_len);
    // Post-deletion cleanup:
    //   ends -at/-bl/-iz  -> append `e`
    //   ends in double    -> remove last letter (unless preceded by
    //                        a/e/o at start of word — that's the
    //                        aeo-atlimit exception)
    //   short word        -> append `e`
    if w.ends_with(b"at") || w.ends_with(b"bl") || w.ends_with(b"iz") {
        w.push(b'e');
        return;
    }
    if ends_double(w) {
        // Don't remove if word is exactly 3 chars starting with a/e/o
        // (this preserves e.g. `add`, `egg`, `odd`, `off`).
        let n = w.len();
        if !(n == 3 && matches!(w[0], b'a' | b'e' | b'o')) {
            w.pop();
        }
        return;
    }
    if is_short_word(w, p1) {
        w.push(b'e');
    }
}

/// True if `w` ends in a Porter2 "double" — `bb`, `dd`, `ff`, `gg`,
/// `mm`, `nn`, `pp`, `rr`, `tt`.
fn ends_double(w: &[u8]) -> bool {
    let n = w.len();
    n >= 2 && w[n - 1] == w[n - 2] && is_double_byte(w[n - 1])
}

/// True if `w` is a Porter2 "short word": it ends in a short syllable
/// AND R1 is null (`p1 >= w.len()`).
fn is_short_word(w: &[u8], p1: usize) -> bool {
    p1 >= w.len() && ends_short_syllable(w)
}

/// Snowball's `shortv` predicate at end of word.
///
/// A word ends in a short syllable if:
/// (a) its last three bytes are (non-v)(v)(non-v-not-in-{w,x,Y}); OR
/// (b) its length is exactly 2 and it is (v)(non-v); OR
/// (c) the word is `past` (Snowball extension for -past- prefix).
fn ends_short_syllable(w: &[u8]) -> bool {
    let n = w.len();
    if n >= 3 {
        let (first, mid, last) = (w[n - 3], w[n - 2], w[n - 1]);
        if !is_vowel(first) && is_vowel(mid) && !is_v_or_wxy(last) {
            return true;
        }
    }
    if n == 2 && is_vowel(w[0]) && !is_vowel(w[1]) {
        return true;
    }
    w == b"past"
}

// ---------------------------------------------------------------------------
// Step 1c: replace terminal y or Y by i if preceded by a non-vowel that
// is not the first letter of the word.
// ---------------------------------------------------------------------------

fn step_1c(w: &mut [u8]) {
    let n = w.len();
    if n < 3 {
        return;
    }
    let last = w[n - 1];
    if (last == b'y' || last == b'Y') && !is_vowel(w[n - 2]) {
        // The non-vowel at n-2 must not be at position 0.
        // Equivalently: n - 2 > 0, i.e., n >= 3 (already ensured).
        w[n - 1] = b'i';
    }
}

// ---------------------------------------------------------------------------
// Step 2 (in R1): long-suffix mappings.
// ---------------------------------------------------------------------------

/// Step 2 rules: `(suffix, replacement)` sorted longest-first.
const STEP_2: &[(&[u8], &[u8])] = &[
    (b"ization", b"ize"),
    (b"ational", b"ate"),
    (b"fulness", b"ful"),
    (b"ousness", b"ous"),
    (b"iveness", b"ive"),
    (b"tional", b"tion"),
    (b"biliti", b"ble"),
    (b"lessli", b"less"),
    (b"entli", b"ent"),
    (b"ation", b"ate"),
    (b"alism", b"al"),
    (b"aliti", b"al"),
    (b"ousli", b"ous"),
    (b"iviti", b"ive"),
    (b"fulli", b"ful"),
    (b"enci", b"ence"),
    (b"anci", b"ance"),
    (b"abli", b"able"),
    (b"izer", b"ize"),
    (b"ator", b"ate"),
    (b"alli", b"al"),
    (b"bli", b"ble"),
    (b"ogi", b"og"), // conditional: preceded by `l`
    (b"li", b""),    // conditional: preceded by a valid-li letter (delete)
    (b"ogist", b"og"),
];

fn step_2(w: &mut Vec<u8>, p1: usize) {
    let Some((suf, rep, suf_len)) = longest_match(w, STEP_2) else {
        return;
    };
    let stem_len = w.len() - suf_len;
    // Must be in R1.
    if stem_len < p1 {
        return;
    }
    // Conditional suffixes: `ogi` fires only when preceded by `l`,
    // and `li` fires only when preceded by a valid-li character.
    if suf == b"ogi" && !(stem_len > 0 && w[stem_len - 1] == b'l') {
        return;
    }
    if suf == b"li" && !(stem_len > 0 && is_valid_li(w[stem_len - 1])) {
        return;
    }
    w.truncate(stem_len);
    w.extend_from_slice(rep);
}

// ---------------------------------------------------------------------------
// Step 3 (in R1): a few more long suffix mappings, plus one R2 rule
// (`ative` deleted if in R2).
// ---------------------------------------------------------------------------

/// Step 3 rules: `(suffix, replacement)` sorted longest-first.
///
/// Note that `ative` is present here but is deleted only if in R2 —
/// handled by an explicit branch below.
const STEP_3: &[(&[u8], &[u8])] = &[
    (b"ational", b"ate"),
    (b"tional", b"tion"),
    (b"alize", b"al"),
    (b"icate", b"ic"),
    (b"iciti", b"ic"),
    (b"ative", b""), // in R2 only
    (b"ical", b"ic"),
    (b"ness", b""),
    (b"ful", b""),
];

fn step_3(w: &mut Vec<u8>, p1: usize, p2: usize) {
    let Some((suf, rep, suf_len)) = longest_match(w, STEP_3) else {
        return;
    };
    let stem_len = w.len() - suf_len;
    if stem_len < p1 {
        return;
    }
    if suf == b"ative" {
        // Additional gate: must be in R2.
        if stem_len < p2 {
            return;
        }
    }
    w.truncate(stem_len);
    w.extend_from_slice(rep);
}

// ---------------------------------------------------------------------------
// Step 4 (in R2): residual suffix stripping (plus the `ion` rule that
// also requires the preceding character to be `s` or `t`).
// ---------------------------------------------------------------------------

/// Step 4 rules: all delete-only, sorted longest-first. `ion` is
/// handled separately because it needs a follow-on predicate on the
/// preceding character.
const STEP_4: &[&[u8]] = &[
    b"ement", b"ance", b"ence", b"able", b"ible", b"ment", b"ant", b"ent", b"ism", b"ate", b"iti",
    b"ous", b"ive", b"ize", b"al", b"er", b"ic",
];

fn step_4(w: &mut Vec<u8>, p2: usize) {
    // ion is special: must be in R2 AND preceded by s or t.
    if let Some(new_len) = try_step_4_ion(w, p2) {
        w.truncate(new_len);
        return;
    }
    for &suf in STEP_4 {
        if w.ends_with(suf) {
            let stem_len = w.len() - suf.len();
            if stem_len >= p2 {
                w.truncate(stem_len);
            }
            return;
        }
    }
}

/// Handles Step 4's `ion` rule: delete `ion` if in R2 and preceded by
/// `s` or `t`. Returns the new word length on success, `None`
/// otherwise.
fn try_step_4_ion(w: &[u8], p2: usize) -> Option<usize> {
    if !w.ends_with(b"ion") {
        return None;
    }
    let stem_len = w.len() - 3;
    if stem_len < p2 {
        return None;
    }
    if stem_len == 0 {
        return None;
    }
    if !matches!(w[stem_len - 1], b's' | b't') {
        return None;
    }
    Some(stem_len)
}

// ---------------------------------------------------------------------------
// Step 5: terminal `e` and terminal `l`.
//
//   `e`  delete if in R2, OR in R1 and not preceded by a short syllable
//   `l`  delete if in R2 and preceded by `l`
// ---------------------------------------------------------------------------

fn step_5(w: &mut Vec<u8>, p1: usize, p2: usize) {
    let n = w.len();
    if n == 0 {
        return;
    }
    match w[n - 1] {
        b'e' => {
            let stem_len = n - 1;
            let in_r2 = stem_len >= p2;
            let in_r1_not_shortv = stem_len >= p1 && !ends_short_syllable(&w[..stem_len]);
            if in_r2 || in_r1_not_shortv {
                w.pop();
            }
        }
        b'l' if n >= 2 && w[n - 2] == b'l' && n > p2 => {
            w.pop();
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Helpers.
// ---------------------------------------------------------------------------

/// If `w` ends with one of `suffixes` (longest wins), returns the
/// matched suffix's length.
fn ends_with_any(w: &[u8], suffixes: &[&[u8]]) -> Option<usize> {
    let mut best: Option<usize> = None;
    for &s in suffixes {
        if w.ends_with(s) && best.is_none_or(|b| s.len() > b) {
            best = Some(s.len());
        }
    }
    best
}

/// Longest-match lookup across a `(suffix, replacement)` table.
///
/// Returns `(matched_suffix, replacement, matched_suffix_len)` for the
/// longest suffix in `table` that ends `w`, or `None`.
fn longest_match<'a>(
    w: &[u8],
    table: &'a [(&'a [u8], &'a [u8])],
) -> Option<(&'a [u8], &'a [u8], usize)> {
    let mut best: Option<(&[u8], &[u8], usize)> = None;
    for &(suf, rep) in table {
        if w.ends_with(suf) && best.is_none_or(|(_, _, len)| suf.len() > len) {
            best = Some((suf, rep, suf.len()));
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stem(w: &str) -> String {
        Porter2.stem(w).into_owned()
    }

    #[test]
    fn empty_and_short_inputs_pass_through() {
        assert_eq!(stem(""), "");
        assert_eq!(stem("a"), "a");
        assert_eq!(stem("by"), "by");
        assert_eq!(stem("of"), "of");
    }

    #[test]
    fn exception_table_hits() {
        // Special mappings.
        assert_eq!(stem("skis"), "ski");
        assert_eq!(stem("skies"), "sky");
        assert_eq!(stem("idly"), "idl");
        assert_eq!(stem("gently"), "gentl");
        assert_eq!(stem("ugly"), "ugli");
        assert_eq!(stem("early"), "earli");
        assert_eq!(stem("only"), "onli");
        assert_eq!(stem("singly"), "singl");
        // Invariants.
        assert_eq!(stem("sky"), "sky");
        assert_eq!(stem("news"), "news");
        assert_eq!(stem("howe"), "howe");
        assert_eq!(stem("atlas"), "atlas");
        assert_eq!(stem("cosmos"), "cosmos");
        assert_eq!(stem("bias"), "bias");
        assert_eq!(stem("andes"), "andes");
    }

    #[test]
    fn region_markers_match_snowball_examples() {
        // Snowball spec examples for R1 / R2.
        //   beautiful      R1 = iful, R2 = ul
        //   beauty         R1 = y,    R2 = empty
        //   beau           R1 = empty,R2 = empty
        //   animadversion  R1 = imadversion, R2 = adversion
        //   sprinkled      R1 = kled, R2 = empty
        //   eucharist      R1 = harist, R2 = ist
        let cases = [
            ("beautiful", 5, 7),
            ("beauty", 5, 6),
            ("beau", 4, 4),
            ("animadversion", 2, 4),
            ("sprinkled", 5, 9),
            ("eucharist", 3, 6),
        ];
        for (w, p1, p2) in cases {
            let mut bytes = w.as_bytes().to_vec();
            prelude(&mut bytes);
            let (got1, got2) = mark_regions(&bytes);
            assert_eq!(got1, p1, "p1 wrong for {w}");
            assert_eq!(got2, p2, "p2 wrong for {w}");
        }
    }

    #[test]
    fn special_prefixes_set_r1() {
        // gener -> R1 starts at 5.
        for (w, p1) in [
            ("generate", 5),
            ("generous", 5),
            ("communism", 6),
            ("arsenal", 5),
            ("university", 7),
            ("emerging", 5),
            ("organism", 5),
            ("internal", 5),
            ("laterally", 5),
        ] {
            let mut bytes = w.as_bytes().to_vec();
            prelude(&mut bytes);
            let (got1, _) = mark_regions(&bytes);
            assert_eq!(got1, p1, "special-prefix p1 wrong for {w}");
        }
    }

    #[test]
    fn prelude_y_treatment_examples() {
        // Initial y -> Y.
        let mut w = b"yes".to_vec();
        prelude(&mut w);
        assert_eq!(&w, b"Yes");
        // y after vowel -> Y.
        let mut w = b"day".to_vec();
        prelude(&mut w);
        assert_eq!(&w, b"daY");
        // y after non-vowel -> y (unchanged).
        let mut w = b"sky".to_vec();
        prelude(&mut w);
        assert_eq!(&w, b"sky");
        // Leading apostrophe stripped.
        let mut w = b"'twas".to_vec();
        prelude(&mut w);
        assert_eq!(&w, b"twas");
        // Multiple y-in-vowel positions.
        let mut w = b"yay".to_vec();
        prelude(&mut w);
        assert_eq!(&w, b"YaY");
    }

    #[test]
    fn step_1a_paper_examples() {
        assert_eq!(stem("caresses"), "caress");
        assert_eq!(stem("ponies"), "poni");
        assert_eq!(stem("ties"), "tie"); // Porter2's difference from Porter (1980)
        assert_eq!(stem("caress"), "caress");
        assert_eq!(stem("cats"), "cat");
        // 's' rule: don't delete when preceding word part lacks a vowel
        // before the immediate pre-s slot.
        assert_eq!(stem("gas"), "gas");
        assert_eq!(stem("this"), "this");
        assert_eq!(stem("gaps"), "gap");
        assert_eq!(stem("kiwis"), "kiwi");
    }

    #[test]
    fn step_1b_paper_examples() {
        // eed/eedly rules.
        assert_eq!(stem("feed"), "feed"); // stem before eed is "f" (not in R1).
        assert_eq!(stem("agreed"), "agre"); // eed->ee; step 5 strips trailing e.
        assert_eq!(stem("proceed"), "proceed"); // proc/exc/succ exception.
        assert_eq!(stem("exceed"), "exceed");
        assert_eq!(stem("succeed"), "succeed");
        // Special -ying rule.
        assert_eq!(stem("dying"), "die");
        assert_eq!(stem("lying"), "lie");
        assert_eq!(stem("tying"), "tie");
        assert_eq!(stem("vying"), "vie");
        // -ing invariants.
        assert_eq!(stem("inning"), "inning");
        assert_eq!(stem("outing"), "outing");
        assert_eq!(stem("canning"), "canning");
        assert_eq!(stem("herring"), "herring");
        assert_eq!(stem("earring"), "earring");
        assert_eq!(stem("evening"), "evening");
        // Standard ing / ed handling.
        assert_eq!(stem("motoring"), "motor");
        assert_eq!(stem("plastered"), "plaster");
        assert_eq!(stem("bled"), "bled");
        assert_eq!(stem("sing"), "sing");
        assert_eq!(stem("hopping"), "hop");
        assert_eq!(stem("tanned"), "tan");
        assert_eq!(stem("hoping"), "hope");
        assert_eq!(stem("filing"), "file");
        assert_eq!(stem("adding"), "add"); // aeo-atlimit protects the double.
    }

    #[test]
    fn step_1c_examples() {
        assert_eq!(stem("happy"), "happi");
        assert_eq!(stem("cry"), "cri");
        // 2-char words don't get the y->i transform (stem short-circuit).
        assert_eq!(stem("by"), "by");
    }

    #[test]
    fn step_2_paper_examples() {
        assert_eq!(stem("relational"), "relat");
        assert_eq!(stem("conditional"), "condit");
        assert_eq!(stem("digitizer"), "digit");
        assert_eq!(stem("vietnamization"), "vietnam");
        assert_eq!(stem("predication"), "predic");
        assert_eq!(stem("operator"), "oper");
        assert_eq!(stem("feudalism"), "feudal");
        assert_eq!(stem("callousness"), "callous");
    }

    #[test]
    fn step_3_paper_examples() {
        assert_eq!(stem("triplicate"), "triplic");
        // Porter2 differs from Porter (1980) here: Step 3's `ative`
        // only fires in R2. formative's `ative` sits in R1 but not R2
        // (p2=6, stem_len=4), so Step 3 leaves it alone; Step 4's
        // `ive` rule then deletes.
        assert_eq!(stem("formative"), "format");
        assert_eq!(stem("formalize"), "formal");
        assert_eq!(stem("electriciti"), "electr");
        assert_eq!(stem("electrical"), "electr");
        assert_eq!(stem("hopeful"), "hope");
        assert_eq!(stem("goodness"), "good");
    }

    #[test]
    fn step_4_examples() {
        assert_eq!(stem("revival"), "reviv");
        assert_eq!(stem("allowance"), "allow");
        assert_eq!(stem("inference"), "infer");
        assert_eq!(stem("gyroscopic"), "gyroscop");
        assert_eq!(stem("adjustable"), "adjust");
        assert_eq!(stem("defensible"), "defens");
        assert_eq!(stem("irritant"), "irrit");
        assert_eq!(stem("replacement"), "replac");
        assert_eq!(stem("adjustment"), "adjust");
        assert_eq!(stem("dependent"), "depend");
        assert_eq!(stem("adoption"), "adopt");
        assert_eq!(stem("homologous"), "homolog");
        assert_eq!(stem("effective"), "effect");
        assert_eq!(stem("bowdlerize"), "bowdler");
    }

    #[test]
    fn step_5_examples() {
        assert_eq!(stem("rate"), "rate");
        assert_eq!(stem("file"), "file");
        assert_eq!(stem("cease"), "ceas");
        assert_eq!(stem("controll"), "control");
        assert_eq!(stem("roll"), "roll");
    }

    #[test]
    fn common_english_words() {
        assert_eq!(stem("running"), "run");
        assert_eq!(stem("runs"), "run");
        assert_eq!(stem("runner"), "runner");
        assert_eq!(stem("stemming"), "stem");
        assert_eq!(stem("crying"), "cri");
    }

    #[test]
    fn uppercase_input_is_lowercased() {
        assert_eq!(stem("CARESSES"), "caress");
        assert_eq!(stem("Caresses"), "caress");
        assert_eq!(stem("PONIES"), "poni");
    }

    #[test]
    fn borrows_input_when_unchanged() {
        // Porter2 leaves "caress" alone (Step 1a's ss rule).
        let out = Porter2.stem("caress");
        assert!(matches!(out, Cow::Borrowed(_)));
    }

    #[test]
    fn short_syllable_predicate() {
        assert!(ends_short_syllable(b"rap")); // non-v v non-v
        assert!(ends_short_syllable(b"trap")); // ends non-v v non-v
        assert!(ends_short_syllable(b"entrap")); // ends non-v v non-v
        assert!(ends_short_syllable(b"ow")); // 2 chars: v non-v -> wait, o v then w. Actually short syllable (b): vowel at start + non-vowel.
        assert!(ends_short_syllable(b"on")); // v non-v, length 2.
        assert!(ends_short_syllable(b"at")); // v non-v, length 2.
        assert!(!ends_short_syllable(b"uproot")); // ends oot: non-v v v? o at n-3, o at n-2, t at n-1. o at n-3 is v so fails alt 1.
        assert!(!ends_short_syllable(b"bestow")); // ends tow: t v w. w is in wxY -> fails alt 1.
        assert!(!ends_short_syllable(b"disturb")); // ends urb: u v b. v at n-3, fails alt 1 (u is v).
        // Snowball extension: "past" is a short syllable in itself.
        assert!(ends_short_syllable(b"past"));
    }

    #[test]
    fn double_predicate() {
        assert!(ends_double(b"add"));
        assert!(ends_double(b"hopp"));
        assert!(ends_double(b"tt"));
        assert!(!ends_double(b"cat"));
        assert!(!ends_double(b"ll")); // Not in the double list (only bb/dd/ff/gg/mm/nn/pp/rr/tt).
        assert!(!ends_double(b"cc"));
    }
}
