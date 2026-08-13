//! Built-in UAX #14 `Line_Break` property classifier.
//!
//! Compact `match`-driven classifier that consumes a Unicode scalar
//! and returns the corresponding [`LineBreakClass`] discriminant.
//!
//! # Coverage vs. size
//!
//! The full UCD `LineBreak.txt` table covers thousands of ranges
//! across all 17 Unicode planes. Phase 5's follow-up ships a
//! **pragmatic subset** covering: ASCII, Latin-1 Supplement, Latin
//! Extended-A/B (through the Basic Multilingual Plane), CJK
//! Ideographs, Hangul (jamo + precomposed), Regional Indicators
//! (`U+1F1E6..U+1F1FF`), combining marks (`U+0300..U+036F`),
//! variation selectors (`U+FE00..U+FE0F`), ZWJ (`U+200D`), the main
//! emoji planes (`U+1F300..U+1FAFF`), and the punctuation +
//! whitespace classes used by the LB rules.
//!
//! Scalars outside the covered ranges fall through to
//! [`LineBreakClass::Xx`]; the algorithm's `resolve_class` fold
//! ([`crate::resolve_class`]) maps them to `AL` per LB1. A caller
//! that supplies its own class ranges through a SCUD pack overrides
//! the built-in classifier via
//! [`crate::LineBreakEngine::with_pack`].
//!
//! # Data provenance
//!
//! Ranges derived from the Unicode 15.1 UCD data files:
//!
//! * `LineBreak.txt` — <https://www.unicode.org/Public/15.1.0/ucd/LineBreak.txt>
//! * `emoji-data.txt` (`Extended_Pictographic`; used for `EB` / `EM`
//!   mixed with the LineBreak.txt classes) —
//!   <https://www.unicode.org/Public/15.1.0/ucd/emoji/emoji-data.txt>

use stringcheese_scud::LineBreakClass;

/// Classify a Unicode scalar under `Line_Break`. Returns
/// [`LineBreakClass::Xx`] for unclassified scalars — the algorithm's
/// LB1 resolution step folds `Xx` into `AL` before the pair table is
/// consulted.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn line_break_class(cp: u32) -> LineBreakClass {
    // ASCII carve-outs first (frequent).
    match cp {
        0x000A => return LineBreakClass::Lf,
        0x000D => return LineBreakClass::Cr,
        0x0085 => return LineBreakClass::Nl,
        0x2028 | 0x2029 => return LineBreakClass::Bk,
        0x0009 => return LineBreakClass::Ba, // TAB — BA per UAX #14
        0x0020 => return LineBreakClass::Sp,
        0x00A0 => return LineBreakClass::Gl, // NO-BREAK SPACE — GL
        0x200B => return LineBreakClass::Zw,
        0x200C => return LineBreakClass::Cm, // ZWNJ — CM
        0x200D => return LineBreakClass::Zwj,
        0x2060 | 0xFEFF => return LineBreakClass::Wj,
        _ => {}
    }
    // Regional Indicators.
    if (0x1F1E6..=0x1F1FF).contains(&cp) {
        return LineBreakClass::Ri;
    }
    // Hangul jamo -> JL/JV/JT.
    if (0x1100..=0x115F).contains(&cp) || (0xA960..=0xA97C).contains(&cp) {
        return LineBreakClass::Jl;
    }
    if (0x1160..=0x11A7).contains(&cp) || (0xD7B0..=0xD7C6).contains(&cp) {
        return LineBreakClass::Jv;
    }
    if (0x11A8..=0x11FF).contains(&cp) || (0xD7CB..=0xD7FB).contains(&cp) {
        return LineBreakClass::Jt;
    }
    // Precomposed Hangul syllables: LV (multiple of 28) vs LVT.
    if (0xAC00..=0xD7A3).contains(&cp) {
        return if (cp - 0xAC00).is_multiple_of(28) {
            LineBreakClass::H2
        } else {
            LineBreakClass::H3
        };
    }
    // CJK Ideographs -> ID.
    if (0x3400..=0x4DBF).contains(&cp)
        || (0x4E00..=0x9FFF).contains(&cp)
        || (0x20000..=0x2A6DF).contains(&cp)
        || (0x2A700..=0x2EBEF).contains(&cp)
        || (0x30000..=0x3134F).contains(&cp)
        || (0xF900..=0xFAFF).contains(&cp)
    // CJK Compatibility Ideographs
    {
        return LineBreakClass::Id;
    }
    // Hiragana / Katakana -> ID (with small-kana carve-outs handled
    // by is_small_kana below producing CJ).
    if is_small_kana(cp) {
        return LineBreakClass::Cj;
    }
    if (0x3040..=0x309F).contains(&cp) || (0x30A0..=0x30FF).contains(&cp) {
        return LineBreakClass::Id;
    }
    // Kana repetition marks — NS per UAX #14 (or CJ under loose /
    // strict); pass as NS for the default.
    if matches!(
        cp,
        0x3005 | 0x303B | 0x3041 | 0x3043 | 0x3045 | 0x3047 | 0x3049
    ) {
        return LineBreakClass::Ns;
    }
    // Combining diacritical marks and mark ranges -> CM.
    if (0x0300..=0x036F).contains(&cp)
        || (0x1AB0..=0x1AFF).contains(&cp)
        || (0x1DC0..=0x1DFF).contains(&cp)
        || (0x20D0..=0x20FF).contains(&cp)
        || (0xFE20..=0xFE2F).contains(&cp)
    {
        return LineBreakClass::Cm;
    }
    // Variation selectors + Mongolian FVS -> CM.
    if (0xFE00..=0xFE0F).contains(&cp) || (0xE0100..=0xE01EF).contains(&cp) || cp == 0x180E {
        return LineBreakClass::Cm;
    }
    // Emoji base / modifier (subset — real EB/EM extends further but
    // covers the interactive smoke cases we need).
    if is_emoji_base(cp) {
        return LineBreakClass::Eb;
    }
    if (0x1F3FB..=0x1F3FF).contains(&cp) {
        return LineBreakClass::Em;
    }
    // Extended Pictographic (fallback for the rest of the emoji
    // planes) -> ID; matches ICU's default for stand-alone emoji.
    if (0x1F300..=0x1F5FF).contains(&cp)
        || (0x1F600..=0x1F64F).contains(&cp)
        || (0x1F680..=0x1F6FF).contains(&cp)
        || (0x1F900..=0x1F9FF).contains(&cp)
        || (0x1FA70..=0x1FAFF).contains(&cp)
        || (0x2600..=0x27BF).contains(&cp)
    {
        return LineBreakClass::Id;
    }
    // South-East Asian scripts -> SA (dictionary-based; LB1 folds SA
    // to AL in Phase 5 pending dictionary support).
    if (0x0E00..=0x0E7F).contains(&cp) // Thai
        || (0x0E80..=0x0EFF).contains(&cp) // Lao
        || (0x1000..=0x109F).contains(&cp) // Myanmar
        || (0x1780..=0x17FF).contains(&cp)
    {
        return LineBreakClass::Sa;
    }
    // Explicit ASCII / Latin-1 punctuation + digit classification.
    match cp {
        // Digits — Nu.
        0x0030..=0x0039 => return LineBreakClass::Nu,
        // Space -> Sp already handled above.
        // Open punctuation -> OP.
        0x0028 | 0x005B | 0x007B => return LineBreakClass::Op,
        // Close punctuation: ) ] → CP; } → CL.
        0x0029 | 0x005D => return LineBreakClass::Cp,
        0x007D => return LineBreakClass::Cl,
        // Quotation marks -> QU.
        0x0022 | 0x0027 | 0x00AB | 0x00BB | 0x201C | 0x201D | 0x2018 | 0x2019 | 0x2039 | 0x203A => {
            return LineBreakClass::Qu;
        }
        // Exclamation / question / full-width variants -> EX.
        0x0021 | 0x003F | 0xFF01 | 0xFF1F => return LineBreakClass::Ex,
        // Comma / semicolon / colon / etc. -> IS (infix numeric sep).
        // Comma is IS per UAX #14 (behaves as infix inside numerics
        // via LB25).
        0x002C | 0x002E | 0x003A | 0x003B => return LineBreakClass::Is,
        // Solidus / slash -> SY.
        0x002F => return LineBreakClass::Sy,
        // Currency prefix -> PR.
        0x0024 | 0x00A3 | 0x00A5 | 0x20AC => return LineBreakClass::Pr,
        // Percent / permille — PO (postfix numeric).
        0x0025 | 0x2030 | 0x2031 => return LineBreakClass::Po,
        // ASCII hyphen-minus -> HY (also SY, but HY wins under LB25
        // and general HY semantics).
        0x002D => return LineBreakClass::Hy,
        // Middle dot / no-break hyphen -> GL.
        0x2011 => return LineBreakClass::Gl,
        // Break-after: standard hyphens, dashes.
        0x2010 | 0x2012 | 0x2013 => return LineBreakClass::Ba,
        // Em dash -> B2.
        0x2014 => return LineBreakClass::B2,
        // Word joiner -> WJ (already above).
        _ => {}
    }
    // Alphabetic / letters — most of the BMP. Give ASCII / Latin-1 /
    // Latin Extended letters AL.
    if is_letter(cp) {
        // Hebrew letters -> HL.
        if (0x0590..=0x05FF).contains(&cp) {
            return LineBreakClass::Hl;
        }
        return LineBreakClass::Al;
    }
    LineBreakClass::Xx
}

fn is_letter(cp: u32) -> bool {
    // ASCII letters.
    (0x0041..=0x005A).contains(&cp)
        || (0x0061..=0x007A).contains(&cp)
        // Latin-1 Supplement letters.
        || (0x00C0..=0x00D6).contains(&cp)
        || (0x00D8..=0x00F6).contains(&cp)
        || (0x00F8..=0x00FF).contains(&cp)
        // Latin Extended-A.
        || (0x0100..=0x017F).contains(&cp)
        // Latin Extended-B.
        || (0x0180..=0x024F).contains(&cp)
        // Greek + Coptic + Cyrillic.
        || (0x0370..=0x03FF).contains(&cp)
        || (0x0400..=0x04FF).contains(&cp)
        || (0x0500..=0x052F).contains(&cp)
        // Hebrew.
        || (0x0590..=0x05FF).contains(&cp)
        // Arabic (approximate — arabic contains many other classes).
        || (0x0600..=0x06FF).contains(&cp)
        // Devanagari / Bengali / Gurmukhi / etc. — pragmatic AL for now.
        || (0x0900..=0x0DFF).contains(&cp)
        // Armenian / Georgian.
        || (0x0530..=0x058F).contains(&cp)
        || (0x10A0..=0x10FF).contains(&cp)
}

/// Small kana (`ぁ`, `ぃ`, …) that under CLDR default strictness are
/// non-starters (`CJ` → `NS`). Loose strictness demotes to `ID`;
/// strict strictness keeps them `NS`.
fn is_small_kana(cp: u32) -> bool {
    matches!(
        cp,
        0x3041 // ぁ
        | 0x3043 // ぃ
        | 0x3045 // ぅ
        | 0x3047 // ぇ
        | 0x3049 // ぉ
        | 0x3063 // っ
        | 0x3083 // ゃ
        | 0x3085 // ゅ
        | 0x3087 // ょ
        | 0x308E // ゎ
        | 0x30A1 // ァ
        | 0x30A3 // ィ
        | 0x30A5 // ゥ
        | 0x30A7 // ェ
        | 0x30A9 // ォ
        | 0x30C3 // ッ
        | 0x30E3 // ャ
        | 0x30E5 // ュ
        | 0x30E7 // ョ
        | 0x30EE // ヮ
    )
}

fn is_emoji_base(cp: u32) -> bool {
    // Pragmatic EB subset — humanoid / face-with-hand emoji that
    // pair with modifier tones.
    matches!(cp, 0x261D | 0x26F9 | 0x270A | 0x270B | 0x270C | 0x270D)
        || (0x1F385..=0x1F385).contains(&cp)
        || (0x1F3C2..=0x1F3C4).contains(&cp)
        || (0x1F3C7..=0x1F3C7).contains(&cp)
        || (0x1F3CA..=0x1F3CC).contains(&cp)
        || (0x1F442..=0x1F443).contains(&cp)
        || (0x1F446..=0x1F450).contains(&cp)
        || (0x1F466..=0x1F478).contains(&cp)
        || (0x1F47C..=0x1F47C).contains(&cp)
        || (0x1F481..=0x1F483).contains(&cp)
        || (0x1F485..=0x1F487).contains(&cp)
        || (0x1F4AA..=0x1F4AA).contains(&cp)
        || (0x1F574..=0x1F575).contains(&cp)
        || (0x1F57A..=0x1F57A).contains(&cp)
        || (0x1F590..=0x1F590).contains(&cp)
        || (0x1F595..=0x1F596).contains(&cp)
        || (0x1F645..=0x1F647).contains(&cp)
        || (0x1F64B..=0x1F64F).contains(&cp)
        || (0x1F6A3..=0x1F6A3).contains(&cp)
        || (0x1F6B4..=0x1F6B6).contains(&cp)
        || (0x1F6C0..=0x1F6C0).contains(&cp)
        || (0x1F6CC..=0x1F6CC).contains(&cp)
        || (0x1F918..=0x1F91C).contains(&cp)
        || (0x1F91E..=0x1F91F).contains(&cp)
        || (0x1F926..=0x1F926).contains(&cp)
        || (0x1F930..=0x1F939).contains(&cp)
        || (0x1F93D..=0x1F93E).contains(&cp)
        || (0x1F9B5..=0x1F9B6).contains(&cp)
        || (0x1F9B8..=0x1F9B9).contains(&cp)
        || (0x1F9BB..=0x1F9BB).contains(&cp)
        || (0x1F9CD..=0x1F9CF).contains(&cp)
        || (0x1F9D1..=0x1F9DD).contains(&cp)
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_letter_is_al() {
        assert_eq!(line_break_class('a' as u32), LineBreakClass::Al);
        assert_eq!(line_break_class('Z' as u32), LineBreakClass::Al);
    }

    #[test]
    fn ascii_digit_is_nu() {
        assert_eq!(line_break_class('0' as u32), LineBreakClass::Nu);
        assert_eq!(line_break_class('9' as u32), LineBreakClass::Nu);
    }

    #[test]
    fn ascii_space_is_sp() {
        assert_eq!(line_break_class(' ' as u32), LineBreakClass::Sp);
    }

    #[test]
    fn nbsp_is_gl() {
        assert_eq!(line_break_class(0x00A0), LineBreakClass::Gl);
    }

    #[test]
    fn line_terminators_map_correctly() {
        assert_eq!(line_break_class(0x000A), LineBreakClass::Lf);
        assert_eq!(line_break_class(0x000D), LineBreakClass::Cr);
        assert_eq!(line_break_class(0x0085), LineBreakClass::Nl);
        assert_eq!(line_break_class(0x2028), LineBreakClass::Bk);
        assert_eq!(line_break_class(0x2029), LineBreakClass::Bk);
    }

    #[test]
    fn ascii_hyphen_is_hy() {
        assert_eq!(line_break_class('-' as u32), LineBreakClass::Hy);
    }

    #[test]
    fn zwj_is_zwj() {
        assert_eq!(line_break_class(0x200D), LineBreakClass::Zwj);
    }

    #[test]
    fn cjk_ideograph_is_id() {
        assert_eq!(line_break_class(0x4E2D), LineBreakClass::Id); // 中
    }

    #[test]
    fn regional_indicator_is_ri() {
        assert_eq!(line_break_class(0x1F1EC), LineBreakClass::Ri);
    }

    #[test]
    fn hangul_syllable_is_h2_or_h3() {
        assert_eq!(line_break_class(0xAC00), LineBreakClass::H2); // 가
        assert_eq!(line_break_class(0xAC01), LineBreakClass::H3);
    }

    #[test]
    fn open_close_punctuation() {
        assert_eq!(line_break_class(u32::from(b'(')), LineBreakClass::Op);
        assert_eq!(line_break_class(u32::from(b')')), LineBreakClass::Cp);
        assert_eq!(line_break_class(u32::from(b'[')), LineBreakClass::Op);
        assert_eq!(line_break_class(u32::from(b']')), LineBreakClass::Cp);
    }

    #[test]
    fn combining_mark_is_cm() {
        assert_eq!(line_break_class(0x0301), LineBreakClass::Cm);
    }

    #[test]
    fn unclassified_is_xx() {
        // U+E000 (Private Use Area start) — no LineBreak coverage in
        // the pragmatic subset.
        assert_eq!(line_break_class(0xE000), LineBreakClass::Xx);
    }

    #[test]
    fn small_kana_is_cj() {
        assert_eq!(line_break_class(0x3041), LineBreakClass::Cj); // ぁ
    }
}
