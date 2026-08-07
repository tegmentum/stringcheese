//! [`GermanCollator`] — DIN 5007 German dictionary and phonebook
//! collation.
//!
//! Implements [`stringcheese_lang::Collator`] for the two shipped
//! variants of DIN 5007:
//!
//! * **DIN 5007-1 (dictionary / Duden ordering).** Umlauts fold to
//!   their base letter — `ä = a`, `ö = o`, `ü = u`, and `ß = ss`. So
//!   `"Bär"` sorts with `"Bar"`, and `"Müller"` sorts as if it were
//!   `"Muller"`. This is the ordering German dictionaries and
//!   encyclopedias use.
//! * **DIN 5007-2 (phonebook ordering).** Umlauts expand to the
//!   digraph they historically abbreviate — `ä = ae`, `ö = oe`,
//!   `ü = ue`, and `ß = ss`. So `"Bär"` sorts as if it were `"Baer"`
//!   (between `"Bad"` and `"Baf"`), and `"Müller"` sorts as if it were
//!   `"Mueller"` (which puts it just before `"Muller"` in the alphabet).
//!   This is the ordering German telephone directories use.
//!
//! The two variants agree on the treatment of `ß` (both expand it to
//! `ss`); they diverge on the umlauts.
//!
//! # Presets
//!
//! Three shipped configurations cover the common cases:
//!
//! * [`GermanCollator::DIN_5007_DICTIONARY`] — DIN 5007-1 with ASCII
//!   case folding. This is what
//!   [`GERMAN_WITH_DIN5007_DICTIONARY`](crate::GERMAN_WITH_DIN5007_DICTIONARY)
//!   hands back from [`Language::collator`](stringcheese_lang::Language::collator).
//! * [`GermanCollator::DIN_5007_PHONEBOOK`] — DIN 5007-2 with ASCII
//!   case folding. This is what
//!   [`GERMAN_WITH_DIN5007_PHONEBOOK`](crate::GERMAN_WITH_DIN5007_PHONEBOOK)
//!   hands back.
//! * [`GermanCollator::ASCII`] — the identity fallback with no umlaut
//!   expansion, no case folding, and equivalent to raw [`str::cmp`].
//!   Useful as a baseline for tests or when the caller wants a
//!   locale-neutral comparator carried through the
//!   [`Language`](stringcheese_lang::Language) trait rather than
//!   plumbed separately.
//!
//! # Zero allocation
//!
//! [`GermanCollator::compare`] walks both inputs
//! character-by-character. The umlaut expansions never produce more
//! than two output characters per input character (`ß → ss`,
//! `ü → ue`), so the walk carries a single one-`char` pending slot per
//! side and never allocates. Case folding is a per-character ASCII
//! bit-twiddle; non-ASCII characters (other than the DIN 5007
//! expansions themselves) pass through untouched.
//!
//! # Example
//!
//! ```
//! use stringcheese_de::GermanCollator;
//! use stringcheese_lang::Collator;
//! use core::cmp::Ordering;
//!
//! // DIN 5007-1 (dictionary): "Bär" sorts with "Bar" (ä folds to a).
//! assert_eq!(
//!     GermanCollator::DIN_5007_DICTIONARY.compare("Bär", "Bar"),
//!     Ordering::Equal,
//! );
//!
//! // DIN 5007-2 (phonebook): "Bär" sorts as "Baer" (ä expands to ae).
//! assert_eq!(
//!     GermanCollator::DIN_5007_PHONEBOOK.compare("Bär", "Baer"),
//!     Ordering::Equal,
//! );
//!
//! // Both variants agree that `ß` expands to `ss`.
//! assert_eq!(
//!     GermanCollator::DIN_5007_DICTIONARY.compare("Straße", "Strasse"),
//!     Ordering::Equal,
//! );
//! assert_eq!(
//!     GermanCollator::DIN_5007_PHONEBOOK.compare("Straße", "Strasse"),
//!     Ordering::Equal,
//! );
//! ```

use core::cmp::Ordering;

use stringcheese_lang::Collator;

/// The DIN 5007 variant a [`GermanCollator`] follows.
///
/// See the [module-level docs](self) for the difference between the
/// dictionary (variant 1) and phonebook (variant 2) conventions.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum GermanCollatorVariant {
    /// DIN 5007-1 — dictionary / Duden ordering: `ä = a`, `ö = o`,
    /// `ü = u`, `ß = ss`.
    Din5007Variant1,
    /// DIN 5007-2 — phonebook ordering: `ä = ae`, `ö = oe`, `ü = ue`,
    /// `ß = ss`.
    Din5007Variant2,
    /// No German-specific folding; raw code-point comparison. Provided
    /// as an identity fallback for tests and for callers who want a
    /// locale-neutral comparator that still travels through the
    /// [`Language::collator`](stringcheese_lang::Language::collator)
    /// trait accessor.
    Ascii,
}

/// A DIN 5007 German collator.
///
/// See the [module-level docs](self) for the two conventions the
/// collator implements and the three shipped presets
/// ([`DIN_5007_DICTIONARY`](Self::DIN_5007_DICTIONARY),
/// [`DIN_5007_PHONEBOOK`](Self::DIN_5007_PHONEBOOK),
/// [`ASCII`](Self::ASCII)).
///
/// Constructed either through the presets, through
/// [`new`](Self::new) (which returns
/// [`DIN_5007_DICTIONARY`](Self::DIN_5007_DICTIONARY)), or through the
/// builder-style `with_*` methods that toggle a single field at a
/// time.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct GermanCollator {
    variant: GermanCollatorVariant,
    case_insensitive: bool,
}

impl GermanCollator {
    /// The DIN 5007-1 (dictionary / Duden) preset with ASCII case
    /// folding on.
    ///
    /// This is what [`GERMAN_WITH_DIN5007_DICTIONARY`](crate::GERMAN_WITH_DIN5007_DICTIONARY)
    /// exposes through
    /// [`Language::collator`](stringcheese_lang::Language::collator).
    pub const DIN_5007_DICTIONARY: GermanCollator = GermanCollator {
        variant: GermanCollatorVariant::Din5007Variant1,
        case_insensitive: true,
    };

    /// The DIN 5007-2 (phonebook) preset with ASCII case folding on.
    ///
    /// This is what [`GERMAN_WITH_DIN5007_PHONEBOOK`](crate::GERMAN_WITH_DIN5007_PHONEBOOK)
    /// exposes through
    /// [`Language::collator`](stringcheese_lang::Language::collator).
    pub const DIN_5007_PHONEBOOK: GermanCollator = GermanCollator {
        variant: GermanCollatorVariant::Din5007Variant2,
        case_insensitive: true,
    };

    /// The identity preset: no umlaut expansion, no case folding.
    ///
    /// Equivalent to [`str::cmp`] as a total order — useful as a
    /// baseline for tests or when the caller wants a locale-neutral
    /// comparator carried through the
    /// [`Language`](stringcheese_lang::Language) trait rather than
    /// plumbed separately.
    pub const ASCII: GermanCollator = GermanCollator {
        variant: GermanCollatorVariant::Ascii,
        case_insensitive: false,
    };

    /// Construct a new collator with the
    /// [`DIN_5007_DICTIONARY`](Self::DIN_5007_DICTIONARY) preset.
    ///
    /// Equivalent to `GermanCollator::DIN_5007_DICTIONARY`; provided
    /// as a familiar entry point for `Default`-shaped code.
    #[must_use]
    pub const fn new() -> Self {
        Self::DIN_5007_DICTIONARY
    }

    /// Return a collator with the DIN 5007 variant swapped.
    ///
    /// The `case_insensitive` flag is left as-is; callers who want to
    /// switch both together should chain
    /// [`with_case_insensitive`](Self::with_case_insensitive).
    #[must_use]
    pub const fn with_variant(mut self, variant: GermanCollatorVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Toggle the ASCII case-fold rule.
    ///
    /// The DIN 5007 presets ship with this on; the
    /// [`ASCII`](Self::ASCII) preset ships with it off.
    #[must_use]
    pub const fn with_case_insensitive(mut self, v: bool) -> Self {
        self.case_insensitive = v;
        self
    }

    /// Character-comparison key for a single (post-expansion) scalar.
    ///
    /// Case-folds ASCII letters when `case_insensitive` is set; passes
    /// non-ASCII scalars through unchanged. DIN 5007's umlaut
    /// expansions produce ASCII letters, so they land in the case-fold
    /// path.
    #[inline]
    fn char_key(self, c: char) -> u32 {
        if self.case_insensitive && c.is_ascii_alphabetic() {
            return c.to_ascii_lowercase() as u32;
        }
        c as u32
    }

    /// Expand a single scalar according to the DIN 5007 variant.
    ///
    /// Returns the first output character and (for two-character
    /// expansions) an optional pending second character. `ß → ss` is
    /// shared between both DIN variants; the umlauts diverge —
    /// variant 1 folds them to their base vowel, variant 2 expands
    /// them to the classic digraph.
    ///
    /// Under the [`Ascii`](GermanCollatorVariant::Ascii) variant the
    /// input scalar passes through untouched.
    #[inline]
    fn expand(self, c: char) -> (char, Option<char>) {
        match (self.variant, c) {
            (GermanCollatorVariant::Ascii, _) => (c, None),

            // Variant 1: umlauts fold to their base letter.
            (GermanCollatorVariant::Din5007Variant1, 'ä') => ('a', None),
            (GermanCollatorVariant::Din5007Variant1, 'Ä') => ('A', None),
            (GermanCollatorVariant::Din5007Variant1, 'ö') => ('o', None),
            (GermanCollatorVariant::Din5007Variant1, 'Ö') => ('O', None),
            (GermanCollatorVariant::Din5007Variant1, 'ü') => ('u', None),
            (GermanCollatorVariant::Din5007Variant1, 'Ü') => ('U', None),

            // Variant 2: umlauts expand to their digraph.
            (GermanCollatorVariant::Din5007Variant2, 'ä') => ('a', Some('e')),
            (GermanCollatorVariant::Din5007Variant2, 'Ä') => ('A', Some('E')),
            (GermanCollatorVariant::Din5007Variant2, 'ö') => ('o', Some('e')),
            (GermanCollatorVariant::Din5007Variant2, 'Ö') => ('O', Some('E')),
            (GermanCollatorVariant::Din5007Variant2, 'ü') => ('u', Some('e')),
            (GermanCollatorVariant::Din5007Variant2, 'Ü') => ('U', Some('E')),

            // ß is treated as ss under both variants.
            (
                GermanCollatorVariant::Din5007Variant1 | GermanCollatorVariant::Din5007Variant2,
                'ß',
            ) => ('s', Some('s')),

            (_, c) => (c, None),
        }
    }
}

impl Default for GermanCollator {
    fn default() -> Self {
        Self::DIN_5007_DICTIONARY
    }
}

impl Collator for GermanCollator {
    fn compare(&self, a: &str, b: &str) -> Ordering {
        let mut ai = a.chars();
        let mut bi = b.chars();
        let mut a_pending: Option<char> = None;
        let mut b_pending: Option<char> = None;
        loop {
            let ac = next_expanded(*self, &mut ai, &mut a_pending);
            let bc = next_expanded(*self, &mut bi, &mut b_pending);
            match (ac, bc) {
                (None, None) => return Ordering::Equal,
                (None, Some(_)) => return Ordering::Less,
                (Some(_), None) => return Ordering::Greater,
                (Some(ac), Some(bc)) => {
                    let ak = self.char_key(ac);
                    let bk = self.char_key(bc);
                    match ak.cmp(&bk) {
                        Ordering::Equal => {}
                        ord => return ord,
                    }
                }
            }
        }
    }
}

/// Pull the next post-expansion character from a chars iterator plus
/// its one-slot pending buffer.
///
/// Isolated as a free function so both sides of the comparison in
/// [`GermanCollator::compare`] can share the same walker without
/// borrowing `self` twice.
#[inline]
fn next_expanded(
    collator: GermanCollator,
    iter: &mut core::str::Chars<'_>,
    pending: &mut Option<char>,
) -> Option<char> {
    if let Some(p) = pending.take() {
        return Some(p);
    }
    let c = iter.next()?;
    let (first, rest) = collator.expand(c);
    *pending = rest;
    Some(first)
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    extern crate alloc;

    use super::*;
    use alloc::vec::Vec;

    // ---- expansion primitives ---------------------------------------

    #[test]
    fn variant1_folds_umlauts_to_base_letter() {
        let c = GermanCollator::DIN_5007_DICTIONARY;
        assert_eq!(c.expand('ä'), ('a', None));
        assert_eq!(c.expand('ö'), ('o', None));
        assert_eq!(c.expand('ü'), ('u', None));
        assert_eq!(c.expand('Ä'), ('A', None));
        assert_eq!(c.expand('Ö'), ('O', None));
        assert_eq!(c.expand('Ü'), ('U', None));
    }

    #[test]
    fn variant2_expands_umlauts_to_digraph() {
        let c = GermanCollator::DIN_5007_PHONEBOOK;
        assert_eq!(c.expand('ä'), ('a', Some('e')));
        assert_eq!(c.expand('ö'), ('o', Some('e')));
        assert_eq!(c.expand('ü'), ('u', Some('e')));
        assert_eq!(c.expand('Ä'), ('A', Some('E')));
        assert_eq!(c.expand('Ö'), ('O', Some('E')));
        assert_eq!(c.expand('Ü'), ('U', Some('E')));
    }

    #[test]
    fn both_variants_expand_ss_ligature() {
        assert_eq!(
            GermanCollator::DIN_5007_DICTIONARY.expand('ß'),
            ('s', Some('s'))
        );
        assert_eq!(
            GermanCollator::DIN_5007_PHONEBOOK.expand('ß'),
            ('s', Some('s'))
        );
    }

    #[test]
    fn ascii_variant_passes_everything_through() {
        let c = GermanCollator::ASCII;
        for ch in ['a', 'z', 'ä', 'ö', 'ü', 'ß', 'A', 'Ä'] {
            assert_eq!(c.expand(ch), (ch, None));
        }
    }

    // ---- DIN 5007-1 (dictionary) reference orderings -----------------

    #[test]
    fn dictionary_bar_baer_baer_ordering() {
        // Bär (=Bar under DIN-1) sorts equal to Bar; "Baer" is 'B','a','e','r'
        // which comes before "Bar" ('B','a','r') because 'e' < 'r'.
        let c = GermanCollator::DIN_5007_DICTIONARY;
        assert_eq!(c.compare("Bär", "Bar"), Ordering::Equal);
        assert_eq!(c.compare("Baer", "Bar"), Ordering::Less);
        assert_eq!(c.compare("Baer", "Bär"), Ordering::Less);
    }

    #[test]
    fn dictionary_mueller_ordering() {
        // Müller → "Muller"; sorts equal to "Muller". "Mueller" (literal)
        // sorts before both because 'e' < 'l' at position 2.
        let c = GermanCollator::DIN_5007_DICTIONARY;
        assert_eq!(c.compare("Müller", "Muller"), Ordering::Equal);
        assert_eq!(c.compare("Mueller", "Müller"), Ordering::Less);
        assert_eq!(c.compare("Mueller", "Muller"), Ordering::Less);
    }

    #[test]
    fn dictionary_mueller_munk_muster_classic_example() {
        // From the DIN 5007-1 reference: Müller (=Muller) < Munk < Muster.
        let c = GermanCollator::DIN_5007_DICTIONARY;
        let mut ws: Vec<&str> = ["Muster", "Müller", "Munk"].into();
        ws.sort_by(|a, b| c.compare(a, b));
        assert_eq!(ws, ["Müller", "Munk", "Muster"]);
    }

    #[test]
    fn dictionary_strasse_ss_equivalence() {
        let c = GermanCollator::DIN_5007_DICTIONARY;
        assert_eq!(c.compare("Straße", "Strasse"), Ordering::Equal);
    }

    #[test]
    fn dictionary_case_insensitive_on_ascii() {
        let c = GermanCollator::DIN_5007_DICTIONARY;
        assert_eq!(c.compare("Müller", "MÜLLER"), Ordering::Equal);
        assert_eq!(c.compare("apfel", "APFEL"), Ordering::Equal);
    }

    // ---- DIN 5007-2 (phonebook) reference orderings ------------------

    #[test]
    fn phonebook_bar_baer_ordering() {
        // Under DIN-2, Bär → "Baer". So Bär sorts equal to "Baer" and
        // both come before "Bar" ('e' < 'r' at position 2).
        let c = GermanCollator::DIN_5007_PHONEBOOK;
        assert_eq!(c.compare("Bär", "Baer"), Ordering::Equal);
        assert_eq!(c.compare("Bär", "Bar"), Ordering::Less);
        assert_eq!(c.compare("Baer", "Bar"), Ordering::Less);
    }

    #[test]
    fn phonebook_mueller_ordering() {
        // Under DIN-2, Müller → "Mueller"; sorts equal to "Mueller" and
        // before "Muller" ('e' < 'l' at position 2).
        let c = GermanCollator::DIN_5007_PHONEBOOK;
        assert_eq!(c.compare("Müller", "Mueller"), Ordering::Equal);
        assert_eq!(c.compare("Müller", "Muller"), Ordering::Less);
        assert_eq!(c.compare("Mueller", "Muller"), Ordering::Less);
    }

    #[test]
    fn phonebook_puts_umlaut_between_letters() {
        // From the DIN 5007-2 reference: "Muller" > "Müller" (=Mueller),
        // and "Müller" < "Munk" < "Muster".
        let c = GermanCollator::DIN_5007_PHONEBOOK;
        let mut ws: Vec<&str> = ["Muller", "Muster", "Müller", "Munk"].into();
        ws.sort_by(|a, b| c.compare(a, b));
        // "Muell..." < "Mull..." < "Munk" < "Muster"
        assert_eq!(ws, ["Müller", "Muller", "Munk", "Muster"]);
    }

    #[test]
    fn phonebook_strasse_ss_equivalence() {
        let c = GermanCollator::DIN_5007_PHONEBOOK;
        assert_eq!(c.compare("Straße", "Strasse"), Ordering::Equal);
    }

    // ---- ASCII preset -----------------------------------------------

    #[test]
    fn ascii_preset_is_raw_str_cmp() {
        let c = GermanCollator::ASCII;
        for (a, b) in [
            ("A", "a"),
            ("Bär", "Bar"),
            ("Straße", "Strasse"),
            ("Müller", "Muller"),
            ("äpfel", "apfel"),
        ] {
            assert_eq!(c.compare(a, b), a.cmp(b));
        }
    }

    // ---- builder / defaults -----------------------------------------

    #[test]
    fn dictionary_new_equals_dictionary_preset() {
        assert_eq!(GermanCollator::new(), GermanCollator::DIN_5007_DICTIONARY);
        assert_eq!(
            GermanCollator::default(),
            GermanCollator::DIN_5007_DICTIONARY
        );
    }

    #[test]
    fn with_variant_switches_only_the_variant() {
        let c = GermanCollator::DIN_5007_DICTIONARY
            .with_variant(GermanCollatorVariant::Din5007Variant2);
        assert_eq!(c, GermanCollator::DIN_5007_PHONEBOOK);
    }

    #[test]
    fn with_case_insensitive_toggles() {
        let c = GermanCollator::DIN_5007_DICTIONARY.with_case_insensitive(false);
        // With case-fold off, "Müller" (fold ü→u) still compares as
        // "Muller", but ASCII casing now matters.
        assert_eq!(c.compare("Müller", "Muller"), Ordering::Equal);
        assert_eq!(c.compare("Muller", "muller"), "Muller".cmp("muller"));
    }

    // ---- Collator contract axioms (spot checks) ---------------------

    #[test]
    fn compare_is_reflexive() {
        for c in [
            GermanCollator::DIN_5007_DICTIONARY,
            GermanCollator::DIN_5007_PHONEBOOK,
            GermanCollator::ASCII,
        ] {
            for s in ["", "Müller", "Straße", "Bär", "algorithmus"] {
                assert_eq!(c.compare(s, s), Ordering::Equal);
            }
        }
    }

    #[test]
    fn compare_is_antisymmetric() {
        for c in [
            GermanCollator::DIN_5007_DICTIONARY,
            GermanCollator::DIN_5007_PHONEBOOK,
        ] {
            for (a, b) in [
                ("Bär", "Bar"),
                ("Müller", "Mueller"),
                ("Straße", "Strasse"),
                ("Muller", "Munk"),
            ] {
                let ab = c.compare(a, b);
                let ba = c.compare(b, a);
                assert_eq!(ab, ba.reverse(), "compare({a:?}, {b:?}) antisymmetry");
            }
        }
    }

    #[test]
    fn empty_strings_compare_equal() {
        let c = GermanCollator::DIN_5007_DICTIONARY;
        assert_eq!(c.compare("", ""), Ordering::Equal);
        assert_eq!(c.compare("", "a"), Ordering::Less);
        assert_eq!(c.compare("a", ""), Ordering::Greater);
    }

    #[test]
    fn char_key_case_folds_ascii_only() {
        let c = GermanCollator::DIN_5007_DICTIONARY;
        // ASCII: 'A' and 'a' produce the same key.
        assert_eq!(c.char_key('A'), c.char_key('a'));
        // Non-ASCII passes through unchanged.
        assert_eq!(c.char_key('ä'), 'ä' as u32);
        assert_eq!(c.char_key('ß'), 'ß' as u32);
    }
}
