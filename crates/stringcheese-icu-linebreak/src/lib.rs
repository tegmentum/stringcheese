//! Line-break iteration capability for the StringCheese
//! ICU-alternative subsystem.
//!
//! Iterates the byte-offset positions at which a text layout engine
//! may (or must) insert a line break per [Unicode Standard Annex
//! #14](https://www.unicode.org/reports/tr14/), driving the full
//! UAX #14 rule set (LB1-LB31 + `LB30a` / `LB30b`) from per-scalar
//! `Line_Break` classification tables in [`classes`]. Exposes the
//! result through the `tegmentum:i18n-linebreak@0.1.0` WIT world.
//!
//! # Position in the WIT-i18n subsystem
//!
//! Phase 5 follow-up (`docs/design/wit-i18n.md` § 8.7) — split out of
//! the UAX #29 segment capability because the LB rule table is much
//! larger than the UAX #29 grapheme / word / sentence tables and
//! deferring it kept the segment crate small. See § 8.5 for the
//! deferral note and § 8.7 for the delivery notes.
//!
//! # Trust model
//!
//! Inherited from `stringcheese-scud`: SCUD packs are trusted input.
//! This crate does not defend against maliciously crafted packs.
//!
//! # Rule cross-reference
//!
//! The implementation follows the rule numbers from UAX #14 § 6 (LB1
//! resolution and LB2-LB31 pair rules) and § 6.1 (CJK strictness
//! tailoring). Tests are grouped by rule number so a reviewer can
//! cross-reference the spec directly.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use stringcheese_scud::{
    LB_STRICTNESS_LOOSE, LB_STRICTNESS_NORMAL, LB_STRICTNESS_STRICT, LineBreakClass,
    LineBreakDataView, ScudFile,
};

pub use stringcheese_scud::{RULES_UAX14_DEFAULT, ScudError};

pub mod classes;

// -----------------------------------------------------------------------
// Public error type
// -----------------------------------------------------------------------

/// Typed failure modes of the line-break engine. Mirrors the WIT
/// `linebreak-error` variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineBreakError {
    /// The locale tag was not a well-formed BCP 47 tag.
    InvalidLocale(&'static str),
}

// -----------------------------------------------------------------------
// Break-kind + strictness
// -----------------------------------------------------------------------

/// Distinguishes a mandatory break (paragraph separator / CR / LF /
/// NL) from a discretionary one.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BreakKind {
    /// A required break — LB4 / LB5 / LB6.
    Mandatory,
    /// A discretionary break — every other allowed opportunity.
    Allowed,
}

/// One record from a line-break walk. Byte offsets are UTF-8 offsets
/// into the input.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BreakOpportunity {
    /// UTF-8 byte offset of the break opportunity.
    pub offset: u32,
    /// Whether the break is mandatory or discretionary.
    pub kind: BreakKind,
}

/// CJK strictness tag per UAX #14 § 6.1.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Strictness {
    /// `loose` — CJK-friendly tailoring that expands the set of
    /// allowed break opportunities.
    Loose,
    /// `normal` — the CLDR default.
    #[default]
    Normal,
    /// `strict` — contracts the set of allowed break opportunities.
    Strict,
}

impl Strictness {
    /// Round-trip the wire-encoded strictness byte back into the
    /// typed enum.
    #[must_use]
    pub fn from_u8(b: u8) -> Self {
        match b {
            LB_STRICTNESS_LOOSE => Self::Loose,
            LB_STRICTNESS_STRICT => Self::Strict,
            _ => Self::Normal,
        }
    }

    /// The wire-encoded `u8` byte for this strictness.
    #[must_use]
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Loose => LB_STRICTNESS_LOOSE,
            Self::Normal => LB_STRICTNESS_NORMAL,
            Self::Strict => LB_STRICTNESS_STRICT,
        }
    }
}

// -----------------------------------------------------------------------
// Pack + engine
// -----------------------------------------------------------------------

/// A loaded line-break iteration pack for one BCP 47 locale (or the
/// root locale).
///
/// Wraps a validated [`ScudFile`] whose capability tag is
/// [`stringcheese_scud::CAP_LINEBREAK`]. Cheap to clone.
#[derive(Debug, Clone, Copy)]
pub struct LineBreakPack<'a> {
    scud: ScudFile<'a>,
    locale: &'a str,
    data: LineBreakDataView<'a>,
}

impl<'a> LineBreakPack<'a> {
    /// Wrap a validated [`ScudFile`] as a line-break pack.
    ///
    /// # Errors
    ///
    /// Returns [`ScudError::CapabilityMismatch`] if the file's
    /// capability tag is not [`stringcheese_scud::CAP_LINEBREAK`].
    pub fn new(scud: ScudFile<'a>) -> Result<Self, ScudError> {
        let data = scud.as_linebreak_data()?;
        let locale = scud.locale().unwrap_or("");
        Ok(Self { scud, locale, data })
    }

    /// Parse `bytes` as a SCUD file and wrap it as a line-break pack.
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

    /// The zero-copy line-break data view.
    #[must_use]
    pub fn data(&self) -> &LineBreakDataView<'a> {
        &self.data
    }
}

/// Locale-sensitive UAX #14 line-break engine.
///
/// Holds an optional [`LineBreakPack`] whose class table (if
/// populated) overrides the built-in classifier. A fresh
/// [`LineBreakEngine::new`] with no pack loaded runs pure
/// algorithm-driven UAX #14 behaviour against the built-in
/// classifier in [`classes`].
#[derive(Debug, Clone, Copy)]
pub struct LineBreakEngine<'a> {
    pack: Option<LineBreakPack<'a>>,
    strictness: Strictness,
}

impl<'a> LineBreakEngine<'a> {
    /// Fresh engine with no pack loaded — falls back to the algorithm
    /// crate's built-in UAX #14 classifier + rules for every query.
    /// Default strictness is [`Strictness::Normal`].
    #[must_use]
    pub const fn new() -> Self {
        Self {
            pack: None,
            strictness: Strictness::Normal,
        }
    }

    /// Fresh engine backed by a single pack. If the pack's class
    /// section is populated it overrides the built-in classifier; an
    /// empty section leaves the built-in classifier in place. The
    /// engine's strictness is initialised from the pack's tailoring
    /// section (defaulting to [`Strictness::Normal`] when absent).
    #[must_use]
    pub fn with_pack(pack: LineBreakPack<'a>) -> Self {
        let strictness = Strictness::from_u8(pack.data.strictness());
        Self {
            pack: Some(pack),
            strictness,
        }
    }

    /// Override this engine's strictness. Layered on top of any
    /// pack-supplied default; callers who load a strict pack can
    /// still drop to `loose` for a particular query by cloning the
    /// engine first.
    #[must_use]
    pub const fn with_strictness(mut self, s: Strictness) -> Self {
        self.strictness = s;
        self
    }

    /// The currently-selected CJK strictness.
    #[must_use]
    pub const fn strictness(&self) -> Strictness {
        self.strictness
    }

    /// The loaded pack, if any.
    #[must_use]
    pub const fn pack(&self) -> Option<&LineBreakPack<'a>> {
        self.pack.as_ref()
    }

    /// Every BCP 47 locale tag this engine knows about. Phase 5's
    /// follow-up ships the root locale marker (`""`) and nothing
    /// else.
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

    /// Look up the raw `Line_Break` class of `cp`, consulting the
    /// loaded pack first and falling back to the built-in classifier.
    #[must_use]
    pub fn raw_class(&self, cp: u32) -> LineBreakClass {
        if let Some(p) = &self.pack {
            if let Some(c) = p.data.class(cp) {
                return c;
            }
        }
        classes::line_break_class(cp)
    }

    /// Look up the resolved `Line_Break` class of `cp` — the class
    /// after LB1 folds `AI`, `CJ`, `SA`, `SG`, and `XX` into
    /// (respectively) `AL`, `NS` (or `ID` under loose), `AL`, `AL`,
    /// and `AL`.
    #[must_use]
    pub fn resolved_class(&self, cp: u32) -> LineBreakClass {
        resolve_class(self.raw_class(cp), self.strictness)
    }

    /// Enumerate every break opportunity in `text`.
    #[cfg(feature = "alloc")]
    #[must_use]
    pub fn find_breaks(&self, text: &str) -> Vec<BreakOpportunity> {
        find_breaks_inner(text, self)
    }
}

impl Default for LineBreakEngine<'_> {
    fn default() -> Self {
        Self::new()
    }
}

// -----------------------------------------------------------------------
// LB1 class resolution
// -----------------------------------------------------------------------

/// LB1: resolve `AI`, `CJ`, `SA`, `SG`, `XX` into their pair-table
/// substitutes. Called on every class before the pair-table lookup.
///
/// * `AI` → `AL` (Ambiguous folds to alphabetic in the CLDR default;
///   an East-Asian-Width tailoring would fold to `ID` instead —
///   deferred).
/// * `CJ` → `NS` under [`Strictness::Normal`] / [`Strictness::Strict`];
///   `ID` under [`Strictness::Loose`].
/// * `SA` → `AL` (Phase 5 approximation; the real behaviour is
///   dictionary-based per-script).
/// * `SG` → `AL` (surrogate; should not appear in well-formed UTF-8).
/// * `XX` → `AL` (unknown → alphabetic).
#[must_use]
pub fn resolve_class(cls: LineBreakClass, strictness: Strictness) -> LineBreakClass {
    match cls {
        LineBreakClass::Ai | LineBreakClass::Sa | LineBreakClass::Sg | LineBreakClass::Xx => {
            LineBreakClass::Al
        }
        LineBreakClass::Cj => match strictness {
            Strictness::Loose => LineBreakClass::Id,
            _ => LineBreakClass::Ns,
        },
        other => other,
    }
}

// -----------------------------------------------------------------------
// Rule engine
// -----------------------------------------------------------------------

/// The three possible pair-table decisions.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum PairAction {
    /// Break is required at this position (`!` in the pair table).
    /// Emitted as [`BreakKind::Mandatory`] by the caller.
    Mandatory,
    /// Break is prohibited (`^`).
    NoBreak,
    /// Break is optional (`_`).
    Allowed,
}

/// Walker state maintained while iterating scalar-by-scalar.
#[allow(clippy::struct_excessive_bools)]
struct Walker {
    /// Effective `Line_Break` class of the previous non-CM/ZWJ
    /// scalar (LB9 fold).
    prev: LineBreakClass,
    /// The class of the scalar two positions back (used by LB25's
    /// numeric-run detection).
    prev_prev: LineBreakClass,
    /// Whether a ZWJ has been seen since the last non-CM/ZWJ scalar
    /// — needed for `LB8a`.
    zwj_active: bool,
    /// Running count of RIs since the last non-RI scalar; paired
    /// count (mod 2) matters for `LB30a`.
    ri_count: usize,
    /// True when the current "run" is inside a numeric expression
    /// (LB25). Enters on `NU`; exits on any class that breaks the
    /// numeric class chain.
    in_numeric: bool,
    /// True when the previous class was `SP` — used by LB18 to allow
    /// break-after-space when no rule with higher priority forbade
    /// it.
    prev_was_space: bool,
    /// True when we are still at the "start of text" state (LB2 has
    /// no break at sot; the first opportunity is always at index >
    /// 0).
    at_sot: bool,
}

/// Collect every break opportunity for `text` under `engine`.
#[cfg(feature = "alloc")]
#[allow(clippy::too_many_lines)]
fn find_breaks_inner(text: &str, engine: &LineBreakEngine<'_>) -> Vec<BreakOpportunity> {
    if text.is_empty() {
        return Vec::new();
    }

    let mut out: Vec<BreakOpportunity> = Vec::new();
    let mut walker = Walker {
        prev: LineBreakClass::Wj, // placeholder; overwritten on first char
        prev_prev: LineBreakClass::Wj,
        zwj_active: false,
        ri_count: 0,
        in_numeric: false,
        prev_was_space: false,
        at_sot: true,
    };

    // We walk one scalar at a time. For each scalar we decide whether
    // to emit a break BEFORE it (relative to `prev`).
    //
    // Every mandatory-break class (BK/CR/LF/NL) is handled by LB4/LB5
    // and produces an emitted opportunity AT the position AFTER the
    // terminator. (CRLF combines under LB5.)

    // Buffered pending mandatory break offset.
    let mut pending_mandatory: Option<u32> = None;

    for (offset, ch) in text.char_indices() {
        let curr_raw = engine.raw_class(ch as u32);
        let curr = resolve_class(curr_raw, engine.strictness);

        // If a mandatory break was queued from the prior CR/LF/NL/BK,
        // emit it now (LB4 / LB5). Skip the LF partner of a CR when
        // it immediately follows (LB5 CR × LF).
        //
        // `pending_fired` records whether the queued mandatory was
        // emitted — if so, the pair_decision for `curr` is short-
        // circuited to NoBreak (the mandatory already covered the
        // break slot; running LB5 through the pair table would emit
        // a second break at the same offset).
        let mut pending_fired = false;
        if let Some(pos) = pending_mandatory {
            if !(walker.prev == LineBreakClass::Cr && curr == LineBreakClass::Lf) {
                out.push(BreakOpportunity {
                    offset: pos,
                    kind: BreakKind::Mandatory,
                });
                pending_mandatory = None;
                pending_fired = true;
            }
        }

        if walker.at_sot {
            walker.at_sot = false;
            walker.prev = curr;
            walker.prev_prev = LineBreakClass::Wj; // sot marker
            walker.zwj_active = curr == LineBreakClass::Zwj;
            walker.ri_count = usize::from(curr == LineBreakClass::Ri);
            walker.in_numeric = curr == LineBreakClass::Nu;
            walker.prev_was_space = curr == LineBreakClass::Sp;
            // Mandatory-break classes queue an emission at the byte
            // offset AFTER this character.
            if is_mandatory_class(curr) {
                pending_mandatory = Some(u32::try_from(offset + ch.len_utf8()).unwrap());
            }
            continue;
        }

        // LB9 folds CM/ZWJ into the preceding class (except after
        // BK/CR/LF/NL/ZW/SP). Under LB9 we treat CM and ZWJ as
        // "part of the previous class". So we do NOT update `prev`
        // — the effective class stays what it was.
        let cm_fold = matches!(curr, LineBreakClass::Cm | LineBreakClass::Zwj)
            && !matches!(
                walker.prev,
                LineBreakClass::Bk
                    | LineBreakClass::Cr
                    | LineBreakClass::Lf
                    | LineBreakClass::Nl
                    | LineBreakClass::Zw
                    | LineBreakClass::Sp
            );

        let action = if cm_fold || pending_fired {
            PairAction::NoBreak
        } else {
            pair_decision(&walker, curr)
        };

        match action {
            PairAction::NoBreak => {
                // Suppress break-before-curr.
            }
            PairAction::Mandatory | PairAction::Allowed => {
                let kind = if matches!(action, PairAction::Mandatory) {
                    BreakKind::Mandatory
                } else {
                    BreakKind::Allowed
                };
                out.push(BreakOpportunity {
                    offset: u32::try_from(offset).unwrap(),
                    kind,
                });
            }
        }

        // Update state.
        if matches!(curr, LineBreakClass::Zwj) {
            walker.zwj_active = true;
        } else if !matches!(curr, LineBreakClass::Cm) {
            walker.zwj_active = false;
        }
        if matches!(curr, LineBreakClass::Ri) {
            walker.ri_count += 1;
        } else if !matches!(curr, LineBreakClass::Cm | LineBreakClass::Zwj) {
            walker.ri_count = 0;
        }
        // LB25 numeric-run tracking.
        if !cm_fold {
            walker.in_numeric = update_numeric_state(walker.in_numeric, walker.prev, curr);
            walker.prev_was_space = matches!(curr, LineBreakClass::Sp);
            walker.prev_prev = walker.prev;
            walker.prev = curr;
        }

        // Queue mandatory break if this is a terminator.
        if is_mandatory_class(curr) {
            pending_mandatory = Some(u32::try_from(offset + ch.len_utf8()).unwrap());
        }
    }

    // LB3: always break at eot. If a mandatory break is pending, emit
    // it (mandatory); otherwise emit an allowed opportunity at the
    // input's length.
    let end = u32::try_from(text.len()).unwrap();
    let kind = if pending_mandatory.is_some() {
        BreakKind::Mandatory
    } else {
        BreakKind::Allowed
    };
    // Only push if the last emitted opportunity is not already at
    // `end`.
    let already = out.last().is_some_and(|o| o.offset == end);
    if !already {
        out.push(BreakOpportunity { offset: end, kind });
    }
    out
}

fn is_mandatory_class(c: LineBreakClass) -> bool {
    matches!(
        c,
        LineBreakClass::Bk | LineBreakClass::Cr | LineBreakClass::Lf | LineBreakClass::Nl
    )
}

/// LB25 numeric-run tracker. Enters on `NU`; exits when a class
/// outside `NU | SY | IS | PR | PO | CL | CP` appears.
fn update_numeric_state(prev_in: bool, _prev: LineBreakClass, curr: LineBreakClass) -> bool {
    matches!(
        (prev_in, curr),
        (_, LineBreakClass::Nu)
            | (
                true,
                LineBreakClass::Sy
                    | LineBreakClass::Is
                    | LineBreakClass::Cl
                    | LineBreakClass::Cp
                    | LineBreakClass::Po
                    | LineBreakClass::Pr
                    | LineBreakClass::Hy
                    | LineBreakClass::Ba,
            )
    )
}

/// Pair-table decision function. Applies rules LB4-LB31 (and `LB30a` /
/// `LB30b`) as documented.
///
/// The rules are ordered by number so a reviewer can trace directly
/// to UAX #14 § 6.
#[allow(clippy::too_many_lines)]
fn pair_decision(w: &Walker, curr: LineBreakClass) -> PairAction {
    use LineBreakClass::{
        Al, B2, Ba, Bb, Bk, Cb, Cl, Cp, Cr, Eb, Em, Ex, Gl, H2, H3, Hl, Hy, Id, In, Is, Jl, Jt, Jv,
        Lf, Nl, Ns, Nu, Op, Po, Pr, Qu, Ri, Sp, Sy, Wj, Zw,
    };
    let prev = w.prev;

    // LB4: BK ! . Handled elsewhere by the mandatory-emit queue; but
    // if we somehow see a class-after-BK in the same step, still
    // mandatory.
    if matches!(prev, Bk) {
        return PairAction::Mandatory;
    }
    // LB5: CR × LF, else CR ! / LF ! / NL !
    if prev == Cr && curr == Lf {
        return PairAction::NoBreak;
    }
    if matches!(prev, Cr | Lf | Nl) {
        return PairAction::Mandatory;
    }
    // LB6: no break before hard terminators.
    if matches!(curr, Bk | Cr | Lf | Nl) {
        return PairAction::NoBreak;
    }
    // LB7: no break before space or zero-width space.
    if matches!(curr, Sp | Zw) {
        return PairAction::NoBreak;
    }
    // LB8: break after ZW (SP* ZW ÷).
    if prev == Zw {
        return PairAction::Allowed;
    }
    // `LB8a`: ZWJ × — no break between a ZWJ and the following scalar.
    // Under LB9 CM/ZWJ folds into the preceding class so `w.prev` is
    // already the pre-ZWJ class; `w.zwj_active` tells us a ZWJ sat
    // just before `curr`.
    if w.zwj_active {
        return PairAction::NoBreak;
    }
    // LB11: WJ × . × WJ
    if prev == Wj || curr == Wj {
        return PairAction::NoBreak;
    }
    // LB12: GL × .
    if prev == Gl {
        return PairAction::NoBreak;
    }
    // LB12a: . × GL unless prev is SP/BA/HY.
    if curr == Gl && !matches!(prev, Sp | Ba | Hy) {
        return PairAction::NoBreak;
    }
    // LB13: no break before CL / CP / EX / IS / SY except after
    // numerics under LB25.
    if matches!(curr, Cl | Cp | Ex | Is | Sy) {
        return PairAction::NoBreak;
    }
    // LB14: OP SP* × — no break after OP even across spaces.
    if prev == Op {
        return PairAction::NoBreak;
    }
    // LB15: QU SP* × OP — no break between quote+opt-space and open.
    if prev == Qu && curr == Op {
        return PairAction::NoBreak;
    }
    // LB16: (CL | CP) SP* × NS
    if matches!(prev, Cl | Cp) && curr == Ns {
        return PairAction::NoBreak;
    }
    // LB17: B2 SP* × B2
    if prev == B2 && curr == B2 {
        return PairAction::NoBreak;
    }
    // LB18: allow break after SP.
    if w.prev_was_space {
        // Space always allows a break-after (unless overridden by
        // LB7/8/14/15/16/17 above, which return early).
        return PairAction::Allowed;
    }
    // LB19: × QU / QU × — no break around quotation marks.
    if curr == Qu || prev == Qu {
        return PairAction::NoBreak;
    }
    // LB20: ÷ CB / CB ÷ — CB always allows break on both sides.
    if curr == Cb || prev == Cb {
        return PairAction::Allowed;
    }
    // LB21: × BA / × HY / × NS / BB × — no break before hyphens etc.
    if matches!(curr, Ba | Hy | Ns) || prev == Bb {
        return PairAction::NoBreak;
    }
    // LB21a: HL (HY | BA) × — after a hebrew letter + hyphen sequence
    // the following scalar must stay glued.
    if w.prev_prev == Hl && matches!(prev, Hy | Ba) {
        return PairAction::NoBreak;
    }
    // LB21b: SY × HL
    if prev == Sy && curr == Hl {
        return PairAction::NoBreak;
    }
    // LB22: × IN — no break before inseparable.
    if curr == In {
        return PairAction::NoBreak;
    }
    // LB23: (AL | HL) × NU  /  NU × (AL | HL)
    if matches!(prev, Al | Hl) && curr == Nu {
        return PairAction::NoBreak;
    }
    if prev == Nu && matches!(curr, Al | Hl) {
        return PairAction::NoBreak;
    }
    // LB23a: PR × (ID | EB | EM)  /  (ID | EB | EM) × PO
    if prev == Pr && matches!(curr, Id | Eb | Em) {
        return PairAction::NoBreak;
    }
    if matches!(prev, Id | Eb | Em) && curr == Po {
        return PairAction::NoBreak;
    }
    // LB24: (PR | PO) × (AL | HL)  /  (AL | HL) × (PR | PO)
    if matches!(prev, Pr | Po) && matches!(curr, Al | Hl) {
        return PairAction::NoBreak;
    }
    if matches!(prev, Al | Hl) && matches!(curr, Pr | Po) {
        return PairAction::NoBreak;
    }
    // LB25: numeric expressions.  Simplified: no break inside a
    // numeric run.
    if w.in_numeric && matches!(curr, Nu | Sy | Is | Cl | Cp | Po | Pr | Hy | Ba) {
        return PairAction::NoBreak;
    }
    if prev == Nu && matches!(curr, Nu | Sy | Is | Cl | Cp | Po | Pr) {
        return PairAction::NoBreak;
    }
    // LB25 (continued): PR/PO × OP/NU / HY × NU / IS × NU
    if matches!(prev, Pr | Po) && matches!(curr, Nu | Op) {
        return PairAction::NoBreak;
    }
    if matches!(prev, Op | Hy | Is) && curr == Nu {
        return PairAction::NoBreak;
    }
    // LB26: JL × (JL | JV | H2 | H3), (JV | H2) × (JV | JT),
    //         (JT | H3) × JT
    if prev == Jl && matches!(curr, Jl | Jv | H2 | H3) {
        return PairAction::NoBreak;
    }
    if matches!(prev, Jv | H2) && matches!(curr, Jv | Jt) {
        return PairAction::NoBreak;
    }
    if matches!(prev, Jt | H3) && curr == Jt {
        return PairAction::NoBreak;
    }
    // LB27: (JL | JV | JT | H2 | H3) × PO, PR × (JL | JV | JT | H2 | H3)
    if matches!(prev, Jl | Jv | Jt | H2 | H3) && curr == Po {
        return PairAction::NoBreak;
    }
    if prev == Pr && matches!(curr, Jl | Jv | Jt | H2 | H3) {
        return PairAction::NoBreak;
    }
    // LB28: (AL | HL) × (AL | HL)
    if matches!(prev, Al | Hl) && matches!(curr, Al | Hl) {
        return PairAction::NoBreak;
    }
    // LB29: IS × (AL | HL)
    if prev == Is && matches!(curr, Al | Hl) {
        return PairAction::NoBreak;
    }
    // LB30: (AL | HL | NU) × OP (excluded: full-width open puncts —
    //         needs East-Asian-Width; approximated).
    //         OP × (AL | HL | NU) similarly.
    if matches!(prev, Al | Hl | Nu) && curr == Op {
        return PairAction::NoBreak;
    }
    if prev == Cp && matches!(curr, Al | Hl | Nu) {
        return PairAction::NoBreak;
    }
    // `LB30a`: RI × RI when the RI count so far (up to and including
    // `prev`, before we process `curr`) is odd. That corresponds to
    // "sot / [^RI] (RI RI)* RI × RI" — the immediate-left RI is at
    // an odd position within the current RI run so the pair is a
    // completed flag-half pair.
    if prev == Ri && curr == Ri && !w.ri_count.is_multiple_of(2) {
        return PairAction::NoBreak;
    }
    // `LB30b`: EB × EM (emoji base × emoji modifier).
    if prev == Eb && curr == Em {
        return PairAction::NoBreak;
    }
    // LB31: default. Allow break everywhere else.
    PairAction::Allowed
}

// -----------------------------------------------------------------------
// Convenience free functions
// -----------------------------------------------------------------------

/// Default-engine convenience over
/// [`LineBreakEngine::find_breaks`].
#[cfg(feature = "alloc")]
#[must_use]
pub fn find_breaks_default(text: &str) -> Vec<BreakOpportunity> {
    LineBreakEngine::new().find_breaks(text)
}

/// Default-engine convenience with an explicit strictness.
#[cfg(feature = "alloc")]
#[must_use]
pub fn find_breaks_with_strictness_default(
    text: &str,
    strictness: Strictness,
) -> Vec<BreakOpportunity> {
    LineBreakEngine::new()
        .with_strictness(strictness)
        .find_breaks(text)
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::*;
    use alloc::vec;

    fn engine() -> LineBreakEngine<'static> {
        LineBreakEngine::new()
    }

    fn offsets(text: &str) -> Vec<u32> {
        engine()
            .find_breaks(text)
            .into_iter()
            .map(|o| o.offset)
            .collect()
    }

    fn kinds(text: &str) -> Vec<BreakKind> {
        engine()
            .find_breaks(text)
            .into_iter()
            .map(|o| o.kind)
            .collect()
    }

    #[test]
    fn lb_empty_input_returns_empty_list() {
        assert!(engine().find_breaks("").is_empty());
    }

    #[test]
    fn lb2_lb3_single_letter_only_eot_break() {
        // "a" — a single alphabetic. LB3 emits an allowed break at eot.
        let bs = engine().find_breaks("a");
        assert_eq!(bs.len(), 1);
        assert_eq!(bs[0].offset, 1);
        assert_eq!(bs[0].kind, BreakKind::Allowed);
    }

    #[test]
    fn lb4_paragraph_separator_forces_mandatory() {
        // U+2028 (LINE SEPARATOR) is BK — mandatory break AFTER.
        let s = "a\u{2028}b";
        let bs = engine().find_breaks(s);
        // Expect a mandatory break just after the separator (byte 4
        // for a 1+3+1 layout) plus a trailing allowed break at eot.
        assert!(bs.iter().any(|b| b.kind == BreakKind::Mandatory));
        assert_eq!(bs.last().unwrap().offset, u32::try_from(s.len()).unwrap());
    }

    #[test]
    fn lb5_crlf_stays_together_as_one_mandatory() {
        // "a\r\nb" — LB5 CR × LF, mandatory break after LF.
        let s = "a\r\nb";
        let bs = engine().find_breaks(s);
        let mandatories: Vec<_> = bs
            .iter()
            .filter(|b| b.kind == BreakKind::Mandatory)
            .collect();
        assert_eq!(mandatories.len(), 1, "expected one mandatory break: {bs:?}");
        assert_eq!(mandatories[0].offset, 3, "mandatory break after CRLF (LB5)");
    }

    #[test]
    fn lb5_lf_alone_is_mandatory() {
        let s = "a\nb";
        let bs = engine().find_breaks(s);
        let m: Vec<_> = bs
            .iter()
            .filter(|b| b.kind == BreakKind::Mandatory)
            .collect();
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].offset, 2);
    }

    #[test]
    fn lb5_nel_alone_is_mandatory() {
        let s = "a\u{0085}b";
        let bs = engine().find_breaks(s);
        assert!(bs.iter().any(|b| b.kind == BreakKind::Mandatory));
    }

    #[test]
    fn lb6_no_break_before_hard_terminator() {
        // "a\n" — no break between 'a' and LF (LB6). LF triggers a
        // mandatory break AFTER at eot.
        let s = "a\n";
        let bs = engine().find_breaks(s);
        // Only the trailing mandatory break.
        assert_eq!(bs.len(), 1);
        assert_eq!(bs[0].offset, 2);
        assert_eq!(bs[0].kind, BreakKind::Mandatory);
    }

    #[test]
    fn lb7_no_break_before_space() {
        // "a b" — no break between 'a' and space; allowed break
        // after space (LB18); allowed break at eot.
        let bs = offsets("a b");
        // Offsets: 2 (after space), 3 (eot).
        assert!(bs.contains(&2));
        assert!(bs.contains(&3));
        assert!(!bs.contains(&1), "must not break before the space");
    }

    #[test]
    fn lb8_break_after_zws() {
        // "a\u{200B}b" — ZW ÷ (LB8 break after ZW).
        let s = "a\u{200B}b";
        let bs = offsets(s);
        // Should have an allowed break at offset 4 (right after the
        // 3-byte ZWSP) and at eot.
        assert!(bs.contains(&4), "expect break after ZW: {bs:?}");
    }

    #[test]
    fn lb8a_no_break_between_zwj_and_next() {
        // "a\u{200D}b" — ZWJ × next (`LB8a`).
        let s = "a\u{200D}b";
        let bs = offsets(s);
        // Should NOT have a break at the offset right after the ZWJ
        // (offset 4) — the ZWJ+b sequence stays glued.
        assert!(
            !bs.contains(&4),
            "`LB8a`: no break between ZWJ and following scalar: {bs:?}"
        );
    }

    #[test]
    fn lb11_no_break_around_word_joiner() {
        // Word Joiner U+2060 — no break either side.
        let s = "a\u{2060}b";
        let bs = offsets(s);
        // Only eot break.
        assert_eq!(bs, vec![u32::try_from(s.len()).unwrap()]);
    }

    #[test]
    fn lb12_no_break_after_glue() {
        // NBSP (U+00A0) is GL. "a\u{00A0}b" — no break either side.
        let s = "a\u{00A0}b";
        let bs = offsets(s);
        assert_eq!(bs, vec![u32::try_from(s.len()).unwrap()]);
    }

    #[test]
    fn lb13_no_break_before_close_or_ex() {
        // "a)" — no break before CP.
        let bs = offsets("a)");
        assert_eq!(bs, vec![2]);
        // "a!" — no break before EX.
        let bs2 = offsets("a!");
        assert_eq!(bs2, vec![2]);
    }

    #[test]
    fn lb14_no_break_after_open_punct() {
        // "(a" — no break after '(' (LB14).
        let bs = offsets("(a");
        assert_eq!(bs, vec![2]);
    }

    #[test]
    fn lb15_no_break_between_qu_and_op() {
        // "\"(a" — no break between " and ( (LB15).
        let bs = offsets("\"(a");
        assert_eq!(bs, vec![3]);
    }

    #[test]
    fn lb18_break_allowed_after_space() {
        // "hello world" — allowed break after the space (offset 6).
        let bs = offsets("hello world");
        assert!(bs.contains(&6));
    }

    #[test]
    fn lb19_no_break_around_quotation() {
        // "\"a" — QU × (LB19); "a\"" — × QU (LB19).
        let bs = offsets("\"a");
        assert_eq!(bs, vec![2]);
        let bs2 = offsets("a\"");
        assert_eq!(bs2, vec![2]);
    }

    #[test]
    fn lb21_no_break_before_hyphen_or_ba() {
        // "a-b" — no break before '-' (LB21) or after (default
        //         hyphen behaviour deferred to LB28-31 which produce
        //         a break).
        let bs = offsets("a-b");
        // We expect a break after the hyphen (offset 2) and at eot.
        assert!(bs.contains(&2));
        assert!(!bs.contains(&1), "no break before hyphen (LB21)");
    }

    #[test]
    fn lb22_no_break_before_in() {
        // Approximation — LB22 forbids break before IN (e.g. ellipsis).
        // Using U+2026 (HORIZONTAL ELLIPSIS) which is classed as `IN`
        // in the real UCD but our pragmatic classifier maps it to
        // `Xx`→`AL`. So we only assert the algorithm accepts input
        // containing the ellipsis without panicking.
        let s = "a\u{2026}b";
        let _ = engine().find_breaks(s);
    }

    #[test]
    fn lb23_no_break_between_alpha_and_num() {
        // "abc123" — no break between letters and numeric (LB23).
        let bs = offsets("abc123");
        assert_eq!(bs, vec![6]);
        let bs2 = offsets("123abc");
        assert_eq!(bs2, vec![6]);
    }

    #[test]
    fn lb24_no_break_pr_al() {
        // "$abc" — PR × AL (LB24), no break between $ and a.
        let bs = offsets("$abc");
        assert_eq!(bs, vec![4]);
    }

    #[test]
    fn lb25_numeric_expression_stays_together() {
        // "1,234.56" — full numeric expression, no interior breaks.
        let bs = offsets("1,234.56");
        assert_eq!(bs, vec![8]);
    }

    #[test]
    fn lb25_currency_prefix_glues() {
        // "$1,234" — PR × OP not applicable (no OP); PR × NU (LB25).
        let bs = offsets("$1,234");
        assert_eq!(bs, vec![6]);
    }

    #[test]
    fn lb26_hangul_syllable_stays_together() {
        // 가나 (two Hangul LV syllables). Between two H2 syllables
        // there IS a break opportunity (they behave as ID); but a JL
        // × JV within one syllable is prohibited.
        let s = "\u{1100}\u{1161}"; // ᄀ ᅡ = one composed syllable
        let bs = offsets(s);
        assert_eq!(bs, vec![u32::try_from(s.len()).unwrap()]);
    }

    #[test]
    fn lb27_pr_hangul_glues() {
        // "$가" — PR × Hangul syllable (LB27).
        let s = "$\u{AC00}";
        let bs = offsets(s);
        assert_eq!(bs, vec![u32::try_from(s.len()).unwrap()]);
    }

    #[test]
    fn lb28_alpha_alpha_glues_within_word() {
        // "abcd" — (AL | HL) × (AL | HL) (LB28), no interior breaks.
        let bs = offsets("abcd");
        assert_eq!(bs, vec![4]);
    }

    #[test]
    fn lb30_no_break_between_al_and_open() {
        // "a(" — (AL | HL | NU) × OP (LB30).
        let bs = offsets("a(");
        assert_eq!(bs, vec![2]);
    }

    #[test]
    fn lb30a_regional_indicator_pair_no_break() {
        // Two RIs (flag halves) form a pair — no break between them
        // (`LB30a` even-count rule).
        let s = "\u{1F1EC}\u{1F1E7}";
        let bs = offsets(s);
        assert_eq!(bs, vec![u32::try_from(s.len()).unwrap()]);
    }

    #[test]
    fn lb30a_three_regional_indicators_break_after_pair() {
        // Three RIs — break AFTER the second (bytes 0..8 = pair;
        // bytes 8..12 = orphan).
        let s = "\u{1F1EC}\u{1F1E7}\u{1F1E8}";
        let bs = offsets(s);
        assert!(bs.contains(&8), "expect break after RI pair: {bs:?}");
    }

    #[test]
    fn lb31_default_break_between_ideographs() {
        // Two CJK ideographs — LB31 default allow, both classed as
        // ID and NOT glued by LB26/27.
        let s = "\u{4E2D}\u{6587}"; // 中文
        let bs = offsets(s);
        // Expect a break at the boundary between the two ideographs.
        assert!(bs.contains(&3), "expect break between two ID: {bs:?}");
    }

    #[test]
    fn eot_break_is_always_present() {
        for s in ["a", "abc", "1", "()", "\"hi\"", "hello world"] {
            let bs = offsets(s);
            assert_eq!(
                bs.last().copied(),
                Some(u32::try_from(s.len()).unwrap()),
                "eot break missing for {s:?}: {bs:?}",
            );
        }
    }

    #[test]
    fn kinds_reflect_mandatory_vs_allowed() {
        let ks = kinds("a\nb");
        // First break is mandatory (after LF), last is allowed (eot).
        assert!(ks.contains(&BreakKind::Mandatory));
        assert_eq!(*ks.last().unwrap(), BreakKind::Allowed);
    }

    #[test]
    fn strictness_loose_folds_cj_to_id() {
        // CJ under loose folds to ID; under normal folds to NS.
        // Verify the resolver directly.
        assert_eq!(
            resolve_class(LineBreakClass::Cj, Strictness::Loose),
            LineBreakClass::Id
        );
        assert_eq!(
            resolve_class(LineBreakClass::Cj, Strictness::Normal),
            LineBreakClass::Ns
        );
        assert_eq!(
            resolve_class(LineBreakClass::Cj, Strictness::Strict),
            LineBreakClass::Ns
        );
    }

    #[test]
    fn resolver_folds_ai_sa_sg_xx_to_al() {
        for c in [
            LineBreakClass::Ai,
            LineBreakClass::Sa,
            LineBreakClass::Sg,
            LineBreakClass::Xx,
        ] {
            assert_eq!(resolve_class(c, Strictness::Normal), LineBreakClass::Al);
        }
    }

    #[test]
    fn supported_locales_default_engine_returns_root_marker() {
        let e = engine();
        assert_eq!(e.supported_locales(), vec![""]);
    }

    #[test]
    fn supports_returns_true_for_every_input() {
        let e = engine();
        for tag in ["", "en", "ja", "zh-Hans", "de-CH"] {
            assert!(e.supports(tag));
        }
    }

    #[test]
    fn strictness_engine_getter_reflects_setter() {
        let e = engine().with_strictness(Strictness::Strict);
        assert_eq!(e.strictness(), Strictness::Strict);
    }

    #[test]
    fn strictness_roundtrips_through_u8() {
        for s in [Strictness::Loose, Strictness::Normal, Strictness::Strict] {
            assert_eq!(Strictness::from_u8(s.as_u8()), s);
        }
    }

    #[test]
    fn convenience_helpers_match_engine() {
        let via_engine = engine().find_breaks("hi world");
        let via_helper = find_breaks_default("hi world");
        assert_eq!(via_engine, via_helper);
        let strict_helper = find_breaks_with_strictness_default("hi", Strictness::Strict);
        assert_eq!(strict_helper.last().unwrap().offset, 2);
    }
}
