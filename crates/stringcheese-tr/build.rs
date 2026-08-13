//! Build-time codegen for the Turkish pack's case-tr.scud pack.
//!
//! Emits `$OUT_DIR/case-tr.scud` — the WIT-i18n Phase 1
//! case-mapping SCUD blob shipped by the crate under the `case-scud`
//! feature. Runs `stringcheese-scud::ScudWriter` at build time; the
//! shipped binary contains only the encoded blob, not the writer
//! machinery.
//!
//! See `docs/design/wit-i18n.md` § 6 (per-crate structure) and § 4
//! (SCUD wire format).

use std::env;
use std::fs;
use std::path::PathBuf;

use stringcheese_scud::{
    CAP_CASE, CAP_COLLATION, CaseSectionBuilder, CollationSectionBuilder, ContextKind,
    SECT_COLLATION_OPTIONS, SECT_CONTEXT, SECT_EXPANSIONS, SECT_FULL_UPPER, SECT_PRIMARY_OVERRIDES,
    SECT_SIMPLE_LOWER, SECT_SIMPLE_UPPER, ScudWriter,
};

/// CLDR version the shipped tables were compiled against.
const CLDR_VERSION: &str = "44.1";

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR set by cargo"));
    let scud_path = out_dir.join("case-tr.scud");
    let scud_bytes = build_case_tr_scud();
    fs::write(&scud_path, &scud_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", scud_path.display()));

    let coll_path = out_dir.join("collation-tr.scud");
    let coll_bytes = build_collation_tr_scud();
    fs::write(&coll_path, &coll_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", coll_path.display()));

    println!("cargo:rerun-if-changed=build.rs");
}

/// Build the collation-tr SCUD pack in memory.
///
/// Turkish collation orders the alphabet as
/// `a b c ç d e f g ğ h ı i j k l m n o ö p r s ş t u ü v y z`.
/// The distinctive tailoring is the **primary-distinct** ordering
/// of dotless `ı` before dotted `i` — default UCA treats the two
/// as primary-equal, tertiary-distinct. CLDR Turkish weighs
/// `ı` between `h` and `i` at the primary level.
///
/// The pack ships primary-weight override rows for the full
/// Turkish lowercase alphabet via
/// [`SECT_PRIMARY_OVERRIDES`](stringcheese_scud::SECT_PRIMARY_OVERRIDES).
/// The `stringcheese-icu-collation` engine picks up the overrides
/// and ranks characters by their tabled primary weight rather than
/// DUCET root at compare / sort-key time. Characters outside the
/// override table (digits, punctuation, non-Turkish letters) fall
/// back to their ASCII-lowercased codepoint as an approximation.
///
/// # Coverage
///
/// * **Primary overrides** — every letter of the Turkish alphabet
///   (a, b, c, ç, d, e, f, g, ğ, h, ı, i, j, k, l, m, n, o, ö, p,
///   r, s, ş, t, u, ü, v, y, z), assigned monotonically-increasing
///   primary weights in dictionary order.
/// * **ß → ss expansions** — belt-and-braces so Turkish text
///   quoting a German loanword (`Straße`) collates identically to
///   the English/German packs.
/// * **Default strength tertiary** — case-sensitive.
fn build_collation_tr_scud() -> Vec<u8> {
    let mut c = CollationSectionBuilder::new();

    // German ß expansion — uniform with en/de.
    c.push_expansion(0x00DF, &[0x0073, 0x0073]); // ß → ss
    c.push_expansion(0x1E9E, &[0x0053, 0x0053]); // ẞ → SS

    // Primary overrides — the Turkish alphabet in dictionary
    // order. Weights are monotonic and leave gaps for future
    // additions.
    //
    // Uppercase letters resolve via the engine's
    // ASCII-lowercase-then-lookup rule (see
    // `overridden_key`); non-ASCII uppercase like Ç / Ğ / Ş
    // additionally ship explicit rows so the fold is
    // codepoint-exact.
    let alphabet: &[(u32, u32)] = &[
        (u32::from(b'a'), 100),
        (u32::from(b'b'), 110),
        (u32::from(b'c'), 120),
        (0x00E7, 130), // ç
        (u32::from(b'd'), 140),
        (u32::from(b'e'), 150),
        (u32::from(b'f'), 160),
        (u32::from(b'g'), 170),
        (0x011F, 180), // ğ
        (u32::from(b'h'), 190),
        (0x0131, 200), // ı (dotless-i, between h and i)
        (u32::from(b'i'), 210),
        (u32::from(b'j'), 220),
        (u32::from(b'k'), 230),
        (u32::from(b'l'), 240),
        (u32::from(b'm'), 250),
        (u32::from(b'n'), 260),
        (u32::from(b'o'), 270),
        (0x00F6, 280), // ö
        (u32::from(b'p'), 290),
        (u32::from(b'r'), 300),
        (u32::from(b's'), 310),
        (0x015F, 320), // ş
        (u32::from(b't'), 330),
        (u32::from(b'u'), 340),
        (0x00FC, 350), // ü
        (u32::from(b'v'), 360),
        (u32::from(b'y'), 370),
        (u32::from(b'z'), 380),
    ];
    for (cp, weight) in alphabet {
        c.push_primary_override(*cp, *weight, 0, 0);
    }
    // İ (dotted capital I, U+0130) — tie with lowercase `i`
    // (U+0069) at primary. The ASCII fold path already handles
    // uppercase ASCII → lowercase; the non-ASCII Turkish
    // uppercase pair is explicit.
    c.push_primary_override(0x0130, 210, 0, 0);

    // Default strength tertiary (case + diacritics + base).
    c.set_default_strength(2);
    c.set_case_insensitive(false);

    let mut w = ScudWriter::new(CAP_COLLATION, CLDR_VERSION, Some("tr"));
    w.append_section(SECT_EXPANSIONS, &c.expansion_bytes());
    w.append_section(SECT_COLLATION_OPTIONS, &c.options_bytes());
    w.append_section(SECT_PRIMARY_OVERRIDES, &c.primary_overrides_bytes());
    w.finish()
}

/// Build the case-tr SCUD pack in memory.
///
/// Turkish tailors two orthographic distinctions on Latin `I`/`i`:
///
/// * `I` (U+0049) — dotless capital — lowers to `ı` (U+0131), *not*
///   to `i` (U+0069) as default Unicode folding says.
/// * `i` (U+0069) — dotted lowercase — uppers to `İ` (U+0130), *not*
///   to `I` as default folding says.
/// * `İ` (U+0130) — dotted capital — lowers to `i`.
/// * `ı` (U+0131) — dotless lowercase — uppers to `I`.
///
/// The first two are the *contextual* overrides — they diverge from
/// default Unicode behaviour and are the locale-specific tailoring
/// the pack ships. The last two land in the simple upper/lower
/// tables because default Unicode already handles them correctly and
/// the pack's presence should not change the behaviour.
///
/// This pack does *not* re-ship the ASCII lower/upper tables that
/// `stringcheese-en` carries; a caller who composes both packs falls
/// back through the CLDR chain (`tr → ""`) and picks up the ASCII
/// tables from the English pack or from Rust's `char::to_lowercase`.
fn build_case_tr_scud() -> Vec<u8> {
    let mut c = CaseSectionBuilder::new();

    // Contextual mappings — the two whose divergence from default
    // Unicode is the whole point of shipping a Turkish pack.
    // Kind = LocaleOverrideLower: `I → ı` under Turkish locale.
    c.push_context('I' as u32, ContextKind::LocaleOverrideLower, 0x0131);
    // Kind = LocaleOverrideUpper: `i → İ` under Turkish locale.
    c.push_context('i' as u32, ContextKind::LocaleOverrideUpper, 0x0130);

    // Simple mappings — round-trip the dotted / dotless capital pair.
    // These agree with default Unicode; kept here so the pack's
    // `simple_lower` / `simple_upper` tables cover the full pair
    // symmetrically.
    c.push_simple_lower(0x0130, 0x0069); // İ → i
    c.push_simple_upper(0x0131, 0x0049); // ı → I

    // Turkish letters that already fold correctly under default
    // Unicode — Ç, Ğ, Ö, Ş, Ü. Included so a caller who queries
    // Turkish uppercase / lowercase for these letters gets a
    // pack-hit rather than falling through to `char::to_*case`.
    for (upper, lower) in [
        (0x00C7u32, 0x00E7u32), // Ç ç
        (0x011E, 0x011F),       // Ğ ğ
        (0x00D6, 0x00F6),       // Ö ö
        (0x015E, 0x015F),       // Ş ş
        (0x00DC, 0x00FC),       // Ü ü
    ] {
        c.push_simple_lower(upper, lower);
        c.push_simple_upper(lower, upper);
    }

    // German ß expansion — a pack that never sees German text still
    // ships this row so the composed engine handles ß correctly
    // regardless of which pack the query resolves through first.
    // Cheap belt-and-braces; keeps the composed component's behaviour
    // uniform.
    c.push_full_upper(0x00DF, &[0x0053, 0x0053]);

    let mut w = ScudWriter::new(CAP_CASE, CLDR_VERSION, Some("tr"));
    w.append_section(SECT_CONTEXT, &c.context_bytes());
    w.append_section(SECT_SIMPLE_LOWER, &c.simple_lower_bytes());
    w.append_section(SECT_SIMPLE_UPPER, &c.simple_upper_bytes());
    w.append_section(SECT_FULL_UPPER, &c.full_upper_bytes());
    w.finish()
}
