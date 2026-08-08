//! [`KoreanStemmer`] — a deliberately small suffix / particle stripper.
//!
//! # Why so small?
//!
//! Korean is **agglutinative**: nouns take a stack of case particles
//! (`이/가` subject, `을/를` object, `에` locative, `에서` ablative,
//! `에게` dative, `으로` instrumental, `와/과` comitative, `의`
//! genitive, `도` also, `만` only, `까지` until, `부터` from), and
//! verbs and adjectives take a stack of tense / mood / aspect / speech-
//! level suffixes (`-습니다`, `-어요`, `-았`, `-겠`, `-는`, `-은`, …).
//! A proper Korean stemmer needs a morphological analyzer that
//! recognizes verb / adjective conjugation paradigms and case-particle
//! selection rules (`이/가` after a consonant vs. `가` after a vowel,
//! `을/를` similarly, `과/와` similarly, `으로/로` similarly). That
//! kind of quality is dictionary-driven (`mecab-ko`, `khaiii`) and out
//! of scope for a wasm-first / offline-first language pack.
//!
//! Instead, this crate ships a *coarse suffix stripper* — not a true
//! stemmer — that iteratively removes a small closed inventory of the
//! most common noun-attached case particles from the syllable end.
//!
//! # Rules
//!
//! The stemmer iteratively strips the *longest matching* entry from
//! this closed set at the end of a word, repeating until no entry
//! matches:
//!
//! ```text
//! -에서   ablative "from"
//! -까지   allative "until"
//! -부터   ablative "from" (time)
//! -에게   dative "to (animate)"
//! -으로   instrumental "with / by"
//! -는     topic (after vowel)
//! -은     topic (after consonant)
//! -을     object (after consonant)
//! -를     object (after vowel)
//! -이     subject (after consonant)
//! -가     subject (after vowel)
//! -에     locative "at / to"
//! -로     instrumental short form
//! -와     comitative (after vowel)
//! -과     comitative (after consonant)
//! -의     genitive "of"
//! -도     also
//! -만     only
//! -다     dictionary-form marker (nouns / adjectives used as adjectives)
//! ```
//!
//! # Contract
//!
//! - **Idempotent.** `stem(stem(w)) == stem(w)` for every input the
//!   stripping rules touch. Iterating the rules until a fixed point is
//!   what makes this true when a compound suffix stack is present
//!   (e.g., `학교에서도` "at school too" strips `-도` first, then
//!   `-에서`).
//! - **Non-lengthening.** The output is never longer than the input
//!   (every rule is a strict suffix deletion).
//! - **Non-empty output.** The stemmer refuses to strip a suffix that
//!   would leave an empty stem — a bare `는` or `이` returns unchanged.
//! - **Min-stem-length guard.** The stemmer refuses to strip when the
//!   remaining stem would be fewer than 1 character. Combined with the
//!   non-empty rule this means every output is at least 1 Hangul
//!   syllable.
//!
//! # Non-goals
//!
//! - **Verb / adjective conjugation stripping.** Korean verb endings
//!   (`-습니다`, `-어요`, `-았`, `-겠`, `-는`, `-은`, `-어서`, `-니까`,
//!   …) attach to stems whose surface form varies with the following
//!   vowel (`먹` "eat" + `-어요` → `먹어요`; `가` "go" + `-아요` →
//!   `가요` after vowel elision). Recognizing the vowel elision needs
//!   a paradigm-aware analyzer. Deferred to a morphological analyzer.
//! - **Verb-tense particle stripping (-았, -겠).** Same reasoning as
//!   the above.
//! - **Nominalizer suffixes (-기, -음, -ㅁ).** The `-ㅁ` nominalizer
//!   attaches as a jongseong to the last vowel of the stem, which is
//!   only visible at the jamo level — stripping it requires
//!   decomposing the syllable, removing the jongseong, and recomposing.
//!   Deferred.
//! - **Compound-word splitting.** Korean writes long compounds like
//!   `대한민국` "Republic of Korea" as one orthographic word; splitting
//!   it into `대한` + `민국` needs a compound dictionary.

use alloc::borrow::Cow;

use stringcheese_lang::Stemmer;

/// The Korean coarse particle stripper.
///
/// A zero-sized value; construct as [`KoreanStemmer`] and reuse the
/// value freely across threads and calls.
///
/// See the [module-level docs](self) for the (small) rule set and the
/// contract.
///
/// # Example
///
/// ```
/// use stringcheese_ko::KoreanStemmer;
/// use stringcheese_lang::Stemmer;
///
/// // Simple particles.
/// assert_eq!(KoreanStemmer.stem("책은"), "책");
/// assert_eq!(KoreanStemmer.stem("학교에서"), "학교");
/// // Iterative: `-도` after `-에서`.
/// assert_eq!(KoreanStemmer.stem("학교에서도"), "학교");
/// // Non-lengthening + non-emptying: bare particle is returned as-is.
/// assert_eq!(KoreanStemmer.stem("는"), "는");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct KoreanStemmer;

/// The suffix table. Entries are ordered by **descending length** so
/// the stripper trips the longest match at each iteration — otherwise
/// `학교에서` would strip `-에` before ever seeing `-에서`, leaving a
/// stray `서` behind.
///
/// Every entry is a Korean particle from the closed set the module
/// docs enumerate. The rules are strict suffix deletion — no rewriting.
const SUFFIXES: &[&str] = &[
    // Three-syllable — none in this set (Korean particles are 1-2
    // syllables).
    // Two-syllable particles.
    "에서", // ablative "from"
    "까지", // allative "until"
    "부터", // ablative-time "from"
    "에게", // dative "to (animate)"
    "으로", // instrumental (after consonant)
    // Single-syllable particles.
    "는", // topic (after vowel)
    "은", // topic (after consonant)
    "을", // object (after consonant)
    "를", // object (after vowel)
    "이", // subject (after consonant)
    "가", // subject (after vowel)
    "에", // locative
    "로", // instrumental short form
    "와", // comitative (after vowel)
    "과", // comitative (after consonant)
    "의", // genitive
    "도", // also
    "만", // only
    "다", // dictionary-form marker
];

impl KoreanStemmer {
    /// Strip Korean particles from `word`, iterating to a fixed point.
    ///
    /// The iteration handles particle stacks — Korean allows a limited
    /// set of stacked particles, e.g. `학교에서도` "at school too"
    /// (locative `-에서` + focus `-도`), and each iteration removes
    /// exactly one particle. The loop terminates because every strip
    /// removes at least one character, so it runs in O(len(word))
    /// passes in the worst case.
    ///
    /// Returns [`Cow::Borrowed`] when no suffix matches on the first
    /// pass (fast path); [`Cow::Owned`] when any suffix is stripped
    /// (the stripped tail is released as owned bytes and the loop
    /// continues on the owned buffer).
    ///
    /// See the type-level docs for the contract (idempotent,
    /// non-lengthening, non-emptying, min-stem-length ≥ 1 syllable).
    #[must_use]
    pub fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        let Some(mut stripped) = try_strip(word) else {
            return Cow::Borrowed(word);
        };
        while let Some(next) = try_strip(&stripped) {
            stripped = next;
        }
        Cow::Owned(stripped)
    }
}

/// One pass of suffix stripping: return `Some(stem)` if any suffix in
/// the table matches; `None` otherwise. Enforces the non-emptying rule
/// (refuses to strip if the resulting stem would be empty) and the
/// 1-syllable min-stem-length guard.
fn try_strip(word: &str) -> Option<alloc::string::String> {
    for &suffix in SUFFIXES {
        if word.len() > suffix.len() && word.ends_with(suffix) {
            let stem_len = word.len() - suffix.len();
            // `str::ends_with`'s contract guarantees `stem_len` is a
            // valid UTF-8 boundary.
            let stem = &word[..stem_len];
            // Non-empty (redundant with `word.len() > suffix.len()`
            // when the suffix is at least one byte, but kept for
            // documentation) plus the 1-scalar minimum.
            if !stem.is_empty() && stem.chars().count() >= 1 {
                return Some(alloc::string::String::from(stem));
            }
        }
    }
    None
}

impl Stemmer for KoreanStemmer {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        Self::stem(self, word)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &str) -> alloc::string::String {
        KoreanStemmer.stem(w).into_owned()
    }

    // ---------------------------------------------------------------
    // Case particles.
    // ---------------------------------------------------------------

    #[test]
    fn strips_topic_marker_neun() {
        assert_eq!(s("나는"), "나");
    }

    #[test]
    fn strips_topic_marker_eun() {
        assert_eq!(s("책은"), "책");
    }

    #[test]
    fn strips_subject_i() {
        assert_eq!(s("사람이"), "사람");
    }

    #[test]
    fn strips_subject_ga() {
        assert_eq!(s("친구가"), "친구");
    }

    #[test]
    fn strips_object_eul() {
        assert_eq!(s("책을"), "책");
    }

    #[test]
    fn strips_object_reul() {
        assert_eq!(s("나를"), "나");
    }

    #[test]
    fn strips_locative_e() {
        assert_eq!(s("집에"), "집");
    }

    #[test]
    fn strips_ablative_eseo() {
        // Longest-match: `-에서` trips before `-에`.
        assert_eq!(s("학교에서"), "학교");
    }

    #[test]
    fn strips_allative_kkaji() {
        assert_eq!(s("여기까지"), "여기");
    }

    #[test]
    fn strips_ablative_time_buteo() {
        assert_eq!(s("지금부터"), "지금");
    }

    #[test]
    fn strips_dative_ege() {
        assert_eq!(s("친구에게"), "친구");
    }

    #[test]
    fn strips_instrumental_euro() {
        assert_eq!(s("연필으로"), "연필");
    }

    #[test]
    fn strips_genitive_ui() {
        assert_eq!(s("나의"), "나");
    }

    #[test]
    fn strips_also_do() {
        assert_eq!(s("나도"), "나");
    }

    #[test]
    fn strips_only_man() {
        assert_eq!(s("너만"), "너");
    }

    // ---------------------------------------------------------------
    // Iterative fixed-point stripping (Korean is agglutinative).
    // ---------------------------------------------------------------

    #[test]
    fn iteratively_strips_stacked_particles() {
        // `학교에서도` = 학교 + -에서 + -도 → 학교
        assert_eq!(s("학교에서도"), "학교");
    }

    #[test]
    fn iteratively_strips_from_and_until() {
        // Hypothetical `여기부터까지` stack — the iterative loop peels
        // both.
        assert_eq!(s("여기부터까지"), "여기");
    }

    // ---------------------------------------------------------------
    // Contract: idempotent, non-lengthening, non-emptying.
    // ---------------------------------------------------------------

    #[test]
    fn identity_on_no_match() {
        assert_eq!(s("사람"), "사람");
        assert_eq!(s("apple"), "apple");
        assert_eq!(s(""), "");
    }

    #[test]
    fn idempotent_second_pass_is_noop() {
        for w in ["책은", "학교에서", "학교에서도", "나의", "친구에게"] {
            let once = KoreanStemmer.stem(w).into_owned();
            let twice = KoreanStemmer.stem(&once).into_owned();
            assert_eq!(once, twice, "stem not idempotent on {w:?}");
        }
    }

    #[test]
    fn refuses_to_produce_empty_stem() {
        // Bare particle inputs — nothing to strip without emptying.
        assert_eq!(s("는"), "는");
        assert_eq!(s("을"), "을");
        assert_eq!(s("에서"), "에서");
    }

    #[test]
    fn output_never_longer_than_input() {
        for w in ["책은", "학교에서", "사람", "hello", "학교에서도"] {
            let out = KoreanStemmer.stem(w);
            assert!(out.len() <= w.len(), "stem grew on {w:?}: {out:?}");
        }
    }

    #[test]
    fn identity_on_non_korean_input() {
        assert_eq!(s("running"), "running");
        assert_eq!(s("hello world"), "hello world");
    }

    #[test]
    fn borrowed_when_no_match_owned_when_matched() {
        let borrowed = KoreanStemmer.stem("사람");
        assert!(matches!(borrowed, Cow::Borrowed(_)));
        let owned = KoreanStemmer.stem("책은");
        assert!(matches!(owned, Cow::Owned(_)));
    }
}
