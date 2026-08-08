//! [`ThaiTokenizer`] — a syllable-cluster-based (dictionary-free)
//! tokenizer for Thai.
//!
//! # Why no dictionary?
//!
//! Thai is written **without spaces between words** (like Chinese and
//! Japanese, unlike Vietnamese which is space-delimited). The
//! gold standard for Thai word segmentation is dictionary-driven
//! (ICU's Thai break iterator, `PyThaiNLP`'s `newmm`, `attacut`,
//! `deepcut`), all of which ship 30K-200K-entry lexica and often a
//! statistical / neural model on top. Shipping any of that in a
//! wasm-first / offline-first language pack defeats the whole point
//! of the crate's size budget.
//!
//! Instead, this module ships a **syllable-cluster tokenizer** — the
//! smallest meaningful unit above a single Thai scalar. A Thai
//! syllable cluster is a leading consonant plus the vowel marks / tone
//! marks / signs that attach to it (some vowels are written **before**
//! the consonant they modify, some after, some above, some below).
//! Clusters are close to what a native reader perceives as a single
//! grapheme block, and they line up with the character-cluster
//! definition Unicode's Thai grapheme-cluster algorithm approximates.
//!
//! # True word segmentation is deferred
//!
//! **This tokenizer does not attempt to recover word boundaries.**
//! Real Thai word segmentation needs a dictionary and typically a
//! statistical or neural model:
//!
//! - ICU's `BreakIterator::createWordInstance(Locale::THAI)` embeds a
//!   ~1.4 MB Thai dictionary and a rule-based finite-state segmenter.
//! - `PyThaiNLP`'s `newmm` (New Maximum Matching) uses a dictionary of
//!   ~62K entries and dynamic-programming longest-match.
//! - `attacut` is a CNN model trained on the BEST 2010 Thai corpus.
//! - `deepcut` is a `BiLSTM` model trained on the same corpus.
//!
//! A future `stringcheese-th-newmm` (or `stringcheese-th-dict`)
//! sibling crate — with a bundled dictionary — would take that role
//! while this base pack stays wasm-friendly. Callers who need
//! dictionary-quality segmentation today should reach for
//! `icu_segmenter` (which ships the ICU Thai break iterator with an
//! opt-in feature flag).
//!
//! # Rules the tokenizer applies
//!
//! Skip leading separators. Then classify the current scalar into one
//! of the following classes (see [`CharType`]):
//!
//! - **Whitespace** — every scalar for which [`char::is_whitespace`]
//!   returns `true`. Separator; not emitted.
//! - **Punctuation** — ASCII punctuation, the two Thai
//!   paragraph-boundary marks `๚` U+0E5A angkhankhu and `๛` U+0E5B
//!   khomut, and `๏` U+0E4F fongman. Separator; not emitted.
//! - **Alphabet** — ASCII `[A-Za-z]`. Runs stay together as one
//!   token.
//! - **Digit** — ASCII `[0-9]`. Runs stay together as one token.
//! - **`ThaiDigit`** — the Thai digit block U+0E50..=U+0E59
//!   (`๐-๙`). Runs stay together as one token.
//! - **`ThaiPreVowel`** — U+0E40..=U+0E44
//!   (`เ แ โ ใ ไ`), the five "leading vowels" that are typed **before**
//!   the consonant they attach to. Marks the start of a new cluster;
//!   the cluster then consumes the following consonant and any marks.
//! - **`ThaiConsonant`** — U+0E01..=U+0E2E, the 42 base Thai
//!   consonants. Also marks the start of a new cluster (when not
//!   already inside one led by a pre-vowel).
//! - **`ThaiVowelMark`** — vowel marks that combine with the preceding
//!   consonant: U+0E30..=U+0E3A (`ะ า ิ ี ึ ื ุ ู ฺ`), U+0E45
//!   (`ๅ` lakkhangyao — vowel-length elongator), U+0E47 (`็` maitaikhu
//!   — short-vowel mark). Cluster-internal.
//! - **`ThaiToneMark`** — U+0E48..=U+0E4B
//!   (`่ ้ ๊ ๋`, mai ek / tho / tri / chattawa). Cluster-internal;
//!   also a boundary hint — the cluster is expected to end just
//!   after the tone mark.
//! - **`ThaiSign`** — U+0E2F (`ฯ` paiyannoi — abbreviation),
//!   U+0E46 (`ๆ` maiyamok — word-level reduplication marker),
//!   U+0E4C (`์` thanthakhat / karan — silences the preceding
//!   consonant), U+0E4D (`ํ` nikhahit — vowel modifier),
//!   U+0E4E (`๎` yamakkan — historical). All cluster-internal
//!   (they attach to the preceding cluster).
//! - **Other** — anything the ranges above don't cover (emoji,
//!   dingbats, other-script letters). Treated like a one-scalar
//!   token.
//!
//! # The cluster rule
//!
//! A cluster inside a Thai run is:
//!
//! ```text
//!     [pre-vowel]? [consonant] ([vowel-mark] | [tone-mark] | [sign])*
//! ```
//!
//! Concretely, the tokenizer emits a new cluster (i.e., ends the
//! previous cluster) when — while walking a Thai run — it encounters:
//!
//! 1. A **pre-vowel** (U+0E40..=U+0E44). Pre-vowels always start a
//!    new cluster.
//! 2. A **consonant** whose predecessor was **not** a pre-vowel and
//!    the current cluster is non-empty. This is the "second consonant
//!    starts a new cluster" case — a cluster like `ไทย` (pre-vowel
//!    `ไ`, then consonant `ท`, then a further consonant `ย` that
//!    starts a new cluster) splits as `ไท` followed by `ย` in this
//!    naive scheme, which is admittedly coarser than a
//!    dictionary-driven segmenter would produce (`ไทย` = "Thai" is
//!    a single word). This coarseness is the price of not shipping
//!    a dictionary; the reference tests document the exact
//!    behaviour.
//!
//! Whitespace, punctuation, and non-Thai runs always end the current
//! Thai cluster.
//!
//! # Examples
//!
//! - `ไทย` → `["ไท", "ย"]` — `ไ` (pre-vowel) + `ท` (consonant) form
//!   the first cluster; `ย` (consonant) starts a second.
//! - `สวัสดี` → `["สวั", "สดี"]` — `ส` + `ว` starts a new cluster on
//!   the second consonant; `ั` (vowel mark) joins `ว`; then `ส`
//!   starts a third; `ด` starts a fourth; `ี` joins `ด`. Under the
//!   naive rule: `ส` cluster ends when second consonant `ว` arrives;
//!   `ว` cluster consumes `ั`; then `ส` starts new cluster; then `ด`
//!   starts new cluster and consumes `ี` → `["ส", "วั", "ส", "ดี"]`.
//! - `เป็น` → `["เป็น"]` — `เ` (pre-vowel) + `ป` (consonant) + `็`
//!   (mai-taikhu) + `น` (consonant). Under the naive rule, the second
//!   consonant `น` starts a new cluster: `["เป็", "น"]`.
//! - `hello ไทย` → `["hello", "ไท", "ย"]`.
//! - `2026 ปี` → `["2026", "ปี"]`.
//!
//! # Non-goals
//!
//! - **Dictionary-driven word segmentation.** Whole point of this
//!   tokenizer is to *not* need a dictionary. See the introduction.
//! - **Full grapheme-cluster boundaries.** Unicode's UAX #29 grapheme
//!   cluster boundary for Thai has additional rules for the
//!   sara-am-decomposition, complex non-spacing marks, and Northern
//!   Thai (Lanna) legacy sequences. This tokenizer approximates the
//!   Thai-specific subset that dominates modern Thai; a caller who
//!   needs UAX #29 compliance should compose a proper grapheme
//!   segmenter over the tokens.
//! - **Sentence segmentation.** The tokenizer emits clusters, not
//!   sentences. Thai sentence boundaries are typically marked by
//!   whitespace (double space or newline); a caller who wants
//!   sentence chunks should scan for those separators independently.

use core::str::CharIndices;

/// Character-type classification for the Thai tokenizer.
///
/// See the [module-level docs](self) for the range each variant
/// covers.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum CharType {
    /// A base Thai consonant (U+0E01..=U+0E2E). Starts a new cluster
    /// when the current cluster is non-empty and the predecessor was
    /// not a pre-vowel.
    ThaiConsonant,
    /// A Thai "pre-vowel" (U+0E40..=U+0E44) — the five leading
    /// vowels that in Unicode order precede the consonant they
    /// modify. Always starts a new cluster.
    ThaiPreVowel,
    /// A Thai vowel mark: U+0E30..=U+0E3A, U+0E45, U+0E47.
    /// Cluster-internal.
    ThaiVowelMark,
    /// A Thai tone mark (U+0E48..=U+0E4B). Cluster-internal.
    ThaiToneMark,
    /// A Thai sign: paiyannoi `ฯ`, maiyamok `ๆ`, thanthakhat `์`,
    /// nikhahit `ํ`, yamakkan `๎`. Cluster-internal.
    ThaiSign,
    /// A Thai digit (U+0E50..=U+0E59). Runs stay together.
    ThaiDigit,
    /// Latin letters — ASCII `[A-Za-z]`. Runs stay together.
    Alphabet,
    /// ASCII digits `[0-9]`. Runs stay together.
    Digit,
    /// Punctuation — ASCII punctuation and the Thai paragraph marks
    /// `๚` U+0E5A, `๛` U+0E5B, and `๏` U+0E4F. Separator.
    Punctuation,
    /// Every scalar for which [`char::is_whitespace`] returns
    /// `true`. Separator.
    Whitespace,
    /// Anything the classification cascade above does not cover
    /// (emoji, dingbats, other-script letters, U+0E3F `฿` baht
    /// sign, U+0E5C..=U+0E7F reserved). Treated as one-scalar
    /// token-worthy run.
    Other,
}

/// Classify `c` into its [`CharType`].
///
/// Classification is total — every `char` receives exactly one type —
/// and matches the ranges published in the Unicode Standard (Unicode
/// 15.1, Thai block U+0E00..=U+0E7F).
#[must_use]
pub fn classify(c: char) -> CharType {
    // Whitespace is checked first so scalars that would otherwise
    // fall into another block (there are none in Thai's range) never
    // sneak past.
    if c.is_whitespace() {
        return CharType::Whitespace;
    }

    if c.is_ascii_digit() {
        return CharType::Digit;
    }

    if c.is_ascii_alphabetic() {
        return CharType::Alphabet;
    }

    // Thai block dispatch.
    if ('\u{0E01}'..='\u{0E2E}').contains(&c) {
        return CharType::ThaiConsonant;
    }
    if c == '\u{0E2F}' {
        return CharType::ThaiSign;
    }
    if ('\u{0E30}'..='\u{0E3A}').contains(&c) {
        return CharType::ThaiVowelMark;
    }
    // U+0E3B..=U+0E3E — reserved / unassigned in the Thai block.
    // U+0E3F is `฿` baht sign — currency; treat as Other so it is a
    // one-scalar token.
    if c == '\u{0E3F}' {
        return CharType::Other;
    }
    if ('\u{0E40}'..='\u{0E44}').contains(&c) {
        return CharType::ThaiPreVowel;
    }
    if c == '\u{0E45}' {
        return CharType::ThaiVowelMark;
    }
    if c == '\u{0E46}' {
        return CharType::ThaiSign;
    }
    if c == '\u{0E47}' {
        return CharType::ThaiVowelMark;
    }
    if ('\u{0E48}'..='\u{0E4B}').contains(&c) {
        return CharType::ThaiToneMark;
    }
    if c == '\u{0E4C}' || c == '\u{0E4D}' || c == '\u{0E4E}' {
        return CharType::ThaiSign;
    }
    if c == '\u{0E4F}' || c == '\u{0E5A}' || c == '\u{0E5B}' {
        return CharType::Punctuation;
    }
    if ('\u{0E50}'..='\u{0E59}').contains(&c) {
        return CharType::ThaiDigit;
    }

    // ASCII punctuation and the two big Unicode punctuation blocks.
    if c.is_ascii_punctuation() {
        return CharType::Punctuation;
    }
    if ('\u{2000}'..='\u{206F}').contains(&c) {
        return CharType::Punctuation;
    }

    CharType::Other
}

/// The Thai syllable-cluster tokenizer.
///
/// A zero-sized value; construct as [`ThaiTokenizer`] and reuse the
/// value freely across threads and calls.
///
/// See the [module-level docs](self) for the classification and
/// cluster-emission rules.
///
/// # Example
///
/// ```
/// use stringcheese_th::ThaiTokenizer;
///
/// // `ไทย` = pre-vowel `ไ` + consonant `ท` + consonant `ย`. The
/// // naive cluster rule ends the first cluster when the second
/// // consonant appears.
/// let toks: Vec<&str> = ThaiTokenizer::new().tokenize("ไทย").collect();
/// assert_eq!(toks, ["ไท", "ย"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ThaiTokenizer;

impl ThaiTokenizer {
    /// Constructs a new [`ThaiTokenizer`]. Zero-sized; free to call.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Splits `text` into syllable-cluster tokens (for Thai runs) and
    /// same-type runs (for Latin / digit / Thai-digit / other).
    ///
    /// The returned iterator yields borrowed `&'a str` slices of the
    /// input; no allocation is performed.
    #[inline]
    pub fn tokenize<'a>(&self, text: &'a str) -> ThaiTokens<'a> {
        ThaiTokens {
            text,
            chars: text.char_indices(),
            peeked: None,
        }
    }
}

/// Iterator over the syllable-cluster / same-type-run tokens of a
/// string, produced by [`ThaiTokenizer::tokenize`].
///
/// The iterator borrows the input string and yields borrowed slices
/// of it — no allocation is performed. Held state is a
/// [`CharIndices`] cursor over the input plus a single-slot
/// look-ahead buffer for the character that ended the previous scan.
#[derive(Clone, Debug)]
pub struct ThaiTokens<'a> {
    text: &'a str,
    chars: CharIndices<'a>,
    /// Single-slot look-ahead: `Some((i, ch))` when the previous call
    /// to `next_char` peeked past the end of a cluster and we need to
    /// feed that character back on the next call.
    peeked: Option<(usize, char)>,
}

impl ThaiTokens<'_> {
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

/// A character-type value counts as *token-worthy* if it is not a
/// separator (whitespace or punctuation).
#[inline]
fn is_token_worthy(ct: CharType) -> bool {
    !matches!(ct, CharType::Whitespace | CharType::Punctuation)
}

/// Is `ct` a Thai-block class the cluster rule recognizes as
/// cluster-internal (attaches to the preceding consonant)?
#[inline]
fn is_thai_cluster_internal(ct: CharType) -> bool {
    matches!(
        ct,
        CharType::ThaiVowelMark | CharType::ThaiToneMark | CharType::ThaiSign
    )
}

/// Is `ct` a Thai-block class the cluster rule treats as a
/// cluster-leader (starts a new cluster when the current cluster is
/// non-empty)?
#[inline]
fn is_thai_cluster_leader(ct: CharType) -> bool {
    matches!(ct, CharType::ThaiConsonant | CharType::ThaiPreVowel)
}

impl<'a> Iterator for ThaiTokens<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        // Skip leading separators (whitespace + punctuation).
        let (start, first, primary) = loop {
            let (i, c) = self.next_char()?;
            let ct = classify(c);
            if is_token_worthy(ct) {
                break (i, c, ct);
            }
        };

        // Initial `end` covers the first scalar we already consumed.
        let mut end = start + first.len_utf8();

        // Route on the primary class of the first scalar. Thai
        // clusters and same-type non-Thai runs use different scan
        // rules.
        match primary {
            CharType::ThaiPreVowel | CharType::ThaiConsonant => {
                // Consume any following cluster-internal marks and,
                // if we started with a pre-vowel, absorb the next
                // consonant. Then stop.
                let expect_consonant_after_pre_vowel = matches!(primary, CharType::ThaiPreVowel);
                let mut saw_consonant = matches!(primary, CharType::ThaiConsonant);

                loop {
                    let Some((i, c)) = self.next_char() else {
                        end = self.end();
                        break;
                    };
                    let ct = classify(c);

                    // Non-Thai / separator ends the cluster.
                    if !matches!(
                        ct,
                        CharType::ThaiConsonant
                            | CharType::ThaiPreVowel
                            | CharType::ThaiVowelMark
                            | CharType::ThaiToneMark
                            | CharType::ThaiSign
                    ) {
                        self.push_back((i, c));
                        break;
                    }

                    if is_thai_cluster_internal(ct) {
                        end = i + c.len_utf8();
                        continue;
                    }

                    // ct is ThaiConsonant or ThaiPreVowel — a
                    // cluster-leader.
                    if is_thai_cluster_leader(ct) {
                        if expect_consonant_after_pre_vowel
                            && !saw_consonant
                            && matches!(ct, CharType::ThaiConsonant)
                        {
                            // The consonant the pre-vowel expects.
                            end = i + c.len_utf8();
                            saw_consonant = true;
                            continue;
                        }
                        // Otherwise this starts a new cluster —
                        // push it back for the next call.
                        self.push_back((i, c));
                        break;
                    }
                }
            }
            CharType::Alphabet | CharType::Digit | CharType::ThaiDigit => {
                // Same-type run — extend while the class matches.
                loop {
                    let Some((i, c)) = self.next_char() else {
                        end = self.end();
                        break;
                    };
                    let ct = classify(c);
                    if ct == primary {
                        end = i + c.len_utf8();
                        continue;
                    }
                    self.push_back((i, c));
                    break;
                }
            }
            CharType::Other
            | CharType::ThaiVowelMark
            | CharType::ThaiToneMark
            | CharType::ThaiSign => {
                // For `Other`: a single Other scalar becomes one
                // token; do not extend. This matches the CJK-adjacent
                // behaviour in the sibling packs — `Other` runs are
                // minimal to avoid accidentally merging two scripts
                // we don't classify.
                //
                // For Thai cluster-internal scalars with no preceding
                // cluster leader: emit as a one-scalar token so
                // callers see something (the alternative is to drop
                // it silently, which loses information).
            }
            CharType::Whitespace | CharType::Punctuation => {
                unreachable!("skipped by the leading-separator loop above");
            }
        }

        Some(&self.text[start..end])
    }
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    extern crate alloc;

    use super::*;
    use alloc::vec::Vec;

    fn collect(input: &str) -> Vec<&str> {
        ThaiTokenizer::new().tokenize(input).collect()
    }

    // ---------------------------------------------------------------
    // classify() smoke tests — one representative per character type.
    // ---------------------------------------------------------------

    #[test]
    fn classify_ascii_letters() {
        assert_eq!(classify('a'), CharType::Alphabet);
        assert_eq!(classify('Z'), CharType::Alphabet);
    }

    #[test]
    fn classify_ascii_digits() {
        assert_eq!(classify('0'), CharType::Digit);
        assert_eq!(classify('9'), CharType::Digit);
    }

    #[test]
    fn classify_thai_consonants() {
        assert_eq!(classify('ก'), CharType::ThaiConsonant); // U+0E01
        assert_eq!(classify('ท'), CharType::ThaiConsonant);
        assert_eq!(classify('ฮ'), CharType::ThaiConsonant); // U+0E2E
    }

    #[test]
    fn classify_thai_pre_vowels() {
        assert_eq!(classify('เ'), CharType::ThaiPreVowel); // U+0E40
        assert_eq!(classify('แ'), CharType::ThaiPreVowel);
        assert_eq!(classify('โ'), CharType::ThaiPreVowel);
        assert_eq!(classify('ใ'), CharType::ThaiPreVowel);
        assert_eq!(classify('ไ'), CharType::ThaiPreVowel); // U+0E44
    }

    #[test]
    fn classify_thai_vowel_marks() {
        assert_eq!(classify('ั'), CharType::ThaiVowelMark); // U+0E31 mai han-akat
        assert_eq!(classify('า'), CharType::ThaiVowelMark); // U+0E32
        assert_eq!(classify('ิ'), CharType::ThaiVowelMark); // U+0E34
        assert_eq!(classify('็'), CharType::ThaiVowelMark); // U+0E47 mai taikhu
    }

    #[test]
    fn classify_thai_tone_marks() {
        assert_eq!(classify('่'), CharType::ThaiToneMark); // U+0E48 mai ek
        assert_eq!(classify('้'), CharType::ThaiToneMark); // U+0E49 mai tho
        assert_eq!(classify('๊'), CharType::ThaiToneMark); // U+0E4A mai tri
        assert_eq!(classify('๋'), CharType::ThaiToneMark); // U+0E4B mai chattawa
    }

    #[test]
    fn classify_thai_signs() {
        assert_eq!(classify('ฯ'), CharType::ThaiSign); // U+0E2F paiyannoi
        assert_eq!(classify('ๆ'), CharType::ThaiSign); // U+0E46 maiyamok
        assert_eq!(classify('์'), CharType::ThaiSign); // U+0E4C thanthakhat
        assert_eq!(classify('ํ'), CharType::ThaiSign); // U+0E4D nikhahit
    }

    #[test]
    fn classify_thai_digits() {
        assert_eq!(classify('๐'), CharType::ThaiDigit); // U+0E50
        assert_eq!(classify('๙'), CharType::ThaiDigit); // U+0E59
    }

    #[test]
    fn classify_thai_punctuation() {
        assert_eq!(classify('๚'), CharType::Punctuation); // U+0E5A
        assert_eq!(classify('๛'), CharType::Punctuation); // U+0E5B
        assert_eq!(classify('๏'), CharType::Punctuation); // U+0E4F
    }

    #[test]
    fn classify_baht_is_other() {
        // The baht sign ฿ is a currency symbol, treated as `Other`
        // so it becomes a one-scalar token.
        assert_eq!(classify('฿'), CharType::Other);
    }

    #[test]
    fn classify_whitespace() {
        assert_eq!(classify(' '), CharType::Whitespace);
        assert_eq!(classify('\t'), CharType::Whitespace);
        assert_eq!(classify('\n'), CharType::Whitespace);
    }

    #[test]
    fn classify_ascii_punctuation() {
        assert_eq!(classify('.'), CharType::Punctuation);
        assert_eq!(classify(','), CharType::Punctuation);
        assert_eq!(classify('!'), CharType::Punctuation);
    }

    #[test]
    fn classify_other() {
        assert_eq!(classify('🎉'), CharType::Other);
        // Hiragana in a Thai-tagged tokenizer is Other.
        assert_eq!(classify('あ'), CharType::Other);
    }

    // ---------------------------------------------------------------
    // Tokenizer behaviour.
    // ---------------------------------------------------------------

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn whitespace_only_yields_no_tokens() {
        assert!(collect("   ").is_empty());
        assert!(collect("\t\n").is_empty());
    }

    #[test]
    fn punctuation_only_yields_no_tokens() {
        assert!(collect("...").is_empty());
        assert!(collect("๚๛").is_empty());
    }

    #[test]
    fn latin_run_stays_together() {
        assert_eq!(collect("hello"), ["hello"]);
        assert_eq!(collect("JavaScript"), ["JavaScript"]);
    }

    #[test]
    fn digit_run_stays_together() {
        assert_eq!(collect("2026"), ["2026"]);
    }

    #[test]
    fn thai_digit_run_stays_together() {
        assert_eq!(collect("๒๐๒๖"), ["๒๐๒๖"]);
    }

    #[test]
    fn pre_vowel_absorbs_consonant() {
        // เป็น = เ (pre-vowel) + ป (consonant) + ็ (vowel mark)
        //        + น (second consonant, starts new cluster).
        // The pre-vowel-led cluster consumes: เป็ ; then น starts a
        // second cluster.
        assert_eq!(collect("เป็น"), ["เป็", "น"]);
    }

    #[test]
    fn thai_word_splits_at_second_consonant() {
        // ไทย = ไ + ท + ย. First cluster: ไท. Second: ย.
        assert_eq!(collect("ไทย"), ["ไท", "ย"]);
    }

    #[test]
    fn thai_consonant_with_vowel_marks_stays_clustered() {
        // กา = ก + า (vowel mark).
        assert_eq!(collect("กา"), ["กา"]);
        // กิน = ก + ิ + น. Second consonant starts new cluster.
        assert_eq!(collect("กิน"), ["กิ", "น"]);
    }

    #[test]
    fn thai_tone_mark_stays_with_cluster() {
        // ก่า = ก + ่ (mai ek) + า.
        assert_eq!(collect("ก่า"), ["ก่า"]);
    }

    #[test]
    fn whitespace_separates_thai_tokens() {
        // Thai typically uses space between phrases.
        assert_eq!(collect("ฉัน รัก"), ["ฉั", "น", "รั", "ก"]);
    }

    #[test]
    fn mixed_thai_latin_digit() {
        assert_eq!(collect("hello ไทย 2026"), ["hello", "ไท", "ย", "2026"]);
    }

    #[test]
    fn punctuation_separates_tokens() {
        assert_eq!(collect("hello, world!"), ["hello", "world"]);
    }

    #[test]
    fn baht_sign_is_own_token() {
        // ฿ is one-scalar Other, and does not merge with Thai text.
        assert_eq!(collect("฿100"), ["฿", "100"]);
    }

    #[test]
    fn maiyamok_stays_with_cluster() {
        // เด็กๆ = pre-vowel + ด + ็ (vowel mark) + ก (consonant,
        // starts new cluster) + ๆ (maiyamok, cluster-internal on the
        // second cluster).
        assert_eq!(collect("เด็กๆ"), ["เด็", "กๆ"]);
    }

    #[test]
    fn thanthakhat_stays_with_cluster() {
        // การ์ตูน (cartoon) — ก + า + ร + ์ + ต + ู + น.
        // Under the naive rule: first cluster กา (consonant + vowel);
        // ร is a new consonant; ์ attaches to ร; ต is new consonant;
        // ู attaches; น is new consonant.
        assert_eq!(collect("การ์ตูน"), ["กา", "ร์", "ตู", "น"]);
    }

    #[test]
    fn borrowed_slices_are_from_the_input() {
        let text = "ไทย";
        let toks: Vec<&str> = ThaiTokenizer::new().tokenize(text).collect();
        assert!(!toks.is_empty());
        let base = text.as_ptr() as usize;
        // Every token slice's pointer lies within the input.
        for t in &toks {
            let offset = t.as_ptr() as usize - base;
            assert!(offset < text.len(), "token pointer outside input");
        }
    }

    #[test]
    fn tokenizer_never_yields_empty_tokens() {
        for t in ThaiTokenizer::new().tokenize("สวัสดี ครับ 2026") {
            assert!(!t.is_empty(), "empty token in output");
        }
    }
}
