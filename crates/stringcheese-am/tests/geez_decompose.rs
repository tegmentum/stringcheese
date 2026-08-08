//! Ge'ez syllable decompose / compose round-trip tests.
//!
//! Enumerates a broad sample of the main Ge'ez block
//! (U+1200..=U+137F) and verifies that:
//!
//! 1. Every main-block scalar decomposes to `Some((base, order))`
//!    where `base` sits at a family-head offset (offset % 8 == 0)
//!    and `order` is in the 0..=7 range.
//! 2. `compose(base, order)` reconstitutes the original scalar.
//! 3. Every family-head scalar decomposes to itself with order 0.
//! 4. Scalars outside the main block (supplement, extended, Latin,
//!    Hebrew, Arabic) all return `None`.

use stringcheese_am::geez::{compose, decompose, is_geez_main};

const MAIN_START: u32 = 0x1200;
const MAIN_END: u32 = 0x137F;
const COLUMNS_PER_FAMILY: u32 = 8;

#[test]
fn every_main_block_scalar_round_trips() {
    // Enumerate every code point in the main Ge'ez block.
    let mut failures = 0;
    let mut count = 0;
    for cp in MAIN_START..=MAIN_END {
        let c = char::from_u32(cp).expect("main-block cp is a valid scalar");
        count += 1;

        let Some((base, order)) = decompose(c) else {
            failures += 1;
            eprintln!("decompose({c:?}) returned None for U+{cp:04X}");
            continue;
        };

        // The base must sit at a family-head offset.
        let base_cp = base as u32;
        assert!(
            is_geez_main(base),
            "base of {c:?} = {base:?} not in main block"
        );
        assert_eq!(
            (base_cp - MAIN_START) % COLUMNS_PER_FAMILY,
            0,
            "base of {c:?} = U+{base_cp:04X} not at a family-head offset"
        );
        assert!(
            u32::from(order) < COLUMNS_PER_FAMILY,
            "order of {c:?} = {order} out of range"
        );

        // Round-trip: compose(base, order) == c.
        let back = compose(base, order);
        assert_eq!(back, Some(c), "compose round-trip failed for {c:?}");
    }
    assert_eq!(
        failures, 0,
        "{failures} of {count} main-block scalars failed to decompose"
    );
    assert_eq!(count, 384, "expected 384 scalars in the main block");
}

#[test]
fn every_family_head_decomposes_to_self_order_zero() {
    // 48 rows × 8 columns = 384 scalars in the main block, so the
    // family heads sit at offsets 0, 8, 16, ..., 376 (48 total).
    for family in 0u32..48 {
        let cp = MAIN_START + family * COLUMNS_PER_FAMILY;
        let c = char::from_u32(cp).unwrap();
        let (base, order) = decompose(c).unwrap();
        assert_eq!(base, c, "family {family}: head should decompose to itself");
        assert_eq!(order, 0, "family {family}: head should have order 0");
    }
}

#[test]
fn ha_family_all_seven_vowel_orders() {
    // The h family at U+1200 covers orders 0..=6 (canonical vowels).
    // Order 7 is the labialized column and is often unassigned.
    let syllables = [
        ('ሀ', 0, 'ሀ'), // hä (order 0)
        ('ሁ', 1, 'ሀ'), // hu (order 1)
        ('ሂ', 2, 'ሀ'), // hi (order 2)
        ('ሃ', 3, 'ሀ'), // ha (order 3)
        ('ሄ', 4, 'ሀ'), // he (order 4)
        ('ህ', 5, 'ሀ'), // hɨ (order 5)
        ('ሆ', 6, 'ሀ'), // ho (order 6)
    ];
    for (scalar, expected_order, expected_base) in syllables {
        let (base, order) = decompose(scalar).unwrap();
        assert_eq!(order, expected_order, "order of {scalar:?}");
        assert_eq!(base, expected_base, "base of {scalar:?}");
        let back = compose(base, order).unwrap();
        assert_eq!(back, scalar, "round-trip for {scalar:?}");
    }
}

#[test]
fn m_family_all_seven_vowel_orders() {
    // The m family at U+1218 covers orders 0..=6.
    let syllables = [
        ('መ', 0, 'መ'), // mä
        ('ሙ', 1, 'መ'), // mu
        ('ሚ', 2, 'መ'), // mi
        ('ማ', 3, 'መ'), // ma
        ('ሜ', 4, 'መ'), // me
        ('ም', 5, 'መ'), // mɨ
        ('ሞ', 6, 'መ'), // mo
    ];
    for (scalar, expected_order, expected_base) in syllables {
        let (base, order) = decompose(scalar).unwrap();
        assert_eq!(order, expected_order, "order of {scalar:?}");
        assert_eq!(base, expected_base, "base of {scalar:?}");
    }
}

#[test]
fn glottal_family_covers_the_amharic_e_vowels() {
    // The ' family at U+12A0 has the eight glottal-plus-vowel forms.
    // These are the vowels that "start with a vowel" in Amharic
    // orthography (Amharic writes independent vowels as glottal +
    // vowel).
    let (base, order) = decompose('አ').unwrap();
    assert_eq!(base, 'አ');
    assert_eq!(order, 0);
    let (base, order) = decompose('ኡ').unwrap();
    assert_eq!(base, 'አ');
    assert_eq!(order, 1);
    let (base, order) = decompose('ኢ').unwrap();
    assert_eq!(base, 'አ');
    assert_eq!(order, 2);
    let (base, order) = decompose('ኦ').unwrap();
    assert_eq!(base, 'አ');
    assert_eq!(order, 6);
}

#[test]
fn non_geez_scalars_return_none() {
    // Latin.
    assert_eq!(decompose('A'), None);
    assert_eq!(decompose('z'), None);
    // ASCII digits.
    assert_eq!(decompose('7'), None);
    // Space and punctuation.
    assert_eq!(decompose(' '), None);
    assert_eq!(decompose(','), None);
    // Hebrew.
    assert_eq!(decompose('א'), None);
    // Arabic.
    assert_eq!(decompose('ا'), None);
    // Bengali.
    assert_eq!(decompose('ক'), None);
    // Devanagari.
    assert_eq!(decompose('क'), None);
}

#[test]
fn supplement_and_extended_return_none() {
    // Supplement U+1380..=U+139F — labialized-u/-i variants and
    // Ethiopic tonal marks. Not covered by the 8-column layout.
    for cp in 0x1380u32..=0x139Fu32 {
        let c = char::from_u32(cp).unwrap();
        assert_eq!(
            decompose(c),
            None,
            "supplement scalar {c:?} (U+{cp:04X}) should return None"
        );
    }
    // Extended U+2D80..=U+2DDF.
    for cp in 0x2D80u32..=0x2DDFu32 {
        let c = char::from_u32(cp).unwrap();
        assert_eq!(
            decompose(c),
            None,
            "extended scalar {c:?} (U+{cp:04X}) should return None"
        );
    }
}

#[test]
fn compose_rejects_out_of_range_order() {
    assert_eq!(compose('ሀ', 8), None);
    assert_eq!(compose('ሀ', 100), None);
    assert_eq!(compose('ሀ', 255), None);
}

#[test]
fn compose_rejects_non_family_head_base() {
    // ሁ is the u-form of the h family, not the family head.
    assert_eq!(compose('ሁ', 0), None);
    assert_eq!(compose('ሆ', 3), None);
}

#[test]
fn compose_rejects_non_geez_base() {
    assert_eq!(compose('A', 0), None);
    assert_eq!(compose('א', 0), None);
    assert_eq!(compose('ك', 0), None);
}

#[test]
fn sample_families_across_the_block() {
    // Sanity-check the family heads at various offsets across the
    // main block. Each of these is at offset family * 8 from U+1200.
    let samples = [
        (0u32, 'ሀ'), // U+1200 h family
        (1, 'ለ'),    // U+1208 l family
        (3, 'መ'),    // U+1218 m family
        (5, 'ረ'),    // U+1228 r family
        (6, 'ሰ'),    // U+1230 s family
        (12, 'በ'),   // U+1260 b family
        (14, 'ተ'),   // U+1270 t family
        (18, 'ነ'),   // U+1290 n family
        (20, 'አ'),   // U+12A0 glottal family
        (21, 'ከ'),   // U+12A8 k family
        (25, 'ወ'),   // U+12C8 w family
        (27, 'ዘ'),   // U+12D8 z family
        (29, 'የ'),   // U+12E8 y family
        (30, 'ደ'),   // U+12F0 d family
        (32, 'ጀ'),   // U+1300 j family
        (33, 'ገ'),   // U+1308 g family
        (36, 'ጠ'),   // U+1320 emphatic t
        (41, 'ፈ'),   // U+1348 f family
        (42, 'ፐ'),   // U+1350 p family
    ];
    for (family, head) in samples {
        let cp = 0x1200 + family * 8;
        let c = char::from_u32(cp).unwrap();
        assert_eq!(c, head, "family {family}: U+{cp:04X} should be {head:?}");
        let (base, order) = decompose(c).unwrap();
        assert_eq!(base, head, "decompose base of family {family}");
        assert_eq!(order, 0, "decompose order of family {family}");
    }
}
