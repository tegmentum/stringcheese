//! # WIT-i18n case-mapping across locales
//!
//! Load the English + German + Turkish `CasePack`s and drive a single
//! `CaseEngine` across all three. The output demonstrates locale-
//! tailored case-mapping — most visibly the Turkish dotted / dotless-I
//! rule (`i` -> `İ`, `I` -> `ı`) that diverges from the default
//! Unicode mappings every non-Turkic locale keeps.
//!
//! SCUD packs loaded: `case-en` (`stringcheese-en::case_data`),
//! `case-de` (`stringcheese-de::case_data`), `case-tr`
//! (`stringcheese-tr::case_data`).
//!
//! Run: `cargo run --example i18n_case -p stringcheese --all-features`
use stringcheese_icu_case::CaseEngine;

fn main() {
    // Compose one engine over three per-locale packs. Query-time
    // fallback picks the pack whose BCP 47 tag matches the query.
    let engine = CaseEngine::new(vec![
        stringcheese_en::case_data::case_pack().expect("case-en pack loads"),
        stringcheese_de::case_data::case_pack().expect("case-de pack loads"),
        stringcheese_tr::case_data::case_pack().expect("case-tr pack loads"),
    ]);

    // Each row: (locale, input, why it demonstrates locale tailoring).
    let cases = [
        ("en", "istanbul", "default Unicode: i -> I"),
        (
            "tr",
            "istanbul",
            "Turkish tailoring: i -> \u{0130} (dotted I)",
        ),
        ("en", "ISTANBUL", "default Unicode: I -> i"),
        (
            "tr",
            "ISTANBUL",
            "Turkish tailoring: I -> \u{0131} (dotless i)",
        ),
        (
            "de",
            "stra\u{00DF}e",
            "German \u{00DF} -> SS under full-uppercase",
        ),
        ("de", "M\u{00E4}dchen", "German umlaut roundtrip"),
    ];

    println!("{:<8}  {:<12}  {:<12}  note", "locale", "input", "to_upper");
    println!("{:<8}  {:<12}  {:<12}  ----", "------", "-----", "--------");
    for (locale, input, note) in cases {
        let upper = engine.to_upper(input, locale);
        println!("{locale:<8}  {input:<12}  {upper:<12}  {note}");
    }
    println!();

    // Same shape, but showing to_lower on the mixed-case forms.
    println!("{:<8}  {:<12}  {:<12}", "locale", "input", "to_lower");
    println!("{:<8}  {:<12}  {:<12}", "------", "-----", "--------");
    for (locale, input) in [
        ("en", "ISTANBUL"),
        ("tr", "ISTANBUL"),
        ("de", "M\u{00DC}NCHEN"),
    ] {
        let lower = engine.to_lower(input, locale);
        println!("{locale:<8}  {input:<12}  {lower:<12}");
    }
}
