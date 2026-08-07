//! [`EnglishCldrCollator`] — CLDR-tailored English collation backed by
//! [ICU4X's `icu_collator`](https://docs.rs/icu_collator).
//!
//! Only compiled when the `icu-collator` Cargo feature is enabled.
//!
//! # When to pick this over [`EnglishCollator`](crate::EnglishCollator)
//!
//! The default English collator in this crate
//! ([`EnglishCollator`](crate::EnglishCollator)) is a fast, ~zero-byte
//! ASCII walker with three opinionated rules (article-strip, case-fold,
//! digits-after-letters). It is what most callers reach for: it
//! handles English titles, dictionary entries, and glossary output the
//! way a librarian would.
//!
//! `EnglishCldrCollator` is for the cases where that is not enough:
//!
//! * **Full Unicode input, not just ASCII.** The dictionary collator
//!   only case-folds ASCII letters; the CLDR collator honors the whole
//!   UCA table, so `"café" ~= "cafe"` at primary strength, `"ß" ~= "ss"`
//!   in the German fallback, and `"æ"` sorts in the right place.
//! * **Locale-conformant ordering.** CLDR encodes English's specific
//!   tailorings of the Default Unicode Collation Element Table (DUCET)
//!   — including how English handles contractions and expansions.
//! * **Configurable strength.** The four UCA strength levels
//!   (primary, secondary, tertiary, quaternary) let a caller decide
//!   whether case, accents, or punctuation should be tie-breakers.
//! * **Numeric-value sorting.** `"file2" < "file10"` when numeric
//!   sorting is enabled (off by default; enable via
//!   [`CldrCollatorOptions::with_numeric`]).
//!
//! # Cost
//!
//! Pulling in `icu_collator` adds roughly 40 KB to a release wasm
//! build — that's the crate itself plus the compiled CLDR collation
//! table baked in via `icu_collator/compiled_data`. That is the whole
//! reason this collator is opt-in; a stringcheese-en default build
//! stays free of ICU4X.
//!
//! # Example
//!
//! ```
//! # #[cfg(feature = "icu-collator")]
//! # {
//! use stringcheese_en::ENGLISH_CLDR_COLLATOR;
//! use stringcheese_lang::Collator;
//! use core::cmp::Ordering;
//!
//! // Accent-insensitive at primary strength (default is tertiary,
//! // so the accent DOES matter here — use `CldrCollatorOptions` to
//! // pick primary if that is what you want).
//! assert_eq!(
//!     ENGLISH_CLDR_COLLATOR.compare("apple", "banana"),
//!     Ordering::Less,
//! );
//! # }
//! ```

use core::cmp::Ordering;
use std::sync::OnceLock;

use icu_collator::{Collator as IcuCollator, CollatorOptions};
use icu_locid::locale;

use stringcheese_lang::Collator;

// The four UCA strength levels are re-exported here so callers of
// `CldrCollatorOptions::with_strength` do not need to depend on
// `icu_collator` directly.
pub use icu_collator::Strength;

/// Runtime options for [`EnglishCldrCollator`].
///
/// A small, closed set of the ICU4X [`CollatorOptions`] knobs — the
/// ones a downstream caller is likely to want to tweak on a per-index
/// basis. Callers who need the full ICU4X option surface should reach
/// for `icu_collator` directly rather than driving it through this
/// wrapper.
///
/// Defaults:
///
/// * `strength = Strength::Tertiary` (case- and accent-sensitive, the
///   ICU4X default and what CLDR ships as the English "root" ordering).
/// * `numeric = false` (raw code-point compare on digit runs; enable
///   for `"file2" < "file10"`).
/// * `case_level = false` (case is folded into the tertiary weight
///   rather than its own level; enable for a dedicated case tie-break
///   between the secondary and tertiary levels).
#[derive(Copy, Clone, Debug)]
pub struct CldrCollatorOptions {
    strength: Strength,
    numeric: bool,
    case_level: bool,
}

impl CldrCollatorOptions {
    /// Construct the default option set (tertiary strength,
    /// non-numeric, case as tertiary tie-break). Const-usable so the
    /// exported [`ENGLISH_CLDR_COLLATOR`] static can be built at
    /// compile time.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            strength: Strength::Tertiary,
            numeric: false,
            case_level: false,
        }
    }

    /// Set the UCA collation strength.
    ///
    /// See [`Strength`] for the four supported levels. Higher strength
    /// distinguishes finer variations (accent, case, punctuation);
    /// lower strength collapses them.
    #[must_use]
    pub const fn with_strength(mut self, strength: Strength) -> Self {
        self.strength = strength;
        self
    }

    /// Enable numeric-value sorting for runs of ASCII digits.
    ///
    /// With this on, `"file2"` sorts before `"file10"` — the digit run
    /// is compared as a number rather than character-by-character.
    #[must_use]
    pub const fn with_numeric(mut self, on: bool) -> Self {
        self.numeric = on;
        self
    }

    /// Enable the case level.
    ///
    /// With the case level on, case differences become their own
    /// dedicated tie-break between the secondary and tertiary levels
    /// — useful when you want the collator to be accent-insensitive at
    /// primary strength but still stable in case ordering.
    #[must_use]
    pub const fn with_case_level(mut self, on: bool) -> Self {
        self.case_level = on;
        self
    }
}

impl Default for CldrCollatorOptions {
    fn default() -> Self {
        Self::new()
    }
}

/// English collator backed by ICU4X's Unicode Collation Algorithm
/// implementation with English (`en`) CLDR tailoring.
///
/// See the [module-level docs](self) for when to pick this over the
/// tiny ASCII [`EnglishCollator`](crate::EnglishCollator).
///
/// The inner ICU4X [`icu_collator::Collator`] is built lazily on the
/// first [`compare`](Self::compare) call — building it is measured in
/// microseconds but is still slow enough that a hot loop should
/// [`compare`](Self::compare) through the same
/// `EnglishCldrCollator` value rather than constructing a new one per
/// call. The exported [`ENGLISH_CLDR_COLLATOR`] static is the intended
/// entry point; the built collator is amortized across every caller
/// that reaches for it.
pub struct EnglishCldrCollator {
    options: CldrCollatorOptions,
    // `OnceLock` rather than `LazyLock` so the construction closure can
    // capture `self.options` — the LazyLock variant of the API requires
    // a bare `fn` pointer, which can't carry runtime options.
    inner: OnceLock<IcuCollator>,
}

impl EnglishCldrCollator {
    /// Construct with the default option set
    /// ([`CldrCollatorOptions::new`]).
    ///
    /// The inner ICU4X collator is not built until the first
    /// [`compare`](Self::compare) call; construction here is cheap and
    /// const-usable.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            options: CldrCollatorOptions::new(),
            inner: OnceLock::new(),
        }
    }

    /// Construct with an explicit option set.
    ///
    /// Same lazy-init semantics as [`new`](Self::new).
    #[must_use]
    pub const fn with_options(options: CldrCollatorOptions) -> Self {
        Self {
            options,
            inner: OnceLock::new(),
        }
    }

    /// The stored options.
    #[must_use]
    pub const fn options(&self) -> CldrCollatorOptions {
        self.options
    }

    /// Access the underlying ICU4X collator, building it on first
    /// call. Escape hatch for callers who want to reach past this
    /// wrapper — most callers should go through
    /// [`compare`](Self::compare) instead.
    #[must_use]
    pub fn icu(&self) -> &IcuCollator {
        self.inner.get_or_init(|| build_icu_collator(self.options))
    }
}

impl Default for EnglishCldrCollator {
    fn default() -> Self {
        Self::new()
    }
}

impl Collator for EnglishCldrCollator {
    fn compare(&self, a: &str, b: &str) -> Ordering {
        self.icu().compare(a, b)
    }
}

/// The default [`EnglishCldrCollator`] instance.
///
/// Amortizes the lazy ICU4X collator build across every caller. This
/// is what [`English::collator`](crate::English) hands back when the
/// pack is constructed via
/// [`English::with_cldr_collator`](crate::English::with_cldr_collator)
/// (or the shipped [`ENGLISH_WITH_CLDR_COLLATOR`](crate::ENGLISH_WITH_CLDR_COLLATOR)
/// constant).
pub static ENGLISH_CLDR_COLLATOR: EnglishCldrCollator = EnglishCldrCollator::new();

/// Build the inner ICU4X collator from the wrapper's options.
///
/// `Collator::try_new` cannot fail for the English locale with the
/// baked-data path (`icu_collator/compiled_data`) — the English
/// collation table is always present. The `expect` therefore
/// witnesses an ICU4X invariant, not a runtime condition the caller
/// can influence.
fn build_icu_collator(options: CldrCollatorOptions) -> IcuCollator {
    let mut opts = CollatorOptions::new();
    opts.strength = Some(options.strength);
    if options.numeric {
        opts.numeric = Some(icu_collator::Numeric::On);
    }
    if options.case_level {
        opts.case_level = Some(icu_collator::CaseLevel::On);
    }
    IcuCollator::try_new(&locale!("en").into(), opts)
        .expect("icu_collator English locale is baked in via compiled_data")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_options_are_tertiary() {
        let o = CldrCollatorOptions::new();
        assert!(matches!(o.strength, Strength::Tertiary));
        assert!(!o.numeric);
        assert!(!o.case_level);
    }

    #[test]
    fn options_builder_composes() {
        let o = CldrCollatorOptions::new()
            .with_strength(Strength::Primary)
            .with_numeric(true)
            .with_case_level(true);
        assert!(matches!(o.strength, Strength::Primary));
        assert!(o.numeric);
        assert!(o.case_level);
    }

    #[test]
    fn compare_orders_lowercase_letters() {
        // Baseline: at tertiary strength "apple" < "banana".
        assert_eq!(
            ENGLISH_CLDR_COLLATOR.compare("apple", "banana"),
            Ordering::Less,
        );
        assert_eq!(
            ENGLISH_CLDR_COLLATOR.compare("banana", "apple"),
            Ordering::Greater,
        );
        assert_eq!(
            ENGLISH_CLDR_COLLATOR.compare("apple", "apple"),
            Ordering::Equal,
        );
    }

    #[test]
    fn cldr_case_differs_from_ascii_ordering() {
        // Raw ASCII would put uppercase before lowercase because
        // 'Z' (0x5A) < 'a' (0x61); UCA at tertiary strength orders
        // by base letter first (primary), and "Zebra" > "apple" at
        // primary strength regardless of case. Witnesses the
        // difference between the ASCII collator's raw code-point
        // walk and the CLDR collator's UCA-tailored ordering.
        assert_eq!(
            ENGLISH_CLDR_COLLATOR.compare("apple", "Zebra"),
            Ordering::Less,
        );
        // ASCII sanity check for contrast:
        assert_eq!("apple".cmp("Zebra"), Ordering::Greater);
    }

    #[test]
    fn accent_insensitive_at_primary_strength() {
        // At primary strength, "café" and "cafe" collate as equal
        // (accents are secondary differences that are ignored).
        let c = EnglishCldrCollator::with_options(
            CldrCollatorOptions::new().with_strength(Strength::Primary),
        );
        assert_eq!(c.compare("café", "cafe"), Ordering::Equal);
        // The default (tertiary) collator, in contrast, treats them
        // as distinct.
        assert_ne!(
            ENGLISH_CLDR_COLLATOR.compare("café", "cafe"),
            Ordering::Equal,
        );
    }

    #[test]
    fn numeric_option_orders_by_value() {
        // Without numeric: lexicographic — "file10" < "file2".
        let plain = EnglishCldrCollator::new();
        assert_eq!(plain.compare("file10", "file2"), Ordering::Less);
        // With numeric: value-based — "file10" > "file2".
        let numeric =
            EnglishCldrCollator::with_options(CldrCollatorOptions::new().with_numeric(true));
        assert_eq!(numeric.compare("file2", "file10"), Ordering::Less);
    }

    #[test]
    fn same_locale_case_ordering() {
        // "coke" vs "Coke" — English CLDR (root-derived) puts
        // lowercase before uppercase within an otherwise-equal
        // primary/secondary key, at tertiary strength. Locking this
        // in guards against a future ICU4X update flipping the
        // default case-first behavior.
        assert_eq!(
            ENGLISH_CLDR_COLLATOR.compare("coke", "Coke"),
            Ordering::Less,
        );
    }

    #[test]
    fn compare_is_reflexive_on_a_few_inputs() {
        for s in ["", "a", "The Beatles", "café", "über", "1984"] {
            assert_eq!(ENGLISH_CLDR_COLLATOR.compare(s, s), Ordering::Equal);
        }
    }

    #[test]
    fn compare_is_antisymmetric_on_a_few_pairs() {
        let cases = [
            ("apple", "banana"),
            ("Zebra", "apple"),
            ("café", "cafe"),
            ("The Beatles", "Abbey Road"),
        ];
        for (a, b) in cases {
            let ab = ENGLISH_CLDR_COLLATOR.compare(a, b);
            let ba = ENGLISH_CLDR_COLLATOR.compare(b, a);
            assert_eq!(ab, ba.reverse(), "compare({a:?}, {b:?}) antisymmetry");
        }
    }

    #[test]
    fn empty_strings_compare_equal() {
        assert_eq!(ENGLISH_CLDR_COLLATOR.compare("", ""), Ordering::Equal);
        assert_eq!(ENGLISH_CLDR_COLLATOR.compare("", "a"), Ordering::Less);
        assert_eq!(ENGLISH_CLDR_COLLATOR.compare("a", ""), Ordering::Greater);
    }

    #[test]
    fn options_accessor_round_trips() {
        let opts = CldrCollatorOptions::new()
            .with_strength(Strength::Secondary)
            .with_numeric(true);
        let c = EnglishCldrCollator::with_options(opts);
        assert!(matches!(c.options().strength, Strength::Secondary));
        assert!(c.options().numeric);
    }

    #[test]
    fn icu_accessor_returns_working_collator() {
        // The escape hatch produces a functioning ICU4X collator that
        // agrees with our wrapper.
        let inner = ENGLISH_CLDR_COLLATOR.icu();
        assert_eq!(
            inner.compare("apple", "banana"),
            ENGLISH_CLDR_COLLATOR.compare("apple", "banana"),
        );
    }
}

// Property tests — same gate pattern as the crate's other
// property-test modules: std-only and off wasm.
#[cfg(all(test, feature = "std", not(target_family = "wasm")))]
mod properties {
    use super::*;
    use proptest::prelude::*;

    fn arbitrary_text() -> impl Strategy<Value = String> {
        prop::string::string_regex("[a-zA-Z0-9 çéñü'.]{0,30}").expect("static regex is valid")
    }

    proptest! {
        /// Total: every call returns one of `Less`, `Equal`, `Greater`
        /// and never panics.
        #[test]
        fn cldr_collator_is_total(a in arbitrary_text(), b in arbitrary_text()) {
            let ord = ENGLISH_CLDR_COLLATOR.compare(&a, &b);
            prop_assert!(matches!(ord, Ordering::Less | Ordering::Equal | Ordering::Greater));
        }

        /// Reflexive: `compare(x, x) == Equal`.
        #[test]
        fn cldr_collator_is_reflexive(a in arbitrary_text()) {
            prop_assert_eq!(ENGLISH_CLDR_COLLATOR.compare(&a, &a), Ordering::Equal);
        }

        /// Antisymmetric: `compare(a, b) == compare(b, a).reverse()`.
        #[test]
        fn cldr_collator_is_antisymmetric(a in arbitrary_text(), b in arbitrary_text()) {
            prop_assert_eq!(
                ENGLISH_CLDR_COLLATOR.compare(&a, &b),
                ENGLISH_CLDR_COLLATOR.compare(&b, &a).reverse(),
            );
        }

        /// Transitive on the ≤ direction (and by symmetry the ≥
        /// direction): if `a ≤ b` and `b ≤ c` then `a ≤ c`.
        #[test]
        fn cldr_collator_is_transitive(
            a in arbitrary_text(),
            b in arbitrary_text(),
            c in arbitrary_text(),
        ) {
            let ab = ENGLISH_CLDR_COLLATOR.compare(&a, &b);
            let bc = ENGLISH_CLDR_COLLATOR.compare(&b, &c);
            let ac = ENGLISH_CLDR_COLLATOR.compare(&a, &c);
            if ab != Ordering::Greater && bc != Ordering::Greater {
                prop_assert_ne!(ac, Ordering::Greater);
            }
            if ab != Ordering::Less && bc != Ordering::Less {
                prop_assert_ne!(ac, Ordering::Less);
            }
        }

        /// Deterministic: repeated calls with the same inputs yield
        /// the same answer.
        #[test]
        fn cldr_collator_is_deterministic(a in arbitrary_text(), b in arbitrary_text()) {
            let first = ENGLISH_CLDR_COLLATOR.compare(&a, &b);
            let second = ENGLISH_CLDR_COLLATOR.compare(&a, &b);
            prop_assert_eq!(first, second);
        }
    }
}
