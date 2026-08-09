//! # Tier-0 script classifier
//!
//! The lightest possible language-detection primitive: scan `text`,
//! count code points per Unicode script, return the dominant ISO 15924
//! code. Zero dependencies, no models, no trigrams — a byte-scan on
//! the input, done.
//!
//! ## Where this sits
//!
//! StringCheese's browser-oriented detection stack loads in tiers:
//!
//! - **Tier 0** — this crate. Always resident. Answers "what script?".
//!   ~5 KB after strip; one CDN request per session, then cached.
//! - **Tier 1** — `stringcheese-detect-whatlang` per-script components.
//!   Lazy-loaded once per script the user actually types in. Answers
//!   "which language within that script?" with cheap trigram scoring.
//! - **Tier 2** — `stringcheese-detect-lingua` per-language components.
//!   Lazy-loaded only when a caller demands high-confidence detection.
//!
//! Tier 0's job is to make Tier 1 lazy: without knowing the script
//! there's no way to pick the right per-script component to fetch.
//!
//! ## Semantics
//!
//! - Whitespace, digits, ASCII punctuation, and controls are ignored.
//! - Latin block runs (including Latin-1 Supplement and Latin Extended)
//!   collapse under `"Latn"`.
//! - Japanese is signalled by ANY presence of Hiragana / Katakana —
//!   even if the majority of the input is Han (Kanji) — because a
//!   mixed Han+Kana document is Japanese, not Chinese. Pure-Han input
//!   returns `"Hans"`; Simplified vs Traditional discrimination is
//!   left to Tier 1 / Tier 2 and not attempted here.
//! - Korean is signalled by ANY presence of Hangul syllables (with or
//!   without Han).
//! - Input with no classifiable code points returns `None`.
//!
//! ## Return codes
//!
//! ISO 15924 four-letter script codes. The set covered here maps 1:1
//! to the language packs StringCheese ships against; expanding it is
//! adding a range check.
//!
//! | Script       | Code   | Approximate Unicode range                     |
//! |--------------|--------|-----------------------------------------------|
//! | Latin        | `Latn` | U+0000..U+024F, U+1E00..U+1EFF                |
//! | Cyrillic     | `Cyrl` | U+0400..U+052F, U+2DE0..U+2DFF, U+A640..U+A69F |
//! | Greek        | `Grek` | U+0370..U+03FF, U+1F00..U+1FFF                |
//! | Arabic       | `Arab` | U+0600..U+06FF, U+0750..U+077F, U+FB50..U+FDFF |
//! | Hebrew       | `Hebr` | U+0590..U+05FF                                |
//! | Devanagari   | `Deva` | U+0900..U+097F                                |
//! | Bengali      | `Beng` | U+0980..U+09FF                                |
//! | Gurmukhi     | `Guru` | U+0A00..U+0A7F                                |
//! | Gujarati     | `Gujr` | U+0A80..U+0AFF                                |
//! | Tamil        | `Taml` | U+0B80..U+0BFF                                |
//! | Telugu       | `Telu` | U+0C00..U+0C7F                                |
//! | Kannada      | `Knda` | U+0C80..U+0CFF                                |
//! | Malayalam    | `Mlym` | U+0D00..U+0D7F                                |
//! | Sinhala      | `Sinh` | U+0D80..U+0DFF                                |
//! | Thai         | `Thai` | U+0E00..U+0E7F                                |
//! | Lao          | `Laoo` | U+0E80..U+0EFF                                |
//! | Myanmar      | `Mymr` | U+1000..U+109F                                |
//! | Georgian     | `Geor` | U+10A0..U+10FF, U+2D00..U+2D2F, U+1C90..U+1CBF |
//! | Ethiopic     | `Ethi` | U+1200..U+137F                                |
//! | Armenian     | `Armn` | U+0530..U+058F                                |
//! | Khmer        | `Khmr` | U+1780..U+17FF                                |
//! | Hangul       | `Hang` | U+1100..U+11FF (jamo) + U+AC00..U+D7A3 (syllables) |
//! | Japanese     | `Jpan` | Any of U+3040..U+30FF present                 |
//! | Han          | `Hans` | U+4E00..U+9FFF, U+3400..U+4DBF                |
//!
//! `Hant` (Traditional Chinese) is deliberately NOT distinguished at
//! this tier — the Simplified vs Traditional split needs a Tier-1
//! or Tier-2 classifier and can't be answered by block membership
//! alone.

// Note: this crate is written to be no_std-compatible in code but
// doesn't declare `#![no_std]` — the WIT-component build brings a
// panic handler through wit-bindgen-rt, and native builds need std
// for tests. The library body itself uses no allocations and no
// std-only APIs, so a downstream no_std consumer can pull it in
// via a small feature-gate flip once we ship one.
#![deny(unsafe_code)]

// WIT bindings + Guest wiring only compile on WASM targets with the
// `wit-component` feature — native builds and non-component wasm
// consumers (e.g. the `stringcheese-detect` umbrella linking this
// crate as an `rlib` sibling of the other detect backends) skip
// both, so `cargo test`, plain `rlib` consumers, and the ~5 KB
// always-resident story stay unaffected by the component-build
// machinery. The feature-gate matters on wasm: without it, three
// backends `export!`-ing the same interface into one binary would
// collide at link time.
#[cfg(all(target_family = "wasm", feature = "wit-component"))]
#[allow(
    unsafe_code,
    unsafe_op_in_unsafe_fn,
    missing_docs,
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::restriction
)]
mod bindings;
#[cfg(all(target_family = "wasm", feature = "wit-component"))]
#[allow(unsafe_code, unsafe_op_in_unsafe_fn)]
mod wit;

/// Classify `text` by its dominant Unicode script and return the ISO
/// 15924 four-letter code, or `None` when no code point falls into a
/// recognized script.
///
/// See the [module docs](self) for the covered set and the Japanese /
/// Korean disambiguation rules.
#[must_use]
pub fn detect_script(text: &str) -> Option<&'static str> {
    // Japanese-kana presence overrides everything else — even one
    // hiragana / katakana scalar in an otherwise-Han document flags
    // Japanese.
    let mut has_kana = false;
    // Hangul presence overrides Han for Korean detection.
    let mut has_hangul = false;

    // Score table sized to the recognized-script count. Indexed via
    // the internal [`Script`] enum's `as usize`.
    let mut scores = [0u32; SCRIPT_COUNT];

    for ch in text.chars() {
        // Skip common noise before script classification — whitespace,
        // ASCII controls, ASCII digits, ASCII punctuation. Every real
        // signal comes from the letters.
        if ch.is_ascii_whitespace()
            || ch.is_ascii_digit()
            || ch.is_ascii_punctuation()
            || ch.is_control()
        {
            continue;
        }
        if let Some(script) = classify(ch) {
            match script {
                Script::Hiragana | Script::Katakana => has_kana = true,
                Script::Hangul => has_hangul = true,
                _ => {}
            }
            scores[script as usize] += 1;
        }
    }

    // Precedence rules — Japanese wins over Han when kana present,
    // Korean wins over Han when hangul present.
    if has_kana {
        return Some("Jpan");
    }
    if has_hangul {
        return Some("Hang");
    }

    // Otherwise, return the ISO 15924 code of the winning bucket.
    // Written as an explicit fold rather than `.max_by_key().expect(...)`
    // so there's no theoretical panic site on a fixed-size array —
    // `[u32; SCRIPT_COUNT]` is always non-empty, but expressing that
    // to clippy without an expect() keeps `missing_panics_doc` happy.
    let mut best_idx = 0usize;
    let mut best_count = 0u32;
    for (i, &c) in scores.iter().enumerate() {
        if c > best_count {
            best_count = c;
            best_idx = i;
        }
    }
    if best_count == 0 {
        return None;
    }
    Some(Script::from_index(best_idx).iso_15924())
}

/// Internal script enumeration. Not exposed — callers get ISO 15924
/// strings which are the stable public identifier.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
enum Script {
    Latin,
    Cyrillic,
    Greek,
    Arabic,
    Hebrew,
    Devanagari,
    Bengali,
    Gurmukhi,
    Gujarati,
    Tamil,
    Telugu,
    Kannada,
    Malayalam,
    Sinhala,
    Thai,
    Lao,
    Myanmar,
    Georgian,
    Ethiopic,
    Armenian,
    Khmer,
    Hangul,
    Hiragana,
    Katakana,
    Han,
}

/// Number of variants — sized upper bound on the scores table.
const SCRIPT_COUNT: usize = 25;

impl Script {
    fn from_index(i: usize) -> Self {
        // The variant indices are stable via `#[repr(u8)]` + the
        // declaration order above. Cast-back is safe by construction
        // (only values 0..SCRIPT_COUNT are ever indexed).
        match i {
            0 => Self::Latin,
            1 => Self::Cyrillic,
            2 => Self::Greek,
            3 => Self::Arabic,
            4 => Self::Hebrew,
            5 => Self::Devanagari,
            6 => Self::Bengali,
            7 => Self::Gurmukhi,
            8 => Self::Gujarati,
            9 => Self::Tamil,
            10 => Self::Telugu,
            11 => Self::Kannada,
            12 => Self::Malayalam,
            13 => Self::Sinhala,
            14 => Self::Thai,
            15 => Self::Lao,
            16 => Self::Myanmar,
            17 => Self::Georgian,
            18 => Self::Ethiopic,
            19 => Self::Armenian,
            20 => Self::Khmer,
            21 => Self::Hangul,
            22 => Self::Hiragana,
            23 => Self::Katakana,
            24 => Self::Han,
            _ => unreachable!("index bounded by SCRIPT_COUNT"),
        }
    }

    /// The four-letter ISO 15924 script code every Script maps to.
    /// Hiragana and Katakana intentionally collapse to `"Jpan"` here
    /// because a document with either is treated as Japanese; the
    /// caller of the classifier applies that precedence rule before
    /// consulting this method.
    const fn iso_15924(self) -> &'static str {
        match self {
            Self::Latin => "Latn",
            Self::Cyrillic => "Cyrl",
            Self::Greek => "Grek",
            Self::Arabic => "Arab",
            Self::Hebrew => "Hebr",
            Self::Devanagari => "Deva",
            Self::Bengali => "Beng",
            Self::Gurmukhi => "Guru",
            Self::Gujarati => "Gujr",
            Self::Tamil => "Taml",
            Self::Telugu => "Telu",
            Self::Kannada => "Knda",
            Self::Malayalam => "Mlym",
            Self::Sinhala => "Sinh",
            Self::Thai => "Thai",
            Self::Lao => "Laoo",
            Self::Myanmar => "Mymr",
            Self::Georgian => "Geor",
            Self::Ethiopic => "Ethi",
            Self::Armenian => "Armn",
            Self::Khmer => "Khmr",
            Self::Hangul => "Hang",
            Self::Hiragana | Self::Katakana => "Jpan",
            Self::Han => "Hans",
        }
    }
}

/// Map a scalar to its (internal) script. Returns `None` for
/// scalars that don't fall in any recognized block — including all
/// symbols, dingbats, and private-use.
#[allow(clippy::too_many_lines)] // one contiguous range table; splitting hurts readability
fn classify(ch: char) -> Option<Script> {
    let c = ch as u32;
    // Fast path — anything in ASCII letters is Latin. The vast
    // majority of Latin-script input falls through here.
    if ch.is_ascii_alphabetic() {
        return Some(Script::Latin);
    }
    match c {
        // Latin extended blocks.
        0x0080..=0x024F | 0x1E00..=0x1EFF | 0x2C60..=0x2C7F | 0xA720..=0xA7FF => {
            Some(Script::Latin)
        }
        // Greek (basic + extended).
        0x0370..=0x03FF | 0x1F00..=0x1FFF => Some(Script::Greek),
        // Cyrillic (basic + supplements).
        0x0400..=0x052F | 0x2DE0..=0x2DFF | 0xA640..=0xA69F => Some(Script::Cyrillic),
        // Armenian.
        0x0530..=0x058F => Some(Script::Armenian),
        // Hebrew.
        0x0590..=0x05FF => Some(Script::Hebrew),
        // Arabic (basic + supplement + presentation forms).
        0x0600..=0x06FF | 0x0750..=0x077F | 0x0870..=0x089F | 0xFB50..=0xFDFF | 0xFE70..=0xFEFF => {
            Some(Script::Arabic)
        }
        // Devanagari.
        0x0900..=0x097F => Some(Script::Devanagari),
        // Bengali.
        0x0980..=0x09FF => Some(Script::Bengali),
        // Gurmukhi.
        0x0A00..=0x0A7F => Some(Script::Gurmukhi),
        // Gujarati.
        0x0A80..=0x0AFF => Some(Script::Gujarati),
        // Tamil.
        0x0B80..=0x0BFF => Some(Script::Tamil),
        // Telugu.
        0x0C00..=0x0C7F => Some(Script::Telugu),
        // Kannada.
        0x0C80..=0x0CFF => Some(Script::Kannada),
        // Malayalam.
        0x0D00..=0x0D7F => Some(Script::Malayalam),
        // Sinhala.
        0x0D80..=0x0DFF => Some(Script::Sinhala),
        // Thai.
        0x0E00..=0x0E7F => Some(Script::Thai),
        // Lao.
        0x0E80..=0x0EFF => Some(Script::Lao),
        // Myanmar.
        0x1000..=0x109F | 0xAA60..=0xAA7F => Some(Script::Myanmar),
        // Georgian (Mkhedruli, Asomtavruli, Nuskhuri, Mtavruli).
        0x10A0..=0x10FF | 0x2D00..=0x2D2F | 0x1C90..=0x1CBF => Some(Script::Georgian),
        // Ethiopic (Ge'ez).
        0x1200..=0x137F | 0x1380..=0x139F | 0x2D80..=0x2DDF => Some(Script::Ethiopic),
        // Khmer.
        0x1780..=0x17FF => Some(Script::Khmer),
        // Hangul (jamo + syllables + compatibility).
        0x1100..=0x11FF | 0x3130..=0x318F | 0xA960..=0xA97F | 0xD7B0..=0xD7FF | 0xAC00..=0xD7A3 => {
            Some(Script::Hangul)
        }
        // Hiragana.
        0x3040..=0x309F => Some(Script::Hiragana),
        // Katakana.
        0x30A0..=0x30FF | 0x31F0..=0x31FF => Some(Script::Katakana),
        // Han (CJK Unified Ideographs — basic + Extension A;
        // extensions B..I omitted for size but the same rule applies).
        0x3400..=0x4DBF | 0x4E00..=0x9FFF => Some(Script::Han),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_english_is_latin() {
        assert_eq!(detect_script("The quick brown fox"), Some("Latn"));
    }

    #[test]
    fn german_umlauts_still_latin() {
        assert_eq!(detect_script("über schön grüßen"), Some("Latn"));
    }

    #[test]
    fn russian_is_cyrillic() {
        assert_eq!(detect_script("Привет мир"), Some("Cyrl"));
    }

    #[test]
    fn greek_is_greek() {
        assert_eq!(detect_script("Καλημέρα κόσμε"), Some("Grek"));
    }

    #[test]
    fn arabic_is_arabic() {
        assert_eq!(detect_script("مرحبا بالعالم"), Some("Arab"));
    }

    #[test]
    fn hebrew_is_hebrew() {
        assert_eq!(detect_script("שלום עולם"), Some("Hebr"));
    }

    #[test]
    fn hindi_is_devanagari() {
        assert_eq!(detect_script("नमस्ते दुनिया"), Some("Deva"));
    }

    #[test]
    fn bengali_is_bengali() {
        assert_eq!(detect_script("ওহে বিশ্ব"), Some("Beng"));
    }

    #[test]
    fn punjabi_is_gurmukhi() {
        assert_eq!(detect_script("ਸਤਿ ਸ੍ਰੀ ਅਕਾਲ"), Some("Guru"));
    }

    #[test]
    fn tamil_is_tamil() {
        assert_eq!(detect_script("வணக்கம் உலகம்"), Some("Taml"));
    }

    #[test]
    fn thai_is_thai() {
        assert_eq!(detect_script("สวัสดี ชาวโลก"), Some("Thai"));
    }

    #[test]
    fn amharic_is_ethiopic() {
        assert_eq!(detect_script("ሰላም ዓለም"), Some("Ethi"));
    }

    #[test]
    fn georgian_is_georgian() {
        assert_eq!(detect_script("გამარჯობა მსოფლიო"), Some("Geor"));
    }

    #[test]
    fn armenian_is_armenian() {
        assert_eq!(detect_script("Բարեւ աշխարհ"), Some("Armn"));
    }

    #[test]
    fn korean_hangul_wins_over_han() {
        // Even mixed Hangul+Han text is Korean, not Chinese.
        assert_eq!(detect_script("한국어 中國"), Some("Hang"));
        assert_eq!(detect_script("안녕하세요"), Some("Hang"));
    }

    #[test]
    fn japanese_kana_wins_over_han() {
        // Kana presence signals Japanese regardless of the Han
        // majority in a real Japanese document.
        assert_eq!(detect_script("これは日本語のテストです"), Some("Jpan"));
        assert_eq!(detect_script("ひらがな"), Some("Jpan"));
    }

    #[test]
    fn pure_han_is_hans() {
        // No Simplified-vs-Traditional discrimination at Tier 0.
        assert_eq!(detect_script("你好世界"), Some("Hans"));
    }

    #[test]
    fn empty_input_returns_none() {
        assert_eq!(detect_script(""), None);
    }

    #[test]
    fn whitespace_and_digits_are_not_classifiable() {
        assert_eq!(detect_script("   "), None);
        assert_eq!(detect_script("12345 678"), None);
        assert_eq!(detect_script(",.!?;:"), None);
    }

    #[test]
    fn mixed_input_returns_dominant_script() {
        // Overwhelmingly Latin, with one Cyrillic letter — still Latin.
        assert_eq!(detect_script("Hello Мир"), Some("Latn"));
    }
}
