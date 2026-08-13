//! Break-iteration capability for the StringCheese ICU-alternative
//! subsystem.
//!
//! Iterates Unicode grapheme cluster, word, and sentence boundaries
//! per [Unicode Standard Annex #29](https://www.unicode.org/reports/tr29/)
//! using per-locale data supplied through one or more
//! `stringcheese-scud` break packs, and exposes the result through
//! the `tegmentum:i18n-segment@0.1.0` WIT world. Callers construct a
//! [`BreakEngine`] from an optional [`BreakPack`] and issue
//! [`segment_graphemes`](BreakEngine::segment_graphemes) /
//! [`segment_words`](BreakEngine::segment_words) /
//! [`segment_sentences`](BreakEngine::segment_sentences) queries.
//!
//! # Position in the WIT-i18n subsystem
//!
//! Phase 5 of the WIT-i18n design (`docs/design/wit-i18n.md` § 8.5)
//! — the fifth capability delivered on top of the shared
//! `stringcheese-scud` loader after case-mapping (Phase 1),
//! collation (Phase 2), plural + number (Phase 3), and date/time
//! (Phase 4). Phase 5 ships a **locale-neutral default pack**: the
//! algorithm crate owns the UAX #29 default classification tables +
//! rule state machines, and the SCUD pack carries only the well-
//! known "use default rules" markers (see
//! [`stringcheese_scud::RULES_UAX29_DEFAULT`]). Locale-specific
//! tailorings (Japanese/Chinese word-break dictionaries, Thai/Lao/
//! Khmer syllable segmentation) are deferred to a follow-up.
//!
//! # WIT surface
//!
//! The WIT file at `component/wit/segment/stringcheese-icu-segment.wit`
//! defines four exports on the `segment-world` world:
//!
//! * `segment-graphemes(text)` → cluster boundary byte offsets.
//! * `segment-words(text, locale)` → word segment records with
//!   `is-word-like` flags.
//! * `segment-sentences(text, locale)` → sentence boundary byte
//!   offsets.
//! * `supported-locales()` / `supports(loc)` — introspection.
//!
//! # Line breaking is deferred
//!
//! Phase 5 covers UAX #29 (grapheme/word/sentence). UAX #14 (line
//! breaking) is a separate crate `stringcheese-icu-linebreak` in a
//! future phase — the rule sets are large enough that shipping both
//! at once would balloon the pack.
//!
//! # Rule numbering cross-reference
//!
//! The implementation follows the rule numbers from UAX #29 Section
//! 3 (grapheme), Section 4 (word), and Section 5 (sentence). Tests
//! are grouped by rule number so a reviewer can cross-reference the
//! spec directly.
//!
//! # Trust model
//!
//! Inherited from `stringcheese-scud`: SCUD packs are trusted
//! input. This crate does not defend against maliciously crafted
//! packs.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
use alloc::string::String;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use stringcheese_scud::{
    BreakDataView, GraphemeClass, ScudFile, SentenceClass, WordClass, WordDictView,
};

pub use stringcheese_scud::{RULES_UAX29_DEFAULT, ScudError};

pub mod classes;

// -----------------------------------------------------------------------
// Typed error surface
// -----------------------------------------------------------------------

/// Typed failure modes of the break engine. Mirrors the WIT
/// `segment-error` variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SegmentError {
    /// The locale tag was not a well-formed BCP 47 tag.
    InvalidLocale(&'static str),
}

// -----------------------------------------------------------------------
// Word-segment record
// -----------------------------------------------------------------------

/// One record from a word-segmentation walk.
///
/// `start` / `end` are UTF-8 byte offsets into the input; `end >=
/// start`. `is_word_like` distinguishes word-like runs (letters,
/// digits, joiners) from punctuation / whitespace segments.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct WordSegment {
    /// Inclusive UTF-8 byte offset where the segment starts.
    pub start: u32,
    /// Exclusive UTF-8 byte offset where the segment ends.
    pub end: u32,
    /// True iff the segment consists of word-like scalars.
    pub is_word_like: bool,
}

// -----------------------------------------------------------------------
// Pack + engine
// -----------------------------------------------------------------------

/// A loaded break-iteration pack for one BCP 47 locale (or the
/// root locale).
///
/// Wraps a validated [`ScudFile`] whose capability tag is
/// [`stringcheese_scud::CAP_BREAK`]. Cheap to clone.
#[derive(Debug, Clone, Copy)]
pub struct BreakPack<'a> {
    scud: ScudFile<'a>,
    locale: &'a str,
    data: BreakDataView<'a>,
}

impl<'a> BreakPack<'a> {
    /// Wrap a validated [`ScudFile`] as a break pack.
    ///
    /// # Errors
    ///
    /// Returns [`ScudError::CapabilityMismatch`] if the file's
    /// capability tag is not [`stringcheese_scud::CAP_BREAK`].
    pub fn new(scud: ScudFile<'a>) -> Result<Self, ScudError> {
        let data = scud.as_break_data()?;
        let locale = scud.locale().unwrap_or("");
        Ok(Self { scud, locale, data })
    }

    /// Parse `bytes` as a SCUD file and wrap it as a break pack.
    ///
    /// # Errors
    ///
    /// See [`ScudFile::from_slice`] and [`Self::new`].
    pub fn from_scud_bytes(bytes: &'a [u8]) -> Result<Self, ScudError> {
        let scud = ScudFile::from_slice(bytes)?;
        Self::new(scud)
    }

    /// The BCP 47 locale tag associated with this pack.
    #[must_use]
    pub fn locale(&self) -> &'a str {
        self.locale
    }

    /// The CLDR version the pack was generated from.
    #[must_use]
    pub fn cldr_version(&self) -> &'a str {
        self.scud.cldr_version()
    }

    /// Total byte length of the underlying SCUD file.
    #[must_use]
    pub fn scud_bytes_len(&self) -> usize {
        self.scud.len()
    }

    /// The zero-copy break data view.
    #[must_use]
    pub fn data(&self) -> &BreakDataView<'a> {
        &self.data
    }
}

/// Locale-sensitive break-iteration engine.
///
/// Holds an optional [`BreakPack`] whose class tables (if
/// populated) override the built-in classifier. The Phase 5 default
/// pack ships empty class sections, so a caller who loads only the
/// default pack gets pure algorithm-driven UAX #29 behaviour.
#[derive(Debug, Clone, Copy)]
pub struct BreakEngine<'a> {
    pack: Option<BreakPack<'a>>,
}

impl<'a> BreakEngine<'a> {
    /// Construct a fresh engine with no pack loaded — falls back to
    /// the algorithm crate's built-in UAX #29 default tables +
    /// rules for every query.
    #[must_use]
    pub const fn new() -> Self {
        Self { pack: None }
    }

    /// Construct an engine backed by a single pack. If the pack's
    /// class sections are populated they override the built-in
    /// classifier; empty sections leave the built-in classifier in
    /// place.
    #[must_use]
    pub const fn with_pack(pack: BreakPack<'a>) -> Self {
        Self { pack: Some(pack) }
    }

    /// The loaded pack, if any.
    #[must_use]
    pub const fn pack(&self) -> Option<&BreakPack<'a>> {
        self.pack.as_ref()
    }

    /// Grapheme-cluster boundaries per UAX #29 § 3. Returns UTF-8
    /// byte offsets: leading `0`, one boundary per cluster break,
    /// trailing `len(text)`. The list length equals `N + 1` where
    /// `N` is the number of clusters — except for empty input,
    /// which returns `[0]` (the trailing boundary coincides with
    /// the leading one and is elided).
    #[cfg(feature = "alloc")]
    #[must_use]
    pub fn segment_graphemes(&self, text: &str) -> Vec<u32> {
        if text.is_empty() {
            return alloc::vec![0];
        }
        collect_boundaries(GraphemeIter::new(text, self))
    }

    /// Word segments per UAX #29 § 4. Returns a list of
    /// [`WordSegment`] records covering the whole input contiguously
    /// (`segments[0].start == 0`, `segments.last().end ==
    /// len(text)`).
    ///
    /// # CJK dictionary tailoring
    ///
    /// When the loaded pack carries a
    /// [`WordDictView`] — the CJK word-break dictionary section —
    /// and the requested locale begins with `"ja"` or `"zh"`, the
    /// engine drives a
    /// **forward-maximum-match (FMM)** segmenter over each
    /// contiguous CJK-script run in the input: at every position it
    /// takes the longest dictionary entry that matches, or emits a
    /// single scalar if none matches. Non-CJK runs still fall
    /// through the UAX #29 default rules, so mixed CJK / Latin
    /// input (`私はJavaScriptを勉強します`) behaves the way callers
    /// expect.
    ///
    /// When no dictionary is loaded, or the locale does not match,
    /// segmentation is pure UAX #29 (each ideograph becomes its own
    /// word — the Phase 5 default).
    #[cfg(feature = "alloc")]
    #[must_use]
    pub fn segment_words(&self, text: &str, locale: &str) -> Vec<WordSegment> {
        // Dictionary-tailored path — Japanese / Chinese FMM.
        if let Some(dict) = self.pack.as_ref().and_then(|p| p.data.word_dict()) {
            if locale_wants_cjk_dict(locale) {
                return segment_words_with_dict(text, self, &dict);
            }
        }
        WordIter::new(text, self).collect()
    }

    /// Sentence boundaries per UAX #29 § 5. Returns UTF-8 byte
    /// offsets in the same shape as
    /// [`segment_graphemes`](Self::segment_graphemes).
    #[cfg(feature = "alloc")]
    #[must_use]
    pub fn segment_sentences(&self, text: &str, _locale: &str) -> Vec<u32> {
        if text.is_empty() {
            return alloc::vec![0];
        }
        collect_boundaries(SentenceIter::new(text, self))
    }

    /// Every BCP 47 locale tag this engine knows about. Phase 5's
    /// default pack advertises the root locale marker (`""`) and
    /// nothing else.
    #[cfg(feature = "alloc")]
    #[must_use]
    pub fn supported_locales(&self) -> Vec<&'a str> {
        match self.pack {
            Some(p) => alloc::vec![p.locale],
            None => alloc::vec![""],
        }
    }

    /// True iff a query in the given locale would use a locale-
    /// specific pack. Phase 5 always uses the default; returns
    /// `true` for every input for forward compatibility.
    #[must_use]
    pub const fn supports(&self, _locale: &str) -> bool {
        true
    }

    // ---- classifier delegation ----

    /// Grapheme-cluster class for `cp`, consulting the loaded pack
    /// first and falling back to the built-in classifier.
    #[must_use]
    pub fn grapheme_class(&self, cp: u32) -> GraphemeClass {
        if let Some(p) = &self.pack {
            if let Some(c) = p.data.grapheme_class(cp) {
                return c;
            }
        }
        classes::grapheme_class(cp)
    }

    /// Word-break class for `cp`.
    #[must_use]
    pub fn word_class(&self, cp: u32) -> WordClass {
        if let Some(p) = &self.pack {
            if let Some(c) = p.data.word_class(cp) {
                return c;
            }
        }
        classes::word_class(cp)
    }

    /// Sentence-break class for `cp`.
    #[must_use]
    pub fn sentence_class(&self, cp: u32) -> SentenceClass {
        if let Some(p) = &self.pack {
            if let Some(c) = p.data.sentence_class(cp) {
                return c;
            }
        }
        classes::sentence_class(cp)
    }
}

impl Default for BreakEngine<'_> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "alloc")]
fn collect_boundaries<I: Iterator<Item = u32>>(iter: I) -> Vec<u32> {
    iter.collect()
}

// -----------------------------------------------------------------------
// Grapheme cluster iterator (UAX #29 § 3)
// -----------------------------------------------------------------------

/// Iterator over UAX #29 grapheme cluster boundary byte offsets.
///
/// Yields the sequence `0, b_1, b_2, …, b_N, len(text)` where each
/// `b_i` marks the byte offset between two clusters. Rules
/// implemented: GB1-GB13 (line 20 UAX #29 rev 43). `GB9c` ("Extend
/// after `InCB=Consonant`") is approximated by GB9 for Phase 5;
/// Indic sequence tailoring lands with the follow-up rule-table
/// section.
pub struct GraphemeIter<'a, 'e> {
    text: &'a str,
    engine: &'e BreakEngine<'e>,
    /// Next byte offset to yield (0 initially, then boundary
    /// positions).
    cursor: usize,
    /// Whether we have already emitted the initial `0` (GB1).
    emitted_start: bool,
    /// Whether we have already emitted the trailing `len` (GB2).
    emitted_end: bool,
    /// RI parity counter — GB12/GB13 require an even count of RIs
    /// preceding the current position.
    ri_count: usize,
}

impl<'a, 'e> GraphemeIter<'a, 'e> {
    /// Fresh iterator over `text` under `engine`.
    #[must_use]
    pub fn new(text: &'a str, engine: &'e BreakEngine<'e>) -> Self {
        Self {
            text,
            engine,
            cursor: 0,
            emitted_start: false,
            emitted_end: false,
            ri_count: 0,
        }
    }

    /// Locate the next cluster boundary strictly greater than
    /// `self.cursor`. Returns `None` when the cursor has reached
    /// the input's end.
    fn advance_to_next_boundary(&mut self) -> Option<usize> {
        if self.cursor >= self.text.len() {
            return None;
        }
        let bytes = self.text.as_bytes();
        // Walk char-by-char applying GB rules; the first pair with a
        // break becomes the new cursor.
        let mut iter = self.text[self.cursor..].char_indices();
        let (_, first) = iter.next().unwrap();
        let first_class = self.engine.grapheme_class(first as u32);
        // Update RI parity: any non-RI resets to 0, RI increments.
        if matches!(first_class, GraphemeClass::RegionalIndicator) {
            self.ri_count += 1;
        } else {
            self.ri_count = 0;
        }
        // ExtPict tracking for GB11.
        let mut last_extpict = matches!(first_class, GraphemeClass::ExtendedPictographic);
        // Track ZWJ-after-ExtPict-with-only-Extends state for GB11.
        let mut extpict_extend_zwj_active = false;

        let mut prev_class = first_class;
        let mut prev_offset = self.cursor;

        for (rel_off, ch) in iter {
            let curr_offset = self.cursor + rel_off;
            let curr_class = self.engine.grapheme_class(ch as u32);

            let should_break = grapheme_break_between(
                prev_class,
                curr_class,
                self.ri_count,
                extpict_extend_zwj_active,
            );

            if should_break {
                self.cursor = curr_offset;
                return Some(curr_offset);
            }

            // Update RI parity (per UAX #29 the "run" of RIs matters).
            if matches!(curr_class, GraphemeClass::RegionalIndicator) {
                self.ri_count += 1;
            } else {
                self.ri_count = 0;
            }

            // Update ExtPict-tracking state for GB11.
            match curr_class {
                GraphemeClass::ExtendedPictographic => {
                    last_extpict = true;
                    extpict_extend_zwj_active = false;
                }
                GraphemeClass::Extend => {
                    // Extend after an ExtPict keeps the state alive.
                    if !last_extpict {
                        extpict_extend_zwj_active = false;
                    }
                }
                GraphemeClass::Zwj => {
                    extpict_extend_zwj_active = last_extpict;
                }
                _ => {
                    last_extpict = false;
                    extpict_extend_zwj_active = false;
                }
            }

            prev_class = curr_class;
            prev_offset = curr_offset;
        }
        // Ran off the end.
        let _ = prev_offset; // silence unused warning when panic-free
        self.cursor = bytes.len();
        None
    }
}

/// Should we break *between* two adjacent scalars whose classes are
/// `left` and `right`? Encodes UAX #29 GB3-GB13.
fn grapheme_break_between(
    left: GraphemeClass,
    right: GraphemeClass,
    ri_count_including_left: usize,
    extpict_extend_zwj_active: bool,
) -> bool {
    use GraphemeClass::{
        Cr, Extend, ExtendedPictographic, HangulL, HangulLv, HangulLvt, HangulT, HangulV, Lf,
        Prepend, RegionalIndicator, SpacingMark, Zwj,
    };
    // GB3: CR × LF (no break)
    if left == Cr && right == Lf {
        return false;
    }
    // GB4: (Control | CR | LF) ÷
    if matches!(left, Cr | Lf | GraphemeClass::Control) {
        return true;
    }
    // GB5: ÷ (Control | CR | LF)
    if matches!(right, Cr | Lf | GraphemeClass::Control) {
        return true;
    }
    // GB6: L × (L | V | LV | LVT)
    if left == HangulL && matches!(right, HangulL | HangulV | HangulLv | HangulLvt) {
        return false;
    }
    // GB7: (LV | V) × (V | T)
    if matches!(left, HangulLv | HangulV) && matches!(right, HangulV | HangulT) {
        return false;
    }
    // GB8: (LVT | T) × T
    if matches!(left, HangulLvt | HangulT) && right == HangulT {
        return false;
    }
    // GB9: × (Extend | ZWJ)
    if matches!(right, Extend | Zwj) {
        return false;
    }
    // GB9a: × SpacingMark
    if right == SpacingMark {
        return false;
    }
    // GB9b: Prepend ×
    if left == Prepend {
        return false;
    }
    // GB11: \p{ExtPict} Extend* ZWJ × \p{ExtPict}
    if extpict_extend_zwj_active && left == Zwj && right == ExtendedPictographic {
        return false;
    }
    // GB12/GB13: [^RI] (RI RI)* RI × RI  and sot (RI RI)* RI × RI
    // The state we carry: `ri_count_including_left` is the number of
    // RIs in the current run ending at `left`. If it is odd (1, 3,
    // 5, …), the RI-RI pair should not break; if even (2, 4, …), it
    // should break.
    if left == RegionalIndicator && right == RegionalIndicator && ri_count_including_left % 2 == 1 {
        return false;
    }
    // GB999: else break.
    true
}

impl Iterator for GraphemeIter<'_, '_> {
    type Item = u32;
    fn next(&mut self) -> Option<Self::Item> {
        if !self.emitted_start {
            self.emitted_start = true;
            return Some(0);
        }
        if let Some(next) = self.advance_to_next_boundary() {
            return Some(u32::try_from(next).unwrap_or(u32::MAX));
        }
        if !self.emitted_end {
            self.emitted_end = true;
            return Some(u32::try_from(self.text.len()).unwrap_or(u32::MAX));
        }
        None
    }
}

// -----------------------------------------------------------------------
// Word iterator (UAX #29 § 4)
// -----------------------------------------------------------------------

/// Iterator over UAX #29 word segments.
///
/// Yields one [`WordSegment`] per word or non-word run. Rules
/// implemented: WB1-WB16 (line 20 UAX #29 rev 43). The engine uses
/// WB4 to fold Extend/Format/ZWJ into the preceding class.
pub struct WordIter<'a, 'e> {
    text: &'a str,
    engine: &'e BreakEngine<'e>,
    /// Byte offset of the next character to inspect.
    cursor: usize,
    /// Whether we have already emitted the trailing empty segment
    /// (never — we short-circuit on empty input in `next()`).
    finished: bool,
}

impl<'a, 'e> WordIter<'a, 'e> {
    /// Fresh iterator over `text` under `engine`.
    #[must_use]
    pub fn new(text: &'a str, engine: &'e BreakEngine<'e>) -> Self {
        Self {
            text,
            engine,
            cursor: 0,
            finished: text.is_empty(),
        }
    }
}

impl Iterator for WordIter<'_, '_> {
    type Item = WordSegment;
    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        let start = self.cursor;
        // Walk until the next boundary.
        let end = next_word_boundary(self.text, self.engine, self.cursor);
        // Classify the segment.
        let seg = &self.text[start..end];
        let is_word_like = segment_is_word_like(seg, self.engine);
        self.cursor = end;
        if self.cursor >= self.text.len() {
            self.finished = true;
        }
        Some(WordSegment {
            start: u32::try_from(start).unwrap_or(u32::MAX),
            end: u32::try_from(end).unwrap_or(u32::MAX),
            is_word_like,
        })
    }
}

// -----------------------------------------------------------------------
// CJK dictionary-based word segmentation (FMM)
// -----------------------------------------------------------------------

/// True iff a BCP 47 locale tag opts into the CJK dictionary-based
/// word segmenter. Matches `"ja"` and `"zh"` and any tag whose
/// primary subtag is one of those (`"ja-JP"`, `"zh-Hans-CN"`).
#[must_use]
fn locale_wants_cjk_dict(locale: &str) -> bool {
    let primary = locale.split(['-', '_']).next().unwrap_or("");
    matches!(primary, "ja" | "zh")
}

/// FMM word segmentation over an input backed by a
/// [`WordDictView`].
///
/// Walks the input in single-scalar steps. Each contiguous run of
/// CJK-script scalars is consumed by
/// [`forward_max_match`] against the dictionary — the longest
/// dictionary entry that matches the input's prefix is emitted as a
/// word, then the cursor advances past it and the loop repeats. If
/// no dictionary entry matches, one scalar is emitted as an
/// unknown-word singleton (still word-like). Runs of non-CJK
/// scalars fall through to the UAX #29 [`WordIter`] the pack-less
/// default already ships.
///
/// The output contract mirrors
/// [`BreakEngine::segment_words`]: contiguous coverage of the input
/// from `0` to `text.len()`, in ascending byte order.
#[cfg(feature = "alloc")]
fn segment_words_with_dict(
    text: &str,
    engine: &BreakEngine<'_>,
    dict: &WordDictView<'_>,
) -> Vec<WordSegment> {
    let mut out: Vec<WordSegment> = Vec::new();
    let bytes_len = text.len();
    let mut cursor = 0usize;
    while cursor < bytes_len {
        // Peek at the first char to decide CJK vs non-CJK path.
        let first_ch = text[cursor..]
            .chars()
            .next()
            .expect("cursor < len ⇒ at least one char");
        if classes::is_cjk_scalar(first_ch as u32) {
            // Consume a maximal CJK run FMM-style.
            let run_end = end_of_cjk_run(text, cursor);
            fmm_run(text, cursor, run_end, dict, &mut out);
            cursor = run_end;
        } else {
            // Consume a maximal non-CJK run using UAX #29 iteration.
            let run_end = end_of_non_cjk_run(text, cursor);
            let slice = &text[cursor..run_end];
            let offset_u32 = u32::try_from(cursor).unwrap_or(u32::MAX);
            for seg in WordIter::new(slice, engine) {
                out.push(WordSegment {
                    start: seg.start.saturating_add(offset_u32),
                    end: seg.end.saturating_add(offset_u32),
                    is_word_like: seg.is_word_like,
                });
            }
            cursor = run_end;
        }
    }
    out
}

/// Locate the byte offset of the first non-CJK scalar at or after
/// `start`. Returns `text.len()` when the whole tail is CJK.
fn end_of_cjk_run(text: &str, start: usize) -> usize {
    let mut off = start;
    for (rel, ch) in text[start..].char_indices() {
        if !classes::is_cjk_scalar(ch as u32) {
            return start + rel;
        }
        off = start + rel + ch.len_utf8();
    }
    off
}

/// Locate the byte offset of the first CJK scalar at or after
/// `start`. Returns `text.len()` when the whole tail is non-CJK.
fn end_of_non_cjk_run(text: &str, start: usize) -> usize {
    let mut off = start;
    for (rel, ch) in text[start..].char_indices() {
        if classes::is_cjk_scalar(ch as u32) {
            return start + rel;
        }
        off = start + rel + ch.len_utf8();
    }
    off
}

/// Emit dictionary-driven word segments covering `text[start..end]`,
/// treating every scalar in that range as CJK (the caller guards
/// that invariant via [`end_of_cjk_run`]).
#[cfg(feature = "alloc")]
fn fmm_run(
    text: &str,
    start: usize,
    end: usize,
    dict: &WordDictView<'_>,
    out: &mut Vec<WordSegment>,
) {
    let bytes = text.as_bytes();
    let mut cursor = start;
    while cursor < end {
        // Bound the FMM probe to the CJK run's tail and the dict's
        // max word length.
        let remaining = end - cursor;
        let probe_hi = remaining.min(dict.max_word_len_bytes());
        // Look up the longest dictionary entry that matches. Only
        // accept matches that land on a char boundary — the input is
        // valid UTF-8 so we must not emit a byte-range that splits a
        // scalar.
        let match_len = dict
            .longest_prefix_match(&bytes[cursor..cursor + probe_hi])
            .filter(|&len| text.is_char_boundary(cursor + len));
        if let Some(len) = match_len {
            let seg_start = u32::try_from(cursor).unwrap_or(u32::MAX);
            let seg_end = u32::try_from(cursor + len).unwrap_or(u32::MAX);
            out.push(WordSegment {
                start: seg_start,
                end: seg_end,
                is_word_like: true,
            });
            cursor += len;
        } else {
            // Unknown-word fallback: emit one scalar. CJK scalars
            // are word-like by construction.
            let ch = text[cursor..]
                .chars()
                .next()
                .expect("cursor < end ⇒ at least one char");
            let clen = ch.len_utf8();
            let seg_start = u32::try_from(cursor).unwrap_or(u32::MAX);
            let seg_end = u32::try_from(cursor + clen).unwrap_or(u32::MAX);
            out.push(WordSegment {
                start: seg_start,
                end: seg_end,
                is_word_like: true,
            });
            cursor += clen;
        }
    }
}

fn segment_is_word_like(seg: &str, engine: &BreakEngine<'_>) -> bool {
    // A segment is word-like if it contains any ALetter,
    // HebrewLetter, Katakana, Numeric, Hiragana (ExtPict), or
    // ExtendNumLet scalar. Whitespace / punctuation / newline
    // segments are not word-like.
    for ch in seg.chars() {
        let c = engine.word_class(ch as u32);
        match c {
            WordClass::ALetter
            | WordClass::HebrewLetter
            | WordClass::Katakana
            | WordClass::Numeric
            | WordClass::ExtendNumLet
            | WordClass::RegionalIndicator
            | WordClass::ExtendedPictographic => return true,
            _ => {}
        }
        // Hiragana (WordClass::Other for Phase 5) — accept as
        // word-like when the character is CJK-ideographic.
        if classes::is_extended_pictographic(ch as u32) {
            return true;
        }
    }
    false
}

/// Find the next word boundary strictly greater than `start`.
///
/// This walks the text and applies WB rules by folding each new
/// character into the running state. The rule numbers used inline
/// map to UAX #29 § 4.
fn next_word_boundary(text: &str, engine: &BreakEngine<'_>, start: usize) -> usize {
    let bytes_len = text.len();
    if start >= bytes_len {
        return bytes_len;
    }
    // We track the "effective class" of the previous non-ignored
    // scalar (WB4 folds Extend/Format/ZWJ into the preceding class,
    // unless the preceding class is CR/LF/Newline).
    let mut prev_effective: Option<WordClass> = None;
    // Track the class before `prev` (used by WB6/WB7 lookahead).
    let mut prev_prev_effective: Option<WordClass> = None;
    // Track the raw class of the immediately preceding scalar —
    // WB3c needs to see the actual ZWJ, not the folded ExtPict.
    let mut prev_raw: Option<WordClass> = None;
    // RI parity for WB15/WB16.
    let mut ri_run_len: usize = 0;

    let iter = text[start..].char_indices();
    let mut last_offset = start;
    for (rel_off, ch) in iter {
        let curr_offset = start + rel_off;
        let curr_raw = engine.word_class(ch as u32);

        // WB3: CR × LF handled inside the break test.
        // WB3a/WB3b: CR/LF/Newline always break on either side
        // except CR × LF.
        if let Some(prev) = prev_effective {
            let should_break = word_break_between(
                prev_prev_effective,
                prev,
                prev_raw,
                curr_raw,
                ri_run_len,
                &LookaheadCtx {
                    text,
                    engine,
                    curr_offset,
                },
            );
            if should_break {
                return curr_offset;
            }
        }

        // Update RI parity. WB4 does not fold RIs; they carry their
        // own counter.
        if matches!(curr_raw, WordClass::RegionalIndicator) {
            ri_run_len += 1;
        } else {
            ri_run_len = 0;
        }

        // Apply WB4: Extend / Format / ZWJ (but not after
        // CR/LF/Newline) inherit the previous effective class.
        let curr_effective = fold_extend_format_zwj(prev_effective, curr_raw);

        // Only advance the shift when the current class was NOT
        // folded away (i.e. the scalar contributed a new class).
        if !matches!(
            curr_raw,
            WordClass::Extend | WordClass::Format | WordClass::Zwj
        ) || matches!(
            prev_effective,
            Some(WordClass::Cr | WordClass::Lf | WordClass::Newline) | None
        ) {
            prev_prev_effective = prev_effective;
            prev_effective = Some(curr_effective);
        }
        prev_raw = Some(curr_raw);
        last_offset = curr_offset + ch.len_utf8();
    }
    // WB2: Any ÷ eot.
    last_offset
}

/// WB4: Extend/Format/ZWJ following non-newline scalars inherit the
/// previous class; following newline scalars they stand alone.
fn fold_extend_format_zwj(prev: Option<WordClass>, curr: WordClass) -> WordClass {
    if matches!(curr, WordClass::Extend | WordClass::Format | WordClass::Zwj) {
        match prev {
            None | Some(WordClass::Cr | WordClass::Lf | WordClass::Newline) => curr,
            Some(p) => p,
        }
    } else {
        curr
    }
}

/// Context passed into the word-break-between test — needed for
/// WB6/WB7 lookahead across intervening `(MidLetter | MidNumLetQ)`
/// runs.
struct LookaheadCtx<'a, 'e> {
    text: &'a str,
    engine: &'e BreakEngine<'e>,
    /// Byte offset of the character we are testing (i.e. the
    /// "right" side of the pair).
    curr_offset: usize,
}

fn word_break_between(
    prev_prev: Option<WordClass>,
    prev: WordClass,
    prev_raw: Option<WordClass>,
    curr: WordClass,
    ri_run_len: usize,
    ctx: &LookaheadCtx<'_, '_>,
) -> bool {
    use WordClass::{
        ALetter, Cr, DoubleQuote, Extend, ExtendNumLet, ExtendedPictographic, Format, HebrewLetter,
        Katakana, Lf, MidLetter, MidNum, MidNumLet, Newline, Numeric, RegionalIndicator,
        SingleQuote, WSegSpace, Zwj,
    };

    // WB3: CR × LF (no break)
    if prev == Cr && curr == Lf {
        return false;
    }
    // WB3a: (Newline | CR | LF) ÷
    if matches!(prev, Newline | Cr | Lf) {
        return true;
    }
    // WB3b: ÷ (Newline | CR | LF)
    if matches!(curr, Newline | Cr | Lf) {
        return true;
    }
    // WB3c: ZWJ × \p{Extended_Pictographic} — checked against the
    // *raw* prev class so the WB4 fold doesn't hide the ZWJ.
    if prev_raw == Some(Zwj) && curr == ExtendedPictographic {
        return false;
    }
    // WB3d: Keep horizontal whitespace together.
    if prev == WSegSpace && curr == WSegSpace {
        return false;
    }
    // WB4: Extend/Format/ZWJ folded into previous class (handled by
    // caller). If either side is Extend/Format/ZWJ here, no break.
    if matches!(curr, Extend | Format | Zwj) {
        return false;
    }

    // AHLetter = ALetter | HebrewLetter.
    let is_ah = |c: WordClass| matches!(c, ALetter | HebrewLetter);

    // WB5: (AHLetter) × (AHLetter)
    if is_ah(prev) && is_ah(curr) {
        return false;
    }
    // WB6: (AHLetter) × (MidLetter | MidNumLetQ) (AHLetter) — needs
    // lookahead past a single MidLetter/MidNumLetQ character to see
    // if AHLetter follows.
    if is_ah(prev) && matches!(curr, MidLetter | MidNumLet | SingleQuote) {
        if let Some(next) = next_word_class_skipping_extend(ctx) {
            if is_ah(next) {
                return false;
            }
        }
    }
    // WB7: (AHLetter) (MidLetter | MidNumLetQ) × (AHLetter)
    if let Some(pp) = prev_prev {
        if is_ah(pp) && matches!(prev, MidLetter | MidNumLet | SingleQuote) && is_ah(curr) {
            return false;
        }
    }
    // WB7a: Hebrew_Letter × Single_Quote
    if prev == HebrewLetter && curr == SingleQuote {
        return false;
    }
    // WB7b: Hebrew_Letter × Double_Quote Hebrew_Letter
    if prev == HebrewLetter && curr == DoubleQuote {
        if let Some(next) = next_word_class_skipping_extend(ctx) {
            if next == HebrewLetter {
                return false;
            }
        }
    }
    // WB7c: Hebrew_Letter Double_Quote × Hebrew_Letter
    if let Some(pp) = prev_prev {
        if pp == HebrewLetter && prev == DoubleQuote && curr == HebrewLetter {
            return false;
        }
    }
    // WB8: Numeric × Numeric
    if prev == Numeric && curr == Numeric {
        return false;
    }
    // WB9: AHLetter × Numeric
    if is_ah(prev) && curr == Numeric {
        return false;
    }
    // WB10: Numeric × AHLetter
    if prev == Numeric && is_ah(curr) {
        return false;
    }
    // WB11: Numeric (MidNum | MidNumLetQ) × Numeric
    if let Some(pp) = prev_prev {
        if pp == Numeric && matches!(prev, MidNum | MidNumLet | SingleQuote) && curr == Numeric {
            return false;
        }
    }
    // WB12: Numeric × (MidNum | MidNumLetQ) Numeric
    if prev == Numeric && matches!(curr, MidNum | MidNumLet | SingleQuote) {
        if let Some(next) = next_word_class_skipping_extend(ctx) {
            if next == Numeric {
                return false;
            }
        }
    }
    // WB13: Katakana × Katakana
    if prev == Katakana && curr == Katakana {
        return false;
    }
    // WB13a: (AHLetter | Numeric | Katakana | ExtendNumLet) ×
    //         ExtendNumLet
    if matches!(
        prev,
        ALetter | HebrewLetter | Numeric | Katakana | ExtendNumLet
    ) && curr == ExtendNumLet
    {
        return false;
    }
    // WB13b: ExtendNumLet × (AHLetter | Numeric | Katakana)
    if prev == ExtendNumLet && matches!(curr, ALetter | HebrewLetter | Numeric | Katakana) {
        return false;
    }
    // WB15/WB16: Regional_Indicator pair when the RI run so far
    // (including `prev`) is odd.
    if prev == RegionalIndicator && curr == RegionalIndicator && ri_run_len % 2 == 1 {
        return false;
    }
    // WB999: else break.
    true
}

fn next_word_class_skipping_extend(ctx: &LookaheadCtx<'_, '_>) -> Option<WordClass> {
    // Look at the character right after `curr_offset`, skipping
    // Extend/Format/ZWJ. Because `curr_offset` is the offset of the
    // "right-hand" character in the WB6/WB7b/WB12 rules, we start
    // the walk one character past it.
    let start = ctx.curr_offset;
    let bytes = ctx.text.as_bytes();
    // Compute the length of the character at `curr_offset` so we
    // can step past it.
    let first_char_len = ctx.text[start..].chars().next()?.len_utf8();
    let mut walk = start + first_char_len;
    while walk < bytes.len() {
        let ch = ctx.text[walk..].chars().next()?;
        let cls = ctx.engine.word_class(ch as u32);
        if !matches!(cls, WordClass::Extend | WordClass::Format | WordClass::Zwj) {
            return Some(cls);
        }
        walk += ch.len_utf8();
    }
    None
}

// -----------------------------------------------------------------------
// Sentence iterator (UAX #29 § 5)
// -----------------------------------------------------------------------

/// Iterator over UAX #29 sentence boundary byte offsets.
///
/// Yields `0, b_1, …, b_N, len(text)` under the same shape as
/// [`GraphemeIter`]. Rules implemented: SB1-SB11.
pub struct SentenceIter<'a, 'e> {
    text: &'a str,
    engine: &'e BreakEngine<'e>,
    cursor: usize,
    emitted_start: bool,
    emitted_end: bool,
}

impl<'a, 'e> SentenceIter<'a, 'e> {
    /// Fresh iterator over `text` under `engine`.
    #[must_use]
    pub fn new(text: &'a str, engine: &'e BreakEngine<'e>) -> Self {
        Self {
            text,
            engine,
            cursor: 0,
            emitted_start: false,
            emitted_end: false,
        }
    }
}

impl Iterator for SentenceIter<'_, '_> {
    type Item = u32;
    fn next(&mut self) -> Option<Self::Item> {
        if !self.emitted_start {
            self.emitted_start = true;
            return Some(0);
        }
        if let Some(next) = advance_sentence(self.text, self.engine, self.cursor) {
            self.cursor = next;
            return Some(u32::try_from(next).unwrap_or(u32::MAX));
        }
        if !self.emitted_end {
            self.emitted_end = true;
            self.cursor = self.text.len();
            return Some(u32::try_from(self.text.len()).unwrap_or(u32::MAX));
        }
        None
    }
}

/// Locate the next sentence boundary strictly greater than
/// `start`. Returns `None` when no interior boundary remains.
fn advance_sentence(text: &str, engine: &BreakEngine<'_>, start: usize) -> Option<usize> {
    if start >= text.len() {
        return None;
    }
    // We walk char-by-char maintaining a small state machine.
    // Effective class per SB5 (Extend/Format absorbed into
    // preceding class).
    //
    // The trigger for a sentence break in the default rules is
    // "SATerm Close* Sp* ParaSep?" followed by anything that is
    // not (Numeric | (Upper|Lower) after ATerm | SContinue |
    // SATerm | Sp | Close). We drive a state machine that tracks:
    //
    //   AwaitSATerm | SawATerm | SawSTerm | SawClose(sa) | SawSp(sa)
    //
    // where `sa` remembers whether the terminator was ATerm or
    // STerm (SB7 only applies to ATerm).
    let mut prev_effective: Option<SentenceClass> = None;
    let mut prev_prev_effective: Option<SentenceClass> = None;
    let mut state = SbLikeState::None;

    for (rel_off, ch) in text[start..].char_indices() {
        let curr_offset = start + rel_off;
        let curr_raw = engine.sentence_class(ch as u32);
        // Apply SB5: Extend/Format fold into previous class,
        // preserving CR/LF/Sep boundaries.
        let curr_effective = if matches!(curr_raw, SentenceClass::Extend | SentenceClass::Format) {
            match prev_effective {
                None | Some(SentenceClass::Cr | SentenceClass::Lf | SentenceClass::Sep) => curr_raw,
                Some(p) => p,
            }
        } else {
            curr_raw
        };

        // Consider breaking BEFORE this char if the prior state
        // wants it.
        if let Some(prev) = prev_effective {
            // SB3: CR × LF (no break).
            if prev == SentenceClass::Cr && curr_effective == SentenceClass::Lf {
                // pass through
            } else if matches!(
                prev,
                SentenceClass::Cr | SentenceClass::Lf | SentenceClass::Sep
            ) {
                // SB4: ParaSep ÷ Any.
                return Some(curr_offset);
            } else {
                // Consult the state machine.
                let break_here =
                    sb_should_break_before(state, prev_prev_effective, prev, curr_effective);
                if break_here {
                    return Some(curr_offset);
                }
            }
        }

        // Update state machine.
        state = advance_sb_state(state, curr_effective);

        // Update prev pointers only when the current scalar was NOT
        // absorbed as Extend/Format.
        if !matches!(curr_raw, SentenceClass::Extend | SentenceClass::Format)
            || matches!(
                prev_effective,
                None | Some(SentenceClass::Cr | SentenceClass::Lf | SentenceClass::Sep)
            )
        {
            prev_prev_effective = prev_effective;
            prev_effective = Some(curr_effective);
        }
        let _ = ch;
    }
    // Ran off the end without an interior boundary.
    None
}

fn advance_sb_state(state: /* prior */ SbLikeState, curr: SentenceClass) -> SbLikeState {
    use SentenceClass::{ATerm, Close, Cr, Lf, STerm, Sep, Sp};
    match (state, curr) {
        (_, ATerm) => SbLikeState::SawATerm,
        (_, STerm) => SbLikeState::SawSTerm,
        (SbLikeState::SawATerm, Close) => SbLikeState::InTail {
            sterm: false,
            saw_sp: false,
            saw_para: false,
        },
        (SbLikeState::SawSTerm, Close) => SbLikeState::InTail {
            sterm: true,
            saw_sp: false,
            saw_para: false,
        },
        (
            SbLikeState::InTail {
                sterm,
                saw_sp,
                saw_para,
            },
            Close,
        ) if !saw_sp && !saw_para => SbLikeState::InTail {
            sterm,
            saw_sp,
            saw_para,
        },
        (SbLikeState::SawATerm, Sp) => SbLikeState::InTail {
            sterm: false,
            saw_sp: true,
            saw_para: false,
        },
        (SbLikeState::SawSTerm, Sp) => SbLikeState::InTail {
            sterm: true,
            saw_sp: true,
            saw_para: false,
        },
        (
            SbLikeState::InTail {
                sterm, saw_para, ..
            },
            Sp,
        ) if !saw_para => SbLikeState::InTail {
            sterm,
            saw_sp: true,
            saw_para,
        },
        (SbLikeState::InTail { sterm, saw_sp, .. }, Cr | Lf | Sep) => SbLikeState::InTail {
            sterm,
            saw_sp,
            saw_para: true,
        },
        _ => SbLikeState::None,
    }
}

/// SB rule test: when driving the walk one character at a time,
/// should we break BEFORE `curr`, given the state summarising all
/// prior scalars?
fn sb_should_break_before(
    state: SbLikeState,
    prev_prev: Option<SentenceClass>,
    prev: SentenceClass,
    curr: SentenceClass,
) -> bool {
    use SentenceClass::{
        ATerm, Close, Cr, Extend, Format, Lf, Lower, Numeric, SContinue, STerm, Sep, Sp, Upper,
    };
    // SB6: ATerm × Numeric  (no break)
    if prev == ATerm && curr == Numeric {
        return false;
    }
    // SB7: (Upper|Lower) ATerm × Upper (no break)
    if let SbLikeState::SawATerm = state {
        if curr == Upper && matches!(prev_prev, Some(Upper | Lower)) {
            return false;
        }
    }
    // SB8a: SATerm Close* Sp* × (SContinue | SATerm)
    if matches!(curr, SContinue | ATerm | STerm)
        && matches!(
            state,
            SbLikeState::SawATerm | SbLikeState::SawSTerm | SbLikeState::InTail { .. }
        )
    {
        return false;
    }
    // SB9: SATerm Close* × (Close | Sp | Sep | CR | LF)
    if matches!(
        state,
        SbLikeState::SawATerm
            | SbLikeState::SawSTerm
            | SbLikeState::InTail {
                saw_sp: false,
                saw_para: false,
                ..
            }
    ) && matches!(curr, Close | Sp | Sep | Cr | Lf)
    {
        return false;
    }
    // SB10: SATerm Close* Sp* × (Sp | Sep | CR | LF)
    if matches!(
        state,
        SbLikeState::InTail {
            saw_sp: true,
            saw_para: false,
            ..
        }
    ) && matches!(curr, Sp | Sep | Cr | Lf)
    {
        return false;
    }
    // SB11: break after "SATerm Close* Sp* ParaSep?" run when we
    // see anything else. Fires when the state is "SawATerm" /
    // "SawSTerm" (bare terminator followed by curr) or "InTail" and
    // curr is not covered by SB8a/SB9/SB10 above.
    match state {
        SbLikeState::SawATerm => {
            // SB8: ATerm Close* Sp* × (¬(OLetter | Upper | Lower |
            // ParaSep | SATerm)* Lower). Simplified: if curr is
            // Lower we do not break. Also do not break on
            // Extend/Format — folded by SB5.
            !matches!(curr, Lower | Extend | Format)
        }
        SbLikeState::SawSTerm | SbLikeState::InTail { .. } => true,
        SbLikeState::None => {
            let _ = prev;
            false
        }
    }
}

/// Local alias to keep the state enum used only inside this module
/// from leaking into the crate's public surface.
#[derive(Copy, Clone, Debug)]
enum SbLikeState {
    None,
    SawATerm,
    SawSTerm,
    InTail {
        sterm: bool,
        saw_sp: bool,
        saw_para: bool,
    },
}

// -----------------------------------------------------------------------
// Convenience free functions
// -----------------------------------------------------------------------

/// Default-engine convenience over
/// [`BreakEngine::segment_graphemes`].
#[cfg(feature = "alloc")]
#[must_use]
pub fn segment_graphemes_default(text: &str) -> Vec<u32> {
    BreakEngine::new().segment_graphemes(text)
}

/// Default-engine convenience over
/// [`BreakEngine::segment_words`].
#[cfg(feature = "alloc")]
#[must_use]
pub fn segment_words_default(text: &str) -> Vec<WordSegment> {
    BreakEngine::new().segment_words(text, "")
}

/// Default-engine convenience over
/// [`BreakEngine::segment_sentences`].
#[cfg(feature = "alloc")]
#[must_use]
pub fn segment_sentences_default(text: &str) -> Vec<u32> {
    BreakEngine::new().segment_sentences(text, "")
}

/// Extract the text slice of `text` between two adjacent boundaries.
///
/// Convenience helper for callers who want a `&str` per grapheme or
/// sentence rather than the raw offset list.
#[cfg(feature = "alloc")]
#[must_use]
pub fn slice_between(text: &str, boundaries: &[u32]) -> Vec<String> {
    let mut out = Vec::with_capacity(boundaries.len().saturating_sub(1));
    for pair in boundaries.windows(2) {
        let (a, b) = (pair[0] as usize, pair[1] as usize);
        if a <= b && b <= text.len() {
            out.push(text[a..b].into());
        }
    }
    out
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::*;
    use alloc::vec;

    fn engine() -> BreakEngine<'static> {
        BreakEngine::new()
    }

    // -- Grapheme tests ------------------------------------------------

    #[test]
    fn gb_empty_input() {
        assert_eq!(engine().segment_graphemes(""), vec![0]);
    }

    #[test]
    fn gb1_gb2_ascii_letters_each_stand_alone() {
        // Rule GB1/GB2 + GB999.
        assert_eq!(engine().segment_graphemes("abc"), vec![0, 1, 2, 3]);
    }

    #[test]
    fn gb3_cr_lf_stays_together() {
        // Rule GB3: CR × LF (one grapheme).
        assert_eq!(engine().segment_graphemes("a\r\nb"), vec![0, 1, 3, 4]);
    }

    #[test]
    fn gb4_gb5_control_breaks_on_both_sides() {
        // Rule GB4/GB5: control characters always break.
        // \x07 is BEL (Cc). a BEL b -> [0,1,2,3]
        assert_eq!(engine().segment_graphemes("a\x07b"), vec![0, 1, 2, 3]);
    }

    #[test]
    fn gb6_hangul_l_v_stays_together() {
        // Rule GB6: L × V.
        let s = "\u{1100}\u{1161}"; // ᄀ + ᅡ = one syllable
        assert_eq!(engine().segment_graphemes(s), vec![0, 6]);
    }

    #[test]
    fn gb7_hangul_lv_t_stays_together() {
        // Rule GB7/GB8.
        let s = "\u{AC00}\u{11A8}"; // 가 + trailing jamo
        assert_eq!(engine().segment_graphemes(s), vec![0, 6]);
    }

    #[test]
    fn gb9_extend_glues_to_previous() {
        // Rule GB9: base + combining mark = one grapheme.
        // "e" + U+0301 (combining acute) → one grapheme, 3 bytes.
        assert_eq!(engine().segment_graphemes("e\u{0301}"), vec![0, 3]);
    }

    #[test]
    fn gb9_zwj_glues_to_previous() {
        // Rule GB9: × ZWJ — no break between 'a' and ZWJ. But GB11
        // does not apply (neither side is ExtPict), so ZWJ still
        // breaks before 'b' under GB999. Boundaries: 0, 4 (end of
        // 'a' + ZWJ cluster), 5 (end of 'b').
        assert_eq!(engine().segment_graphemes("a\u{200D}b"), vec![0, 4, 5]);
    }

    #[test]
    fn gb11_extpict_zwj_extpict_stays_together() {
        // Rule GB11: emoji + ZWJ + emoji = one grapheme.
        // Man + ZWJ + Woman = 1 grapheme.
        let s = "\u{1F468}\u{200D}\u{1F469}";
        assert_eq!(engine().segment_graphemes(s), vec![0, 11]);
    }

    #[test]
    fn gb11_family_emoji_is_one_grapheme() {
        // Man + ZWJ + Woman + ZWJ + Girl = one grapheme.
        let s = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
        assert_eq!(
            engine().segment_graphemes(s),
            vec![0, u32::try_from(s.len()).unwrap()]
        );
    }

    #[test]
    fn gb12_gb13_regional_indicator_pairs() {
        // 🇬🇧 (Regional_Indicator G + Regional_Indicator B) — one
        // grapheme (rule GB12/GB13: RI × RI when preceding RIs are
        // even).
        let s = "\u{1F1EC}\u{1F1E7}";
        assert_eq!(engine().segment_graphemes(s), vec![0, 8]);
    }

    #[test]
    fn gb12_gb13_three_regional_indicators_break_after_second() {
        // Three RIs in a row: first two glue (odd RI count), third
        // stands alone (even count at boundary).
        let s = "\u{1F1EC}\u{1F1E7}\u{1F1E8}";
        // First cluster: bytes 0..8 (RI G + RI B), second cluster:
        // bytes 8..12 (RI C).
        assert_eq!(engine().segment_graphemes(s), vec![0, 8, 12]);
    }

    #[test]
    fn gb_precomposed_e_acute_is_one_grapheme() {
        assert_eq!(engine().segment_graphemes("\u{00E9}"), vec![0, 2]);
    }

    #[test]
    fn gb_decomposed_cafe_is_four_graphemes() {
        // "cafe\u{0301}" — c a f e´ = 4 graphemes; byte layout is
        // 4 ASCII + 2-byte combining = 6 bytes total, with
        // boundaries at 0,1,2,3,6.
        assert_eq!(
            engine().segment_graphemes("cafe\u{0301}"),
            vec![0, 1, 2, 3, 6]
        );
    }

    #[test]
    fn gb_devanagari_prepend_no_break() {
        // U+0D4E MALAYALAM LETTER DOT REPH is Prepend; it should
        // stay glued to the following consonant. Use U+0D2A (letter
        // pa) to keep the example minimal.
        let s = "\u{0D4E}\u{0D2A}";
        assert_eq!(
            engine().segment_graphemes(s),
            vec![0, u32::try_from(s.len()).unwrap()]
        );
    }

    // -- Word tests ----------------------------------------------------

    #[test]
    fn wb_empty_input() {
        assert_eq!(engine().segment_words("", ""), vec![]);
    }

    #[test]
    fn wb5_ascii_word_stays_together() {
        // "hello" → single word segment 0..5.
        let out = engine().segment_words("hello", "");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].start, 0);
        assert_eq!(out[0].end, 5);
        assert!(out[0].is_word_like);
    }

    #[test]
    fn wb3d_space_run_stays_together() {
        // "hi   there" → hi | "   " | there
        let out = engine().segment_words("hi   there", "");
        let starts: Vec<u32> = out.iter().map(|s| s.start).collect();
        assert_eq!(starts, vec![0, 2, 5]);
        assert!(out[0].is_word_like);
        assert!(!out[1].is_word_like); // whitespace
        assert!(out[2].is_word_like);
    }

    #[test]
    fn wb6_wb7_apostrophe_glues_words() {
        // "don't" is one word (WB6/WB7 with MidNumLet apostrophe).
        let out = engine().segment_words("don't", "");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].start, 0);
        assert_eq!(out[0].end, 5);
        assert!(out[0].is_word_like);
    }

    #[test]
    fn wb8_numeric_stays_together() {
        // "12345" → one segment.
        let out = engine().segment_words("12345", "");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].end, 5);
        assert!(out[0].is_word_like);
    }

    #[test]
    fn wb11_wb12_numeric_with_decimal_glued() {
        // "3.14" → one word.
        let out = engine().segment_words("3.14", "");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].end, 4);
    }

    #[test]
    fn wb9_wb10_alnum_glued() {
        // "abc123def" — WB9 (ALetter × Numeric) + WB10 (Numeric ×
        // ALetter) → one word.
        let out = engine().segment_words("abc123def", "");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].end, 9);
    }

    #[test]
    fn wb13a_extendnumlet_glues() {
        // "foo_bar" — underscore is ExtendNumLet, WB13a/WB13b glue
        // both sides.
        let out = engine().segment_words("foo_bar", "");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].end, 7);
    }

    #[test]
    fn wb_punctuation_between_words() {
        // "hello, world" → hello | ", " | world (comma+space)
        // The comma is MidNum, space is WSegSpace.
        let out = engine().segment_words("hello, world", "");
        let starts: Vec<u32> = out.iter().map(|s| s.start).collect();
        // Expect at least hello (0), separator segment(s), and
        // world.
        assert_eq!(starts.first(), Some(&0));
        // Ensure "world" is one segment.
        let world_seg = out.iter().find(|s| s.start == 7).expect("world segment");
        assert_eq!(world_seg.end, 12);
        assert!(world_seg.is_word_like);
    }

    #[test]
    fn wb15_wb16_regional_indicator_pair_is_one_word() {
        // 🇬🇧 → one word segment (RI + RI).
        let s = "\u{1F1EC}\u{1F1E7}";
        let out = engine().segment_words(s, "");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].end, 8);
        assert!(out[0].is_word_like);
    }

    #[test]
    fn wb3_cr_lf_kept_together_but_word_break() {
        // "a\r\nb" — WB3 (CR × LF), WB3a (Newline ÷), WB3b (÷
        // Newline). Expect 3 segments: a | CRLF | b.
        let out = engine().segment_words("a\r\nb", "");
        let starts: Vec<u32> = out.iter().map(|s| s.start).collect();
        assert_eq!(starts, vec![0, 1, 3]);
    }

    // -- Sentence tests ------------------------------------------------

    #[test]
    fn sb_empty_input() {
        assert_eq!(engine().segment_sentences("", ""), vec![0]);
    }

    #[test]
    fn sb11_sentence_terminator_breaks() {
        // "Hi. Bye." → two sentences.
        let out = engine().segment_sentences("Hi. Bye.", "");
        assert_eq!(out.first(), Some(&0));
        assert_eq!(out.last(), Some(&8));
        assert!(out.len() >= 3, "expected at least 3 boundaries: {out:?}");
    }

    #[test]
    fn sb_question_mark_breaks() {
        // "Really? Yes." → two sentences.
        let out = engine().segment_sentences("Really? Yes.", "");
        assert_eq!(out.first(), Some(&0));
        assert_eq!(out.last(), Some(&12));
        assert!(out.len() >= 3);
    }

    #[test]
    fn sb_exclamation_breaks() {
        // "Wow! Ok." → two sentences.
        let out = engine().segment_sentences("Wow! Ok.", "");
        assert!(out.len() >= 3);
        assert_eq!(out.last(), Some(&8));
    }

    #[test]
    fn sb6_aterm_before_numeric_no_break() {
        // "3.14 is pi." — the "." between digits should not break.
        let out = engine().segment_sentences("3.14 is pi.", "");
        // Only one sentence (SB11 fires only at trailing ".").
        assert_eq!(out.first(), Some(&0));
        assert_eq!(out.last(), Some(&11));
    }

    #[test]
    fn sb_cr_lf_breaks() {
        // ParaSep (LF) forces a break.
        let out = engine().segment_sentences("Hi\nBye", "");
        assert_eq!(out.first(), Some(&0));
        assert_eq!(out.last(), Some(&6));
        assert!(out.len() >= 3);
    }

    #[test]
    fn sb_uppercase_after_atem_breaks() {
        // "Wait. Then go." — SB11 breaks after ". ".
        let out = engine().segment_sentences("Wait. Then go.", "");
        assert!(out.len() >= 3);
    }

    // -- Long-input smoke ---------------------------------------------

    #[test]
    fn long_input_does_not_panic() {
        // Bench-scale smoke: build a long string, iterate all three
        // segmenters. Just assert the returned vectors are non-empty
        // and boundaries are non-decreasing.
        let sample = "The quick brown fox jumps over the lazy dog. ";
        let mut s = String::with_capacity(sample.len() * 200);
        for _ in 0..200 {
            s.push_str(sample);
        }
        let g = engine().segment_graphemes(&s);
        let w = engine().segment_words(&s, "");
        let se = engine().segment_sentences(&s, "");
        assert_eq!(*g.first().unwrap(), 0);
        assert_eq!(*g.last().unwrap() as usize, s.len());
        assert!(!w.is_empty());
        assert_eq!(w[0].start, 0);
        assert_eq!(w.last().unwrap().end as usize, s.len());
        assert_eq!(*se.first().unwrap(), 0);
        assert_eq!(*se.last().unwrap() as usize, s.len());
    }

    // -- Pack-driven engine construction ------------------------------

    #[test]
    fn engine_with_scud_pack_falls_through_when_class_missing() {
        // The pack only overrides for scalars it covers; unknown
        // scalars still use the built-in classifier.
        use stringcheese_scud::{
            BreakSectionBuilder, CAP_BREAK, GraphemeClass, SECT_GRAPHEME_CLASSES,
            SECT_GRAPHEME_RULES, SECT_SENTENCE_CLASSES, SECT_SENTENCE_RULES, SECT_WORD_CLASSES,
            SECT_WORD_RULES, ScudWriter,
        };
        let mut b = BreakSectionBuilder::new();
        // Override 'x' as Extend so it glues to previous — a
        // synthetic test of the override plumbing.
        b.push_grapheme_range('x' as u32, 1, GraphemeClass::Extend);
        b.set_default_rules();
        let mut w = ScudWriter::new(CAP_BREAK, "44.1", Some(""));
        w.append_section(SECT_GRAPHEME_CLASSES, &b.grapheme_classes_bytes());
        w.append_section(SECT_WORD_CLASSES, &b.word_classes_bytes());
        w.append_section(SECT_SENTENCE_CLASSES, &b.sentence_classes_bytes());
        w.append_section(SECT_GRAPHEME_RULES, &b.grapheme_rules_bytes());
        w.append_section(SECT_WORD_RULES, &b.word_rules_bytes());
        w.append_section(SECT_SENTENCE_RULES, &b.sentence_rules_bytes());
        let bytes = w.finish();
        let pack = BreakPack::from_scud_bytes(&bytes).unwrap();
        let e = BreakEngine::with_pack(pack);
        // "ax" — 'a' + Extend-x should be one grapheme under the
        // override.
        assert_eq!(e.segment_graphemes("ax"), vec![0, 2]);
        // Sanity: without the pack, "ax" splits into two graphemes.
        assert_eq!(BreakEngine::new().segment_graphemes("ax"), vec![0, 1, 2]);
    }

    #[test]
    fn engine_supports_reports_true_for_every_locale() {
        let e = engine();
        assert!(e.supports(""));
        assert!(e.supports("en"));
        assert!(e.supports("ja"));
    }

    #[test]
    fn slice_between_helper() {
        let text = "abc";
        let bs = engine().segment_graphemes(text);
        let slices = slice_between(text, &bs);
        assert_eq!(slices, vec!["a", "b", "c"]);
    }

    // -- Extended: variation selector ---------------------------------

    #[test]
    fn gb_variation_selector_glues() {
        // U+FE0F (VS16) glues to the previous character (GB9 —
        // Extend).
        let s = "\u{2764}\u{FE0F}"; // ❤ + VS16 = one grapheme
        assert_eq!(
            engine().segment_graphemes(s),
            vec![0, u32::try_from(s.len()).unwrap()]
        );
    }

    #[test]
    fn wb_zwj_pictographic_no_break() {
        // WB3c: ZWJ × ExtPict — a ZWJ between two emoji keeps them
        // in one word.
        let s = "\u{1F468}\u{200D}\u{1F4BB}"; // technologist
        let out = engine().segment_words(s, "");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].end, u32::try_from(s.len()).unwrap());
    }

    // -- CJK dictionary FMM smoke ------------------------------------

    fn build_break_pack_with_dict(entries: &[&str], locale: &str) -> alloc::vec::Vec<u8> {
        use stringcheese_scud::{
            BreakSectionBuilder, CAP_BREAK, SECT_GRAPHEME_CLASSES, SECT_GRAPHEME_RULES,
            SECT_SENTENCE_CLASSES, SECT_SENTENCE_RULES, SECT_WORD_CLASSES, SECT_WORD_DICT,
            SECT_WORD_RULES, ScudWriter,
        };
        let mut b = BreakSectionBuilder::new();
        b.set_default_rules();
        for e in entries {
            b.push_dict_entry(e);
        }
        let mut w = ScudWriter::new(CAP_BREAK, "44.1", Some(locale));
        w.append_section(SECT_GRAPHEME_CLASSES, &b.grapheme_classes_bytes());
        w.append_section(SECT_WORD_CLASSES, &b.word_classes_bytes());
        w.append_section(SECT_SENTENCE_CLASSES, &b.sentence_classes_bytes());
        w.append_section(SECT_GRAPHEME_RULES, &b.grapheme_rules_bytes());
        w.append_section(SECT_WORD_RULES, &b.word_rules_bytes());
        w.append_section(SECT_SENTENCE_RULES, &b.sentence_rules_bytes());
        w.append_section(SECT_WORD_DICT, &b.word_dict_bytes());
        w.finish()
    }

    #[test]
    fn fmm_dict_segments_japanese() {
        let bytes = build_break_pack_with_dict(
            &[
                "\u{79C1}",         // 私
                "\u{306F}",         // は
                "\u{5B66}\u{751F}", // 学生
                "\u{3067}\u{3059}", // です
            ],
            "ja",
        );
        let pack = BreakPack::from_scud_bytes(&bytes).unwrap();
        let e = BreakEngine::with_pack(pack);
        // 私は学生です → [私, は, 学生, です]
        let out = e.segment_words("\u{79C1}\u{306F}\u{5B66}\u{751F}\u{3067}\u{3059}", "ja");
        assert_eq!(out.len(), 4);
        assert_eq!(out[0].end - out[0].start, 3); // 私
        assert_eq!(out[1].end - out[1].start, 3); // は
        assert_eq!(out[2].end - out[2].start, 6); // 学生
        assert_eq!(out[3].end - out[3].start, 6); // です
        for seg in &out {
            assert!(seg.is_word_like);
        }
    }

    #[test]
    fn fmm_dict_unknown_falls_through_as_single_char() {
        // Dict only covers 私; 中 is unknown and should stand alone.
        let bytes = build_break_pack_with_dict(&["\u{79C1}"], "ja");
        let pack = BreakPack::from_scud_bytes(&bytes).unwrap();
        let e = BreakEngine::with_pack(pack);
        let out = e.segment_words("\u{79C1}\u{4E2D}", "ja");
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].end - out[0].start, 3);
        assert_eq!(out[1].end - out[1].start, 3);
    }

    #[test]
    fn fmm_dict_ignored_for_non_cjk_locale() {
        let bytes = build_break_pack_with_dict(&["\u{79C1}"], "ja");
        let pack = BreakPack::from_scud_bytes(&bytes).unwrap();
        let e = BreakEngine::with_pack(pack);
        // "en" locale — FMM should NOT engage; standard UAX #29 runs.
        let out = e.segment_words("hello", "en");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].end, 5);
    }

    #[test]
    fn fmm_dict_ignored_without_pack() {
        let e = BreakEngine::new();
        let out = e.segment_words("\u{79C1}\u{306F}", "ja");
        // Falls through to UAX #29 default: each scalar becomes its
        // own word (both are Other/Hiragana).
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn fmm_dict_prefers_longest() {
        // Both 東京 and 東京大学 in the dict — the longer match wins.
        let bytes = build_break_pack_with_dict(
            &[
                "\u{6771}\u{4EAC}",                 // 東京
                "\u{6771}\u{4EAC}\u{5927}\u{5B66}", // 東京大学
                "\u{306B}",                         // に
            ],
            "ja",
        );
        let pack = BreakPack::from_scud_bytes(&bytes).unwrap();
        let e = BreakEngine::with_pack(pack);
        // 東京大学に → [東京大学, に]
        let out = e.segment_words("\u{6771}\u{4EAC}\u{5927}\u{5B66}\u{306B}", "ja");
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].end - out[0].start, 12); // 東京大学
        assert_eq!(out[1].end - out[1].start, 3); // に
    }

    #[test]
    fn fmm_mixed_cjk_latin() {
        // Dict covers 私; the "ABC" run should segment via UAX #29
        // (single Latin word).
        let bytes = build_break_pack_with_dict(&["\u{79C1}"], "ja");
        let pack = BreakPack::from_scud_bytes(&bytes).unwrap();
        let e = BreakEngine::with_pack(pack);
        // 私 A B C → [私, ABC]
        let out = e.segment_words("\u{79C1}ABC", "ja");
        assert!(out.len() >= 2);
        // First seg: 私 (3 bytes)
        assert_eq!(out[0].end, 3);
        // Last seg ends at total length.
        assert_eq!(out.last().unwrap().end as usize, "\u{79C1}ABC".len());
    }

    #[test]
    fn fmm_locale_variant_engages_dict() {
        let bytes = build_break_pack_with_dict(&["\u{79C1}"], "ja");
        let pack = BreakPack::from_scud_bytes(&bytes).unwrap();
        let e = BreakEngine::with_pack(pack);
        // "ja-JP" — primary subtag matches, FMM engages.
        let out = e.segment_words("\u{79C1}\u{4E2D}", "ja-JP");
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn fmm_covers_input_contiguously() {
        let bytes = build_break_pack_with_dict(&["\u{79C1}", "\u{5B66}\u{751F}"], "ja");
        let pack = BreakPack::from_scud_bytes(&bytes).unwrap();
        let e = BreakEngine::with_pack(pack);
        let text = "\u{79C1}\u{5B66}\u{751F}";
        let out = e.segment_words(text, "ja");
        // Coverage invariant.
        assert_eq!(out[0].start, 0);
        assert_eq!(out.last().unwrap().end as usize, text.len());
        for pair in out.windows(2) {
            assert_eq!(pair[0].end, pair[1].start);
        }
    }

    #[test]
    fn ri_run_length_is_reset_after_non_ri() {
        // RI + non-RI + RI + RI should segment as: 1 + 1 + 1 =
        // three graphemes for the RI portion (the middle non-RI
        // resets the counter).
        // Use "🇬a🇬🇧" — first RI is alone, then 'a', then RI+RI
        // pair.
        let s = "\u{1F1EC}a\u{1F1EC}\u{1F1E7}";
        let bs = engine().segment_graphemes(s);
        // Expected boundaries: 0, 4 (end of first RI), 5 (end of
        // 'a'), 13 (end of second RI-pair).
        assert_eq!(bs, vec![0, 4, 5, 13]);
    }
}
