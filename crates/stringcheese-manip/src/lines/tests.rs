//! Tests for [`crate::lines`].
//!
//! Unit tests cover empty input, LF vs. CRLF terminators, trailing
//! newlines, empty vs. whitespace-only lines, dedent with mixed leading
//! whitespace, and indent with empty lines. Property tests confirm the
//! count-preservation, roundtrip, and idempotence laws.

use super::*;

// -----------------------------------------------------------------
// lines / lines_with_terminators / non_empty_lines / count_lines
// -----------------------------------------------------------------

#[test]
fn lines_empty_is_empty() {
    let out: Vec<&str> = lines("").collect();
    assert_eq!(out, Vec::<&str>::new());
}

#[test]
fn lines_no_terminator() {
    let out: Vec<&str> = lines("abc").collect();
    assert_eq!(out, vec!["abc"]);
}

#[test]
fn lines_trailing_newline_no_extra_line() {
    // str::lines convention — a trailing terminator does not add an
    // empty final line.
    let out: Vec<&str> = lines("a\n").collect();
    assert_eq!(out, vec!["a"]);
}

#[test]
fn lines_crlf_terminator() {
    let out: Vec<&str> = lines("a\r\nb").collect();
    assert_eq!(out, vec!["a", "b"]);
}

#[test]
fn lines_multiple() {
    let out: Vec<&str> = lines("a\nb\nc").collect();
    assert_eq!(out, vec!["a", "b", "c"]);
}

#[test]
fn lines_with_terminators_empty() {
    let out: Vec<&str> = lines_with_terminators("").collect();
    assert_eq!(out, Vec::<&str>::new());
}

#[test]
fn lines_with_terminators_preserves_lf() {
    let out: Vec<&str> = lines_with_terminators("a\nb\nc").collect();
    assert_eq!(out, vec!["a\n", "b\n", "c"]);
}

#[test]
fn lines_with_terminators_preserves_trailing_lf() {
    let out: Vec<&str> = lines_with_terminators("a\nb\n").collect();
    assert_eq!(out, vec!["a\n", "b\n"]);
}

#[test]
fn lines_with_terminators_preserves_crlf() {
    let out: Vec<&str> = lines_with_terminators("a\r\nb").collect();
    assert_eq!(out, vec!["a\r\n", "b"]);
}

#[test]
fn lines_with_terminators_reassembly_recovers_input() {
    let inputs = ["", "a", "a\n", "a\nb", "a\nb\n", "a\r\nb", "a\n\nb\n"];
    for s in inputs {
        let reassembled: String = lines_with_terminators(s).collect();
        assert_eq!(reassembled, s, "input={s:?}");
    }
}

#[test]
fn non_empty_lines_skips_zero_length_lines() {
    let out: Vec<&str> = non_empty_lines("a\n\nb\n\nc").collect();
    assert_eq!(out, vec!["a", "b", "c"]);
}

#[test]
fn non_empty_lines_keeps_whitespace_only_lines() {
    // "Empty" here means literally zero-length. A whitespace-only line
    // is one non-empty character, so it passes the filter.
    let out: Vec<&str> = non_empty_lines("a\n \nb").collect();
    assert_eq!(out, vec!["a", " ", "b"]);
}

#[test]
fn count_lines_agrees_with_lines_count() {
    let inputs = ["", "a", "a\n", "a\nb", "a\nb\nc", "a\n\nb", "a\r\nb"];
    for s in inputs {
        assert_eq!(count_lines(s), s.lines().count(), "input={s:?}");
    }
}

// -----------------------------------------------------------------
// trim_lines / prefix_lines / suffix_lines
// -----------------------------------------------------------------

#[cfg(feature = "alloc")]
mod owned_output {
    use super::*;

    #[test]
    fn trim_lines_ascii() {
        assert_eq!(trim_lines("  a  \n  b  \n"), "a\nb\n");
    }

    #[test]
    fn trim_lines_tab_and_newline() {
        assert_eq!(trim_lines("\ta\t\n\tb\t"), "a\nb");
    }

    #[test]
    fn trim_lines_preserves_crlf() {
        assert_eq!(trim_lines("  a  \r\n  b  \r\n"), "a\r\nb\r\n");
    }

    #[test]
    fn trim_lines_collapses_whitespace_only() {
        assert_eq!(trim_lines("a\n   \nb"), "a\n\nb");
    }

    #[test]
    fn trim_lines_empty() {
        assert_eq!(trim_lines(""), "");
    }

    #[test]
    fn prefix_lines_basic() {
        assert_eq!(prefix_lines("a\nb\nc", "> "), "> a\n> b\n> c");
    }

    #[test]
    fn prefix_lines_trailing_newline_preserved() {
        assert_eq!(prefix_lines("a\nb\n", "> "), "> a\n> b\n");
    }

    #[test]
    fn prefix_lines_empty_input() {
        assert_eq!(prefix_lines("", "> "), "");
    }

    #[test]
    fn prefix_lines_empty_prefix_is_identity() {
        let inputs = ["", "a", "a\nb", "a\nb\n"];
        for s in inputs {
            assert_eq!(prefix_lines(s, ""), s, "input={s:?}");
        }
    }

    #[test]
    fn suffix_lines_basic() {
        assert_eq!(suffix_lines("a\nb\nc", ";"), "a;\nb;\nc;");
    }

    #[test]
    fn suffix_lines_trailing_newline_preserved() {
        assert_eq!(suffix_lines("a\nb\n", ";"), "a;\nb;\n");
    }

    #[test]
    fn suffix_lines_crlf_preserved() {
        assert_eq!(suffix_lines("a\r\nb", ";"), "a;\r\nb;");
    }

    #[test]
    fn suffix_lines_empty_input() {
        assert_eq!(suffix_lines("", ";"), "");
    }

    // -----------------------------------------------------------------
    // indent
    // -----------------------------------------------------------------

    #[test]
    fn indent_basic() {
        assert_eq!(indent("a\nb\nc", 2), "  a\n  b\n  c");
    }

    #[test]
    fn indent_zero_is_identity() {
        let inputs = ["", "a", "a\nb", "a\nb\n"];
        for s in inputs {
            assert_eq!(indent(s, 0), s, "input={s:?}");
        }
    }

    #[test]
    fn indent_empty_lines_not_indented() {
        assert_eq!(indent("a\n\nb", 4), "    a\n\n    b");
    }

    #[test]
    fn indent_preserves_terminators() {
        assert_eq!(indent("a\nb\n", 2), "  a\n  b\n");
        assert_eq!(indent("a\r\nb", 2), "  a\r\n  b");
    }

    // -----------------------------------------------------------------
    // dedent
    // -----------------------------------------------------------------

    #[test]
    fn dedent_uniform_indent() {
        assert_eq!(dedent("    a\n    b\n"), "a\nb\n");
    }

    #[test]
    fn dedent_mixed_depths_common_prefix() {
        assert_eq!(dedent("  a\n    b\n"), "a\n  b\n");
    }

    #[test]
    fn dedent_no_common_prefix_is_identity() {
        assert_eq!(dedent("a\n    b\n"), "a\n    b\n");
    }

    #[test]
    fn dedent_whitespace_only_lines_dont_constrain_prefix() {
        // The blank line should not force the common prefix to "".
        assert_eq!(dedent("    a\n\n    b\n"), "a\n\nb\n");
    }

    #[test]
    fn dedent_whitespace_only_line_collapses_to_terminator() {
        // A line consisting only of whitespace becomes empty (just the
        // newline), per Python textwrap.dedent semantics.
        assert_eq!(dedent("    a\n     \n    b\n"), "a\n\nb\n");
    }

    #[test]
    fn dedent_tabs_and_spaces_are_distinct() {
        // "\tfoo" and "    foo" share no common leading whitespace
        // because "\t" != " ".
        assert_eq!(dedent("\ta\n    b\n"), "\ta\n    b\n");
    }

    #[test]
    fn dedent_empty_input() {
        assert_eq!(dedent(""), "");
    }

    #[test]
    fn dedent_single_line() {
        assert_eq!(dedent("    hello"), "hello");
        assert_eq!(dedent("hello"), "hello");
    }
}

// -----------------------------------------------------------------
// Property tests
// -----------------------------------------------------------------

#[cfg(all(feature = "std", not(target_family = "wasm")))]
mod properties {
    use super::*;
    use proptest::prelude::*;

    fn multi_line_string() -> impl Strategy<Value = String> {
        // Small alphabet including \n so multiple lines actually arise.
        prop::string::string_regex("[a-z \\n]{0,32}").expect("static regex is valid")
    }

    fn ascii_prefix() -> impl Strategy<Value = String> {
        prop::string::string_regex("[>#-]{0,4}").expect("static regex is valid")
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        // prefix_lines preserves line count.
        #[test]
        fn prefix_lines_preserves_count(s in multi_line_string(), p in ascii_prefix()) {
            let out = prefix_lines(&s, &p);
            prop_assert_eq!(count_lines(&out), count_lines(&s));
        }

        // suffix_lines preserves line count.
        #[test]
        fn suffix_lines_preserves_count(s in multi_line_string(), p in ascii_prefix()) {
            let out = suffix_lines(&s, &p);
            prop_assert_eq!(count_lines(&out), count_lines(&s));
        }

        // trim_lines never grows line count. It can shrink the count
        // when a trailing whitespace-only line without a terminator
        // trims to empty and disappears, so equality does not hold in
        // general.
        #[test]
        fn trim_lines_never_grows_count(s in multi_line_string()) {
            prop_assert!(count_lines(&trim_lines(&s)) <= count_lines(&s));
        }

        // count_lines agrees with str::lines().count().
        #[test]
        fn count_lines_matches_std(s in multi_line_string()) {
            prop_assert_eq!(count_lines(&s), s.lines().count());
        }

        // lines_with_terminators reassembles to the input.
        #[test]
        fn lines_with_terminators_reassembly(s in multi_line_string()) {
            let reassembled: String = lines_with_terminators(&s).collect();
            prop_assert_eq!(reassembled, s);
        }

        // indent(_, 0) is identity.
        #[test]
        fn indent_zero_is_identity(s in multi_line_string()) {
            prop_assert_eq!(indent(&s, 0), s.clone());
        }

        // dedent is idempotent — dedenting again removes nothing.
        #[test]
        fn dedent_is_idempotent(s in multi_line_string()) {
            let once = dedent(&s);
            let twice = dedent(&once);
            prop_assert_eq!(once, twice);
        }
    }
}
