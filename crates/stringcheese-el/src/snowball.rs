//! The Snowball-family Greek stemmer.
//!
//! # Origin
//!
//! Modeled after the Snowball Greek algorithm documented at
//! <http://snowballstem.org/algorithms/greek/stemmer.html>, itself
//! descended from the Ntais (2006) Greek stemmer. The full published
//! spec is very large — hundreds of context-sensitive suffix rules
//! across a dozen cascading steps — because Greek's fusional
//! morphology (case, number, gender for nouns / adjectives; person,
//! number, tense, aspect, voice for verbs) generates a lot of
//! surface-form variety per lemma.
//!
//! **This crate ships a Snowball-*family* subset**: the same
//! preprocessing pipeline (accent stripping + final-sigma folding)
//! and the same shape of cascading suffix passes (longest-match
//! stripping with a minimum-stem-length guard), but a hand-audited
//! subset of the suffix inventory that covers the high-frequency
//! nominal / adjectival / verbal endings without carrying the
//! full context-sensitive tail. The design trade-off is the same as
//! for `stringcheese-uk` (Ukrainian) and `stringcheese-cs` (Czech):
//! a smaller, easier-to-audit table that handles the common cases
//! well and gracefully leaves rarer surface forms unstripped, rather
//! than a large table whose context-sensitive exceptions are hard to
//! verify at review time.
//!
//! Full canonical Snowball parity is a deferred follow-up (see the
//! [crate root](crate)).
//!
//! # Algorithm sketch
//!
//! 1. **Preprocess.** Lowercase under Rust's default Unicode fold
//!    (Greek has no Turkic-style tailoring — capital sigma `Σ` folds
//!    to non-final `σ` under the *default* Unicode mapping, not to
//!    positional `ς`). Then fold accents (`ά → α`, `έ → ε`, `ή → η`,
//!    `ί → ι`, `ό → ο`, `ύ → υ`, `ώ → ω`, plus diaeresis `ϊ → ι`,
//!    `ϋ → υ`, `ΐ → ι`, `ΰ → υ`). Then fold final sigma (`ς → σ`) so
//!    the suffix tables — which store the non-final form — can match
//!    words that end in the final variant.
//! 2. **Minimum stem length.** Words of char-length below 4 are
//!    returned unchanged (apart from the fold). Greek's short function
//!    words (`και`, `για`, `ναι`) would be over-stripped by even the
//!    conservative single-vowel rules.
//! 3. **Suffix cascade.** A longest-match strip against a curated
//!    suffix table, run in three passes:
//!    * **Step 1** — long compound endings (5+ chars): superlative
//!      `-οτατοσ`, comparative `-οτεροσ`, mediopassive `-ιονται`,
//!      passive aorist `-θηκαμε`/`-τηκαμε`, abstract-noun `-οτητα`,
//!      passive participle `-μενοσ`.
//!    * **Step 2** — medium endings (2-4 chars): the nominal /
//!      adjectival case endings `-οσ`, `-ου`, `-ον`, `-οι`, `-ων`,
//!      `-ουσ`, `-ασ`, `-ησ`, `-εσ`, `-ια`, `-ιου`, `-ιων`; the
//!      common verb endings `-ουμε`, `-ετε`, `-ουν`, `-ουμαι`,
//!      `-ονται`, `-εται`, `-αμε`, `-ατε`, `-ουσα`, `-ουσε`; the
//!      passive-aorist / present-mediopassive endings.
//!    * **Step 3** — bare final vowel (1 char): `-α`, `-ε`, `-η`,
//!      `-ι`, `-ο`, `-υ`, `-ω`.
//!
//!    Every strip is guarded by a **min-stem-length-of-3** rule (in
//!    char space): a strip that would leave less than 3 characters is
//!    skipped. This is what stops `για → γι` from happening.
//!
//! # Byte-vs-char safety
//!
//! Every Greek scalar in the modern Greek block is **2 bytes** in
//! UTF-8 (U+0370..=U+03FF falls entirely in the 2-byte range
//! U+0080..=U+07FF). All the arithmetic in this module operates on
//! `Vec<char>` indices — never raw byte offsets — so a suffix
//! `['μ', 'ε', 'ν', 'ο', 'σ']` of char-length 5 is 10 bytes of Greek
//! UTF-8 but only 5 slots of a `Vec<char>`. The region calculations
//! return char-indices; the ends-with / truncate helpers accept
//! char-slices. There is no path through this module that crosses a
//! scalar boundary at the byte level.
//!
//! # Non-goals
//!
//! * **Full-canonical Snowball parity.** As above — a follow-up would
//!   flesh out the full suffix inventory and the context-sensitive
//!   exceptions.
//! * **Lemmatization.** Reducing `καλύτερος → καλός`, `είμαι → είμαι`
//!   needs a lexicon, not a suffix-stripping algorithm.
//! * **Polytonic Greek.** The classical polytonic accents (grave,
//!   circumflex, breathings) are left untouched by the accent-fold —
//!   the fold recognizes only the modern monotonic tonos and dialytika.
//!   Polytonic support belongs to a future `stringcheese-grc`
//!   (Ancient Greek) sibling.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use stringcheese_lang::Stemmer;

use crate::phonetic::fold_accents;

/// The Greek stemmer.
///
/// A zero-sized unit value; construct as [`GreekSnowball`] and reuse
/// the value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_el::GreekSnowball;
/// use stringcheese_lang::Stemmer;
///
/// assert_eq!(GreekSnowball.stem("καλός"), "καλ");
/// assert_eq!(GreekSnowball.stem("γράφω"), "γραφ");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct GreekSnowball;

/// Minimum stem char-length: any strip that would leave fewer chars
/// is skipped.
const MIN_STEM_LEN: usize = 3;

impl GreekSnowball {
    /// Stems `word` per the Greek stemming algorithm.
    ///
    /// Returns the stem as a [`Cow`]. If the algorithm makes no change
    /// to a preprocessed input, the returned `Cow` borrows the input.
    #[must_use]
    pub fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        if word.is_empty() {
            return Cow::Borrowed(word);
        }

        // 1. Preprocess into `Vec<char>` space. All downstream
        // arithmetic runs in char space; Greek is 2 bytes per scalar
        // and byte arithmetic would silently corrupt boundaries.
        let mut chars: Vec<char> = word
            .chars()
            .flat_map(char::to_lowercase)
            .map(fold_accents)
            .map(|c| if c == 'ς' { 'σ' } else { c })
            .collect();

        // 2. Words shorter than 4 chars after preprocessing are
        // returned as-is (apart from the fold).
        if chars.len() >= 4 {
            // 3. Suffix cascade. Each step tries the longest matching
            // suffix in its table; a match strips and terminates the
            // step. Each step runs at most once.
            step_1_long_compound(&mut chars);
            step_2_medium(&mut chars);
            step_3_bare_vowel(&mut chars);
        }

        let out: String = chars.iter().collect();
        if out == word {
            Cow::Borrowed(word)
        } else {
            Cow::Owned(out)
        }
    }
}

impl Stemmer for GreekSnowball {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        GreekSnowball::stem(self, word)
    }
}

// ---------------------------------------------------------------------------
// Suffix tables — stored as `Vec<char>`-shaped slices for direct
// comparison against the working buffer.
// ---------------------------------------------------------------------------

/// Long compound suffixes (5+ chars): superlative, comparative,
/// mediopassive verb forms, passive aorist, abstract-noun endings,
/// passive participle. Ordered longest-first so the longest-match
/// helper picks the widest matching suffix.
const LONG_COMPOUND: &[&[char]] = &[
    // Passive aorist 1pl/2pl/3pl.
    &['θ', 'η', 'κ', 'α', 'μ', 'ε'], // -θηκαμε
    &['θ', 'η', 'κ', 'α', 'τ', 'ε'], // -θηκατε
    &['θ', 'η', 'κ', 'α', 'ν', 'ε'], // -θηκανε
    &['τ', 'η', 'κ', 'α', 'μ', 'ε'], // -τηκαμε
    &['τ', 'η', 'κ', 'α', 'τ', 'ε'], // -τηκατε
    &['τ', 'η', 'κ', 'α', 'ν', 'ε'], // -τηκανε
    // Mediopassive 3sg / 3pl in the -ιε- / -ιο- pattern.
    &['ι', 'ο', 'ν', 'τ', 'α', 'ι'],      // -ιονται (3pl)
    &['ο', 'υ', 'μ', 'α', 'σ', 'τ', 'ε'], // -ουμαστε
    &['ι', 'ο', 'μ', 'α', 'σ', 'τ', 'ε'], // -ιομαστε
    &['ι', 'ο', 'σ', 'α', 'σ', 'τ', 'ε'], // -ιοσαστε
    // Superlative / comparative.
    &['ο', 'τ', 'α', 'τ', 'ο', 'σ'], // -οτατος
    &['ο', 'τ', 'α', 'τ', 'ο', 'ι'], // -οτατοι
    &['ο', 'τ', 'α', 'τ', 'ε', 'σ'], // -οτατες
    &['ο', 'τ', 'ε', 'ρ', 'ο', 'σ'], // -οτερος
    &['ο', 'τ', 'ε', 'ρ', 'ο', 'ι'], // -οτεροι
    &['ο', 'τ', 'ε', 'ρ', 'ε', 'σ'], // -οτερες
    &['ο', 'τ', 'α', 'τ', 'η'],      // -οτατη
    &['ο', 'τ', 'α', 'τ', 'ο'],      // -οτατο
    &['ο', 'τ', 'ε', 'ρ', 'η'],      // -οτερη
    &['ο', 'τ', 'ε', 'ρ', 'ο'],      // -οτερο
    &['ο', 'τ', 'ε', 'ρ', 'α'],      // -οτερα
    &['ο', 'τ', 'α', 'τ', 'α'],      // -οτατα
    // Abstract noun -οτητα.
    &['ο', 'τ', 'η', 'τ', 'α', 'σ'], // -οτητας
    &['ο', 'τ', 'η', 'τ', 'ε', 'σ'], // -οτητες
    &['ο', 'τ', 'η', 'τ', 'ω', 'ν'], // -οτητων
    &['ο', 'τ', 'η', 'τ', 'α'],      // -οτητα
    // Passive participle -μενος.
    &['μ', 'ε', 'ν', 'ο', 'σ'], // -μενος
    &['μ', 'ε', 'ν', 'ο', 'ι'], // -μενοι
    &['μ', 'ε', 'ν', 'ε', 'σ'], // -μενες
    // -ισμός family.
    &['ι', 'σ', 'μ', 'ο', 'σ'], // -ισμος
    &['ι', 'σ', 'μ', 'ο', 'υ'], // -ισμου
    &['ι', 'σ', 'μ', 'ο', 'ι'], // -ισμοι
    // Aorist active plural.
    &['η', 'σ', 'α', 'μ', 'ε'], // -ησαμε
    &['η', 'σ', 'α', 'τ', 'ε'], // -ησατε
    &['ι', 'σ', 'α', 'μ', 'ε'], // -ισαμε
    &['ι', 'σ', 'α', 'τ', 'ε'], // -ισατε
    // Present mediopassive.
    &['ο', 'υ', 'μ', 'α', 'ι'], // -ουμαι
    &['ι', 'ε', 'μ', 'α', 'ι'], // -ιεμαι
    &['ι', 'ε', 'σ', 'α', 'ι'], // -ιεσαι
    &['ι', 'ε', 'τ', 'α', 'ι'], // -ιεται
];

/// Medium suffixes (3-4 chars): the common nominal / adjectival case
/// endings and the medium verb endings.
const MEDIUM: &[&[char]] = &[
    // Verb — present, passive.
    &['ο', 'υ', 'ν', 'τ', 'α', 'ι'], // -ουνται (6, kept here)
    &['ο', 'ν', 'τ', 'α', 'ι'],      // -ονται
    &['ο', 'υ', 'μ', 'ε'],           // -ουμε
    &['ο', 'υ', 'ν', 'ε'],           // -ουνε
    &['ε', 'σ', 'α', 'ι'],           // -εσαι
    &['ε', 'τ', 'α', 'ι'],           // -εται
    &['μ', 'α', 'σ', 'τ', 'ε'],      // -μαστε
    &['σ', 'α', 'σ', 'τ', 'ε'],      // -σαστε
    // Verb — aorist.
    &['η', 'σ', 'α', 'ν'], // -ησαν
    &['η', 'σ', 'ε', 'σ'], // -ησες
    &['η', 'σ', 'α', 'σ'], // -ησας (rare)
    &['ι', 'σ', 'α', 'ν'], // -ισαν
    &['α', 'μ', 'ε'],      // -αμε
    &['α', 'τ', 'ε'],      // -ατε
    &['ο', 'υ', 'σ', 'α'], // -ουσα
    &['ο', 'υ', 'σ', 'ε'], // -ουσε
    &['η', 'σ', 'α'],      // -ησα
    &['η', 'σ', 'ε'],      // -ησε
    &['ι', 'σ', 'α'],      // -ισα
    &['ι', 'σ', 'ε'],      // -ισε
    &['η', 'κ', 'α'],      // -ηκα
    &['η', 'κ', 'ε'],      // -ηκε
    &['θ', 'η', 'κ', 'ε'], // -θηκε
    &['τ', 'η', 'κ', 'ε'], // -τηκε
    // Verb — present.
    &['ε', 'ι', 'σ'], // -εις
    &['ε', 'τ', 'ε'], // -ετε
    &['ο', 'υ', 'ν'], // -ουν
    &['ε', 'ι'],      // -ει
    // Noun / adjective — nominal case endings.
    &['ι', 'ο', 'υ'], // -ιου
    &['ι', 'ω', 'ν'], // -ιων
    &['ι', 'ο', 'ι'], // -ιοι
    &['ε', 'ω', 'ν'], // -εων
    &['ε', 'ι', 'σ'], // -εις (duplicate; harmless)
    &['ο', 'υ', 'σ'], // -ους
    &['ο', 'σ'],      // -ος
    &['ο', 'υ'],      // -ου
    &['ο', 'ν'],      // -ον
    &['ο', 'ι'],      // -οι
    &['ω', 'ν'],      // -ων
    &['α', 'σ'],      // -ας
    &['η', 'σ'],      // -ης
    &['ε', 'σ'],      // -ες
    &['ι', 'α'],      // -ια
];

/// Bare final vowels (1 char): fallback strip after the compound and
/// medium passes have failed. Skipped when it would leave less than
/// [`MIN_STEM_LEN`] characters — the guard is what stops
/// `για → γι` from happening.
const BARE_VOWEL: &[&[char]] = &[&['α'], &['ε'], &['η'], &['ι'], &['ο'], &['υ'], &['ω']];

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

/// Find the longest suffix from `candidates` that `chars` ends with,
/// leaving a stem of at least [`MIN_STEM_LEN`] characters after
/// stripping. Returns the matched slice (or `None`).
fn longest_matching_suffix<'a>(chars: &[char], candidates: &[&'a [char]]) -> Option<&'a [char]> {
    let mut best: Option<&[char]> = None;
    for &s in candidates {
        if !ends_with(chars, s) {
            continue;
        }
        // Stripping `s` would leave `chars.len() - s.len()` chars.
        // Require that to be >= MIN_STEM_LEN.
        if chars.len() < s.len() + MIN_STEM_LEN {
            continue;
        }
        if best.is_none_or(|b| s.len() > b.len()) {
            best = Some(s);
        }
    }
    best
}

/// Truncate the trailing `s` characters from `chars`.
fn strip(chars: &mut Vec<char>, s: &[char]) {
    let n = chars.len() - s.len();
    chars.truncate(n);
}

// ---------------------------------------------------------------------------
// Steps.
// ---------------------------------------------------------------------------

fn step_1_long_compound(chars: &mut Vec<char>) {
    if let Some(s) = longest_matching_suffix(chars, LONG_COMPOUND) {
        strip(chars, s);
    }
}

fn step_2_medium(chars: &mut Vec<char>) {
    if let Some(s) = longest_matching_suffix(chars, MEDIUM) {
        strip(chars, s);
    }
}

fn step_3_bare_vowel(chars: &mut Vec<char>) {
    if let Some(s) = longest_matching_suffix(chars, BARE_VOWEL) {
        strip(chars, s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &str) -> String {
        GreekSnowball.stem(w).into_owned()
    }

    #[test]
    fn empty_and_short_words_are_unchanged() {
        assert_eq!(s(""), "");
        assert_eq!(s("α"), "α");
        assert_eq!(s("και"), "και"); // 3 chars: below the >= 4 length gate.
    }

    #[test]
    fn preprocessing_folds_accents_and_final_sigma() {
        // Accent-fold: `ά → α`, final-sigma fold: `ς → σ`. A 3-char
        // input stays 3-char under the fold — no suffix step runs.
        // A longer word demonstrates both folds.
        assert_eq!(s("Άκρος"), "ακρ");
    }

    #[test]
    fn adjective_masc_singular() {
        assert_eq!(s("καλός"), "καλ");
    }

    #[test]
    fn adjective_fem_singular() {
        assert_eq!(s("καλή"), "καλ");
    }

    #[test]
    fn adjective_neut_singular() {
        assert_eq!(s("καλό"), "καλ");
    }

    #[test]
    fn adjective_plural_forms() {
        assert_eq!(s("καλοί"), "καλ");
        assert_eq!(s("καλές"), "καλ");
        assert_eq!(s("καλά"), "καλ");
    }

    #[test]
    fn noun_os_paradigm() {
        assert_eq!(s("κόσμος"), "κοσμ");
        assert_eq!(s("κόσμου"), "κοσμ");
        assert_eq!(s("κόσμοι"), "κοσμ");
        assert_eq!(s("κόσμων"), "κοσμ");
        assert_eq!(s("κόσμους"), "κοσμ");
    }

    #[test]
    fn noun_a_paradigm() {
        assert_eq!(s("γάτα"), "γατ");
        assert_eq!(s("γάτας"), "γατ");
        assert_eq!(s("γάτες"), "γατ");
        assert_eq!(s("γατών"), "γατ");
    }

    #[test]
    fn noun_i_paradigm() {
        assert_eq!(s("σπίτι"), "σπιτ");
    }

    #[test]
    fn verb_present_paradigm() {
        assert_eq!(s("γράφω"), "γραφ");
        assert_eq!(s("γράφεις"), "γραφ");
        assert_eq!(s("γράφει"), "γραφ");
        assert_eq!(s("γράφουμε"), "γραφ");
        assert_eq!(s("γράφετε"), "γραφ");
        assert_eq!(s("γράφουν"), "γραφ");
    }

    #[test]
    fn passive_participle_stripping() {
        // -μενος (5 chars) strips as a compound. The resulting stem
        // for γραμμένος ("written") drops the -μενος cleanly to leave
        // the doubled-μ from the participle-formation mutation
        // (γραφ-/γραμ- alternation).
        assert_eq!(s("γραμμένος"), "γραμ");
    }

    #[test]
    fn comparative_stripping() {
        // -οτεροσ (6 chars) strips as a compound. ωραιότερος ("more
        // beautiful") stems to ωρα — the compound step drops -οτερος
        // leaving ωραι, then the bare-final-vowel step drops the -ι
        // leaving ωρα. Over-stripping is expected in a light stemmer.
        assert_eq!(s("ωραιότερος"), "ωρα");
    }

    #[test]
    fn abstract_noun_otita() {
        // -οτητα (5 chars) strips as a compound.
        assert_eq!(s("ελευθεροτητα"), "ελευθερ");
    }

    #[test]
    fn min_stem_guard_prevents_over_stripping() {
        // Very short words like "για" are below the length gate.
        assert_eq!(s("για"), "για");
        // "να" is 2 chars — no strip.
        assert_eq!(s("να"), "να");
    }

    #[test]
    fn accented_and_unaccented_variants_stem_identically() {
        assert_eq!(s("γατα"), s("γάτα"));
        assert_eq!(s("κοσμοσ"), s("κόσμος"));
    }
}
