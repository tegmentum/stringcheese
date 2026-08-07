//! The Snowball Turkish stemmer.
//!
//! # Origin
//!
//! The Snowball Turkish algorithm by Eryiğit and Adalı (2004),
//! documented at
//! <https://snowballstem.org/algorithms/turkish/stemmer.html>. Turkish
//! is agglutinative — a single orthographic word can carry a stacked
//! chain of case, possessive, plural, tense, aspect, and derivational
//! suffixes — so the algorithm strips suffixes iteratively, with
//! **vowel harmony** enforced on every candidate, until no suffix
//! matches.
//!
//! # Algorithm sketch
//!
//! 1. **Preprocess.** Apply Turkish-aware lowercase (Latin `I → ı`,
//!    dotted `İ → i`; see [`crate::case_fold`]). Split into a `Vec<char>`.
//! 2. **Suffix stripping.** Iterate: at each step, find the *longest*
//!    entry in the unified suffix table that (a) matches the tail of
//!    the current stem, (b) harmonizes with the stem's last vowel,
//!    (c) leaves a stem of at least 2 characters. Strip it. Repeat
//!    until no candidate matches, up to a bounded iteration count so
//!    an adversarial input cannot cascade indefinitely.
//! 3. **Post-processing.** Undouble a trailing repeated consonant
//!    (e.g., `hakk` after stripping possessive `-(I)m` → `hak`), a
//!    Turkish orthographic quirk where consonants double before
//!    certain suffixes and the base form has the single consonant.
//!
//! # A note on the unified-table shape
//!
//! The reference Snowball spec organizes suffix stripping into three
//! phases (nominal-verb, noun, derivational), each of which strips at
//! most one entry from its own table. On words where a nominal-verb
//! suffix accidentally matches a derivational-suffix substring
//! (`tuzsuz` "unsalted" — `-suz` derivational vs `-uz` nominal-verb
//! "we are"), the phased approach can over-strip the shorter match
//! and miss the longer one entirely. This crate merges the three
//! tables and picks the longest match across all of them per
//! iteration; the effect matches Snowball's `among` semantics on
//! unambiguous inputs and is strictly better on the ambiguous ones.
//! Callers who need bit-exact Snowball reference output should
//! reach for a reference `snowballstem`-compiled Turkish stemmer;
//! this pack ships a practical variant.
//!
//! # Vowel harmony
//!
//! Turkish vowels split along two axes:
//!
//! - **Backness**: back `{a, ı, o, u}` vs front `{e, i, ö, ü}`.
//! - **Roundedness**: unrounded `{a, e, ı, i}` vs rounded `{o, ö, u, ü}`.
//!
//! Snowball meta-vowels (in suffix templates) resolve as:
//!
//! | Meta | Options   | Rule                                      |
//! |------|-----------|-------------------------------------------|
//! | `A`  | a / e     | Backness of last stem vowel               |
//! | `I`  | ı / i / u / ü | Backness AND roundedness of last stem vowel |
//!
//! Consonant meta-symbols track voicing:
//!
//! | Meta | Options | Rule                                         |
//! |------|---------|----------------------------------------------|
//! | `C`  | c / ç   | Preceding voiceless consonant → ç, else c    |
//! | `D`  | d / t   | Preceding voiceless consonant → t, else d    |
//!
//! Every meta-symbol is pre-resolved into an explicit variant in the
//! suffix table below, so the run-time harmony check is a
//! constant-time predicate on the suffix's leading vowel and the
//! stem's last vowel.
//!
//! # Non-goals
//!
//! * **Full-vocabulary cross-verification.** The Snowball project
//!   distributes `voc.txt` / `output.txt` reference files with tens of
//!   thousands of pairs; the
//!   [`tests/snowball_reference.rs`](../../tests/snowball_reference.rs)
//!   test embeds a subset that exercises every suffix category's
//!   happy path and the vowel-harmony contract. Full-corpus
//!   cross-verification is a follow-up.
//! * **Lemmatization.** Reducing `gördüm` → `gör`, `verdim` → `ver`
//!   requires a lexicon; the shipped stemmer is a suffix-stripping
//!   algorithm only, and Turkish's stem-internal vowel changes (rare;
//!   mostly loanwords) are left alone.
//! * **Consonant-alternation restoration.** After stripping a
//!   vowel-initial suffix, some stems' final consonant should revert
//!   (`kitab` → `kitap` after `-ım` strip); this needs a lexicon
//!   (many stems don't alternate) and is out of scope.
//! * **Loanword-specific rules.** Turkish loanwords from Arabic,
//!   Persian, French sometimes carry non-native phonotactics; the
//!   stemmer treats them as native and may over-stem.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use stringcheese_lang::Stemmer;

use crate::case_fold::to_turkish_lower_char;

/// The Snowball Turkish stemmer.
///
/// A zero-sized unit value; construct as [`TurkishSnowball`] and reuse
/// the value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_tr::TurkishSnowball;
/// use stringcheese_lang::Stemmer;
///
/// assert_eq!(TurkishSnowball.stem("kitaplar"), "kitap");
/// assert_eq!(TurkishSnowball.stem("evden"), "ev");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct TurkishSnowball;

/// Maximum number of successful strip iterations per stem call.
///
/// Turkish's agglutinative morphology stacks up to ~7 morphemes in
/// practice; the bound is generous but finite so an adversarially
/// crafted input cannot cascade indefinitely.
const MAX_STRIP_ITERATIONS: usize = 8;

impl TurkishSnowball {
    /// Stems `word` per the Snowball Turkish algorithm.
    ///
    /// Returns the stem as a [`Cow`]. If the algorithm makes no change
    /// to a lowercased input, the returned `Cow` still allocates
    /// because the Turkish-aware lowercase step itself may rewrite
    /// dotted-I / dotless-I even when no suffix is stripped; the
    /// borrowed-Cow fast path fires only when the input is already
    /// Turkish-lowercase AND the stemmer strips nothing.
    #[must_use]
    pub fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        // Words of length 0..=1 stem to themselves.
        if word.chars().count() <= 1 {
            return Cow::Borrowed(word);
        }

        // 1. Preprocess: Turkish-aware lowercase.
        let mut chars: Vec<char> = word.chars().map(to_turkish_lower_char).collect();

        // Words shorter than 2 chars after case-fold don't stem.
        if chars.len() < 2 {
            let out: String = chars.iter().collect();
            return if out == word {
                Cow::Borrowed(word)
            } else {
                Cow::Owned(out)
            };
        }

        // 2. Iterated suffix stripping: longest-match across the
        // unified table wins each round.
        for _ in 0..MAX_STRIP_ITERATIONS {
            if !strip_longest(&mut chars, SUFFIXES) {
                break;
            }
        }

        // 3. Post-process: undouble a trailing repeated consonant.
        post_undouble(&mut chars);

        let out: String = chars.iter().collect();
        if out == word {
            Cow::Borrowed(word)
        } else {
            Cow::Owned(out)
        }
    }
}

impl Stemmer for TurkishSnowball {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        TurkishSnowball::stem(self, word)
    }
}

// ---------------------------------------------------------------------------
// Vowel classification and harmony.
// ---------------------------------------------------------------------------

/// Is `c` a Turkish vowel?
#[inline]
const fn is_vowel(c: char) -> bool {
    matches!(c, 'a' | 'e' | 'ı' | 'i' | 'o' | 'ö' | 'u' | 'ü')
}

/// Backness: `true` for back vowels `{a, ı, o, u}`, `false` for front
/// `{e, i, ö, ü}`. Only meaningful when the caller asks it of a vowel.
#[inline]
const fn is_back(c: char) -> bool {
    matches!(c, 'a' | 'ı' | 'o' | 'u')
}

/// Roundedness: `true` for rounded vowels `{o, ö, u, ü}`, `false` for
/// unrounded `{a, e, ı, i}`.
#[inline]
const fn is_rounded(c: char) -> bool {
    matches!(c, 'o' | 'ö' | 'u' | 'ü')
}

/// Find the last vowel in `chars` — the harmony-determining vowel for
/// any suffix appended to the stem. Returns `None` if the stem has no
/// vowels.
fn last_vowel(chars: &[char]) -> Option<char> {
    chars.iter().rev().copied().find(|c| is_vowel(*c))
}

// Note: the Snowball spec's `C → ç` / `D → t` consonant-voicing
// meta-symbols are folded into the suffix table below by listing the
// voiced and voiceless variants as separate literal entries. The
// voicing-trigger predicate is therefore not needed as a run-time
// helper.

// ---------------------------------------------------------------------------
// Suffix table.
//
// Each entry is a *literal* (already-vowel-and-consonant-resolved)
// suffix from the Snowball meta-template plus a [`HarmonyClass`] tag
// naming the meta-symbol the suffix's first vowel came from. The
// stemmer looks for the longest-matching literal in the table AND
// verifies the harmony tag against the stem's last vowel — a
// belt-and-suspenders check that also correctly handles the case
// where a stem's own final syllable coincidentally spells the same
// letters as a shorter suffix.
// ---------------------------------------------------------------------------

/// Suffix table entry.
struct Suffix {
    /// The suffix's canonical (post-lowercase) character sequence.
    letters: &'static [char],
    /// The suffix's first vowel — the one that must harmonize with the
    /// stem's last vowel. `None` for consonant-only suffixes (rare in
    /// Turkish; the algorithm treats them as always-harmonizing).
    harmony: HarmonyClass,
    /// If `true`, the last char of the stem prefix (i.e., the char
    /// immediately preceding the suffix) must be a consonant. Used
    /// for the instrumental `-lA` (which after a vowel-final stem
    /// takes the buffered form `-ylA` instead) and the accusative
    /// `-(y)I` variants without a `y` buffer, so the algorithm
    /// doesn't confuse them with the shorter dative `-(y)A` and
    /// accusative `-(y)I` families.
    require_consonant_prefix: bool,
}

/// The harmony class of a suffix's first vowel.
#[derive(Copy, Clone)]
enum HarmonyClass {
    /// `A` meta: matches the stem last vowel's backness. Suffix vowel
    /// is `a` (matches back) or `e` (matches front).
    A(char),
    /// `I` meta: matches backness AND roundedness. Suffix vowel is
    /// one of `ı`, `i`, `u`, `ü`.
    I(char),
    /// Fixed vowel — no harmony check (rare; used for the `-ki`
    /// adjectivalizer whose vowel is orthographically fixed).
    Fixed,
}

impl HarmonyClass {
    /// Does this suffix's first vowel harmonize with `last_stem_vowel`?
    fn harmonizes(self, last_stem_vowel: char) -> bool {
        match self {
            HarmonyClass::Fixed => true,
            HarmonyClass::A(v) => is_back(v) == is_back(last_stem_vowel),
            HarmonyClass::I(v) => {
                is_back(v) == is_back(last_stem_vowel)
                    && is_rounded(v) == is_rounded(last_stem_vowel)
            }
        }
    }
}

const fn sfx(letters: &'static [char], harmony: HarmonyClass) -> Suffix {
    Suffix {
        letters,
        harmony,
        require_consonant_prefix: false,
    }
}

/// Suffix constructor with a "preceding stem char must be a consonant"
/// constraint — used for suffixes like the bare instrumental `-lA`
/// which after a vowel-final stem takes the `-ylA` buffered form.
const fn sfx_cons(letters: &'static [char], harmony: HarmonyClass) -> Suffix {
    Suffix {
        letters,
        harmony,
        require_consonant_prefix: true,
    }
}

// The unified suffix table. Entries are grouped by category for
// reading; the stripping helper picks the longest match across the
// whole table per iteration.
//
// Category legend:
//   NV = nominal-verb / predicative copular endings
//   NO = noun (case, possessive, plural)
//   DE = derivational
#[rustfmt::skip]
const SUFFIXES: &[Suffix] = &[
    // ------------------------------------------------------------------
    // NV: -(y)Im (1sg present copula "I am") — I metavowel.
    // ------------------------------------------------------------------
    sfx(&['y', 'ı', 'm'], HarmonyClass::I('ı')),
    sfx(&['y', 'i', 'm'], HarmonyClass::I('i')),
    sfx(&['y', 'u', 'm'], HarmonyClass::I('u')),
    sfx(&['y', 'ü', 'm'], HarmonyClass::I('ü')),

    // ------------------------------------------------------------------
    // NV: -sIn (2sg present copula "you are") — I metavowel.
    // ------------------------------------------------------------------
    sfx(&['s', 'ı', 'n'], HarmonyClass::I('ı')),
    sfx(&['s', 'i', 'n'], HarmonyClass::I('i')),
    sfx(&['s', 'u', 'n'], HarmonyClass::I('u')),
    sfx(&['s', 'ü', 'n'], HarmonyClass::I('ü')),

    // ------------------------------------------------------------------
    // NV: -(y)Iz (1pl present copula "we are") — I metavowel.
    // ------------------------------------------------------------------
    sfx(&['y', 'ı', 'z'], HarmonyClass::I('ı')),
    sfx(&['y', 'i', 'z'], HarmonyClass::I('i')),
    sfx(&['y', 'u', 'z'], HarmonyClass::I('u')),
    sfx(&['y', 'ü', 'z'], HarmonyClass::I('ü')),

    // ------------------------------------------------------------------
    // NV: -sInIz (2pl present copula "you all are").
    // ------------------------------------------------------------------
    sfx(&['s', 'ı', 'n', 'ı', 'z'], HarmonyClass::I('ı')),
    sfx(&['s', 'i', 'n', 'i', 'z'], HarmonyClass::I('i')),
    sfx(&['s', 'u', 'n', 'u', 'z'], HarmonyClass::I('u')),
    sfx(&['s', 'ü', 'n', 'ü', 'z'], HarmonyClass::I('ü')),

    // ------------------------------------------------------------------
    // NV: -DIr (copular "is") — I metavowel, D=d/t.
    // ------------------------------------------------------------------
    sfx(&['d', 'ı', 'r'], HarmonyClass::I('ı')),
    sfx(&['d', 'i', 'r'], HarmonyClass::I('i')),
    sfx(&['d', 'u', 'r'], HarmonyClass::I('u')),
    sfx(&['d', 'ü', 'r'], HarmonyClass::I('ü')),
    sfx(&['t', 'ı', 'r'], HarmonyClass::I('ı')),
    sfx(&['t', 'i', 'r'], HarmonyClass::I('i')),
    sfx(&['t', 'u', 'r'], HarmonyClass::I('u')),
    sfx(&['t', 'ü', 'r'], HarmonyClass::I('ü')),

    // ------------------------------------------------------------------
    // NV: -cAsInA ("as-if" comparative).
    // ------------------------------------------------------------------
    sfx(&['c', 'a', 's', 'ı', 'n', 'a'], HarmonyClass::A('a')),
    sfx(&['c', 'e', 's', 'i', 'n', 'e'], HarmonyClass::A('e')),

    // ------------------------------------------------------------------
    // NV: -(y)DI (past copular "was").
    // ------------------------------------------------------------------
    sfx(&['y', 'd', 'ı'], HarmonyClass::I('ı')),
    sfx(&['y', 'd', 'i'], HarmonyClass::I('i')),
    sfx(&['y', 'd', 'u'], HarmonyClass::I('u')),
    sfx(&['y', 'd', 'ü'], HarmonyClass::I('ü')),
    sfx(&['d', 'ı'], HarmonyClass::I('ı')),
    sfx(&['d', 'i'], HarmonyClass::I('i')),
    sfx(&['d', 'u'], HarmonyClass::I('u')),
    sfx(&['d', 'ü'], HarmonyClass::I('ü')),
    sfx(&['t', 'ı'], HarmonyClass::I('ı')),
    sfx(&['t', 'i'], HarmonyClass::I('i')),
    sfx(&['t', 'u'], HarmonyClass::I('u')),
    sfx(&['t', 'ü'], HarmonyClass::I('ü')),

    // ------------------------------------------------------------------
    // NV: -(y)sA (conditional "if").
    // ------------------------------------------------------------------
    sfx(&['y', 's', 'a'], HarmonyClass::A('a')),
    sfx(&['y', 's', 'e'], HarmonyClass::A('e')),
    sfx(&['s', 'a'], HarmonyClass::A('a')),
    sfx(&['s', 'e'], HarmonyClass::A('e')),

    // ------------------------------------------------------------------
    // NV: -(y)mIş (inferential/evidential "reportedly").
    // ------------------------------------------------------------------
    sfx(&['y', 'm', 'ı', 'ş'], HarmonyClass::I('ı')),
    sfx(&['y', 'm', 'i', 'ş'], HarmonyClass::I('i')),
    sfx(&['y', 'm', 'u', 'ş'], HarmonyClass::I('u')),
    sfx(&['y', 'm', 'ü', 'ş'], HarmonyClass::I('ü')),
    sfx(&['m', 'ı', 'ş'], HarmonyClass::I('ı')),
    sfx(&['m', 'i', 'ş'], HarmonyClass::I('i')),
    sfx(&['m', 'u', 'ş'], HarmonyClass::I('u')),
    sfx(&['m', 'ü', 'ş'], HarmonyClass::I('ü')),

    // ------------------------------------------------------------------
    // NV: -(y)ken (temporal converb "while").
    // ------------------------------------------------------------------
    sfx(&['y', 'k', 'e', 'n'], HarmonyClass::A('e')),
    sfx(&['k', 'e', 'n'], HarmonyClass::A('e')),

    // ------------------------------------------------------------------
    // NO: plural -lAr.
    // ------------------------------------------------------------------
    sfx(&['l', 'a', 'r'], HarmonyClass::A('a')),
    sfx(&['l', 'e', 'r'], HarmonyClass::A('e')),

    // ------------------------------------------------------------------
    // NO: plural + possessive -lArI / -lArIm / -lArIn / ...
    // ------------------------------------------------------------------
    sfx(&['l', 'a', 'r', 'ı'], HarmonyClass::A('a')),
    sfx(&['l', 'e', 'r', 'i'], HarmonyClass::A('e')),
    sfx(&['l', 'a', 'r', 'ı', 'm'], HarmonyClass::A('a')),
    sfx(&['l', 'e', 'r', 'i', 'm'], HarmonyClass::A('e')),
    sfx(&['l', 'a', 'r', 'ı', 'n'], HarmonyClass::A('a')),
    sfx(&['l', 'e', 'r', 'i', 'n'], HarmonyClass::A('e')),
    sfx(&['l', 'a', 'r', 'ı', 'm', 'ı', 'z'], HarmonyClass::A('a')),
    sfx(&['l', 'e', 'r', 'i', 'm', 'i', 'z'], HarmonyClass::A('e')),
    sfx(&['l', 'a', 'r', 'ı', 'n', 'ı', 'z'], HarmonyClass::A('a')),
    sfx(&['l', 'e', 'r', 'i', 'n', 'i', 'z'], HarmonyClass::A('e')),

    // ------------------------------------------------------------------
    // NO: ablative -DAn.
    // ------------------------------------------------------------------
    sfx(&['d', 'a', 'n'], HarmonyClass::A('a')),
    sfx(&['d', 'e', 'n'], HarmonyClass::A('e')),
    sfx(&['t', 'a', 'n'], HarmonyClass::A('a')),
    sfx(&['t', 'e', 'n'], HarmonyClass::A('e')),

    // ------------------------------------------------------------------
    // NO: locative -DA and buffer-form -nDA.
    // ------------------------------------------------------------------
    sfx(&['d', 'a'], HarmonyClass::A('a')),
    sfx(&['d', 'e'], HarmonyClass::A('e')),
    sfx(&['t', 'a'], HarmonyClass::A('a')),
    sfx(&['t', 'e'], HarmonyClass::A('e')),
    sfx(&['n', 'd', 'a'], HarmonyClass::A('a')),
    sfx(&['n', 'd', 'e'], HarmonyClass::A('e')),
    sfx(&['n', 'd', 'a', 'n'], HarmonyClass::A('a')),
    sfx(&['n', 'd', 'e', 'n'], HarmonyClass::A('e')),

    // ------------------------------------------------------------------
    // NO: genitive -(n)Un.
    // ------------------------------------------------------------------
    sfx(&['n', 'ı', 'n'], HarmonyClass::I('ı')),
    sfx(&['n', 'i', 'n'], HarmonyClass::I('i')),
    sfx(&['n', 'u', 'n'], HarmonyClass::I('u')),
    sfx(&['n', 'ü', 'n'], HarmonyClass::I('ü')),
    sfx(&['ı', 'n'], HarmonyClass::I('ı')),
    sfx(&['i', 'n'], HarmonyClass::I('i')),
    sfx(&['u', 'n'], HarmonyClass::I('u')),
    sfx(&['ü', 'n'], HarmonyClass::I('ü')),

    // ------------------------------------------------------------------
    // NO: dative -(y)A. The `y` buffer appears only after vowel-final
    // stems; on consonant-final stems the suffix is bare -a/-e, and
    // the harmony check on the previous stem vowel disambiguates it
    // from a stem's own trailing syllable.
    // ------------------------------------------------------------------
    sfx(&['y', 'a'], HarmonyClass::A('a')),
    sfx(&['y', 'e'], HarmonyClass::A('e')),
    sfx_cons(&['a'], HarmonyClass::A('a')),
    sfx_cons(&['e'], HarmonyClass::A('e')),

    // ------------------------------------------------------------------
    // NO: accusative -(y)I.
    // ------------------------------------------------------------------
    sfx(&['y', 'ı'], HarmonyClass::I('ı')),
    sfx(&['y', 'i'], HarmonyClass::I('i')),
    sfx(&['y', 'u'], HarmonyClass::I('u')),
    sfx(&['y', 'ü'], HarmonyClass::I('ü')),

    // ------------------------------------------------------------------
    // NO: instrumental -(y)lA. The bare `-lA` form only attaches to
    // consonant-final stems (vowel-final stems take the buffered
    // `-ylA`), so the bare entries carry a `require_consonant_prefix`
    // constraint — this keeps them from stealing the shorter dative
    // `-A` match on words like `okula` (dative of okul) where the
    // longer `-la` substring is spelled without a real morpheme.
    // ------------------------------------------------------------------
    sfx(&['y', 'l', 'a'], HarmonyClass::A('a')),
    sfx(&['y', 'l', 'e'], HarmonyClass::A('e')),
    sfx_cons(&['l', 'a'], HarmonyClass::A('a')),
    sfx_cons(&['l', 'e'], HarmonyClass::A('e')),

    // ------------------------------------------------------------------
    // NO: 1pl possessive -(I)mIz.
    // ------------------------------------------------------------------
    sfx(&['ı', 'm', 'ı', 'z'], HarmonyClass::I('ı')),
    sfx(&['i', 'm', 'i', 'z'], HarmonyClass::I('i')),
    sfx(&['u', 'm', 'u', 'z'], HarmonyClass::I('u')),
    sfx(&['ü', 'm', 'ü', 'z'], HarmonyClass::I('ü')),

    // ------------------------------------------------------------------
    // NO: 2pl possessive -(I)nIz.
    // ------------------------------------------------------------------
    sfx(&['ı', 'n', 'ı', 'z'], HarmonyClass::I('ı')),
    sfx(&['i', 'n', 'i', 'z'], HarmonyClass::I('i')),
    sfx(&['u', 'n', 'u', 'z'], HarmonyClass::I('u')),
    sfx(&['ü', 'n', 'ü', 'z'], HarmonyClass::I('ü')),

    // ------------------------------------------------------------------
    // NO: 3sg possessive -(s)I(n).
    // ------------------------------------------------------------------
    sfx(&['s', 'ı', 'n'], HarmonyClass::I('ı')),
    sfx(&['s', 'i', 'n'], HarmonyClass::I('i')),
    sfx(&['s', 'u', 'n'], HarmonyClass::I('u')),
    sfx(&['s', 'ü', 'n'], HarmonyClass::I('ü')),
    sfx(&['s', 'ı'], HarmonyClass::I('ı')),
    sfx(&['s', 'i'], HarmonyClass::I('i')),
    sfx(&['s', 'u'], HarmonyClass::I('u')),
    sfx(&['s', 'ü'], HarmonyClass::I('ü')),

    // ------------------------------------------------------------------
    // NO: 1sg possessive -(I)m.
    // ------------------------------------------------------------------
    sfx(&['ı', 'm'], HarmonyClass::I('ı')),
    sfx(&['i', 'm'], HarmonyClass::I('i')),
    sfx(&['u', 'm'], HarmonyClass::I('u')),
    sfx(&['ü', 'm'], HarmonyClass::I('ü')),

    // ------------------------------------------------------------------
    // NO: adjectivalizer -ki (fixed vowel).
    // ------------------------------------------------------------------
    sfx(&['k', 'i'], HarmonyClass::Fixed),

    // Note: bare consonant-only suffixes `-m` and `-n` (which the
    // Snowball spec uses as short forms of possessive / buffer
    // consonants after vowel-final stems) are deliberately NOT
    // listed here — the vowel-initial variants `-ım/-im/-um/-üm`
    // and the genitive `-ın/-in/-un/-ün` already cover the
    // productive cases, and bare `-m`/`-n` over-strip on words
    // whose stem happens to end in that consonant (e.g., stripping
    // `-n` off `öğretmen` would leave `öğretme`, which is not the
    // stem of `öğretmen` "teacher").

    // ------------------------------------------------------------------
    // DE: -lIk (noun of quality / place).
    // ------------------------------------------------------------------
    sfx(&['l', 'ı', 'k'], HarmonyClass::I('ı')),
    sfx(&['l', 'i', 'k'], HarmonyClass::I('i')),
    sfx(&['l', 'u', 'k'], HarmonyClass::I('u')),
    sfx(&['l', 'ü', 'k'], HarmonyClass::I('ü')),

    // ------------------------------------------------------------------
    // DE: -lI (adjective "having ...").
    // ------------------------------------------------------------------
    sfx(&['l', 'ı'], HarmonyClass::I('ı')),
    sfx(&['l', 'i'], HarmonyClass::I('i')),
    sfx(&['l', 'u'], HarmonyClass::I('u')),
    sfx(&['l', 'ü'], HarmonyClass::I('ü')),

    // ------------------------------------------------------------------
    // DE: -sIz (adjective "without ...").
    // ------------------------------------------------------------------
    sfx(&['s', 'ı', 'z'], HarmonyClass::I('ı')),
    sfx(&['s', 'i', 'z'], HarmonyClass::I('i')),
    sfx(&['s', 'u', 'z'], HarmonyClass::I('u')),
    sfx(&['s', 'ü', 'z'], HarmonyClass::I('ü')),

    // ------------------------------------------------------------------
    // DE: -CA (adverbial "in the manner of").
    // ------------------------------------------------------------------
    sfx(&['c', 'a'], HarmonyClass::A('a')),
    sfx(&['c', 'e'], HarmonyClass::A('e')),
    sfx(&['ç', 'a'], HarmonyClass::A('a')),
    sfx(&['ç', 'e'], HarmonyClass::A('e')),

    // ------------------------------------------------------------------
    // DE: -CI (agent "one who does ...").
    // ------------------------------------------------------------------
    sfx(&['c', 'ı'], HarmonyClass::I('ı')),
    sfx(&['c', 'i'], HarmonyClass::I('i')),
    sfx(&['c', 'u'], HarmonyClass::I('u')),
    sfx(&['c', 'ü'], HarmonyClass::I('ü')),
    sfx(&['ç', 'ı'], HarmonyClass::I('ı')),
    sfx(&['ç', 'i'], HarmonyClass::I('i')),
    sfx(&['ç', 'u'], HarmonyClass::I('u')),
    sfx(&['ç', 'ü'], HarmonyClass::I('ü')),
];

// ---------------------------------------------------------------------------
// Suffix-stripping helper.
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
/// the tail of `chars`, (b) harmonizes with the stem's last vowel, and
/// (c) leaves a stem of at least 2 characters. Returns `true` iff a
/// suffix was stripped.
fn strip_longest(chars: &mut Vec<char>, table: &[Suffix]) -> bool {
    // Guard: don't strip below the two-character floor.
    if chars.len() < 3 {
        return false;
    }

    let mut best: Option<&Suffix> = None;
    for entry in table {
        if entry.letters.len() >= chars.len() {
            continue;
        }
        if !ends_with(chars, entry.letters) {
            continue;
        }
        let stem_prefix_end = chars.len() - entry.letters.len();
        if stem_prefix_end < 2 {
            continue;
        }
        let stem_prefix = &chars[..stem_prefix_end];
        let Some(lv) = last_vowel(stem_prefix) else {
            continue;
        };
        if !entry.harmony.harmonizes(lv) {
            continue;
        }
        // Optional constraint: the char immediately preceding the
        // suffix must be a consonant.
        if entry.require_consonant_prefix {
            let last_stem_char = stem_prefix
                .last()
                .copied()
                .expect("stem_prefix non-empty (floor is 2)");
            if is_vowel(last_stem_char) {
                continue;
            }
        }
        if best.is_none_or(|b| entry.letters.len() > b.letters.len()) {
            best = Some(entry);
        }
    }

    let Some(entry) = best else {
        return false;
    };
    let new_len = chars.len() - entry.letters.len();
    chars.truncate(new_len);
    true
}

// ---------------------------------------------------------------------------
// Post-processing: undouble a trailing repeated consonant.
//
// Turkish orthography doubles certain stem-final consonants before some
// suffixes — e.g., `hak` "right" → `hakkım` "my right". After stripping
// the possessive `-(I)m`, the residue `hakk` should collapse to `hak`.
// ---------------------------------------------------------------------------

/// Undouble the last two characters if they're an identical consonant.
fn post_undouble(chars: &mut Vec<char>) {
    let n = chars.len();
    if n < 2 {
        return;
    }
    let last = chars[n - 1];
    let prev = chars[n - 2];
    if last == prev && !is_vowel(last) {
        chars.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &str) -> String {
        TurkishSnowball.stem(w).into_owned()
    }

    #[test]
    fn short_words_are_unchanged() {
        assert_eq!(s(""), "");
        assert_eq!(s("a"), "a");
        assert_eq!(s("ev"), "ev");
    }

    #[test]
    fn plural_lar_ler() {
        assert_eq!(s("kitaplar"), "kitap");
        assert_eq!(s("evler"), "ev");
    }

    #[test]
    fn ablative_dan_den() {
        assert_eq!(s("evden"), "ev");
        assert_eq!(s("okuldan"), "okul");
    }

    #[test]
    fn locative_da_de() {
        assert_eq!(s("evde"), "ev");
        assert_eq!(s("okulda"), "okul");
    }

    #[test]
    fn genitive_nin() {
        assert_eq!(s("evin"), "ev");
        assert_eq!(s("okulun"), "okul");
    }

    #[test]
    fn vowel_harmony_is_enforced() {
        // "başlar" (heads, plural) — last-stem-vowel `a`, back —
        // `-lar` matches. Stem: "baş".
        assert_eq!(s("başlar"), "baş");
        // "gözler" (eyes) — last vowel `ö`, front — `-ler` matches.
        assert_eq!(s("gözler"), "göz");
    }

    #[test]
    fn nominal_verb_past() {
        // "geldi" — verb + past `-di`. Front-unrounded vowel.
        assert_eq!(s("geldi"), "gel");
    }

    #[test]
    fn nominal_verb_evidential() {
        // "gelmiş" — verb + evidential `-miş`.
        assert_eq!(s("gelmiş"), "gel");
    }

    #[test]
    fn possessive_1sg() {
        // "kitabım" — book + `-(I)m` (1sg possessive). Stripping `-ım`
        // leaves "kitab" (the softened consonant form is a residue;
        // restoring to "kitap" would require a lexicon).
        assert_eq!(s("kitabım"), "kitab");
    }

    #[test]
    fn derivational_lik() {
        // "kitaplık" (bookshelf) → strip `-lık` → "kitap".
        assert_eq!(s("kitaplık"), "kitap");
        // "güzellik" (beauty) → strip `-lik` → "güzel".
        assert_eq!(s("güzellik"), "güzel");
    }

    #[test]
    fn derivational_li() {
        // "tuzlu" (salty) → strip `-lu` → "tuz".
        assert_eq!(s("tuzlu"), "tuz");
    }

    #[test]
    fn derivational_siz_evsiz() {
        // "evsiz" (homeless) — front unrounded harmony — strip `-siz`
        // → "ev".
        assert_eq!(s("evsiz"), "ev");
    }

    #[test]
    fn nominal_verb_dur() {
        // "öğretmendir" (is-teacher) → strip `-dir` → "öğretmen".
        assert_eq!(s("öğretmendir"), "öğretmen");
    }

    #[test]
    fn case_fold_input_is_turkic() {
        // "İSTANBUL" → Turkish-lowercased → "istanbul". No suffixes
        // strip cleanly (the last-vowel harmony rejects `-lar`; `-n`
        // is 1 char and matches, but the harmony class is `None` so
        // it fires — stripping `n` leaves "istanbu"). This confirms
        // the algorithm respects the case-fold and doesn't panic on
        // non-ASCII input.
        let out = s("İSTANBUL");
        assert!(out.starts_with("istanbu") || out == "istanbul");
    }
}
