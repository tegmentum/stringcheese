//! [`GeorgianStemmer`] — a longest-match suffix stripper for Modern
//! Georgian.
//!
//! # Georgian morphology
//!
//! Georgian is **agglutinative-fusional**. Nominal morphology packs
//! seven grammatical cases onto every noun / adjective / pronoun —
//! nominative, dative-accusative, ergative, genitive, instrumental,
//! adverbial, and vocative — plus a plural marker (contemporary
//! `-ები`, archaic `-ნი` / `-თა`), plus **agglutinated postpositions**
//! (`-ში` "in", `-ზე` "on", `-თან` "at", `-გან` "from", `-კენ`
//! "toward") that attach after the case marker. Verb morphology is
//! polypersonal (subject *and* object marked in the same word), with
//! prefixes for person, preverbs for direction / aspect, tense
//! markers, and personal endings.
//!
//! A full-parity Georgian lemmatizer would need a morphological
//! analyzer with a lexicon — nothing that fits inside a wasm-first /
//! offline-first language pack. The shipped stemmer is a **suffix
//! stripper** in the same shape as `stringcheese-bn` (Bengali) and
//! `stringcheese-cs` (Czech): a longest-match table over the closed
//! set of case endings, plural markers, agglutinated postpositions,
//! and the highest-frequency verb personal / tense endings, guarded
//! by a 2-scalar minimum stem length.
//!
//! # Suffix inventory (longest-first)
//!
//! The table below is stored in the `SUFFIXES` constant. All strings
//! are shown as their Mkhedruli surface form; internally the table is
//! stored as `&'static [char]` slices so scalar-indexed arithmetic
//! never crosses a byte boundary.
//!
//! ## Plural + case + postposition compounds (5 scalars)
//! - `-ებთან` plural + `-თან` "at"
//! - `-ებგან` plural + `-გან` "from"
//! - `-ებკენ` plural + `-კენ` "toward"
//!
//! ## Plural + case / postposition (4 scalars)
//! - `-ებით` plural + instrumental
//! - `-ებში` plural + "in"
//! - `-ებზე` plural + "on"
//! - `-ებმა` plural + ergative
//! - `-ების` plural + genitive
//! - `-ებად` plural + adverbial
//! - `-ისკენ` genitive + "toward"
//! - `-სთვის` dative + "for"
//!
//! ## Case / plural / postposition / verb (3 scalars)
//! - `-ები` plural nominative (contemporary)
//! - `-ებს` plural dative
//! - `-თან` "at"
//! - `-გან` "from"
//! - `-კენ` "toward"
//! - `-ვდი` verb past-continuous 1sg
//! - `-იან` verb 3pl future / present
//! - `-ავს` verb 3sg present
//!
//! ## Case / postposition / verb (2 scalars)
//! - `-ის` genitive
//! - `-ით` instrumental
//! - `-ად` adverbial
//! - `-მა` ergative
//! - `-ში` "in"
//! - `-ზე` "on"
//! - `-ნი` archaic plural
//! - `-თა` archaic plural / archaic genitive-plural
//! - `-დი` verb past 2sg
//!
//! ## Bare 1-scalar case (fired only if no longer suffix matches)
//! - `-ი` nominative
//! - `-ს` dative-accusative
//!
//! Longest-match wins so `-ებთან` beats `-თან`, `-ებით` beats `-ით`,
//! etc. Every strip is guarded by a **2-scalar minimum stem length**:
//! a strip that would leave fewer than 2 characters is skipped. This
//! is what stops single-word items like `მე` "I" or `ის` "he/she"
//! from being over-stripped (they never fire in practice because they
//! are entries of the stopword list, but the guard is defense in
//! depth).
//!
//! # Byte-vs-char safety
//!
//! Every Mkhedruli scalar (U+10D0..=U+10FF) is **3 bytes** in UTF-8
//! (the block falls in UTF-8's 3-byte range U+0800..=U+FFFF). All the
//! arithmetic in this module operates on `Vec<char>` indices — never
//! raw byte offsets — so a suffix `['ე', 'ბ', 'თ', 'ა', 'ნ']` of
//! char-length 5 is 15 bytes of Mkhedruli UTF-8 but only 5 slots of
//! a `Vec<char>`. The ends-with / truncate helpers accept
//! char-slices. There is no path through this module that crosses a
//! scalar boundary at the byte level.
//!
//! # Known over-strips
//!
//! * **Archaic `-ნი` plural vs. stem-final `ნ`.** The archaic plural
//!   `-ნი` (2 chars) is longer than the bare nominative `-ი` (1
//!   char), so a modern noun whose stem ends in `-ნ` (e.g. `წიგნი`
//!   "book", stem `წიგნ`; `ცხენი` "horse", stem `ცხენ`) is stripped
//!   as if it were an archaic plural: `წიგნი → წიგ`, `ცხენი → ცხე`.
//!   The reverse trade-off — dropping `-ნი` from the table — would
//!   correctly handle the modern singular but miss the archaic
//!   plural entirely; without a lexicon we cannot tell the two
//!   apart. The shipped table keeps `-ნი` (per the pack's task
//!   spec) and accepts the modern over-strip.
//! * **Archaic `-თა` plural vs. stem-final `თ`.** Same trade-off
//!   applies to `-თა` and words like `მთა` "mountain" (stem `მთ` —
//!   the guard `2 < 2 + 2 = 4` skips the strip here, so this
//!   specific short word is safe; longer stems ending in `-თა`
//!   would over-strip).
//!
//! # Non-goals
//!
//! * **Lemmatization.** Reducing `ვწერ` "I write" and `ვწერდი` "I was
//!   writing" to a shared lemma needs a lexicon; the shipped stemmer
//!   only strips the personal ending.
//! * **Preverb / prefix stripping.** Georgian verbs take direction /
//!   aspect preverbs (`მო-`, `წა-`, `გა-`, `და-`, etc.); stripping
//!   these correctly needs context and is deferred.
//! * **Screeve / tense-alternation reversal.** Different tenses of the
//!   same verb take different vowel patterns in the stem (present
//!   `ვწერ` vs. aorist `დავწერე`); a real lemmatizer would fold these,
//!   but that needs lexical support.
//! * **Old Georgian.** Old Georgian inflection is richer (five cases
//!   in some paradigms, additional aorist forms). A future
//!   `stringcheese-oka` sibling could handle the Asomtavruli /
//!   Nuskhuri corpus with its own table.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use stringcheese_lang::Stemmer;

/// Minimum stem char-length: any strip that would leave fewer chars
/// is skipped.
const MIN_STEM_LEN: usize = 2;

/// The Georgian suffix table. Ordered longest-first so the
/// longest-match helper picks the widest matching suffix.
///
/// Every entry is stored as a `&'static [char]` — matched on
/// `Vec<char>` tails, never on raw bytes (every Mkhedruli scalar is
/// 3 UTF-8 bytes, and byte offsets would silently corrupt boundaries).
const SUFFIXES: &[&[char]] = &[
    // ---------------------------------------------------------------
    // 5-scalar plural + postposition compounds.
    // ---------------------------------------------------------------
    &['ე', 'ბ', 'თ', 'ა', 'ნ'], // -ებთან (plural + "at")
    &['ე', 'ბ', 'გ', 'ა', 'ნ'], // -ებგან (plural + "from")
    &['ე', 'ბ', 'კ', 'ე', 'ნ'], // -ებკენ (plural + "toward")
    // ---------------------------------------------------------------
    // 5-scalar "for" (genitive + postposition თვის).
    // ---------------------------------------------------------------
    &['ი', 'ს', 'თ', 'ვ', 'ი', 'ს'], // -ისთვის (6 chars)
    // ---------------------------------------------------------------
    // 4-scalar plural + case / postposition compounds.
    // ---------------------------------------------------------------
    &['ე', 'ბ', 'ი', 'თ'], // -ებით (plural + instrumental)
    &['ე', 'ბ', 'შ', 'ი'], // -ებში (plural + "in")
    &['ე', 'ბ', 'ზ', 'ე'], // -ებზე (plural + "on")
    &['ე', 'ბ', 'მ', 'ა'], // -ებმა (plural + ergative)
    &['ე', 'ბ', 'ი', 'ს'], // -ების (plural + genitive)
    &['ე', 'ბ', 'ა', 'დ'], // -ებად (plural + adverbial)
    // ---------------------------------------------------------------
    // 4-scalar dative + postposition "for" and genitive + "toward".
    // ---------------------------------------------------------------
    &['ს', 'თ', 'ვ', 'ი', 'ს'], // -სთვის (5 chars, dative + "for")
    &['ი', 'ს', 'კ', 'ე', 'ნ'], // -ისკენ (5 chars, gen + "toward")
    // ---------------------------------------------------------------
    // 4-scalar verb tense/personal endings.
    // ---------------------------------------------------------------
    &['ო', 'ბ', 'დ', 'ი'], // -ობდი (2sg past continuous)
    &['ე', 'ბ', 'დ', 'ი'], // -ებდი (2sg past continuous variant)
    // ---------------------------------------------------------------
    // 3-scalar plural / postposition / verb endings.
    // ---------------------------------------------------------------
    &['ე', 'ბ', 'ი'], // -ები (plural nominative, contemporary)
    &['ე', 'ბ', 'ს'], // -ებს (plural dative)
    &['თ', 'ა', 'ნ'], // -თან ("at")
    &['გ', 'ა', 'ნ'], // -გან ("from")
    &['კ', 'ე', 'ნ'], // -კენ ("toward")
    &['ვ', 'დ', 'ი'], // -ვდი (1sg past continuous)
    &['ი', 'ა', 'ნ'], // -იან (3pl future/present)
    &['ა', 'ვ', 'ს'], // -ავს (3sg present)
    // ---------------------------------------------------------------
    // 2-scalar case / postposition / plural / verb endings.
    // ---------------------------------------------------------------
    &['ი', 'ს'], // -ის (genitive)
    &['ი', 'თ'], // -ით (instrumental)
    &['ა', 'დ'], // -ად (adverbial)
    &['მ', 'ა'], // -მა (ergative)
    &['შ', 'ი'], // -ში ("in")
    &['ზ', 'ე'], // -ზე ("on")
    &['ნ', 'ი'], // -ნი (archaic plural)
    &['თ', 'ა'], // -თა (archaic plural / archaic genitive-plural)
    &['დ', 'ი'], // -დი (2sg past)
    // ---------------------------------------------------------------
    // Bare 1-scalar case endings — fire only when no longer suffix
    // matched. Guarded by MIN_STEM_LEN like every other strip.
    // ---------------------------------------------------------------
    &['ი'], // -ი nominative
    &['ს'], // -ს dative
];

/// The Georgian stemmer.
///
/// A zero-sized unit value; construct as [`GeorgianStemmer`] and reuse
/// the value freely across threads and calls.
///
/// See the [module-level docs](self) for the rules and origin.
///
/// # Example
///
/// ```
/// use stringcheese_ka::GeorgianStemmer;
/// use stringcheese_lang::Stemmer;
///
/// // Nominative plural: -ები strips.
/// assert_eq!(GeorgianStemmer.stem("წიგნები"), "წიგნ");
/// // Genitive singular: -ის strips.
/// assert_eq!(GeorgianStemmer.stem("წიგნის"), "წიგნ");
/// // Postposition -ში strips.
/// assert_eq!(GeorgianStemmer.stem("სახლში"), "სახლ");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct GeorgianStemmer;

impl GeorgianStemmer {
    /// Stems `word` per the Georgian rule set.
    ///
    /// Returns the stem as a [`Cow`]. If no rule fires, the returned
    /// `Cow` borrows the input.
    #[must_use]
    pub fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        if word.is_empty() {
            return Cow::Borrowed(word);
        }

        // Fold Mtavruli (U+1C90..=U+1CBF, capitalized Mkhedruli
        // added in Unicode 11) to Mkhedruli via Rust's default
        // Unicode lowercase — the Unicode tables pair every Mtavruli
        // scalar with its Mkhedruli counterpart. This keeps
        // Mtavruli-cased inputs matching the Mkhedruli suffix table.
        let chars: Vec<char> = word.chars().flat_map(char::to_lowercase).collect();

        if chars.len() < MIN_STEM_LEN {
            // Under the length gate — return the folded form if it
            // differs from the input, otherwise borrow.
            let out: String = chars.iter().collect();
            return if out == word {
                Cow::Borrowed(word)
            } else {
                Cow::Owned(out)
            };
        }

        // Longest-match suffix strip.
        if let Some(suffix) = longest_matching_suffix(&chars) {
            let stem_len = chars.len() - suffix.len();
            let stem: String = chars[..stem_len].iter().collect();
            return Cow::Owned(stem);
        }

        let out: String = chars.iter().collect();
        if out == word {
            Cow::Borrowed(word)
        } else {
            Cow::Owned(out)
        }
    }
}

impl Stemmer for GeorgianStemmer {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        GeorgianStemmer::stem(self, word)
    }
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

/// Find the longest suffix from [`SUFFIXES`] that `chars` ends with,
/// leaving a stem of at least [`MIN_STEM_LEN`] characters after
/// stripping. Returns the matched slice (or `None`).
fn longest_matching_suffix(chars: &[char]) -> Option<&'static [char]> {
    let mut best: Option<&'static [char]> = None;
    for &s in SUFFIXES {
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
        GeorgianStemmer.stem(w).into_owned()
    }

    #[test]
    fn empty_and_short_words_are_unchanged() {
        assert_eq!(s(""), "");
        assert_eq!(s("ა"), "ა");
        assert_eq!(s("მე"), "მე"); // 2 chars: below the length gate for stripping.
    }

    #[test]
    fn nominative_singular_i_strips() {
        // ქართული "Georgian" → strip -ი (nominative) → ქართულ.
        assert_eq!(s("ქართული"), "ქართულ");
    }

    #[test]
    fn dative_s_strips() {
        // წიგნს "book (dat)" → strip -ს → წიგნ.
        assert_eq!(s("წიგნს"), "წიგნ");
    }

    #[test]
    fn ergative_ma_strips() {
        // კაცმა "man (erg)" → strip -მა → კაც.
        assert_eq!(s("კაცმა"), "კაც");
    }

    #[test]
    fn genitive_is_strips() {
        // წიგნის "book (gen)" → strip -ის → წიგნ.
        assert_eq!(s("წიგნის"), "წიგნ");
    }

    #[test]
    fn instrumental_it_strips() {
        // ხელით "hand (instr)" → strip -ით → ხელ.
        assert_eq!(s("ხელით"), "ხელ");
    }

    #[test]
    fn adverbial_ad_strips() {
        // მასწავლებლად "as teacher (adv)" → strip -ად → მასწავლებლ.
        assert_eq!(s("მასწავლებლად"), "მასწავლებლ");
    }

    #[test]
    fn plural_ebi_strips() {
        // წიგნები "books" → strip -ები → წიგნ.
        assert_eq!(s("წიგნები"), "წიგნ");
    }

    #[test]
    fn plural_dative_ebs_strips() {
        // წიგნებს "books (dat)" → strip -ებს → წიგნ.
        assert_eq!(s("წიგნებს"), "წიგნ");
    }

    #[test]
    fn plural_genitive_ebis_strips() {
        // წიგნების "books (gen)" → strip -ების → წიგნ.
        assert_eq!(s("წიგნების"), "წიგნ");
    }

    #[test]
    fn plural_instrumental_ebit_strips() {
        // strip -ებით → წიგნ.
        assert_eq!(s("წიგნებით"), "წიგნ");
    }

    #[test]
    fn postposition_shi_strips() {
        // სახლში "in the house" → strip -ში → სახლ.
        assert_eq!(s("სახლში"), "სახლ");
    }

    #[test]
    fn postposition_ze_strips() {
        // მაგიდაზე "on the table" → strip -ზე → მაგიდა.
        assert_eq!(s("მაგიდაზე"), "მაგიდა");
    }

    #[test]
    fn postposition_tan_strips() {
        // მასთან "at him" → strip -თან → მას.
        assert_eq!(s("მასთან"), "მას");
    }

    #[test]
    fn postposition_gan_strips() {
        // მისგან "from him" → strip -გან → მის.
        assert_eq!(s("მისგან"), "მის");
    }

    #[test]
    fn postposition_ken_strips() {
        // ქალაქისკენ "toward the city" → strip -ისკენ → ქალაქ.
        assert_eq!(s("ქალაქისკენ"), "ქალაქ");
    }

    #[test]
    fn plural_ebtan_strips_over_bare_tan() {
        // ბავშვებთან "with the children" → strip -ებთან (longer) → ბავშვ.
        assert_eq!(s("ბავშვებთან"), "ბავშვ");
    }

    #[test]
    fn verb_past_continuous_vdi_strips() {
        // ვწერდი has -დი; ვწერავდი (rare) would trigger -ვდი; use a
        // safer target: ვხატავდი "I was drawing" → strip -ვდი →
        // ვხატა. Note: verb prefixes (ვ-) are NOT stripped.
        assert_eq!(s("ვხატავდი"), "ვხატა");
    }

    #[test]
    fn verb_present_3sg_avs_strips() {
        // ხატავს "he draws" → strip -ავს → ხატ.
        assert_eq!(s("ხატავს"), "ხატ");
    }

    #[test]
    fn min_stem_guard_prevents_over_stripping() {
        // Very short function words like "ის" (2 chars) — the strip
        // of -ის would leave 0 chars; guarded.
        assert_eq!(s("ის"), "ის");
        // "მე" 2 chars → no strip (bare -ე isn't in the table anyway).
        assert_eq!(s("მე"), "მე");
        // "და" 2 chars → no strip.
        assert_eq!(s("და"), "და");
    }

    #[test]
    fn longer_suffix_beats_shorter_one() {
        // ისთვის (6 chars, "for") beats -ის (2 chars, gen). ნესვისთვის
        // "for the melon" → strip -ისთვის → ნესვ.
        assert_eq!(s("ნესვისთვის"), "ნესვ");
    }

    #[test]
    fn mtavruli_input_folds_to_mkhedruli_and_stems() {
        // Mtavruli input should case-fold to Mkhedruli via Unicode
        // default lowercase, then stem as normal.
        // ᲙᲐᲪᲘ (Mtavruli კაცი "man") → კაცი → strip -ი → კაც.
        // Note: `წიგნი` "book" would over-strip to `წიგ` because the
        // stem-final `ნ` collides with the archaic-plural `-ნი`
        // suffix — see the module docs for the trade-off. `კაცი`
        // avoids that ambiguity.
        let out = GeorgianStemmer.stem("ᲙᲐᲪᲘ").into_owned();
        assert_eq!(out, "კაც");
    }

    #[test]
    fn identity_on_bare_stems() {
        // A word that already ends outside the suffix table passes
        // through unchanged (identity, but folded to lowercase).
        assert_eq!(s("წიგნ"), "წიგნ");
    }

    #[test]
    fn borrowed_when_no_match_owned_when_matched() {
        let borrowed = GeorgianStemmer.stem("წიგნ");
        assert!(matches!(borrowed, Cow::Borrowed(_)));
        let owned = GeorgianStemmer.stem("წიგნები");
        assert!(matches!(owned, Cow::Owned(_)));
    }

    #[test]
    fn idempotent_on_reference_words() {
        // Words picked so that the stem itself does not end in another
        // suffix. `მასთან → მას` is *not* idempotent — `მას` would
        // then match the bare `-ს` dative — but running the stemmer
        // twice on the same *surface* word always returns the same
        // stem (that is the deterministic-fixed-point property; see
        // the property test).
        for w in ["წიგნები", "წიგნის", "სახლში", "ხატავს", "ვხატავდი"]
        {
            let once = GeorgianStemmer.stem(w).into_owned();
            let twice = GeorgianStemmer.stem(&once).into_owned();
            assert_eq!(once, twice, "stem not idempotent on {w:?}");
        }
    }
}
