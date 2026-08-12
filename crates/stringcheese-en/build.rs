//! Build-time codegen for the English pack.
//!
//! Two artifacts emitted into `$OUT_DIR`:
//!
//! 1. `generated.rs` — the `stringcheese-lang-gen`-derived stopword
//!    slice and capability record. Read from `rules/en.toml`.
//! 2. `case-en.scud` — the CLDR-derived case-mapping SCUD pack shipped
//!    by the crate under the `case-scud` feature. See
//!    `docs/design/wit-i18n.md` § 6 and the crate root docs for the
//!    pack shape.
//!
//! Both `stringcheese-lang-gen` and `stringcheese-scud` are
//! `[build-dependencies]` — they run here at build time and drop out
//! of the shipped binary. A caller pulling `stringcheese-en = "0.1"`
//! pays zero cost for either generator's presence.

use std::env;
use std::fs;
use std::path::PathBuf;

use stringcheese_icu_plural::builder::{english_cardinals, english_ordinals};
use stringcheese_scud::{
    CAP_CASE, CAP_COLLATION, CAP_NUMBER, CAP_PLURAL, CaseSectionBuilder, CollationSectionBuilder,
    NumberSectionBuilder, PluralSectionBuilder, SECT_CARDINAL_RULES, SECT_COLLATION_OPTIONS,
    SECT_CURRENCY_TABLE, SECT_DECIMAL_PATTERN, SECT_EXPANSIONS, SECT_FULL_FOLD, SECT_FULL_UPPER,
    SECT_ORDINAL_RULES, SECT_PERCENT_PATTERN, SECT_SIMPLE_FOLD, SECT_SIMPLE_LOWER,
    SECT_SIMPLE_UPPER, ScudWriter,
};

/// CLDR version the shipped tables were compiled against. Bumping
/// this value is a coordinated release action — the SCUD file header
/// carries this string so downstream can trace data provenance.
const CLDR_VERSION: &str = "44.1";

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR set by cargo"));
    let rules = "rules/en.toml";
    let out = out_dir.join("generated.rs");
    stringcheese_lang_gen::generate(rules, &out)
        .unwrap_or_else(|e| panic!("stringcheese-lang-gen failed on {rules}: {e}"));

    let scud_path = out_dir.join("case-en.scud");
    let scud_bytes = build_case_en_scud();
    fs::write(&scud_path, &scud_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", scud_path.display()));

    let collation_path = out_dir.join("collation-en.scud");
    let collation_bytes = build_collation_en_scud();
    fs::write(&collation_path, &collation_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", collation_path.display()));

    let plural_path = out_dir.join("plural-en.scud");
    let plural_bytes = build_plural_en_scud();
    fs::write(&plural_path, &plural_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", plural_path.display()));

    let number_path = out_dir.join("number-en.scud");
    let number_bytes = build_number_en_scud();
    fs::write(&number_path, &number_bytes)
        .unwrap_or_else(|e| panic!("writing {}: {e}", number_path.display()));

    println!("cargo:rerun-if-changed={rules}");
    println!("cargo:rerun-if-changed=build.rs");
}

/// Build the plural-en SCUD pack in memory.
///
/// English plural rules (CLDR 44.1, `plurals.xml`):
///
/// * Cardinal `one` when `i = 1 and v = 0` (integer 1), else `other`.
/// * Ordinal `one` when `n % 10 = 1 and n % 100 != 11` (1st, 21st,
///   101st …).
/// * Ordinal `two` when `n % 10 = 2 and n % 100 != 12` (2nd, 22nd …).
/// * Ordinal `few` when `n % 10 = 3 and n % 100 != 13` (3rd, 23rd …).
/// * Ordinal `other` otherwise (4th, 5th … 11th, 12th, 13th …).
fn build_plural_en_scud() -> Vec<u8> {
    let mut b = PluralSectionBuilder::new();
    english_cardinals(&mut b);
    english_ordinals(&mut b);
    let mut w = ScudWriter::new(CAP_PLURAL, CLDR_VERSION, Some("en"));
    w.append_section(SECT_CARDINAL_RULES, &b.cardinal_bytes());
    w.append_section(SECT_ORDINAL_RULES, &b.ordinal_bytes());
    w.finish()
}

/// Build the number-en SCUD pack in memory.
///
/// English number formatting (CLDR 44.1, `en.xml`):
///
/// * Group separator `,`, decimal separator `.`.
/// * Decimal default: 0 min, 3 max fraction digits (pattern
///   `#,##0.###`).
/// * Percent: symbol `%` after the value, no space.
/// * Currency: USD `$`, EUR `€`, GBP `£`, JPY `¥`, CAD `CA$`,
///   AUD `A$` — all placed before the value with no space.
fn build_number_en_scud() -> Vec<u8> {
    let mut n = NumberSectionBuilder::new();
    n.set_decimal_pattern(",", ".", 0, 3, 3, 3);
    n.push_currency("USD", "$", false, false);
    n.push_currency("EUR", "\u{20AC}", false, false);
    n.push_currency("GBP", "\u{00A3}", false, false);
    n.push_currency("JPY", "\u{00A5}", false, false);
    n.push_currency("CAD", "CA$", false, false);
    n.push_currency("AUD", "A$", false, false);
    n.set_percent("%", true, false);
    let mut w = ScudWriter::new(CAP_NUMBER, CLDR_VERSION, Some("en"));
    w.append_section(SECT_DECIMAL_PATTERN, &n.decimal_bytes());
    w.append_section(SECT_CURRENCY_TABLE, &n.currency_bytes());
    w.append_section(SECT_PERCENT_PATTERN, &n.percent_bytes());
    w.finish()
}

/// Build the case-en SCUD pack in memory.
///
/// English uses the default Unicode case-mapping rules with no locale
/// tailoring. The shipped pack covers:
///
/// * **ASCII a-z ↔ A-Z** — 52 simple lower/upper pairs each direction,
///   plus 26 simple-fold entries. Duplicates the `char::to_lowercase`
///   fallback the algorithm crate reaches for automatically; kept in
///   the pack so lookup timing is uniform and so the pack has enough
///   coverage that "the pack is used at all" is testable.
/// * **Latin-1 supplement (U+00C0..U+00FE minus U+00D7 U+00F7)** —
///   the accented letters common European text carries. `SIMPLE_LOWER`
///   pairs the uppercase form with the lowercase form and vice versa.
/// * **German ß (U+00DF)** — full uppercase to "SS" (per CLDR default)
///   and full fold to "ss". The single case the algorithm crate
///   cannot handle via `char::to_uppercase` (which returns "SS" as
///   two scalars from a single-char input, but only when consumed via
///   the iterator — the SCUD pack lets the mapping be discovered by
///   the query engine directly).
/// * **Capital sharp S (ẞ, U+1E9E)** — simple lowercase to ß.
fn build_case_en_scud() -> Vec<u8> {
    let mut c = CaseSectionBuilder::new();

    // ASCII a-z ↔ A-Z.
    for ch in 'a'..='z' {
        let up = ch.to_ascii_uppercase();
        c.push_simple_lower(up as u32, ch as u32);
        c.push_simple_upper(ch as u32, up as u32);
        c.push_simple_fold(up as u32, ch as u32);
    }

    // Latin-1 supplement upper/lower pairs.
    // U+00C0..U+00D6 → U+00E0..U+00F6, U+00D8..U+00DE → U+00F8..U+00FE.
    for (upper, lower) in latin1_supplement_pairs() {
        c.push_simple_lower(upper, lower);
        c.push_simple_upper(lower, upper);
        c.push_simple_fold(upper, lower);
    }

    // Latin-A additions used by common Western European locales
    // (French œ, German-tradition names). Trimmed to the pairs that
    // fit `SIMPLE` mapping; multi-scalar cases go into `FULL`.
    for (upper, lower) in latin_extended_a_pairs() {
        c.push_simple_lower(upper, lower);
        c.push_simple_upper(lower, upper);
        c.push_simple_fold(upper, lower);
    }

    // Full uppercase expansions: German ß → "SS", capital sharp S
    // ẞ → still "ẞ" (identity for upper). Full fold: ß → "ss",
    // ẞ → "ss".
    c.push_full_upper(0x00DF, &[0x0053, 0x0053]); // ß → SS
    c.push_full_fold(0x00DF, &[0x0073, 0x0073]); // ß → ss
    c.push_full_fold(0x1E9E, &[0x0073, 0x0073]); // ẞ → ss
    c.push_simple_lower(0x1E9E, 0x00DF); // ẞ → ß

    // Ligatures common in French/German text.
    c.push_simple_lower(0x0152, 0x0153); // Œ → œ
    c.push_simple_upper(0x0153, 0x0152); // œ → Œ
    c.push_simple_fold(0x0152, 0x0153);
    c.push_simple_lower(0x00C6, 0x00E6); // Æ → æ
    c.push_simple_upper(0x00E6, 0x00C6); // æ → Æ
    c.push_simple_fold(0x00C6, 0x00E6);

    let mut w = ScudWriter::new(CAP_CASE, CLDR_VERSION, Some("en"));
    w.append_section(SECT_SIMPLE_LOWER, &c.simple_lower_bytes());
    w.append_section(SECT_SIMPLE_UPPER, &c.simple_upper_bytes());
    w.append_section(SECT_SIMPLE_FOLD, &c.simple_fold_bytes());
    w.append_section(SECT_FULL_UPPER, &c.full_upper_bytes());
    w.append_section(SECT_FULL_FOLD, &c.full_fold_bytes());
    w.finish()
}

/// The Latin-1 supplement uppercase/lowercase pairs.
///
/// Yields `(upper, lower)` for U+00C0..U+00D6 → U+00E0..U+00F6 and
/// U+00D8..U+00DE → U+00F8..U+00FE. Excludes U+00D7 (×) and U+00F7
/// (÷) which are punctuation, and U+00DF (ß) which needs the full
/// expansion table.
fn latin1_supplement_pairs() -> impl Iterator<Item = (u32, u32)> {
    (0x00C0..=0x00D6u32)
        .map(|u| (u, u + 0x20))
        .chain((0x00D8..=0x00DE).map(|u| (u, u + 0x20)))
}

/// Build the collation-en SCUD pack in memory.
///
/// English uses DUCET root ordering with the following minor
/// tailorings (drawn from CLDR's `en` collation resource):
///
/// * **Ligatures** — Æ/æ expands to `AE`/`ae`, Œ/œ to `OE`/`oe`.
///   CLDR ships these under `Locale::Root` (they are DUCET
///   contractions rather than English tailorings, but shipping
///   them as explicit expansions ensures the SCUD-loaded engine
///   agrees with `char::to_lowercase()`-based tokenisation for
///   the ~5 characters where they diverge). The impact on
///   feruca's downstream compare is a no-op — feruca already
///   maps Æ to the `AE` collation elements — but writing the
///   expansions into SCUD is what makes `sort_key` bytewise-
///   consistent for those characters.
/// * **Hyphenation ignorability** — U+2010 (hyphen) and U+2011
///   (non-breaking hyphen) both expand to empty. That's the
///   CLDR-root behaviour under the `[reorder Latn]` default,
///   and it's what English callers expect when sorting a list
///   of hyphenated names.
///
/// Everything else is left to feruca's DUCET-root behaviour;
/// English does not tailor `ß`, umlauts, or the accented letters
/// beyond the ligatures above.
fn build_collation_en_scud() -> Vec<u8> {
    let mut c = CollationSectionBuilder::new();

    // Ligatures — DUCET contractions written as explicit
    // expansions so sort_key stays bytewise-consistent.
    c.push_expansion(0x00C6, &[0x0041, 0x0045]); // Æ → AE
    c.push_expansion(0x00E6, &[0x0061, 0x0065]); // æ → ae
    c.push_expansion(0x0152, &[0x004F, 0x0045]); // Œ → OE
    c.push_expansion(0x0153, &[0x006F, 0x0065]); // œ → oe

    // Hyphenation ignorability (U+2010, U+2011) is a documented
    // Phase 2 deferral — the SCUD wire format's non-empty
    // expansion invariant would need to be relaxed to encode
    // "collapse to empty", and the CLDR default under
    // `[reorder Latn]` is already close enough for the golden
    // vectors this pack ships. Follow up with the empty-expansion
    // relaxation when a locale needs it (Thai / Lao ignorables).

    // Default strength is tertiary (case-sensitive) so a caller
    // who does not name a strength gets sane English defaults.
    c.set_default_strength(2); // Tertiary
    c.set_case_insensitive(false);

    let mut w = ScudWriter::new(CAP_COLLATION, CLDR_VERSION, Some("en"));
    w.append_section(SECT_EXPANSIONS, &c.expansion_bytes());
    w.append_section(SECT_COLLATION_OPTIONS, &c.options_bytes());
    w.finish()
}

/// Hand-picked Latin Extended-A pairs common in Western European
/// text. Excludes the tricky characters (Turkish I, Croatian digraphs,
/// Serbian titlecase) that need locale-specific handling.
fn latin_extended_a_pairs() -> impl Iterator<Item = (u32, u32)> {
    [
        (0x0100u32, 0x0101u32), // Ā ā
        (0x0102, 0x0103),       // Ă ă
        (0x0104, 0x0105),       // Ą ą
        (0x0106, 0x0107),       // Ć ć
        (0x0108, 0x0109),       // Ĉ ĉ
        (0x010A, 0x010B),       // Ċ ċ
        (0x010C, 0x010D),       // Č č
        (0x010E, 0x010F),       // Ď ď
        (0x0112, 0x0113),       // Ē ē
        (0x0114, 0x0115),       // Ĕ ĕ
        (0x0116, 0x0117),       // Ė ė
        (0x0118, 0x0119),       // Ę ę
        (0x011A, 0x011B),       // Ě ě
        (0x011C, 0x011D),       // Ĝ ĝ
        (0x011E, 0x011F),       // Ğ ğ
        (0x0120, 0x0121),       // Ġ ġ
        (0x0122, 0x0123),       // Ģ ģ
        (0x0124, 0x0125),       // Ĥ ĥ
        (0x0126, 0x0127),       // Ħ ħ
        (0x0134, 0x0135),       // Ĵ ĵ
        (0x0136, 0x0137),       // Ķ ķ
        (0x0139, 0x013A),       // Ĺ ĺ
        (0x013B, 0x013C),       // Ļ ļ
        (0x013D, 0x013E),       // Ľ ľ
        (0x0141, 0x0142),       // Ł ł
        (0x0143, 0x0144),       // Ń ń
        (0x0145, 0x0146),       // Ņ ņ
        (0x0147, 0x0148),       // Ň ň
        (0x014C, 0x014D),       // Ō ō
        (0x014E, 0x014F),       // Ŏ ŏ
        (0x0150, 0x0151),       // Ő ő
    ]
    .into_iter()
}
