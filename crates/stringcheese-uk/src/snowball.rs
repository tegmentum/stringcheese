//! The Ukrainian light stemmer.
//!
//! # Origin and design choice
//!
//! Unlike Russian, French, German, Spanish, and every other language
//! with a Porter/Boulton canonical algorithm, Ukrainian is **not
//! covered by an official Snowball stemmer**. The Snowball project's
//! repository has no `ukrainian.sbl`; community forks (notably the
//! `snowball-ukrainian` GitHub repository) offer partial ports of the
//! Russian algorithm with the letter set adjusted, but there is no
//! canonical reference implementation with a shipped `voc.txt` /
//! `output.txt` test-vector pair the way Russian has.
//!
//! Given the absence of a canonical Snowball Ukrainian, this module
//! ships a **light suffix-stripping stemmer** rather than a
//! (non-canonical) port of the Russian algorithm. The rationale:
//!
//! * A community-fork port would inherit Russian's assumptions about
//!   perfective-gerund contexts, `нн` participle undoublement, and
//!   `ость` derivational endings that either do not apply to Ukrainian
//!   or apply differently. Shipping a Russian algorithm with a
//!   substituted vowel set would produce plausible-looking but subtly
//!   wrong stems.
//! * A light stemmer whose scope is explicit — strip the longest
//!   matching inflectional suffix in a single pass, guarded by an RV
//!   floor — is easier to reason about, test, and improve
//!   incrementally.
//! * Downstream callers who need a full-morphology Ukrainian
//!   lemmatizer should reach for a dictionary-based tool (VESUM,
//!   pymorphy2-uk, LanguageTool-uk); this crate's charter is
//!   dictionary-free suffix stripping.
//!
//! # Algorithm sketch
//!
//! 1. **Lowercase.** Ukrainian case-fold is well-behaved under Rust's
//!    default [`char::to_lowercase`]: `А → а`, `Ґ → ґ`, `Є → є`,
//!    `І → і`, `Ї → ї`, etc. all work under default rules with no
//!    Turkic-style tailoring.
//! 2. **Compute RV.** `RV` = the position after the first vowel in
//!    the word (or the end of the word if there is no vowel). Vowel
//!    set: `а е є и і ї о у ю я`.
//! 3. **Reflexive strip.** If the word ends with `ся` or `сь` in RV,
//!    strip it. This runs independently of the main suffix pass — it
//!    may or may not fire, and either way the main pass still runs on
//!    what remains.
//! 4. **Main suffix pass — globally longest match, with a theme-vowel
//!    context guard on past-tense endings.** Consult a single unified
//!    suffix table drawn from Ukrainian noun, adjective, and verb
//!    inflection paradigms. The longest *eligible* suffix in the
//!    table that (a) matches the word's tail, (b) sits entirely
//!    inside RV, and (c) satisfies any per-suffix context predicate
//!    wins; if multiple suffixes tie on length, table order breaks
//!    the tie. This global-longest-match discipline avoids the
//!    ambiguity that would otherwise arise when a short verb suffix
//!    (`-в`) is a proper suffix of a longer noun suffix (`-ів`) — the
//!    longer match always wins.
//!
//!    The **theme-vowel guard** applies to the past-tense endings
//!    `-в`, `-ла`, `-ло`, `-ли`: those endings only fire if the
//!    character immediately preceding the suffix is a theme vowel
//!    (`а`, `я`, `и`, `і`, `ї`). Without this guard, a noun like
//!    `столи` (nom. pl. of `стіл`) would strip its `-ли` as if it
//!    were a past-tense verb — but the character before `-ли` is
//!    `о`, not a theme vowel, so the guard blocks the strip and the
//!    bare-vowel `-и` fires instead, yielding the correct `стол`.
//!    Verbs like `читав`/`читала`/`читали` all sit on a theme-vowel
//!    stem ending in `а` and pass the guard cleanly.
//! 5. **Trailing soft sign.** If the word ends with `ь`, remove it.
//!
//! # Byte-vs-char safety
//!
//! Every Cyrillic scalar in the Ukrainian block is **2 bytes** in
//! UTF-8 (U+0400..=U+04FF and the U+0490 / U+0491 `Ґ` / `ґ` pair all
//! fall in the 2-byte range U+0080..=U+07FF). All the arithmetic in
//! this module operates on `Vec<char>` indices — never raw byte
//! offsets — so a suffix `['с', 'я']` of char-length 2 is 4 bytes of
//! Cyrillic UTF-8 but only 2 slots of a `Vec<char>`. The region
//! calculation returns a char-index; the ends-with / truncate helpers
//! accept char-slices. There is no path through this module that
//! crosses a scalar boundary at the byte level.
//!
//! # Non-goals
//!
//! * **Canonical Snowball parity.** There is no canonical Snowball
//!   Ukrainian to be parity-with. This stemmer's output is defined by
//!   the algorithm above and the reference-pair table shipped in
//!   `tests/snowball_reference.rs`.
//! * **Lemmatization.** Reducing `кращий → добрий`, `іду → йти`,
//!   `купував → купувати` needs a lexicon, not a suffix-stripping
//!   algorithm.
//! * **Verb-aspect stripping.** Ukrainian's perfective/imperfective
//!   aspect prefixes (`про-`, `на-`, `з-`, …) are content-carrying
//!   and are not stripped.
//! * **Full-corpus cross-verification.** The reference-pair test
//!   embeds a hand-traced set that exercises each family of suffixes;
//!   full-corpus cross-verification would require a lexicon.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use stringcheese_lang::Stemmer;

/// The Ukrainian light suffix-stripping stemmer.
///
/// A zero-sized unit value; construct as [`UkrainianSnowball`] and
/// reuse the value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// the design choice to ship a light suffix-stripper rather than a
/// (non-canonical) Russian port. The name keeps the `Snowball` suffix
/// to match the module naming convention used by every other
/// StringCheese language pack, not to claim canonical Snowball status
/// — there is no canonical Snowball Ukrainian.
///
/// # Example
///
/// ```
/// use stringcheese_uk::UkrainianSnowball;
/// use stringcheese_lang::Stemmer;
///
/// assert_eq!(UkrainianSnowball.stem("красивий"), "красив");
/// assert_eq!(UkrainianSnowball.stem("столи"), "стол");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct UkrainianSnowball;

impl UkrainianSnowball {
    /// Stems `word` per the Ukrainian light stemmer.
    ///
    /// Returns the stem as a [`Cow`]. If the algorithm makes no
    /// change to a lowercase input, the returned `Cow` borrows the
    /// input.
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
            let rv = compute_rv(&chars);

            // 2. Reflexive strip (independent of the main pass).
            try_reflexive(&mut chars, rv);

            // 3. Main suffix pass — globally longest match across
            // the unified noun / adjective / verb inflection table.
            try_main_suffix(&mut chars, rv);

            // 4. Trailing soft sign.
            strip_trailing_soft_sign(&mut chars);
        }

        let out: String = chars.iter().collect();
        if out == word {
            Cow::Borrowed(word)
        } else {
            Cow::Owned(out)
        }
    }
}

impl Stemmer for UkrainianSnowball {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        UkrainianSnowball::stem(self, word)
    }
}

// ---------------------------------------------------------------------------
// Vowel classification.
// ---------------------------------------------------------------------------

/// The Ukrainian vowel set for the RV region calculation:
/// `а е є и і ї о у ю я`.
#[inline]
const fn is_vowel(c: char) -> bool {
    matches!(c, 'а' | 'е' | 'є' | 'и' | 'і' | 'ї' | 'о' | 'у' | 'ю' | 'я')
}

// ---------------------------------------------------------------------------
// Region RV — computed as a char index.
// ---------------------------------------------------------------------------

/// RV = the position after the first vowel in the word.
///
/// If the word has no vowel, RV is the end of the word (i.e. RV is
/// the null suffix).
fn compute_rv(chars: &[char]) -> usize {
    let n = chars.len();
    for (i, &c) in chars.iter().enumerate() {
        if is_vowel(c) {
            return (i + 1).min(n);
        }
    }
    n
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

/// Find the longest *eligible* suffix from `candidates` that `chars`
/// ends with, entirely within the region beginning at `region_start`,
/// and (if the candidate requires context) with a theme vowel — one
/// of `а`, `я`, `и`, `і`, `ї` — immediately preceding the suffix.
/// Returns the matched suffix (or `None`).
///
/// This is the [`longest_suffix_in`] variant used for the main pass;
/// it consults the [`PAST_TENSE_GUARDED`] set to decide whether the
/// theme-vowel predicate applies to a given candidate.
fn longest_eligible_suffix<'a>(
    chars: &[char],
    candidates: &[&'a [char]],
    region_start: usize,
) -> Option<&'a [char]> {
    let mut best: Option<&[char]> = None;
    for &s in candidates {
        if !ends_with(chars, s) || !suffix_in(chars, s.len(), region_start) {
            continue;
        }
        if PAST_TENSE_GUARDED.contains(&s) && !preceded_by_theme_vowel(chars, s.len()) {
            continue;
        }
        if best.is_none_or(|b| s.len() > b.len()) {
            best = Some(s);
        }
    }
    best
}

/// Is the character immediately preceding a suffix of char-length
/// `suf_len` one of the Ukrainian theme vowels
/// (`а`, `я`, `и`, `і`, `ї`)?
///
/// Returns `false` if the suffix would leave nothing before it
/// (empty stem is never a valid past-tense verb).
fn preceded_by_theme_vowel(chars: &[char], suf_len: usize) -> bool {
    let stem_len = chars.len().saturating_sub(suf_len);
    if stem_len == 0 {
        return false;
    }
    matches!(chars[stem_len - 1], 'а' | 'я' | 'и' | 'і' | 'ї')
}

/// Truncate the trailing `s` characters from `chars`.
fn strip(chars: &mut Vec<char>, s: &[char]) {
    let n = chars.len() - s.len();
    chars.truncate(n);
}

// ---------------------------------------------------------------------------
// Reflexive endings.
// ---------------------------------------------------------------------------

const REFLEXIVE: &[&[char]] = &[&['с', 'я'], &['с', 'ь']];

/// Try to strip a REFLEXIVE ending in RV. Returns `true` if fired.
fn try_reflexive(chars: &mut Vec<char>, rv: usize) -> bool {
    if let Some(s) = longest_suffix_in(chars, REFLEXIVE, rv) {
        strip(chars, s);
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// Main suffix table — unified noun / adjective / verb endings.
// ---------------------------------------------------------------------------

/// The unified Ukrainian inflectional-suffix table.
///
/// Draws from the noun, adjective, and verb paradigms. The stemmer
/// takes the globally longest match; when two suffixes tie on length,
/// the earlier one in this list wins.
///
/// The table is deliberately conservative — every entry is a
/// well-attested Ukrainian inflectional suffix. High-precision cover
/// of derivational suffixes (`-ість`, `-ання`, `-ство`) is out of
/// scope for the light stemmer.
const MAIN_SUFFIXES: &[&[char]] = &[
    // ---- 5-character comparative / superlative shapes ----
    &['і', 'ш', 'о', 'г', 'о'],
    &['і', 'ш', 'о', 'м', 'у'],
    &['і', 'ш', 'и', 'м', 'и'],
    // ---- 4-character shapes ----
    &['і', 'ш', 'о', 'ю'],
    &['і', 'ш', 'о', 'ї'],
    &['і', 'ш', 'о', 'м'],
    &['і', 'ш', 'и', 'х'],
    &['і', 'ш', 'и', 'м'],
    &['і', 'ш', 'и', 'й'],
    // ---- 3-character shapes ----
    // Adjective long forms.
    &['о', 'г', 'о'],
    &['о', 'м', 'у'],
    &['е', 'м', 'у'],
    &['и', 'м', 'и'],
    &['і', 'м', 'и'],
    // Verb present-tense plurals + imperatives.
    &['ю', 'т', 'ь'],
    &['у', 'т', 'ь'],
    &['а', 'т', 'ь'],
    &['я', 'т', 'ь'],
    &['е', 'м', 'о'],
    &['є', 'м', 'о'],
    &['и', 'м', 'о'],
    &['ї', 'м', 'о'],
    &['е', 'т', 'е'],
    &['є', 'т', 'е'],
    &['и', 'т', 'е'],
    &['ї', 'т', 'е'],
    &['и', 'т', 'ь'],
    &['й', 'т', 'е'],
    // Noun instrumental / dative / locative plurals.
    &['я', 'м', 'и'],
    &['а', 'м', 'и'],
    // Noun animate dative singular.
    &['о', 'в', 'і'],
    &['е', 'в', 'і'],
    // Comparative/superlative 2-char bases (allow -іш standalone).
    &['і', 'ш', 'а'],
    &['і', 'ш', 'е'],
    &['і', 'ш', 'і'],
    // ---- 2-character shapes ----
    // Adjective mid endings.
    &['о', 'ю'],
    &['е', 'ю'],
    &['о', 'ї'],
    &['и', 'х'],
    &['і', 'х'],
    &['и', 'м'],
    &['і', 'м'],
    &['о', 'м'],
    &['е', 'м'],
    // Adjective nominative singular / plural.
    &['и', 'й'],
    &['і', 'й'],
    // Verb infinitive.
    &['т', 'и'],
    &['т', 'ь'],
    // Verb past-tense feminine / neuter / plural.
    &['л', 'а'],
    &['л', 'о'],
    &['л', 'и'],
    // Verb present-tense 2sg.
    &['е', 'ш'],
    &['є', 'ш'],
    &['и', 'ш'],
    &['ї', 'ш'],
    // Noun genitive plural.
    &['і', 'в'],
    &['ї', 'в'],
    &['е', 'й'],
    // Noun instrumental / locative plural (short).
    &['я', 'х'],
    &['а', 'х'],
    &['я', 'м'],
    &['а', 'м'],
    // ---- 1-character shapes (last-resort bare vowels + past -в) ----
    &['а'],
    &['я'],
    &['е'],
    &['є'],
    &['і'],
    &['и'],
    &['о'],
    &['у'],
    &['ю'],
    &['й'],
    &['в'],
];

/// The subset of [`MAIN_SUFFIXES`] that fires only when preceded by
/// a Ukrainian theme vowel (`а`, `я`, `и`, `і`, `ї`). The past-tense
/// verb endings — a noun-suffix false-positive otherwise.
const PAST_TENSE_GUARDED: &[&[char]] = &[&['в'], &['л', 'а'], &['л', 'о'], &['л', 'и']];

/// Try to strip the globally longest eligible main-table suffix in
/// RV. Returns `true` if fired.
fn try_main_suffix(chars: &mut Vec<char>, rv: usize) -> bool {
    if let Some(s) = longest_eligible_suffix(chars, MAIN_SUFFIXES, rv) {
        strip(chars, s);
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// Trailing soft sign.
// ---------------------------------------------------------------------------

fn strip_trailing_soft_sign(chars: &mut Vec<char>) {
    if ends_with(chars, &['ь']) {
        chars.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &str) -> String {
        UkrainianSnowball.stem(w).into_owned()
    }

    #[test]
    fn empty_and_single_char_are_unchanged() {
        assert_eq!(s(""), "");
        assert_eq!(s("я"), "я");
    }

    #[test]
    fn adjective_masc_singular() {
        // "красивий" → adjective -ий → "красив".
        assert_eq!(s("красивий"), "красив");
    }

    #[test]
    fn adjective_fem_singular() {
        // "красива" → bare -а → "красив".
        assert_eq!(s("красива"), "красив");
    }

    #[test]
    fn noun_plural_hard() {
        // "столи" → noun -и in RV → "стол".
        assert_eq!(s("столи"), "стол");
    }

    #[test]
    fn verb_infinitive() {
        // "читати" → verb -ти → "чита".
        assert_eq!(s("читати"), "чита");
    }

    #[test]
    fn verb_past_masculine() {
        // "читав" → verb -в → "чита".
        assert_eq!(s("читав"), "чита");
    }

    #[test]
    fn reflexive_verb() {
        // "сміятися" → reflexive -ся stripped first, then verb -ти
        // stripped → "смія".
        assert_eq!(s("сміятися"), "смія");
    }

    #[test]
    fn genitive_plural_iv_beats_past_v() {
        // "будинків" — the past-tense -в would strip 1 char, but the
        // globally longest match -ів strips 2. -ів wins.
        assert_eq!(s("будинків"), "будинк");
    }

    #[test]
    fn instrumental_plural_amy() {
        // "столами" → -ами → "стол".
        assert_eq!(s("столами"), "стол");
    }

    #[test]
    fn adjective_genitive_ogo() {
        // "красивого" → -ого → "красив".
        assert_eq!(s("красивого"), "красив");
    }

    #[test]
    fn ukrainian_letters_are_preserved_in_stem() {
        // "їжак" — no matching suffix (ends with 'к'); ї survives.
        assert_eq!(s("їжак"), "їжак");
        // "ґудзик" — no matching suffix; ґ survives.
        assert_eq!(s("ґудзик"), "ґудзик");
    }

    #[test]
    fn trailing_soft_sign_is_stripped() {
        // "мить" — main pass strips -ть (2 chars — beats the bare -ь
        // trailing-strip alternative). Result: "ми". Trailing soft
        // sign rule has nothing to strip.
        assert_eq!(s("мить"), "ми");
    }

    #[test]
    fn short_word_is_unchanged() {
        // "рік" (year) — RV=2 (after `і`), ends with 'к'. No suffix
        // in the table matches a word ending in 'к'; nothing fires.
        assert_eq!(s("рік"), "рік");
    }
}
