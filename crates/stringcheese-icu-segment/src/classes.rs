//! Built-in UAX #29 property classifiers.
//!
//! Compact `match`-driven classifiers for the three axes UAX #29
//! defines: `Grapheme_Cluster_Break`, `Word_Break`, `Sentence_Break`.
//! Each classifier accepts a Unicode scalar and returns the
//! corresponding SCUD-shared enum value.
//!
//! # Coverage vs. size
//!
//! The full UCD auxiliary property tables cover thousands of ranges
//! across all 17 Unicode planes. Phase 5 ships a **pragmatic
//! subset** covering: ASCII, Latin-1 Supplement, Latin Extended-A/B
//! (through the Basic Multilingual Plane), Cyrillic, Greek, Hebrew,
//! Arabic, Hangul (jamo + precomposed), CJK Ideographs, Regional
//! Indicators (U+1F1E6..U+1F1FF), ZWJ (U+200D), variation selectors
//! (U+FE00..U+FE0F), combining marks (U+0300..U+036F + adjacent
//! ranges), the main emoji planes (U+1F300..U+1FAFF), and the
//! terminator / punctuation classes used by the SB rules.
//!
//! Scalars outside the covered ranges fall through to the default
//! (`Other` for every axis). A caller that supplies its own class
//! ranges through a SCUD pack overrides the built-in classifier via
//! [`crate::BreakEngine::with_pack`].
//!
//! # Data provenance
//!
//! Ranges derived from the Unicode 15.1 auxiliary property files:
//!
//! * `GraphemeBreakProperty.txt` — <https://www.unicode.org/Public/15.1.0/ucd/auxiliary/GraphemeBreakProperty.txt>
//! * `WordBreakProperty.txt` — <https://www.unicode.org/Public/15.1.0/ucd/auxiliary/WordBreakProperty.txt>
//! * `SentenceBreakProperty.txt` — <https://www.unicode.org/Public/15.1.0/ucd/auxiliary/SentenceBreakProperty.txt>
//! * `emoji-data.txt` (`Extended_Pictographic`) — <https://www.unicode.org/Public/15.1.0/ucd/emoji/emoji-data.txt>

use stringcheese_scud::{GraphemeClass, SentenceClass, WordClass};

/// Classify a Unicode scalar under `Grapheme_Cluster_Break`.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn grapheme_class(cp: u32) -> GraphemeClass {
    // CR / LF are called out separately from the Control run.
    if cp == 0x000D {
        return GraphemeClass::Cr;
    }
    if cp == 0x000A {
        return GraphemeClass::Lf;
    }
    if cp == 0x200D {
        return GraphemeClass::Zwj;
    }
    // Regional indicators (flag halves).
    if (0x1F1E6..=0x1F1FF).contains(&cp) {
        return GraphemeClass::RegionalIndicator;
    }
    // Hangul jamo. Precomposed syllables handled below.
    if (0x1100..=0x115F).contains(&cp) {
        return GraphemeClass::HangulL;
    }
    if (0xA960..=0xA97C).contains(&cp) {
        return GraphemeClass::HangulL;
    }
    if (0x1160..=0x11A7).contains(&cp) {
        return GraphemeClass::HangulV;
    }
    if (0xD7B0..=0xD7C6).contains(&cp) {
        return GraphemeClass::HangulV;
    }
    if (0x11A8..=0x11FF).contains(&cp) {
        return GraphemeClass::HangulT;
    }
    if (0xD7CB..=0xD7FB).contains(&cp) {
        return GraphemeClass::HangulT;
    }
    // Precomposed Hangul syllables (AC00..D7A3). LV vs LVT depends
    // on whether the syllable's trailing-jamo index is zero.
    if (0xAC00..=0xD7A3).contains(&cp) {
        return if (cp - 0xAC00).is_multiple_of(28) {
            GraphemeClass::HangulLv
        } else {
            GraphemeClass::HangulLvt
        };
    }
    // Control runs.
    if is_gcb_control(cp) {
        return GraphemeClass::Control;
    }
    // Prepend.
    if is_gcb_prepend(cp) {
        return GraphemeClass::Prepend;
    }
    // Extended_Pictographic (emoji).
    if is_extended_pictographic(cp) {
        return GraphemeClass::ExtendedPictographic;
    }
    // Extend / SpacingMark.
    if is_gcb_extend(cp) {
        return GraphemeClass::Extend;
    }
    if is_gcb_spacing_mark(cp) {
        return GraphemeClass::SpacingMark;
    }
    GraphemeClass::Other
}

/// Classify a Unicode scalar under `Word_Break`.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn word_class(cp: u32) -> WordClass {
    if cp == 0x000D {
        return WordClass::Cr;
    }
    if cp == 0x000A {
        return WordClass::Lf;
    }
    if cp == 0x200D {
        return WordClass::Zwj;
    }
    if cp == 0x0027 {
        return WordClass::SingleQuote;
    }
    if cp == 0x0022 {
        return WordClass::DoubleQuote;
    }
    if (0x1F1E6..=0x1F1FF).contains(&cp) {
        return WordClass::RegionalIndicator;
    }
    if is_wb_newline(cp) {
        return WordClass::Newline;
    }
    // Katakana.
    if is_katakana(cp) {
        return WordClass::Katakana;
    }
    // Hebrew_Letter.
    if is_hebrew_letter(cp) {
        return WordClass::HebrewLetter;
    }
    if is_wb_wseg_space(cp) {
        return WordClass::WSegSpace;
    }
    if is_wb_extendnumlet(cp) {
        return WordClass::ExtendNumLet;
    }
    if is_wb_midletter(cp) {
        return WordClass::MidLetter;
    }
    if is_wb_midnumlet(cp) {
        return WordClass::MidNumLet;
    }
    if is_wb_midnum(cp) {
        return WordClass::MidNum;
    }
    if is_wb_numeric(cp) {
        return WordClass::Numeric;
    }
    if is_wb_format(cp) {
        return WordClass::Format;
    }
    if is_wb_extend(cp) {
        return WordClass::Extend;
    }
    if is_extended_pictographic(cp) {
        return WordClass::ExtendedPictographic;
    }
    if is_wb_aletter(cp) {
        return WordClass::ALetter;
    }
    WordClass::Other
}

/// Classify a Unicode scalar under `Sentence_Break`.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn sentence_class(cp: u32) -> SentenceClass {
    if cp == 0x000D {
        return SentenceClass::Cr;
    }
    if cp == 0x000A {
        return SentenceClass::Lf;
    }
    if is_sb_sep(cp) {
        return SentenceClass::Sep;
    }
    if is_sb_sp(cp) {
        return SentenceClass::Sp;
    }
    if is_sb_aterm(cp) {
        return SentenceClass::ATerm;
    }
    if is_sb_sterm(cp) {
        return SentenceClass::STerm;
    }
    if is_sb_close(cp) {
        return SentenceClass::Close;
    }
    if is_sb_scontinue(cp) {
        return SentenceClass::SContinue;
    }
    if is_sb_numeric(cp) {
        return SentenceClass::Numeric;
    }
    if is_sb_upper(cp) {
        return SentenceClass::Upper;
    }
    if is_sb_lower(cp) {
        return SentenceClass::Lower;
    }
    if is_sb_oletter(cp) {
        return SentenceClass::OLetter;
    }
    if is_sb_format(cp) {
        return SentenceClass::Format;
    }
    if is_sb_extend(cp) {
        return SentenceClass::Extend;
    }
    SentenceClass::Other
}

// -----------------------------------------------------------------------
// Grapheme_Cluster_Break helpers
// -----------------------------------------------------------------------

fn is_gcb_control(cp: u32) -> bool {
    // Cc excluding CR/LF; a broad slice of Cf; line/paragraph
    // separators; various format-related runs.
    matches!(
        cp,
        0x0000..=0x0009
        | 0x000B..=0x000C
        | 0x000E..=0x001F
        | 0x007F..=0x009F
        | 0x00AD
        | 0x061C
        | 0x180E
        | 0x200B
        | 0x200E..=0x200F
        | 0x2028..=0x202E
        | 0x2060..=0x2064
        | 0x2066..=0x206F
        | 0xFEFF
        | 0xFFF0..=0xFFFB
    )
}

#[allow(clippy::too_many_lines)]
fn is_gcb_extend(cp: u32) -> bool {
    // Nonspacing marks (Mn) + Variation Selectors + a subset of Me.
    // Broad ranges covering the most common combining marks.
    matches!(
        cp,
        0x0300..=0x036F
        | 0x0483..=0x0489
        | 0x0591..=0x05BD
        | 0x05BF
        | 0x05C1..=0x05C2
        | 0x05C4..=0x05C5
        | 0x05C7
        | 0x0610..=0x061A
        | 0x064B..=0x065F
        | 0x0670
        | 0x06D6..=0x06DC
        | 0x06DF..=0x06E4
        | 0x06E7..=0x06E8
        | 0x06EA..=0x06ED
        | 0x0711
        | 0x0730..=0x074A
        | 0x07A6..=0x07B0
        | 0x07EB..=0x07F3
        | 0x0816..=0x0819
        | 0x081B..=0x0823
        | 0x0825..=0x0827
        | 0x0829..=0x082D
        | 0x0859..=0x085B
        | 0x08D3..=0x08E1
        | 0x08E3..=0x0902
        | 0x093A
        | 0x093C
        | 0x0941..=0x0948
        | 0x094D
        | 0x0951..=0x0957
        | 0x0962..=0x0963
        | 0x0981
        | 0x09BC
        | 0x09C1..=0x09C4
        | 0x09CD
        | 0x09E2..=0x09E3
        | 0x09FE
        | 0x0A01..=0x0A02
        | 0x0A3C
        | 0x0A41..=0x0A42
        | 0x0A47..=0x0A48
        | 0x0A4B..=0x0A4D
        | 0x0A51
        | 0x0A70..=0x0A71
        | 0x0A75
        | 0x0A81..=0x0A82
        | 0x0ABC
        | 0x0AC1..=0x0AC5
        | 0x0AC7..=0x0AC8
        | 0x0ACD
        | 0x0AE2..=0x0AE3
        | 0x0AFA..=0x0AFF
        | 0x0B01
        | 0x0B3C
        | 0x0B3F
        | 0x0B41..=0x0B44
        | 0x0B4D
        | 0x0B55..=0x0B56
        | 0x0B62..=0x0B63
        | 0x0B82
        | 0x0BC0
        | 0x0BCD
        | 0x0C00
        | 0x0C04
        | 0x0C3E..=0x0C40
        | 0x0C46..=0x0C48
        | 0x0C4A..=0x0C4D
        | 0x0C55..=0x0C56
        | 0x0C62..=0x0C63
        | 0x0C81
        | 0x0CBC
        | 0x0CBF
        | 0x0CC6
        | 0x0CCC..=0x0CCD
        | 0x0CE2..=0x0CE3
        | 0x0D00..=0x0D01
        | 0x0D3B..=0x0D3C
        | 0x0D41..=0x0D44
        | 0x0D4D
        | 0x0D62..=0x0D63
        | 0x0DCA
        | 0x0DD2..=0x0DD4
        | 0x0DD6
        | 0x0E31
        | 0x0E34..=0x0E3A
        | 0x0E47..=0x0E4E
        | 0x0EB1
        | 0x0EB4..=0x0EBC
        | 0x0EC8..=0x0ECD
        | 0x0F18..=0x0F19
        | 0x0F35
        | 0x0F37
        | 0x0F39
        | 0x0F71..=0x0F7E
        | 0x0F80..=0x0F84
        | 0x0F86..=0x0F87
        | 0x0F8D..=0x0F97
        | 0x0F99..=0x0FBC
        | 0x0FC6
        | 0x102D..=0x1030
        | 0x1032..=0x1037
        | 0x1039..=0x103A
        | 0x103D..=0x103E
        | 0x1058..=0x1059
        | 0x105E..=0x1060
        | 0x1071..=0x1074
        | 0x1082
        | 0x1085..=0x1086
        | 0x108D
        | 0x109D
        | 0x135D..=0x135F
        | 0x1712..=0x1714
        | 0x1732..=0x1734
        | 0x1752..=0x1753
        | 0x1772..=0x1773
        | 0x17B4..=0x17B5
        | 0x17B7..=0x17BD
        | 0x17C6
        | 0x17C9..=0x17D3
        | 0x17DD
        | 0x180B..=0x180D
        | 0x1885..=0x1886
        | 0x18A9
        | 0x1920..=0x1922
        | 0x1927..=0x1928
        | 0x1932
        | 0x1939..=0x193B
        | 0x1A17..=0x1A18
        | 0x1A1B
        | 0x1A56
        | 0x1A58..=0x1A5E
        | 0x1A60
        | 0x1A62
        | 0x1A65..=0x1A6C
        | 0x1A73..=0x1A7C
        | 0x1A7F
        | 0x1AB0..=0x1ABD
        | 0x1B00..=0x1B03
        | 0x1B34
        | 0x1B36..=0x1B3A
        | 0x1B3C
        | 0x1B42
        | 0x1B6B..=0x1B73
        | 0x1B80..=0x1B81
        | 0x1BA2..=0x1BA5
        | 0x1BA8..=0x1BA9
        | 0x1BAB..=0x1BAD
        | 0x1BE6
        | 0x1BE8..=0x1BE9
        | 0x1BED
        | 0x1BEF..=0x1BF1
        | 0x1C2C..=0x1C33
        | 0x1C36..=0x1C37
        | 0x1CD0..=0x1CD2
        | 0x1CD4..=0x1CE0
        | 0x1CE2..=0x1CE8
        | 0x1CED
        | 0x1CF4
        | 0x1CF8..=0x1CF9
        | 0x1DC0..=0x1DFF
        | 0x200C
        | 0x20D0..=0x20F0
        | 0x2CEF..=0x2CF1
        | 0x2D7F
        | 0x2DE0..=0x2DFF
        | 0x302A..=0x302D
        | 0x3099..=0x309A
        | 0xA66F..=0xA672
        | 0xA674..=0xA67D
        | 0xA69E..=0xA69F
        | 0xA6F0..=0xA6F1
        | 0xA802
        | 0xA806
        | 0xA80B
        | 0xA825..=0xA826
        | 0xA8C4..=0xA8C5
        | 0xA8E0..=0xA8F1
        | 0xA8FF
        | 0xA926..=0xA92D
        | 0xA947..=0xA951
        | 0xA980..=0xA982
        | 0xA9B3
        | 0xA9B6..=0xA9B9
        | 0xA9BC..=0xA9BD
        | 0xA9E5
        | 0xAA29..=0xAA2E
        | 0xAA31..=0xAA32
        | 0xAA35..=0xAA36
        | 0xAA43
        | 0xAA4C
        | 0xAA7C
        | 0xAAB0
        | 0xAAB2..=0xAAB4
        | 0xAAB7..=0xAAB8
        | 0xAABE..=0xAABF
        | 0xAAC1
        | 0xAAEC..=0xAAED
        | 0xAAF6
        | 0xABE5
        | 0xABE8
        | 0xABED
        | 0xFB1E
        | 0xFE00..=0xFE0F
        | 0xFE20..=0xFE2F
        | 0xFF9E..=0xFF9F
        | 0x101FD
        | 0x102E0
        | 0x10376..=0x1037A
        | 0x10A01..=0x10A03
        | 0x10A05..=0x10A06
        | 0x10A0C..=0x10A0F
        | 0x10A38..=0x10A3A
        | 0x10A3F
        | 0x10AE5..=0x10AE6
        | 0x11001
        | 0x11038..=0x11046
        | 0x1107F..=0x11081
        | 0x110B3..=0x110B6
        | 0x110B9..=0x110BA
        | 0x11100..=0x11102
        | 0x11127..=0x1112B
        | 0x1112D..=0x11134
        | 0x11173
        | 0x11180..=0x11181
        | 0x111B6..=0x111BE
        | 0x111C9..=0x111CC
        | 0x1122F..=0x11231
        | 0x11234
        | 0x11236..=0x11237
        | 0x1123E
        | 0x112DF
        | 0x112E3..=0x112EA
        | 0x11300..=0x11301
        | 0x1133C
        | 0x11340
        | 0x11366..=0x1136C
        | 0x11370..=0x11374
        | 0x11438..=0x1143F
        | 0x11442..=0x11444
        | 0x11446
        | 0x1145E
        | 0x114B3..=0x114B8
        | 0x114BA
        | 0x114BF..=0x114C0
        | 0x114C2..=0x114C3
        | 0x115B2..=0x115B5
        | 0x115BC..=0x115BD
        | 0x115BF..=0x115C0
        | 0x115DC..=0x115DD
        | 0x11633..=0x1163A
        | 0x1163D
        | 0x1163F..=0x11640
        | 0x116AB
        | 0x116AD
        | 0x116B0..=0x116B5
        | 0x116B7
        | 0x1171D..=0x1171F
        | 0x11722..=0x11725
        | 0x11727..=0x1172B
        | 0x1182F..=0x11837
        | 0x11839..=0x1183A
        | 0x11A01..=0x11A0A
        | 0x11A33..=0x11A38
        | 0x11A3B..=0x11A3E
        | 0x11A47
        | 0x11A51..=0x11A56
        | 0x11A59..=0x11A5B
        | 0x11A8A..=0x11A96
        | 0x11A98..=0x11A99
        | 0x11C30..=0x11C36
        | 0x11C38..=0x11C3D
        | 0x11C3F
        | 0x11C92..=0x11CA7
        | 0x11CAA..=0x11CB0
        | 0x11CB2..=0x11CB3
        | 0x11CB5..=0x11CB6
        | 0x11D31..=0x11D36
        | 0x11D3A
        | 0x11D3C..=0x11D3D
        | 0x11D3F..=0x11D45
        | 0x11D47
        | 0x11D90..=0x11D91
        | 0x11D95
        | 0x11D97
        | 0x11EF3..=0x11EF4
        | 0xE0100..=0xE01EF
    )
}

fn is_gcb_prepend(cp: u32) -> bool {
    matches!(
        cp,
        0x0600..=0x0605
        | 0x06DD
        | 0x070F
        | 0x0890..=0x0891
        | 0x08E2
        | 0x0D4E
        | 0x110BD
        | 0x110CD
        | 0x111C2..=0x111C3
        | 0x1193F
        | 0x11941
        | 0x11A3A
        | 0x11A84..=0x11A89
    )
}

fn is_gcb_spacing_mark(cp: u32) -> bool {
    // A subset of Indic spacing combining marks. Kept minimal — GB9a
    // is a "× SpacingMark" only, and the algorithm falls through to
    // Other for anything not covered.
    matches!(
        cp,
        0x0903
        | 0x093B
        | 0x093E..=0x0940
        | 0x0949..=0x094C
        | 0x094E..=0x094F
        | 0x0982..=0x0983
        | 0x09BE..=0x09C0
        | 0x09C7..=0x09C8
        | 0x09CB..=0x09CC
        | 0x09D7
        | 0x0A03
        | 0x0A3E..=0x0A40
        | 0x0A83
        | 0x0ABE..=0x0AC0
        | 0x0AC9
        | 0x0ACB..=0x0ACC
        | 0x0B02..=0x0B03
        | 0x0B3E
        | 0x0B40
        | 0x0B47..=0x0B48
        | 0x0B4B..=0x0B4C
        | 0x0B57
        | 0x0BBE..=0x0BBF
        | 0x0BC1..=0x0BC2
        | 0x0BC6..=0x0BC8
        | 0x0BCA..=0x0BCC
        | 0x0BD7
        | 0x0C01..=0x0C03
        | 0x0C41..=0x0C44
        | 0x0C82..=0x0C83
        | 0x0CBE
        | 0x0CC0..=0x0CC4
        | 0x0CC7..=0x0CC8
        | 0x0CCA..=0x0CCB
        | 0x0CD5..=0x0CD6
        | 0x0D02..=0x0D03
        | 0x0D3E..=0x0D40
        | 0x0D46..=0x0D48
        | 0x0D4A..=0x0D4C
        | 0x0D57
        | 0x0D82..=0x0D83
        | 0x0DCF..=0x0DD1
        | 0x0DD8..=0x0DDF
        | 0x0DF2..=0x0DF3
        | 0x0F3E..=0x0F3F
        | 0x0F7F
    )
}

// -----------------------------------------------------------------------
// Extended_Pictographic (shared between GCB rules and WB rules)
// -----------------------------------------------------------------------

/// Broadly covers the emoji planes plus the pre-emoji-encoded
/// pictographics per UAX #29 § 3.
#[must_use]
pub fn is_extended_pictographic(cp: u32) -> bool {
    matches!(
        cp,
        0x00A9
        | 0x00AE
        | 0x203C
        | 0x2049
        | 0x2122
        | 0x2139
        | 0x2194..=0x2199
        | 0x21A9..=0x21AA
        | 0x231A..=0x231B
        | 0x2328
        | 0x2388
        | 0x23CF
        | 0x23E9..=0x23F3
        | 0x23F8..=0x23FA
        | 0x24C2
        | 0x25AA..=0x25AB
        | 0x25B6
        | 0x25C0
        | 0x25FB..=0x25FE
        | 0x2600..=0x2605
        | 0x2607..=0x2612
        | 0x2614..=0x2685
        | 0x2690..=0x2705
        | 0x2708..=0x2712
        | 0x2714
        | 0x2716
        | 0x271D
        | 0x2721
        | 0x2728
        | 0x2733..=0x2734
        | 0x2744
        | 0x2747
        | 0x274C
        | 0x274E
        | 0x2753..=0x2755
        | 0x2757
        | 0x2763..=0x2767
        | 0x2795..=0x2797
        | 0x27A1
        | 0x27B0
        | 0x27BF
        | 0x2934..=0x2935
        | 0x2B05..=0x2B07
        | 0x2B1B..=0x2B1C
        | 0x2B50
        | 0x2B55
        | 0x3030
        | 0x303D
        | 0x3297
        | 0x3299
        | 0x1F000..=0x1F0FF
        | 0x1F10D..=0x1F10F
        | 0x1F12F
        | 0x1F16C..=0x1F171
        | 0x1F17E..=0x1F17F
        | 0x1F18E
        | 0x1F191..=0x1F19A
        | 0x1F1AD..=0x1F1E5
        | 0x1F201..=0x1F20F
        | 0x1F21A
        | 0x1F22F
        | 0x1F232..=0x1F23A
        | 0x1F23C..=0x1F23F
        | 0x1F249..=0x1F3FA
        | 0x1F400..=0x1F53D
        | 0x1F546..=0x1F64F
        | 0x1F680..=0x1F6FF
        | 0x1F774..=0x1F77F
        | 0x1F7D5..=0x1F7FF
        | 0x1F80C..=0x1F80F
        | 0x1F848..=0x1F84F
        | 0x1F85A..=0x1F85F
        | 0x1F888..=0x1F88F
        | 0x1F8AE..=0x1F8FF
        | 0x1F90C..=0x1F93A
        | 0x1F93C..=0x1F945
        | 0x1F947..=0x1FAFF
        | 0x1FC00..=0x1FFFD
    )
}

// -----------------------------------------------------------------------
// Word_Break helpers
// -----------------------------------------------------------------------

fn is_wb_newline(cp: u32) -> bool {
    matches!(cp, 0x000B..=0x000C | 0x0085 | 0x2028..=0x2029)
}

fn is_katakana(cp: u32) -> bool {
    matches!(
        cp,
        0x3031..=0x3035
        | 0x309B..=0x309C
        | 0x30A0..=0x30FA
        | 0x30FC..=0x30FF
        | 0x31F0..=0x31FF
        | 0x32D0..=0x32FE
        | 0x3300..=0x3357
        | 0xFF66..=0xFF9D
        | 0x1B000
    )
}

fn is_hebrew_letter(cp: u32) -> bool {
    matches!(cp, 0x05D0..=0x05EA | 0x05EF..=0x05F2 | 0xFB1D | 0xFB1F..=0xFB28 | 0xFB2A..=0xFB4F)
}

fn is_wb_wseg_space(cp: u32) -> bool {
    // WSegSpace = Zs (space separators) minus a small carveout —
    // U+00A0 (no-break) and U+2007 (figure) are excluded (they act
    // as MidNum in numeric contexts, or Other). Phase 5 treats them
    // as Other for simplicity.
    matches!(
        cp,
        0x0020
        | 0x1680
        | 0x2000..=0x2006
        | 0x2008..=0x200A
        | 0x205F
        | 0x3000
    )
}

fn is_wb_extendnumlet(cp: u32) -> bool {
    matches!(
        cp,
        0x005F
        | 0x203F..=0x2040
        | 0x2054
        | 0xFE33..=0xFE34
        | 0xFE4D..=0xFE4F
        | 0xFF3F
    )
}

fn is_wb_midletter(cp: u32) -> bool {
    matches!(
        cp,
        0x003A | 0x00B7 | 0x0387 | 0x055F | 0x05F4 | 0x2027 | 0xFE13 | 0xFE55 | 0xFF1A
    )
}

fn is_wb_midnumlet(cp: u32) -> bool {
    matches!(
        cp,
        0x002E | 0x2018 | 0x2019 | 0x2024 | 0xFE52 | 0xFF07 | 0xFF0E
    )
}

fn is_wb_midnum(cp: u32) -> bool {
    matches!(
        cp,
        0x002C | 0x003B | 0x037E | 0x0589 | 0x060C
            ..=0x060D
                | 0x066C
                | 0x07F8
                | 0x2044
                | 0xFE10
                | 0xFE14
                | 0xFE50
                | 0xFE54
                | 0xFF0C
                | 0xFF1B
    )
}

fn is_wb_numeric(cp: u32) -> bool {
    matches!(
        cp,
        0x0030..=0x0039
        | 0x0660..=0x0669
        | 0x066B..=0x066C
        | 0x06F0..=0x06F9
        | 0x07C0..=0x07C9
        | 0x0966..=0x096F
        | 0x09E6..=0x09EF
        | 0x0A66..=0x0A6F
        | 0x0AE6..=0x0AEF
        | 0x0B66..=0x0B6F
        | 0x0BE6..=0x0BEF
        | 0x0C66..=0x0C6F
        | 0x0CE6..=0x0CEF
        | 0x0D66..=0x0D6F
        | 0x0DE6..=0x0DEF
        | 0x0E50..=0x0E59
        | 0x0ED0..=0x0ED9
        | 0x1040..=0x1049
        | 0x1090..=0x1099
    )
}

fn is_wb_format(cp: u32) -> bool {
    matches!(
        cp,
        0x00AD
        | 0x0600..=0x0605
        | 0x061C
        | 0x06DD
        | 0x070F
        | 0x180E
        | 0x200E..=0x200F
        | 0x202A..=0x202E
        | 0x2060..=0x2064
        | 0x2066..=0x206F
        | 0xFEFF
        | 0xFFF9..=0xFFFB
    )
}

fn is_wb_extend(cp: u32) -> bool {
    // Same Extend base as the GCB axis for words (per UAX #29 the
    // Word_Break Extend property is derived from Grapheme_Extend +
    // General_Category=Mc).
    is_gcb_extend(cp) || is_gcb_spacing_mark(cp)
}

fn is_wb_aletter(cp: u32) -> bool {
    // ALetter — Alphabetic minus Ideographic minus Katakana minus
    // Hebrew_Letter minus a handful of specific carve-outs. Phase 5
    // ships a broad approximation covering the Latin/Greek/Cyrillic
    // alphabets, IPA extensions, Latin extended, and a handful of
    // supplemental letter ranges. Ideographs classify as Other so
    // each CJK character stands alone as its own "word".
    if !is_letter_like(cp) {
        return false;
    }
    // Ideographs — Other, not ALetter.
    if is_ideographic(cp) {
        return false;
    }
    // Hiragana/Katakana handled separately.
    if is_katakana(cp) {
        return false;
    }
    if (0x3040..=0x309F).contains(&cp) {
        // Hiragana: WB Other so it clusters as one word per glyph.
        return false;
    }
    // Hebrew_Letter handled separately.
    if is_hebrew_letter(cp) {
        return false;
    }
    true
}

#[allow(clippy::too_many_lines)]
fn is_letter_like(cp: u32) -> bool {
    // A broad "is this a letter" classifier. Covers ASCII letters,
    // Latin-1 supplement letters, Latin Extended-A/B, IPA
    // extensions, Greek, Cyrillic, Armenian, Arabic, Devanagari,
    // and various supplemental blocks. Missing scalars fall through
    // to Other.
    matches!(
        cp,
        0x0041..=0x005A
        | 0x0061..=0x007A
        | 0x00AA
        | 0x00B5
        | 0x00BA
        | 0x00C0..=0x00D6
        | 0x00D8..=0x00F6
        | 0x00F8..=0x02AF
        | 0x02B0..=0x02C1
        | 0x02C6..=0x02D1
        | 0x02E0..=0x02E4
        | 0x02EC
        | 0x02EE
        | 0x0370..=0x0373
        | 0x0376..=0x0377
        | 0x037A..=0x037D
        | 0x037F
        | 0x0386
        | 0x0388..=0x038A
        | 0x038C
        | 0x038E..=0x03A1
        | 0x03A3..=0x03F5
        | 0x03F7..=0x0481
        | 0x048A..=0x0559
        | 0x0561..=0x0587
        | 0x05EF..=0x05F2
        | 0x0620..=0x064A
        | 0x066E..=0x066F
        | 0x0671..=0x06D3
        | 0x06D5
        | 0x06E5..=0x06E6
        | 0x06EE..=0x06EF
        | 0x06FA..=0x06FC
        | 0x06FF
        | 0x0710
        | 0x0712..=0x072F
        | 0x074D..=0x07A5
        | 0x07B1
        | 0x07CA..=0x07EA
        | 0x07F4..=0x07F5
        | 0x07FA
        | 0x0800..=0x0815
        | 0x081A
        | 0x0824
        | 0x0828
        | 0x0840..=0x0858
        | 0x0860..=0x086A
        | 0x0870..=0x0887
        | 0x0889..=0x088E
        | 0x08A0..=0x08C9
        | 0x0904..=0x0939
        | 0x093D
        | 0x0958..=0x0961
        | 0x0971..=0x0980
        | 0x0985..=0x09B9
        | 0x09BD
        | 0x09CE
        | 0x09DC..=0x09E1
        | 0x09F0..=0x09F1
        | 0x09FC
        | 0x0A05..=0x0A28
        | 0x0A2A..=0x0A30
        | 0x0A32..=0x0A33
        | 0x0A35..=0x0A36
        | 0x0A38..=0x0A39
        | 0x0A59..=0x0A5E
        | 0x0A72..=0x0A74
        | 0x0A85..=0x0AB9
        | 0x0ABD
        | 0x0AD0
        | 0x0AE0..=0x0AE1
        | 0x0AF9
        | 0x0B05..=0x0B39
        | 0x0B3D
        | 0x0B5C..=0x0B61
        | 0x0B71
        | 0x0B83
        | 0x0B85..=0x0BB9
        | 0x0BD0
        | 0x0C05..=0x0C39
        | 0x0C3D
        | 0x0C58..=0x0C61
        | 0x0C80
        | 0x0C85..=0x0CB9
        | 0x0CBD
        | 0x0CDE..=0x0CE1
        | 0x0CF1..=0x0CF2
        | 0x0D04..=0x0D3A
        | 0x0D3D
        | 0x0D4E
        | 0x0D54..=0x0D56
        | 0x0D5F..=0x0D61
        | 0x0D7A..=0x0D7F
        | 0x0D85..=0x0DC6
        | 0x0E01..=0x0E30
        | 0x0E32..=0x0E33
        | 0x0E40..=0x0E46
        | 0x0E81..=0x0EB0
        | 0x0EB2..=0x0EB3
        | 0x0EBD
        | 0x0EC0..=0x0EC4
        | 0x0EC6
        | 0x0EDC..=0x0EDF
        | 0x0F00
        | 0x0F40..=0x0F6C
        | 0x0F88..=0x0F8C
        | 0x1000..=0x102A
        | 0x103F
        | 0x1050..=0x1055
        | 0x105A..=0x105D
        | 0x1061
        | 0x1065..=0x1066
        | 0x106E..=0x1070
        | 0x1075..=0x1081
        | 0x108E
        | 0x10A0..=0x10C5
        | 0x10C7
        | 0x10CD
        | 0x10D0..=0x10FA
        | 0x10FC..=0x1248
        | 0x124A..=0x124D
        | 0x1250..=0x1256
        | 0x1258
        | 0x125A..=0x125D
        | 0x1260..=0x1288
        | 0x128A..=0x128D
        | 0x1290..=0x12B0
        | 0x12B2..=0x12B5
        | 0x12B8..=0x12BE
        | 0x12C0
        | 0x12C2..=0x12C5
        | 0x12C8..=0x12D6
        | 0x12D8..=0x1310
        | 0x1312..=0x1315
        | 0x1318..=0x135A
        | 0x1380..=0x138F
        | 0x13A0..=0x13F5
        | 0x13F8..=0x13FD
        | 0x1401..=0x166C
        | 0x166F..=0x167F
        | 0x1681..=0x169A
        | 0x16A0..=0x16EA
        | 0x16F1..=0x16F8
        | 0x1700..=0x1711
        | 0x171F..=0x1731
        | 0x1740..=0x1751
        | 0x1760..=0x1770
        | 0x1780..=0x17B3
        | 0x17D7
        | 0x17DC
        | 0x1820..=0x1878
        | 0x1880..=0x1884
        | 0x1887..=0x18A8
        | 0x18AA
        | 0x18B0..=0x18F5
        | 0x1900..=0x191E
        | 0x1A00..=0x1A16
        | 0x1A20..=0x1A54
        | 0x1B05..=0x1B33
        | 0x1B45..=0x1B4B
        | 0x1B83..=0x1BA0
        | 0x1BAE..=0x1BAF
        | 0x1BBA..=0x1BE5
        | 0x1C00..=0x1C23
        | 0x1C4D..=0x1C4F
        | 0x1C5A..=0x1C7D
        | 0x1C80..=0x1C88
        | 0x1C90..=0x1CBA
        | 0x1CBD..=0x1CBF
        | 0x1CE9..=0x1CEC
        | 0x1CEE..=0x1CF3
        | 0x1CF5..=0x1CF6
        | 0x1CFA
        | 0x1D00..=0x1DBF
        | 0x1E00..=0x1F15
        | 0x1F18..=0x1F1D
        | 0x1F20..=0x1F45
        | 0x1F48..=0x1F4D
        | 0x1F50..=0x1F57
        | 0x1F59
        | 0x1F5B
        | 0x1F5D
        | 0x1F5F..=0x1F7D
        | 0x1F80..=0x1FB4
        | 0x1FB6..=0x1FBC
        | 0x1FBE
        | 0x1FC2..=0x1FC4
        | 0x1FC6..=0x1FCC
        | 0x1FD0..=0x1FD3
        | 0x1FD6..=0x1FDB
        | 0x1FE0..=0x1FEC
        | 0x1FF2..=0x1FF4
        | 0x1FF6..=0x1FFC
        | 0x2071
        | 0x207F
        | 0x2090..=0x209C
        | 0x2102
        | 0x2107
        | 0x210A..=0x2113
        | 0x2115
        | 0x2119..=0x211D
        | 0x2124
        | 0x2126
        | 0x2128
        | 0x212A..=0x212D
        | 0x212F..=0x2139
        | 0x213C..=0x213F
        | 0x2145..=0x2149
        | 0x214E
        | 0x2183..=0x2184
        | 0x2C00..=0x2CE4
        | 0x2CEB..=0x2CEE
        | 0x2CF2..=0x2CF3
        | 0x2D00..=0x2D25
        | 0x2D27
        | 0x2D2D
        | 0x2D30..=0x2D67
        | 0x2D6F
        | 0x2D80..=0x2D96
        | 0x2DA0..=0x2DA6
        | 0x2DA8..=0x2DAE
        | 0x2DB0..=0x2DB6
        | 0x2DB8..=0x2DBE
        | 0x2DC0..=0x2DC6
        | 0x2DC8..=0x2DCE
        | 0x2DD0..=0x2DD6
        | 0x2DD8..=0x2DDE
        | 0xA640..=0xA66E
        | 0xA67F..=0xA69D
        | 0xA6A0..=0xA6E5
        | 0xA717..=0xA71F
        | 0xA722..=0xA788
        | 0xA78B..=0xA7CA
        | 0xA7D0..=0xA7D9
        | 0xA7F5..=0xA801
        | 0xA803..=0xA805
        | 0xA807..=0xA80A
        | 0xA80C..=0xA822
        | 0xA840..=0xA873
        | 0xA882..=0xA8B3
        | 0xA8F2..=0xA8F7
        | 0xA8FB
        | 0xA8FD..=0xA8FE
        | 0xA90A..=0xA925
        | 0xA930..=0xA946
        | 0xA960..=0xA97C
        | 0xA984..=0xA9B2
        | 0xAAE0..=0xAAEA
        | 0xAAF2..=0xAAF4
        | 0xAB01..=0xAB06
        | 0xAB09..=0xAB0E
        | 0xAB11..=0xAB16
        | 0xAB20..=0xAB26
        | 0xAB28..=0xAB2E
        | 0xAB30..=0xAB69
        | 0xFB00..=0xFB06
        | 0xFB13..=0xFB17
        | 0xFB50..=0xFBB1
        | 0xFBD3..=0xFD3D
        | 0xFD50..=0xFDFB
        | 0xFE70..=0xFEFC
        | 0xFF21..=0xFF3A
        | 0xFF41..=0xFF5A
        | 0xFFA0..=0xFFBE
        | 0xFFC2..=0xFFC7
        | 0xFFCA..=0xFFCF
        | 0xFFD2..=0xFFD7
        | 0xFFDA..=0xFFDC
    )
}

fn is_ideographic(cp: u32) -> bool {
    // CJK Unified Ideographs + extensions + compatibility.
    matches!(
        cp,
        0x3006
        | 0x3007
        | 0x3021..=0x3029
        | 0x3038..=0x303A
        | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF
        | 0xF900..=0xFAFF
        | 0x20000..=0x2FFFD
        | 0x30000..=0x3FFFD
    )
}

/// Broad "is this a CJK-script scalar" classifier used by the
/// dictionary-based word segmenter to isolate CJK runs.
///
/// Covers Han (CJK Unified Ideographs and extensions), Hiragana
/// (`0x3040..=0x309F`), Katakana (`0x30A0..=0x30FF`), half-width
/// katakana (`0xFF65..=0xFF9F`), and the CJK punctuation /
/// symbol / radicals blocks. The forward-maximum-match walker
/// collects contiguous CJK runs and consumes them against the
/// pack's dictionary; anything outside the CJK set falls through
/// to the UAX #29 default rules.
#[must_use]
pub fn is_cjk_scalar(cp: u32) -> bool {
    if is_ideographic(cp) {
        return true;
    }
    if is_katakana(cp) {
        return true;
    }
    // Hiragana + Katakana Phonetic Extensions + Katakana block.
    matches!(
        cp,
        0x3040..=0x309F
        | 0x30A0..=0x30FF
        | 0x31F0..=0x31FF
        | 0xFF65..=0xFF9F
        | 0x3005          // Ideographic iteration mark (々)
        | 0x3006          // Ideographic closing mark
        | 0x3007          // Ideographic number zero (〇)
    )
}

// -----------------------------------------------------------------------
// Sentence_Break helpers
// -----------------------------------------------------------------------

fn is_sb_sep(cp: u32) -> bool {
    matches!(cp, 0x0085 | 0x2028..=0x2029)
}

fn is_sb_sp(cp: u32) -> bool {
    // Zs whitespace plus tab. Line-tab and form-feed are Sp per
    // UAX #29 (they are separately handled in the grapheme/word
    // classifiers as Control/Newline but Sentence_Break groups them
    // under Sp).
    matches!(
        cp,
        0x0009
        | 0x000B..=0x000C
        | 0x0020
        | 0x00A0
        | 0x1680
        | 0x2000..=0x200A
        | 0x202F
        | 0x205F
        | 0x3000
    )
}

fn is_sb_aterm(cp: u32) -> bool {
    matches!(cp, 0x002E | 0x2024 | 0xFE52 | 0xFF0E)
}

fn is_sb_sterm(cp: u32) -> bool {
    matches!(
        cp,
        0x0021
        | 0x003F
        | 0x0589
        | 0x061F
        | 0x06D4
        | 0x0700..=0x0702
        | 0x07F9
        | 0x0837
        | 0x0839
        | 0x083D..=0x083E
        | 0x0964..=0x0965
        | 0x104A..=0x104B
        | 0x1362
        | 0x1367..=0x1368
        | 0x166E
        | 0x1735..=0x1736
        | 0x17D4..=0x17D5
        | 0x1803
        | 0x1809
        | 0x1944..=0x1945
        | 0x1AA8..=0x1AAB
        | 0x1B4E..=0x1B4F
        | 0x1B5A..=0x1B5B
        | 0x1B5E..=0x1B5F
        | 0x1C3B..=0x1C3C
        | 0x1C7E..=0x1C7F
        | 0x203C..=0x203D
        | 0x2047..=0x2049
        | 0x2E2E
        | 0x2E3C
        | 0x2E53..=0x2E54
        | 0x3002
        | 0xA4FF
        | 0xA60E..=0xA60F
        | 0xA6F3
        | 0xA6F7
        | 0xA876..=0xA877
        | 0xA8CE..=0xA8CF
        | 0xA92F
        | 0xA9C8..=0xA9C9
        | 0xAA5D..=0xAA5F
        | 0xAAF0..=0xAAF1
        | 0xABEB
        | 0xFE56..=0xFE57
        | 0xFF01
        | 0xFF1F
        | 0xFF61
    )
}

fn is_sb_close(cp: u32) -> bool {
    // Closing punctuation (Pe) + certain Pf/Ps that count as
    // "close" in the SB rules — parens/brackets/quotes.
    matches!(
        cp,
        0x0022
        | 0x0027..=0x0029
        | 0x005B
        | 0x005D
        | 0x0060
        | 0x007B
        | 0x007D
        | 0x00AB
        | 0x00BB
        | 0x2018..=0x201F
        | 0x2039..=0x203A
        | 0x2045
        | 0x2046
        | 0x207D..=0x207E
        | 0x208D..=0x208E
        | 0x2308..=0x230B
        | 0x2329..=0x232A
        | 0x275B..=0x2760
        | 0x2768..=0x2775
        | 0x27C5..=0x27C6
        | 0x27E6..=0x27EF
        | 0x2983..=0x2998
        | 0x29D8..=0x29DB
        | 0x29FC..=0x29FD
        | 0x2E00..=0x2E0D
        | 0x2E1C..=0x2E1D
        | 0x2E20..=0x2E29
        | 0x2E42
        | 0x3008..=0x3011
        | 0x3014..=0x301B
        | 0x301D..=0x301F
        | 0xFE17..=0xFE18
        | 0xFE35..=0xFE44
        | 0xFE47..=0xFE48
        | 0xFE59..=0xFE5E
        | 0xFF08..=0xFF09
        | 0xFF3B
        | 0xFF3D
        | 0xFF5B
        | 0xFF5D
        | 0xFF5F..=0xFF60
        | 0xFF62..=0xFF63
    )
}

fn is_sb_scontinue(cp: u32) -> bool {
    matches!(
        cp,
        0x002C..=0x002D
        | 0x003A
        | 0x055D
        | 0x060C..=0x060D
        | 0x07F8
        | 0x1802
        | 0x1808
        | 0x2013..=0x2014
        | 0x3001
        | 0xFE10..=0xFE11
        | 0xFE13
        | 0xFE31..=0xFE32
        | 0xFE50..=0xFE51
        | 0xFE55
        | 0xFE58
        | 0xFE63
        | 0xFF0C..=0xFF0D
        | 0xFF1A
        | 0xFF64
    )
}

fn is_sb_numeric(cp: u32) -> bool {
    is_wb_numeric(cp)
}

fn is_sb_upper(cp: u32) -> bool {
    // A subset of Lu — ASCII uppercase plus common European
    // uppercase. Missing scalars fall to OLetter.
    matches!(
        cp,
        0x0041..=0x005A
        | 0x00C0..=0x00D6
        | 0x00D8..=0x00DE
        | 0x0100
        | 0x0102
        | 0x0104
        | 0x0106
        | 0x0108
        | 0x010A
        | 0x010C
        | 0x010E
        | 0x0110
        | 0x0112
        | 0x0114
        | 0x0116
        | 0x0118
        | 0x011A
        | 0x011C
        | 0x011E
        | 0x0120
        | 0x0122
        | 0x0124
        | 0x0126
        | 0x0128
        | 0x012A
        | 0x012C
        | 0x012E
        | 0x0130
        | 0x0132
        | 0x0134
        | 0x0136
        | 0x0139
        | 0x013B
        | 0x013D
        | 0x013F
        | 0x0141
        | 0x0143
        | 0x0145
        | 0x0147
        | 0x014A
        | 0x014C
        | 0x014E
        | 0x0150
        | 0x0152
        | 0x0154
        | 0x0156
        | 0x0158
        | 0x015A
        | 0x015C
        | 0x015E
        | 0x0160
        | 0x0162
        | 0x0164
        | 0x0166
        | 0x0168
        | 0x016A
        | 0x016C
        | 0x016E
        | 0x0170
        | 0x0172
        | 0x0174
        | 0x0176
        | 0x0178..=0x0179
        | 0x017B
        | 0x017D
        | 0x0181..=0x0182
        | 0x0184
        | 0x0186..=0x0187
        | 0x0389..=0x038A
        | 0x038C
        | 0x038E..=0x038F
        | 0x0391..=0x03A1
        | 0x03A3..=0x03AB
        | 0x0400..=0x042F
        | 0x0531..=0x0556
        | 0x10A0..=0x10C5
        | 0xFF21..=0xFF3A
    )
}

#[allow(clippy::too_many_lines)]
fn is_sb_lower(cp: u32) -> bool {
    // Ll — ASCII + Latin-1 + common European. Broad approximation.
    matches!(
        cp,
        0x0061..=0x007A
        | 0x00AA
        | 0x00B5
        | 0x00BA
        | 0x00DF..=0x00F6
        | 0x00F8..=0x00FF
        | 0x0101
        | 0x0103
        | 0x0105
        | 0x0107
        | 0x0109
        | 0x010B
        | 0x010D
        | 0x010F
        | 0x0111
        | 0x0113
        | 0x0115
        | 0x0117
        | 0x0119
        | 0x011B
        | 0x011D
        | 0x011F
        | 0x0121
        | 0x0123
        | 0x0125
        | 0x0127
        | 0x0129
        | 0x012B
        | 0x012D
        | 0x012F
        | 0x0131
        | 0x0133
        | 0x0135
        | 0x0137..=0x0138
        | 0x013A
        | 0x013C
        | 0x013E
        | 0x0140
        | 0x0142
        | 0x0144
        | 0x0146
        | 0x0148..=0x0149
        | 0x014B
        | 0x014D
        | 0x014F
        | 0x0151
        | 0x0153
        | 0x0155
        | 0x0157
        | 0x0159
        | 0x015B
        | 0x015D
        | 0x015F
        | 0x0161
        | 0x0163
        | 0x0165
        | 0x0167
        | 0x0169
        | 0x016B
        | 0x016D
        | 0x016F
        | 0x0171
        | 0x0173
        | 0x0175
        | 0x0177
        | 0x017A
        | 0x017C
        | 0x017E..=0x0180
        | 0x0183
        | 0x0185
        | 0x0188
        | 0x018C..=0x018D
        | 0x0192
        | 0x0195
        | 0x0199..=0x019B
        | 0x019E
        | 0x01A1
        | 0x01A3
        | 0x01A5
        | 0x01A8
        | 0x01AA..=0x01AB
        | 0x01AD
        | 0x01B0
        | 0x01B4
        | 0x01B6
        | 0x01B9..=0x01BA
        | 0x01BD..=0x01BF
        | 0x01C6
        | 0x01C9
        | 0x01CC
        | 0x01CE
        | 0x01D0
        | 0x01D2
        | 0x01D4
        | 0x01D6
        | 0x01D8
        | 0x01DA
        | 0x01DC..=0x01DD
        | 0x01DF
        | 0x01E1
        | 0x01E3
        | 0x01E5
        | 0x01E7
        | 0x01E9
        | 0x01EB
        | 0x01ED
        | 0x01EF..=0x01F0
        | 0x01F3
        | 0x01F5
        | 0x01F9
        | 0x0430..=0x045F
        | 0x0561..=0x0587
        | 0x10D0..=0x10FA
        | 0xFF41..=0xFF5A
    )
}

fn is_sb_oletter(cp: u32) -> bool {
    // "Other letters" — Alphabetic scalars that are neither Upper
    // nor Lower. Ideographs, Katakana, Hiragana, Hebrew, Arabic,
    // Devanagari, etc. Approximate as "letter-like AND not upper
    // AND not lower".
    if is_ideographic(cp) {
        return true;
    }
    if is_katakana(cp) {
        return true;
    }
    if is_hebrew_letter(cp) {
        return true;
    }
    // Hiragana.
    if (0x3040..=0x309F).contains(&cp) {
        return true;
    }
    // Arabic letters + Devanagari + other scripts we cover.
    if is_letter_like(cp) && !is_sb_upper(cp) && !is_sb_lower(cp) {
        return true;
    }
    false
}

fn is_sb_format(cp: u32) -> bool {
    is_wb_format(cp)
}

fn is_sb_extend(cp: u32) -> bool {
    // SB Extend = Grapheme_Extend + a few carve-outs. Reuse the GCB
    // Extend classifier as a good-enough proxy.
    is_gcb_extend(cp) || cp == 0x200C || cp == 0x200D
}
