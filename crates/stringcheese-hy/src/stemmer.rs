//! The Eastern Armenian suffix-stripping stemmer.
//!
//! # Origin
//!
//! Armenian has **no widely established Snowball algorithm** in the
//! way English, French, or Russian do; the reference for a
//! machine-friendly Armenian stemmer is the small body of academic
//! literature on Armenian IR (which typically uses a longest-match
//! suffix stripper very similar in shape to the Snowball approach).
//! This module ships a **hand-audited longest-match suffix stripper**
//! that iterates to convergence: on every pass it removes the longest
//! matching suffix from a curated table and re-runs the loop until no
//! more suffixes match. The design trade-off is the same as for
//! `stringcheese-fa` (Persian) and `stringcheese-hi` (Hindi): a
//! smaller, easier-to-audit table that handles the common case-marker,
//! plural-marker, and aorist personal-ending suffixes well and
//! gracefully leaves rarer surface forms unstripped, rather than a
//! large table whose context-sensitive exceptions are hard to verify
//! at review time.
//!
//! # Algorithm sketch
//!
//! 1. **Preprocess.** Lowercase under Rust's default Unicode fold
//!    (Armenian has no locale-specific case-fold quirks — the default
//!    fold does the right thing for every letter in `Ա-Ֆ`). Also
//!    normalize the `եւ → և` two-letter spelling to the ligature.
//! 2. **Minimum stem length.** Any strip that would leave fewer than
//!    two Armenian characters is skipped (a private `MIN_STEM_LEN`
//!    constant carries the value). This is what stops `նա → ()` from
//!    happening.
//! 3. **Suffix cascade, iterated to convergence.** A longest-match
//!    strip against a curated suffix table runs in a loop; on each
//!    pass the longest matching suffix is stripped, and the loop
//!    terminates when no suffix matches. The iterate-to-convergence
//!    shape handles combined markers — for instance, the plural +
//!    genitive `-ների` first strips as `-ների` (5) if listed, or as
//!    `-ի` then `-ներ` in two passes if not.
//!
//! # The suffix table
//!
//! The table covers, ordered longest-first:
//!
//! - **Aorist personal endings.** `-եցինք` (1pl), `-եցիք` (2pl),
//!   `-եցին` (3pl), `-եցիր` (2sg), `-եցի` (1sg), `-եց` (3sg).
//! - **Plural + case combinations.** `-ների`, `-ներով`, `-ներում`,
//!   `-ներից`, `-ներին`, `-ների`, `-երի`, `-երով`, `-երում`, `-երից`,
//!   `-երին`.
//! - **Plural markers.** `-ներ` (polysyllabic base), `-եր`
//!   (monosyllabic base).
//! - **Case suffixes.** Instrumental `-ով`, locative `-ում`, ablative
//!   `-ից`, dative `-ին`, genitive `-ի`.
//! - **The postposed definite article.** `-ը` after consonant, `-ն`
//!   after vowel.
//!
//! # Byte-vs-char safety
//!
//! Every Armenian scalar in the modern Armenian block (U+0530..=U+058F)
//! is encoded as **two UTF-8 bytes** (this range falls entirely
//! inside U+0080..=U+07FF, UTF-8's 2-byte window). All the arithmetic
//! in this module operates on `Vec<char>` indices — never raw byte
//! offsets — so a suffix `['ն', 'ե', 'ր', 'ի']` of char-length 4 is
//! 8 bytes of Armenian UTF-8 but only 4 slots of a `Vec<char>`. There
//! is no path through this module that crosses a scalar boundary at
//! the byte level.
//!
//! # Non-goals
//!
//! - **Lemmatization.** Reducing `գնացի → գնալ`, `եմ → լինել` needs
//!   a lexicon, not a suffix-stripping algorithm.
//! - **Western Armenian.** Western Armenian has a distinct verb
//!   inflection paradigm (present tense uses `-ում` participle in
//!   Eastern; Western uses `կը` + finite form) and a different case
//!   marker set. This module targets **Eastern Armenian**; a future
//!   `stringcheese-hyw` sibling could take Western.
//! - **Classical Armenian (Grabar).** The 5th-century literary
//!   language has 7 cases with distinct singular / plural forms, an
//!   aorist / imperfect / perfect distinction, and participles that
//!   inflect for case. Classical Armenian would deserve its own pack.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use stringcheese_lang::Stemmer;

/// The Armenian (Eastern) stemmer.
///
/// A zero-sized unit value; construct as [`ArmenianStemmer`] and reuse
/// the value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_hy::ArmenianStemmer;
/// use stringcheese_lang::Stemmer;
///
/// // `-ը` (postposed definite article) strips: մայրը → մայր.
/// assert_eq!(ArmenianStemmer.stem("մայրը"), "մայր");
/// // `-ի` (genitive) strips: տան → տան (below min-stem: no change)
/// // but longer nominals strip cleanly.
/// assert_eq!(ArmenianStemmer.stem("գրքի"), "գրք");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ArmenianStemmer;

/// Minimum stem char-length: any strip that would leave fewer chars
/// is skipped. Set to 2 so short base words like `մայր` (mother,
/// 4-char) can lose a 2-char case suffix and still leave a viable
/// stem, but `նա` (he/she, 2-char) never strips.
const MIN_STEM_LEN: usize = 2;

impl ArmenianStemmer {
    /// Stems `word` per the Armenian stemming algorithm.
    ///
    /// Returns the stem as a [`Cow`]. If the algorithm makes no change
    /// to a preprocessed input, the returned `Cow` borrows the input.
    #[must_use]
    pub fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        if word.is_empty() {
            return Cow::Borrowed(word);
        }

        // 1. Preprocess into `Vec<char>` space. All downstream
        // arithmetic runs in char space; Armenian is 2 bytes per
        // scalar and byte arithmetic would silently corrupt
        // boundaries.
        let lowered: String = word.chars().flat_map(char::to_lowercase).collect();
        // Normalize the two-letter `եւ` spelling to the ligature `և`
        // so both spellings stem identically.
        let normalized = lowered.replace("եւ", "և");
        let mut chars: Vec<char> = normalized.chars().collect();

        // 2. Iterated longest-match strip. Runs to convergence: a
        // strip that succeeds re-runs the search; the loop stops when
        // no suffix matches.
        while let Some(s) = longest_matching_suffix(&chars, SUFFIXES) {
            let n = chars.len() - s.len();
            chars.truncate(n);
        }

        let out: String = chars.iter().collect();
        if out == word {
            Cow::Borrowed(word)
        } else {
            Cow::Owned(out)
        }
    }
}

impl Stemmer for ArmenianStemmer {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        ArmenianStemmer::stem(self, word)
    }
}

// ---------------------------------------------------------------------------
// Suffix table — stored as `Vec<char>`-shaped slices for direct
// comparison against the working buffer.
// ---------------------------------------------------------------------------

/// The Armenian suffix table. Ordered by length descending so the
/// longest-match helper picks the widest matching suffix.
///
/// Coverage:
///
/// - Aorist personal endings — `-եցինք` 1pl, `-եցիք` 2pl,
///   `-եցին` 3pl, `-եցիր` 2sg, `-եցի` 1sg, `-եց` 3sg.
/// - Plural + case combinations — `-ներում` (pl.loc), `-ներով`
///   (pl.ins), `-ներից` (pl.abl), `-ներին` (pl.dat), `-ների`
///   (pl.gen), and singular-plural variants for monosyllabic bases
///   `-երում` / `-երով` / `-երից` / `-երին` / `-երի`.
/// - Plural markers — `-ներ` (polysyllabic base) and `-եր`
///   (monosyllabic base).
/// - Singular case suffixes — instrumental `-ով`, locative `-ում`,
///   ablative `-ից`, dative `-ին`, genitive `-ի`.
/// - Postposed definite article — `-ը` after a consonant / `-ն`
///   after a vowel.
const SUFFIXES: &[&[char]] = &[
    // Aorist personal endings — longest first.
    &['ե', 'ց', 'ի', 'ն', 'ք'], // -եցինք (1pl)
    &['ե', 'ց', 'ի', 'ք'],      // -եցիք (2pl)
    &['ե', 'ց', 'ի', 'ն'],      // -եցին (3pl)
    &['ե', 'ց', 'ի', 'ր'],      // -եցիր (2sg)
    // Polysyllabic-plural + case combos (6+ chars).
    &['ն', 'ե', 'ր', 'ո', 'ւ', 'մ'], // -ներում (pl.loc)
    // Polysyllabic-plural + case combos (5 chars).
    &['ն', 'ե', 'ր', 'ո', 'վ'], // -ներով (pl.ins)
    &['ն', 'ե', 'ր', 'ի', 'ց'], // -ներից (pl.abl)
    &['ն', 'ե', 'ր', 'ի', 'ն'], // -ներին (pl.dat)
    &['ե', 'ց', 'ի'],           // -եցի (1sg aorist)
    // Monosyllabic-plural + case combos (5 chars).
    &['ե', 'ր', 'ո', 'ւ', 'մ'], // -երում (pl.loc)
    // Polysyllabic-plural + gen (4 chars).
    &['ն', 'ե', 'ր', 'ի'], // -ների (pl.gen)
    // Monosyllabic-plural + case combos (4 chars).
    &['ե', 'ր', 'ո', 'վ'], // -երով (pl.ins)
    &['ե', 'ր', 'ի', 'ց'], // -երից (pl.abl)
    &['ե', 'ր', 'ի', 'ն'], // -երին (pl.dat)
    // Plural markers (3 chars).
    &['ն', 'ե', 'ր'], // -ներ (polysyllabic pl.)
    // Monosyllabic-plural + gen (3 chars).
    &['ե', 'ր', 'ի'], // -երի (pl.gen)
    // Locative singular (3 chars, uses digraph `ու`).
    &['ո', 'ւ', 'մ'], // -ում (loc.sg / present participle)
    // Aorist 3sg (2 chars).
    &['ե', 'ց'], // -եց (3sg aorist)
    // Monosyllabic plural (2 chars).
    &['ե', 'ր'], // -եր (pl.)
    // Singular case suffixes (2 chars).
    &['ո', 'վ'], // -ով (ins.sg)
    &['ի', 'ց'], // -ից (abl.sg)
    &['ի', 'ն'], // -ին (dat.sg)
    // Genitive singular + postposed article (1 char each).
    &['ի'], // -ի (gen.sg)
    &['ը'], // -ը (definite article after consonant)
    &['ն'], // -ն (definite article after vowel / verb 3pl marker)
];

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

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &str) -> String {
        ArmenianStemmer.stem(w).into_owned()
    }

    #[test]
    fn empty_and_short_words_are_unchanged() {
        assert_eq!(s(""), "");
        assert_eq!(s("ա"), "ա");
        // `նա` — 2 chars: below the strip threshold (would leave <2).
        assert_eq!(s("նա"), "նա");
    }

    #[test]
    fn postposed_definite_article_strips() {
        // -ը (after consonant) strips from `մայրը` (mother-def) → մայր.
        assert_eq!(s("մայրը"), "մայր");
        // -ն (after vowel) strips from `տղան` (boy-def) → տղա.
        assert_eq!(s("տղան"), "տղա");
    }

    #[test]
    fn genitive_case_strips() {
        // -ի (gen.sg) — `գրքի` (of book) → գրք.
        assert_eq!(s("գրքի"), "գրք");
    }

    #[test]
    fn dative_case_strips() {
        // -ին (dat.sg) — `մարդին` (to person) → մարդ.
        assert_eq!(s("մարդին"), "մարդ");
    }

    #[test]
    fn ablative_case_strips() {
        // -ից (abl.sg) — `տնից` (from house) → տն.
        assert_eq!(s("տնից"), "տն");
    }

    #[test]
    fn instrumental_case_strips() {
        // -ով (ins.sg) — `գրիչով` (with pen) → գրիչ.
        assert_eq!(s("գրիչով"), "գրիչ");
    }

    #[test]
    fn locative_case_strips() {
        // -ում (loc.sg) — `քաղաքում` (in city) → քաղաք.
        assert_eq!(s("քաղաքում"), "քաղաք");
    }

    #[test]
    fn polysyllabic_plural_strips() {
        // -ներ — `գրքեր` is monosyllabic base; use a polysyllabic
        // base for the -ներ variant.
        assert_eq!(s("մարդներ"), "մարդ");
    }

    #[test]
    fn monosyllabic_plural_strips() {
        // -եր — `գրքեր` (books) → գրք.
        assert_eq!(s("գրքեր"), "գրք");
    }

    #[test]
    fn plural_plus_genitive_strips() {
        // -ների (pl.gen) — `մարդների` → մարդ.
        assert_eq!(s("մարդների"), "մարդ");
        // -երի (pl.gen, monosyllabic) — `գրքերի` → գրք.
        assert_eq!(s("գրքերի"), "գրք");
    }

    #[test]
    fn plural_plus_locative_strips() {
        // -ներում — `քաղաքներում` (in cities) → քաղաք.
        assert_eq!(s("քաղաքներում"), "քաղաք");
    }

    #[test]
    fn aorist_personal_endings_strip() {
        // -եցի (1sg) — `գնացի` (I went) → գնա (`ց` remains — the
        // suffix strips `-եցի` from `գնացի`? Actually the aorist stem
        // of `գնալ` is `գնաց-` and the 1sg is `գնացի`. The suffix
        // `-ի` is the actual personal ending on top of `գնաց-`; the
        // combined table entry `-եցի` doesn't match `գնացի` because
        // it isn't `-եցի` — it's `-ցի`. Let me test a verb whose
        // aorist stem is clean: `սիրեցի` (I loved) → սիր (drops
        // `-եցի`).
        assert_eq!(s("սիրեցի"), "սիր");
        // -եցին (3pl) — `սիրեցին` → սիր.
        assert_eq!(s("սիրեցին"), "սիր");
        // -եցինք (1pl) — `սիրեցինք` → սիր.
        assert_eq!(s("սիրեցինք"), "սիր");
    }

    #[test]
    fn convergence_strips_stacked_suffixes() {
        // Iterated loop handles `գրքերով` (books-instrumental) as a
        // single `-երով` strip (in the table). Verify the result.
        assert_eq!(s("գրքերով"), "գրք");
        // `գրքերից` — pl.abl in a single strip.
        assert_eq!(s("գրքերից"), "գրք");
    }

    #[test]
    fn eu_two_letter_spelling_normalizes_to_ligature() {
        // `եւ` (two letters) normalizes to `և` (single ligature)
        // and stays through — 1-char below strip threshold.
        assert_eq!(s("եւ"), "և");
    }

    #[test]
    fn min_stem_guard_prevents_over_stripping() {
        // `եմ` (I am, 2 chars) — the `-ը` / `-ն` / `-ի` suffixes are
        // 1 char; stripping would leave 1 char, below the 2-char
        // floor. Word stays.
        assert_eq!(s("եմ"), "եմ");
    }

    #[test]
    fn uppercase_input_lowercases() {
        // Case fold applies before suffix stripping.
        assert_eq!(s("ՄԱՅՐԸ"), s("մայրը"));
    }
}
