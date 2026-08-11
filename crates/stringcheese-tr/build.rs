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
    CAP_CASE, CaseSectionBuilder, ContextKind, SECT_CONTEXT, SECT_FULL_UPPER, SECT_SIMPLE_LOWER,
    SECT_SIMPLE_UPPER, ScudWriter,
};

/// CLDR version the shipped tables were compiled against.
const CLDR_VERSION: &str = "44.1";

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR set by cargo"));
    let scud_path = out_dir.join("case-tr.scud");
    let scud_bytes = build_case_tr_scud();
    fs::write(&scud_path, &scud_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", scud_path.display()));

    println!("cargo:rerun-if-changed=build.rs");
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
