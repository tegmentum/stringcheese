//! Ge'ez syllable decompose / compose helpers.
//!
//! # Ge'ez script structure (the abugida / syllabary layout)
//!
//! Ge'ez (also called *Ethiopic*) is an **abugida** — each character
//! represents a **consonant + vowel** combination. Unlike Devanagari
//! or Bengali, where a base consonant plus a dependent-vowel matra
//! form two scalars, Ge'ez packs each consonant + vowel *pair* into
//! **one scalar**. So while `कि` (Devanagari "ki") is two scalars
//! (`क` + `ि`), Amharic `ኪ` (Ge'ez "ki") is a single scalar.
//!
//! There are **7 canonical vowel orders** (called *የግዕዝ ፊደላት*
//! "orders" or *ምዕራፍ* "columns"), plus an eighth column often used
//! for *labialized* forms:
//!
//! | Order | Amharic name | Vowel | Example (from base ሀ = h) |
//! |-------|--------------|-------|-----------------------------|
//! | 0     | ግዕዝ         | ə     | ሀ  (hä / hə)                |
//! | 1     | ካዕብ         | u     | ሁ  (hu)                     |
//! | 2     | ሣልስ         | i     | ሂ  (hi)                     |
//! | 3     | ራብዕ         | a     | ሃ  (ha)                     |
//! | 4     | ሓምስ         | e     | ሄ  (he)                     |
//! | 5     | ሳድስ         | ɨ / ∅ | ህ  (hɨ — often "no vowel")  |
//! | 6     | ሳብዕ         | o     | ሆ  (ho)                     |
//! | 7     | (labialized) | wa/oa | ሇ  (variable / often absent) |
//!
//! Column 5 (ሳድስ) is the **6th order** — historically a short high
//! central vowel /ɨ/ that in Amharic often reduces to no vowel at
//! all. Column 7 (labialized) is present for some consonant
//! families (e.g. `ኰ` = kʷä, `ጐ` = gʷä) and reserved / absent for
//! others.
//!
//! # Unicode layout
//!
//! The **main Ge'ez block** occupies **U+1200..=U+137F** (384 scalars
//! = 48 rows × 8 columns). Each row is a *consonant family*: all
//! eight columns for that family sit at consecutive code points.
//! For example, the `h` family (row 0):
//!
//! | Scalar | Codepoint | Order | Amharic | Rom. |
//! |--------|-----------|-------|---------|------|
//! | ሀ      | U+1200    | 0     | hä      | he   |
//! | ሁ      | U+1201    | 1     | hu      | hu   |
//! | ሂ      | U+1202    | 2     | hi      | hi   |
//! | ሃ      | U+1203    | 3     | ha      | ha   |
//! | ሄ      | U+1204    | 4     | he      | hie  |
//! | ህ      | U+1205    | 5     | hɨ      | h    |
//! | ሆ      | U+1206    | 6     | ho      | ho   |
//! | ሇ      | U+1207    | 7     | hoa     | hoa  |
//!
//! The **algorithm** is therefore purely arithmetic on the scalar's
//! offset from the block base:
//!
//! ```text
//!   offset  = scalar - 0x1200
//!   family  = offset / 8       // 0..=47 for the main block
//!   column  = offset % 8       // 0..=7, the vowel order
//!   base    = 0x1200 + family * 8   // the ə-form (order 0) scalar
//! ```
//!
//! The inverse (compose) checks that `base` is itself a family head
//! (offset % 8 == 0) and that `order < 8`, then returns `base + order`.
//!
//! # Limitations — what this module intentionally does *not* handle
//!
//! * **Supplement block U+1380..=U+139F.** The supplement holds
//!   labialized-*u*/-*i* variants (ኈ ኌ etc.) and Ethiopic tonal
//!   marks. Its layout is **not** the 8-column-per-family grid —
//!   it's a mixed block of extension letters. This module returns
//!   `None` for scalars in the supplement.
//! * **Extended block U+2D80..=U+2DDF.** Extended-Ethiopic covers
//!   additional consonants for Sebat Bet Gurage, Blin, Meʼen, and
//!   other languages. Again, non-8-column layout — this module
//!   returns `None`.
//! * **Extended-A block U+AB00..=U+AB2F.** Extended-A adds more
//!   Amharic and Gurage variants; not covered.
//! * **Reserved slots.** Even within the main U+1200..=U+137F range,
//!   several column-7 slots are **unassigned** in Unicode — the
//!   arithmetic still produces `Some((base, 7))` for them because
//!   the scalar is otherwise well-formed (`char::from_u32` would
//!   accept them). Callers who need to guard against reserved
//!   slots should consult the Unicode data files themselves; the
//!   `char::from_u32(base + order as u32)` pass in the round-trip
//!   test below happens to accept the reserved code points because
//!   they are within the main block.
//!
//! # RTL / LTR note
//!
//! Ge'ez is written **left-to-right** — no RTL surprises here (this
//! is the *first Ge'ez-script pack* but nothing about the tokenizer
//! or stemmer needs bidi machinery). Every scalar in the main block
//! is **3 bytes in UTF-8** (U+1200..=U+137F falls in UTF-8's 3-byte
//! range U+0800..=U+FFFF), so this crate's tokenizer and stemmer
//! walk `Vec<char>` rather than raw byte offsets — same rule as the
//! Bengali and Devanagari packs.

// -----------------------------------------------------------------------
// Block constants.
// -----------------------------------------------------------------------

/// First scalar of the main Ge'ez / Ethiopic block, U+1200.
pub const GEEZ_MAIN_START: u32 = 0x1200;

/// Last scalar of the main Ge'ez / Ethiopic block, U+137F.
///
/// The main block runs U+1200..=U+137F (384 scalars = 48 rows × 8
/// columns). Beyond this range the supplement (U+1380..=U+139F) and
/// the extended block (U+2D80..=U+2DDF) live, both with non-8-column
/// layouts — this module returns `None` for scalars outside the main
/// range.
pub const GEEZ_MAIN_END: u32 = 0x137F;

/// The number of columns per consonant family in the main block —
/// **8** (7 canonical vowel orders + a labialized eighth column).
pub const COLUMNS_PER_FAMILY: u32 = 8;

/// The vowel order index of the base (ə / ግዕዝ) form — column 0.
pub const ORDER_E: u8 = 0;

/// The vowel order index of the *u* / ካዕብ form — column 1.
pub const ORDER_U: u8 = 1;

/// The vowel order index of the *i* / ሣልስ form — column 2.
pub const ORDER_I: u8 = 2;

/// The vowel order index of the *a* / ራብዕ form — column 3.
pub const ORDER_A: u8 = 3;

/// The vowel order index of the *e* / ሓምስ form — column 4.
pub const ORDER_LONG_E: u8 = 4;

/// The vowel order index of the *ɨ* / ሳድስ form (the 6th order —
/// often silent / no vowel) — column 5.
pub const ORDER_SILENT: u8 = 5;

/// The vowel order index of the *o* / ሳብዕ form — column 6.
pub const ORDER_O: u8 = 6;

/// The vowel order index of the labialized form (*wa* / *oa*) —
/// column 7. Present for some consonant families, reserved / absent
/// for others.
pub const ORDER_WA: u8 = 7;

// -----------------------------------------------------------------------
// Decompose / compose.
// -----------------------------------------------------------------------

/// Decompose a Ge'ez scalar into its `(consonant_base, vowel_order)`
/// pair.
///
/// Returns `Some((base, order))` where `base` is the *ə*-form (order 0)
/// of the same consonant family and `order` is the column index
/// (0..=7), or `None` if `c` is outside the main Ge'ez block
/// U+1200..=U+137F.
///
/// The algorithm is purely arithmetic on the scalar's offset from
/// the block base — see the [module-level docs](self#unicode-layout)
/// for the full derivation.
///
/// # Examples
///
/// ```
/// use stringcheese_am::geez::decompose;
///
/// // ሀ (U+1200) — base form of the `h` family; order 0.
/// assert_eq!(decompose('ሀ'), Some(('ሀ', 0)));
/// // ሁ (U+1201) — `h` + u, order 1.
/// assert_eq!(decompose('ሁ'), Some(('ሀ', 1)));
/// // ሆ (U+1206) — `h` + o, order 6.
/// assert_eq!(decompose('ሆ'), Some(('ሀ', 6)));
/// // መ (U+1218) — base form of the `m` family; order 0.
/// assert_eq!(decompose('መ'), Some(('መ', 0)));
/// // Non-Ge'ez scalar returns `None`.
/// assert_eq!(decompose('A'), None);
/// ```
#[must_use]
pub fn decompose(c: char) -> Option<(char, u8)> {
    let cp = c as u32;
    if !(GEEZ_MAIN_START..=GEEZ_MAIN_END).contains(&cp) {
        return None;
    }
    let offset = cp - GEEZ_MAIN_START;
    let family = offset / COLUMNS_PER_FAMILY;
    let column = (offset % COLUMNS_PER_FAMILY) as u8;
    let base_cp = GEEZ_MAIN_START + family * COLUMNS_PER_FAMILY;
    // `char::from_u32` cannot fail here: base_cp lies within
    // U+1200..=U+1378, which is a valid Unicode scalar range.
    let base = char::from_u32(base_cp)?;
    Some((base, column))
}

/// Compose a `(consonant_base, vowel_order)` pair back into a single
/// Ge'ez scalar.
///
/// Returns `Some(scalar)` if `base` is a valid family-head (order 0)
/// scalar in the main Ge'ez block and `order` is in the 0..=7 range,
/// or `None` otherwise (base not in the block, base not a family
/// head, or order >= 8).
///
/// # Examples
///
/// ```
/// use stringcheese_am::geez::compose;
///
/// // `h` family (base ሀ) + order 1 (u) → ሁ.
/// assert_eq!(compose('ሀ', 1), Some('ሁ'));
/// // `h` family + order 6 (o) → ሆ.
/// assert_eq!(compose('ሀ', 6), Some('ሆ'));
/// // Base must be a family head — ሁ is order 1, not order 0.
/// assert_eq!(compose('ሁ', 1), None);
/// // Order must be < 8.
/// assert_eq!(compose('ሀ', 8), None);
/// // Base must be in the main Ge'ez block.
/// assert_eq!(compose('A', 0), None);
/// ```
#[must_use]
pub fn compose(base: char, order: u8) -> Option<char> {
    if u32::from(order) >= COLUMNS_PER_FAMILY {
        return None;
    }
    let base_cp = base as u32;
    if !(GEEZ_MAIN_START..=GEEZ_MAIN_END).contains(&base_cp) {
        return None;
    }
    // `base` must itself be a family-head (order 0).
    if (base_cp - GEEZ_MAIN_START) % COLUMNS_PER_FAMILY != 0 {
        return None;
    }
    char::from_u32(base_cp + u32::from(order))
}

/// Is `c` any scalar in the main Ge'ez block U+1200..=U+137F?
///
/// A convenience predicate the tokenizer uses to treat Ge'ez scalars
/// as word-internal (Unicode does not classify every Ge'ez scalar
/// under `is_alphanumeric` — a handful of column-7 labialized slots
/// carry the `Lo` "Letter, other" category the same as the rest, but
/// this predicate does not rely on the character-class database).
#[inline]
#[must_use]
pub const fn is_geez_main(c: char) -> bool {
    let cp = c as u32;
    GEEZ_MAIN_START <= cp && cp <= GEEZ_MAIN_END
}

/// Is `c` any scalar in the Ge'ez / Ethiopic *supplement* block
/// U+1380..=U+139F?
///
/// The supplement carries labialized variants (ኈ ኌ) and Ethiopic
/// tonal marks. It does *not* follow the 8-column layout the main
/// block uses; [`decompose`] and [`compose`] return `None` for
/// scalars in this range.
#[inline]
#[must_use]
pub const fn is_geez_supplement(c: char) -> bool {
    let cp = c as u32;
    0x1380 <= cp && cp <= 0x139F
}

/// Is `c` any scalar in the Ge'ez / Ethiopic *extended* block
/// U+2D80..=U+2DDF?
///
/// The extended block covers additional consonants for Sebat Bet
/// Gurage, Blin, Meʼen, and other Ethio-Semitic and Cushitic
/// languages. It does *not* follow the 8-column layout; [`decompose`]
/// and [`compose`] return `None` for scalars in this range.
#[inline]
#[must_use]
pub const fn is_geez_extended(c: char) -> bool {
    let cp = c as u32;
    0x2D80 <= cp && cp <= 0x2DDF
}

/// Is `c` any Ge'ez-script scalar (main, supplement, or extended)?
///
/// Note: [`decompose`] and [`compose`] only work on the main block;
/// this predicate is broader.
#[inline]
#[must_use]
pub const fn is_geez(c: char) -> bool {
    is_geez_main(c) || is_geez_supplement(c) || is_geez_extended(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decompose_h_family_order_0() {
        // ሀ (U+1200) — the base of the h family, order 0.
        assert_eq!(decompose('ሀ'), Some(('ሀ', 0)));
    }

    #[test]
    fn decompose_h_family_covers_all_seven_vowel_orders() {
        // ሀ ሁ ሂ ሃ ሄ ህ ሆ — orders 0..=6 of the h family.
        let syllables = ['ሀ', 'ሁ', 'ሂ', 'ሃ', 'ሄ', 'ህ', 'ሆ'];
        for (order, &s) in syllables.iter().enumerate() {
            let (base, ord) = decompose(s).unwrap();
            assert_eq!(base, 'ሀ', "order {order}: base of {s} should be ሀ");
            assert_eq!(ord as usize, order, "order of {s} should be {order}");
        }
    }

    #[test]
    fn decompose_returns_none_for_non_geez() {
        assert_eq!(decompose('A'), None);
        assert_eq!(decompose('א'), None); // Hebrew
        assert_eq!(decompose('ا'), None); // Arabic
        assert_eq!(decompose(' '), None);
    }

    #[test]
    fn decompose_returns_none_for_supplement_and_extended() {
        // Supplement U+1380..=U+139F.
        assert_eq!(decompose('\u{1380}'), None);
        // Extended U+2D80..=U+2DDF.
        assert_eq!(decompose('\u{2D80}'), None);
    }

    #[test]
    fn compose_round_trip_h_family() {
        for order in 0u8..7 {
            let scalar = compose('ሀ', order).unwrap();
            let (base, ord) = decompose(scalar).unwrap();
            assert_eq!(base, 'ሀ');
            assert_eq!(ord, order);
        }
    }

    #[test]
    fn compose_rejects_non_family_head_base() {
        // ሁ is order 1, not a family head.
        assert_eq!(compose('ሁ', 0), None);
    }

    #[test]
    fn compose_rejects_order_out_of_range() {
        assert_eq!(compose('ሀ', 8), None);
        assert_eq!(compose('ሀ', 255), None);
    }

    #[test]
    fn compose_rejects_non_geez_base() {
        assert_eq!(compose('A', 0), None);
        assert_eq!(compose('א', 0), None);
    }

    #[test]
    fn is_geez_predicates() {
        assert!(is_geez_main('ሀ'));
        assert!(!is_geez_main('A'));
        assert!(is_geez_supplement('\u{1380}'));
        assert!(!is_geez_supplement('ሀ'));
        assert!(is_geez_extended('\u{2D80}'));
        assert!(!is_geez_extended('ሀ'));
        assert!(is_geez('ሀ'));
        assert!(is_geez('\u{1380}'));
        assert!(is_geez('\u{2D80}'));
        assert!(!is_geez('A'));
    }

    #[test]
    fn m_family_decompose() {
        // መ (U+1218) — base of the m family, order 0.
        assert_eq!(decompose('መ'), Some(('መ', 0)));
        // ሙ (U+1219) — m + u, order 1.
        assert_eq!(decompose('ሙ'), Some(('መ', 1)));
        // ሞ (U+121E) — m + o, order 6.
        assert_eq!(decompose('ሞ'), Some(('መ', 6)));
    }

    #[test]
    fn every_family_head_decomposes_to_itself_with_order_zero() {
        // 48 families in the main block; every family head at
        // U+1200 + family*8 must decompose to (itself, 0).
        for family in 0u32..48 {
            let cp = GEEZ_MAIN_START + family * COLUMNS_PER_FAMILY;
            let c = char::from_u32(cp).unwrap();
            let (base, order) = decompose(c).unwrap();
            assert_eq!(base, c, "family {family}: base should be itself");
            assert_eq!(order, 0, "family {family}: order should be 0");
        }
    }
}
