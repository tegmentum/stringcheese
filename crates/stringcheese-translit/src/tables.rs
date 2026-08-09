//! Built-in transliteration tables.
//!
//! The initial set is small — one Cyrillic → Latin table under
//! ISO 9 as a template. Additional per-script tables land as
//! follow-ups without touching the [`crate::Transliterator`]
//! trait.

use crate::TableTransliterator;

/// Cyrillic → Latin under [ISO 9:1995](https://en.wikipedia.org/wiki/ISO_9).
///
/// The strict romanization: every Cyrillic letter maps to
/// exactly one Latin letter (with diacritics). Bijective —
/// intended for round-tripping identifiers through ASCII-adjacent
/// channels; the paired inverse table (Latin → Cyrillic) can land
/// in a follow-up.
///
/// Covers the modern Russian / Ukrainian / Serbian / Bulgarian
/// letter sets. Church-Slavonic and historic-only letters are
/// omitted (they're not in ISO 9's basic table anyway).
#[must_use]
pub fn cyrillic_to_latin_iso9() -> TableTransliterator {
    TableTransliterator::new(CYRILLIC_ISO9)
}

/// The raw table backing [`cyrillic_to_latin_iso9`]. Public so
/// callers can extend it with locale-specific additions without
/// re-deriving the base.
pub const CYRILLIC_ISO9: &[(char, &str)] = &[
    // Basic Russian alphabet — uppercase.
    ('А', "A"),
    ('Б', "B"),
    ('В', "V"),
    ('Г', "G"),
    ('Д', "D"),
    ('Е', "E"),
    ('Ё', "Ë"),
    ('Ж', "Ž"),
    ('З', "Z"),
    ('И', "I"),
    ('Й', "J"),
    ('К', "K"),
    ('Л', "L"),
    ('М', "M"),
    ('Н', "N"),
    ('О', "O"),
    ('П', "P"),
    ('Р', "R"),
    ('С', "S"),
    ('Т', "T"),
    ('У', "U"),
    ('Ф', "F"),
    ('Х', "H"),
    ('Ц', "C"),
    ('Ч', "Č"),
    ('Ш', "Š"),
    ('Щ', "Ŝ"),
    ('Ъ', "ʺ"),
    ('Ы', "Y"),
    ('Ь', "ʹ"),
    ('Э', "È"),
    ('Ю', "Û"),
    ('Я', "Â"),
    // Lowercase mirrors.
    ('а', "a"),
    ('б', "b"),
    ('в', "v"),
    ('г', "g"),
    ('д', "d"),
    ('е', "e"),
    ('ё', "ë"),
    ('ж', "ž"),
    ('з', "z"),
    ('и', "i"),
    ('й', "j"),
    ('к', "k"),
    ('л', "l"),
    ('м', "m"),
    ('н', "n"),
    ('о', "o"),
    ('п', "p"),
    ('р', "r"),
    ('с', "s"),
    ('т', "t"),
    ('у', "u"),
    ('ф', "f"),
    ('х', "h"),
    ('ц', "c"),
    ('ч', "č"),
    ('ш', "š"),
    ('щ', "ŝ"),
    ('ъ', "ʺ"),
    ('ы', "y"),
    ('ь', "ʹ"),
    ('э', "è"),
    ('ю', "û"),
    ('я', "â"),
    // Ukrainian additions.
    ('Є', "Ê"),
    ('є', "ê"),
    ('І', "Ì"),
    ('і', "ì"),
    ('Ї', "Ï"),
    ('ї', "ï"),
    ('Ґ', "G̀"),
    ('ґ', "g̀"),
    // Serbian / Macedonian additions.
    ('Ђ', "Đ"),
    ('ђ', "đ"),
    ('Ј', "J"),
    ('ј', "j"),
    ('Љ', "L̂"),
    ('љ', "l̂"),
    ('Њ', "N̂"),
    ('њ', "n̂"),
    ('Ћ', "Ć"),
    ('ћ', "ć"),
    ('Ќ', "Ḱ"),
    ('ќ', "ḱ"),
    ('Ѓ', "Ǵ"),
    ('ѓ', "ǵ"),
    ('Ѕ', "Ẑ"),
    ('ѕ', "ẑ"),
    ('Џ', "D̂"),
    ('џ', "d̂"),
    // Belarusian addition.
    ('Ў', "Ŭ"),
    ('ў', "ŭ"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Transliterator;

    #[test]
    fn russian_word_transliterates() {
        let t = cyrillic_to_latin_iso9();
        assert_eq!(t.transliterate("Привет"), "Privet");
        assert_eq!(t.transliterate("мир"), "mir");
    }

    #[test]
    fn ukrainian_specific_letters() {
        let t = cyrillic_to_latin_iso9();
        // Ukrainian "її" — both letters are UA-specific.
        assert_eq!(t.transliterate("її"), "ïï");
    }

    #[test]
    fn preserves_non_cyrillic_scalars() {
        let t = cyrillic_to_latin_iso9();
        assert_eq!(t.transliterate("Hello, Привет!"), "Hello, Privet!");
    }

    #[test]
    fn empty_input_stays_empty() {
        assert_eq!(cyrillic_to_latin_iso9().transliterate(""), "");
    }
}
