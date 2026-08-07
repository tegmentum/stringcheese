//! Turkish (Turkic) case folding.
//!
//! Turkish has a special orthographic distinction that default Unicode
//! case folding gets wrong:
//!
//! - Uppercase `I` (U+0049) is the *upper form of dotless* `ı`
//!   (U+0131), NOT of `i` (U+0069). Default Unicode folding lowers
//!   `I → i`; Turkish folding lowers `I → ı`.
//! - Uppercase `İ` (U+0130) is the *upper form of dotted* `i`
//!   (U+0069). Default Unicode folding maps `İ → i + U+0307` (combining
//!   dot above); Turkish folding maps `İ → i`.
//!
//! Getting these wrong quietly breaks stopword membership, stemming,
//! and case-insensitive comparison for any Turkish-locale input. This
//! module is the pack's internal reference for Turkish-aware
//! lowercasing.
//!
//! # Relationship to `stringcheese-unicode`
//!
//! The `stringcheese-unicode` crate exposes a
//! [`case_fold_turkic`](https://docs.rs/stringcheese-unicode) function
//! backed by ICU's authoritative tables. That crate is a **heavier**
//! dependency (it pulls in `icu_casemap` and its transitive ICU4X
//! data crates), and no other language pack in this workspace
//! currently depends on it — pulling it in for one function would
//! roughly double the wasm-size footprint of `stringcheese-tr`. The
//! Turkish dotted-I distinction is a single-scalar rewrite; a small
//! in-crate helper is sufficient and keeps the pack's dependency
//! surface minimal.
//!
//! # Scope
//!
//! This helper covers only the dotted/dotless-I distinction — the one
//! case-fold rule that is materially different under the Turkish
//! locale. Every other character is folded with the default Rust
//! [`char::to_lowercase`] step. Callers that need the full ICU
//! Turkic-fold semantics (including edge cases around combining
//! sequences) should reach for `stringcheese-unicode::case_fold_turkic`
//! directly.

use alloc::string::String;

/// Fold a single character to its Turkish lowercase form.
///
/// # Rules
///
/// - `'I'` (U+0049) → `'ı'` (U+0131) — Turkish dotless `i`.
/// - `'İ'` (U+0130) → `'i'` (U+0069) — Turkish dotted `i`.
/// - Every other character is left to Rust's default
///   [`char::to_lowercase`]; the *first* scalar of the (possibly
///   multi-scalar) lowercase mapping is returned. This is a loss when
///   default folding is multi-scalar (e.g., `ß → ss`), but Turkish
///   text does not carry the multi-scalar mappings that matter to it,
///   and preserving a `char → char` interface keeps the stemmer's
///   character-array pipeline simple.
#[inline]
#[must_use]
pub fn to_turkish_lower_char(c: char) -> char {
    match c {
        'I' => 'ı',
        'İ' => 'i',
        other => other.to_lowercase().next().unwrap_or(other),
    }
}

/// Fold `input` to its Turkish lowercase form.
///
/// Applies [`to_turkish_lower_char`] to each scalar.
///
/// # Examples
///
/// ```
/// use stringcheese_tr::case_fold::to_turkish_lower;
///
/// assert_eq!(to_turkish_lower("İstanbul"), "istanbul");
/// assert_eq!(to_turkish_lower("IŞIK"), "ışık");
/// assert_eq!(to_turkish_lower("HELLO"), "hello");
/// ```
#[must_use]
pub fn to_turkish_lower(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        out.push(to_turkish_lower_char(c));
    }
    out
}

/// Case-insensitive equality under Turkish case-fold rules.
///
/// Equivalent to `to_turkish_lower(a) == to_turkish_lower(b)` but
/// avoids the intermediate allocations by comparing scalar-by-scalar.
///
/// # Examples
///
/// ```
/// use stringcheese_tr::case_fold::eq_ignore_turkish_case;
///
/// assert!(eq_ignore_turkish_case("İSTANBUL", "istanbul"));
/// assert!(eq_ignore_turkish_case("IŞIK", "ışık"));
/// // Under Turkish rules, `I` is NOT equivalent to `i` (they lower
/// // to different forms — `ı` and `i`).
/// assert!(!eq_ignore_turkish_case("I", "i"));
/// ```
#[must_use]
pub fn eq_ignore_turkish_case(a: &str, b: &str) -> bool {
    let mut ac = a.chars();
    let mut bc = b.chars();
    loop {
        match (ac.next(), bc.next()) {
            (None, None) => return true,
            (Some(x), Some(y)) => {
                if to_turkish_lower_char(x) != to_turkish_lower_char(y) {
                    return false;
                }
            }
            _ => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dotless_capital_i_lowers_to_dotless_i() {
        assert_eq!(to_turkish_lower_char('I'), 'ı');
    }

    #[test]
    fn dotted_capital_i_lowers_to_dotted_i() {
        assert_eq!(to_turkish_lower_char('İ'), 'i');
    }

    #[test]
    fn ascii_letters_lower_normally() {
        assert_eq!(to_turkish_lower_char('A'), 'a');
        assert_eq!(to_turkish_lower_char('Z'), 'z');
    }

    #[test]
    fn already_lowercase_is_identity() {
        assert_eq!(to_turkish_lower_char('i'), 'i');
        assert_eq!(to_turkish_lower_char('ı'), 'ı');
        assert_eq!(to_turkish_lower_char('a'), 'a');
    }

    #[test]
    fn turkish_special_letters_lower_correctly() {
        // These are non-ASCII but their default Unicode lowercase is
        // already the desired Turkish lowercase.
        assert_eq!(to_turkish_lower_char('Ç'), 'ç');
        assert_eq!(to_turkish_lower_char('Ğ'), 'ğ');
        assert_eq!(to_turkish_lower_char('Ö'), 'ö');
        assert_eq!(to_turkish_lower_char('Ş'), 'ş');
        assert_eq!(to_turkish_lower_char('Ü'), 'ü');
    }

    #[test]
    fn string_level_fold() {
        assert_eq!(to_turkish_lower("İstanbul"), "istanbul");
        assert_eq!(to_turkish_lower("IŞIK"), "ışık");
        assert_eq!(to_turkish_lower("hello"), "hello");
        assert_eq!(to_turkish_lower(""), "");
    }

    #[test]
    fn fold_is_idempotent() {
        // Folding twice equals folding once — the crux of the case-fold
        // property tests in `properties.rs`.
        for s in [
            "İstanbul",
            "IŞIK",
            "HELLO",
            "ışık",
            "istanbul",
            "Öğrenci",
            "ÜÇÜNCÜ",
        ] {
            let once = to_turkish_lower(s);
            let twice = to_turkish_lower(&once);
            assert_eq!(once, twice, "not idempotent on {s:?}");
        }
    }

    #[test]
    fn eq_ignore_turkish_case_handles_dotted_pair() {
        // İ vs i — equal under Turkish fold.
        assert!(eq_ignore_turkish_case("İ", "i"));
        // I vs ı — equal under Turkish fold.
        assert!(eq_ignore_turkish_case("I", "ı"));
        // I vs i — NOT equal under Turkish fold (they differ).
        assert!(!eq_ignore_turkish_case("I", "i"));
        // İ vs ı — NOT equal.
        assert!(!eq_ignore_turkish_case("İ", "ı"));
    }

    #[test]
    fn eq_ignore_turkish_case_handles_mixed_content() {
        assert!(eq_ignore_turkish_case("İSTANBUL", "istanbul"));
        assert!(!eq_ignore_turkish_case("Istanbul", "istanbul"));
        assert!(!eq_ignore_turkish_case("hello", "world"));
    }
}
