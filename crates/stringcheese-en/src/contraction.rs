//! [`ContractionTokenizer`] — English contraction-aware tokenizer.
//!
//! Splits English contractions into a base word and a suffix fragment
//! so a downstream stemmer or stopword filter can see the underlying
//! form. `"don't"` becomes `["do", "n't"]`, `"I'll"` becomes
//! `["I", "'ll"]`, `"can't"` becomes `["can", "n't"]`, and so on.
//!
//! Modeled on the tokenization convention used by NLTK's
//! [`WordPunctTokenizer`] / [`TreebankWordTokenizer`], spaCy's English
//! tokenizer, and Penn Treebank tokenization: the contraction fragment
//! is emitted as its own token and carries the leading apostrophe (or
//! the fused `n` for `-n't`) so a detokenizer can reconstruct the
//! original text by simple concatenation.
//!
//! [`WordPunctTokenizer`]: https://www.nltk.org/api/nltk.tokenize.regexp.html
//! [`TreebankWordTokenizer`]: https://www.nltk.org/api/nltk.tokenize.treebank.html
//!
//! # Two modes
//!
//! * [`STANDARD`](ContractionTokenizer::STANDARD) — preserves
//!   contraction fragments as their own tokens. `"don't"` yields
//!   `["do", "n't"]`, `"I've"` yields `["I", "'ve"]`. Round-trip
//!   friendly: joining the tokens with spaces and re-tokenizing yields
//!   the same token count. `special_forms` is on, so `"won't"`,
//!   `"can't"`, `"shan't"`, `"gonna"`, `"wanna"`, `"gotta"` are
//!   rewritten to their underlying forms (`"will"`, `"can"`, `"shall"`,
//!   `"going" + "to"`, …).
//! * [`NORMALIZED`](ContractionTokenizer::NORMALIZED) — expands the
//!   contraction fragments to their full-word English equivalents.
//!   `"don't"` → `["do", "not"]`, `"I'll"` → `["I", "will"]`. Useful
//!   for indexing pipelines that want `"not"` findable when the source
//!   text used `"don't"`. Ambiguous fragments (`-'s`, `-'m`) are left
//!   as-is: the tokenizer does not attempt to guess whether `"he's"`
//!   is `"he is"` or `"he has"`.
//!
//! # Recognized contractions
//!
//! * `-n't`: `aren't`, `can't`, `couldn't`, `didn't`, `doesn't`,
//!   `don't`, `hadn't`, `hasn't`, `haven't`, `isn't`, `mustn't`,
//!   `needn't`, `shan't`, `shouldn't`, `wasn't`, `weren't`, `won't`,
//!   `wouldn't`
//! * `-'ll`: `I'll`, `you'll`, `he'll`, `she'll`, `it'll`, `we'll`,
//!   `they'll`, and any proper name
//! * `-'ve`: `I've`, `you've`, `we've`, `they've`, `could've`,
//!   `should've`, `would've`, `might've`
//! * `-'re`: `you're`, `we're`, `they're`
//! * `-'d`: `I'd`, `you'd`, `he'd`, `she'd`, `we'd`, `they'd`
//!   (ambiguous — `would` vs `had`; the normalized form picks
//!   `would`)
//! * `-'s`: `he's`, `she's`, `it's`, `that's`, `there's`, `here's`
//!   (ambiguous — `is` vs `has`; treated as-is in either mode)
//! * `-'m`: `I'm` (treated as-is in either mode; there is no
//!   `normalize_m` flag)
//! * Special forms: `won't` → `will` + `n't`, `can't` → `can` + `n't`,
//!   `shan't` → `shall` + `n't`, `gonna` → `going` + `to`,
//!   `wanna` → `want` + `to`, `gotta` → `got` + `to` — all
//!   case-insensitive, controlled by [`special_forms`](Self).
//!
//! # Apostrophe forms
//!
//! Both the ASCII apostrophe `'` (U+0027) and the typographic right
//! single quotation mark `’` (U+2019) are recognized wherever an
//! apostrophe is expected. Output fragments always use the ASCII form.
//!
//! # Return type
//!
//! [`ContractionTokenizer::tokenize`] returns `Box<dyn Iterator<Item = String>>`
//! because normalization may expand the input (e.g. `"n't"` → `"not"`).
//! Callers that can work with borrowed slices should reach for
//! [`ContractionTokenizer::tokenize_borrowed`], which yields `&'a str`
//! and never allocates a per-token `String` — every fragment is either
//! a slice of the input or a `&'static str` for the expansion.

use alloc::boxed::Box;
use alloc::string::{String, ToString};

/// An English contraction-aware tokenizer.
///
/// See the [module-level docs](self) for the tokenization rules and
/// the recognized contraction list.
///
/// # Example
///
/// ```
/// use stringcheese_en::ContractionTokenizer;
///
/// let toks: Vec<String> = ContractionTokenizer::STANDARD
///     .tokenize("I don't know what she'll do.")
///     .collect();
/// assert_eq!(toks, ["I", "do", "n't", "know", "what", "she", "'ll", "do"]);
///
/// let expanded: Vec<String> = ContractionTokenizer::NORMALIZED
///     .tokenize("won't")
///     .collect();
/// assert_eq!(expanded, ["will", "not"]);
/// ```
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
// The six flags are the natural shape of the API — a `NormalizeFlags`
// bitfield or per-suffix state machine would obscure the "toggle one
// suffix's expansion" mental model documented in `Two modes` above.
#[allow(clippy::struct_excessive_bools)]
pub struct ContractionTokenizer {
    /// Expand `-'ll` to `"will"` when `true`; keep as `"'ll"` when
    /// `false`.
    normalize_ll: bool,
    /// Expand `-'ve` to `"have"` when `true`; keep as `"'ve"` when
    /// `false`.
    normalize_ve: bool,
    /// Expand `-'re` to `"are"` when `true`; keep as `"'re"` when
    /// `false`.
    normalize_re: bool,
    /// Expand `-'d` to `"would"` when `true`; keep as `"'d"` when
    /// `false`. Choice of `"would"` over `"had"` is a heuristic:
    /// `-'d` is ambiguous and the collocation-frequency literature
    /// tips slightly toward `would`.
    normalize_d: bool,
    /// Expand `-n't` to `"not"` when `true`; keep as `"n't"` when
    /// `false`.
    normalize_nt: bool,
    /// Rewrite the six irregular contractions (`won't`, `can't`,
    /// `shan't`, `gonna`, `wanna`, `gotta`) to their base + suffix
    /// forms when `true`. When `false`, `won't` falls through to the
    /// generic `-n't` rule and yields `["wo", "n't"]`, which is
    /// morphologically wrong but predictable.
    special_forms: bool,
}

impl ContractionTokenizer {
    /// The [`STANDARD`](Self::STANDARD) preset: split contractions but
    /// preserve fragment forms (`n't`, `'ll`, `'ve`, `'re`, `'d`, `'s`,
    /// `'m`). Special-form rewrites (`won't` → `will` + `n't`, etc.)
    /// are on.
    ///
    /// Round-trip friendly: joining the tokens with spaces and
    /// re-tokenizing yields the same token count.
    pub const STANDARD: Self = Self {
        normalize_ll: false,
        normalize_ve: false,
        normalize_re: false,
        normalize_d: false,
        normalize_nt: false,
        special_forms: true,
    };

    /// The [`NORMALIZED`](Self::NORMALIZED) preset: expand contraction
    /// fragments to their full-word equivalents (`n't` → `not`, `'ll` →
    /// `will`, `'ve` → `have`, `'re` → `are`, `'d` → `would`).
    /// Ambiguous fragments (`'s`, `'m`) are left as-is. Special-form
    /// rewrites are on.
    pub const NORMALIZED: Self = Self {
        normalize_ll: true,
        normalize_ve: true,
        normalize_re: true,
        normalize_d: true,
        normalize_nt: true,
        special_forms: true,
    };

    /// Construct a tokenizer with the [`STANDARD`](Self::STANDARD)
    /// preset.
    #[must_use]
    pub const fn new() -> Self {
        Self::STANDARD
    }

    /// Toggle `-'ll` expansion.
    #[must_use]
    pub const fn with_normalize_ll(mut self, v: bool) -> Self {
        self.normalize_ll = v;
        self
    }

    /// Toggle `-'ve` expansion.
    #[must_use]
    pub const fn with_normalize_ve(mut self, v: bool) -> Self {
        self.normalize_ve = v;
        self
    }

    /// Toggle `-'re` expansion.
    #[must_use]
    pub const fn with_normalize_re(mut self, v: bool) -> Self {
        self.normalize_re = v;
        self
    }

    /// Toggle `-'d` expansion (`would` is used for the ambiguous
    /// `would`/`had` case).
    #[must_use]
    pub const fn with_normalize_d(mut self, v: bool) -> Self {
        self.normalize_d = v;
        self
    }

    /// Toggle `-n't` expansion.
    #[must_use]
    pub const fn with_normalize_nt(mut self, v: bool) -> Self {
        self.normalize_nt = v;
        self
    }

    /// Toggle special-form rewrites (`won't`, `can't`, `shan't`,
    /// `gonna`, `wanna`, `gotta`).
    #[must_use]
    pub const fn with_special_forms(mut self, v: bool) -> Self {
        self.special_forms = v;
        self
    }

    /// Tokenize `text` into owned `String` tokens.
    ///
    /// Convenience wrapper that maps [`tokenize_borrowed`](Self::tokenize_borrowed)
    /// through `String::from`. Callers that can work with borrowed
    /// slices should use `tokenize_borrowed` directly to avoid the
    /// per-token allocation.
    pub fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = String> + 'a> {
        Box::new(self.tokenize_borrowed(text).map(ToString::to_string))
    }

    /// Tokenize `text` into borrowed `&'a str` tokens.
    ///
    /// Every yielded slice is either a sub-slice of `text` or a
    /// `&'static str` for an expansion / static fragment. No `String`
    /// allocation is performed per token.
    pub fn tokenize_borrowed<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
        Box::new(ContractionTokens {
            cfg: *self,
            raw: RawWords { text, offset: 0 },
            pending: None,
        })
    }
}

impl Default for ContractionTokenizer {
    fn default() -> Self {
        Self::STANDARD
    }
}

/// Table of special-form contractions.
///
/// Each entry is `(form, base, suffix, is_nt_suffix)`. Recognition is
/// case-insensitive against `form`; either apostrophe form (ASCII or
/// U+2019) matches. When `is_nt_suffix` is `true` and the tokenizer's
/// `normalize_nt` flag is set, the suffix is rewritten to `"not"`.
/// `gonna`/`wanna`/`gotta` carry `to` as their suffix and are never
/// normalized further (there is no `normalize_to` flag).
const SPECIAL_FORMS: &[(&str, &str, &str, bool)] = &[
    ("won't", "will", "n't", true),
    ("can't", "can", "n't", true),
    ("shan't", "shall", "n't", true),
    ("gonna", "going", "to", false),
    ("wanna", "want", "to", false),
    ("gotta", "got", "to", false),
];

/// The iterator produced by [`ContractionTokenizer::tokenize_borrowed`].
struct ContractionTokens<'a> {
    cfg: ContractionTokenizer,
    raw: RawWords<'a>,
    /// Single-slot buffer for the second token of a split contraction.
    pending: Option<&'a str>,
}

impl<'a> Iterator for ContractionTokens<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        if let Some(p) = self.pending.take() {
            return Some(p);
        }
        let word = self.raw.next()?;
        let (first, second) = split_word(self.cfg, word);
        self.pending = second;
        Some(first)
    }
}

/// Raw word tokenizer: emits maximal runs of alphanumerics with
/// internal apostrophes retained.
///
/// The rule matches spaCy / NLTK Treebank tokenization for the
/// purposes of contraction recognition: `"don't"` is one raw word (not
/// two split on the apostrophe), `"aujourd'hui"` is one raw word,
/// `"O'Neill"` is one raw word, `"hello, world"` is `["hello",
/// "world"]`. A word may begin with an apostrophe when followed by an
/// alphanumeric — the STANDARD tokenizer emits `"'ll"` as its own
/// contraction fragment, and this rule lets `"I 'll go"` re-tokenize
/// into three tokens rather than two.
struct RawWords<'a> {
    text: &'a str,
    offset: usize,
}

impl<'a> Iterator for RawWords<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        // Skip separators. A "separator" is any character that isn't
        // (a) alphanumeric or (b) an apostrophe followed by an
        // alphanumeric.
        loop {
            let rest = self.text.get(self.offset..)?;
            let ch = rest.chars().next()?;
            let ch_len = ch.len_utf8();
            if ch.is_alphanumeric() {
                break;
            }
            if is_apostrophe(ch) {
                let after = &rest[ch_len..];
                if let Some(next_ch) = after.chars().next()
                    && next_ch.is_alphanumeric()
                {
                    break;
                }
            }
            self.offset += ch_len;
        }

        let start = self.offset;

        // Collect the run: alphanumerics plus internal apostrophes
        // that are followed by an alphanumeric.
        while let Some(rest) = self.text.get(self.offset..) {
            let Some(ch) = rest.chars().next() else { break };
            let ch_len = ch.len_utf8();
            if ch.is_alphanumeric() {
                self.offset += ch_len;
                continue;
            }
            if is_apostrophe(ch) {
                let after = &rest[ch_len..];
                match after.chars().next() {
                    Some(next_ch) if next_ch.is_alphanumeric() => {
                        self.offset += ch_len;
                        continue;
                    }
                    _ => break,
                }
            }
            break;
        }

        if start == self.offset {
            None
        } else {
            Some(&self.text[start..self.offset])
        }
    }
}

/// Attempt to split `word` into a base + suffix contraction pair.
///
/// Returns `(base, None)` when `word` is not a contraction; otherwise
/// returns `(base, Some(suffix))` with the two tokens the tokenizer
/// should emit.
fn split_word(cfg: ContractionTokenizer, word: &str) -> (&str, Option<&str>) {
    // Idempotence guard: a raw word with two or more apostrophes cannot
    // split cleanly — after peeling one suffix, the base still holds an
    // apostrophe, and re-tokenizing the joined output would split it
    // again. Every recognized contraction (`don't`, `I'll`, `won't`,
    // ...) carries exactly one apostrophe, so treat multi-apostrophe
    // input as atomic and preserve the tokenizer's idempotence contract.
    if word.chars().filter(|&c| is_apostrophe(c)).count() >= 2 {
        return (word, None);
    }

    // Special forms (case-insensitive full-word match) fire first so
    // "won't" doesn't fall through to the generic `-n't` rule and yield
    // `("wo", "n't")`.
    if cfg.special_forms {
        for &(form, base, suffix, is_nt) in SPECIAL_FORMS {
            if eq_ci_apostrophe(word, form) {
                let s = if is_nt && cfg.normalize_nt {
                    "not"
                } else {
                    suffix
                };
                return (base, Some(s));
            }
        }
    }

    // -n't (must be checked before -'d and -'t patterns that overlap
    // with typographic-apostrophe encodings).
    if let Some(base_end) = match_suffix(word, &['n', '\'', 't'])
        && base_end > 0
    {
        let base = &word[..base_end];
        let suffix = if cfg.normalize_nt { "not" } else { "n't" };
        return (base, Some(suffix));
    }

    // -'ll
    if let Some(base_end) = match_suffix(word, &['\'', 'l', 'l'])
        && base_end > 0
    {
        let base = &word[..base_end];
        let suffix = if cfg.normalize_ll { "will" } else { "'ll" };
        return (base, Some(suffix));
    }

    // -'ve
    if let Some(base_end) = match_suffix(word, &['\'', 'v', 'e'])
        && base_end > 0
    {
        let base = &word[..base_end];
        let suffix = if cfg.normalize_ve { "have" } else { "'ve" };
        return (base, Some(suffix));
    }

    // -'re
    if let Some(base_end) = match_suffix(word, &['\'', 'r', 'e'])
        && base_end > 0
    {
        let base = &word[..base_end];
        let suffix = if cfg.normalize_re { "are" } else { "'re" };
        return (base, Some(suffix));
    }

    // -'d (2-char pattern — check after 3-char patterns above so it
    // doesn't preempt them; harmless in practice because the 3-char
    // patterns start with different terminal letters, but stable order
    // makes the algorithm easier to reason about).
    if let Some(base_end) = match_suffix(word, &['\'', 'd'])
        && base_end > 0
    {
        let base = &word[..base_end];
        let suffix = if cfg.normalize_d { "would" } else { "'d" };
        return (base, Some(suffix));
    }

    // -'s (ambiguous — no normalization flag; treated as-is)
    if let Some(base_end) = match_suffix(word, &['\'', 's'])
        && base_end > 0
    {
        let base = &word[..base_end];
        return (base, Some("'s"));
    }

    // -'m (no normalization flag; treated as-is)
    if let Some(base_end) = match_suffix(word, &['\'', 'm'])
        && base_end > 0
    {
        let base = &word[..base_end];
        return (base, Some("'m"));
    }

    (word, None)
}

/// Is `c` an apostrophe? Accepts the ASCII `'` (U+0027) *and* the
/// typographic right-single-quote `’` (U+2019).
#[inline]
fn is_apostrophe(c: char) -> bool {
    c == '\'' || c == '\u{2019}'
}

/// Match `word` against `pattern` from the tail.
///
/// Returns the byte offset in `word` at which the matched suffix
/// begins, or `None` if the pattern does not match. Each `pattern`
/// character is compared case-insensitively (ASCII fold); a `'` in the
/// pattern matches either apostrophe form. When `pattern` is longer
/// than the character count of `word`, the match fails.
fn match_suffix(word: &str, pattern: &[char]) -> Option<usize> {
    let mut end = word.len();
    for &pat_char in pattern.iter().rev() {
        let head = word.get(..end)?;
        let (idx, ch) = head.char_indices().next_back()?;
        let matches = if pat_char == '\'' {
            is_apostrophe(ch)
        } else {
            ch.eq_ignore_ascii_case(&pat_char)
        };
        if !matches {
            return None;
        }
        end = idx;
    }
    Some(end)
}

/// Case-insensitive full-word equality with apostrophe-form tolerance.
///
/// `canonical` uses the ASCII apostrophe; `word` may use either the
/// ASCII apostrophe or U+2019.
fn eq_ci_apostrophe(word: &str, canonical: &str) -> bool {
    let mut wi = word.chars();
    let mut ci = canonical.chars();
    loop {
        match (wi.next(), ci.next()) {
            (None, None) => return true,
            (None, _) | (_, None) => return false,
            (Some(w), Some(c)) => {
                let ok = if c == '\'' {
                    is_apostrophe(w)
                } else {
                    w.eq_ignore_ascii_case(&c)
                };
                if !ok {
                    return false;
                }
            }
        }
    }
}

/// The [`STANDARD`](ContractionTokenizer::STANDARD) tokenizer, exposed
/// as a `const` so callers can name it directly.
///
/// Wire into an [`English`](crate::English) pack via
/// [`English::with_contraction_tokenizer`](crate::English::with_contraction_tokenizer),
/// or reach for the [`ENGLISH_WITH_CONTRACTIONS`](crate::ENGLISH_WITH_CONTRACTIONS)
/// pre-configured singleton.
pub const CONTRACTION_TOKENIZER: ContractionTokenizer = ContractionTokenizer::STANDARD;

#[cfg(test)]
mod tests {
    extern crate alloc;

    use super::*;
    use alloc::vec::Vec;

    // ---- helpers ----------------------------------------------------

    fn collect_standard(input: &str) -> Vec<String> {
        ContractionTokenizer::STANDARD.tokenize(input).collect()
    }

    fn collect_normalized(input: &str) -> Vec<String> {
        ContractionTokenizer::NORMALIZED.tokenize(input).collect()
    }

    // ---- empty / non-contraction inputs -----------------------------

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect_standard("").is_empty());
        assert!(collect_normalized("").is_empty());
    }

    #[test]
    fn plain_words_unchanged() {
        assert_eq!(collect_standard("hello world"), ["hello", "world"]);
        assert_eq!(collect_normalized("hello world"), ["hello", "world"]);
    }

    // ---- -n't -------------------------------------------------------

    #[test]
    fn nt_standard_preserves_fragment() {
        assert_eq!(collect_standard("don't"), ["do", "n't"]);
        assert_eq!(collect_standard("isn't"), ["is", "n't"]);
        assert_eq!(collect_standard("aren't"), ["are", "n't"]);
        assert_eq!(collect_standard("didn't"), ["did", "n't"]);
        assert_eq!(collect_standard("doesn't"), ["does", "n't"]);
        assert_eq!(collect_standard("hadn't"), ["had", "n't"]);
        assert_eq!(collect_standard("hasn't"), ["has", "n't"]);
        assert_eq!(collect_standard("haven't"), ["have", "n't"]);
        assert_eq!(collect_standard("wasn't"), ["was", "n't"]);
        assert_eq!(collect_standard("weren't"), ["were", "n't"]);
        assert_eq!(collect_standard("couldn't"), ["could", "n't"]);
        assert_eq!(collect_standard("shouldn't"), ["should", "n't"]);
        assert_eq!(collect_standard("wouldn't"), ["would", "n't"]);
        assert_eq!(collect_standard("mustn't"), ["must", "n't"]);
        assert_eq!(collect_standard("needn't"), ["need", "n't"]);
    }

    #[test]
    fn nt_normalized_expands_to_not() {
        assert_eq!(collect_normalized("don't"), ["do", "not"]);
        assert_eq!(collect_normalized("isn't"), ["is", "not"]);
        assert_eq!(collect_normalized("aren't"), ["are", "not"]);
    }

    // ---- -'ll -------------------------------------------------------

    #[test]
    fn ll_standard_preserves_fragment() {
        assert_eq!(collect_standard("I'll"), ["I", "'ll"]);
        assert_eq!(collect_standard("you'll"), ["you", "'ll"]);
        assert_eq!(collect_standard("he'll"), ["he", "'ll"]);
        assert_eq!(collect_standard("she'll"), ["she", "'ll"]);
        assert_eq!(collect_standard("it'll"), ["it", "'ll"]);
        assert_eq!(collect_standard("we'll"), ["we", "'ll"]);
        assert_eq!(collect_standard("they'll"), ["they", "'ll"]);
    }

    #[test]
    fn ll_normalized_expands_to_will() {
        assert_eq!(collect_normalized("I'll"), ["I", "will"]);
        assert_eq!(collect_normalized("she'll"), ["she", "will"]);
    }

    // ---- -'ve -------------------------------------------------------

    #[test]
    fn ve_standard_preserves_fragment() {
        assert_eq!(collect_standard("I've"), ["I", "'ve"]);
        assert_eq!(collect_standard("you've"), ["you", "'ve"]);
        assert_eq!(collect_standard("could've"), ["could", "'ve"]);
        assert_eq!(collect_standard("might've"), ["might", "'ve"]);
    }

    #[test]
    fn ve_normalized_expands_to_have() {
        assert_eq!(collect_normalized("I've"), ["I", "have"]);
        assert_eq!(collect_normalized("could've"), ["could", "have"]);
    }

    // ---- -'re -------------------------------------------------------

    #[test]
    fn re_standard_preserves_fragment() {
        assert_eq!(collect_standard("you're"), ["you", "'re"]);
        assert_eq!(collect_standard("we're"), ["we", "'re"]);
        assert_eq!(collect_standard("they're"), ["they", "'re"]);
    }

    #[test]
    fn re_normalized_expands_to_are() {
        assert_eq!(collect_normalized("you're"), ["you", "are"]);
        assert_eq!(collect_normalized("they're"), ["they", "are"]);
    }

    // ---- -'d --------------------------------------------------------

    #[test]
    fn d_standard_preserves_fragment() {
        assert_eq!(collect_standard("I'd"), ["I", "'d"]);
        assert_eq!(collect_standard("she'd"), ["she", "'d"]);
        assert_eq!(collect_standard("they'd"), ["they", "'d"]);
    }

    #[test]
    fn d_normalized_expands_to_would() {
        // Ambiguous ('d could be `had` or `would`); heuristic picks
        // `would`.
        assert_eq!(collect_normalized("I'd"), ["I", "would"]);
        assert_eq!(collect_normalized("they'd"), ["they", "would"]);
    }

    // ---- -'s (ambiguous, always as-is) ------------------------------

    #[test]
    fn s_treated_as_is_in_both_modes() {
        // -'s is ambiguous (is/has); both modes keep the fragment.
        assert_eq!(collect_standard("he's"), ["he", "'s"]);
        assert_eq!(collect_normalized("he's"), ["he", "'s"]);
        assert_eq!(collect_standard("that's"), ["that", "'s"]);
        assert_eq!(collect_normalized("here's"), ["here", "'s"]);
    }

    // ---- -'m --------------------------------------------------------

    #[test]
    fn m_treated_as_is_in_both_modes() {
        assert_eq!(collect_standard("I'm"), ["I", "'m"]);
        assert_eq!(collect_normalized("I'm"), ["I", "'m"]);
    }

    // ---- special forms ---------------------------------------------

    #[test]
    fn special_won_t_standard_rewrites_will() {
        assert_eq!(collect_standard("won't"), ["will", "n't"]);
    }

    #[test]
    fn special_won_t_normalized_rewrites_will_not() {
        assert_eq!(collect_normalized("won't"), ["will", "not"]);
    }

    #[test]
    fn special_can_t_standard_rewrites_can_nt() {
        assert_eq!(collect_standard("can't"), ["can", "n't"]);
    }

    #[test]
    fn special_can_t_normalized_rewrites_can_not() {
        assert_eq!(collect_normalized("can't"), ["can", "not"]);
    }

    #[test]
    fn special_shan_t_standard_rewrites_shall_nt() {
        assert_eq!(collect_standard("shan't"), ["shall", "n't"]);
    }

    #[test]
    fn special_shan_t_normalized_rewrites_shall_not() {
        assert_eq!(collect_normalized("shan't"), ["shall", "not"]);
    }

    #[test]
    fn special_gonna_rewrites_going_to() {
        assert_eq!(collect_standard("gonna"), ["going", "to"]);
        assert_eq!(collect_normalized("gonna"), ["going", "to"]);
    }

    #[test]
    fn special_wanna_rewrites_want_to() {
        assert_eq!(collect_standard("wanna"), ["want", "to"]);
    }

    #[test]
    fn special_gotta_rewrites_got_to() {
        assert_eq!(collect_standard("gotta"), ["got", "to"]);
    }

    #[test]
    fn special_forms_are_case_insensitive() {
        assert_eq!(collect_standard("WON'T"), ["will", "n't"]);
        assert_eq!(collect_standard("Can't"), ["can", "n't"]);
        assert_eq!(collect_standard("Gonna"), ["going", "to"]);
    }

    #[test]
    fn special_forms_disabled_falls_through_to_nt() {
        let cfg = ContractionTokenizer::STANDARD.with_special_forms(false);
        let toks: Vec<String> = cfg.tokenize("won't").collect();
        // Without special_forms, "won't" is treated as a generic -n't
        // and yields ["wo", "n't"] — morphologically wrong, but the
        // documented fallback.
        assert_eq!(toks, ["wo", "n't"]);
    }

    // ---- typographic apostrophe ------------------------------------

    #[test]
    fn typographic_apostrophe_accepted() {
        assert_eq!(collect_standard("don\u{2019}t"), ["do", "n't"]);
        assert_eq!(collect_standard("I\u{2019}ll"), ["I", "'ll"]);
        assert_eq!(collect_standard("won\u{2019}t"), ["will", "n't"]);
    }

    // ---- multi-word sentences --------------------------------------

    #[test]
    fn splits_full_sentence() {
        assert_eq!(
            collect_standard("I don't think she'll go."),
            ["I", "do", "n't", "think", "she", "'ll", "go"],
        );
    }

    #[test]
    fn normalized_full_sentence() {
        assert_eq!(
            collect_normalized("I don't think she'll go."),
            ["I", "do", "not", "think", "she", "will", "go"],
        );
    }

    // ---- round-trip: STANDARD is idempotent ------------------------

    #[test]
    fn standard_round_trips_with_space_join() {
        for text in [
            "I don't know.",
            "won't happen",
            "she'll be there",
            "we're not going",
            "I've had enough",
            "they'd better",
            "I'm here",
            "he's fine",
        ] {
            let toks1: Vec<String> = collect_standard(text);
            let joined = toks1.join(" ");
            let toks2: Vec<String> = collect_standard(&joined);
            assert_eq!(toks1, toks2, "round-trip failed for {text:?}");
        }
    }

    // ---- borrowed API ----------------------------------------------

    #[test]
    fn tokenize_borrowed_yields_static_or_input_slices() {
        let text = "I don't know";
        let toks: Vec<&str> = ContractionTokenizer::STANDARD
            .tokenize_borrowed(text)
            .collect();
        assert_eq!(toks, ["I", "do", "n't", "know"]);
    }

    #[test]
    fn tokenize_borrowed_and_tokenize_agree() {
        for text in ["hello", "don't", "won't", "I'll go", "gonna"] {
            let borrowed: Vec<&str> = ContractionTokenizer::STANDARD
                .tokenize_borrowed(text)
                .collect();
            let owned: Vec<String> = collect_standard(text);
            assert_eq!(borrowed.len(), owned.len());
            for (b, o) in borrowed.iter().zip(owned.iter()) {
                assert_eq!(*b, o.as_str());
            }
        }
    }

    // ---- non-contraction apostrophe words --------------------------

    #[test]
    fn possessives_are_split_off_via_s() {
        // The tokenizer treats -'s as ambiguous and always splits.
        assert_eq!(collect_standard("John's book"), ["John", "'s", "book"]);
    }

    #[test]
    fn oclock_stays_together_after_no_split() {
        // "o'clock" doesn't match any suffix pattern, so it stays as
        // a single raw token.
        assert_eq!(collect_standard("o'clock"), ["o'clock"]);
    }

    // ---- token construction / config -------------------------------

    #[test]
    fn new_equals_standard() {
        assert_eq!(ContractionTokenizer::new(), ContractionTokenizer::STANDARD);
        assert_eq!(
            ContractionTokenizer::default(),
            ContractionTokenizer::STANDARD
        );
    }

    #[test]
    fn builder_toggles_individual_flags() {
        let c = ContractionTokenizer::STANDARD.with_normalize_nt(true);
        assert!(c.normalize_nt);
        assert!(!c.normalize_ll);
        let toks: Vec<String> = c.tokenize("don't").collect();
        assert_eq!(toks, ["do", "not"]);
    }

    // ---- suffix matcher direct checks ------------------------------

    #[test]
    fn match_suffix_recognizes_nt() {
        assert_eq!(match_suffix("don't", &['n', '\'', 't']), Some(2));
        assert_eq!(match_suffix("isn't", &['n', '\'', 't']), Some(2));
        assert_eq!(match_suffix("hello", &['n', '\'', 't']), None);
        // Shorter than pattern.
        assert_eq!(match_suffix("t", &['n', '\'', 't']), None);
        // Empty.
        assert_eq!(match_suffix("", &['n', '\'', 't']), None);
    }

    #[test]
    fn match_suffix_handles_typographic_apostrophe() {
        assert_eq!(match_suffix("don\u{2019}t", &['n', '\'', 't']), Some(2),);
    }

    // ---- eq_ci_apostrophe direct checks ----------------------------

    #[test]
    fn eq_ci_apostrophe_matches_case_variants() {
        assert!(eq_ci_apostrophe("won't", "won't"));
        assert!(eq_ci_apostrophe("WON'T", "won't"));
        assert!(eq_ci_apostrophe("Won\u{2019}t", "won't"));
        assert!(!eq_ci_apostrophe("cant", "can't"));
        assert!(!eq_ci_apostrophe("won", "won't"));
    }
}
