//! [`FrenchTokenizer`] — an elision-aware French tokenizer.
//!
//! French routinely elides a small set of clitic words when the
//! following word begins with a vowel or an unaspirated `h`. The clitics
//! are all one letter (`l'` for *le/la*, `d'` for *de*, `j'` for *je*,
//! `m'` for *me*, `t'` for *te*, `s'` for *se*, `n'` for *ne*, `c'` for
//! *ce*) plus two-letter `qu'` (for *que*) and its compounds (`jusqu'`,
//! `lorsqu'`, `puisqu'`, `quoiqu'`). Every one of these attaches to the
//! next word with an apostrophe: `l'homme`, `d'accord`, `qu'est-ce`,
//! `c'est`, `n'était`, `aujourd'hui` — except that the last is not an
//! elision at all but a compound (`au jour de + hui`, frozen into one
//! orthographic word).
//!
//! # Tokenization rule
//!
//! The tokenizer treats every apostrophe as *potentially* a clitic
//! boundary. When it hits `'`, it checks whether the token collected so
//! far is one of the eleven known clitic prefixes above (case-folded
//! ASCII: `l`, `d`, `j`, `m`, `t`, `s`, `n`, `c`, `qu`, `jusqu`,
//! `lorsqu`, `puisqu`, `quoiqu`). If it is, the apostrophe is included
//! in the current token (so `l'homme` yields `["l'", "homme"]`, not
//! `["l", "homme"]`) and a fresh token begins after it. Otherwise the
//! apostrophe is kept inside the current token (`aujourd'hui` yields
//! `["aujourd'hui"]`, not `["aujourd", "hui"]`).
//!
//! # Why include the apostrophe in the clitic
//!
//! The property test
//! `tokenizer_preserves_input_character_count` (in the property-test module)
//! requires the sum of token character counts to equal the count of
//! *token-worthy* characters in the input (alphabetic scalars plus
//! apostrophes that end a clitic). Keeping the apostrophe attached to
//! the clitic — rather than emitting it as its own zero-width nothing
//! or dropping it silently — makes the arithmetic honest and lets a
//! downstream detokenizer round-trip an "l' + homme" pair back to
//! `l'homme` by simple concatenation.
//!
//! An alternative convention — Lucene's `ElisionFilter` — strips the
//! apostrophe and yields `["l", "homme"]`. That is legitimate; it just
//! makes reconstruction lossy. The stopword list in
//! [`crate::stopwords::STOPWORDS`] lists both forms so a caller who
//! swaps in a stripping tokenizer still gets clitic recognition.
//!
//! # Non-goals
//!
//! - **Hyphen handling.** A hyphen (`-`) is a hard separator here,
//!   which is the right call for `qu'est-ce` (`["qu'", "est", "ce"]`)
//!   and for hyphenated proper nouns in most IR uses, but the wrong
//!   call for `dix-neuf` if you want the number as one token. A
//!   downstream tokenizer with a numeral parser can override this.
//! - **Aspirated h.** French distinguishes *aspirated* and *unaspirated*
//!   `h` (aspirated `h` blocks elision, so `le haricot` never elides to
//!   `l'haricot`). The distinction is lexical, not orthographic, and
//!   would require a lexicon we don't want to ship. The tokenizer is
//!   agnostic — it doesn't try to reject or produce elisions, it only
//!   parses the ones the input author wrote.
//! - **Numbers.** Decimal marks are treated the same as
//!   [`stringcheese_lang::SimpleTokenizer`] treats them: as separators.
//!   `3,14` yields `["3", "14"]`.

use core::str::CharIndices;

/// Length-in-bytes-and-scalar table of the French clitic prefixes the
/// tokenizer recognizes. Each entry is a lowercase ASCII byte slice; a
/// match against the collected token is case-insensitive.
///
/// The order does not matter (we match by exact equality after
/// case-folding); the list is small enough for a linear scan to beat
/// any hashing scheme.
const CLITICS: &[&str] = &[
    "l", "d", "j", "m", "t", "s", "n", "c", "qu", "jusqu", "lorsqu", "puisqu", "quoiqu",
];

/// The French elision-aware tokenizer.
///
/// A zero-sized value; construct as [`FrenchTokenizer`] and reuse the
/// value freely across threads and calls.
///
/// See the [module-level docs](self) for the tokenization rule.
///
/// # Example
///
/// ```
/// use stringcheese_fr::FrenchTokenizer;
///
/// let toks: Vec<&str> = FrenchTokenizer::new()
///     .tokenize("L'homme qui aimait aujourd'hui.")
///     .collect();
/// assert_eq!(toks, ["L'", "homme", "qui", "aimait", "aujourd'hui"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct FrenchTokenizer;

impl FrenchTokenizer {
    /// Constructs a new [`FrenchTokenizer`]. Zero-sized; free to call.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Splits `text` into elision-aware tokens.
    ///
    /// The returned iterator yields borrowed `&'a str` slices of the
    /// input; no allocation is performed.
    #[inline]
    pub fn tokenize<'a>(&self, text: &'a str) -> FrenchTokens<'a> {
        FrenchTokens {
            text,
            chars: text.char_indices(),
            peeked: None,
        }
    }
}

/// Iterator over the elision-aware tokens of a string, produced by
/// [`FrenchTokenizer::tokenize`].
///
/// The iterator borrows the input string and yields borrowed slices of
/// it — no allocation is performed. Held state is a
/// [`CharIndices`] cursor over the input plus a
/// single-slot look-ahead buffer for the character that ended the
/// previous scan.
#[derive(Clone, Debug)]
pub struct FrenchTokens<'a> {
    text: &'a str,
    chars: CharIndices<'a>,
    /// Single-slot look-ahead: `Some((i, ch))` when the previous call
    /// to `next_char` peeked past the end of a token and we need to
    /// feed that character back on the next call.
    peeked: Option<(usize, char)>,
}

impl FrenchTokens<'_> {
    /// Byte index one past the end of the input.
    #[inline]
    fn end(&self) -> usize {
        self.text.len()
    }

    /// Consume the next `(byte_index, char)` pair, honoring the
    /// look-ahead buffer.
    #[inline]
    fn next_char(&mut self) -> Option<(usize, char)> {
        if let Some(x) = self.peeked.take() {
            return Some(x);
        }
        self.chars.next()
    }

    /// Push a `(byte_index, char)` pair back onto the input.
    #[inline]
    fn push_back(&mut self, x: (usize, char)) {
        debug_assert!(self.peeked.is_none(), "look-ahead slot is single-shot");
        self.peeked = Some(x);
    }
}

impl<'a> Iterator for FrenchTokens<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        // Skip any leading separators (anything that isn't alphabetic).
        // The apostrophe is *never* a token-starter: a leading `'` is a
        // separator (it can't precede a valid clitic — the clitic
        // itself is a letter).
        let (start, first) = loop {
            let (i, c) = self.next_char()?;
            if c.is_alphabetic() {
                break (i, c);
            }
        };

        // Collect the alphabetic run. When we hit an apostrophe, check
        // whether what we've collected so far is a clitic — if it is,
        // include the apostrophe in the current token and stop; if it
        // isn't, keep collecting (the apostrophe becomes part of the
        // token, e.g. `aujourd'hui`).
        //
        // The initial `end` must cover the first scalar we already
        // consumed to find `start` — otherwise a one-character token
        // followed immediately by a separator (`"a b"`) would return
        // the empty slice `&text[start..start]`.
        let mut end = start + first.len_utf8();
        loop {
            let Some((i, c)) = self.next_char() else {
                // End of input — extend `end` to the input length so
                // the last character we consumed is included.
                end = self.end();
                break;
            };
            if c.is_alphabetic() {
                // Advance `end` past this scalar.
                end = i + c.len_utf8();
                continue;
            }
            if is_apostrophe(c) {
                // Is what we've collected so far a clitic?
                let so_far = &self.text[start..i];
                if is_clitic(so_far) {
                    // Include the apostrophe in the token and stop.
                    end = i + c.len_utf8();
                    break;
                }
                // Not a clitic — is there another alphabetic scalar
                // right after the apostrophe? If yes, keep the
                // apostrophe as part of the token; if no, drop the
                // apostrophe (treat it as a separator, don't include
                // it in the token).
                match self.next_char() {
                    Some(peeked) if peeked.1.is_alphabetic() => {
                        // Keep the apostrophe: `aujourd'hui`. Push the
                        // scalar we just peeked back onto the input so
                        // the outer loop consumes it (and extends
                        // `end` past its bytes).
                        end = i + c.len_utf8();
                        self.push_back(peeked);
                        continue;
                    }
                    Some(other) => {
                        // Apostrophe followed by non-letter — token
                        // ends before the apostrophe. Push the
                        // non-letter back so the outer loop sees it as
                        // the separator it is.
                        self.push_back(other);
                        break;
                    }
                    None => {
                        // Trailing apostrophe at end of input — treat
                        // as a separator (drop it).
                        break;
                    }
                }
            }
            // Non-alphabetic, non-apostrophe scalar — this is a hard
            // separator, token ends before it.
            break;
        }

        Some(&self.text[start..end])
    }
}

/// Is `c` an apostrophe? Accepts the ASCII `'` (U+0027) *and* the
/// typographic right-single-quote `’` (U+2019), which many modern
/// French inputs (word processors, mobile keyboards) produce in place
/// of the ASCII form.
#[inline]
fn is_apostrophe(c: char) -> bool {
    c == '\'' || c == '\u{2019}'
}

/// Is the ASCII-lowercased form of `s` in the French clitic table?
///
/// Case folding is ASCII-only — clitics are all lowercase Latin letters
/// (`l`, `qu`, `jusqu`, …), so a non-ASCII scalar in the collected
/// prefix means the prefix isn't a clitic regardless of case, and an
/// ASCII-only [`str::eq_ignore_ascii_case`] scan is the fastest honest
/// answer.
fn is_clitic(s: &str) -> bool {
    if s.is_empty() || !s.is_ascii() {
        return false;
    }
    CLITICS.iter().any(|c| c.eq_ignore_ascii_case(s))
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    extern crate alloc;

    use super::*;
    use alloc::vec::Vec;

    fn collect(input: &str) -> Vec<&str> {
        FrenchTokenizer::new().tokenize(input).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn whitespace_only_yields_no_tokens() {
        assert!(collect("   ").is_empty());
        assert!(collect("\t\n\r ").is_empty());
    }

    #[test]
    fn punctuation_only_yields_no_tokens() {
        assert!(collect("!!!").is_empty());
        assert!(collect("---,,,;;;").is_empty());
        // Bare apostrophes with no letter context are separators.
        assert!(collect("'''").is_empty());
    }

    #[test]
    fn splits_l_apostrophe_homme() {
        assert_eq!(collect("l'homme"), ["l'", "homme"]);
    }

    #[test]
    fn splits_every_single_letter_clitic() {
        assert_eq!(collect("l'ami"), ["l'", "ami"]);
        assert_eq!(collect("d'accord"), ["d'", "accord"]);
        assert_eq!(collect("j'ai"), ["j'", "ai"]);
        assert_eq!(collect("m'aime"), ["m'", "aime"]);
        assert_eq!(collect("t'aime"), ["t'", "aime"]);
        assert_eq!(collect("s'appelle"), ["s'", "appelle"]);
        assert_eq!(collect("n'est"), ["n'", "est"]);
        assert_eq!(collect("c'est"), ["c'", "est"]);
    }

    #[test]
    fn splits_qu_clitic() {
        assert_eq!(collect("qu'il"), ["qu'", "il"]);
        assert_eq!(collect("qu'est-ce"), ["qu'", "est", "ce"]);
    }

    #[test]
    fn splits_multi_letter_qu_clitics() {
        assert_eq!(collect("jusqu'à"), ["jusqu'", "à"]);
        assert_eq!(collect("lorsqu'il"), ["lorsqu'", "il"]);
        assert_eq!(collect("puisqu'on"), ["puisqu'", "on"]);
        assert_eq!(collect("quoiqu'il"), ["quoiqu'", "il"]);
    }

    #[test]
    fn keeps_compound_aujourdhui_together() {
        // "aujourd'hui" is not an elision — it's a frozen compound
        // (au jour de hui). "aujourd" is not a French clitic, so the
        // tokenizer must keep the apostrophe inside the token.
        assert_eq!(collect("aujourd'hui"), ["aujourd'hui"]);
    }

    #[test]
    fn case_insensitive_clitic_matching() {
        assert_eq!(collect("L'homme"), ["L'", "homme"]);
        assert_eq!(collect("QU'IL"), ["QU'", "IL"]);
        assert_eq!(collect("Jusqu'ici"), ["Jusqu'", "ici"]);
    }

    #[test]
    fn accepts_typographic_apostrophe() {
        // U+2019 RIGHT SINGLE QUOTATION MARK is the apostrophe most
        // modern French input methods (word processors, mobile
        // keyboards) produce.
        assert_eq!(collect("l\u{2019}homme"), ["l\u{2019}", "homme"]);
        assert_eq!(collect("qu\u{2019}il"), ["qu\u{2019}", "il"]);
    }

    #[test]
    fn hyphen_is_a_separator() {
        assert_eq!(collect("qu'est-ce"), ["qu'", "est", "ce"]);
        assert_eq!(collect("dix-neuf"), ["dix", "neuf"]);
        assert_eq!(collect("peut-être"), ["peut", "être"]);
    }

    #[test]
    fn digits_are_dropped_as_non_alphabetic() {
        // Unlike SimpleTokenizer, this tokenizer only accepts
        // alphabetic scalars — `is_alphabetic()`, not
        // `is_alphanumeric()` — so digits are separators. That's the
        // right call for French clitic recognition: `l'12` is not a
        // real elision, and letting digits into tokens would defeat
        // the clitic check.
        assert_eq!(collect("hello 2026"), ["hello"]);
    }

    #[test]
    fn accented_letters_stay_inside_tokens() {
        assert_eq!(collect("être"), ["être"]);
        assert_eq!(collect("déjà là"), ["déjà", "là"]);
        assert_eq!(collect("j'étais"), ["j'", "étais"]);
    }

    #[test]
    fn full_sentence() {
        assert_eq!(
            collect("L'homme qui n'était pas là s'appelle Pierre."),
            [
                "L'", "homme", "qui", "n'", "était", "pas", "là", "s'", "appelle", "Pierre",
            ]
        );
    }

    #[test]
    fn trailing_apostrophe_after_clitic_is_dropped() {
        // Malformed input `l'` with no following letter — the clitic
        // check fires (we've collected `l`), so the token includes the
        // apostrophe: `l'`. That's fine; a caller who wants stricter
        // parsing can filter empty-following-word clitics themselves.
        assert_eq!(collect("l'"), ["l'"]);
    }

    #[test]
    fn trailing_apostrophe_after_non_clitic_is_dropped() {
        // Input like `hello'` — the token is `hello`, the apostrophe
        // is dropped as a separator.
        assert_eq!(collect("hello'"), ["hello"]);
    }

    #[test]
    fn borrowed_slices_are_from_the_input() {
        let text = "l'homme";
        let toks: Vec<&str> = FrenchTokenizer::new().tokenize(text).collect();
        // Both tokens are borrowed slices of the input; verify by
        // taking each token's byte range from the input pointer
        // arithmetic (no unsafe: subtract token pointer values).
        let base = text.as_ptr() as usize;
        assert_eq!(toks[0].as_ptr() as usize - base, 0);
        assert_eq!(toks[1].as_ptr() as usize - base, 2);
        assert_eq!(toks[0], "l'");
        assert_eq!(toks[1], "homme");
    }

    #[test]
    fn is_clitic_recognizes_all_forms() {
        for c in [
            "l", "d", "j", "m", "t", "s", "n", "c", "qu", "jusqu", "lorsqu", "puisqu", "quoiqu",
        ] {
            assert!(is_clitic(c), "{c:?} should be a clitic");
            let upper: alloc::string::String = c.to_ascii_uppercase();
            assert!(
                is_clitic(&upper),
                "{upper:?} should be a clitic (case-insensitive)"
            );
        }
        for w in ["hello", "aujourd", "the", "", "quo", "l ", "'"] {
            assert!(!is_clitic(w), "{w:?} should not be a clitic");
        }
    }
}
