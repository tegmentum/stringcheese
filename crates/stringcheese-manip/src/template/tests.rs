//! Tests for [`crate::template`].
//!
//! Unit tests cover the empty / literal-only / duplicate-reference /
//! escape-brace edge cases plus the three error kinds. Property tests
//! confirm that a single-placeholder template `"{x}"` renders to
//! whatever value the context returns for `x`.

use super::*;
use alloc::collections::BTreeMap;
#[cfg(feature = "std")]
use std::collections::HashMap;

// -----------------------------------------------------------------
// Basic rendering
// -----------------------------------------------------------------

#[test]
fn render_empty_template() {
    let vars: &[(&str, &str)] = &[];
    assert_eq!(render_with("", vars).unwrap(), "");
}

#[test]
fn render_literal_only() {
    let vars: &[(&str, &str)] = &[];
    assert_eq!(
        render_with("no placeholders here!", vars).unwrap(),
        "no placeholders here!"
    );
}

#[test]
fn render_single_placeholder() {
    assert_eq!(
        render_with("Hello, {name}!", &[("name", "world")]).unwrap(),
        "Hello, world!"
    );
}

#[test]
fn render_multiple_placeholders() {
    assert_eq!(
        render_with(
            "{greeting}, {name}!",
            &[("greeting", "Hello"), ("name", "world")]
        )
        .unwrap(),
        "Hello, world!"
    );
}

#[test]
fn render_duplicate_placeholder_uses_same_value() {
    assert_eq!(render_with("{x}{x}{x}", &[("x", "!")]).unwrap(), "!!!");
}

#[test]
fn render_placeholder_at_edges() {
    assert_eq!(
        render_with("{x}middle{y}", &[("x", "L"), ("y", "R")]).unwrap(),
        "LmiddleR"
    );
}

#[test]
fn render_variable_value_can_be_empty() {
    assert_eq!(render_with("[{v}]", &[("v", "")]).unwrap(), "[]");
}

#[test]
fn render_underscore_identifier() {
    assert_eq!(
        render_with("{_x}{y_2}", &[("_x", "A"), ("y_2", "B")]).unwrap(),
        "AB"
    );
}

// -----------------------------------------------------------------
// Escaped braces
// -----------------------------------------------------------------

#[test]
fn render_double_open_brace_is_literal() {
    let vars: &[(&str, &str)] = &[];
    assert_eq!(render_with("{{", vars).unwrap(), "{");
}

#[test]
fn render_double_close_brace_is_literal() {
    let vars: &[(&str, &str)] = &[];
    assert_eq!(render_with("}}", vars).unwrap(), "}");
}

#[test]
fn render_escaped_braces_around_placeholder() {
    assert_eq!(render_with("{{ {x} }}", &[("x", "42")]).unwrap(), "{ 42 }");
}

// -----------------------------------------------------------------
// Errors
// -----------------------------------------------------------------

#[test]
fn render_unknown_variable_errors() {
    let vars: &[(&str, &str)] = &[];
    let err = render_with("hi {name}", vars).unwrap_err();
    match err {
        TemplateError::UnknownVariable { name, position } => {
            assert_eq!(name, "name");
            assert_eq!(position, 3);
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn render_unbalanced_open_errors() {
    let vars: &[(&str, &str)] = &[];
    let err = render_with("hi {name", vars).unwrap_err();
    assert!(matches!(
        err,
        TemplateError::UnbalancedBrace { position: 3 }
    ));
}

#[test]
fn render_stray_close_errors() {
    let vars: &[(&str, &str)] = &[];
    let err = render_with("hi }", vars).unwrap_err();
    assert!(matches!(
        err,
        TemplateError::UnbalancedBrace { position: 3 }
    ));
}

#[test]
fn render_invalid_identifier_errors() {
    let vars: &[(&str, &str)] = &[("a b", "x")];
    let err = render_with("{a b}", vars).unwrap_err();
    match err {
        TemplateError::InvalidIdentifier { name, position } => {
            assert_eq!(name, "a b");
            assert_eq!(position, 0);
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn render_empty_placeholder_is_invalid_identifier() {
    let vars: &[(&str, &str)] = &[];
    let err = render_with("{}", vars).unwrap_err();
    assert!(matches!(err, TemplateError::InvalidIdentifier { .. }));
}

#[test]
fn render_nested_open_brace_errors() {
    let vars: &[(&str, &str)] = &[];
    // `{a{b}` — inner `{` before the close means the first `{a` is
    // unbalanced.
    let err = render_with("{a{b}", vars).unwrap_err();
    assert!(matches!(
        err,
        TemplateError::UnbalancedBrace { position: 0 }
    ));
}

// -----------------------------------------------------------------
// render_positional
// -----------------------------------------------------------------

#[test]
fn positional_basic() {
    assert_eq!(
        render_positional("{0} + {1} = {2}", &["1", "2", "3"]).unwrap(),
        "1 + 2 = 3"
    );
}

#[test]
fn positional_out_of_range_errors() {
    let err = render_positional("{5}", &["a", "b"]).unwrap_err();
    assert!(matches!(err, TemplateError::UnknownVariable { .. }));
}

#[test]
fn positional_named_identifier_errors() {
    // A named placeholder isn't a valid positional index.
    let err = render_positional("{name}", &["a"]).unwrap_err();
    assert!(matches!(err, TemplateError::UnknownVariable { .. }));
}

// -----------------------------------------------------------------
// Permissive rendering
// -----------------------------------------------------------------

#[test]
fn permissive_leaves_unknown_placeholder() {
    let vars: &[(&str, &str)] = &[("name", "world")];
    assert_eq!(
        render_permissive("Hello, {name}, from {who}!", &vars),
        "Hello, world, from {who}!"
    );
}

#[test]
fn permissive_leaves_all_unknown() {
    let vars: &[(&str, &str)] = &[];
    assert_eq!(render_permissive("{a}{b}{c}", &vars), "{a}{b}{c}");
}

#[test]
fn permissive_handles_escaped_braces() {
    let vars: &[(&str, &str)] = &[];
    assert_eq!(render_permissive("{{ok}}", &vars), "{ok}");
}

// -----------------------------------------------------------------
// render_iter
// -----------------------------------------------------------------

#[test]
fn iter_yields_literal_and_variable_spans() {
    let vars: &[(&str, &str)] = &[("x", "42")];
    let vars: &dyn TemplateContext = &vars;
    let spans: Vec<&str> = render_iter("a{x}b", vars).map(Result::unwrap).collect();
    assert_eq!(spans, vec!["a", "42", "b"]);
}

#[test]
fn iter_writes_to_buffer_without_allocating_output() {
    use core::fmt::Write;
    let vars: &[(&str, &str)] = &[("who", "world")];
    let vars: &dyn TemplateContext = &vars;
    let mut out = String::new();
    for span in render_iter("Hello, {who}!", vars) {
        out.write_str(span.unwrap()).unwrap();
    }
    assert_eq!(out, "Hello, world!");
}

#[test]
fn iter_stops_at_first_error() {
    let vars: &[(&str, &str)] = &[];
    let vars: &dyn TemplateContext = &vars;
    let mut it = render_iter("a{missing}b", vars);
    // First span: literal "a".
    assert_eq!(it.next(), Some(Ok("a")));
    // Second: the unknown-variable error.
    let err = it.next().unwrap().unwrap_err();
    assert!(matches!(err, TemplateError::UnknownVariable { .. }));
    // No further items.
    assert_eq!(it.next(), None);
}

// -----------------------------------------------------------------
// TemplateContext impls
// -----------------------------------------------------------------

#[test]
fn context_from_btreemap() {
    let mut map = BTreeMap::new();
    map.insert("name".to_string(), "world".to_string());
    assert_eq!(render("Hello, {name}!", &map).unwrap(), "Hello, world!");
}

#[cfg(feature = "std")]
#[test]
fn context_from_hashmap() {
    let mut map = HashMap::new();
    map.insert("name".to_string(), "world".to_string());
    assert_eq!(render("Hello, {name}!", &map).unwrap(), "Hello, world!");
}

#[test]
fn context_from_pair_slice() {
    let pairs: &[(&str, &str)] = &[("x", "1"), ("y", "2")];
    // Passing directly by-reference — the blanket `&T: TemplateContext`
    // impl kicks in.
    assert_eq!(render("{x}+{y}", &pairs).unwrap(), "1+2");
}

// -----------------------------------------------------------------
// Error Display
// -----------------------------------------------------------------

#[test]
fn error_display_mentions_position_and_name() {
    let err = TemplateError::UnknownVariable {
        name: "foo".into(),
        position: 12,
    };
    let s = alloc::format!("{err}");
    assert!(s.contains("foo"), "{s}");
    assert!(s.contains("12"), "{s}");
}

// -----------------------------------------------------------------
// Property tests
// -----------------------------------------------------------------

#[cfg(all(feature = "std", not(target_family = "wasm")))]
mod properties {
    use super::*;
    use proptest::prelude::*;

    // Values with no `{` or `}` — safe to embed in a template without
    // accidentally producing more placeholders.
    fn safe_value() -> impl Strategy<Value = String> {
        prop::string::string_regex("[\\u0020-\\u007A\\u007C\\u007E]{0,32}")
            .expect("static regex is valid")
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        // The single-placeholder template resolves to exactly the value.
        #[test]
        fn single_placeholder_resolves(v in safe_value()) {
            let out = render_with("{x}", &[("x", v.as_str())]).unwrap();
            prop_assert_eq!(out, v);
        }

        // A literal-only template renders to itself.
        #[test]
        fn literal_only_renders_to_itself(lit in "[a-zA-Z0-9 ]{0,32}") {
            let vars: &[(&str, &str)] = &[];
            let out = render_with(&lit, vars).unwrap();
            prop_assert_eq!(out, lit);
        }

        // Escaped braces round-trip: `{{` and `}}` are the only way to
        // produce literal braces in the output.
        #[test]
        fn escaped_braces_produce_literal(prefix in "[a-z]{0,8}", suffix in "[a-z]{0,8}") {
            let vars: &[(&str, &str)] = &[];
            let tmpl = alloc::format!("{prefix}{{{{{suffix}}}}}");
            let expected = alloc::format!("{prefix}{{{suffix}}}");
            let out = render_with(&tmpl, vars).unwrap();
            prop_assert_eq!(out, expected);
        }

        // Permissive rendering never returns an error and always
        // produces some output.
        #[test]
        fn permissive_never_panics(t in "[a-zA-Z0-9 {}]{0,32}") {
            let vars: &[(&str, &str)] = &[];
            let _ = render_permissive(&t, &vars);
        }

        // Positional rendering with n args and template "{0}...{n-1}"
        // concatenates them.
        #[test]
        fn positional_concat(args in prop::collection::vec("[a-z]{0,4}", 0..8)) {
            use core::fmt::Write;
            let refs: Vec<&str> = args.iter().map(String::as_str).collect();
            let mut tmpl = String::new();
            for i in 0..args.len() {
                write!(tmpl, "{{{i}}}").unwrap();
            }
            let out = render_positional(&tmpl, &refs).unwrap();
            let expected: String = args.concat();
            prop_assert_eq!(out, expected);
        }
    }
}
