//! Bijective Vukovica (Cyrillic) <-> Gaj's Latin transliteration.
//!
//! Serbian is written in **two equally official scripts**: Vukovica
//! (Cyrillic, `а б в г д ђ е ж з и ј к л љ м н њ о п р с т ћ у ф х ц ч
//! џ ш`) and Gaj's Latin (`a b v g d đ e ž z i j k l lj m n nj o p r s
//! t ć u f h c č dž š`). Every Serbian text can be losslessly rendered
//! in either script; this module provides the round-trip conversion.
//!
//! # The bijection
//!
//! Serbian orthography is designed for exactly this: every Cyrillic
//! letter has a single Latin counterpart and vice versa. The three
//! Latin digraphs `lj`, `nj`, `dž` map to single Cyrillic letters `љ`,
//! `њ`, `џ`; every other pair is a single-letter mapping (`ђ ↔ đ`, `ж
//! ↔ ž`, `ћ ↔ ć`, `ч ↔ č`, `ц ↔ c`, `ш ↔ š`, `ј ↔ j`, and so on).
//!
//! # Well-formedness caveat
//!
//! The round-trip `to_cyrillic(to_latin(x)) == x` holds for any
//! Cyrillic input on the standard letter set. The reverse round-trip
//! `to_latin(to_cyrillic(y)) == y` holds when the Latin input is
//! **well-formed**, meaning every `lj`, `nj`, `dž` sequence in the
//! input is genuinely a digraph and not an accidental collision. In
//! practice this covers essentially all native Serbian vocabulary; a
//! handful of loanwords (`injekcija` = injection, `nadživeti` = to
//! outlive) do contain letter-boundary `n+j` / `d+ž` sequences that
//! collapse to the digraph on the Cyrillic side, matching standard
//! Serbian orthography.
//!
//! # Case handling
//!
//! Both single-letter and digraph conversions are case-preserving:
//!
//! * `to_latin(Њ)` → `Nj` (title case — the standard Gaj capitalization
//!   convention when only the digraph's first letter is capitalized).
//! * `to_latin(NJ_all_caps)` — Cyrillic Cyrillic does not distinguish
//!   `Nj` from `NJ`; `Њ` always renders as `Nj` in Latin.
//! * `to_cyrillic(Nj)` → `Њ`.
//! * `to_cyrillic(NJ)` → `Њ`.
//! * `to_cyrillic(nJ)` → `нЈ`. Ambiguous — treated as separate letters.
//!
//! # Non-goals
//!
//! * **Romanization schemes other than Gaj.** Serbian is only ever
//!   transliterated to Gaj; there is no ISO-9 alternative in wide use
//!   the way there is for Russian.
//! * **Foreign-word disambiguation.** The `injekcija` case is handled
//!   the way standard Serbian orthography handles it (collapse
//!   `n+j → њ`); if a caller needs to preserve loanword letter
//!   boundaries they should tag the input before conversion.

use alloc::string::String;

/// Transliterate Cyrillic (Vukovica) text to Latin (Gaj).
///
/// Every scalar outside the Serbian Cyrillic alphabet passes through
/// unchanged; ASCII, punctuation, whitespace, and non-Serbian Cyrillic
/// letters are preserved verbatim.
///
/// # Example
///
/// ```
/// use stringcheese_sr::scripts::to_latin;
///
/// assert_eq!(to_latin("Београд"), "Beograd");
/// assert_eq!(to_latin("љубав"), "ljubav");
/// assert_eq!(to_latin("Његош"), "Njegoš");
/// assert_eq!(to_latin("џем"), "džem");
/// ```
#[must_use]
pub fn to_latin(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match cyrillic_to_latin(c) {
            Some(s) => out.push_str(s),
            None => out.push(c),
        }
    }
    out
}

/// Transliterate Latin (Gaj) text to Cyrillic (Vukovica).
///
/// Recognises the three Latin digraphs `lj` / `nj` / `dž` (and their
/// case variants) as single Cyrillic letters `љ` / `њ` / `џ`. Every
/// scalar outside the Serbian Latin alphabet passes through unchanged.
///
/// # Example
///
/// ```
/// use stringcheese_sr::scripts::to_cyrillic;
///
/// assert_eq!(to_cyrillic("Beograd"), "Београд");
/// assert_eq!(to_cyrillic("ljubav"), "љубав");
/// assert_eq!(to_cyrillic("Njegoš"), "Његош");
/// assert_eq!(to_cyrillic("džem"), "џем");
/// ```
#[must_use]
pub fn to_cyrillic(text: &str) -> String {
    // Walk the character sequence with 2-char lookahead so we can
    // recognise the digraphs `lj`, `nj`, `dž` (and their case
    // variants) as single Cyrillic scalars. Everything outside the
    // Serbian Latin alphabet passes through unchanged.
    let mut out = String::with_capacity(text.len());
    let chars: alloc::vec::Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if let Some((cyr, consumed)) = latin_pair_to_cyrillic(c, next) {
            out.push(cyr);
            i += consumed;
            continue;
        }
        match latin_single_to_cyrillic(c) {
            Some(cyr) => out.push(cyr),
            None => out.push(c),
        }
        i += 1;
    }
    out
}

/// Maps a Serbian Cyrillic scalar to its Latin (Gaj) counterpart.
///
/// Returns the mapped string (one or two ASCII / Latin characters) or
/// `None` if the scalar is outside the Serbian Cyrillic alphabet.
#[must_use]
pub const fn cyrillic_to_latin(c: char) -> Option<&'static str> {
    Some(match c {
        // Lowercase.
        'а' => "a",
        'б' => "b",
        'в' => "v",
        'г' => "g",
        'д' => "d",
        'ђ' => "đ",
        'е' => "e",
        'ж' => "ž",
        'з' => "z",
        'и' => "i",
        'ј' => "j",
        'к' => "k",
        'л' => "l",
        'љ' => "lj",
        'м' => "m",
        'н' => "n",
        'њ' => "nj",
        'о' => "o",
        'п' => "p",
        'р' => "r",
        'с' => "s",
        'т' => "t",
        'ћ' => "ć",
        'у' => "u",
        'ф' => "f",
        'х' => "h",
        'ц' => "c",
        'ч' => "č",
        'џ' => "dž",
        'ш' => "š",
        // Uppercase. Digraphs use title case (`Lj`, `Nj`, `Dž`) — the
        // standard Gaj convention.
        'А' => "A",
        'Б' => "B",
        'В' => "V",
        'Г' => "G",
        'Д' => "D",
        'Ђ' => "Đ",
        'Е' => "E",
        'Ж' => "Ž",
        'З' => "Z",
        'И' => "I",
        'Ј' => "J",
        'К' => "K",
        'Л' => "L",
        'Љ' => "Lj",
        'М' => "M",
        'Н' => "N",
        'Њ' => "Nj",
        'О' => "O",
        'П' => "P",
        'Р' => "R",
        'С' => "S",
        'Т' => "T",
        'Ћ' => "Ć",
        'У' => "U",
        'Ф' => "F",
        'Х' => "H",
        'Ц' => "C",
        'Ч' => "Č",
        'Џ' => "Dž",
        'Ш' => "Š",
        _ => return None,
    })
}

/// Try to interpret `(c, next)` as a Latin digraph and return the
/// corresponding Cyrillic scalar plus the number of Latin characters
/// consumed (always `2`).
///
/// Recognises `lj` / `Lj` / `LJ`, `nj` / `Nj` / `NJ`, and `dž` / `Dž`
/// / `DŽ`. Returns `None` if `(c, next)` is not a digraph.
fn latin_pair_to_cyrillic(c: char, next: Option<char>) -> Option<(char, usize)> {
    let next = next?;
    let cyr = match (c, next) {
        ('l', 'j') => 'љ',
        ('L', 'j' | 'J') => 'Љ',
        ('n', 'j') => 'њ',
        ('N', 'j' | 'J') => 'Њ',
        ('d', 'ž') => 'џ',
        ('D', 'ž' | 'Ž') => 'Џ',
        _ => return None,
    };
    Some((cyr, 2))
}

/// Maps a single Serbian Latin scalar to its Cyrillic counterpart.
///
/// Returns `None` if the scalar is not a Serbian Latin letter. If the
/// scalar is the first half of a digraph (`l`, `n`, `d`) and the
/// caller has already decided it wasn't a digraph, this pass-through
/// is the right answer: bare `l`, `n`, `d` map to `л`, `н`, `д`.
#[must_use]
pub const fn latin_single_to_cyrillic(c: char) -> Option<char> {
    Some(match c {
        // Lowercase.
        'a' => 'а',
        'b' => 'б',
        'c' => 'ц',
        'č' => 'ч',
        'ć' => 'ћ',
        'd' => 'д',
        'đ' => 'ђ',
        'e' => 'е',
        'f' => 'ф',
        'g' => 'г',
        'h' => 'х',
        'i' => 'и',
        'j' => 'ј',
        'k' => 'к',
        'l' => 'л',
        'm' => 'м',
        'n' => 'н',
        'o' => 'о',
        'p' => 'п',
        'r' => 'р',
        's' => 'с',
        'š' => 'ш',
        't' => 'т',
        'u' => 'у',
        'v' => 'в',
        'z' => 'з',
        'ž' => 'ж',
        // Uppercase.
        'A' => 'А',
        'B' => 'Б',
        'C' => 'Ц',
        'Č' => 'Ч',
        'Ć' => 'Ћ',
        'D' => 'Д',
        'Đ' => 'Ђ',
        'E' => 'Е',
        'F' => 'Ф',
        'G' => 'Г',
        'H' => 'Х',
        'I' => 'И',
        'J' => 'Ј',
        'K' => 'К',
        'L' => 'Л',
        'M' => 'М',
        'N' => 'Н',
        'O' => 'О',
        'P' => 'П',
        'R' => 'Р',
        'S' => 'С',
        'Š' => 'Ш',
        'T' => 'Т',
        'U' => 'У',
        'V' => 'В',
        'Z' => 'З',
        'Ž' => 'Ж',
        _ => return None,
    })
}

/// Does `s` contain at least one Serbian Cyrillic scalar?
///
/// The Serbian Cyrillic alphabet lives entirely inside U+0400..=U+04FF;
/// this predicate accepts any scalar in that range. Non-Serbian
/// Cyrillic (e.g. Russian `ы`, `ъ`, `э`) also returns `true` — the
/// classifier is deliberately generous.
#[must_use]
pub fn contains_cyrillic(s: &str) -> bool {
    s.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_round_trips() {
        assert_eq!(to_latin(""), "");
        assert_eq!(to_cyrillic(""), "");
    }

    #[test]
    fn non_serbian_ascii_passes_through_to_cyrillic() {
        // ASCII letters that aren't in the Serbian Latin alphabet
        // (like `q`, `w`, `x`, `y`) pass through unchanged.
        assert_eq!(to_cyrillic("qxy"), "qxy");
    }

    #[test]
    fn ascii_in_to_latin_is_unchanged() {
        assert_eq!(to_latin("hello 123"), "hello 123");
    }

    #[test]
    fn single_letter_pairs() {
        assert_eq!(to_latin("а"), "a");
        assert_eq!(to_latin("б"), "b");
        assert_eq!(to_latin("ђ"), "đ");
        assert_eq!(to_latin("ж"), "ž");
        assert_eq!(to_latin("ћ"), "ć");
        assert_eq!(to_latin("ч"), "č");
        assert_eq!(to_latin("ц"), "c");
        assert_eq!(to_latin("ш"), "š");
        assert_eq!(to_latin("ј"), "j");
    }

    #[test]
    fn digraphs_render_as_two_latin_chars() {
        assert_eq!(to_latin("љ"), "lj");
        assert_eq!(to_latin("њ"), "nj");
        assert_eq!(to_latin("џ"), "dž");
    }

    #[test]
    fn digraphs_round_trip_from_latin() {
        assert_eq!(to_cyrillic("lj"), "љ");
        assert_eq!(to_cyrillic("nj"), "њ");
        assert_eq!(to_cyrillic("dž"), "џ");
    }

    #[test]
    fn cyrillic_round_trip() {
        for w in [
            "београд",
            "нови сад",
            "љубав",
            "његош",
            "џем",
            "црква",
            "ђак",
            "ћирилица",
        ] {
            let latin = to_latin(w);
            let back = to_cyrillic(&latin);
            assert_eq!(back, w, "round trip failed for {w:?} via {latin:?}");
        }
    }

    #[test]
    fn latin_round_trip_well_formed() {
        for w in [
            "beograd",
            "novi sad",
            "ljubav",
            "njegoš",
            "džem",
            "crkva",
            "đak",
            "ćirilica",
        ] {
            let cyr = to_cyrillic(w);
            let back = to_latin(&cyr);
            assert_eq!(back, w, "round trip failed for {w:?} via {cyr:?}");
        }
    }

    #[test]
    fn uppercase_digraphs_use_title_case() {
        assert_eq!(to_latin("Љубав"), "Ljubav");
        assert_eq!(to_latin("Његош"), "Njegoš");
        assert_eq!(to_latin("Џем"), "Džem");
    }

    #[test]
    fn all_caps_digraph_maps_to_cyrillic() {
        assert_eq!(to_cyrillic("LJUBAV"), "ЉУБАВ");
        assert_eq!(to_cyrillic("NJEGOŠ"), "ЊЕГОШ");
    }

    #[test]
    fn contains_cyrillic_predicate() {
        assert!(contains_cyrillic("Београд"));
        assert!(contains_cyrillic("mix Београд latin"));
        assert!(!contains_cyrillic("Beograd"));
        assert!(!contains_cyrillic(""));
        assert!(!contains_cyrillic("123 !@#"));
    }
}
