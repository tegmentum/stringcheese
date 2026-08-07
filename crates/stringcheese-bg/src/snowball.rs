//! The Snowball Bulgarian stemmer.
//!
//! # Origin
//!
//! Based on Preslav Nakov's Bulgarian stemming algorithm (2003),
//! documented at
//! <https://snowballstem.org/algorithms/bulgarian/stemmer.html>. The
//! algorithm is the reference stemmer used across Bulgarian IR
//! pipelines and ships in the canonical Snowball distribution
//! alongside the Russian, Ukrainian-adjacent, and other Slavic
//! stemmers.
//!
//! # Algorithm sketch
//!
//! Bulgarian's morphology has one signature feature no other Slavic
//! language shares: the **postposed definite article**. Where Russian
//! and Ukrainian carry the definite meaning through context (Russian
//! has no article at all; Ukrainian uses word order and demonstratives),
//! Bulgarian attaches the article directly to the noun as a suffix.
//! `книга` means "book"; `книгата` means "the book" — literally
//! "book-the". This has downstream consequences for retrieval:
//! a stemmer that treats `книга` and `книгата` as different surface
//! forms will silently fail to link `book`-scoped documents that
//! happen to render the noun definite. The algorithm therefore
//! **strips the article first**, before any other suffix cascade.
//!
//! 1. **Preprocess.** Lowercase the input under Rust's default
//!    Unicode fold (Cyrillic has no Turkic-style tailoring; `А → а`,
//!    `Ъ → ъ`, `Я → я` all work under default rules — no `ё → е` fold
//!    the way Russian requires, since Bulgarian has no `ё`).
//! 2. **Region R1.** Compute R1 as the position after the first
//!    non-vowel following a vowel. Bulgarian's vowel set is
//!    `а ъ о у е и я ю` — note that `ъ` counts as a vowel in
//!    Bulgarian (it is the mid-central vowel /ɤ/), unlike Russian
//!    where `ъ` is a purely orthographic hard-sign glyph. The Snowball
//!    Bulgarian spec uses R1, not the RV used by the Russian
//!    algorithm; this crate follows the spec.
//! 3. **Step 1 — remove definite article.** Consult the article
//!    suffix table (longest match wins in R1): `-ият` / `-ия` (masc
//!    long-adjective definite), `-ият` (masc noun-adj), `-ата` /
//!    `-ото` (long-adjective definite fem / neut), `-ите` (plural
//!    definite), `-ът` / `-ят` (masc noun full definite), `-та` (fem
//!    noun definite; also neut plural after `-а`), `-то` (neut noun
//!    definite), `-те` (plural noun definite).
//! 4. **Step 2 — remove plural.** Peel plural markers: `-ове` /
//!    `-еве` (masc noun plurals of monosyllabic roots).
//! 5. **Step 3 — remove verb / participle endings.** A verb-inflection
//!    table in R1: imperfect and aorist past forms (`-вах`,
//!    `-вахме`, `-вахте`, `-ах`, `-ох`, `-ех`, `-яха`, `-аха`,
//!    `-оха`, `-еха`), present-tense forms (`-еш`, `-иш`, `-ете`,
//!    `-ите`, `-ем`, `-им`, `-ат`, `-ят`, `-еше`, `-иеше`, `-яше`,
//!    `-ашe`, `-иеше`), l-participles (`-ъл`, `-ла`, `-ло`, `-ли`,
//!    `-л`).
//! 6. **Step 4 — final vowel.** If the word ends with a bare
//!    Bulgarian vowel (`-а`, `-е`, `-и`, `-о`, `-у`, `-я`, `-ю`,
//!    `-ъ`) inside R1, delete it. This is Bulgarian's equivalent of
//!    Russian's step-2 trailing-`и` rule, but generalized to every
//!    bare-vowel case marker (Bulgarian has no nominal case system —
//!    the bare vowels here are old case markers preserved on
//!    high-frequency nouns).
//!
//! # Byte-vs-char safety
//!
//! Every Cyrillic scalar in the modern Bulgarian block is **2 bytes**
//! in UTF-8 (U+0410..=U+044F falls in the 2-byte range). All the
//! arithmetic in this module operates on `Vec<char>` indices — never
//! raw byte offsets — so a suffix `['и', 'т', 'е']` of char-length 3
//! is 6 bytes of Cyrillic UTF-8 but only 3 slots of a `Vec<char>`.
//! The region calculation returns a char-index; the ends-with /
//! truncate helpers accept char-slices. There is no path through this
//! module that crosses a scalar boundary at the byte level.
//!
//! # Palatal alternation — a deliberate non-goal
//!
//! Bulgarian has productive palatal alternations at the k/c, g/z, x/s
//! boundary (`рък-а` / `ръц-е` — "hand" / "hands"; `крак` / `крака` /
//! `крака-та`; `сняг` / `снегът`), and vowel alternations under stress
//! (`голям` / `големи` — "big" masc / "big" pl). These alternations
//! are context-sensitive and hard to reverse without a lexicon:
//! collapsing `ц → к` unconditionally would over-restore for words
//! like `лице` ("face"). The shipped stemmer therefore **does not
//! reverse palatal alternations**; a caller who needs `рък` and `ръц`
//! collapsed to one form needs a dictionary-based lemmatizer. The
//! algorithm's suffix strips are still sound in the presence of the
//! alternations — the alternations just remain visible in the output.
//!
//! # Non-goals
//!
//! * **Full-corpus cross-verification.** The Snowball project's
//!   `voc.txt` / `output.txt` files for Bulgarian are not embedded
//!   here; the `tests/snowball_reference.rs` test carries a hand-
//!   traced subset that exercises each step and the definite-article
//!   pass in particular. Full-corpus cross-verification is a
//!   follow-up.
//! * **Lemmatization.** Reducing `по-хубав → хубав`, `най-голям →
//!   голям` needs a comparative-prefix stripper; that is deferred.
//! * **Aspect-prefix stripping.** Bulgarian's perfective/imperfective
//!   aspect prefixes (`про-`, `на-`, `из-`, `от-`, …) are
//!   content-carrying and are not stripped by this stemmer.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use stringcheese_lang::Stemmer;

/// The Snowball Bulgarian stemmer.
///
/// A zero-sized unit value; construct as [`BulgarianSnowball`] and
/// reuse the value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_bg::BulgarianSnowball;
/// use stringcheese_lang::Stemmer;
///
/// // Definite-article stripping — Bulgarian's signature feature.
/// assert_eq!(BulgarianSnowball.stem("книгата"), "книг");
/// assert_eq!(BulgarianSnowball.stem("човекът"), "човек");
/// assert_eq!(BulgarianSnowball.stem("столовете"), "стол");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct BulgarianSnowball;

impl BulgarianSnowball {
    /// Stems `word` per the Snowball Bulgarian algorithm.
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
            // first because Bulgarian's article is a noun-suffix, and
            // downstream suffix passes would misinterpret an
            // articled form's ending otherwise.
            remove_article(&mut chars, r1);

            // 4. Step 2 — remove plural.
            remove_plural(&mut chars, r1);

            // 5. Step 3 — remove verb / participle endings.
            remove_verb(&mut chars, r1);

            // 6. Step 4 — final bare vowel.
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

impl Stemmer for BulgarianSnowball {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        BulgarianSnowball::stem(self, word)
    }
}

// ---------------------------------------------------------------------------
// Vowel classification.
// ---------------------------------------------------------------------------

/// The Bulgarian vowel set for the R1 region calculation:
/// `а ъ о у е и я ю`. Note that `ъ` is a full vowel in Bulgarian
/// (the mid-central /ɤ/), unlike Russian where it is the hard sign.
#[inline]
const fn is_vowel(c: char) -> bool {
    matches!(c, 'а' | 'ъ' | 'о' | 'у' | 'е' | 'и' | 'я' | 'ю')
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

/// The Bulgarian definite-article suffix table.
///
/// Bulgarian's article agrees with the noun's (or adjective's) gender
/// and number and attaches as a suffix to the last stressable word in
/// the noun phrase. The forms shipped here:
///
/// * `-ият` / `-ия` — masculine long-adjective / adjectival-noun
///   definite (`красивият човек` — "the beautiful man"; `хубавия
///   ден` — "the nice day", accusative).
/// * `-ъят` — variant masculine long-form.
/// * `-ата` / `-ото` — long-adjective definite feminine / neuter
///   (`новата книга` — "the new book"; `новото дете` — "the new
///   child").
/// * `-ите` — long-adjective / plural-noun definite plural (`новите
///   книги` — "the new books").
/// * `-ът` / `-ят` — masculine noun full definite subject-form
///   (`градът` — "the city"; `учителят` — "the teacher").
/// * `-та` — feminine noun definite (`книгата` — "the book"); also
///   the neuter plural article on nouns whose plural ends in `-а`
///   (`имената` — "the names").
/// * `-то` — neuter noun definite (`детето` — "the child").
/// * `-те` — plural noun definite (`столовете` — "the tables").
///
/// The longest match wins. `-ата` (3 chars) is preferred over `-та`
/// (2 chars) on `красивата`, `-ото` over `-то` on `хубавото`, `-ите`
/// over `-те` on `новите`, etc. — the same longest-match rule the
/// other Slavic packs use.
const ARTICLE_SUFFIXES: &[&[char]] = &[
    // ---- 3-character shapes (longest first). ----
    &['и', 'я', 'т'], // -ият: masc long-adjective definite subject.
    &['а', 'т', 'а'], // -ата: fem long-adjective definite.
    &['о', 'т', 'о'], // -ото: neut long-adjective definite.
    &['и', 'т', 'е'], // -ите: plural definite (long-adj / plural noun).
    // ---- 2-character shapes. ----
    &['и', 'я'], // -ия: masc long-adjective definite object.
    &['ъ', 'т'], // -ът: masc noun full definite.
    &['я', 'т'], // -ят: masc soft-noun full definite.
    &['т', 'а'], // -та: fem noun definite (also neut plural in -а).
    &['т', 'о'], // -то: neut noun definite.
    &['т', 'е'], // -те: plural noun definite.
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

/// The Bulgarian plural-suffix table.
///
/// * `-ове` — masculine noun plural of monosyllabic roots
///   (`стол` → `столове` — "table" / "tables").
/// * `-еве` — variant masculine plural
///   (`бой` → `боеве` — "fight" / "fights").
///
/// The bare `-и`, `-а`, `-е` plurals are handled by the step-4
/// final-vowel step; listing them here would over-strip
/// pronouns and short adverbs.
const PLURAL_SUFFIXES: &[&[char]] = &[
    &['о', 'в', 'е'], // -ове.
    &['е', 'в', 'е'], // -еве.
];

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

/// The Bulgarian verb-inflection table.
///
/// Covers the aorist, imperfect, present, and l-participle paradigms.
/// Where a form's ending is a proper suffix of a longer form's ending
/// (`-ах` / `-ахме` / `-ахте`), the longest match wins under the
/// longest-suffix rule.
///
/// Groups:
///
/// * **Aorist / imperfect past** — `-вах` / `-вахме` / `-вахте`,
///   `-ах` / `-ахме` / `-ахте`, `-ох` / `-охме` / `-охте`,
///   `-ех` / `-ехме` / `-ехте`, plus 3sg imperfect
///   `-аше` / `-еше` / `-иеше` / `-яше` and 3pl
///   `-аха` / `-еха` / `-оха` / `-яха`.
/// * **Present tense** — 2sg `-еш` / `-иш`, 1pl `-ем` / `-им`,
///   2pl `-ете` / `-ите` (the plural definite-article form is
///   identical; article step runs first), 3pl `-ат` / `-ят`.
/// * **l-participle** — `-л` (masc), `-ла` (fem), `-ло` (neut),
///   `-ли` (pl), `-ъл` (masc after a consonant cluster).
const VERB_SUFFIXES: &[&[char]] = &[
    // ---- 5-character shapes. ----
    &['в', 'а', 'х', 'м', 'е'],
    &['в', 'а', 'х', 'т', 'е'],
    // ---- 4-character shapes. ----
    &['а', 'х', 'м', 'е'],
    &['а', 'х', 'т', 'е'],
    &['о', 'х', 'м', 'е'],
    &['о', 'х', 'т', 'е'],
    &['е', 'х', 'м', 'е'],
    &['е', 'х', 'т', 'е'],
    &['я', 'х', 'м', 'е'],
    &['я', 'х', 'т', 'е'],
    // ---- 3-character shapes. ----
    &['в', 'а', 'х'],
    &['а', 'ш', 'е'],
    &['е', 'ш', 'е'],
    &['и', 'ш', 'е'],
    &['я', 'ш', 'е'],
    &['а', 'х', 'а'],
    &['е', 'х', 'а'],
    &['о', 'х', 'а'],
    &['я', 'х', 'а'],
    &['е', 'т', 'е'],
    &['и', 'т', 'е'],
    // ---- 2-character shapes. ----
    &['е', 'ш'],
    &['и', 'ш'],
    &['е', 'м'],
    &['и', 'м'],
    &['а', 'х'],
    &['о', 'х'],
    &['е', 'х'],
    &['я', 'х'],
    &['а', 'т'],
    &['я', 'т'],
    &['ъ', 'л'], // l-participle masc after cluster.
    &['л', 'а'], // l-participle fem.
    &['л', 'о'], // l-participle neut.
    &['л', 'и'], // l-participle plural.
                 // NOTE: the masc-sg l-participle in bare `-л` is deliberately
                 // absent from this table. Bulgarian is dense with high-frequency
                 // nouns ending in `-л` after a vowel — `учител` (teacher), `ъгъл`
                 // (angle), `игъл` (needle-gen) — that would over-strip if the
                 // bare `-л` fired. The `-ла` / `-ло` / `-ли` / `-ъл` entries above
                 // cover fem / neut / pl and the consonant-cluster masc variants
                 // (`казъл`-style forms), while `казал` and other bare-vowel-
                 // ending masc l-participles are left intact. A dictionary-based
                 // lemmatizer is the right tool if the caller needs to recover
                 // the l-participle infinitive across genders and numbers.
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

/// The Bulgarian bare-vowel endings the step-4 pass strips in R1.
///
/// These are the surface reflexes of Bulgarian's lost nominal case
/// system (Bulgarian is the only analytic Slavic language — it
/// dropped case declension entirely) — the bare vowels that remain
/// on high-frequency nouns and adjectives (`книга` — the fem nom;
/// `дете` — the neut nom; `голям` / `голяма` / `големи` — masc / fem
/// / pl of "big"). Bulgarian's vowel inventory is
/// `а ъ о у е и я ю`; every one of them can serve as a case-marker
/// tail.
const FINAL_VOWELS: &[&[char]] = &[
    &['а'],
    &['е'],
    &['и'],
    &['о'],
    &['у'],
    &['я'],
    &['ю'],
    &['ъ'],
];

/// If the word ends with a bare Bulgarian vowel inside R1, delete it.
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
        BulgarianSnowball.stem(w).into_owned()
    }

    #[test]
    fn empty_and_single_char_are_unchanged() {
        assert_eq!(s(""), "");
        assert_eq!(s("я"), "я");
    }

    #[test]
    fn definite_article_fem() {
        // The signature Bulgarian move: strip the fem definite article.
        // `книгата` = `книга` (book) + `-та` (the); the algorithm
        // strips the longer `-ата` in one shot (3-char match beats
        // 2-char `-та`) and leaves the pure root `книг`.
        assert_eq!(s("книгата"), "книг");
    }

    #[test]
    fn definite_article_masc_full() {
        // `човекът` = `човек` + `-ът` → `човек`.
        assert_eq!(s("човекът"), "човек");
    }

    #[test]
    fn definite_article_masc_soft() {
        // `учителят` = `учител` + `-ят` → `учител`.
        assert_eq!(s("учителят"), "учител");
    }

    #[test]
    fn definite_article_neut() {
        // `детето` = `дете` + `-то` → after article strip, step 4
        // trims the trailing `-е`, leaving `дет`.
        assert_eq!(s("детето"), "дет");
    }

    #[test]
    fn definite_article_plural() {
        // `столовете` = `стол` + `-ове` (plural) + `-те` (article).
        // Article step strips `-те` → `столове`. Plural step strips
        // `-ове` → `стол`.
        assert_eq!(s("столовете"), "стол");
    }

    #[test]
    fn definite_article_long_adj_fem() {
        // `красивата` = `красив` + `-ата` (long-adj fem definite).
        // The 3-char `-ата` beats the 2-char `-та`.
        assert_eq!(s("красивата"), "красив");
    }

    #[test]
    fn definite_article_long_adj_neut() {
        // `красивото` = `красив` + `-ото` → `красив`.
        assert_eq!(s("красивото"), "красив");
    }

    #[test]
    fn definite_article_long_adj_plural() {
        // `новите` = `нов` + `-ите` → `нов`.
        assert_eq!(s("новите"), "нов");
    }

    #[test]
    fn book_and_the_book_stem_identically() {
        // The whole point of the article step: `книга` and `книгата`
        // collapse to the same stem so an IR pipeline recognizes them
        // as the same lexeme.
        assert_eq!(s("книга"), s("книгата"));
    }

    #[test]
    fn plural_ove() {
        // `столове` = plural of `стол`. Plural step strips `-ове`.
        assert_eq!(s("столове"), "стол");
    }

    #[test]
    fn verb_present_1pl() {
        // `правим` = 1pl present. Verb step strips `-им`.
        assert_eq!(s("правим"), "прав");
    }

    #[test]
    fn verb_present_3pl() {
        // `правят` = 3pl present. Verb step strips `-ят`.
        assert_eq!(s("правят"), "прав");
    }

    #[test]
    fn verb_past_l_participle_fem() {
        // `казала` = l-participle fem. Verb step strips `-ла`, then
        // step 4 strips the surviving `-а`. (The bare masc `-л` is
        // deliberately absent from the verb table to avoid over-
        // stripping nouns like `учител` — see the VERB_SUFFIXES
        // docstring.)
        assert_eq!(s("казала"), "каз");
    }

    #[test]
    fn short_word_is_preserved() {
        // `стол` (table, singular) — R1 = 3, no suffix eligible.
        assert_eq!(s("стол"), "стол");
        // `нас` (us, pronoun) — no vowel-ending, no suffix.
        assert_eq!(s("нас"), "нас");
    }

    #[test]
    fn bare_vowel_final_strips_in_r1() {
        // `книга` — R1 = 4, `-а` at pos 4 fires. Stem `книг`.
        assert_eq!(s("книга"), "книг");
    }

    #[test]
    fn er_goljam_vowel_is_a_vowel() {
        // `път` = ['п', 'ъ', 'т']. `ъ` counts as a vowel, so R1 =
        // position after `т`, i.e. R1 = 3 (end). No suffix eligible.
        // Word stays as-is.
        assert_eq!(s("път"), "път");
        // `България` = ['б','ъ','л','г','а','р','и','я']. R1 = 3.
        // The suffix `-ия` is in the article table (masc long-adj
        // definite object form); it fires here as a false-positive
        // article match on a proper noun, stripping to `българ`.
        // Without lexical knowledge, the dictionary-free stemmer
        // cannot distinguish an articled noun from a country name
        // ending in the same string. Callers who need name-aware
        // stemming should combine this pack with a named-entity
        // filter.
        assert_eq!(s("България"), "българ");
    }
}
