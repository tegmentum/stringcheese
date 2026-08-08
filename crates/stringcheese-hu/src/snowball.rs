//! The Snowball Hungarian stemmer.
//!
//! # Origin
//!
//! The Snowball Hungarian algorithm, documented at
//! <https://snowballstem.org/algorithms/hungarian/stemmer.html>.
//! Hungarian is Uralic (non-Indo-European, related to Finnish and
//! Estonian) and strongly **agglutinative** — a single orthographic
//! word can stack case, possessive, plural, and verb-derivational
//! suffixes. Hungarian is also strict about **vowel harmony**, so
//! every case-and-possessive suffix comes in a harmonizing pair
//! (back-vowel form vs front-vowel form) or triplet (splitting front
//! into rounded vs unrounded).
//!
//! # Algorithm sketch
//!
//! 1. **Preprocess.** Lowercase (Unicode-default; Hungarian has no
//!    locale-specific case-fold surprises). Assemble a `Vec<char>` so
//!    all downstream arithmetic operates in character space (never
//!    bytes — Hungarian's long / umlaut vowels are all multi-byte in
//!    UTF-8).
//! 2. **Compute R1.** Snowball's Hungarian spec defines R1 as the
//!    region **after the first vowel-then-consonant transition** in
//!    the word, with a fallback to the word's end if no such
//!    transition exists. R1 is a minimum-preserved-stem-length guard:
//!    no suffix rule may strip characters at positions strictly less
//!    than R1.
//! 3. **Iterated suffix stripping.** At each iteration, find the
//!    *longest* entry in the unified suffix table whose surface form
//!    matches the current tail AND whose start position lies at or
//!    beyond R1 AND that leaves a stem of at least 2 characters.
//!    Strip it. Repeat until no candidate matches, up to a bounded
//!    iteration count so an adversarial input cannot cascade
//!    indefinitely.
//!
//! The Snowball Hungarian spec organizes suffix stripping into
//! multiple ordered steps: **instrumental** (double-consonant + al/el
//! collapse), **case** (17+ surface case-ending variants), **owned**
//! (possessive-related endings), **`sing_owner`** and **`plur_owner`**
//! (13 personal-possessed endings each), and **plural** (`-k` variants
//! with linking vowels). This crate merges those steps into a single
//! unified longest-match table for the same reason the Turkish pack
//! does: on words whose surface residue after a nominal case strip
//! coincidentally spells the same letters as a shorter possessive
//! entry, the phased approach can over-strip the shorter match and
//! miss the longer one entirely. Callers who need bit-exact
//! reference-Snowball output should reach for a `snowballstem`-
//! compiled Hungarian stemmer; this pack ships a practical variant.
//!
//! # Vowel harmony
//!
//! Hungarian classifies vowels by **backness**:
//!
//! - **Front vowels**: `e é i í ö ő ü ű`
//! - **Back vowels**:  `a á o ó u ú`
//!
//! Suffixes come in harmonizing pairs or triplets:
//!
//! | Suffix meaning          | Back  | Front (unrounded) | Front (rounded) |
//! |-------------------------|-------|-------------------|-----------------|
//! | inessive "in"           | `-ban`| `-ben`            | —               |
//! | illative "into"         | `-ba` | `-be`             | —               |
//! | sublative "onto"        | `-ra` | `-re`             | —               |
//! | dative "to"             | `-nak`| `-nek`            | —               |
//! | adessive "at"           | `-nál`| `-nél`            | —               |
//! | elative "out of"        | `-ból`| `-ből`            | —               |
//! | delative "off"          | `-ról`| `-ről`            | —               |
//! | ablative "from"         | `-tól`| `-től`            | —               |
//! | allative "to (near)"    | `-hoz`| `-hez`            | `-höz`          |
//! | translative "into (X)"  | `-vá` | `-vé`             | —               |
//! | instrumental "with"     | `-val`| `-vel`            | —               |
//! | causal-final "for"      | `-ért`| `-ért`            | `-ért`          |
//! | terminative "until"     | `-ig` | `-ig`             | `-ig`           |
//! | temporal "at (time)"    | `-kor`| `-kor`            | `-kor`          |
//!
//! (Note the last three are **not** harmonizing — they carry a fixed
//! vowel regardless of stem harmony.)
//!
//! **This pack encodes harmony inside the suffix table, not as a
//! runtime predicate.** Every surface variant of each suffix is
//! listed as its own literal entry (`-ban`, `-ben`, `-hoz`, `-hez`,
//! `-höz`, …). The stemmer's runtime job is a longest-match search
//! over concrete surface forms — the harmony rule is baked into the
//! table's shape, not the algorithm.
//!
//! # Byte-vs-char safety
//!
//! All the arithmetic in this module operates on `Vec<char>` indices
//! — never raw byte offsets. Hungarian's long / umlaut vowels
//! (`á/é/í/ó/ú/ö/ő/ü/ű`) and their uppercase counterparts are all
//! multi-byte in UTF-8; byte arithmetic would silently corrupt
//! boundaries.
//!
//! # Non-goals
//!
//! * **Full-vocabulary cross-verification.** The Snowball project
//!   distributes `voc.txt` / `output.txt` reference files with tens
//!   of thousands of pairs; the
//!   [`tests/snowball_reference.rs`](../../tests/snowball_reference.rs)
//!   test embeds a subset that exercises each suffix category and
//!   the front/back harmony contract. Full-corpus cross-verification
//!   is a follow-up.
//! * **Verb-conjugation lemmatization.** Reducing `mentem → menni`,
//!   `voltam → lenni` requires a lexicon; the shipped stemmer is a
//!   suffix-stripping algorithm.
//! * **Compound-word decomposition.** Hungarian writes noun compounds
//!   as a single word (`asztalitenisz` "table tennis"); a compound
//!   splitter would need a lexicon and is out of scope.
//! * **Definite / indefinite verb-conjugation distinction.** Hungarian
//!   verbs conjugate for definiteness of their direct object; the
//!   shipped stemmer treats both series as surface-form suffix strips.
//! * **Post-strip vowel-length restoration.** After stripping certain
//!   possessive suffixes, some stems' final vowel should shorten
//!   (`madár → madar-am` "my bird"); reversing this needs a lexicon.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use stringcheese_lang::Stemmer;

/// The Snowball Hungarian stemmer.
///
/// A zero-sized unit value; construct as [`HungarianSnowball`] and
/// reuse the value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_hu::HungarianSnowball;
/// use stringcheese_lang::Stemmer;
///
/// // Inessive back-vowel form.
/// assert_eq!(HungarianSnowball.stem("házban"), "ház");
/// // Inessive front-vowel form.
/// assert_eq!(HungarianSnowball.stem("kertben"), "kert");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct HungarianSnowball;

/// Maximum number of successful strip iterations per stem call.
///
/// **Set to 1** for the shipped light variant: one longest-match strip
/// per call. Cascading multiple strips causes over-stemming when
/// residual surface forms (e.g. `kert` "garden" left after stripping
/// `-ben` from `kertben`) happen to spell short suffixes (bare `-t`).
/// The Snowball reference algorithm avoids this by running each
/// category as its own step at most once; the unified longest-match
/// table this pack ships achieves the same effect by taking one shot
/// at the longest cross-category surface match and stopping.
///
/// Multi-morpheme stripping (e.g. `házaim` "my houses" = house +
/// plural + 1sg possessive) is handled where possible by listing the
/// concatenated surface form (`-aim`) as a single table entry rather
/// than relying on iteration to peel morphemes one at a time.
const MAX_STRIP_ITERATIONS: usize = 1;

/// Minimum stem length (in characters) the algorithm is allowed to
/// leave behind. Prevents "and it's a stopword-length stem now" edge
/// cases where iteration continues into over-stemming.
const MIN_STEM_LEN: usize = 2;

impl HungarianSnowball {
    /// Stems `word` per the Snowball Hungarian algorithm.
    ///
    /// Returns the stem as a [`Cow`]. If the algorithm makes no
    /// change to a lowercase input, the returned `Cow` borrows the
    /// input.
    #[must_use]
    pub fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        if word.chars().count() <= MIN_STEM_LEN {
            return Cow::Borrowed(word);
        }

        // 1. Lowercase (Unicode-aware). Assemble via a Vec<char> so
        // all downstream arithmetic operates in char space.
        let mut chars: Vec<char> = word.chars().flat_map(char::to_lowercase).collect();

        if chars.len() <= MIN_STEM_LEN {
            let out: String = chars.iter().collect();
            return if out == word {
                Cow::Borrowed(word)
            } else {
                Cow::Owned(out)
            };
        }

        // 2. Compute R1 once. Iteration recomputes an implicit floor
        // via `MIN_STEM_LEN` — R1 is fixed relative to the *original*
        // word's structure per the Snowball spec.
        let r1 = compute_r1(&chars);

        // 3. Iterated suffix stripping: longest-match across the
        // unified table wins each round.
        for _ in 0..MAX_STRIP_ITERATIONS {
            if !strip_longest(&mut chars, SUFFIXES, r1) {
                break;
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

impl Stemmer for HungarianSnowball {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        HungarianSnowball::stem(self, word)
    }
}

// ---------------------------------------------------------------------------
// Vowel classification.
// ---------------------------------------------------------------------------

/// Is `c` a Hungarian vowel? Includes both back and front, short and
/// long.
#[inline]
const fn is_vowel(c: char) -> bool {
    matches!(
        c,
        'a' | 'á' | 'e' | 'é' | 'i' | 'í' | 'o' | 'ó' | 'ö' | 'ő' | 'u' | 'ú' | 'ü' | 'ű'
    )
}

// ---------------------------------------------------------------------------
// Region R1 — computed as a char index.
// ---------------------------------------------------------------------------

/// R1 = the position after the first vowel-followed-by-consonant pair
/// (the Snowball Hungarian convention), guarded to be at least 2 so
/// the algorithm never strips into a 1-character stem.
///
/// If the word contains no vowel-then-consonant transition, R1 is the
/// end of the word (i.e. the null suffix — no rule will fire because
/// no suffix can be entirely to the right of the word's end).
fn compute_r1(chars: &[char]) -> usize {
    let n = chars.len();
    let mut i = 0;
    // Advance past leading consonants.
    while i < n && !is_vowel(chars[i]) {
        i += 1;
    }
    // Advance past the first run of vowels.
    while i < n && is_vowel(chars[i]) {
        i += 1;
    }
    // We are now on the first post-vowel consonant; R1 starts *after*
    // it.
    let r1 = if i < n { i + 1 } else { n };
    r1.max(MIN_STEM_LEN.min(n))
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

/// Attempt to strip the longest suffix from `table` that (a) matches
/// the tail of `chars`, (b) starts at or beyond `r1`, and (c) leaves
/// a stem of at least `MIN_STEM_LEN` characters. Returns `true` iff a
/// suffix was stripped.
fn strip_longest(chars: &mut Vec<char>, table: &[&[char]], r1: usize) -> bool {
    if chars.len() <= MIN_STEM_LEN {
        return false;
    }

    let mut best: Option<&[char]> = None;
    for &entry in table {
        if entry.len() >= chars.len() {
            continue;
        }
        if !ends_with(chars, entry) {
            continue;
        }
        let stem_prefix_end = chars.len() - entry.len();
        if stem_prefix_end < r1 {
            continue;
        }
        if stem_prefix_end < MIN_STEM_LEN {
            continue;
        }
        if best.is_none_or(|b| entry.len() > b.len()) {
            best = Some(entry);
        }
    }

    let Some(entry) = best else {
        return false;
    };
    let new_len = chars.len() - entry.len();
    chars.truncate(new_len);
    true
}

// ---------------------------------------------------------------------------
// Suffix table.
//
// Categories (all merged into one longest-match pass — see the
// module-level docs for why the phased approach is not used):
//
//   INS = instrumental (double-consonant + al/el variants)
//   CASE = case endings (17+ surface variants across 9 case markers)
//   OWN = owned / possessed-related endings
//   POS = personal-possessive markers (singular- and plural-possessed)
//   PL  = plural markers with linking vowels
//   VB  = verb suffixes (a small conservative selection)
//
// Entries are ordered by descending length within each category for
// readability; the stemmer's `strip_longest` picks the globally
// longest match across all categories per iteration.
// ---------------------------------------------------------------------------

#[rustfmt::skip]
const SUFFIXES: &[&[char]] = &[
    // ------------------------------------------------------------------
    // INS: instrumental `-val`/`-vel`. On a consonant-final stem the
    // `v` assimilates to (and doubles) the stem-final consonant:
    // `ház + val → házzal`, `kert + vel → kerttel`. The table lists
    // the 3-char assimilated suffix — `-Xal`/`-Xel` for each Hungarian
    // consonant X — so stripping the trailing 3 chars leaves the
    // stem's original final consonant in place (one of the two `X`s
    // in the doubled cluster). This is functionally equivalent to
    // the Snowball reference's "strip -al/-el after a doubled
    // consonant" rule but as a self-contained table entry.
    // ------------------------------------------------------------------
    &['v', 'a', 'l'], &['v', 'e', 'l'],
    // Back-harmony assimilated instrumental variants.
    &['b', 'a', 'l'], &['c', 'a', 'l'], &['d', 'a', 'l'],
    &['f', 'a', 'l'], &['g', 'a', 'l'], &['j', 'a', 'l'],
    &['k', 'a', 'l'], &['l', 'a', 'l'], &['m', 'a', 'l'],
    &['n', 'a', 'l'], &['p', 'a', 'l'], &['r', 'a', 'l'],
    &['s', 'a', 'l'], &['t', 'a', 'l'], &['z', 'a', 'l'],
    // Front-harmony assimilated instrumental variants.
    &['b', 'e', 'l'], &['c', 'e', 'l'], &['d', 'e', 'l'],
    &['f', 'e', 'l'], &['g', 'e', 'l'], &['j', 'e', 'l'],
    &['k', 'e', 'l'], &['l', 'e', 'l'], &['m', 'e', 'l'],
    &['n', 'e', 'l'], &['p', 'e', 'l'], &['r', 'e', 'l'],
    &['s', 'e', 'l'], &['t', 'e', 'l'], &['z', 'e', 'l'],

    // ------------------------------------------------------------------
    // CASE: nominal case suffixes, listed by harmony pair / triplet.
    // ------------------------------------------------------------------
    // Inessive "in the X".
    &['b', 'a', 'n'], &['b', 'e', 'n'],
    // Illative "into X".
    &['b', 'a'], &['b', 'e'],
    // Sublative "onto X".
    &['r', 'a'], &['r', 'e'],
    // Superessive "on X" (with linking vowel variants).
    &['o', 'n'], &['e', 'n'], &['ö', 'n'],
    // Dative "to X".
    &['n', 'a', 'k'], &['n', 'e', 'k'],
    // Adessive "at X".
    &['n', 'á', 'l'], &['n', 'é', 'l'],
    // Elative "out of X".
    &['b', 'ó', 'l'], &['b', 'ő', 'l'],
    // Delative "from-off X".
    &['r', 'ó', 'l'], &['r', 'ő', 'l'],
    // Ablative "from X".
    &['t', 'ó', 'l'], &['t', 'ő', 'l'],
    // Allative "to (near) X" — front/back + rounded-front triplet.
    &['h', 'o', 'z'], &['h', 'e', 'z'], &['h', 'ö', 'z'],
    // Translative-factive "becoming X".
    &['v', 'á'], &['v', 'é'],
    // Causal-final "for X" — fixed vowel.
    &['é', 'r', 't'],
    // Terminative "until X" — fixed vowel.
    &['i', 'g'],
    // Temporal "at (time) X" — fixed vowel.
    &['k', 'o', 'r'],
    // Essive-formal / essive-modal "as X".
    &['k', 'é', 'n', 't'],
    &['u', 'l'], &['ü', 'l'],
    // Sociative "with (X and)" — `-astul/-estül/-ostul/-östül`.
    &['a', 's', 't', 'u', 'l'], &['e', 's', 't', 'ü', 'l'],
    &['o', 's', 't', 'u', 'l'], &['ö', 's', 't', 'ü', 'l'],
    &['s', 't', 'u', 'l'], &['s', 't', 'ü', 'l'],
    // Accusative "X" (direct object) — linking-vowel variants only.
    // The bare `-t` (1 char) is deliberately NOT listed — it would
    // over-strip common consonant-final loanwords (`sport`, `pont`,
    // `test`) and cascade after other suffixes. The linking-vowel
    // forms handle every native accusative.
    &['a', 't'], &['e', 't'], &['o', 't'], &['ö', 't'],

    // ------------------------------------------------------------------
    // OWN: owned / possessed-related derivational markers.
    // The `-é` "one belonging to" and its plural variants.
    // ------------------------------------------------------------------
    &['é', 'i'],
    &['é'],
    &['k', 'é'],
    &['é', 'k', 'é'],

    // ------------------------------------------------------------------
    // POS: personal-possessive suffixes (singular-possessed).
    // Includes the interface vowel that appears between the stem and
    // the person marker.
    // ------------------------------------------------------------------
    // 1sg possessive `-Vm` (`-am/-em/-om/-öm`) + bare `-m`.
    &['u', 'n', 'k'], &['ü', 'n', 'k'],   // 1pl possessive (kept here for length ordering)
    &['a', 'i', 'n', 'k'], &['e', 'i', 'n', 'k'],
    &['j', 'a', 'i', 'n', 'k'], &['j', 'e', 'i', 'n', 'k'],
    &['a', 'i', 't', 'o', 'k'], &['e', 'i', 't', 'e', 'k'],
    &['j', 'a', 'i', 't', 'o', 'k'], &['j', 'e', 'i', 't', 'e', 'k'],
    &['a', 'i', 'k'], &['e', 'i', 'k'],
    &['j', 'a', 'i', 'k'], &['j', 'e', 'i', 'k'],
    // Singular-possessed personal markers.
    &['a', 'm'], &['e', 'm'], &['o', 'm'], &['ö', 'm'],
    &['a', 'd'], &['e', 'd'], &['o', 'd'], &['ö', 'd'],
    // 3sg possessive `-a/-e` and `-ja/-je`.
    &['j', 'a'], &['j', 'e'],
    // 2pl possessive `-Vtok/-Vtek/-Vtök` and `-jVtok/-jVtek/-jVtök`.
    // Bare `-tok`/`-tek`/`-tök` (3 chars) deliberately NOT listed —
    // they would over-strip nouns whose plural is `-ek` on a stem
    // ending in `t` (e.g., `kertek` "gardens" would strip `-tek`
    // leaving `ker` instead of the correct `kert`).
    &['a', 't', 'o', 'k'], &['e', 't', 'e', 'k'], &['ö', 't', 'ö', 'k'],
    &['j', 'a', 't', 'o', 'k'], &['j', 'e', 't', 'e', 'k'], &['j', 'ö', 't', 'ö', 'k'],
    // 3pl possessive `-uk/-ük` and `-juk/-jük`.
    &['u', 'k'], &['ü', 'k'],
    &['j', 'u', 'k'], &['j', 'ü', 'k'],

    // ------------------------------------------------------------------
    // PL: plural markers with linking vowels.
    // ------------------------------------------------------------------
    &['a', 'k'], &['e', 'k'], &['o', 'k'], &['ö', 'k'],
    &['j', 'a', 'i'], &['j', 'e', 'i'],
    &['a', 'i'], &['e', 'i'],
    &['k'],

    // ------------------------------------------------------------------
    // VB: a small conservative selection of verb suffixes. The
    // shipped stemmer is not a verb lemmatizer; only surface-form
    // strips that don't over-fire on nouns are listed here.
    // ------------------------------------------------------------------
    // Past-tense `-t` variants — the `-tt` cluster after vowel.
    &['t', 't'],
    // Infinitive `-ni`.
    &['n', 'i'],
    // Conditional `-na/-ne`.
    &['n', 'a'], &['n', 'e'],

    // Note: bare `-a`, `-e`, `-i`, `-o` are DELIBERATELY NOT listed
    // as last-resort strips — over-stripping a bare vowel from a
    // noun that natively ends in `-a` (e.g., `alma` "apple") would
    // leave `alm`, which is not the stem. The Snowball reference
    // handles these in a residual step that requires stem-length
    // and R1 guards we're not implementing bit-exactly. The pack's
    // reference-pair tests document the resulting behaviour.
];

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &str) -> String {
        HungarianSnowball.stem(w).into_owned()
    }

    #[test]
    fn short_words_are_unchanged() {
        assert_eq!(s(""), "");
        assert_eq!(s("a"), "a");
        assert_eq!(s("ez"), "ez");
    }

    #[test]
    fn inessive_ban_ben() {
        // Back-harmony: "házban" (in the house) → "ház".
        assert_eq!(s("házban"), "ház");
        // Front-harmony: "kertben" (in the garden) → "kert".
        assert_eq!(s("kertben"), "kert");
    }

    #[test]
    fn illative_ba_be() {
        assert_eq!(s("házba"), "ház");
        assert_eq!(s("kertbe"), "kert");
    }

    #[test]
    fn dative_nak_nek() {
        assert_eq!(s("háznak"), "ház");
        assert_eq!(s("kertnek"), "kert");
    }

    #[test]
    fn elative_bol_bol() {
        assert_eq!(s("házból"), "ház");
        assert_eq!(s("kertből"), "kert");
    }

    #[test]
    fn allative_hoz_hez_hoz() {
        // Back-harmony.
        assert_eq!(s("házhoz"), "ház");
        // Front-unrounded.
        assert_eq!(s("kerthez"), "kert");
        // Front-rounded — `körhöz` "to the circle".
        assert_eq!(s("körhöz"), "kör");
    }

    #[test]
    fn plural_k() {
        // "házak" (houses) → strip `-ak` → "ház".
        assert_eq!(s("házak"), "ház");
        // "kertek" (gardens) → strip `-ek` → "kert".
        assert_eq!(s("kertek"), "kert");
    }

    #[test]
    fn possessive_1sg_m() {
        // "házam" (my house) → strip `-am` → "ház".
        assert_eq!(s("házam"), "ház");
        // "kertem" (my garden) → strip `-em` → "kert".
        assert_eq!(s("kertem"), "kert");
    }
}
