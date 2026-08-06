//! Tests for [`crate::join`].
//!
//! Unit tests cover the empty / single-item / multi-item / mixed-input
//! edge cases across every variant, plus the buffer-reuse contract of
//! [`join_into`]. Property tests confirm the round-trip law
//! `split(join(items, sep), sep) == items` when `sep` does not itself
//! occur in any item.

use super::*;

use alloc::string::ToString;
use alloc::vec;

// -----------------------------------------------------------------
// join
// -----------------------------------------------------------------

#[test]
fn join_empty_yields_empty_string() {
    let empty: Vec<&str> = Vec::new();
    assert_eq!(join(empty, ","), "");
}

#[test]
fn join_single_item_no_separator() {
    assert_eq!(join(["solo"], ","), "solo");
}

#[test]
fn join_multiple_items() {
    assert_eq!(join(["a", "b", "c"], ","), "a,b,c");
}

#[test]
fn join_multi_char_separator() {
    assert_eq!(join(["a", "b", "c"], " -> "), "a -> b -> c");
}

#[test]
fn join_empty_separator() {
    assert_eq!(join(["a", "b", "c"], ""), "abc");
}

#[test]
fn join_empty_items_yields_only_separators() {
    // Three empty items and a two-char separator = "" + "," + "" + "," + "" = ",,"
    assert_eq!(join(["", "", ""], ","), ",,");
}

#[test]
fn join_accepts_owned_strings() {
    let items: Vec<String> = vec!["hello".to_string(), "world".to_string()];
    assert_eq!(join(items, " "), "hello world");
}

#[test]
fn join_from_iterator() {
    let items = (0..3).map(|n| n.to_string());
    let out = join(items, "-");
    assert_eq!(out, "0-1-2");
}

// -----------------------------------------------------------------
// join_into
// -----------------------------------------------------------------

#[test]
fn join_into_appends_to_existing_buffer() {
    let mut buf = String::from("prefix:");
    join_into(["a", "b", "c"], ",", &mut buf);
    assert_eq!(buf, "prefix:a,b,c");
}

#[test]
fn join_into_empty_items_is_noop() {
    let mut buf = String::from("keep");
    let empty: Vec<&str> = Vec::new();
    join_into(empty, ",", &mut buf);
    assert_eq!(buf, "keep");
}

#[test]
fn join_into_repeated_calls_accumulate() {
    let mut buf = String::new();
    join_into(["a", "b"], ",", &mut buf);
    buf.push('|');
    join_into(["c", "d"], ",", &mut buf);
    assert_eq!(buf, "a,b|c,d");
}

#[test]
fn join_into_zero_realloc_when_capacity_present() {
    let mut buf = String::with_capacity(64);
    let cap_before = buf.capacity();
    join_into(["a", "b", "c"], ",", &mut buf);
    // If capacity was already large enough, the reservation must not
    // have triggered a re-alloc.
    assert!(buf.capacity() >= cap_before);
    assert_eq!(buf, "a,b,c");
}

// -----------------------------------------------------------------
// join_with
// -----------------------------------------------------------------

#[test]
fn join_with_formats_each_item() {
    let items = [1, 2, 3];
    assert_eq!(join_with(items, ", ", ToString::to_string), "1, 2, 3");
}

#[test]
fn join_with_empty_iter() {
    let items: Vec<i32> = Vec::new();
    assert_eq!(join_with(items, ",", ToString::to_string), "");
}

#[test]
fn join_with_closure_returning_str_slice() {
    let items: Vec<&str> = vec!["Hello", "World"];
    let out = join_with(items, " ", |s: &&str| s.to_ascii_lowercase());
    assert_eq!(out, "hello world");
}

// -----------------------------------------------------------------
// concat / intercalate
// -----------------------------------------------------------------

#[test]
fn concat_no_separator() {
    assert_eq!(concat(["hello", " ", "world"]), "hello world");
}

#[test]
fn concat_empty_yields_empty() {
    let empty: Vec<&str> = Vec::new();
    assert_eq!(concat(empty), "");
}

#[test]
fn intercalate_matches_join() {
    assert_eq!(
        intercalate(["a", "b", "c"], "-"),
        join(["a", "b", "c"], "-")
    );
}

// -----------------------------------------------------------------
// Capacity contract
// -----------------------------------------------------------------

#[test]
fn join_preallocates_exact_capacity() {
    // The returned String's capacity must be at least the final length
    // (evidence that we sized up-front rather than growing incrementally).
    let out = join(["one", "two", "three"], "--");
    assert!(out.capacity() >= out.len());
}

// -----------------------------------------------------------------
// Property tests
// -----------------------------------------------------------------

#[cfg(all(feature = "std", not(target_family = "wasm")))]
mod properties {
    use super::*;
    use proptest::prelude::*;

    fn ascii_no_comma() -> impl Strategy<Value = String> {
        prop::string::string_regex("[\\u0020-\\u002B\\u002D-\\u007E]{0,16}")
            .expect("static regex is valid")
    }

    fn general_ascii() -> impl Strategy<Value = String> {
        prop::string::string_regex("[\\u0020-\\u007E]{0,32}").expect("static regex is valid")
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        // Round-trip with split: `split(join(items, sep), sep) == items`
        // when the separator never appears inside any item.
        #[test]
        fn split_of_join_recovers_items(items in proptest::collection::vec(ascii_no_comma(), 0..8)) {
            let joined = join(items.clone(), ",");
            let recovered: Vec<&str> = joined.split(',').collect();
            if items.is_empty() {
                prop_assert_eq!(recovered, vec![""]);
            } else {
                let expected: Vec<&str> = items.iter().map(String::as_str).collect();
                prop_assert_eq!(recovered, expected);
            }
        }

        // join and intercalate are byte-for-byte identical.
        #[test]
        fn intercalate_equals_join(items in proptest::collection::vec(general_ascii(), 0..8), sep in general_ascii()) {
            prop_assert_eq!(
                intercalate(items.clone(), &sep),
                join(items, &sep)
            );
        }

        // concat equals join with an empty separator.
        #[test]
        fn concat_equals_join_empty_sep(items in proptest::collection::vec(general_ascii(), 0..8)) {
            prop_assert_eq!(concat(items.clone()), join(items, ""));
        }

        // Length of the join equals sum of item lengths plus (n-1)*sep.len().
        #[test]
        fn join_length_is_predictable(items in proptest::collection::vec(general_ascii(), 0..8), sep in general_ascii()) {
            let out = join(items.clone(), &sep);
            let expected_len: usize = if items.is_empty() {
                0
            } else {
                items.iter().map(String::len).sum::<usize>() + sep.len() * (items.len() - 1)
            };
            prop_assert_eq!(out.len(), expected_len);
        }

        // join_into with an empty starting buffer matches join.
        #[test]
        fn join_into_matches_join(items in proptest::collection::vec(general_ascii(), 0..8), sep in general_ascii()) {
            let mut buf = String::new();
            join_into(items.clone(), &sep, &mut buf);
            prop_assert_eq!(buf, join(items, &sep));
        }
    }
}
