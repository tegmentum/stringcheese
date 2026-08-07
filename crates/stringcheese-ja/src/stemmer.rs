//! [`JapaneseStemmer`] — a deliberately minimal noun-suffix and
//! polite-form stripper.
//!
//! # Why so small?
//!
//! Japanese has no inflection-heavy morphology like English's `-ing`
//! / `-ed` / `-s` or French's conjugation ladder. Verb conjugation is
//! *productive* — every verb pattern-matches one of two paradigms
//! (godan / ichidan) and a full stemmer would need a
//! part-of-speech-aware morphological analyzer to distinguish
//! `-ます` (polite auxiliary) from `-ま` inside a native verb stem
//! (compare `のみます` "drink-POLITE" vs. `まがる` "bend").
//!
//! Instead, this crate ships a *token-simplifier* — not a true stemmer
//! — that strips two families of clearly-decorative surface material:
//!
//! - **Plural markers** (`-たち`, `-達`, `-ら`, `-ども`) — Japanese
//!   nouns don't inflect for number, but a small set of suffixes
//!   marks (usually animate) plural. Stripping them collapses `私たち`
//!   ("we") to `私` ("I"), `子供達` ("children") to `子供` ("child").
//! - **Polite auxiliaries** (`-ます`, `-です`) — the polite verb
//!   auxiliary and the polite copula. Stripping them collapses
//!   `のみます` to `のみ` and `学生です` to `学生`, which is what an
//!   IR pipeline typically wants (register-independence).
//!
//! # Contract
//!
//! - **Idempotent.** `stem(stem(w)) == stem(w)` for every input the
//!   stripping rules touch. A word ending in `たち達` (a hypothetical
//!   Frankenstein) reduces in one pass to the empty-suffix root; a
//!   second call is a no-op.
//! - **Non-lengthening.** The output is never longer than the input
//!   (all rules are strict suffix deletions).
//! - **Non-empty output.** The stemmer refuses to strip a suffix if
//!   stripping would leave an empty stem (so `ます` on its own does
//!   not stem to `""`).
//!
//! # Non-goals
//!
//! - **Verb conjugation stripping.** Removing `-る`/`-た`/`-て`/`-ば`
//!   from verb forms requires paradigm knowledge (`のむ` → `のみ` in
//!   masu-form vs. `のる` → `のり` in masu-form) that a suffix-stripper
//!   can't provide. Deferred to a morphological analyzer.
//! - **Adjective inflection.** Similar reasoning — `赤い` → `赤くない`
//!   / `赤かった` / `赤ければ` is a paradigm, not a suffix.
//! - **Compound-noun splitting.** Reducing `日本人` ("Japanese person")
//!   to `日本` + `人` needs a compound dictionary.
//! - **Kanji-plural normalization.** Reducing 人々 ("people") to 人
//!   ("person") needs semantic awareness — the iteration mark 々 is
//!   orthographic, not morphological.

use alloc::borrow::Cow;

use stringcheese_lang::Stemmer;

/// The Japanese "polite-and-plural" simplifier.
///
/// A zero-sized value; construct as [`JapaneseStemmer`] and reuse the
/// value freely across threads and calls.
///
/// See the [module-level docs](self) for the (small) rule set and the
/// contract.
///
/// # Example
///
/// ```
/// use stringcheese_ja::JapaneseStemmer;
/// use stringcheese_lang::Stemmer;
///
/// assert_eq!(JapaneseStemmer.stem("私たち"), "私");
/// assert_eq!(JapaneseStemmer.stem("学生です"), "学生");
/// assert_eq!(JapaneseStemmer.stem("食べます"), "食べ");
/// // Idempotent — a second call on the stemmed output is a no-op.
/// let once = JapaneseStemmer.stem("私たち").into_owned();
/// assert_eq!(JapaneseStemmer.stem(&once), "私");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct JapaneseStemmer;

/// The suffix table. Each entry is a `&'static str` — the stemmer
/// strips the *first* one that matches. The ordering below places
/// longer suffixes first so `たち達` (contrived) or `でした` (past
/// polite copula) trip the longest match.
///
/// The rules are strict suffix deletion — no rewriting.
const SUFFIXES: &[&str] = &[
    // Polite past copula: でした (`gakusei desita` → `gakusei`).
    "でした",
    // Polite past auxiliary: ました (`nomimasita` → `nomi`).
    "ました",
    // Polite negative auxiliary: ません (`nomimasen` → `nomi`).
    "ません",
    // Plural markers.
    "たち",
    "達",
    "ども",
    // Polite auxiliaries.
    "ます",
    "です",
    // Short plural marker.
    "ら",
];

impl JapaneseStemmer {
    /// Strip polite/plural suffixes from `word`, iterating to a fixed
    /// point.
    ///
    /// The iteration is required because a compound stack like
    /// `-たち達` (contrived — a plural marker doubled) or `-達ら`
    /// (kanji plural + short plural) needs two passes to fully
    /// collapse: one call strips the outermost suffix, and only a
    /// second call sees the newly-exposed inner suffix. Iterating
    /// inside [`stem`](Self::stem) means external callers get a genuine idempotent
    /// interface: `stem(stem(w)) == stem(w)` in one call.
    ///
    /// Returns [`Cow::Borrowed`] when no suffix matches on the first
    /// pass (fast path); [`Cow::Owned`] when any suffix is stripped
    /// (the stripped tail is released as owned bytes and the loop
    /// continues on the owned buffer).
    ///
    /// See the type-level docs for the contract (idempotent,
    /// non-lengthening, non-emptying).
    #[must_use]
    pub fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        // Fast-path check: if the first pass finds nothing, return
        // a borrow.
        let Some(mut stripped) = try_strip(word) else {
            return Cow::Borrowed(word);
        };
        // A suffix matched; keep stripping until a pass changes
        // nothing. Each pass removes at least one whole suffix
        // (bytes-wise, well over one byte), so the loop terminates in
        // O(len(word)) passes in the worst case.
        while let Some(next) = try_strip(&stripped) {
            stripped = next;
        }
        Cow::Owned(stripped)
    }
}

/// One pass of suffix stripping: return `Some(stem)` if any suffix in
/// the table matches; `None` otherwise. Enforces the non-emptying rule
/// (refuses to strip if the resulting stem would be empty).
fn try_strip(word: &str) -> Option<alloc::string::String> {
    for &suffix in SUFFIXES {
        if word.len() > suffix.len() && word.ends_with(suffix) {
            let stem_len = word.len() - suffix.len();
            // Safety: `stem_len` is a valid byte boundary because
            // `word.ends_with(suffix)` iff `word[stem_len..] == suffix`
            // per `str::ends_with`'s contract — which itself relies
            // on byte-boundary alignment.
            let stem = &word[..stem_len];
            if !stem.is_empty() {
                return Some(alloc::string::String::from(stem));
            }
        }
    }
    None
}

impl Stemmer for JapaneseStemmer {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        Self::stem(self, word)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &str) -> alloc::string::String {
        JapaneseStemmer.stem(w).into_owned()
    }

    // ---------------------------------------------------------------
    // Plural markers.
    // ---------------------------------------------------------------

    #[test]
    fn strips_tachi() {
        assert_eq!(s("私たち"), "私");
        assert_eq!(s("君たち"), "君");
    }

    #[test]
    fn strips_kanji_tachi() {
        assert_eq!(s("子供達"), "子供");
    }

    #[test]
    fn strips_ra() {
        assert_eq!(s("彼ら"), "彼");
    }

    #[test]
    fn strips_domo() {
        assert_eq!(s("私ども"), "私");
    }

    // ---------------------------------------------------------------
    // Polite auxiliaries.
    // ---------------------------------------------------------------

    #[test]
    fn strips_masu() {
        assert_eq!(s("食べます"), "食べ");
        assert_eq!(s("のみます"), "のみ");
    }

    #[test]
    fn strips_desu() {
        assert_eq!(s("学生です"), "学生");
    }

    #[test]
    fn strips_mashita_before_masu() {
        // Longest-match ordering: ました trips before ます.
        assert_eq!(s("食べました"), "食べ");
    }

    #[test]
    fn strips_masen_before_masu() {
        // Longest-match: ません trips before ます.
        assert_eq!(s("行きません"), "行き");
    }

    #[test]
    fn strips_deshita_before_desu() {
        assert_eq!(s("学生でした"), "学生");
    }

    // ---------------------------------------------------------------
    // Contract: idempotent, non-lengthening, non-emptying.
    // ---------------------------------------------------------------

    #[test]
    fn identity_on_no_match() {
        // A word with no matching suffix is returned unchanged.
        assert_eq!(s("桜"), "桜");
        assert_eq!(s("apple"), "apple");
        assert_eq!(s(""), "");
    }

    #[test]
    fn idempotent_second_pass_is_noop() {
        for w in ["私たち", "食べます", "学生です", "彼ら", "子供達"] {
            let once = JapaneseStemmer.stem(w).into_owned();
            let twice = JapaneseStemmer.stem(&once).into_owned();
            assert_eq!(once, twice, "stem not idempotent on {w:?}");
        }
    }

    #[test]
    fn refuses_to_produce_empty_stem() {
        // Stripping ます from the bare word ます would leave "". The
        // `word.len() > suffix.len()` guard should prevent it.
        assert_eq!(s("ます"), "ます");
        assert_eq!(s("です"), "です");
        assert_eq!(s("たち"), "たち");
    }

    #[test]
    fn output_never_longer_than_input() {
        for w in ["私たち", "食べます", "桜", "hello", "学生でした"] {
            let out = JapaneseStemmer.stem(w);
            assert!(out.len() <= w.len(), "stem grew on {w:?}: {out:?}");
        }
    }

    #[test]
    fn identity_on_non_japanese_input() {
        // Nothing in the suffix table matches English suffixes, so the
        // input is returned unchanged. This is the point — the stemmer
        // is a no-op for non-Japanese input.
        assert_eq!(s("running"), "running");
        assert_eq!(s("hello world"), "hello world");
    }

    #[test]
    fn borrowed_when_no_match_owned_when_matched() {
        let borrowed = JapaneseStemmer.stem("桜");
        assert!(matches!(borrowed, Cow::Borrowed(_)));
        let owned = JapaneseStemmer.stem("私たち");
        assert!(matches!(owned, Cow::Owned(_)));
    }
}
