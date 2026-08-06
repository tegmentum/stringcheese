//! Tests for [`crate::slice`].
//!
//! Unit tests exercise the empty / ASCII / multi-byte-scalar /
//! multi-scalar-grapheme edge cases across every function, plus the
//! boundary-splitting risks unique to [`slice_bytes`] (non-boundary
//! endpoints must produce `None`). Property tests confirm the
//! identity, monotonicity, and roundtrip laws that hold across all
//! inputs.

use super::*;

// -----------------------------------------------------------------
// slice_bytes
// -----------------------------------------------------------------

#[test]
fn slice_bytes_empty() {
    assert_eq!(slice_bytes("", 0..0), Some(""));
    assert_eq!(slice_bytes("", ..), Some(""));
    // Out of bounds is None.
    assert_eq!(slice_bytes("", 0..1), None);
}

#[test]
fn slice_bytes_ascii_full_range() {
    let s = "hello";
    assert_eq!(slice_bytes(s, 0..s.len()), Some(s));
    assert_eq!(slice_bytes(s, ..), Some(s));
}

#[test]
fn slice_bytes_ascii_sub_range() {
    assert_eq!(slice_bytes("hello", 1..4), Some("ell"));
    assert_eq!(slice_bytes("hello", ..3), Some("hel"));
    assert_eq!(slice_bytes("hello", 2..), Some("llo"));
}

#[test]
fn slice_bytes_multi_byte_scalar_on_boundary() {
    let s = "café"; // c a f é(2 bytes) — 5 bytes total
    assert_eq!(slice_bytes(s, 0..3), Some("caf"));
    assert_eq!(slice_bytes(s, 3..5), Some("é"));
    assert_eq!(slice_bytes(s, 3..), Some("é"));
}

#[test]
fn slice_bytes_multi_byte_scalar_mid_scalar_is_none() {
    let s = "café";
    // Byte offset 4 lands inside "é".
    assert_eq!(slice_bytes(s, 0..4), None);
    assert_eq!(slice_bytes(s, 4..5), None);
}

#[test]
fn slice_bytes_out_of_bounds_is_none() {
    assert_eq!(slice_bytes("hi", 0..100), None);
    assert_eq!(slice_bytes("hi", 100..200), None);
}

#[test]
fn slice_bytes_inclusive_range() {
    // Sanity: inclusive end acts like exclusive-plus-one.
    assert_eq!(slice_bytes("hello", 0..=2), Some("hel"));
}

// -----------------------------------------------------------------
// take_bytes / drop_bytes
// -----------------------------------------------------------------

#[test]
fn take_bytes_within_and_beyond() {
    assert_eq!(take_bytes("hello", 0), "");
    assert_eq!(take_bytes("hello", 3), "hel");
    assert_eq!(take_bytes("hello", 5), "hello");
    // Clamps when out of range rather than panicking.
    assert_eq!(take_bytes("hello", 100), "hello");
}

#[test]
fn drop_bytes_within_and_beyond() {
    assert_eq!(drop_bytes("hello", 0), "hello");
    assert_eq!(drop_bytes("hello", 3), "lo");
    assert_eq!(drop_bytes("hello", 5), "");
    // Clamps to end-of-string.
    assert_eq!(drop_bytes("hello", 100), "");
}

#[test]
fn take_and_drop_bytes_split_multi_byte_on_boundary() {
    assert_eq!(take_bytes("café", 3), "caf");
    assert_eq!(drop_bytes("café", 3), "é");
}

#[test]
#[should_panic(expected = "byte index 4 is not a char boundary")]
fn take_bytes_mid_scalar_panics() {
    // Byte offset 4 is mid-"é"; slicing there would produce invalid
    // UTF-8, so the operation panics — the documented boundary
    // contract.
    let _ = take_bytes("café", 4);
}

// -----------------------------------------------------------------
// slice_scalars
// -----------------------------------------------------------------

#[cfg(feature = "alloc")]
mod scalar_and_grapheme {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn slice_scalars_empty() {
        assert_eq!(slice_scalars("", 0..0), "");
        assert_eq!(slice_scalars("", ..), "");
    }

    #[test]
    fn slice_scalars_ascii() {
        assert_eq!(slice_scalars("hello", 0..3), "hel");
        assert_eq!(slice_scalars("hello", 1..4), "ell");
        assert_eq!(slice_scalars("hello", ..), "hello");
    }

    #[test]
    fn slice_scalars_multi_byte() {
        // Each accented letter is one scalar even though it is two bytes.
        assert_eq!(slice_scalars("café", 0..3), "caf");
        assert_eq!(slice_scalars("café", 3..4), "é");
    }

    #[test]
    fn slice_scalars_over_end_truncates() {
        assert_eq!(slice_scalars("hi", 0..100), "hi");
        assert_eq!(slice_scalars("hi", 100..200), "");
    }

    #[test]
    fn slice_scalars_full_range_identity() {
        assert_eq!(slice_scalars("café", ..), "café");
    }

    #[test]
    fn take_scalars_basic() {
        assert_eq!(take_scalars("hello", 3), "hel");
        assert_eq!(take_scalars("café", 3), "caf");
        assert_eq!(take_scalars("café", 4), "café");
    }

    #[test]
    fn take_scalars_beyond_end_returns_whole() {
        assert_eq!(take_scalars("hi", 100), "hi");
    }

    #[test]
    fn drop_scalars_basic() {
        assert_eq!(drop_scalars("hello", 3), "lo");
        assert_eq!(drop_scalars("café", 3), "é");
    }

    #[test]
    fn drop_scalars_beyond_end_returns_empty() {
        assert_eq!(drop_scalars("hi", 100), "");
    }

    #[test]
    fn take_scalars_on_decomposed_grapheme_may_split_it() {
        // The decomposed é is two scalars: 'e' and the combining acute.
        // Taking one scalar keeps only the 'e', splitting the grapheme.
        // Documented boundary behavior — see `take_graphemes` for
        // grapheme-safe extraction.
        assert_eq!(take_scalars("e\u{0301}bc", 1), "e");
    }

    // -----------------------------------------------------------------
    // slice_graphemes / take_graphemes / drop_graphemes
    // -----------------------------------------------------------------

    #[test]
    fn slice_graphemes_empty() {
        assert_eq!(slice_graphemes("", 0..0), "");
        assert_eq!(slice_graphemes("", ..), "");
    }

    #[test]
    fn slice_graphemes_ascii() {
        assert_eq!(slice_graphemes("hello", 0..3), "hel");
    }

    #[test]
    fn slice_graphemes_decomposed_grapheme_intact() {
        // Slicing at grapheme boundaries preserves the decomposed é intact.
        assert_eq!(slice_graphemes("cafe\u{0301}", 0..4), "cafe\u{0301}");
        assert_eq!(slice_graphemes("cafe\u{0301}xy", 4..), "xy");
    }

    #[test]
    fn slice_graphemes_flag_is_one_unit() {
        assert_eq!(
            slice_graphemes("\u{1F1EC}\u{1F1E7}!", 0..1),
            "\u{1F1EC}\u{1F1E7}"
        );
        assert_eq!(slice_graphemes("\u{1F1EC}\u{1F1E7}!", 1..), "!");
    }

    #[test]
    fn slice_graphemes_over_end_truncates() {
        assert_eq!(slice_graphemes("hi", 0..100), "hi");
        assert_eq!(slice_graphemes("hi", 100..200), "");
    }

    #[test]
    fn take_graphemes_basic() {
        assert_eq!(take_graphemes("hello", 3), "hel");
        // A decomposed grapheme is kept whole.
        assert_eq!(take_graphemes("e\u{0301}bc", 1), "e\u{0301}");
    }

    #[test]
    fn take_graphemes_beyond_end_returns_whole() {
        assert_eq!(take_graphemes("hi", 100), "hi");
    }

    #[test]
    fn drop_graphemes_basic() {
        assert_eq!(drop_graphemes("hello", 3), "lo");
        // Dropping the decomposed é as one grapheme.
        assert_eq!(drop_graphemes("e\u{0301}bc", 1), "bc");
    }

    #[test]
    fn drop_graphemes_beyond_end_returns_empty() {
        assert_eq!(drop_graphemes("hi", 100), "");
    }

    #[test]
    fn take_plus_drop_reconstructs_input_ascii() {
        let s = "abcdef";
        for n in 0..=s.chars().count() {
            let head: String = take_scalars(s, n).to_string();
            let tail: String = drop_scalars(s, n).to_string();
            assert_eq!(head + &tail, s, "n={n}");
        }
    }

    #[test]
    fn take_plus_drop_reconstructs_input_multi_byte() {
        let s = "café";
        for n in 0..=s.chars().count() {
            let head: String = take_scalars(s, n).to_string();
            let tail: String = drop_scalars(s, n).to_string();
            assert_eq!(head + &tail, s, "n={n}");
        }
    }
}

// -----------------------------------------------------------------
// Property tests
// -----------------------------------------------------------------

#[cfg(all(feature = "std", not(target_family = "wasm")))]
mod properties {
    use super::*;
    use proptest::prelude::*;

    fn general_unicode() -> impl Strategy<Value = String> {
        prop::string::string_regex("[\\u0000-\\u007F\\u00A0-\\u017F\\u2000-\\u200F]{0,32}")
            .expect("static regex is valid")
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        // Full-range slice_bytes recovers the input.
        #[test]
        fn slice_bytes_full_range_identity(s in general_unicode()) {
            prop_assert_eq!(slice_bytes(&s, 0..s.len()), Some(s.as_str()));
            prop_assert_eq!(slice_bytes(&s, ..), Some(s.as_str()));
        }

        // slice_bytes at char boundaries agrees with plain slicing.
        #[test]
        fn slice_bytes_at_char_boundaries_agrees_with_indexing(s in general_unicode()) {
            let mut boundaries: Vec<usize> = s.char_indices().map(|(i, _)| i).collect();
            boundaries.push(s.len());
            for (i, &a) in boundaries.iter().enumerate() {
                for &b in &boundaries[i..] {
                    prop_assert_eq!(slice_bytes(&s, a..b), Some(&s[a..b]));
                }
            }
        }

        // Full-range slice_scalars is identity.
        #[test]
        fn slice_scalars_full_range_identity(s in general_unicode()) {
            prop_assert_eq!(slice_scalars(&s, ..), s.clone());
        }

        // take_scalars(s, n) + drop_scalars(s, n) reconstructs s.
        #[test]
        fn scalars_take_drop_reconstruct(s in general_unicode(), n in 0usize..64) {
            let head = take_scalars(&s, n);
            let tail = drop_scalars(&s, n);
            let joined = String::from(head) + tail;
            prop_assert_eq!(joined, s);
        }

        // take_bytes(s, n) length is <= n, and <= s.len().
        #[test]
        fn take_bytes_bounded_length(s in general_unicode(), n in 0usize..128) {
            // Only exercise char-boundary offsets to avoid the documented
            // panic-on-mid-scalar.
            let boundary_n = if s.is_char_boundary(n.min(s.len())) {
                n.min(s.len())
            } else {
                s.len()
            };
            let out = take_bytes(&s, boundary_n);
            prop_assert!(out.len() <= boundary_n);
            prop_assert!(out.len() <= s.len());
        }
    }
}
