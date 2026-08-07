//! Tests for [`crate::pipeline`].
//!
//! Unit tests exercise every concrete [`Operation`] in isolation, plus
//! representative 2–4-stage pipelines that combine them. Property tests
//! verify the four laws the pipeline must obey:
//!
//! - **Empty is identity.** [`TextPipeline::new()`]`.apply(s) == s`.
//! - **Single stage matches direct apply.** A pipeline of `[op]` produces
//!   the same output as calling `op.apply(&s, &mut buf)` directly.
//! - **Debug stability.** A pipeline's `Debug` output is deterministic
//!   and identifies every stage.
//! - **Short-circuit is terminal.** Any stages appended *after*
//!   [`Truncate`] do not affect the output (Truncate short-circuits when
//!   the input exceeds the limit).

use super::*;
use crate::escape::PercentSet;
use crate::normalize::LineEnding;
use crate::trim as trim_mod;
use alloc::format;

// =====================================================================
// Individual operation tests
// =====================================================================

fn apply_direct<O: Operation>(op: &O, input: &str) -> String {
    let mut buf = String::new();
    op.apply(input, &mut buf);
    buf
}

// -------- Trim --------------------------------------------------------

#[test]
fn trim_wraps_whitespace_policy() {
    let op = Trim(trim_mod::Trim::whitespace());
    assert_eq!(op.name(), "Trim");
    assert_eq!(apply_direct(&op, "  hi  "), "hi");
}

#[test]
fn trim_wraps_char_policy() {
    let op = Trim(trim_mod::Trim::chars(&['/']));
    assert_eq!(apply_direct(&op, "//a//"), "a");
}

#[test]
fn trim_appends_to_existing_buffer() {
    let op = Trim(trim_mod::Trim::whitespace());
    let mut buf = String::from("[");
    op.apply("  hi  ", &mut buf);
    buf.push(']');
    assert_eq!(buf, "[hi]");
}

// -------- Normalize --------------------------------------------------

#[test]
fn normalize_whitespace_collapses() {
    let op = Normalize(NormalizeKind::Whitespace);
    assert_eq!(op.name(), "Normalize");
    assert_eq!(apply_direct(&op, "  a   b  "), "a b");
}

#[test]
fn normalize_line_endings_to_lf() {
    let op = Normalize(NormalizeKind::LineEndings(LineEnding::Lf));
    assert_eq!(apply_direct(&op, "a\r\nb\rc"), "a\nb\nc");
}

#[test]
fn normalize_control_keeps_common_whitespace() {
    let op = Normalize(NormalizeKind::Control);
    assert_eq!(apply_direct(&op, "a\x07b\tc"), "ab\tc");
}

#[test]
fn normalize_ansi_strips_csi() {
    let op = Normalize(NormalizeKind::Ansi);
    assert_eq!(apply_direct(&op, "\x1b[31mred\x1b[0m"), "red");
}

#[test]
fn normalize_quotes_asciifies() {
    let op = Normalize(NormalizeKind::Quotes);
    assert_eq!(apply_direct(&op, "\u{201C}hi\u{201D}"), "\"hi\"");
}

#[test]
fn normalize_dashes_asciifies() {
    let op = Normalize(NormalizeKind::Dashes);
    assert_eq!(apply_direct(&op, "a\u{2014}b"), "a--b");
}

#[test]
fn normalize_ellipsis_asciifies() {
    let op = Normalize(NormalizeKind::Ellipsis);
    assert_eq!(apply_direct(&op, "wait\u{2026}"), "wait...");
}

#[test]
fn normalize_nfc_composes() {
    let op = Normalize(NormalizeKind::Nfc);
    assert_eq!(apply_direct(&op, "cafe\u{0301}"), "caf\u{00E9}");
}

#[test]
fn normalize_nfd_decomposes() {
    let op = Normalize(NormalizeKind::Nfd);
    assert_eq!(apply_direct(&op, "caf\u{00E9}"), "cafe\u{0301}");
}

#[test]
fn normalize_nfkc_reduces_compatibility() {
    let op = Normalize(NormalizeKind::Nfkc);
    assert_eq!(apply_direct(&op, "\u{FF11}"), "1");
}

#[test]
fn normalize_nfkd_reduces_compatibility() {
    let op = Normalize(NormalizeKind::Nfkd);
    // Full-width digit → ASCII digit under NFKD too.
    assert_eq!(apply_direct(&op, "\u{FF11}"), "1");
}

// -------- CaseFold ---------------------------------------------------

#[test]
fn case_fold_lower() {
    let op = CaseFold(CaseKind::Lower);
    assert_eq!(op.name(), "CaseFold");
    assert_eq!(apply_direct(&op, "HELLO"), "hello");
}

#[test]
fn case_fold_upper() {
    let op = CaseFold(CaseKind::Upper);
    assert_eq!(apply_direct(&op, "straße"), "STRASSE");
}

#[test]
fn case_fold_title() {
    let op = CaseFold(CaseKind::Title);
    assert_eq!(apply_direct(&op, "hello world"), "Hello World");
}

#[test]
fn case_fold_capitalize() {
    let op = CaseFold(CaseKind::Capitalize);
    assert_eq!(apply_direct(&op, "hello"), "Hello");
}

#[test]
fn case_fold_lower_ascii_preserves_non_ascii() {
    let op = CaseFold(CaseKind::LowerAscii);
    assert_eq!(apply_direct(&op, "CAFÉ"), "cafÉ");
}

#[test]
fn case_fold_upper_ascii_preserves_non_ascii() {
    let op = CaseFold(CaseKind::UpperAscii);
    assert_eq!(apply_direct(&op, "straße"), "STRAßE");
}

// -------- CollapseWhitespace ----------------------------------------

#[test]
fn collapse_whitespace_shortcut() {
    let op = CollapseWhitespace;
    assert_eq!(op.name(), "CollapseWhitespace");
    assert_eq!(apply_direct(&op, " a  b\tc "), "a b c");
}

// -------- Remove -----------------------------------------------------

#[test]
fn remove_deletes_all_needles() {
    let op = Remove(String::from("!"));
    assert_eq!(op.name(), "Remove");
    assert_eq!(apply_direct(&op, "hi!! there!"), "hi there");
}

#[test]
fn remove_empty_needle_is_noop() {
    let op = Remove(String::new());
    assert_eq!(apply_direct(&op, "abc"), "abc");
}

// -------- Replace ----------------------------------------------------

#[test]
fn replace_swaps_needle() {
    let op = Replace {
        from: String::from("cat"),
        to: String::from("dog"),
    };
    assert_eq!(op.name(), "Replace");
    assert_eq!(apply_direct(&op, "cat and cat"), "dog and dog");
}

#[test]
fn replace_empty_from_is_noop() {
    let op = Replace {
        from: String::new(),
        to: String::from("!"),
    };
    assert_eq!(apply_direct(&op, "abc"), "abc");
}

// -------- Escape -----------------------------------------------------

#[test]
fn escape_html_encodes_reserved() {
    let op = Escape(EscapeKind::Html);
    assert_eq!(op.name(), "Escape");
    assert_eq!(apply_direct(&op, "<b>&</b>"), "&lt;b&gt;&amp;&lt;/b&gt;");
}

#[test]
fn escape_json_encodes_control() {
    let op = Escape(EscapeKind::Json);
    assert_eq!(apply_direct(&op, "hi\n"), "hi\\n");
}

#[test]
fn escape_shell_posix_quotes() {
    let op = Escape(EscapeKind::ShellPosix);
    // The exact output shape is a `escape_shell_posix` detail; verify it
    // is at least non-empty and different from the input for input that
    // needs quoting.
    let out = apply_direct(&op, "hi there");
    assert_ne!(out, "hi there", "shell-quoted output should differ");
}

#[test]
fn escape_shell_windows_quotes() {
    let op = Escape(EscapeKind::ShellWindows);
    let out = apply_direct(&op, "hi there");
    assert_ne!(out, "hi there");
}

#[test]
fn escape_percent_path_encodes_space() {
    let op = Escape(EscapeKind::Percent(PercentSet::Path));
    assert_eq!(apply_direct(&op, "a b"), "a%20b");
}

#[test]
fn escape_c_string_encodes_control() {
    let op = Escape(EscapeKind::CString);
    assert_eq!(apply_direct(&op, "hi\n"), "hi\\n");
}

#[test]
fn escape_regex_escapes_meta() {
    let op = Escape(EscapeKind::Regex);
    // The literal '.' is a regex meta-char; it must be escaped.
    let out = apply_direct(&op, "a.b");
    assert!(out.contains("\\."), "output {out:?} should escape '.'");
}

// -------- Truncate ---------------------------------------------------

#[test]
fn truncate_under_limit_passes_through() {
    let op = Truncate(10);
    let mut buf = String::new();
    let cont = op.apply("hi", &mut buf);
    assert_eq!(buf, "hi");
    assert!(cont, "should NOT short-circuit when input fits");
}

#[test]
fn truncate_over_limit_short_circuits() {
    let op = Truncate(5);
    let mut buf = String::new();
    let cont = op.apply("hello world", &mut buf);
    assert_eq!(buf, "hello");
    assert!(!cont, "should short-circuit when input exceeds limit");
}

#[test]
fn truncate_respects_scalar_boundary() {
    // "\u{00E9}" is 2 bytes; two copies is 4 bytes. Limit 3 must cut
    // back to the last scalar boundary at or before byte 3 — which is
    // byte 2, giving one "é" out.
    let op = Truncate(3);
    let mut buf = String::new();
    let cont = op.apply("\u{00E9}\u{00E9}", &mut buf);
    assert_eq!(buf, "\u{00E9}");
    assert!(!cont);
}

#[test]
fn truncate_zero_limit_yields_empty() {
    let op = Truncate(0);
    let mut buf = String::new();
    let cont = op.apply("hi", &mut buf);
    assert_eq!(buf, "");
    assert!(!cont);
}

#[test]
fn truncate_empty_input_passes_through() {
    let op = Truncate(5);
    let mut buf = String::new();
    let cont = op.apply("", &mut buf);
    assert_eq!(buf, "");
    assert!(cont, "empty input fits every non-zero limit");
}

// =====================================================================
// TextPipeline core
// =====================================================================

#[test]
fn empty_pipeline_is_identity() {
    assert_eq!(TextPipeline::new().apply("hello"), "hello");
    assert_eq!(TextPipeline::new().apply(""), "");
    assert_eq!(TextPipeline::new().apply("café"), "café");
}

#[test]
fn default_matches_new() {
    let a: TextPipeline = TextPipeline::default();
    let b = TextPipeline::new();
    assert_eq!(a.stages().len(), b.stages().len());
    assert_eq!(a.apply("hi"), b.apply("hi"));
}

#[test]
fn then_appends_stage() {
    let p = TextPipeline::new()
        .then(CollapseWhitespace)
        .then(CaseFold(CaseKind::Lower));
    assert_eq!(p.stages().len(), 2);
    assert_eq!(p.stages()[0].name(), "CollapseWhitespace");
    assert_eq!(p.stages()[1].name(), "CaseFold");
}

#[test]
fn stages_view_is_read_only() {
    let p = TextPipeline::new().then(Truncate(10));
    let stages = p.stages();
    assert_eq!(stages.len(), 1);
    assert_eq!(stages[0].name(), "Truncate");
}

#[test]
fn single_stage_matches_direct_apply() {
    let s = "  HELLO   WORLD  ";
    let via_pipeline = TextPipeline::new().then(CaseFold(CaseKind::Lower)).apply(s);
    let direct = apply_direct(&CaseFold(CaseKind::Lower), s);
    assert_eq!(via_pipeline, direct);
}

#[test]
fn two_stage_pipeline_trim_then_lower() {
    let p = TextPipeline::new()
        .then(Trim(trim_mod::Trim::whitespace()))
        .then(CaseFold(CaseKind::Lower));
    assert_eq!(p.apply("  HELLO  "), "hello");
}

#[test]
fn three_stage_pipeline_trim_collapse_lower() {
    let p = TextPipeline::new()
        .then(Trim(trim_mod::Trim::whitespace()))
        .then(CollapseWhitespace)
        .then(CaseFold(CaseKind::Lower));
    assert_eq!(p.apply("  Hello    WORLD  "), "hello world");
}

#[test]
fn four_stage_pipeline_normalize_trim_collapse_upper() {
    let p = TextPipeline::new()
        .then(Normalize(NormalizeKind::Quotes))
        .then(Trim(trim_mod::Trim::whitespace()))
        .then(CollapseWhitespace)
        .then(CaseFold(CaseKind::Upper));
    assert_eq!(
        p.apply("  \u{201C}hello    world\u{201D}  "),
        "\"HELLO WORLD\""
    );
}

#[test]
fn pipeline_reuses_across_inputs() {
    // The whole point of a pipeline value: build once, apply many.
    let p = TextPipeline::new()
        .then(Trim(trim_mod::Trim::whitespace()))
        .then(CaseFold(CaseKind::Upper));
    assert_eq!(p.apply("  hi  "), "HI");
    assert_eq!(p.apply("  bye  "), "BYE");
    assert_eq!(p.apply("no-edges"), "NO-EDGES");
}

#[test]
fn escape_after_replace() {
    let p = TextPipeline::new()
        .then(Replace {
            from: String::from("cat"),
            to: String::from("<cat>"),
        })
        .then(Escape(EscapeKind::Html));
    assert_eq!(p.apply("cat"), "&lt;cat&gt;");
}

// -------- Short-circuit tests ---------------------------------------

#[test]
fn truncate_short_circuits_downstream_stages() {
    // Truncate followed by an operation that would definitely change the
    // output (upper-case). The `false` return from Truncate must stop
    // the upstream case-fold from ever seeing the truncated prefix.
    let p = TextPipeline::new()
        .then(Truncate(5))
        .then(CaseFold(CaseKind::Upper));
    // If the case-fold had run, the result would be "HELLO". Because
    // Truncate short-circuited, the pipeline stops with "hello".
    assert_eq!(p.apply("hello world"), "hello");
}

#[test]
fn truncate_under_limit_does_not_short_circuit() {
    let p = TextPipeline::new()
        .then(Truncate(20))
        .then(CaseFold(CaseKind::Upper));
    assert_eq!(p.apply("hi"), "HI");
}

// -------- apply_into --------------------------------------------------

#[test]
fn apply_into_appends_to_existing_buffer() {
    let p = TextPipeline::new().then(CaseFold(CaseKind::Upper));
    let mut buf = String::from("greeting: ");
    p.apply_into("hello", &mut buf);
    assert_eq!(buf, "greeting: HELLO");
}

#[test]
fn apply_into_empty_pipeline_appends_input() {
    let p = TextPipeline::new();
    let mut buf = String::from("prefix:");
    p.apply_into("hi", &mut buf);
    assert_eq!(buf, "prefix:hi");
}

#[test]
fn apply_into_matches_apply_for_single_stage() {
    let s = "hello";
    let p = TextPipeline::new().then(CaseFold(CaseKind::Upper));
    let owned = p.apply(s);
    let mut buf = String::new();
    p.apply_into(s, &mut buf);
    assert_eq!(owned, buf);
}

#[test]
fn apply_into_matches_apply_for_multi_stage() {
    let s = "  Hello    WORLD  ";
    let p = TextPipeline::new()
        .then(Trim(trim_mod::Trim::whitespace()))
        .then(CollapseWhitespace)
        .then(CaseFold(CaseKind::Lower));
    let owned = p.apply(s);
    let mut buf = String::new();
    p.apply_into(s, &mut buf);
    assert_eq!(owned, buf);
}

#[test]
fn apply_into_short_circuit_still_appends() {
    let p = TextPipeline::new()
        .then(Truncate(5))
        .then(CaseFold(CaseKind::Upper));
    let mut buf = String::from("out: ");
    p.apply_into("hello world", &mut buf);
    assert_eq!(buf, "out: hello");
}

// -------- Debug -------------------------------------------------------

#[test]
fn empty_pipeline_debug() {
    let p = TextPipeline::new();
    let s = format!("{p:?}");
    assert!(s.contains("TextPipeline"), "{s}");
    // Empty list should show as [].
    assert!(s.contains("stages"), "{s}");
}

#[test]
fn pipeline_debug_lists_stage_names() {
    let p = TextPipeline::new()
        .then(CollapseWhitespace)
        .then(CaseFold(CaseKind::Lower))
        .then(Truncate(10));
    let s = format!("{p:?}");
    assert!(s.contains("CollapseWhitespace"), "{s}");
    assert!(s.contains("CaseFold"), "{s}");
    assert!(s.contains("Truncate"), "{s}");
}

#[test]
fn operation_debug_carries_configuration() {
    // Enum-tagged operations should surface their kind in Debug output.
    let s = format!("{:?}", Normalize(NormalizeKind::Whitespace));
    assert!(s.contains("Whitespace"), "{s}");

    let s = format!("{:?}", CaseFold(CaseKind::Title));
    assert!(s.contains("Title"), "{s}");

    let s = format!("{:?}", Escape(EscapeKind::Html));
    assert!(s.contains("Html"), "{s}");
}

// -------- Send / Sync smoke test ------------------------------------

#[test]
fn pipeline_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<TextPipeline>();
    assert_send_sync::<Box<dyn Operation>>();
}

// -------- Buffer reuse smoke test -----------------------------------

#[test]
fn apply_reuses_buffer_capacity_across_stages() {
    // We can't directly observe internal buffer allocations, but we can
    // verify that a pipeline that grows the intermediate does not panic
    // and returns the expected result — which exercises the ping-pong
    // capacity-preservation path.
    let p = TextPipeline::new()
        .then(Replace {
            from: String::from("a"),
            to: String::from("aa"),
        })
        .then(Replace {
            from: String::from("aa"),
            to: String::from("aaa"),
        });
    // Input "aaa" → after first stage: "aaaaaa" (each 'a' doubles) → after
    // second stage: each "aa" tripled to "aaa"; three "aa" pairs → nine
    // 'a's. Note: `str::replace` matches non-overlapping so 6 as
    // ("aa")("aa")("aa") — nine "a"s total.
    assert_eq!(p.apply("aaa"), "aaaaaaaaa");
}

// =====================================================================
// Property tests
// =====================================================================

#[cfg(all(feature = "std", not(target_family = "wasm")))]
mod properties {
    use super::*;
    use proptest::prelude::*;

    fn general_ascii() -> impl Strategy<Value = String> {
        prop::string::string_regex("[ -~]{0,32}").expect("static regex is valid")
    }

    fn general_unicode() -> impl Strategy<Value = String> {
        prop::string::string_regex("[\\u0000-\\u007F\\u00A0-\\u017F\\u2000-\\u200F]{0,32}")
            .expect("static regex is valid")
    }

    proptest! {
        // Empty pipeline is the identity transformation.
        #[test]
        fn empty_pipeline_is_identity(s in general_unicode()) {
            prop_assert_eq!(TextPipeline::new().apply(&s), s.clone());
        }

        // A pipeline of exactly one operation produces the same output
        // as calling that operation's `apply` directly. This is the
        // "pipeline composition preserves single-stage output" law: the
        // pipeline machinery adds no observable behavior when there is
        // only one op to run.
        #[test]
        fn single_stage_matches_operation(s in general_unicode()) {
            let via_pipeline = TextPipeline::new()
                .then(CaseFold(CaseKind::Lower))
                .apply(&s);
            let mut direct = String::new();
            CaseFold(CaseKind::Lower).apply(&s, &mut direct);
            prop_assert_eq!(via_pipeline, direct);
        }

        // Same law for CollapseWhitespace.
        #[test]
        fn single_stage_collapse_matches_operation(s in general_unicode()) {
            let via_pipeline = TextPipeline::new()
                .then(CollapseWhitespace)
                .apply(&s);
            let mut direct = String::new();
            CollapseWhitespace.apply(&s, &mut direct);
            prop_assert_eq!(via_pipeline, direct);
        }

        // `apply_into` into an empty buffer produces the same output as
        // the owned `apply` — the two entry points are equivalent modulo
        // buffer reuse.
        #[test]
        fn apply_and_apply_into_agree(s in general_unicode()) {
            let p = TextPipeline::new()
                .then(Trim(trim_mod::Trim::whitespace()))
                .then(CaseFold(CaseKind::Lower));
            let owned = p.apply(&s);
            let mut buf = String::new();
            p.apply_into(&s, &mut buf);
            prop_assert_eq!(owned, buf);
        }

        // Debug output is deterministic — printing a pipeline twice must
        // yield byte-identical strings.
        #[test]
        fn debug_is_stable(s in general_ascii()) {
            let p = TextPipeline::new()
                .then(CollapseWhitespace)
                .then(CaseFold(CaseKind::Lower))
                .then(Replace { from: s.clone(), to: String::from("*") });
            let a = format!("{p:?}");
            let b = format!("{p:?}");
            prop_assert_eq!(a, b);
        }

        // Truncate short-circuits: appending any number of operations
        // after a Truncate that fires does not change the output.
        #[test]
        fn truncate_short_circuit_is_terminal(
            s in "[a-z]{6,32}",
            limit in 0usize..5,
        ) {
            // `s` is at least 6 bytes; `limit` is at most 4. Truncate
            // will always fire.
            let base = TextPipeline::new().then(Truncate(limit));
            let extended = TextPipeline::new()
                .then(Truncate(limit))
                .then(CaseFold(CaseKind::Upper))
                .then(Replace { from: String::from("a"), to: String::from("X") });
            prop_assert_eq!(base.apply(&s), extended.apply(&s));
        }

        // If Truncate does NOT fire (input length ≤ limit), the
        // downstream stage does run.
        #[test]
        fn truncate_under_limit_does_not_short_circuit(s in "[a-z]{0,4}") {
            // Every input is ≤ 4 bytes; use limit 10.
            let p = TextPipeline::new()
                .then(Truncate(10))
                .then(CaseFold(CaseKind::Upper));
            prop_assert_eq!(p.apply(&s), s.to_uppercase());
        }

        // Applying the same pipeline to the same input twice yields the
        // same result (determinism).
        #[test]
        fn apply_is_deterministic(s in general_unicode()) {
            let p = TextPipeline::new()
                .then(Trim(trim_mod::Trim::whitespace()))
                .then(CollapseWhitespace)
                .then(CaseFold(CaseKind::Lower));
            prop_assert_eq!(p.apply(&s), p.apply(&s));
        }
    }
}
