//! The Belarusian light stemmer.
//!
//! # Origin and design choice
//!
//! Unlike Russian, French, German, Spanish, and every other language
//! with a Porter/Boulton canonical algorithm, Belarusian is **not
//! covered by an official Snowball stemmer**. The Snowball project's
//! repository has no `belarusian.sbl`; community forks that repurpose
//! the Russian algorithm with the letter set adjusted exist but there
//! is no canonical reference implementation with a shipped `voc.txt` /
//! `output.txt` test-vector pair the way Russian has.
//!
//! Given the absence of a canonical Snowball Belarusian, this module
//! ships a **light suffix-stripping stemmer** rather than a
//! (non-canonical) port of the Russian algorithm. The rationale:
//!
//! * A community-fork port would inherit Russian's assumptions about
//!   perfective-gerund contexts, `нн` participle undoublement, and
//!   `ость` derivational endings that either do not apply to
//!   Belarusian or apply differently. Shipping a Russian algorithm
//!   with a substituted vowel set would produce plausible-looking but
//!   subtly wrong stems.
//! * A light stemmer whose scope is explicit — strip the longest
//!   matching inflectional suffix in a single pass, guarded by an RV
//!   floor — is easier to reason about, test, and improve
//!   incrementally.
//! * Downstream callers who need a full-morphology Belarusian
//!   lemmatizer should reach for a dictionary-based tool; this crate's
//!   charter is dictionary-free suffix stripping.
//!
//! # Algorithm sketch
//!
//! 1. **Lowercase.** Belarusian case-fold is well-behaved under Rust's
//!    default [`char::to_lowercase`]: `А → а`, `Ў → ў`, `І → і`, etc.
//!    all work under default rules with no Turkic-style tailoring.
//! 2. **Compute RV.** `RV` = the position after the first vowel in
//!    the word (or the end of the word if there is no vowel). Vowel
//!    set: `а е ё і о у ы э ю я` — note that **`ў` is a consonant**
//!    (the short-u glide), not a vowel, so it never triggers RV.
//! 3. **Reflexive strip.** If the word ends with `ся` in RV, strip
//!    it. This runs independently of the main suffix pass — it may or
//!    may not fire, and either way the main pass still runs on what
//!    remains.
//! 4. **Main suffix pass — globally longest match, with a theme-vowel
//!    context guard on past-tense endings.** Consult a single unified
//!    suffix table drawn from Belarusian noun, adjective, and verb
//!    inflection paradigms. The longest *eligible* suffix in the
//!    table that (a) matches the word's tail, (b) sits entirely
//!    inside RV, and (c) satisfies any per-suffix context predicate
//!    wins; if multiple suffixes tie on length, table order breaks
//!    the tie. This global-longest-match discipline avoids the
//!    ambiguity that would otherwise arise when a short verb suffix
//!    (`-ў`) is a proper suffix of a longer noun suffix (`-аў`) — the
//!    longer match always wins.
//!
//!    The **theme-vowel guard** applies to the past-tense endings
//!    `-ў`, `-ла`, `-ло`, `-лі`: those endings only fire if the
//!    character immediately preceding the suffix is a Belarusian theme
//!    vowel (`а`, `я`, `е`, `і`, `ы`, `ю`). Without this guard, a noun
//!    like `сталы` (nom. pl. of `стол`) would risk mis-stripping if
//!    the past-tense rule fired blindly; the guard blocks the strip
//!    and the bare-vowel `-ы` fires instead. Verbs like
//!    `чытаў`/`чытала`/`чыталі` all sit on a theme-vowel stem ending
//!    in `а` and pass the guard cleanly.
//! 5. **Trailing soft sign.** If the word ends with `ь`, remove it.
//!
//! # Byte-vs-char safety
//!
//! Every Cyrillic scalar in the Belarusian block is **2 bytes** in
//! UTF-8 (U+0400..=U+04FF and the U+045E / U+040E `ў` / `Ў` pair all
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
//!   Belarusian to be parity-with. This stemmer's output is defined
//!   by the algorithm above and the reference-pair table shipped in
//!   `tests/stemmer_reference.rs`.
//! * **Lemmatization.** Reducing `лепшы → добры`, `іду → ісці` needs
//!   a lexicon, not a suffix-stripping algorithm.
//! * **Verb-aspect stripping.** Belarusian's perfective/imperfective
//!   aspect prefixes (`па-`, `на-`, `за-`, …) are content-carrying
//!   and are not stripped.
//! * **Full-corpus cross-verification.** The reference-pair test
//!   embeds a hand-traced set that exercises each family of suffixes;
//!   full-corpus cross-verification would require a lexicon.
//! * **Taraškievič / Narkamaŭka toggle.** The stemmer is scoped to
//!   the Narkamaŭka orthography — Belarus's official standard. The
//!   two orthographies share the same suffix inventory, so callers
//!   fed Taraškievič input will get plausible stems, but the pack
//!   makes no guarantees about non-Narkamaŭka spelling variants.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use stringcheese_lang::Stemmer;

/// The Belarusian light suffix-stripping stemmer.
///
/// A zero-sized unit value; construct as [`BelarusianStemmer`] and
/// reuse the value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// the design choice to ship a light suffix-stripper rather than a
/// (non-canonical) Russian port.
///
/// # Example
///
/// ```
/// use stringcheese_be::BelarusianStemmer;
/// use stringcheese_lang::Stemmer;
///
/// assert_eq!(BelarusianStemmer.stem("красівы"), "красів");
/// assert_eq!(BelarusianStemmer.stem("сталы"), "стал");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct BelarusianStemmer;

impl BelarusianStemmer {
    /// Stems `word` per the Belarusian light stemmer.
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

impl Stemmer for BelarusianStemmer {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        BelarusianStemmer::stem(self, word)
    }
}

// ---------------------------------------------------------------------------
// Vowel classification.
// ---------------------------------------------------------------------------

/// The Belarusian vowel set for the RV region calculation:
/// `а е ё і о у ы э ю я`. Note that **`ў` (short u) is a consonant**,
/// not a vowel — it never triggers RV.
#[inline]
const fn is_vowel(c: char) -> bool {
    matches!(c, 'а' | 'е' | 'ё' | 'і' | 'о' | 'у' | 'ы' | 'э' | 'ю' | 'я')
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
/// and (if the candidate requires context) with a Belarusian theme
/// vowel — one of `а`, `я`, `е`, `і`, `ы`, `ю` — immediately preceding
/// the suffix. Returns the matched suffix (or `None`).
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
/// `suf_len` one of the Belarusian theme vowels
/// (`а`, `я`, `е`, `і`, `ы`, `ю`)?
///
/// Returns `false` if the suffix would leave nothing before it
/// (empty stem is never a valid past-tense verb).
fn preceded_by_theme_vowel(chars: &[char], suf_len: usize) -> bool {
    let stem_len = chars.len().saturating_sub(suf_len);
    if stem_len == 0 {
        return false;
    }
    matches!(chars[stem_len - 1], 'а' | 'я' | 'е' | 'і' | 'ы' | 'ю')
}

/// Truncate the trailing `s` characters from `chars`.
fn strip(chars: &mut Vec<char>, s: &[char]) {
    let n = chars.len() - s.len();
    chars.truncate(n);
}

// ---------------------------------------------------------------------------
// Reflexive endings.
// ---------------------------------------------------------------------------

const REFLEXIVE: &[&[char]] = &[&['с', 'я']];

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

/// The unified Belarusian inflectional-suffix table.
///
/// Draws from the noun, adjective, and verb paradigms. The stemmer
/// takes the globally longest match; when two suffixes tie on length,
/// the earlier one in this list wins.
///
/// The table is deliberately conservative — every entry is a
/// well-attested Belarusian inflectional suffix. High-precision cover
/// of derivational suffixes is out of scope for the light stemmer.
const MAIN_SUFFIXES: &[&[char]] = &[
    // ---- 3-character shapes ----
    // Adjective genitive / dative masculine.
    &['а', 'г', 'о'],
    &['я', 'г', 'о'],
    &['а', 'г', 'а'],
    &['я', 'г', 'а'],
    &['а', 'м', 'у'],
    &['я', 'м', 'у'],
    &['о', 'м', 'у'],
    &['е', 'м', 'у'],
    // Noun instrumental plurals.
    &['а', 'м', 'і'],
    &['я', 'м', 'і'],
    // Verb present-tense 3pl. The infinitive forms `-аць` / `-яць`
    // are deliberately absent — they would over-strip the theme
    // vowel out of `чытаць` (leaving `чыт` instead of `чыта`). The
    // 2-char `-ць` / `-ці` markers below strip only the infinitive
    // suffix and preserve the theme vowel.
    &['у', 'ц', 'ь'],
    &['ю', 'ц', 'ь'],
    &['е', 'ц', 'е'],
    &['э', 'ц', 'е'],
    &['е', 'м', 'о'],
    // ---- 2-character shapes ----
    // Adjective mid endings.
    &['а', 'я'],
    &['я', 'я'],
    &['а', 'е'],
    &['я', 'е'],
    &['ы', 'я'],
    &['і', 'я'],
    &['ы', 'х'],
    &['і', 'х'],
    &['ы', 'м'],
    &['і', 'м'],
    &['о', 'й'],
    &['е', 'й'],
    &['а', 'й'],
    &['а', 'ю'],
    // Noun instrumental / dative / locative shapes.
    &['а', 'м'],
    &['о', 'м'],
    &['е', 'м'],
    &['а', 'х'],
    &['я', 'х'],
    // Noun genitive plural. `-аў` is deliberately omitted from this
    // list — it would race against the past-tense `-ў` on verbs like
    // `чытаў` (whose stem `чыта` ends in the theme vowel `а`) and
    // steal the 2-char match, leaving `чыт` instead of `чыта`. The
    // theme-vowel-guarded `-ў` alone handles that case cleanly, and
    // this shape yields a plausible stem for genuine gp nouns
    // (`грошаў → гроша`) at the cost of leaving the theme vowel
    // behind. Rare-form under-stemming beats verb-form
    // over-stemming for a light stemmer whose job is clustering.
    &['я', 'ў'],
    &['о', 'ў'],
    &['е', 'ў'],
    // Verb infinitive — strip only the `-ць` / `-ці` marker, not
    // the theme vowel + marker. Words like `чытаць` stem to `чыта`
    // (keeping the theme vowel), matching the past-tense stems
    // `чытаў → чыта` and `чытала → чыта`. The 3-char forms `-аць`
    // and `-яць` are deliberately absent for the same reason as
    // `-аў` above.
    &['ц', 'ь'],
    &['ц', 'і'],
    // Verb present-tense 2sg.
    &['е', 'ш'],
    &['э', 'ш'],
    // Verb past-tense feminine / neuter / plural.
    &['л', 'а'],
    &['л', 'о'],
    &['л', 'і'],
    // ---- 1-character shapes (last-resort bare vowels + past -ў) ----
    &['а'],
    &['я'],
    &['е'],
    &['ё'],
    &['і'],
    &['ы'],
    &['о'],
    &['у'],
    &['ю'],
    &['й'],
    &['ў'],
];

/// The subset of [`MAIN_SUFFIXES`] that fires only when preceded by
/// a Belarusian theme vowel (`а`, `я`, `е`, `і`, `ы`, `ю`). The
/// past-tense verb endings — a noun-suffix false-positive otherwise.
const PAST_TENSE_GUARDED: &[&[char]] = &[&['ў'], &['л', 'а'], &['л', 'о'], &['л', 'і']];

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
        BelarusianStemmer.stem(w).into_owned()
    }

    #[test]
    fn empty_and_single_char_are_unchanged() {
        assert_eq!(s(""), "");
        assert_eq!(s("я"), "я");
    }

    #[test]
    fn adjective_masc_singular() {
        // "красівы" → adjective -ы → "красів".
        assert_eq!(s("красівы"), "красів");
    }

    #[test]
    fn adjective_fem_singular() {
        // "красівая" → -ая (2 chars — beats bare -я) → "красів".
        assert_eq!(s("красівая"), "красів");
    }

    #[test]
    fn adjective_neuter_singular() {
        // "красівае" → -ае → "красів".
        assert_eq!(s("красівае"), "красів");
    }

    #[test]
    fn adjective_plural() {
        // "красівыя" → -ыя → "красів".
        assert_eq!(s("красівыя"), "красів");
    }

    #[test]
    fn noun_plural_hard() {
        // "сталы" → noun -ы in RV → "стал".
        assert_eq!(s("сталы"), "стал");
    }

    #[test]
    fn verb_infinitive() {
        // "чытаць" → verb -ць → "чыта".
        assert_eq!(s("чытаць"), "чыта");
    }

    #[test]
    fn verb_past_masculine() {
        // "чытаў" → verb -ў → "чыта".
        assert_eq!(s("чытаў"), "чыта");
    }

    #[test]
    fn verb_past_feminine() {
        // "чытала" → -ла → "чыта".
        assert_eq!(s("чытала"), "чыта");
    }

    #[test]
    fn verb_past_plural() {
        // "чыталі" → -лі → "чыта".
        assert_eq!(s("чыталі"), "чыта");
    }

    #[test]
    fn reflexive_verb_past() {
        // "чытаўся" → reflexive -ся stripped first → "чытаў";
        // then main pass strips -ў → "чыта".
        assert_eq!(s("чытаўся"), "чыта");
    }

    #[test]
    fn genitive_plural_au_beats_past_u_short() {
        // "садоў" — the past-tense -ў would strip 1 char, but the
        // globally longest match -оў strips 2. -оў wins.
        assert_eq!(s("садоў"), "сад");
    }

    #[test]
    fn adjective_genitive_ago() {
        // "красівага" → -ага → "красів".
        assert_eq!(s("красівага"), "красів");
    }

    #[test]
    fn belarusian_short_u_is_preserved_in_stem_when_no_match() {
        // "аўтар" — RV=2 (after `а`), ends with 'р'; no suffix in the
        // table matches a word ending in 'р'. ў survives inside the
        // stem.
        assert_eq!(s("аўтар"), "аўтар");
    }

    #[test]
    fn trailing_soft_sign_is_stripped() {
        // "путь" — main pass strips -ь? No, `ь` is not in main table.
        // Trailing soft-sign rule strips it → "пут".
        assert_eq!(s("путь"), "пут");
    }

    #[test]
    fn short_word_is_unchanged() {
        // "год" (year) — RV=2 (after `о`), ends with 'д'. No suffix
        // in the table matches a word ending in 'д'; nothing fires.
        assert_eq!(s("год"), "год");
    }

    #[test]
    fn verb_present_3pl() {
        // "чытаюць" → -юць (3 chars) → "чыта".
        assert_eq!(s("чытаюць"), "чыта");
    }

    #[test]
    fn instrumental_plural_ami() {
        // "сталамі" → -амі → "стал".
        assert_eq!(s("сталамі"), "стал");
    }
}
