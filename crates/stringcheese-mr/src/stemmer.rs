//! [`LightMarathiStemmer`] — a light Marathi suffix stripper.
//!
//! # Scope
//!
//! Marathi is an Indo-Aryan language with **agglutinative** case
//! marking, unlike Hindi's mostly analytic postposition-based system.
//! Marathi's case suffixes attach directly to the noun stem —
//! `घराला` "to the house" is one orthographic word (`घर` + `-ला`),
//! not two (compare Hindi `घर को`). Verbs conjugate for tense /
//! aspect / mood / person / gender, and Marathi additionally preserves
//! the **neuter gender** the OIA parent language had (Hindi has only
//! masculine and feminine), so verb and adjective agreement is
//! three-way.
//!
//! This means a rule-based light stemmer is genuinely useful for
//! Marathi in a way it isn't for Hindi — Hindi's postpositions are
//! separate tokens the tokenizer already splits off, so Hindi's
//! stemmer mostly targets gender / number markers on the noun itself,
//! whereas Marathi's stemmer targets the case suffixes that make
//! Marathi *look* like a much more heavily inflected language.
//!
//! There is **no canonical Snowball Marathi algorithm**. Snowball's
//! catalogue does not list Marathi. This module ships a
//! **deliberately conservative** rule-based stemmer covering the
//! agglutinative case markers, the two-form (nom/oblique) plural
//! markers, and the most-common verb personal endings. It is
//! intended to be a starter — a downstream `stringcheese-mr-morph`
//! crate would ship the full analyzer.
//!
//! # Suffix tables — two-phase stripping
//!
//! The stemmer walks two tables in sequence, both longest-match-wins
//! and both matched at the tail of the word's `Vec<char>` form:
//!
//! 1. **Primary table** — the case markers and verb personal endings.
//!    At most ONE primary suffix is stripped per external call. This
//!    single-strip guard is what stops the stemmer from mis-parsing
//!    a coincidental substring: after stripping `-ने` from `मुलाने`
//!    we reach `मुला`, whose tail `-ला` is *not* a second case
//!    suffix but the fossilized linking vowel + stem-final `-ल`.
//!    A general convergence loop would over-strip; the single-shot
//!    primary rule preserves the correct stem.
//! 2. **Cleanup table** — the bare linking-vowel matras
//!    (`-ा`/`-े`/`-ी`) and the oblique-plural markers (`-ां`/`-ीं`).
//!    After the primary strip fires (or immediately, if it didn't),
//!    the cleanup table iterates to convergence, peeling off the
//!    exposed linking vowel and, when the underlying form was a
//!    plural, its `-ां` / `-ीं` plural marker underneath. This lets
//!    `घरांना` (houses-to, oblique-plural + dative) resolve in one
//!    call: primary strip `-ना` → `घरां`, then cleanup strip `-ां`
//!    → `घर`.
//!
//! ## Primary table (case markers + verb personal endings + locative -त)
//!
//! | Suffix    | Chars | Class                                       |
//! |-----------|-------|---------------------------------------------|
//! | `मध्ये`   | 5     | locative postposition (inside / in)         |
//! | `च्या`    | 4     | genitive (f. pl. / oblique agreement)       |
//! | `ल्या`    | 4     | aorist 3pl-f                                |
//! | `हून`    | 3     | ablative (from)                             |
//! | `तात`    | 3     | present habitual 3pl                        |
//! | `तोस`    | 3     | present habitual 2sg-m                      |
//! | `तेस`    | 3     | present habitual 2sg-f                      |
//! | `ऊन`    | 2     | ablative (from — independent-vowel start)   |
//! | `ने`     | 2     | instrumental / ergative sg                  |
//! | `नी`     | 2     | instrumental / ergative pl                  |
//! | `ला`     | 2     | accusative / dative sg / aorist 3sg-m       |
//! | `ना`     | 2     | accusative / dative pl                      |
//! | `ली`     | 2     | aorist 3sg-f                                |
//! | `ले`     | 2     | aorist 3sg-n / 3pl-m                        |
//! | `चा`     | 2     | genitive m. sg.                             |
//! | `ची`     | 2     | genitive f. sg.                             |
//! | `चे`     | 2     | genitive n. sg. / m. pl.                    |
//! | `शी`     | 2     | sociative (with)                            |
//! | `सह`     | 2     | sociative (with — literary)                 |
//! | `णे`     | 2     | infinitive marker                           |
//! | `तो`     | 2     | present habitual 1sg-m / 3sg-m              |
//! | `ते`     | 2     | present habitual 1sg-f / 3sg-f / 3sg-n      |
//! | `त`      | 1     | locative (in / at)                          |
//!
//! ## Cleanup table (plural markers + linking-vowel matras)
//!
//! | Suffix    | Chars | Class                                       |
//! |-----------|-------|---------------------------------------------|
//! | `ां`     | 2     | general oblique plural marker (matra + ं)   |
//! | `ीं`     | 2     | neuter plural variant (matra + ं)           |
//! | `ा`      | 1     | m. sg. / oblique linking-vowel matra        |
//! | `े`       | 1     | m. pl. / n. sg. marker                      |
//! | `ी`      | 1     | f. sg. marker                               |
//!
//! The cleanup entries exist to finish the reduction after a
//! case-marker strip. Marathi noun stems surface with an **oblique
//! linking vowel** `-ा-` before an agglutinative case suffix
//! (`घर` → `घर-ा-ला` "to the house"). The stemmer strips the `-ला`
//! in the primary pass and then the exposed bare `-ा` in the cleanup
//! pass, so `घराला` reduces cleanly to `घर` within a single external
//! call. Keeping `-ला` in the primary table and `-ा` in the cleanup
//! table (rather than merging both into one convergence loop) also
//! stops the stemmer from mis-parsing the coincidental `-ला`
//! substring inside `मुला` as a second case marker.
//!
//! # What is deliberately **not** in the suffix table
//!
//! * **Independent postpositions (`साठी`, `बद्दल`, `बरोबर`,
//!   `पासून`, `पर्यंत`, `शिवाय`).** These are separate orthographic
//!   words in Marathi text; they're handled by the stopword list,
//!   not by the stemmer.
//! * **Standalone matras `ि` and `ु`.** These occur *inside* many
//!   Marathi words as ordinary dependent vowel signs; a trailing bare
//!   matra as an inflectional suffix would over-strip.
//! * **Aspectual / secondary verb endings.** The present-continuous
//!   auxiliary `आहे` "is" and the past-continuous auxiliary `होता`
//!   are handled as separate stopword tokens (they're written as free
//!   words after the participle, e.g. `करत आहे` "is doing").
//!
//! Each suffix is stored as a **[`Vec<char>`]** — no raw byte
//! offsets — and matched at the *scalar-slice tail* of the input's
//! `Vec<char>` form. Devanagari scalars are 3 UTF-8 bytes each, so
//! byte-level arithmetic would silently corrupt boundaries.
//!
//! # Over-stripping safeguard
//!
//! If a strip would leave fewer than 2 *characters* (scalars), the
//! stemmer discards the strip and returns the input unchanged. This
//! guards against pathological inputs like a two-scalar case suffix
//! standing alone.
//!
//! # Deliberate limitations (documented, not bugs)
//!
//! - **Independent postpositions written as separate words are not
//!   seen by this stemmer.** `घरा साठी` (for the house) is two
//!   whitespace-separated tokens; only `घरा` reaches the stemmer,
//!   and its `-ा` matra is what gets stripped.
//! - **No consonant-alternation reversal.** Marathi stems can surface
//!   with alternating final consonants across sandhi boundaries; this
//!   stemmer strips the suffix only.
//! - **No prefix stripping.** Marathi nominal prefixes are rare
//!   (mostly Sanskrit loans); leaving them intact keeps the stem
//!   recognizable.
//!
//! # Contract
//!
//! - **Deterministic.** For any input, two calls to `stem(w)` return
//!   equal outputs.
//! - **Idempotent.** Because the internal loop iterates to
//!   convergence, `stem(stem(w))` == `stem(w)` for every input.
//! - **Non-lengthening.** The output is never longer than the input
//!   (all rules are strict deletions).
//! - **Preserves input on no-match.** Returns [`Cow::Borrowed`] when
//!   no rule fires.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use stringcheese_lang::Stemmer;

/// Over-stripping guard: refuse to strip if the resulting stem would
/// have fewer than this many *characters* (scalars).
const MIN_STEM_CHARS: usize = 2;

/// Primary suffix table — case markers and verb personal endings.
/// At most one primary suffix is stripped per external call. Sorted
/// longest-first so the longest match wins on the initial scan.
///
/// Every entry is a scalar-sequence (`&'static [char]`) rather than a
/// byte string — the stemmer matches on `Vec<char>` tails, never raw
/// bytes.
const PRIMARY_SUFFIXES: &[&[char]] = &[
    // Five-scalar locative postposition (fused).
    &['म', 'ध', '\u{094D}', 'य', 'े'], // मध्ये — locative (in / inside)
    // Four-scalar case / verb markers with a virama.
    &['च', '\u{094D}', 'य', 'ा'], // च्या — genitive f. pl. / oblique agreement
    &['ल', '\u{094D}', 'य', 'ा'], // ल्या — aorist 3pl-f
    // Three-scalar case / verb markers.
    &['ह', 'ू', 'न'],  // हून — ablative (from)
    &['त', 'ा', 'त'], // तात — present habitual 3pl
    &['त', 'ो', 'स'], // तोस — present habitual 2sg-m
    &['त', 'े', 'स'],  // तेस — present habitual 2sg-f
    // Two-scalar case / verb markers.
    &['ऊ', 'न'], // ऊन — ablative (independent-vowel start)
    &['न', 'े'],  // ने — instrumental / ergative sg
    &['न', 'ी'], // नी — instrumental / ergative pl
    &['ल', 'ा'], // ला — accusative / dative sg / aorist 3sg-m
    &['न', 'ा'], // ना — accusative / dative pl
    &['ल', 'ी'], // ली — aorist 3sg-f
    &['ल', 'े'],  // ले — aorist 3sg-n / 3pl-m
    &['च', 'ा'], // चा — genitive m. sg.
    &['च', 'ी'], // ची — genitive f. sg.
    &['च', 'े'],  // चे — genitive n. sg. / m. pl.
    &['श', 'ी'], // शी — sociative (with)
    &['स', 'ह'], // सह — sociative (with — literary)
    &['ण', 'े'],  // णे — infinitive marker
    &['त', 'ो'], // तो — present habitual 1sg-m / 3sg-m
    &['त', 'े'],  // ते — present habitual 1sg-f / 3sg-f / 3sg-n
    // Single-scalar locative case marker.
    &['त'], // त — locative (in / at)
];

/// Cleanup suffix table — the oblique-plural markers and the bare
/// linking-vowel matras. Applied AFTER the primary strip (or
/// standalone, if the primary didn't fire) and iterated to
/// convergence.
///
/// Keeping these in a separate table is what stops the stemmer from
/// mis-parsing a substring like `-ला` inside `मुला` as a second
/// case marker: `-ला` lives in [`PRIMARY_SUFFIXES`] which fires at
/// most once per call, and the cleanup pass only touches bare
/// linking-vowel scalars and the two plural markers.
const CLEANUP_SUFFIXES: &[&[char]] = &[
    &['ा', '\u{0902}'], // ां — general oblique plural marker (matra + anusvara)
    &['ी', '\u{0902}'], // ीं — neuter plural variant (matra + anusvara)
    &['ा'],             // ा — m. sg. / oblique linking-vowel matra
    &['े'],              // े — m. pl. / n. sg. marker
    &['ी'],             // ी — f. sg. marker
];

/// Maximum cleanup iterations. In practice a Marathi surface word
/// carries at most one plural marker + one linking vowel below the
/// primary case suffix; a hard cap of 4 iterations is far more than
/// enough and defends against pathological inputs.
const MAX_CLEANUP_ITERATIONS: usize = 4;

/// The light Marathi suffix stripper.
///
/// A zero-sized value; construct as [`LightMarathiStemmer`] and reuse
/// across threads and calls.
///
/// See the [module-level docs](self) for the rule set and the
/// contract.
///
/// # Example
///
/// ```
/// use stringcheese_mr::LightMarathiStemmer;
/// use stringcheese_lang::Stemmer;
///
/// // Strip the dative suffix -ला (agglutinative in Marathi).
/// assert_eq!(LightMarathiStemmer.stem("घराला"), "घर");
/// // Bare noun — no suffix, no-op.
/// assert_eq!(LightMarathiStemmer.stem("पुस्तक"), "पुस्तक");
/// // Strip the plural + dative stack -ां + -ना, iterating to
/// // convergence.
/// assert_eq!(LightMarathiStemmer.stem("घरांना"), "घर");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct LightMarathiStemmer;

impl LightMarathiStemmer {
    /// Stems `word` per the light Marathi rule set.
    ///
    /// Returns the stem as a [`Cow`]. If no rule fires, the returned
    /// `Cow` borrows the input. The two-phase design — one primary
    /// case/verb strip followed by an iterated cleanup of linking
    /// vowels and plural markers — resolves stackable suffixes like
    /// `घरांना` (dative + oblique-plural) in a single external call
    /// without mis-parsing coincidental substrings like the `-ला` in
    /// `मुला`.
    #[must_use]
    pub fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        // Assemble a Vec<char> for scalar-indexed arithmetic. Every
        // Devanagari letter is 3 bytes in UTF-8, so any code that
        // mixed byte offsets with character-boundary logic would
        // corrupt boundaries — we work strictly in char space.
        let mut chars: Vec<char> = word.chars().collect();

        if chars.len() < MIN_STEM_CHARS {
            return Cow::Borrowed(word);
        }

        let original_len = chars.len();

        // Phase 1 — at most one primary strip.
        strip_one_suffix(&mut chars, PRIMARY_SUFFIXES);

        // Phase 2 — iterate cleanup strips to convergence. Bounded by
        // MAX_CLEANUP_ITERATIONS (each successful strip removes at
        // least one scalar, so the loop terminates even without the
        // bound; the bound is a defensive belt-and-suspenders check).
        for _ in 0..MAX_CLEANUP_ITERATIONS {
            let before = chars.len();
            strip_one_suffix(&mut chars, CLEANUP_SUFFIXES);
            if chars.len() == before {
                break;
            }
        }

        if chars.len() == original_len {
            Cow::Borrowed(word)
        } else {
            Cow::Owned(chars.into_iter().collect::<String>())
        }
    }
}

/// Strip at most one suffix from `chars`, longest-match-wins, in place.
/// Scans `table` in order (the caller is responsible for ordering
/// entries longest-first). No-op if no rule fires or if every
/// candidate would over-strip.
fn strip_one_suffix(chars: &mut Vec<char>, table: &[&[char]]) {
    for &suffix in table {
        if suffix.len() > chars.len() {
            continue;
        }
        let stem_len = chars.len() - suffix.len();
        if chars[stem_len..] != *suffix {
            continue;
        }
        if stem_len < MIN_STEM_CHARS {
            // Over-strip guard fired — keep looking for a shorter
            // suffix (there may not be one, in which case the input
            // passes through unchanged).
            continue;
        }
        chars.truncate(stem_len);
        return;
    }
}

impl Stemmer for LightMarathiStemmer {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        Self::stem(self, word)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &str) -> String {
        LightMarathiStemmer.stem(w).into_owned()
    }

    // -------------------------------------------------------------
    // Case markers — the agglutinative core.
    // -------------------------------------------------------------

    #[test]
    fn strips_dative_singular_la() {
        // घराला → घर (to the house — strip -ला, then bare -ा matra).
        // The convergence loop handles both in a single call.
        assert_eq!(s("घराला"), "घर");
    }

    #[test]
    fn strips_instrumental_singular_ne() {
        // मुलाने → मुल (by the boy).
        assert_eq!(s("मुलाने"), "मुल");
    }

    #[test]
    fn strips_genitive_masculine_singular_cha() {
        // मुलाचा → मुल (of the boy — m. sg. agreement).
        assert_eq!(s("मुलाचा"), "मुल");
    }

    #[test]
    fn strips_genitive_feminine_singular_chi() {
        // मुलाची → मुल (of the boy — f. sg. agreement).
        assert_eq!(s("मुलाची"), "मुल");
    }

    #[test]
    fn strips_locative_madhye() {
        // घरामध्ये → घर (in the house — strip -मध्ये, then bare matra).
        assert_eq!(s("घरामध्ये"), "घर");
    }

    #[test]
    fn strips_ablative_hun() {
        // घराहून → घर (from the house).
        assert_eq!(s("घराहून"), "घर");
    }

    // -------------------------------------------------------------
    // Verb personal endings — present habitual.
    // -------------------------------------------------------------

    #[test]
    fn strips_present_habitual_3sg_m_to() {
        // बोलतो → बोल (I / he speak(s) — m.).
        assert_eq!(s("बोलतो"), "बोल");
    }

    #[test]
    fn strips_present_habitual_3sg_f_te() {
        // बोलते → बोल (I / she speak(s) — f./n.).
        assert_eq!(s("बोलते"), "बोल");
    }

    #[test]
    fn strips_present_habitual_3pl_taat() {
        // बोलतात → बोल (they speak).
        assert_eq!(s("बोलतात"), "बोल");
    }

    #[test]
    fn strips_present_habitual_2sg_m_tos() {
        // बोलतोस → बोल (you speak — m. informal).
        assert_eq!(s("बोलतोस"), "बोल");
    }

    // -------------------------------------------------------------
    // Verb personal endings — aorist / perfective.
    // -------------------------------------------------------------

    #[test]
    fn strips_aorist_3sg_m_la() {
        // बोलला → बोल (he spoke).
        assert_eq!(s("बोलला"), "बोल");
    }

    #[test]
    fn strips_aorist_3sg_f_li() {
        // बोलली → बोल (she spoke).
        assert_eq!(s("बोलली"), "बोल");
    }

    #[test]
    fn strips_aorist_3sg_n_le() {
        // बोलले → बोल (it spoke / they-m spoke).
        assert_eq!(s("बोलले"), "बोल");
    }

    #[test]
    fn strips_aorist_3pl_f_lya() {
        // बोलल्या → बोल (they-f spoke).
        assert_eq!(s("बोलल्या"), "बोल");
    }

    // -------------------------------------------------------------
    // Infinitive marker.
    // -------------------------------------------------------------

    #[test]
    fn strips_infinitive_ne() {
        // करणे → कर (to do).
        assert_eq!(s("करणे"), "कर");
    }

    // -------------------------------------------------------------
    // Plural / oblique markers.
    // -------------------------------------------------------------

    #[test]
    fn strips_oblique_plural_marker() {
        // घरां → घर (houses — oblique plural, strip -ां).
        assert_eq!(s("घरां"), "घर");
    }

    #[test]
    fn strips_stacked_plural_plus_case_iterates_to_convergence() {
        // घरांना → घर (to the houses — strips -ना then -ां).
        // The convergence loop reduces both in one external call.
        assert_eq!(s("घरांना"), "घर");
    }

    // -------------------------------------------------------------
    // Longest-match wins.
    // -------------------------------------------------------------

    #[test]
    fn longest_match_wins_taat_over_bare_ta() {
        // बोलतात — तात beats bare त.
        assert_eq!(s("बोलतात"), "बोल");
    }

    #[test]
    fn longest_match_wins_chya_over_bare_cha() {
        // "मुलांच्या" (of the boys — genitive with oblique-plural
        // agreement) strips -च्या (4 chars) first, leaving "मुलां"
        // (4 chars), then the convergence loop strips -ां leaving
        // "मुल" (3 chars).
        assert_eq!(s("मुलांच्या"), "मुल");
    }

    // -------------------------------------------------------------
    // Contract: identity, idempotent, guarded.
    // -------------------------------------------------------------

    #[test]
    fn identity_on_no_match() {
        assert_eq!(s("पुस्तक"), "पुस्तक");
        assert_eq!(s(""), "");
        assert_eq!(s("hello"), "hello");
    }

    #[test]
    fn idempotent_second_pass_is_noop() {
        // Convergence loop guarantees strict idempotence — every input
        // reaches a fixed point in the first external call.
        for w in [
            "घराला",
            "मुलाने",
            "मुलाचा",
            "बोलतो",
            "बोलते",
            "बोलतात",
            "बोलल्या",
            "करणे",
            "घरांना",
            "मुलांच्या",
        ] {
            let once = LightMarathiStemmer.stem(w).into_owned();
            let twice = LightMarathiStemmer.stem(&once).into_owned();
            assert_eq!(once, twice, "stem not idempotent on {w:?}");
        }
    }

    #[test]
    fn over_strip_guard_refuses_short_stems() {
        // "ला" alone — 2 chars, stripping ला leaves 0 → guarded.
        assert_eq!(s("ला"), "ला");
        // "त" alone — 1 char, length short-circuit.
        assert_eq!(s("त"), "त");
    }

    #[test]
    fn output_never_longer_than_input() {
        for w in ["घराला", "मुलाने", "पुस्तक", "hello", "बोलल्या", "घरांना"]
        {
            let out = LightMarathiStemmer.stem(w);
            assert!(out.len() <= w.len(), "stem grew on {w:?}: {out:?}");
        }
    }

    #[test]
    fn borrowed_when_no_match_owned_when_matched() {
        let borrowed = LightMarathiStemmer.stem("पुस्तक");
        assert!(matches!(borrowed, Cow::Borrowed(_)));
        let owned = LightMarathiStemmer.stem("बोलतो");
        assert!(matches!(owned, Cow::Owned(_)));
    }
}
