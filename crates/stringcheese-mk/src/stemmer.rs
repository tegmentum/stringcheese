//! The lightweight rule-based Macedonian stemmer.
//!
//! # Origin
//!
//! Macedonian has **no canonical Snowball stemmer** — the algorithm
//! Nakov (2003) shipped for Bulgarian is the closest published relative,
//! and the two languages share the analytic South Slavic profile (very
//! little nominal case declension, a postposed definite article as the
//! signature morphological feature). This module ships a hand-rolled
//! rule-based stemmer in the shape of the Bulgarian Snowball algorithm,
//! adapted for Macedonian's letter set and its **three-way** article
//! system (proximal / medial / distal, where Bulgarian has only one).
//!
//! # Algorithm sketch
//!
//! 1. **Preprocess.** Lowercase the input under Rust's default Unicode
//!    fold. Cyrillic case-fold is well-behaved — `А → а`, `Ѓ → ѓ`,
//!    `Ќ → ќ`, `Љ → љ`, `Њ → њ`, `Џ → џ`, `Ѕ → ѕ`, `Ј → ј` — with no
//!    locale-specific tailoring.
//! 2. **Region R1.** Compute R1 as the position after the first
//!    non-vowel following a vowel. Macedonian's vowel set is
//!    `а е и о у`. (Semi-vocalic `ј` counts as a consonant here — its
//!    role is that of a glide, not a vowel.)
//! 3. **Step 1 — remove definite article (in R1).** Macedonian's
//!    article agrees in gender / number *and* proximity:
//!      * Proximal (near / this-here): `-ов` (masc), `-ва` (fem),
//!        `-во` (neut), `-ве` (plur).
//!      * Medial (neutral, the default): `-от` (masc), `-та` (fem),
//!        `-то` (neut), `-те` (plur).
//!      * Distal (far / that-yonder): `-он` (masc), `-на` (fem),
//!        `-но` (neut), `-не` (plur).
//!
//!    All twelve suffixes are considered together; longest match wins.
//! 4. **Step 2 — remove plural.** Peel `-ови` / `-еви` (masc plurals of
//!    monosyllabic roots), `-ња` (neut plural of `-ње` derivations).
//!    The bare `-и` and `-а` plurals are handled by the step-4
//!    final-vowel pass; listing them alone here would over-strip
//!    high-frequency pronouns and adverbs.
//! 5. **Step 3 — remove verb endings.** A verb-inflection table in R1:
//!    present-tense `-ам` / `-аш` / `-а` / `-ат` / `-ме` / `-те`,
//!    aorist `-ав` / `-у`.
//! 6. **Step 4 — final adjective vowel.** If the word ends with a bare
//!    Macedonian vowel `-а` / `-о` / `-и` inside R1, delete it. This
//!    catches the adjective gender / number endings (fem `-a`, neut
//!    `-o`, plur `-и`) and the bare `-и` / `-а` noun plurals that the
//!    plural table above skipped.
//!
//! # Byte-vs-char safety
//!
//! Every Cyrillic scalar in the modern Macedonian block is **2 bytes**
//! in UTF-8 (U+0400..=U+045F falls in the 2-byte range). All the
//! arithmetic in this module operates on `Vec<char>` indices — never
//! raw byte offsets — so a suffix like `['и', 'т', 'е']` of char-length
//! 3 is 6 bytes of Cyrillic UTF-8 but only 3 slots of a `Vec<char>`.
//! The region calculation returns a char-index; the ends-with /
//! truncate helpers accept char-slices. There is no path through this
//! module that crosses a scalar boundary at the byte level.
//!
//! # Non-goals
//!
//! * **Palatal-alternation reversal.** Macedonian has productive
//!   palatal alternations (`к` / `ц`, `г` / `з`, `х` / `с`); reversing
//!   them without a lexicon over-restores for words where the
//!   alternation is fossilized. Deferred.
//! * **Aspect-prefix stripping.** Macedonian's perfective / imperfective
//!   aspect prefixes are content-carrying and are not stripped by this
//!   stemmer.
//! * **Lemmatization.** Full lemmatization requires a dictionary;
//!   deferred.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use stringcheese_lang::Stemmer;

/// The lightweight rule-based Macedonian stemmer.
///
/// A zero-sized unit value; construct as [`MacedonianStemmer`] and reuse
/// the value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules.
///
/// # Example
///
/// ```
/// use stringcheese_mk::MacedonianStemmer;
/// use stringcheese_lang::Stemmer;
///
/// // Definite-article stripping — Macedonian's signature feature.
/// // Medial article `-от`:
/// assert_eq!(MacedonianStemmer.stem("градот"), "град");
/// // Proximal article `-ов`:
/// assert_eq!(MacedonianStemmer.stem("градов"), "град");
/// // Distal article `-он`:
/// assert_eq!(MacedonianStemmer.stem("градон"), "град");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct MacedonianStemmer;

impl MacedonianStemmer {
    /// Stems `word` per the Macedonian rule-based algorithm.
    ///
    /// Returns the stem as a [`Cow`]. If the algorithm makes no change
    /// to a lowercase input, the returned `Cow` borrows the input.
    #[must_use]
    pub fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        if word.is_empty() {
            return Cow::Borrowed(word);
        }

        // 1. Lowercase (Unicode-aware). Assemble via a Vec<char> so
        // all downstream arithmetic operates in char space (never
        // bytes — Cyrillic is 2 bytes per char and byte arithmetic
        // would silently corrupt boundaries).
        let mut chars: Vec<char> = word.chars().flat_map(char::to_lowercase).collect();

        // Words of length 0..=1 (after the fold) stem to themselves;
        // no suffix rules apply on such short inputs.
        if chars.len() > 1 {
            // 2. Compute R1.
            let r1 = compute_r1(&chars);

            // 3. Step 1 — remove definite article (in R1). This runs
            // first because Macedonian's article is a noun-suffix, and
            // downstream suffix passes would misinterpret an articled
            // form's ending otherwise.
            remove_article(&mut chars, r1);

            // 4. Step 2 — remove plural.
            remove_plural(&mut chars, r1);

            // 5. Step 3 — remove verb endings.
            remove_verb(&mut chars, r1);

            // 6. Step 4 — final adjective / noun bare vowel.
            remove_final_vowel(&mut chars, r1);
        }

        let out: String = chars.iter().collect();
        if out == word {
            Cow::Borrowed(word)
        } else {
            Cow::Owned(out)
        }
    }
}

impl Stemmer for MacedonianStemmer {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        MacedonianStemmer::stem(self, word)
    }
}

// ---------------------------------------------------------------------------
// Vowel classification.
// ---------------------------------------------------------------------------

/// The Macedonian vowel set for the R1 region calculation: `а е и о у`.
///
/// Semi-vocalic `ј` is *not* counted — its phonological role is that
/// of a glide, and treating it as a vowel would push R1 too far into
/// short words like `мој`.
#[inline]
const fn is_vowel(c: char) -> bool {
    matches!(c, 'а' | 'е' | 'и' | 'о' | 'у')
}

// ---------------------------------------------------------------------------
// Region R1 — computed as a char index.
// ---------------------------------------------------------------------------

/// R1 = the region after the first non-vowel following a vowel.
///
/// If the word has no such non-vowel, R1 is the end of the word (i.e.
/// R1 is the null suffix).
fn compute_r1(chars: &[char]) -> usize {
    let n = chars.len();
    let mut i = 0;
    // Skip leading non-vowels to find the first vowel.
    while i < n && !is_vowel(chars[i]) {
        i += 1;
    }
    // Skip subsequent vowels.
    while i < n && is_vowel(chars[i]) {
        i += 1;
    }
    // R1 starts one position past the first non-vowel following a
    // vowel; or the end of the word if no such position exists.
    if i < n { i + 1 } else { n }
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

/// Is the suffix of char-length `suf_len` positioned entirely inside
/// the region beginning at char index `region_start`?
///
/// Equivalent to: `chars.len() - suf_len >= region_start`.
#[inline]
fn suffix_in(chars: &[char], suf_len: usize, region_start: usize) -> bool {
    chars.len().saturating_sub(suf_len) >= region_start
}

/// Find the longest suffix from `candidates` that `chars` ends with,
/// entirely within the region beginning at `region_start`. Returns
/// the matched slice (or `None`). When two candidates tie on length,
/// the earlier one in `candidates` wins.
fn longest_suffix_in<'a>(
    chars: &[char],
    candidates: &[&'a [char]],
    region_start: usize,
) -> Option<&'a [char]> {
    let mut best: Option<&[char]> = None;
    for &s in candidates {
        if ends_with(chars, s)
            && suffix_in(chars, s.len(), region_start)
            && best.is_none_or(|b| s.len() > b.len())
        {
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
// Step 1: Remove definite article.
// ---------------------------------------------------------------------------

/// The Macedonian definite-article suffix table — all twelve forms.
///
/// Macedonian's article agrees with the noun's gender and number *and*
/// its proximity to the speaker:
///
/// | Proximity | Masculine | Feminine | Neuter | Plural |
/// |-----------|-----------|----------|--------|--------|
/// | Proximal (near) | `-ов` | `-ва` | `-во` | `-ве` |
/// | Medial (neutral) | `-от` | `-та` | `-то` | `-те` |
/// | Distal (far) | `-он` | `-на` | `-но` | `-не` |
///
/// Only the medial series (`-от / -та / -то / -те`) corresponds to
/// Bulgarian's single article system; the proximal / distal series are
/// distinctly Macedonian.
///
/// All twelve suffixes are two characters long — longest-match wins
/// against the plural / verb / final-vowel passes that come later.
const ARTICLE_SUFFIXES: &[&[char]] = &[
    // Proximal.
    &['о', 'в'],
    &['в', 'а'],
    &['в', 'о'],
    &['в', 'е'],
    // Medial.
    &['о', 'т'],
    &['т', 'а'],
    &['т', 'о'],
    &['т', 'е'],
    // Distal.
    &['о', 'н'],
    &['н', 'а'],
    &['н', 'о'],
    &['н', 'е'],
];

/// Try to strip a definite-article suffix in R1. Returns `true` if
/// fired.
fn remove_article(chars: &mut Vec<char>, r1: usize) -> bool {
    if let Some(s) = longest_suffix_in(chars, ARTICLE_SUFFIXES, r1) {
        strip(chars, s);
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// Step 2: Remove plural.
// ---------------------------------------------------------------------------

/// The Macedonian plural-suffix table.
///
/// * `-ови` — masculine noun plural of monosyllabic roots
///   (`град` → `градови` — "city" / "cities").
/// * `-еви` — variant masculine plural after a soft consonant
///   (`крај` → `краеви` — "end" / "ends").
/// * `-ња` — neuter plural of `-ње` derivations (`пеење` → `пеења`).
///
/// The bare `-и` and `-а` plurals are handled by the step-4 final-vowel
/// step; listing them here would over-strip pronouns and short adverbs.
const PLURAL_SUFFIXES: &[&[char]] = &[&['о', 'в', 'и'], &['е', 'в', 'и'], &['њ', 'а']];

/// Try to strip a plural suffix in R1. Returns `true` if fired.
fn remove_plural(chars: &mut Vec<char>, r1: usize) -> bool {
    if let Some(s) = longest_suffix_in(chars, PLURAL_SUFFIXES, r1) {
        strip(chars, s);
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// Step 3: Remove verb / participle endings.
// ---------------------------------------------------------------------------

/// The Macedonian verb-inflection table.
///
/// Covers the present tense (1sg / 2sg / 3sg / 1pl / 2pl / 3pl) and
/// aorist / imperfect (1sg `-ав`, `-у`).
///
/// The bare 3sg present `-а` overlaps with the fem noun / adjective
/// bare-vowel ending and is deliberately left to the step-4 final-vowel
/// pass so the caller does not double-strip verb-and-noun homographs.
const VERB_SUFFIXES: &[&[char]] = &[
    // 1sg present a-conjugation `правам`, 1sg aorist `правав`.
    &['а', 'м'],
    &['а', 'ш'],
    &['а', 'т'],
    &['м', 'е'],
    &['т', 'е'],
    &['а', 'в'],
];

/// Try to strip a verb-inflection suffix in R1. Returns `true` if
/// fired.
fn remove_verb(chars: &mut Vec<char>, r1: usize) -> bool {
    if let Some(s) = longest_suffix_in(chars, VERB_SUFFIXES, r1) {
        strip(chars, s);
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// Step 4: Final bare vowel.
// ---------------------------------------------------------------------------

/// The Macedonian bare-vowel endings the step-4 pass strips in R1.
///
/// The bare vowels here cover:
///
/// * Adjective gender / number agreement: fem `-a`, neut `-o`,
///   plur `-и`.
/// * Noun plural markers not caught by step 2: bare `-и`, bare `-а`.
/// * The 3sg present `-а` and the 1sg present `-у` verb endings — kept
///   out of the step-3 table because those bare-vowel forms overlap
///   with common noun / adjective endings and are safer to peel here
///   under R1 guard.
///
/// The vowel `-е` is deliberately absent — it is the copula `е` and
/// stripping it would collapse too many high-frequency function words
/// (the copula is already covered by the stopword list). The vowel
/// `-о` covers neut, and `-у` covers the 1sg imperfect / present of
/// certain verbs.
const FINAL_VOWELS: &[&[char]] = &[&['а'], &['и'], &['о'], &['у']];

/// If the word ends with a bare Macedonian vowel inside R1, delete it.
/// Returns `true` if fired.
fn remove_final_vowel(chars: &mut Vec<char>, r1: usize) -> bool {
    if let Some(s) = longest_suffix_in(chars, FINAL_VOWELS, r1) {
        strip(chars, s);
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &str) -> String {
        MacedonianStemmer.stem(w).into_owned()
    }

    #[test]
    fn empty_and_single_char_are_unchanged() {
        assert_eq!(s(""), "");
        assert_eq!(s("а"), "а");
    }

    #[test]
    fn definite_article_medial_masc() {
        // `градот` = `град` + `-от` (medial masc definite) → `град`.
        assert_eq!(s("градот"), "град");
    }

    #[test]
    fn definite_article_medial_fem() {
        // `книгата` = `книга` + `-та` (medial fem definite) → `книг`.
        // After article strip: `книга`. Step 4 strips bare `-а` → `книг`.
        assert_eq!(s("книгата"), "книг");
    }

    #[test]
    fn definite_article_medial_neut() {
        // `детето` = `дете` + `-то` → `дете`. Step 4 strips `-е`? No —
        // `-е` is not in FINAL_VOWELS. So we get `дете`.
        // Actually the final-vowel table drops `-е`, so `дете` stands.
        assert_eq!(s("детето"), "дете");
    }

    #[test]
    fn definite_article_medial_plur() {
        // `градовите` = `град` + `-ови` (plural) + `-те` (article).
        // Article step strips `-те` → `градови`. Plural step strips
        // `-ови` → `град`.
        assert_eq!(s("градовите"), "град");
    }

    #[test]
    fn definite_article_proximal_masc() {
        // `градов` = `град` + `-ов` (proximal, "this city here") →
        // `град`. The proximal article is distinctly Macedonian —
        // Bulgarian has no such form.
        assert_eq!(s("градов"), "град");
    }

    #[test]
    fn definite_article_distal_masc() {
        // `градон` = `град` + `-он` (distal, "that city yonder") →
        // `град`. Distal is likewise distinctly Macedonian.
        assert_eq!(s("градон"), "град");
    }

    #[test]
    fn all_three_proximity_articles_agree() {
        // The whole point of the article step: proximal, medial, and
        // distal articled forms of the same noun all collapse to the
        // same stem as the bare form.
        let bare = s("град");
        assert_eq!(s("градот"), bare);
        assert_eq!(s("градов"), bare);
        assert_eq!(s("градон"), bare);
    }

    #[test]
    fn plural_ови() {
        // `градови` — plural of `град`. Plural step strips `-ови`.
        assert_eq!(s("градови"), "град");
    }

    #[test]
    fn verb_present_1sg() {
        // `правам` = 1sg present. Verb step strips `-ам`.
        assert_eq!(s("правам"), "прав");
    }

    #[test]
    fn verb_present_2sg_аш() {
        // `праваш` = 2sg present a-conjugation. Verb step strips `-аш`
        // → `прав`. (The `-иш` 2sg for i-conjugation verbs is
        // deliberately absent from the verb table — it would collide
        // with common noun endings; a full paradigm treatment would
        // need a lexicon.)
        assert_eq!(s("праваш"), "прав");
    }

    #[test]
    fn verb_aorist_1sg_ав() {
        // `правав` = 1sg aorist. Verb step strips `-ав`.
        assert_eq!(s("правав"), "прав");
    }

    #[test]
    fn short_word_is_preserved() {
        // Short words: R1 protects the stem.
        assert_eq!(s("сум"), "сум");
        assert_eq!(s("не"), "не");
    }

    #[test]
    fn adjective_fem() {
        // `нова` = adjective fem. Step 4 strips `-а` in R1 → `нов`.
        assert_eq!(s("нова"), "нов");
    }

    #[test]
    fn adjective_plural() {
        // `нови` = adjective plur. Step 4 strips `-и` → `нов`.
        assert_eq!(s("нови"), "нов");
    }

    #[test]
    fn macedonian_specific_letter_ќ_survives() {
        // `куќа` = fem noun. Step 4 strips `-а` → `куќ`. The
        // Macedonian-specific `ќ` (U+045C) is preserved.
        assert_eq!(s("куќа"), "куќ");
    }

    #[test]
    fn macedonian_specific_letter_љ_survives_in_bare_stem() {
        // `љубезен` = "polite / kind", masc adj. Ends with `-зен`; no
        // article match, no verb match, no bare-vowel match. The
        // Macedonian-specific `љ` (U+0459) is preserved.
        assert_eq!(s("љубезен"), "љубезен");
    }

    #[test]
    fn proximal_article_over_strips_where_a_noun_happens_to_end_in_ov() {
        // Known limitation of the dictionary-free rule-based stemmer:
        // `љубов` ("love", fem noun) ends in the same character sequence
        // as the masculine proximal article `-ов`, and the stemmer
        // cannot distinguish the two without a lexicon. Callers who
        // need `љубов` recognized as a bare stem should combine this
        // pack with a named-entity / lexicon filter.
        assert_eq!(s("љубов"), "љуб");
    }
}
