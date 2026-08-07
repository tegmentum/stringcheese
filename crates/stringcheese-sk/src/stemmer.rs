//! The Slovak light suffix-stripping stemmer.
//!
//! # Origin and design choice
//!
//! Slovak, like Czech, is **not covered by an official Snowball
//! stemmer**. Community forks and academic algorithms exist (variants
//! of the Dolamic & Savoy Czech light stemmer adapted for Slovak,
//! hand-crafted Slovak stemmers used in academic IR papers) but none
//! has been adopted as canonical.
//!
//! Given the absence of a canonical Snowball Slovak, this module
//! ships a **light suffix-stripping stemmer** with a **deliberately
//! conservative** suffix table hand-audited against the pack's
//! reference-pair tests. Rationale mirrors the Czech pack's:
//!
//! * A published-but-non-canonical algorithm's exact behaviour on
//!   corner cases is uncertain; shipping one would produce
//!   plausible-looking but subtly wrong stems that fail silently.
//! * A light stemmer whose scope is explicit — strip the longest
//!   matching inflectional suffix in a single pass, guarded by an RV
//!   floor — is easier to reason about, test, and improve
//!   incrementally.
//! * Downstream callers who need a full-morphology Slovak lemmatizer
//!   should reach for a dictionary-based tool; this crate's charter
//!   is dictionary-free suffix stripping.
//!
//! **Over-stemming Slovak is easy without a lexicon.** Slovak
//! morphology applies **velar / palatal alternations** to the stem
//! (`k / c / č`, `h / z / ž`, `ch / š`) in certain case-number cells,
//! as Czech does. The light stemmer strips the suffix but does not
//! reverse the alternation. Reversing it is deferred to a
//! lexicon-backed variant.
//!
//! # Slovak vs. Czech
//!
//! This stemmer is shaped after the Czech pack's `stringcheese_cs::stemmer`
//! but diverges from it in a handful of places:
//!
//! * **Infinitive suffix is `-ť`.** Slovak infinitives end in `-ť`,
//!   not Czech's `-t`. The suffix table encodes `-ovať` / `-ať` /
//!   `-iť` / `-ieť` / `-núť`, and there is no `-ovat` / `-at` /
//!   `-it` / `-ět` in the Slovak table.
//! * **Past-tense plural is `-ovali` only.** Slovak past-tense
//!   plural has no gender split (Czech has `-ovali` for masc-anim
//!   and `-ovaly` for the rest); the Slovak table drops the
//!   `-ovaly` entry.
//! * **Present tense of `-ovať` verbs.** The Slovak paradigm is
//!   `-ujem` / `-uješ` / `-uje` / `-ujeme` / `-ujete` / `-ujú`,
//!   which differs from Czech's `-uji` / `-uješ` / `-uje` / `-ujeme`
//!   / `-ujete` / `-ují`. Slovak-specific entries are added and the
//!   Czech-specific `-uji` is dropped.
//! * **Masculine-noun instrumental singular is `-om`.** Czech has
//!   `-em` (`pánem`); Slovak has `-om` (`pánom`). Both are in the
//!   table (Slovak inherits a handful of `-em` paradigms too), but
//!   `-om` is the primary Slovak form.
//! * **Additional Slovak vowels in the RV computation.** The Slovak
//!   vowel inventory adds `ä`, `ô`, and the syllabic long consonants
//!   `ĺ` and `ŕ` (which carry a syllable nucleus in Slovak and are
//!   treated as vowels in vowel-consonant alternation rules).
//! * **No `ě` / `ř` / `ů`.** These Czech letters do not appear in
//!   Slovak; the corresponding suffix entries (`-ět` / `-ěl`, any
//!   `ř`- or `ů`-carrying suffix) are absent from the Slovak table.
//!
//! # Algorithm sketch
//!
//! 1. **Lowercase.** Slovak case-fold is well-behaved under Rust's
//!    default [`char::to_lowercase`]: `Á → á`, `Č → č`, `Ď → ď`,
//!    `Í → í`, `Ĺ → ĺ`, `Ľ → ľ`, `Ň → ň`, `Ó → ó`, `Ô → ô`, `Ŕ → ŕ`,
//!    `Š → š`, `Ť → ť`, `Ú → ú`, `Ý → ý`, `Ž → ž`, `Ä → ä`. There is
//!    no locale tailoring to apply.
//! 2. **Compute RV.** `RV` = the position after the first vowel-then-
//!    consonant pair, or the end of the word if no such position
//!    exists. Vowel set (Slovak): `a e i o u y á é í ó ú ý ä ô ĺ ŕ`.
//!    This is the Snowball-family convention adapted for the Slovak
//!    vowel inventory. RV is a minimum-preserved-stem-length guard:
//!    no suffix rule may strip a character at position < RV.
//! 3. **Main suffix pass — globally longest match.** Consult a single
//!    unified suffix table drawn from Slovak noun / adjective / verb
//!    inflection paradigms. The longest suffix that (a) matches the
//!    word's tail and (b) sits entirely inside RV wins; ties are
//!    broken by table order.
//!
//! # Suffix table shape
//!
//! The table covers, per the task specification and per Slovak
//! morphology:
//!
//! * **Noun / adjective / possessive endings** — `-ovi`, `-ova`,
//!   `-ovo`, `-ove`, `-ovu`, `-ami`, `-emi`, `-ám`, `-ým`, `-ého`,
//!   `-ých`, `-ému`, `-ými`, `-ách`, `-om`, `-em`, `-ou`, bare `-a`,
//!   `-e`, `-i`, `-o`, `-u`, `-y`, `-á`, `-é`, `-í`, `-ý`.
//! * **Verb endings (mostly `-ovať` family, plus present tense)** —
//!   `-oval`, `-ovala`, `-ovalo`, `-ovali`, `-ovať`, `-ujem`,
//!   `-uješ`, `-uje`, `-ujeme`, `-ujete`, `-ujú`.
//! * **Additional inflectional suffixes** — past-tense `-al` /
//!   `-il` / `-el` for the `-ať` / `-iť` / other verb classes;
//!   infinitive `-ať` / `-iť` / `-ieť` / `-núť`; past-tense
//!   `-nul` for `-núť` verbs.
//!
//! # Byte-vs-char safety
//!
//! Every Slovak-specific scalar is UTF-8 multi-byte (2 bytes each in
//! U+0080..=U+07FF for `á/é/í/ó/ú/ý/ä/ô` and 2 bytes in
//! U+0100..=U+017F for the extended Latin `č/š/ž/ď/ť/ň/ľ/ĺ/ŕ`). All
//! the arithmetic in this module operates on `Vec<char>` indices —
//! never raw byte offsets — so a suffix `['o', 'v', 'a', 'ť']` of
//! char-length 4 is 5 bytes of UTF-8 but only 4 slots of a
//! `Vec<char>`. There is no path through this module that crosses a
//! scalar boundary at the byte level.
//!
//! # Non-goals
//!
//! * **Canonical Snowball parity.** There is no canonical Snowball
//!   Slovak to be parity-with. This stemmer's output is defined by
//!   the algorithm above and the reference-pair table shipped in
//!   `tests/stemmer_reference.rs`.
//! * **Aggressive derivational stripping.** The current table sticks
//!   to inflectional suffixes. A parallel aggressive variant would
//!   strip derivational suffixes (`-osť`, `-stvo`, `-izmus`) and add
//!   a palatalization step.
//! * **Consonant alternation reversal.** `ruka / ruce` and similar
//!   velar/palatal alternations need a lexicon to reverse correctly;
//!   the light stemmer strips the suffix and leaves the alternation
//!   in place.
//! * **Lemmatization.** Reducing `lepší → dobrý`, `som → byť` needs a
//!   lexicon, not a suffix-stripping algorithm.
//! * **Full-corpus cross-verification.** The reference-pair test
//!   embeds a hand-traced set that exercises each family of suffixes;
//!   full-corpus cross-verification would require a lexicon.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use stringcheese_lang::Stemmer;

/// The Slovak light suffix-stripping stemmer.
///
/// A zero-sized unit value; construct as [`SlovakStemmer`] and reuse
/// the value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// the design choice to ship a light suffix-stripper rather than a
/// (non-canonical) port of a related-language algorithm.
///
/// # Example
///
/// ```
/// use stringcheese_sk::SlovakStemmer;
/// use stringcheese_lang::Stemmer;
///
/// assert_eq!(SlovakStemmer.stem("pekná"), "pekn");
/// assert_eq!(SlovakStemmer.stem("pracoval"), "prac");
/// assert_eq!(SlovakStemmer.stem("robiť"), "rob");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SlovakStemmer;

impl SlovakStemmer {
    /// Stems `word` per the Slovak light stemmer.
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
        // bytes — Slovak-specific scalars are multi-byte in UTF-8 and
        // byte arithmetic would silently corrupt boundaries).
        let mut chars: Vec<char> = word.chars().flat_map(char::to_lowercase).collect();

        // Words of length 0..=2 (after the fold) stem to themselves;
        // stripping a suffix from a 2-char word would leave a
        // 1-or-0-char stem, which is never useful for IR purposes.
        if chars.len() > 2 {
            let rv = compute_rv(&chars);

            // 2. Main suffix pass — globally longest match across
            // the unified noun / adjective / verb inflection table.
            try_main_suffix(&mut chars, rv);
        }

        let out: String = chars.iter().collect();
        if out == word {
            Cow::Borrowed(word)
        } else {
            Cow::Owned(out)
        }
    }
}

impl Stemmer for SlovakStemmer {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        SlovakStemmer::stem(self, word)
    }
}

// ---------------------------------------------------------------------------
// Vowel classification.
// ---------------------------------------------------------------------------

/// The Slovak vowel set for the RV region calculation. Includes short
/// vowels (`a e i o u y`), long vowels (`á é í ó ú ý`), the
/// Slovak-specific open-e (`ä`) and o-circumflex diphthong marker
/// (`ô`), and the syllabic long consonants (`ĺ`, `ŕ`) that carry a
/// syllable nucleus and are treated as vowels in Slovak
/// vowel-consonant alternation rules.
#[inline]
const fn is_vowel(c: char) -> bool {
    matches!(
        c,
        'a' | 'e'
            | 'i'
            | 'o'
            | 'u'
            | 'y'
            | 'á'
            | 'é'
            | 'í'
            | 'ó'
            | 'ú'
            | 'ý'
            | 'ä'
            | 'ô'
            | 'ĺ'
            | 'ŕ'
    )
}

// ---------------------------------------------------------------------------
// Region RV — computed as a char index.
// ---------------------------------------------------------------------------

/// RV = the position after the first vowel-followed-by-consonant pair
/// (the Snowball-family standard), adjusted so RV ≥ 2 (a minimum
/// preserved-stem-length of 2 characters).
///
/// If the word contains no vowel-then-consonant transition, RV is the
/// end of the word (i.e. the null suffix — no rule will fire because
/// no suffix can be entirely to the right of the word's end).
fn compute_rv(chars: &[char]) -> usize {
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
    // We are now on the first post-vowel consonant; RV starts *after* it.
    let rv = if i < n { i + 1 } else { n };
    rv.max(2.min(n))
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
// Main suffix table — unified noun / adjective / verb endings.
// ---------------------------------------------------------------------------

/// The unified Slovak inflectional-suffix table.
///
/// Draws from the noun, adjective, possessive, and verb paradigms.
/// The stemmer takes the globally longest match; when two suffixes
/// tie on length, the earlier one in this list wins.
///
/// The table is deliberately conservative — every entry is a
/// well-attested Slovak inflectional suffix. Derivational suffixes
/// (`-osť`, `-stvo`, `-izmus`) are deliberately excluded; they would
/// belong in a separate "aggressive" variant.
const MAIN_SUFFIXES: &[&[char]] = &[
    // ---- 5-character shapes ----
    // Verb -ovať past-tense feminine / neuter / plural.
    // (Slovak, unlike Czech, has no gender split in past-tense plural
    // — one -ovali covers all genders.)
    &['o', 'v', 'a', 'l', 'a'],
    &['o', 'v', 'a', 'l', 'o'],
    &['o', 'v', 'a', 'l', 'i'],
    // Verb -ovať present 1pl / 2pl (Slovak paradigm).
    &['u', 'j', 'e', 'm', 'e'],
    &['u', 'j', 'e', 't', 'e'],
    // ---- 4-character shapes ----
    // Verb -ovať past-tense masc. sg. and infinitive.
    &['o', 'v', 'a', 'l'],
    &['o', 'v', 'a', 'ť'],
    // Verb -ovať present 1sg / 2sg (Slovak-specific: -ujem, not
    // Czech's -uji).
    &['u', 'j', 'e', 'm'],
    &['u', 'j', 'e', 'š'],
    // ---- 3-character shapes ----
    // Adjective instrumental plural.
    &['ý', 'm', 'i'], // peknými
    // Possessive-adjective long forms.
    &['o', 'v', 'i'], // Petrovi (dat. sg. / anim. pl.)
    &['o', 'v', 'a'], // Petrova (nom. fem.)
    &['o', 'v', 'o'], // Petrovo (nom. neut.)
    &['o', 'v', 'e'], // Petrove (nom. inan. pl.)
    &['o', 'v', 'u'], // Petrovu (acc. sg. fem.)
    // Noun instrumental plural.
    &['a', 'm', 'i'], // ženami
    &['e', 'm', 'i'], // (poetic / rare)
    // Noun locative plural.
    &['á', 'c', 'h'], // ženách
    // Adjective long forms.
    &['é', 'h', 'o'], // pekného
    &['é', 'm', 'u'], // peknému
    &['ý', 'c', 'h'], // pekných
    // Verb present tense 3sg / 3pl of -ovať verbs.
    &['u', 'j', 'e'], // pracuje
    &['u', 'j', 'ú'], // pracujú
    // -núť infinitive (napadnúť) and Slovak-specific -ieť infinitive
    // (vidieť) — a Slovak / Czech divergence: Czech has vidět with ě.
    &['n', 'ú', 'ť'], // napadnúť
    &['i', 'e', 'ť'], // vidieť
    &['n', 'u', 'l'], // napadnul (past sg.)
    // ---- 2-character shapes ----
    &['á', 'm'], // ženám (dat. pl.)
    &['ý', 'm'], // pekným (instr. sg.)
    &['o', 'm'], // pánom (Slovak instr. sg.; Czech has -em)
    &['e', 'm'], // srdcom paradigm — some Slovak nouns still use -em
    &['o', 'u'], // ženou (instr. sg. fem.)
    &['a', 'ť'], // robať? No — the infinitive class -ať (robievať)
    &['i', 'ť'], // robiť (infinitive)
    &['a', 'l'], // robieval (past sg.)
    &['i', 'l'], // robil (past sg.)
    &['e', 'l'], // videl (past sg. — Slovak also has videl / videla)
    // ---- 1-character shapes (bare vowels) ----
    // Bare noun/adjective endings — the last-resort strips.
    &['a'],
    &['e'],
    &['i'],
    &['o'],
    &['u'],
    &['y'],
    &['á'],
    &['é'],
    &['í'],
    &['ý'],
];

/// Try to strip the globally longest main-table suffix in RV. Returns
/// `true` if fired.
fn try_main_suffix(chars: &mut Vec<char>, rv: usize) -> bool {
    if let Some(s) = longest_suffix_in(chars, MAIN_SUFFIXES, rv) {
        strip(chars, s);
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &str) -> String {
        SlovakStemmer.stem(w).into_owned()
    }

    #[test]
    fn empty_and_short_are_unchanged() {
        assert_eq!(s(""), "");
        assert_eq!(s("a"), "a");
        assert_eq!(s("ja"), "ja");
    }

    #[test]
    fn adjective_masc_singular_bare_y() {
        // "pekný" — 5 chars: p,e,k,n,ý. RV: p(cons), e(vow), k(cons)
        //   → RV = 3. Bare -ý (1 char) at position 4. 4 ≥ 3. Strip
        //   → "pekn".
        assert_eq!(s("pekný"), "pekn");
    }

    #[test]
    fn adjective_fem_singular_bare_a() {
        // "pekná" → bare -á → "pekn".
        assert_eq!(s("pekná"), "pekn");
    }

    #[test]
    fn adjective_gen_singular_eho() {
        // "pekného" → -ého (3 chars) → "pekn".
        assert_eq!(s("pekného"), "pekn");
    }

    #[test]
    fn adjective_dat_singular_emu() {
        assert_eq!(s("peknému"), "pekn");
    }

    #[test]
    fn adjective_gen_plural_ych() {
        assert_eq!(s("pekných"), "pekn");
    }

    #[test]
    fn adjective_instr_plural_ymi() {
        // "peknými" → -ými (3 chars) → "pekn".
        assert_eq!(s("peknými"), "pekn");
    }

    #[test]
    fn noun_dat_plural_am() {
        // "ženám" → -ám → "žen".
        //   ženám = ž,e,n,á,m (5 chars). RV: ž(cons), e(vow), n(cons)
        //   → RV = 3. -ám at position 3. 3 ≥ 3. Strip → "žen".
        assert_eq!(s("ženám"), "žen");
    }

    #[test]
    fn noun_instr_plural_ami() {
        assert_eq!(s("ženami"), "žen");
    }

    #[test]
    fn noun_loc_plural_ach() {
        assert_eq!(s("ženách"), "žen");
    }

    #[test]
    fn noun_instr_singular_ou() {
        // "ženou" → -ou (2 chars) → "žen".
        assert_eq!(s("ženou"), "žen");
    }

    #[test]
    fn noun_instr_singular_om() {
        // "pánom" → -om (2 chars) → "pán".
        //   pánom = p,á,n,o,m (5 chars). RV: p(cons), á(vow), n(cons)
        //   → RV = 3. -om at position 3. 3 ≥ 3. Strip → "pán".
        assert_eq!(s("pánom"), "pán");
    }

    #[test]
    fn verb_past_ovat_family_masc_sg() {
        // "pracoval" → -oval (4 chars) → "prac".
        assert_eq!(s("pracoval"), "prac");
    }

    #[test]
    fn verb_past_ovat_family_fem() {
        assert_eq!(s("pracovala"), "prac");
    }

    #[test]
    fn verb_past_ovat_family_neut() {
        assert_eq!(s("pracovalo"), "prac");
    }

    #[test]
    fn verb_past_ovat_family_plural() {
        // Slovak has one plural for all genders — no -ovaly.
        assert_eq!(s("pracovali"), "prac");
    }

    #[test]
    fn verb_infinitive_ovat_slovak_uses_t_with_hacek() {
        // "pracovať" (Slovak infinitive with -ť) → -ovať (4 chars)
        //   → "prac". This is the marquee Slovak/Czech divergence.
        assert_eq!(s("pracovať"), "prac");
    }

    #[test]
    fn verb_present_1sg_ujem() {
        // "pracujem" → -ujem (4 chars) → "prac". Slovak-specific:
        //   Czech has -uji.
        assert_eq!(s("pracujem"), "prac");
    }

    #[test]
    fn verb_present_2sg_ujes() {
        assert_eq!(s("pracuješ"), "prac");
    }

    #[test]
    fn verb_present_3sg_uje() {
        assert_eq!(s("pracuje"), "prac");
    }

    #[test]
    fn verb_present_1pl_ujeme() {
        assert_eq!(s("pracujeme"), "prac");
    }

    #[test]
    fn verb_present_2pl_ujete() {
        assert_eq!(s("pracujete"), "prac");
    }

    #[test]
    fn verb_present_3pl_uju() {
        assert_eq!(s("pracujú"), "prac");
    }

    #[test]
    fn verb_infinitive_it_uses_t_with_hacek() {
        // "robiť" (Slovak infinitive) → -iť (2 chars) → "rob".
        //   robiť = r,o,b,i,ť (5 chars). RV = 3. -iť at pos 3. 3 ≥ 3.
        //   Strip → "rob".
        assert_eq!(s("robiť"), "rob");
    }

    #[test]
    fn verb_infinitive_iet_slovak_specific() {
        // "vidieť" (Slovak — Czech has vidět with ě) → -ieť (3 chars)
        //   → "vid".
        assert_eq!(s("vidieť"), "vid");
    }

    #[test]
    fn verb_infinitive_nut() {
        // "napadnúť" → -núť (3 chars) → "napad".
        //   napadnúť = n,a,p,a,d,n,ú,ť (8 chars). RV: n(cons),
        //   a(vow), p(cons) → RV = 3. -núť at pos 5. 5 ≥ 3. Strip
        //   → "napad".
        assert_eq!(s("napadnúť"), "napad");
    }

    #[test]
    fn verb_past_it_family() {
        // "hovoril" → -il (2 chars) → "hovor".
        //   hovoril = h,o,v,o,r,i,l (7 chars). RV: h(cons), o(vow),
        //   v(cons) → RV = 3. -il at pos 5. 5 ≥ 3. Strip → "hovor".
        assert_eq!(s("hovoril"), "hovor");
    }

    #[test]
    fn possessive_ovi() {
        // "petrovi" → -ovi (3 chars) → "petr".
        assert_eq!(s("petrovi"), "petr");
    }

    #[test]
    fn possessive_ova() {
        assert_eq!(s("petrova"), "petr");
    }

    #[test]
    fn possessive_ovo() {
        assert_eq!(s("petrovo"), "petr");
    }

    #[test]
    fn possessive_ove() {
        assert_eq!(s("petrove"), "petr");
    }

    #[test]
    fn short_word_untouched() {
        // "on" (2 chars) — chars.len() > 2 predicate fails.
        assert_eq!(s("on"), "on");
        assert_eq!(s("je"), "je");
    }

    #[test]
    fn slovak_letters_preserved_in_stem() {
        // "žltý" (4 chars: ž,l,t,ý) — the only vowel is the final
        //   `ý`. RV: no vowel-then-consonant pair exists inside the
        //   word, so RV = end-of-word = 4. -ý at position 3. 3 < 4.
        //   RV guard blocks stripping. Untouched — demonstrates the
        //   Snowball-style RV floor keeps the sole vowel from being
        //   over-stripped.
        assert_eq!(s("žltý"), "žltý");
        // "chudý" (5 chars: c,h,u,d,ý) — RV: c(cons), h(cons),
        //   u(vow), d(cons) → RV = 4. -ý at pos 4. 4 ≥ 4. Strip
        //   → "chud". Slovak-specific `ý` folds through the case-
        //   fold and the bare-vowel suffix rule correctly.
        assert_eq!(s("chudý"), "chud");
        // "kôň" (3 chars) — RV = 3. No suffix matches (`-ň` is not
        //   in the table). Untouched — the Slovak `ô` and `ň` are
        //   preserved intact.
        assert_eq!(s("kôň"), "kôň");
        // "späť" (4 chars: s,p,ä,ť) — no matching suffix. Untouched
        //   — the Slovak `ä` and `ť` are preserved intact.
        assert_eq!(s("späť"), "späť");
    }

    #[test]
    fn syllabic_l_treated_as_vowel_in_rv() {
        // "stĺp" (4 chars: s,t,ĺ,p). RV: s(cons), t(cons), ĺ(vow),
        //   p(cons) → RV = 4. Length 4. No suffix fits with pos ≥ 4.
        //   Untouched.
        assert_eq!(s("stĺp"), "stĺp");
    }

    #[test]
    fn syllabic_r_treated_as_vowel_in_rv() {
        // "vŕba" (4 chars: v,ŕ,b,a). RV: v(cons), ŕ(vow), b(cons)
        //   → RV = 3. Bare -a (1 char) at pos 3. Strip → "vŕb".
        assert_eq!(s("vŕba"), "vŕb");
    }

    #[test]
    fn word_ending_in_consonant_untouched() {
        // "byt" (3 chars) — RV = 3. Ends in 't'. No matching suffix.
        assert_eq!(s("byt"), "byt");
    }

    #[test]
    fn rv_computation_examples() {
        // Verify a few RV computations by hand-tracing.
        let chars: Vec<char> = "pekný".chars().collect();
        assert_eq!(compute_rv(&chars), 3);
        let chars: Vec<char> = "ženám".chars().collect();
        assert_eq!(compute_rv(&chars), 3);
        let chars: Vec<char> = "pracoval".chars().collect();
        assert_eq!(compute_rv(&chars), 4);
        let chars: Vec<char> = "vŕba".chars().collect();
        assert_eq!(compute_rv(&chars), 3);
    }

    #[test]
    fn convergence_within_bounded_iterations() {
        // The stemmer isn't universally idempotent (e.g. -oval strips
        // to -prac, but a subsequent call on prac finds no matching
        // suffix and is idempotent). Verify convergence within a small
        // number of iterations on a representative vocabulary.
        for w in [
            "pekný",
            "pekná",
            "pekné",
            "pekného",
            "peknému",
            "pekných",
            "peknými",
            "ženám",
            "ženami",
            "ženou",
            "pánom",
            "pracoval",
            "pracovať",
            "pracovala",
            "pracujem",
            "pracuje",
            "pracujú",
            "pracuješ",
            "robiť",
            "hovoril",
            "vidieť",
            "napadnúť",
            "petrovi",
            "petrova",
        ] {
            let mut cur = SlovakStemmer.stem(w).into_owned();
            for _ in 0..5 {
                let next = SlovakStemmer.stem(&cur).into_owned();
                if next == cur {
                    break;
                }
                cur = next;
            }
            let n1 = SlovakStemmer.stem(&cur).into_owned();
            assert_eq!(cur, n1, "did not converge on {w:?}");
        }
    }
}
