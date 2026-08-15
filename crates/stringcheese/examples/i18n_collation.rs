//! # WIT-i18n German phonebook collation
//!
//! Sort a small list of German words using the DIN 5007-2 phonebook
//! collation pack. Under phonebook order the umlaut `\u{00E4}` expands
//! to `ae` for weighting, so `B\u{00E4}r` collates equal to `Baer`
//! and sorts BEFORE `Bar`. A byte-order sort would put `Bar` first
//! and file `B\u{00E4}r` (multi-byte UTF-8) somewhere entirely
//! different — the pack is what carries the German-specific rule.
//!
//! SCUD pack loaded: `collation-de`
//! (`stringcheese-de::collation_data`).
//!
//! Run: `cargo run --example i18n_collation -p stringcheese --all-features`
use stringcheese_icu_collation::{CollationEngine, CollationStrength};

fn main() {
    let engine = CollationEngine::new(vec![
        stringcheese_de::collation_data::collation_pack().expect("collation-de pack loads"),
    ]);

    let mut words = vec!["B\u{00E4}r", "Bar", "Baa", "Baer", "Straße", "Strasse"];

    println!("Input order (as declared):");
    for w in &words {
        println!("  {w}");
    }
    println!();

    // Sort with the German pack at tertiary strength.
    words.sort_by(|a, b| engine.compare(a, b, "de", CollationStrength::Tertiary));

    println!("Sorted under de (DIN 5007-2 phonebook, tertiary):");
    for w in &words {
        println!("  {w}");
    }
    println!();

    // Highlight two comparisons in isolation so the semantics are
    // obvious: umlaut expands to `ae`, so B\u{00E4}r < Bar; sharp-s
    // expands to `ss`, so Straße == Strasse.
    let pairs = [
        ("B\u{00E4}r", "Bar"),
        ("B\u{00E4}r", "Baer"),
        ("Straße", "Strasse"),
    ];
    println!("{:<10}  {:<10}  de-compare", "left", "right");
    println!("{:<10}  {:<10}  ----------", "----", "-----");
    for (a, b) in pairs {
        let ord = engine.compare(a, b, "de", CollationStrength::Tertiary);
        println!("{a:<10}  {b:<10}  {ord:?}");
    }
}
