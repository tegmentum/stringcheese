//! Case transformations for display.
//!
//! This module handles *case mapping* — the operation that changes a
//! string's presentation to lower- or upper-case for display. It is the
//! companion to `stringcheese-unicode`'s `case_fold` family, which
//! handles *case folding* for case-insensitive comparison. The two are
//! related but distinct; see the `stringcheese_unicode::case_folding`
//! module documentation for the differences (Turkish dotless I,
//! ß → ss, and Greek final sigma all diverge between them). If you are
//! folding strings so they can be compared, reach for
//! `stringcheese_unicode`, not this module.
//!
//! (The intra-doc links to `case_fold` / `case_folding` are avoided
//! here so this module documents cleanly whether or not the caller
//! turned on `stringcheese-unicode`'s optional `case-fold` feature.)
//!
//! # What this module offers
//!
//! - **[`to_lowercase`], [`to_uppercase`]** — full Unicode case mapping,
//!   delegating to [`str::to_lowercase`] / [`str::to_uppercase`]. The
//!   output length may differ from the input (`ß.to_uppercase() == "SS"`,
//!   `İ.to_lowercase() == "i\u{0307}"`).
//! - **[`to_title_case`]** — capitalize the first letter of each word,
//!   lowercase the rest. Word boundaries are identified via
//!   [`stringcheese_unicode`]'s grapheme iteration combined with the
//!   Unicode `Alphabetic` property: a new word begins at each grapheme
//!   whose first scalar is alphabetic and whose predecessor was not.
//! - **[`capitalize`]** — uppercase only the first character; leave the
//!   remainder unchanged.
//! - **`_into` variants** — append the transformed output to a
//!   caller-owned `String` buffer. Useful in tight loops where a scratch
//!   buffer can be re-used across many calls.
//! - **ASCII fast paths** — [`to_lowercase_ascii`] and
//!   [`to_uppercase_ascii`] for callers that *know* their input is
//!   ASCII. They are ~5–10× faster than the Unicode variants on pure
//!   ASCII data.
//!
//! # Allocation profile
//!
//! Every owned-`String`-returning function allocates once for the output.
//! The `_into` variants allocate only if the caller-supplied `out`
//! buffer needs to grow. All the transformations run in `O(n)` scalars
//! over the input.
//!
//! # `no_std`
//!
//! Every item in this module is gated on `feature = "alloc"`: an owned
//! `String` output requires the heap, and the `_into` variants require
//! [`String`] itself. A pure-no-alloc build gets an empty surface.

#![cfg(feature = "alloc")]

use alloc::string::String;

#[cfg(test)]
mod tests;

// ---------------------------------------------------------------------
// Owned-output convenience wrappers.
// ---------------------------------------------------------------------

/// Returns the Unicode lowercase form of `s`.
///
/// Delegates to [`str::to_lowercase`]. Note that Unicode case mapping
/// can change the byte length of the string (`İ` → `i̇`, `ß` → `ss`
/// under uppercase but not lowercase, etc.).
///
/// # Examples
///
/// ```
/// use stringcheese_manip::case;
///
/// assert_eq!(case::to_lowercase("Hello"), "hello");
/// // Unicode: capital sharp S lowercases to lowercase sharp S.
/// assert_eq!(case::to_lowercase("\u{1E9E}"), "ß");
/// ```
#[must_use]
pub fn to_lowercase(s: &str) -> String {
    s.to_lowercase()
}

/// Returns the Unicode uppercase form of `s`.
///
/// Delegates to [`str::to_uppercase`]. The output length may exceed the
/// input length (`ß` → `SS` is the canonical example).
///
/// # Examples
///
/// ```
/// use stringcheese_manip::case;
///
/// assert_eq!(case::to_uppercase("hello"), "HELLO");
/// // German sharp S uppercases to SS.
/// assert_eq!(case::to_uppercase("straße"), "STRASSE");
/// ```
#[must_use]
pub fn to_uppercase(s: &str) -> String {
    s.to_uppercase()
}

/// Returns `s` in title case — the first letter of every word is
/// uppercased and the remaining letters of every word are lowercased.
/// Characters that are not part of a word (whitespace, punctuation,
/// symbols) are passed through unchanged.
///
/// Word boundaries are identified with the Unicode `Alphabetic`
/// property: a new word begins at each grapheme whose first scalar is
/// alphabetic and whose predecessor was not.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::case;
///
/// assert_eq!(case::to_title_case("hello world"), "Hello World");
/// assert_eq!(case::to_title_case("HELLO WORLD"), "Hello World");
/// assert_eq!(case::to_title_case("naïve café"), "Naïve Café");
/// // Non-word characters are preserved:
/// assert_eq!(case::to_title_case("one-two"), "One-Two");
/// ```
#[must_use]
pub fn to_title_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    to_title_case_into(s, &mut out);
    out
}

/// Returns `s` with only its first character uppercased; the remainder
/// is left unchanged.
///
/// The first character is uppercased via the Unicode mapping, which can
/// expand into multiple scalars (see [`char::to_uppercase`]). An empty
/// string returns an empty string.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::case;
///
/// assert_eq!(case::capitalize(""), "");
/// assert_eq!(case::capitalize("hello"), "Hello");
/// // Only the first character changes — subsequent letters are preserved.
/// assert_eq!(case::capitalize("hELLO"), "HELLO");
/// // German sharp S uppercases to "SS".
/// assert_eq!(case::capitalize("ßtraße"), "SStraße");
/// ```
#[must_use]
pub fn capitalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    capitalize_into(s, &mut out);
    out
}

// ---------------------------------------------------------------------
// Buffer-appending variants for callers with a scratch buffer.
// ---------------------------------------------------------------------

/// Appends the Unicode lowercase form of `s` to `out`.
///
/// Reuses the caller's buffer to avoid a per-call allocation; useful in
/// tight loops.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::case;
///
/// let mut buf = String::new();
/// case::to_lowercase_into("HELLO", &mut buf);
/// case::to_lowercase_into(" WORLD", &mut buf);
/// assert_eq!(buf, "hello world");
/// ```
pub fn to_lowercase_into(s: &str, out: &mut String) {
    for c in s.chars() {
        for lc in c.to_lowercase() {
            out.push(lc);
        }
    }
}

/// Appends the Unicode uppercase form of `s` to `out`.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::case;
///
/// let mut buf = String::new();
/// case::to_uppercase_into("hello ", &mut buf);
/// case::to_uppercase_into("straße", &mut buf);
/// assert_eq!(buf, "HELLO STRASSE");
/// ```
pub fn to_uppercase_into(s: &str, out: &mut String) {
    for c in s.chars() {
        for uc in c.to_uppercase() {
            out.push(uc);
        }
    }
}

/// Appends the title-cased form of `s` to `out`.
///
/// See [`to_title_case`] for the algorithm.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::case;
///
/// let mut buf = String::new();
/// case::to_title_case_into("hello world", &mut buf);
/// assert_eq!(buf, "Hello World");
/// ```
pub fn to_title_case_into(s: &str, out: &mut String) {
    // Word tails are buffered so we can call `str::to_lowercase` on the
    // whole tail — that call is position-aware (Greek final sigma, for
    // example) whereas a char-by-char loop is not.
    let mut word_tail = String::new();
    let mut inside_word = false;

    for grapheme in stringcheese_unicode::graphemes(s) {
        // A grapheme's "wordness" is determined by its first scalar's
        // Alphabetic property. Digits and combining marks are treated
        // as extending the current run rather than starting a new word.
        let first_scalar = grapheme.chars().next();
        let alphabetic = first_scalar.is_some_and(char::is_alphabetic);
        if alphabetic && !inside_word {
            // Start of a new word: uppercase this grapheme.
            for c in grapheme.chars() {
                for uc in c.to_uppercase() {
                    out.push(uc);
                }
            }
            inside_word = true;
        } else if alphabetic {
            // Continuation of a word: buffer the grapheme; we'll lowercase
            // the whole tail together so `str::to_lowercase` can apply
            // its position-sensitive rules.
            word_tail.push_str(grapheme);
        } else {
            // Non-word grapheme: flush the buffered tail (as lowercased),
            // then pass this grapheme through unchanged.
            if !word_tail.is_empty() {
                out.push_str(&word_tail.to_lowercase());
                word_tail.clear();
            }
            out.push_str(grapheme);
            inside_word = false;
        }
    }
    // Final flush for a word that runs to end-of-input.
    if !word_tail.is_empty() {
        out.push_str(&word_tail.to_lowercase());
    }
}

/// Appends `s` to `out` with only the first character uppercased.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::case;
///
/// let mut buf = String::from("[");
/// case::capitalize_into("hello", &mut buf);
/// buf.push(']');
/// assert_eq!(buf, "[Hello]");
/// ```
pub fn capitalize_into(s: &str, out: &mut String) {
    let mut chars = s.chars();
    if let Some(first) = chars.next() {
        for uc in first.to_uppercase() {
            out.push(uc);
        }
        out.push_str(chars.as_str());
    }
}

// ---------------------------------------------------------------------
// ASCII fast paths — opt-in for callers who know their input is ASCII.
// ---------------------------------------------------------------------

/// Returns the ASCII lowercase form of `s`.
///
/// Every ASCII uppercase letter (`A`..=`Z`) is replaced with its
/// lowercase counterpart; every other byte is passed through unchanged.
/// This is meaningfully faster than [`to_lowercase`] on pure ASCII
/// input because it bypasses Unicode case-mapping tables and never
/// changes the byte length.
///
/// **This is not Unicode-aware.** Non-ASCII bytes are copied through
/// unchanged — the ASCII letters in the input are lowercased normally,
/// but a `É` in the input stays a `É`. Use only when you *know* the
/// input is ASCII, or when case-changing non-ASCII characters would
/// be a bug.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::case;
///
/// assert_eq!(case::to_lowercase_ascii("HELLO"), "hello");
/// assert_eq!(case::to_lowercase_ascii("Hi 42!"), "hi 42!");
/// // Non-ASCII scalars pass through untouched, ASCII letters still lower.
/// assert_eq!(case::to_lowercase_ascii("CAFÉ"), "cafÉ");
/// ```
#[must_use]
pub fn to_lowercase_ascii(s: &str) -> String {
    s.to_ascii_lowercase()
}

/// Returns the ASCII uppercase form of `s`.
///
/// Every ASCII lowercase letter (`a`..=`z`) is replaced with its
/// uppercase counterpart; every other byte is passed through unchanged.
///
/// **This is not Unicode-aware.** Non-ASCII scalars are returned
/// verbatim. Use only when you *know* the input is ASCII.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::case;
///
/// assert_eq!(case::to_uppercase_ascii("hello"), "HELLO");
/// // ß does NOT expand to SS here — that requires the Unicode path.
/// assert_eq!(case::to_uppercase_ascii("straße"), "STRAßE");
/// ```
#[must_use]
pub fn to_uppercase_ascii(s: &str) -> String {
    s.to_ascii_uppercase()
}
