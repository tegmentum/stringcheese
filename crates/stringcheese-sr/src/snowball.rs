//! Serbian light-suffix stemmer, Snowball-family.
//!
//! # Origin and scope
//!
//! Martin Porter's Snowball project ships a Serbian algorithm at
//! <https://snowballstem.org/algorithms/serbian/stemmer.html>. The
//! reference implementation is written for Gaj's Latin script and
//! strips inflectional suffixes from noun, adjective, and verb forms.
//! This module implements a compact Serbian light stemmer in the same
//! family — a single suffix table, longest-match-wins, minimum stem
//! length of three characters. It is not a byte-perfect port of the
//! Snowball reference; it captures the same well-behaved cases the
//! reference does and stops there. See the crate module docs for the
//! full-fidelity port deferred item.
//!
//! # Dual-script strategy: normalize to Latin
//!
//! Serbian is written in **both Cyrillic and Latin** and both scripts
//! are equally valid input. Two implementation choices were on the
//! table:
//!
//! * **(a) Normalize to Latin.** Convert Cyrillic inputs to Latin via
//!   [`crate::scripts::to_latin`], run a single Latin suffix table,
//!   then transliterate the stem back if the input was Cyrillic. One
//!   suffix table; the transliteration cost is a linear scan.
//! * **(b) Dual suffix tables.** Ship parallel Cyrillic and Latin
//!   suffix tables and dispatch on the input's script. No
//!   transliteration cost; two tables to keep in sync.
//!
//! **This module implements option (a).** The rationale:
//!
//! - The Serbian Cyrillic <-> Latin transliteration is bijective on the
//!   standard letter set, so no information is lost.
//! - A single suffix table is easier to reason about, easier to test,
//!   and easier to keep in sync with the Snowball reference (which is
//!   itself in Latin).
//! - The transliteration walks the input once in character space; the
//!   cost is small compared to the suffix-lookup loop.
//!
//! # Digraph safety
//!
//! Serbian has three Latin digraphs — `lj`, `nj`, `dž` — that map to
//! single Cyrillic scalars. The suffix table is designed so no suffix
//! begins with `j` or `ž`, and every suffix starting with `l`, `n`, or
//! `d` is checked to avoid splitting the digraph on the stem side (in
//! practice, none of the shipped suffixes cross a digraph boundary,
//! but the safety check is documented in the code below).
//!
//! # Suffix table
//!
//! The table covers the most common Serbian inflectional endings:
//!
//! - Noun / adjective long-forms: `-ovima`, `-ovima`, `-ijima`, `-ijim`,
//!   `-ijem`, `-ijeg`, `-ijih`, `-ijoj`.
//! - Noun plural masculine: `-ovi`, `-ove`, `-ova`, `-ovu`, `-ovo`,
//!   `-ovom`.
//! - Case endings (long-form): `-ima`, `-ama`, `-oga`, `-ome`, `-ome`.
//! - Adjective short-form endings: `-og`, `-om`, `-im`, `-em`, `-ih`,
//!   `-oj`, `-ov`.
//! - Verb infinitive / L-participle / present: `-ati`, `-iti`, `-uti`,
//!   `-eti`, `-ao`, `-io`, `-la`, `-lo`, `-li`, `-le`, `-ala`, `-alo`,
//!   `-ali`, `-ale`, `-ila`, `-ilo`, `-ili`, `-ile`, `-im`, `-iš`,
//!   `-eš`, `-emo`, `-ete`.
//! - Nominal derivational: `-ost`.
//! - Case suffixes (short): `-a`, `-e`, `-i`, `-o`, `-u`.
//!
//! # Non-goals
//!
//! - **Byte-perfect Snowball parity.** A follow-up wave should port
//!   the full `serbian.sbl` and cross-verify against the Snowball
//!   project's `voc.txt` / `output.txt`.
//! - **Consonant alternations.** Serbian's rich morphological
//!   alternations (`k → č`, `g → ž`, `h → š`, `t → ć`, `d → đ` and the
//!   sibilant-palatalization series) are not modelled. `pišem` and
//!   `pisati` stem to different forms.
//! - **Comparative morphology.** `-iji` / `-ija` comparatives are
//!   stripped as suffixes but the base form is not restored.
//! - **Ekavian / ijekavian distinction.** The stemmer sees both
//!   pronunciations as opaque strings; `vek` and `vijek` stem to
//!   themselves and are not conflated.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use stringcheese_lang::Stemmer;

use crate::scripts::{contains_cyrillic, to_cyrillic, to_latin};

/// Minimum stem length in characters. Strips that would leave a
/// shorter stem are refused.
const MIN_STEM: usize = 3;

/// The Serbian light-suffix stemmer.
///
/// A zero-sized unit value; construct as [`SerbianSnowball`] and reuse
/// the value freely across threads and calls.
///
/// # Example
///
/// ```
/// use stringcheese_sr::SerbianSnowball;
/// use stringcheese_lang::Stemmer;
///
/// // Latin input.
/// assert_eq!(SerbianSnowball.stem("lepa"), "lep");
/// assert_eq!(SerbianSnowball.stem("gradovima"), "grad");
///
/// // Cyrillic input round-trips through the internal Latin form.
/// assert_eq!(SerbianSnowball.stem("лепа"), "леп");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SerbianSnowball;

impl SerbianSnowball {
    /// Stems `word` per the Serbian light-suffix algorithm.
    ///
    /// Cyrillic input is transliterated to Latin before the stemming
    /// pass runs, then the stem is transliterated back to Cyrillic if
    /// the input was Cyrillic. Latin input is stemmed directly.
    ///
    /// Returns [`Cow::Borrowed`] when the input is already the stem
    /// (lowercase Latin, no suffix match); otherwise returns
    /// [`Cow::Owned`].
    #[must_use]
    pub fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        if word.is_empty() {
            return Cow::Borrowed(word);
        }

        let was_cyrillic = contains_cyrillic(word);

        // Normalize to Latin. If the input has no Cyrillic scalar we
        // treat it as already-Latin; the pass-through in `to_latin`
        // would be a no-op but skipping it avoids an allocation.
        let latin_in: Cow<'_, str> = if was_cyrillic {
            Cow::Owned(to_latin(word))
        } else {
            Cow::Borrowed(word)
        };

        // Lowercase (Unicode-aware) into a Vec<char> for suffix
        // arithmetic. Character-space arithmetic is essential — Serbian
        // Latin includes multi-byte scalars (`č`, `ć`, `đ`, `š`, `ž`)
        // and Cyrillic (once we've converted back) is 2 bytes per
        // scalar; byte offsets would silently corrupt suffix
        // boundaries.
        let mut chars: Vec<char> = latin_in.chars().flat_map(char::to_lowercase).collect();

        if chars.len() >= MIN_STEM {
            strip_longest_suffix(&mut chars);
        }

        let latin_out: String = chars.iter().collect();

        // Transliterate back to Cyrillic if the input was Cyrillic.
        let out = if was_cyrillic {
            to_cyrillic(&latin_out)
        } else {
            latin_out
        };

        if out == word {
            Cow::Borrowed(word)
        } else {
            Cow::Owned(out)
        }
    }
}

impl Stemmer for SerbianSnowball {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        SerbianSnowball::stem(self, word)
    }
}

// ---------------------------------------------------------------------------
// Suffix table.
// ---------------------------------------------------------------------------

/// The suffix table, expressed as slices of characters. The stripper
/// scans this table and picks the longest suffix that (a) `chars`
/// ends with and (b) leaves at least [`MIN_STEM`] characters behind.
///
/// # Digraph safety
///
/// None of the shipped suffixes begins with `j` or `ž`, so stripping
/// cannot split an `lj` / `nj` / `dž` digraph on the stem side.
/// Suffixes that begin with `l`, `n`, or `d` (like `-la`, `-lo`,
/// `-ni`) are always preceded on the stem side by a vowel in the
/// intended morphology (past tense `pisala`, `pisalo`), so they do
/// not create fresh digraph collisions either.
static SUFFIXES: &[&[char]] = &[
    // 5-character.
    &['o', 'v', 'i', 'm', 'a'], // -ovima
    &['e', 'v', 'i', 'm', 'a'], // -evima
    // 4-character.
    &['i', 'j', 'i', 'm'], // -ijim
    &['i', 'j', 'e', 'm'], // -ijem
    &['i', 'j', 'e', 'g'], // -ijeg
    &['i', 'j', 'i', 'h'], // -ijih
    &['i', 'j', 'o', 'j'], // -ijoj
    &['o', 'v', 'i', 'm'], // -ovim
    &['o', 'v', 'o', 'm'], // -ovom
    &['e', 'v', 'i', 'm'], // -evim
    &['e', 'v', 'o', 'm'], // -evom
    // 3-character.
    &['a', 'm', 'a'], // -ama
    &['i', 'm', 'a'], // -ima
    &['o', 'v', 'i'], // -ovi
    &['o', 'v', 'e'], // -ove
    &['o', 'v', 'a'], // -ova
    &['o', 'v', 'u'], // -ovu
    &['o', 'v', 'o'], // -ovo
    &['e', 'v', 'i'], // -evi
    &['e', 'v', 'e'], // -eve
    &['e', 'v', 'a'], // -eva
    &['o', 'g', 'a'], // -oga
    &['o', 'm', 'e'], // -ome
    &['o', 'm', 'u'], // -omu
    &['e', 'm', 'u'], // -emu
    &['e', 'g', 'a'], // -ega
    &['i', 'j', 'i'], // -iji
    &['i', 'j', 'a'], // -ija
    &['i', 'j', 'e'], // -ije
    &['i', 'j', 'u'], // -iju
    &['a', 'j', 'u'], // -aju
    &['u', 'j', 'u'], // -uju
    &['e', 'j', 'u'], // -eju
    &['a', 'l', 'i'], // -ali
    &['a', 'l', 'o'], // -alo
    &['a', 'l', 'a'], // -ala
    &['a', 'l', 'e'], // -ale
    &['i', 'l', 'i'], // -ili
    &['i', 'l', 'o'], // -ilo
    &['i', 'l', 'a'], // -ila
    &['i', 'l', 'e'], // -ile
    &['a', 't', 'i'], // -ati
    &['i', 't', 'i'], // -iti
    &['u', 't', 'i'], // -uti
    &['e', 't', 'i'], // -eti
    &['e', 'm', 'o'], // -emo
    &['e', 't', 'e'], // -ete
    &['o', 's', 't'], // -ost
    // 2-character.
    &['o', 'g'],
    &['o', 'm'],
    &['i', 'm'],
    &['e', 'm'],
    &['i', 'h'],
    &['o', 'j'],
    &['o', 'v'],
    &['a', 'o'],
    &['i', 'o'],
    &['i', 'š'],
    &['e', 'š'],
    &['t', 'i'],
    &['n', 'a'],
    &['n', 'o'],
    &['n', 'e'],
    &['l', 'a'],
    &['l', 'o'],
    &['l', 'i'],
    &['l', 'e'],
    // 1-character.
    &['a'],
    &['e'],
    &['i'],
    &['o'],
    &['u'],
];

/// Find the longest suffix from [`SUFFIXES`] that `chars` ends with
/// and that leaves at least [`MIN_STEM`] characters behind; strip it
/// in place.
fn strip_longest_suffix(chars: &mut Vec<char>) {
    let n = chars.len();
    let mut best: Option<usize> = None;
    for &s in SUFFIXES {
        let sl = s.len();
        if sl >= n || n - sl < MIN_STEM {
            continue;
        }
        if !ends_with(chars, s) {
            continue;
        }
        if best.is_none_or(|b| sl > b) {
            best = Some(sl);
        }
    }
    if let Some(sl) = best {
        chars.truncate(n - sl);
    }
}

/// Does `chars` end with the character-sequence `suffix`?
fn ends_with(chars: &[char], suffix: &[char]) -> bool {
    if suffix.len() > chars.len() {
        return false;
    }
    let start = chars.len() - suffix.len();
    chars[start..] == *suffix
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &str) -> String {
        SerbianSnowball.stem(w).into_owned()
    }

    #[test]
    fn empty_stems_to_empty() {
        assert_eq!(s(""), "");
    }

    #[test]
    fn short_words_are_unchanged() {
        // A 2-char word cannot be stripped (min stem is 3).
        assert_eq!(s("na"), "na");
    }

    #[test]
    fn adjective_masc_singular() {
        assert_eq!(s("lepa"), "lep");
        assert_eq!(s("lepe"), "lep");
        assert_eq!(s("lepi"), "lep");
        assert_eq!(s("lepim"), "lep");
        assert_eq!(s("lepom"), "lep");
    }

    #[test]
    fn noun_masc_gradovi_stems_to_grad() {
        assert_eq!(s("grad"), "grad");
        assert_eq!(s("grada"), "grad");
        assert_eq!(s("gradu"), "grad");
        assert_eq!(s("gradovi"), "grad");
        assert_eq!(s("gradovima"), "grad");
    }

    #[test]
    fn noun_fem_kuca_stems_to_kuc() {
        assert_eq!(s("kuća"), "kuć");
        assert_eq!(s("kuće"), "kuć");
        assert_eq!(s("kućama"), "kuć");
    }

    #[test]
    fn verb_infinitive_ati() {
        assert_eq!(s("pisati"), "pis");
        assert_eq!(s("pisao"), "pis");
        assert_eq!(s("pisala"), "pis");
    }

    #[test]
    fn cyrillic_input_round_trips_through_latin() {
        // Cyrillic in, Cyrillic out — the pack converts to Latin
        // internally and back.
        assert_eq!(s("лепа"), "леп");
        assert_eq!(s("градови"), "град");
    }

    #[test]
    fn stem_never_grows_the_input() {
        for w in ["lepa", "grad", "kuća", "лепа", "град", "кућа"] {
            assert!(
                s(w).chars().count() <= w.chars().count(),
                "stem({w:?}) grew"
            );
        }
    }
}
