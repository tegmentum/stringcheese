//! Fuzz target: `SCUD` writer/reader roundtrip.
//!
//! [`scud_load`](./scud_load.rs) shakes the loader by throwing arbitrary
//! bytes at [`ScudFile::from_slice`] — that catches parser panics on
//! garbage input, but it says nothing about whether the *writer* half
//! of the format ([`ScudWriter`] + the section builders) agrees with
//! the reader half. Field-ordering divergence, length-prefix
//! mis-accounting, or a section-encode / section-decode asymmetry
//! would silently produce a file the reader accepts but re-interprets
//! wrong — no panic, no error, just wrong data. That is the exact
//! class of bug this target is designed to surface.
//!
//! # Strategy
//!
//! The libFuzzer byte stream feeds an [`arbitrary::Unstructured`]
//! reader which derives a small structured input describing:
//!
//! * a CLDR version string and locale tag (both bounded),
//! * a `Vec` of case-mapping pairs `(src, dst)` — populated across
//!   all three simple case tables (lower, upper, fold),
//! * a `Vec` of collation primary-weight overrides `(cp, pw, sw, tw)`,
//! * a collation options blob (strength + three flags),
//! * a `Vec` of cardinal plural-rule entries `(category, rule_id)`.
//!
//! Each capability is exercised as its own SCUD file because a SCUD
//! file carries one `(capability, locale)` tuple — a case file, a
//! collation file, and a plural file per fuzz iteration. Every file
//! is round-tripped independently and each input entry is looked up
//! back out of the parsed view; a mismatch asserts loudly, which is
//! the intentional fuzz signal.
//!
//! # Bounded input
//!
//! Every variable-length field is capped through
//! [`Unstructured::int_in_range`] so libFuzzer cannot grow the input
//! into a multi-megabyte pathological blob that times the target out
//! before ever reaching the encode/decode paths. The bounds below
//! comfortably cover realistic pack shapes (CLDR case data for one
//! locale is a few hundred entries at most) while keeping each
//! iteration well under the libFuzzer per-input time budget.
//!
//! # Section coverage
//!
//! Phase 1 of this target covers a subset: simple case tables (all
//! three), collation options + primary overrides, and plural cardinal
//! rules. The full-mapping tables, plural ordinals, and the other
//! capabilities (number / datetime / break / linebreak) are follow-up
//! work — each capability's builder + reader pair is a self-contained
//! roundtrip target and can be added incrementally.

#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;
use stringcheese_scud::{
    CAP_CASE, CAP_COLLATION, CAP_PLURAL, CaseSectionBuilder, CollationSectionBuilder,
    PluralCategory, PluralSectionBuilder, SECT_CARDINAL_RULES, SECT_COLLATION_OPTIONS,
    SECT_PRIMARY_OVERRIDES, SECT_SIMPLE_FOLD, SECT_SIMPLE_LOWER, SECT_SIMPLE_UPPER, ScudFile,
    ScudWriter,
};

// ---- Bounded input constants ----------------------------------------------
//
// Every Vec length and String size below is capped so libFuzzer cannot grow
// the arbitrary input into a pathological blob. The bounds are generous
// enough to cover realistic SCUD pack shapes (case data for one CLDR locale
// is a few hundred entries; primary overrides are a handful; plural rules
// are at most six).

/// Maximum number of case-mapping pairs generated per iteration. 32 is
/// well above what any realistic locale's tailored simple case table
/// carries and keeps encode/decode work bounded.
const MAX_CASE_PAIRS: usize = 32;

/// Maximum number of collation primary-weight override rows per
/// iteration. Realistic locales (Turkish, Estonian, ...) have single-
/// digit override counts.
const MAX_PRIMARY_OVERRIDES: usize = 32;

/// Maximum number of plural cardinal-rule entries per iteration. CLDR
/// caps plural categories at six (zero, one, two, few, many, other);
/// 8 leaves room for duplicates without unbounded growth.
const MAX_PLURAL_RULES: usize = 8;

/// Maximum byte length of the CLDR-version string. CLDR versions are
/// short (e.g. "44.1"); 32 bytes covers any conceivable encoding.
const MAX_CLDR_LEN: usize = 32;

/// Maximum byte length of the locale tag. BCP 47 tags fit comfortably
/// under 64 bytes (`sr-Cyrl-BA` is 10; long tailorings top out well
/// under this bound).
const MAX_LOCALE_LEN: usize = 64;

/// Upper bound on generated codepoint values — the largest valid
/// Unicode scalar. Restricting `src` to a plausible codepoint range
/// exercises the same wire layout the algorithm crates rely on.
const MAX_CODEPOINT: u32 = 0x0010_FFFF;

// ---- Structured fuzz input -------------------------------------------------

/// One case-mapping row shared by simple lower, upper, and fold tables.
/// A single `(src, dst)` pair is populated into all three tables so a
/// single fuzz-generated entry exercises the encode/decode symmetry
/// across every simple case section.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CasePair {
    src: u32,
    dst: u32,
}

/// One collation primary-weight override row.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PrimaryOverride {
    cp: u32,
    primary: u32,
    secondary: u32,
    tertiary: u32,
}

/// Options-blob fields carried by the collation section.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CollationOptions {
    default_strength: u8,
    case_insensitive: bool,
    backwards_secondary: bool,
    case_second: bool,
}

/// One plural cardinal-rule entry.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PluralEntry {
    category: PluralCategory,
    rule_id: u8,
}

/// The top-level structured fuzz input. `Arbitrary` is implemented by
/// hand so that every variable-length field can be bounded through
/// `Unstructured::int_in_range` (the derive would default to unbounded
/// `Vec::arbitrary`, which libFuzzer would grow into MB-sized blobs).
#[derive(Debug, Clone)]
struct FuzzScudInput {
    cldr_version: String,
    locale: Option<String>,
    case_pairs: Vec<CasePair>,
    primary_overrides: Vec<PrimaryOverride>,
    collation_options: Option<CollationOptions>,
    plural_cardinals: Vec<PluralEntry>,
}

impl<'a> Arbitrary<'a> for FuzzScudInput {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let cldr_version = arb_bounded_ascii(u, MAX_CLDR_LEN)?;
        let has_locale: bool = u.arbitrary()?;
        let locale = if has_locale {
            Some(arb_bounded_ascii(u, MAX_LOCALE_LEN)?)
        } else {
            None
        };

        let case_count = u.int_in_range(0..=MAX_CASE_PAIRS)?;
        let mut case_pairs = Vec::with_capacity(case_count);
        for _ in 0..case_count {
            case_pairs.push(CasePair {
                src: u.int_in_range(0..=MAX_CODEPOINT)?,
                dst: u.arbitrary()?,
            });
        }

        let po_count = u.int_in_range(0..=MAX_PRIMARY_OVERRIDES)?;
        let mut primary_overrides = Vec::with_capacity(po_count);
        for _ in 0..po_count {
            primary_overrides.push(PrimaryOverride {
                cp: u.int_in_range(0..=MAX_CODEPOINT)?,
                primary: u.arbitrary()?,
                secondary: u.arbitrary()?,
                tertiary: u.arbitrary()?,
            });
        }

        let has_options: bool = u.arbitrary()?;
        let collation_options = if has_options {
            Some(CollationOptions {
                // Strength is a u8 on the wire; any value round-trips.
                default_strength: u.arbitrary()?,
                case_insensitive: u.arbitrary()?,
                backwards_secondary: u.arbitrary()?,
                case_second: u.arbitrary()?,
            })
        } else {
            None
        };

        let plural_count = u.int_in_range(0..=MAX_PLURAL_RULES)?;
        let mut plural_cardinals = Vec::with_capacity(plural_count);
        for _ in 0..plural_count {
            plural_cardinals.push(PluralEntry {
                category: arb_plural_category(u)?,
                rule_id: u.arbitrary()?,
            });
        }

        Ok(Self {
            cldr_version,
            locale,
            case_pairs,
            primary_overrides,
            collation_options,
            plural_cardinals,
        })
    }
}

/// Consume a bounded number of ASCII bytes from the fuzz input as a
/// `String`. The SCUD header carries the CLDR version and locale as
/// UTF-8 length-prefixed strings; restricting to ASCII keeps the
/// arbitrary decoder cheap while still exercising the length-prefix
/// path faithfully (every valid ASCII byte is valid UTF-8).
fn arb_bounded_ascii(u: &mut Unstructured<'_>, max_len: usize) -> arbitrary::Result<String> {
    let len = u.int_in_range(0..=max_len)?;
    let mut s = String::with_capacity(len);
    for _ in 0..len {
        // 0x20..=0x7E — printable ASCII, safely UTF-8, avoids control chars.
        let byte: u8 = u.int_in_range(0x20..=0x7E)?;
        s.push(byte as char);
    }
    Ok(s)
}

/// Pick a `PluralCategory` variant uniformly. `PluralCategory` doesn't
/// derive `Arbitrary`, so route through the byte generator + the
/// crate's own `from_u8`.
fn arb_plural_category(u: &mut Unstructured<'_>) -> arbitrary::Result<PluralCategory> {
    let v: u8 = u.int_in_range(0..=5)?;
    // `from_u8` returns `None` only for values > 5; the int_in_range
    // above rules that out.
    PluralCategory::from_u8(v).ok_or(arbitrary::Error::IncorrectFormat)
}

// ---- Roundtrip harnesses ---------------------------------------------------

/// Build a CASE-capability SCUD file from the fuzz input, parse it
/// back, and assert every input pair round-trips through the three
/// simple case tables.
///
/// The writer sorts by `src` inside `encode_pair_table` and the
/// reader binary-searches by `src`; duplicates on `src` collapse (the
/// binary search returns one of the tied entries). To keep the
/// assertion definite, this harness dedupes by `src` before writing
/// and asserts the lookup matches the surviving value.
fn roundtrip_case(input: &FuzzScudInput) {
    // Dedupe by src — later entries win so the assertion below can
    // predict which `dst` the encoder emitted for the surviving key.
    let mut deduped: std::collections::BTreeMap<u32, u32> = std::collections::BTreeMap::new();
    for pair in &input.case_pairs {
        deduped.insert(pair.src, pair.dst);
    }

    let mut builder = CaseSectionBuilder::new();
    for (&src, &dst) in &deduped {
        builder.push_simple_lower(src, dst);
        builder.push_simple_upper(src, dst);
        builder.push_simple_fold(src, dst);
    }

    let mut writer = ScudWriter::new(CAP_CASE, &input.cldr_version, input.locale.as_deref());
    writer.append_section(SECT_SIMPLE_LOWER, &builder.simple_lower_bytes());
    writer.append_section(SECT_SIMPLE_UPPER, &builder.simple_upper_bytes());
    writer.append_section(SECT_SIMPLE_FOLD, &builder.simple_fold_bytes());
    let bytes = writer.finish();

    let file = ScudFile::from_slice(&bytes).expect("case roundtrip: writer produced valid bytes");
    assert_eq!(file.capability(), CAP_CASE);
    assert_eq!(file.cldr_version(), input.cldr_version);
    assert_eq!(file.locale(), input.locale.as_deref());

    let view = file
        .as_case_data()
        .expect("case roundtrip: as_case_data must succeed on CAP_CASE file");

    for (&src, &dst) in &deduped {
        assert_eq!(
            view.simple_lower(src),
            Some(dst),
            "case roundtrip: simple_lower({src}) mismatch"
        );
        assert_eq!(
            view.simple_upper(src),
            Some(dst),
            "case roundtrip: simple_upper({src}) mismatch"
        );
        assert_eq!(
            view.simple_fold(src),
            Some(dst),
            "case roundtrip: simple_fold({src}) mismatch"
        );
    }
}

/// Build a COLL-capability SCUD file from the fuzz input's collation
/// fields, parse it back, and assert every override + option
/// round-trips.
fn roundtrip_collation(input: &FuzzScudInput) {
    // Dedupe by cp for the same reason as the case dedupe above.
    let mut deduped: std::collections::BTreeMap<u32, (u32, u32, u32)> =
        std::collections::BTreeMap::new();
    for row in &input.primary_overrides {
        deduped.insert(row.cp, (row.primary, row.secondary, row.tertiary));
    }

    let mut builder = CollationSectionBuilder::new();
    for (&cp, &(pw, sw, tw)) in &deduped {
        builder.push_primary_override(cp, pw, sw, tw);
    }
    if let Some(opts) = &input.collation_options {
        builder.set_default_strength(opts.default_strength);
        builder.set_case_insensitive(opts.case_insensitive);
        builder.set_backwards_secondary(opts.backwards_secondary);
        builder.set_case_second(opts.case_second);
    }

    let mut writer = ScudWriter::new(CAP_COLLATION, &input.cldr_version, input.locale.as_deref());
    if builder.has_primary_overrides() {
        writer.append_section(SECT_PRIMARY_OVERRIDES, &builder.primary_overrides_bytes());
    }
    if builder.is_options_present() {
        writer.append_section(SECT_COLLATION_OPTIONS, &builder.options_bytes());
    }
    let bytes = writer.finish();

    let file =
        ScudFile::from_slice(&bytes).expect("collation roundtrip: writer produced valid bytes");
    assert_eq!(file.capability(), CAP_COLLATION);

    let view = file
        .as_collation_data()
        .expect("collation roundtrip: as_collation_data must succeed on CAP_COLLATION file");

    for (&cp, &(pw, sw, tw)) in &deduped {
        assert_eq!(
            view.primary_override(cp),
            Some((pw, sw, tw)),
            "collation roundtrip: primary_override({cp}) mismatch"
        );
    }

    if let Some(opts) = &input.collation_options {
        assert_eq!(
            view.default_strength(),
            Some(opts.default_strength),
            "collation roundtrip: default_strength mismatch"
        );
        assert_eq!(
            view.case_insensitive(),
            Some(opts.case_insensitive),
            "collation roundtrip: case_insensitive mismatch"
        );
        assert_eq!(
            view.backwards_secondary(),
            Some(opts.backwards_secondary),
            "collation roundtrip: backwards_secondary mismatch"
        );
        assert_eq!(
            view.case_second(),
            Some(opts.case_second),
            "collation roundtrip: case_second mismatch"
        );
    }
}

/// Build a PLUR-capability SCUD file from the fuzz input's plural
/// entries, parse it back, and assert the cardinal-rule sequence
/// round-trips in order.
fn roundtrip_plural(input: &FuzzScudInput) {
    let mut builder = PluralSectionBuilder::new();
    for entry in &input.plural_cardinals {
        builder.push_cardinal(entry.category, entry.rule_id);
    }

    let mut writer = ScudWriter::new(CAP_PLURAL, &input.cldr_version, input.locale.as_deref());
    writer.append_section(SECT_CARDINAL_RULES, &builder.cardinal_bytes());
    let bytes = writer.finish();

    let file = ScudFile::from_slice(&bytes).expect("plural roundtrip: writer produced valid bytes");
    assert_eq!(file.capability(), CAP_PLURAL);

    let view = file
        .as_plural_data()
        .expect("plural roundtrip: as_plural_data must succeed on CAP_PLURAL file");

    let read_back: Vec<(PluralCategory, u8)> = view.cardinal_rules().collect();
    let expected: Vec<(PluralCategory, u8)> = input
        .plural_cardinals
        .iter()
        .map(|e| (e.category, e.rule_id))
        .collect();
    assert_eq!(
        read_back, expected,
        "plural roundtrip: cardinal-rule sequence mismatch"
    );
}

fuzz_target!(|data: &[u8]| {
    let mut u = Unstructured::new(data);
    let Ok(input) = FuzzScudInput::arbitrary(&mut u) else {
        // Not enough bytes to satisfy the bounded decoders — that's a
        // benign result for a fuzz target, not a failure.
        return;
    };

    roundtrip_case(&input);
    roundtrip_collation(&input);
    roundtrip_plural(&input);
});
