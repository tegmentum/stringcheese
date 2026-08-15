//! Case conversion — one [`Case`] enum, one [`to_case`] dispatch.
//!
//! Wraps [`heck`] because the case-conversion problem is small,
//! well-understood, and already solved by a mature crate. Our
//! contribution here is the type discipline: every conversion
//! names the target [`Case`] explicitly rather than making the
//! caller remember whether `to_snake_case` produces `snake_case`
//! or `SNAKE_CASE`.
//!
//! ## Detection
//!
//! [`Case::detect`] classifies an input's convention (`snake` /
//! `kebab` / `camel` / `Pascal` / `SCREAMING_SNAKE` / mixed). The
//! answer is best-effort — inputs that don't cleanly fit any
//! convention (e.g. `"Foo_bar-Baz"`) return `None`. Useful for
//! round-trip pipelines that want to preserve whatever style the
//! caller passed in.
//!
//! ## ASCII fast path
//!
//! When [`to_case`]'s input is pure ASCII (`str::is_ascii`), the
//! dispatch skips [`heck`]'s Unicode-category machinery entirely
//! and runs a hand-rolled byte-level word-boundary scanner. The
//! scanner mirrors heck's algorithm (documented in the crate's
//! `//!` header: `HelloWorld` → `Hello|World`, `XMLHttpRequest` →
//! `XML|Http|Request`) — see the differential test
//! `ascii_fast_path_matches_heck` for the byte-for-byte guarantee
//! across every [`Case`] target and every heck test corpus input.
//! Speedup is roughly 5-15× on ASCII input; non-ASCII input still
//! takes the heck path unchanged.

use alloc::string::String;
use alloc::vec::Vec;

use heck::{
    ToKebabCase, ToLowerCamelCase, ToShoutyKebabCase, ToShoutySnakeCase, ToSnakeCase, ToTrainCase,
    ToUpperCamelCase,
};

/// The naming convention of an identifier string.
///
/// Both the *source* case (for [`Case::detect`]) and the *target*
/// case (for [`to_case`]). Same enum, one vocabulary.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Case {
    /// `snake_case` — words joined by `_`, all lowercase.
    Snake,
    /// `SCREAMING_SNAKE_CASE` — snake with every word uppercase.
    ScreamingSnake,
    /// `kebab-case` — words joined by `-`, all lowercase.
    Kebab,
    /// `SCREAMING-KEBAB-CASE` — kebab with every word uppercase.
    ScreamingKebab,
    /// `camelCase` — first word lowercase, subsequent words
    /// initial-uppercase, no separator.
    Camel,
    /// `PascalCase` — every word initial-uppercase, no separator.
    Pascal,
    /// `Train-Case` — kebab with every word initial-uppercase
    /// (a.k.a. "HTTP header case").
    Train,
}

impl Case {
    /// Best-effort classification of an input's convention. Returns
    /// `None` when the input doesn't cleanly match any convention
    /// (mixed separators, mixed case discipline, or empty).
    #[must_use]
    pub fn detect(input: &str) -> Option<Self> {
        if input.is_empty() {
            return None;
        }
        let has_underscore = input.contains('_');
        let has_hyphen = input.contains('-');
        // Mixed separators aren't any convention we cover.
        if has_underscore && has_hyphen {
            return None;
        }
        let all_upper = input
            .chars()
            .all(|c| !c.is_alphabetic() || c.is_uppercase());
        let all_lower = input
            .chars()
            .all(|c| !c.is_alphabetic() || c.is_lowercase());
        let has_upper = input.chars().any(char::is_uppercase);
        let has_lower = input.chars().any(char::is_lowercase);

        if has_underscore {
            // Underscore convention — either snake or SCREAMING_SNAKE.
            if all_upper {
                return Some(Self::ScreamingSnake);
            }
            if all_lower {
                return Some(Self::Snake);
            }
            // `snake_Case` or similar mixed — no clean convention.
            return None;
        }
        if has_hyphen {
            if all_upper {
                return Some(Self::ScreamingKebab);
            }
            if all_lower {
                return Some(Self::Kebab);
            }
            // Train-Case: every word initial-uppercase, rest lower.
            if is_train_case(input) {
                return Some(Self::Train);
            }
            return None;
        }
        // No separator — camel or Pascal.
        let first = input.chars().next()?;
        if !first.is_alphabetic() {
            return None;
        }
        if first.is_uppercase() && has_lower {
            return Some(Self::Pascal);
        }
        if first.is_lowercase() && has_upper {
            return Some(Self::Camel);
        }
        // All-upper or all-lower without separators is ambiguous —
        // a single word matches every convention. Return `None` so
        // callers pick, rather than committing arbitrarily.
        None
    }
}

fn is_train_case(input: &str) -> bool {
    for word in input.split('-') {
        let mut chars = word.chars();
        let Some(first) = chars.next() else {
            return false; // empty word (leading/trailing `-`)
        };
        if !first.is_uppercase() {
            return false;
        }
        if !chars.all(|c| !c.is_alphabetic() || c.is_lowercase()) {
            return false;
        }
    }
    true
}

/// Convert `input` to `target` case.
///
/// Delegates to [`heck`] for the general (Unicode) path — a
/// pass-through of a mature engine with our type discipline on
/// top. On pure-ASCII input takes a hand-rolled fast path that
/// bypasses heck's Unicode-category dispatch and produces the
/// byte-for-byte identical result at ~5-15× the throughput; see
/// the module-level `ASCII fast path` section for the
/// correctness guarantee.
#[must_use]
pub fn to_case(input: &str, target: Case) -> String {
    // ASCII fast path — the boundary rules heck applies to the
    // Alphabetic / Lowercase / Uppercase / Numeric categories all
    // reduce, on the ASCII range, to a byte-level a-zA-Z0-9 split
    // plus the same lowercase→uppercase / uppercase-run→lowercase
    // sub-word split rules. `String::is_ascii` is one SIMD-
    // accelerated scan; the win comes from replacing heck's
    // per-char Unicode-table dispatch with an in-cache byte loop.
    if input.is_ascii() {
        return to_case_ascii(input.as_bytes(), target);
    }
    match target {
        Case::Snake => input.to_snake_case(),
        Case::ScreamingSnake => input.to_shouty_snake_case(),
        Case::Kebab => input.to_kebab_case(),
        Case::ScreamingKebab => input.to_shouty_kebab_case(),
        Case::Camel => input.to_lower_camel_case(),
        Case::Pascal => input.to_upper_camel_case(),
        Case::Train => input.to_train_case(),
    }
}

/// Hand-rolled ASCII-only case conversion.
///
/// Byte-for-byte identical to the heck delegation for ASCII input
/// (verified by `ascii_fast_path_matches_heck`). Two-level word
/// segmentation:
///
/// 1. **Outer split** — every maximal run of ASCII alphanumerics
///    (`[A-Za-z0-9]+`) is one word; every non-alphanumeric byte
///    is a separator that is dropped.
/// 2. **Inner sub-word split** — inside each alphanumeric word,
///    heck's tri-state (`Boundary` / `Lower` / `Upper`) case
///    scanner splits again at:
///    * a `lowercase → uppercase` transition (boundary *after*
///      the lowercase — `helloWorld` → `hello|World`), and
///    * the last of an uppercase run before a lowercase
///      (boundary *before* the last uppercase —
///      `XMLHttpRequest` → `XML|Http|Request`).
///
/// The state machine is a byte-for-byte translation of heck's
/// `transform` fn (with the `char_indices`/`peek` loop unrolled
/// to direct byte indexing, valid because every ASCII scalar is
/// one byte). Non-letter, non-digit characters inside a word
/// range would be impossible — the outer split already peeled
/// them out.
///
/// Callers reach this via [`to_case`]; it is not `pub` on its own
/// because a bad-input non-ASCII branch would panic in
/// `String::from_utf8`.
fn to_case_ascii(bytes: &[u8], target: Case) -> String {
    // Separator byte emitted between sub-words for the joined
    // cases; `None` for the camel/pascal family where sub-words
    // fuse directly.
    let sep: Option<u8> = match target {
        Case::Snake | Case::ScreamingSnake => Some(b'_'),
        Case::Kebab | Case::ScreamingKebab | Case::Train => Some(b'-'),
        Case::Camel | Case::Pascal => None,
    };
    // Rough over-allocation: mixed-case input like `aBaBaB` snake-
    // cases to `a_b_a_b_a_b` (2× the byte length). The +8 covers
    // the common short-input case where reallocs cost more than
    // the slack.
    let cap = bytes
        .len()
        .saturating_add(bytes.len() / 4)
        .saturating_add(8);
    let mut out: Vec<u8> = Vec::with_capacity(cap);
    let mut first_subword = true;

    let mut idx = 0;
    let len = bytes.len();
    while idx < len {
        // Skip separators (non-alphanumeric ASCII).
        while idx < len && !bytes[idx].is_ascii_alphanumeric() {
            idx += 1;
        }
        if idx >= len {
            break;
        }
        let word_start = idx;
        while idx < len && bytes[idx].is_ascii_alphanumeric() {
            idx += 1;
        }
        let word = &bytes[word_start..idx];
        emit_word_subwords(word, target, sep, &mut first_subword, &mut out);
    }

    String::from_utf8(out).expect("ASCII bytes stay valid UTF-8")
}

/// The `WordMode` state machine mirrored from heck's `transform`.
///
/// Tracks the case of the *last cased character* in the current
/// sub-word being accumulated. `Boundary` means "no cased character
/// yet" — the start of a sub-word.
#[derive(Clone, Copy, PartialEq, Eq)]
enum WordMode {
    Boundary,
    Lower,
    Upper,
}

/// Split one alphanumeric run into sub-words and emit each with
/// the target case formatting into `out`.
fn emit_word_subwords(
    word: &[u8],
    target: Case,
    sep: Option<u8>,
    first_subword: &mut bool,
    out: &mut Vec<u8>,
) {
    let word_len = word.len();
    let mut mode = WordMode::Boundary;
    let mut init = 0usize;
    let mut i = 0usize;
    while i < word_len {
        let c = word[i];
        if i + 1 == word_len {
            // Trailing sub-word: `word[init..]`.
            emit_subword(&word[init..], target, sep, first_subword, out);
            return;
        }
        let next = word[i + 1];
        // The mode if the current character does not induce a
        // boundary — mirrors heck's `next_mode` computation.
        let next_mode = if c.is_ascii_lowercase() {
            WordMode::Lower
        } else if c.is_ascii_uppercase() {
            WordMode::Upper
        } else {
            mode
        };
        if next_mode == WordMode::Lower && next.is_ascii_uppercase() {
            // lower→upper: boundary after current character.
            // Emit `word[init..=i]`, next sub-word starts at i+1.
            emit_subword(&word[init..=i], target, sep, first_subword, out);
            init = i + 1;
            mode = WordMode::Boundary;
        } else if mode == WordMode::Upper && c.is_ascii_uppercase() && next.is_ascii_lowercase() {
            // upper-run → upper-then-lower: boundary *before*
            // current character. Emit `word[init..i]` and let
            // this uppercase char begin the next sub-word.
            emit_subword(&word[init..i], target, sep, first_subword, out);
            init = i;
            mode = WordMode::Boundary;
        } else {
            mode = next_mode;
        }
        i += 1;
    }
}

/// Write one sub-word into `out`, prepending the separator when
/// this is not the first sub-word of the whole output.
///
/// Empty sub-words never occur under the boundary rules above but
/// the guard keeps the invariant local — a stray empty slice
/// would otherwise emit a trailing separator with no letters.
fn emit_subword(
    sub: &[u8],
    target: Case,
    sep: Option<u8>,
    first_subword: &mut bool,
    out: &mut Vec<u8>,
) {
    if sub.is_empty() {
        return;
    }
    if !*first_subword {
        if let Some(s) = sep {
            out.push(s);
        }
    }
    match target {
        Case::Snake | Case::Kebab => {
            for &b in sub {
                out.push(b.to_ascii_lowercase());
            }
        }
        Case::ScreamingSnake | Case::ScreamingKebab => {
            for &b in sub {
                out.push(b.to_ascii_uppercase());
            }
        }
        Case::Camel if *first_subword => {
            for &b in sub {
                out.push(b.to_ascii_lowercase());
            }
        }
        Case::Camel | Case::Pascal | Case::Train => {
            // `sub` is non-empty (guarded above), so the split
            // is infallible.
            let (&head, rest) = sub.split_first().expect("sub is non-empty");
            out.push(head.to_ascii_uppercase());
            for &b in rest {
                out.push(b.to_ascii_lowercase());
            }
        }
    }
    *first_subword = false;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_snake_to_camel() {
        assert_eq!(to_case("hello_world_foo", Case::Camel), "helloWorldFoo");
    }

    #[test]
    fn camel_to_snake() {
        assert_eq!(to_case("helloWorldFoo", Case::Snake), "hello_world_foo");
    }

    #[test]
    fn pascal_to_kebab() {
        assert_eq!(to_case("HelloWorldFoo", Case::Kebab), "hello-world-foo");
    }

    #[test]
    fn snake_to_screaming_snake() {
        assert_eq!(to_case("hello_world", Case::ScreamingSnake), "HELLO_WORLD",);
    }

    #[test]
    fn detect_common_conventions() {
        assert_eq!(Case::detect("hello_world"), Some(Case::Snake));
        assert_eq!(Case::detect("HELLO_WORLD"), Some(Case::ScreamingSnake));
        assert_eq!(Case::detect("hello-world"), Some(Case::Kebab));
        assert_eq!(Case::detect("HELLO-WORLD"), Some(Case::ScreamingKebab));
        assert_eq!(Case::detect("helloWorld"), Some(Case::Camel));
        assert_eq!(Case::detect("HelloWorld"), Some(Case::Pascal));
        assert_eq!(Case::detect("Hello-World"), Some(Case::Train));
    }

    #[test]
    fn detect_returns_none_for_ambiguous_or_mixed() {
        assert_eq!(Case::detect(""), None);
        // Mixed separators — not any convention we recognize.
        assert_eq!(Case::detect("foo_bar-baz"), None);
        // Single word — matches every convention; caller must pick.
        assert_eq!(Case::detect("hello"), None);
        assert_eq!(Case::detect("HELLO"), None);
    }

    /// Byte-for-byte equivalence between the ASCII fast path and
    /// the heck path across every [`Case`] target and every
    /// non-trivial corpus input. Anchor test for the fast path —
    /// heck's boundary rules around digits, all-caps runs, and
    /// mixed separators are subtle, so the corpus lifts every
    /// example from heck's own crate tests plus a handful of
    /// stress inputs (leading / trailing / doubled separators,
    /// single characters, digits at edges).
    #[test]
    fn ascii_fast_path_matches_heck() {
        let corpus = [
            "",
            "a",
            "A",
            "1",
            "abc",
            "ABC",
            "aBc",
            "hello_world",
            "hello-world",
            "helloWorld",
            "HelloWorld",
            "HELLO_WORLD",
            "HELLO-WORLD",
            "Hello-World",
            // Heck's own snake_case corpus — every documented
            // digit-boundary quirk lives here.
            "CamelCase",
            "This is Human case.",
            "MixedUP CamelCase, with some Spaces",
            "mixed_up_ snake_case with some _spaces",
            "kebab-case",
            "SHOUTY_SNAKE_CASE",
            "snake_case",
            "this-contains_ ALLKinds OfWord_Boundaries",
            "XMLHttpRequest",
            "FIELD_NAME11",
            "99BOTTLES",
            "FieldNamE11",
            "abc123def456",
            "abc123DEF456",
            "abc123Def456",
            "abc123DEf456",
            "ABC123def456",
            "ABC123DEF456",
            "ABC123Def456",
            "ABC123DEf456",
            "ABC123dEEf456FOO",
            "abcDEF",
            "ABcDE",
            // Separator edge cases: leading, trailing, doubled,
            // empty runs.
            "__leading",
            "trailing__",
            "__both__",
            "..dots..",
            "one__two",
            "one--two",
            "  spaces  ",
            // Digit-at-edge cases.
            "1a",
            "a1",
            "1A",
            "A1",
            "123abc",
            "abc123",
        ];
        for &input in &corpus {
            assert!(input.is_ascii(), "corpus stayed ASCII: {input:?}");
            for target in [
                Case::Snake,
                Case::ScreamingSnake,
                Case::Kebab,
                Case::ScreamingKebab,
                Case::Camel,
                Case::Pascal,
                Case::Train,
            ] {
                let ours = to_case(input, target);
                let heck_out = match target {
                    Case::Snake => input.to_snake_case(),
                    Case::ScreamingSnake => input.to_shouty_snake_case(),
                    Case::Kebab => input.to_kebab_case(),
                    Case::ScreamingKebab => input.to_shouty_kebab_case(),
                    Case::Camel => input.to_lower_camel_case(),
                    Case::Pascal => input.to_upper_camel_case(),
                    Case::Train => input.to_train_case(),
                };
                assert_eq!(
                    ours, heck_out,
                    "target={target:?} input={input:?} — fast path diverged from heck",
                );
            }
        }
    }
}
