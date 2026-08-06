//! The Porter (1980) stemmer.
//!
//! # Origin
//!
//! Porter's 1980 paper *An algorithm for suffix stripping* (Program,
//! 14(3), 130–137) defines a five-step rule-based stemmer for English.
//! It is the reference stemmer for information-retrieval work and the
//! ancestor of every subsequent "Snowball" stemmer. This module
//! implements the algorithm as stated in the paper (and mirrored on
//! Martin Porter's [tartarus.org page][ref]) — not the improved
//! Porter2 / Snowball variant, which introduces additional rules and
//! a slightly different conditional structure.
//!
//! [ref]: https://tartarus.org/martin/PorterStemmer/
//!
//! # Algorithm sketch
//!
//! Each word is walked through five ordered steps of suffix-stripping
//! rules. The rules are all of the form "if suffix `S1` matches and
//! condition `C` holds on the stem, replace `S1` with `S2`". The
//! conditions are all measured over the stem in terms of Porter's
//! notation:
//!
//! * A **consonant** is a letter other than `A, E, I, O, U`, and also
//!   `Y` when preceded by a consonant (so `TOY` is C-V-C but `SYZYGY`
//!   is C-V-C-V-C-V).
//! * The **measure** `m` of a word is the number of vowel-to-consonant
//!   transitions after any initial run of consonants. `TR` has m=0;
//!   `TREE` has m=0 (no V→C); `TROUBLE` has m=2.
//! * Various letter tests: `*S` (ends in S), `*v*` (contains a vowel),
//!   `*d` (ends in double consonant), `*o` (ends in CVC where the last
//!   C is not W/X/Y).
//!
//! The five steps address, in order: plural / past-participle
//! stripping (1a, 1b), Y→I (1c), long-suffix mappings (2, 3), residual
//! suffix stripping (4), and final -E / double-L cleanup (5).
//!
//! # Non-goals
//!
//! * **Porter2 (Snowball).** Porter2's revised algorithm is a follow-up
//!   for a later wave; the classic Porter algorithm is the reference
//!   the paper and every other library documents against and is enough
//!   for v0.1.
//! * **Case preservation.** Porter's algorithm operates on lowercase
//!   ASCII. This implementation lowercases the input at the entry point
//!   and returns a lowercase stem; callers who need case preservation
//!   must layer it themselves.
//! * **Non-ASCII input.** The algorithm's rules are English-and-ASCII
//!   only. Non-ASCII characters are passed through unchanged (they
//!   don't classify as vowels or consonants, so the measure counts
//!   ignore them); the stem this produces is not meaningful for
//!   non-English input.

use alloc::borrow::Cow;
use alloc::string::{String, ToString};

use stringcheese_lang::Stemmer;

/// The Porter (1980) stemmer.
///
/// A zero-sized unit value; construct as `Porter` and reuse the value
/// freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_en::Porter;
/// use stringcheese_lang::Stemmer;
///
/// assert_eq!(Porter.stem("caresses"), "caress");
/// assert_eq!(Porter.stem("ponies"), "poni");
/// assert_eq!(Porter.stem("running"), "run");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Porter;

impl Porter {
    /// Stems `word` per the Porter (1980) algorithm.
    ///
    /// Returns a lowercase ASCII stem. If `word` is already lowercase
    /// ASCII and the algorithm leaves it unchanged, the returned
    /// [`Cow`] borrows the input.
    #[must_use]
    pub fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        // Porter's rules operate on lowercase ASCII. If the input has
        // any uppercase letter (or any non-ASCII scalar) we must copy.
        let (buf, borrowed) = normalize(word);
        // Words of length <=2 are never modified by Porter (his own
        // condition on the paper's algorithm). Return them as-is.
        if buf.len() <= 2 {
            return borrowed.unwrap_or(Cow::Owned(buf));
        }
        let mut word = buf;
        step_1a(&mut word);
        step_1b(&mut word);
        step_1c(&mut word);
        step_2(&mut word);
        step_3(&mut word);
        step_4(&mut word);
        step_5a(&mut word);
        step_5b(&mut word);

        // If we allocated the working buffer only to lowercase and the
        // algorithm returned it unchanged, we still hand back an owned
        // value — the caller sees the *lowercased* stem, which may
        // differ from the input by case. Only the fast path (already
        // lowercase, algorithm made no change) preserves the borrow.
        match borrowed {
            Some(orig) if orig == word => orig,
            _ => Cow::Owned(word),
        }
    }
}

impl Stemmer for Porter {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        Porter::stem(self, word)
    }
}

/// Normalizes the input to lowercase ASCII in an owned `String`.
///
/// The second return value is `Some(Cow::Borrowed(word))` when the
/// input was already lowercase ASCII — this lets `Porter::stem`
/// preserve the borrow when the algorithm makes no further changes.
fn normalize(word: &str) -> (String, Option<Cow<'_, str>>) {
    if word.bytes().all(|b| b.is_ascii_lowercase()) {
        (word.to_string(), Some(Cow::Borrowed(word)))
    } else {
        (word.to_ascii_lowercase(), None)
    }
}

// ---------------------------------------------------------------------------
// Porter's letter-class predicates.
//
// All the predicates operate on the byte string; they take the byte
// slice plus an index and return true/false. The word is guaranteed
// to be lowercase ASCII on entry (per `normalize`), so we can compare
// bytes directly.
// ---------------------------------------------------------------------------

/// Is byte `b` an ASCII vowel (`a e i o u`)?
#[inline]
fn is_vowel_byte(b: u8) -> bool {
    matches!(b, b'a' | b'e' | b'i' | b'o' | b'u')
}

/// Is the character at position `i` in `w` a consonant per Porter's
/// definition (any letter except a/e/i/o/u; `y` is a consonant if the
/// preceding letter is a vowel — i.e. a vowel-context makes `y`
/// consonantal, and vice versa)?
fn is_consonant(w: &[u8], i: usize) -> bool {
    let b = w[i];
    if is_vowel_byte(b) {
        return false;
    }
    if b == b'y' {
        // Y is a consonant if the preceding letter is a vowel (or if
        // it's the very first letter). Equivalently: Y is a consonant
        // iff it is at position 0 or the preceding letter is not a
        // consonant.
        if i == 0 {
            return true;
        }
        return !is_consonant(w, i - 1);
    }
    true
}

/// The Porter measure `m(w)` — the number of vowel-to-consonant
/// transitions after any initial consonant run.
///
/// Equivalently: `m(w)` counts the number of `[C]VC` chunks in the
/// word's condensed class string.
fn measure(w: &[u8]) -> usize {
    let n = w.len();
    let mut m = 0usize;
    let mut i = 0usize;

    // Skip initial consonants.
    while i < n && is_consonant(w, i) {
        i += 1;
    }

    // Alternately consume a vowel-run and a consonant-run.
    loop {
        // Consume a vowel-run.
        while i < n && !is_consonant(w, i) {
            i += 1;
        }
        if i >= n {
            break;
        }
        m += 1;
        // Consume the following consonant-run.
        while i < n && is_consonant(w, i) {
            i += 1;
        }
        if i >= n {
            break;
        }
    }

    m
}

/// Does the word contain a vowel (`*v*`)?
fn contains_vowel(w: &[u8]) -> bool {
    (0..w.len()).any(|i| !is_consonant(w, i))
}

/// Does the word end in a double consonant (`*d`)? The two consonants
/// must be the same letter (so `bb`, `tt` count; `st` does not).
fn ends_double_consonant(w: &[u8]) -> bool {
    let n = w.len();
    if n < 2 {
        return false;
    }
    w[n - 1] == w[n - 2] && is_consonant(w, n - 1) && is_consonant(w, n - 2)
}

/// Does the word end `cvc` where the second `c` is not `w`, `x`, or
/// `y`? (Porter's `*o` predicate.)
fn ends_cvc(w: &[u8]) -> bool {
    let n = w.len();
    if n < 3 {
        return false;
    }
    let last = w[n - 1];
    is_consonant(w, n - 3)
        && !is_consonant(w, n - 2)
        && is_consonant(w, n - 1)
        && !matches!(last, b'w' | b'x' | b'y')
}

// ---------------------------------------------------------------------------
// Step 1a: Plural stripping.
//
//   SSES -> SS       (caresses -> caress)
//   IES  -> I        (ponies -> poni; ties -> ti)
//   SS   -> SS       (caress -> caress) [no change]
//   S    -> (delete) (cats -> cat)
// ---------------------------------------------------------------------------

fn step_1a(w: &mut String) {
    if w.ends_with("sses") {
        w.truncate(w.len() - 2); // sses -> ss
    } else if w.ends_with("ies") {
        w.truncate(w.len() - 2); // ies -> i
    } else if w.ends_with("ss") {
        // no change
    } else if w.ends_with('s') {
        w.pop();
    }
}

// ---------------------------------------------------------------------------
// Step 1b: Past-tense / progressive stripping.
//
//   (m>0) EED -> EE                (agreed -> agree; feed -> feed)
//   (*v*) ED  -> (delete)          (plastered -> plaster; bled -> bled)
//   (*v*) ING -> (delete)          (motoring -> motor; sing -> sing)
//
// If the second or third rule succeeded, apply the "cleanup" tail:
//   AT -> ATE                      (conflat(ed) -> conflate)
//   BL -> BLE                      (troubl(ed) -> trouble)
//   IZ -> IZE                      (siz(ed) -> size)
//   (*d and not (*L or *S or *Z)) -> single letter
//                                   (hopp(ing) -> hop; tann(ed) -> tan)
//   (m=1 and *o) -> add E          (fil(ing) -> file)
// ---------------------------------------------------------------------------

fn step_1b(w: &mut String) {
    // (m>0) EED -> EE
    if w.ends_with("eed") {
        // stem = w without "eed"
        let stem_len = w.len() - 3;
        if measure(&w.as_bytes()[..stem_len]) > 0 {
            w.truncate(w.len() - 1); // eed -> ee
        }
        return;
    }

    // (*v*) ED -> (delete)
    if w.ends_with("ed") {
        let stem_len = w.len() - 2;
        if contains_vowel(&w.as_bytes()[..stem_len]) {
            w.truncate(stem_len);
            step_1b_cleanup(w);
        }
        return;
    }

    // (*v*) ING -> (delete)
    if w.ends_with("ing") {
        let stem_len = w.len() - 3;
        if contains_vowel(&w.as_bytes()[..stem_len]) {
            w.truncate(stem_len);
            step_1b_cleanup(w);
        }
    }
}

fn step_1b_cleanup(w: &mut String) {
    if w.ends_with("at") || w.ends_with("bl") || w.ends_with("iz") {
        w.push('e');
        return;
    }
    if ends_double_consonant(w.as_bytes()) {
        let last = *w.as_bytes().last().unwrap();
        if !matches!(last, b'l' | b's' | b'z') {
            w.pop();
        }
        return;
    }
    if measure(w.as_bytes()) == 1 && ends_cvc(w.as_bytes()) {
        w.push('e');
    }
}

// ---------------------------------------------------------------------------
// Step 1c: Terminal Y -> I after a vowel.
//
//   (*v*) Y -> I    (happy -> happi; sky -> sky)
// ---------------------------------------------------------------------------

fn step_1c(w: &mut String) {
    if w.ends_with('y') {
        let stem_len = w.len() - 1;
        if contains_vowel(&w.as_bytes()[..stem_len]) {
            w.pop();
            w.push('i');
        }
    }
}

// ---------------------------------------------------------------------------
// Step 2: Long-suffix mappings (m>0 on stem).
// ---------------------------------------------------------------------------

// Porter's (1980) step 2 table, faithful to the original paper. Later
// revisions (Porter2 / Snowball; Vivake Gupta's Python port) change
// "abli -> able" to "bli -> ble" and add "logi -> log"; we stick with
// the 1980 paper.
const STEP_2: &[(&str, &str)] = &[
    ("ational", "ate"),
    ("tional", "tion"),
    ("enci", "ence"),
    ("anci", "ance"),
    ("izer", "ize"),
    ("abli", "able"),
    ("alli", "al"),
    ("entli", "ent"),
    ("eli", "e"),
    ("ousli", "ous"),
    ("ization", "ize"),
    ("ation", "ate"),
    ("ator", "ate"),
    ("alism", "al"),
    ("iveness", "ive"),
    ("fulness", "ful"),
    ("ousness", "ous"),
    ("aliti", "al"),
    ("iviti", "ive"),
    ("biliti", "ble"),
];

fn step_2(w: &mut String) {
    apply_measure_rules(w, STEP_2, |m| m > 0);
}

// ---------------------------------------------------------------------------
// Step 3: More long-suffix mappings (m>0 on stem).
// ---------------------------------------------------------------------------

const STEP_3: &[(&str, &str)] = &[
    ("icate", "ic"),
    ("ative", ""),
    ("alize", "al"),
    ("iciti", "ic"),
    ("ical", "ic"),
    ("ful", ""),
    ("ness", ""),
];

fn step_3(w: &mut String) {
    apply_measure_rules(w, STEP_3, |m| m > 0);
}

// ---------------------------------------------------------------------------
// Step 4: Residual suffix stripping (m>1 on stem).
//
// Special: (m>1 and (*S or *T)) ION -> (delete)
// ---------------------------------------------------------------------------

// Applied first; a suffix here strips to the empty string.
const STEP_4_PLAIN: &[&str] = &[
    "al", "ance", "ence", "er", "ic", "able", "ible", "ant", "ement", "ment", "ent", "ou", "ism",
    "ate", "iti", "ous", "ive", "ize",
];

fn step_4(w: &mut String) {
    // Special ION case first, so that the plain rules that also match
    // (e.g. no plain "ion" rule, so fine — but tie-breaking still
    // matters for the longest-match preference below).

    // The plain rules must be tried longest-first to preserve
    // longest-match semantics (e.g. `ement` before `ent` before `er`).
    for &suffix in STEP_4_PLAIN {
        if w.ends_with(suffix) {
            let stem_len = w.len() - suffix.len();
            if measure(&w.as_bytes()[..stem_len]) > 1 {
                w.truncate(stem_len);
            }
            return;
        }
    }

    // ION rule (special condition: stem must end in S or T).
    if w.ends_with("ion") {
        let stem_len = w.len() - 3;
        let stem = &w.as_bytes()[..stem_len];
        if measure(stem) > 1
            && let Some(&last) = stem.last()
            && (last == b's' || last == b't')
        {
            w.truncate(stem_len);
        }
    }
}

// ---------------------------------------------------------------------------
// Step 5a: Terminal E.
//
//   (m>1) E -> (delete)               (probate -> probat; rate -> rate)
//   (m=1 and not *o) E -> (delete)    (cease -> ceas)
// ---------------------------------------------------------------------------

fn step_5a(w: &mut String) {
    if w.ends_with('e') {
        let stem_len = w.len() - 1;
        let stem = &w.as_bytes()[..stem_len];
        let m = measure(stem);
        if m > 1 || (m == 1 && !ends_cvc(stem)) {
            w.truncate(stem_len);
        }
    }
}

// ---------------------------------------------------------------------------
// Step 5b: Double-L cleanup.
//
//   (m>1 and *d and *L) -> single letter    (controll -> control)
// ---------------------------------------------------------------------------

fn step_5b(w: &mut String) {
    if measure(w.as_bytes()) > 1 && ends_double_consonant(w.as_bytes()) && w.ends_with('l') {
        w.pop();
    }
}

/// Apply a set of `(suffix, replacement)` rules, longest match wins,
/// and only fire the rule if `predicate(measure(stem))` holds.
fn apply_measure_rules<F>(w: &mut String, rules: &[(&str, &str)], predicate: F)
where
    F: Fn(usize) -> bool,
{
    // The rules table is authored longest-suffix-first, but we don't
    // rely on that: sort by descending suffix length in place. Since
    // the tables are `&[...]` and we can't sort them at runtime here
    // without allocating, we instead scan for the longest match.
    let mut best: Option<(usize, &str)> = None;
    for &(suf, rep) in rules {
        if w.ends_with(suf) && best.is_none_or(|(len, _)| suf.len() > len) {
            best = Some((suf.len(), rep));
        }
    }
    let Some((suf_len, rep)) = best else { return };
    let stem_len = w.len() - suf_len;
    if predicate(measure(&w.as_bytes()[..stem_len])) {
        w.truncate(stem_len);
        w.push_str(rep);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &str) -> String {
        Porter.stem(w).into_owned()
    }

    #[test]
    fn measure_matches_paper_examples() {
        // From Porter's paper (§2):
        //   TR       -> m=0
        //   EE       -> m=0
        //   TREE     -> m=0
        //   Y        -> m=0
        //   BY       -> m=0
        //   TROUBLE  -> m=1
        //   OATS     -> m=1
        //   TREES    -> m=1
        //   IVY      -> m=1
        //   TROUBLES -> m=2
        //   PRIVATE  -> m=2
        //   OATEN    -> m=2
        //   ORRERY   -> m=2
        assert_eq!(measure(b"tr"), 0);
        assert_eq!(measure(b"ee"), 0);
        assert_eq!(measure(b"tree"), 0);
        assert_eq!(measure(b"y"), 0);
        assert_eq!(measure(b"by"), 0);
        assert_eq!(measure(b"trouble"), 1);
        assert_eq!(measure(b"oats"), 1);
        assert_eq!(measure(b"trees"), 1);
        assert_eq!(measure(b"ivy"), 1);
        assert_eq!(measure(b"troubles"), 2);
        assert_eq!(measure(b"private"), 2);
        assert_eq!(measure(b"oaten"), 2);
        assert_eq!(measure(b"orrery"), 2);
    }

    #[test]
    fn short_words_are_unchanged() {
        assert_eq!(s("a"), "a");
        assert_eq!(s("at"), "at");
    }

    #[test]
    fn step_1a_paper_examples() {
        // Porter's own examples (§3, step 1a).
        assert_eq!(s("caresses"), "caress");
        assert_eq!(s("ponies"), "poni");
        assert_eq!(s("ties"), "ti");
        assert_eq!(s("caress"), "caress");
        assert_eq!(s("cats"), "cat");
    }

    #[test]
    fn step_1b_paper_examples() {
        // §3, step 1b — with full 5-step processing applied. The
        // per-step outputs in Porter's paper are `agreed -> agree`,
        // `conflated -> conflate`, `troubled -> trouble`, but step 5a
        // then trims the trailing e where its condition holds.
        assert_eq!(s("feed"), "feed"); // EED, m=0 on "f"
        assert_eq!(s("agreed"), "agre"); // EED -> agree; step 5a strips e
        assert_eq!(s("plastered"), "plaster");
        assert_eq!(s("bled"), "bled");
        assert_eq!(s("motoring"), "motor");
        assert_eq!(s("sing"), "sing");
        assert_eq!(s("conflated"), "conflat"); // -> conflate; step 5a strips e
        assert_eq!(s("troubled"), "troubl"); // -> trouble; step 5a strips e
        assert_eq!(s("sized"), "size"); // -> siz -> size; step 5a keeps e (*o)
        assert_eq!(s("hopping"), "hop");
        assert_eq!(s("tanned"), "tan");
        assert_eq!(s("falling"), "fall");
        assert_eq!(s("hissing"), "hiss");
        assert_eq!(s("fizzed"), "fizz");
        assert_eq!(s("failing"), "fail");
        assert_eq!(s("filing"), "file"); // -> fil -> file; step 5a keeps e (*o)
    }

    #[test]
    fn step_1c_paper_examples() {
        // §3, step 1c.
        assert_eq!(s("happy"), "happi");
        assert_eq!(s("sky"), "sky");
    }

    #[test]
    fn step_2_paper_examples() {
        // §3, step 2 — again, expected values reflect the full 5-step
        // Porter output, so what Porter's paper shows as `relational ->
        // relate` becomes `relational -> relat` here (step 5a strips
        // the trailing e whenever `m>1`). The words for which none of
        // step 3, 4, or 5 fire (vileli, feudalism, callousness,
        // formaliti, hopefulness) match the paper's per-step output
        // exactly.
        assert_eq!(s("relational"), "relat");
        assert_eq!(s("conditional"), "condit");
        assert_eq!(s("valenci"), "valenc");
        assert_eq!(s("hesitanci"), "hesit");
        assert_eq!(s("digitizer"), "digit");
        assert_eq!(s("conformabli"), "conform");
        assert_eq!(s("radicalli"), "radic");
        assert_eq!(s("differentli"), "differ");
        assert_eq!(s("vileli"), "vile");
        assert_eq!(s("analogousli"), "analog");
        assert_eq!(s("vietnamization"), "vietnam");
        assert_eq!(s("predication"), "predic");
        assert_eq!(s("operator"), "oper");
        assert_eq!(s("feudalism"), "feudal");
        assert_eq!(s("decisiveness"), "decis");
        assert_eq!(s("hopefulness"), "hope");
        assert_eq!(s("callousness"), "callous");
        assert_eq!(s("formaliti"), "formal");
        assert_eq!(s("sensitiviti"), "sensit");
        assert_eq!(s("sensibiliti"), "sensibl");
    }

    #[test]
    fn step_3_paper_examples() {
        // §3, step 3 — full 5-step outputs (Porter's paper shows
        // `electrical -> electric`; step 4 then strips the -IC to give
        // `electr`).
        assert_eq!(s("triplicate"), "triplic");
        assert_eq!(s("formative"), "form");
        assert_eq!(s("formalize"), "formal");
        assert_eq!(s("electriciti"), "electr");
        assert_eq!(s("electrical"), "electr");
        assert_eq!(s("hopeful"), "hope");
        assert_eq!(s("goodness"), "good");
    }

    #[test]
    fn step_4_paper_examples() {
        // §3, step 4. (m>1)
        assert_eq!(s("revival"), "reviv");
        assert_eq!(s("allowance"), "allow");
        assert_eq!(s("inference"), "infer");
        assert_eq!(s("airliner"), "airlin");
        assert_eq!(s("gyroscopic"), "gyroscop");
        assert_eq!(s("adjustable"), "adjust");
        assert_eq!(s("defensible"), "defens");
        assert_eq!(s("irritant"), "irrit");
        assert_eq!(s("replacement"), "replac");
        assert_eq!(s("adjustment"), "adjust");
        assert_eq!(s("dependent"), "depend");
        assert_eq!(s("adoption"), "adopt");
        assert_eq!(s("homologous"), "homolog");
        assert_eq!(s("effective"), "effect");
        assert_eq!(s("bowdlerize"), "bowdler");
    }

    #[test]
    fn step_5_paper_examples() {
        // §3, step 5a.
        assert_eq!(s("probate"), "probat");
        assert_eq!(s("rate"), "rate");
        assert_eq!(s("cease"), "ceas");
        // §3, step 5b.
        assert_eq!(s("controll"), "control");
        assert_eq!(s("roll"), "roll");
    }

    /// Porter (1980) is *not* universally idempotent when the stem
    /// re-enters an earlier step. `agreed -> agre -> agr` and
    /// `filing -> file -> file` (idempotent) sit side by side. The
    /// property test module documents this and skips the guarantee
    /// on synthetic input; here we just spot-check the words for
    /// which Porter's algorithm *does* reach a fixed point in one
    /// pass — that covers most real English vocabulary.
    #[test]
    fn stem_is_idempotent_on_the_paper_vocabulary_where_expected() {
        // Words whose full-Porter output is also a fixed point of
        // Porter (i.e. `stem(stem(w)) == stem(w)`).
        for w in [
            "caresses",
            "ponies",
            "caress",
            "cats",
            "feed",
            "plastered",
            "bled",
            "motoring",
            "sing",
            "sized",
            "hopping",
            "tanned",
            "falling",
            "hissing",
            "fizzed",
            "failing",
            "filing",
            "happy",
            "sky",
            "feudalism",
            "hopeful",
            "goodness",
            "revival",
            "allowance",
            "adjustment",
            "controll",
        ] {
            let once = Porter.stem(w).into_owned();
            let twice = Porter.stem(&once).into_owned();
            assert_eq!(once, twice, "Porter.stem is not idempotent on {w:?}");
        }
    }

    #[test]
    fn uppercase_input_is_lowercased() {
        assert_eq!(s("CARESSES"), "caress");
        assert_eq!(s("Caresses"), "caress");
        assert_eq!(s("PONIES"), "poni");
    }

    #[test]
    fn empty_and_single_char_are_unchanged() {
        assert_eq!(s(""), "");
        assert_eq!(s("a"), "a");
        assert_eq!(s("i"), "i");
    }

    #[test]
    fn borrows_input_when_lowercase_and_unchanged() {
        // Porter leaves "caress" alone; input is lowercase ASCII, so
        // the returned Cow must borrow.
        let input = "caress";
        let out = Porter.stem(input);
        assert!(matches!(out, Cow::Borrowed(_)));
    }

    #[test]
    fn cvc_predicate_examples() {
        // From Porter's paper: WIL is *o, HOP is *o, but SNOW, BOX,
        // TRAY are NOT *o (W, X, Y suppress it).
        assert!(ends_cvc(b"wil"));
        assert!(ends_cvc(b"hop"));
        assert!(!ends_cvc(b"snow"));
        assert!(!ends_cvc(b"box"));
        assert!(!ends_cvc(b"tray"));
    }

    #[test]
    fn consonant_predicate_examples() {
        // TOY: T-O-Y, y is preceded by o (vowel) so y is a consonant.
        assert!(is_consonant(b"toy", 0)); // t
        assert!(!is_consonant(b"toy", 1)); // o
        assert!(is_consonant(b"toy", 2)); // y after vowel

        // SYZYGY: s-y-z-y-g-y, y at 1 preceded by s (consonant) so y is
        // a vowel; y at 3 preceded by z (consonant) so y is a vowel; y
        // at 5 preceded by g (consonant) so y is a vowel.
        assert!(is_consonant(b"syzygy", 0)); // s
        assert!(!is_consonant(b"syzygy", 1)); // y after consonant -> vowel
        assert!(is_consonant(b"syzygy", 2)); // z
        assert!(!is_consonant(b"syzygy", 3)); // y after consonant -> vowel
        assert!(is_consonant(b"syzygy", 4)); // g
        assert!(!is_consonant(b"syzygy", 5)); // y after consonant -> vowel
    }
}
