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

use alloc::string::String;

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
/// Delegates to [`heck`] for every conversion — pass-through of a
/// mature engine with our type discipline on top.
#[must_use]
pub fn to_case(input: &str, target: Case) -> String {
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
}
