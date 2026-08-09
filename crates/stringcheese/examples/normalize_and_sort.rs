//! # Normalise display names into stable search keys and sort naturally
//!
//! Directory listings, autocomplete indexes, and de-duplication
//! flows all share the same problem: "Café", "CAFÉ", and "café"
//! must collapse to a single key, and "file2" must sort before
//! "file10". This example shows the two-step composition:
//!
//! 1. **Key** — `normalize::search_key` builds the fuzzy-match
//!    key: NFKC → case-fold → strip-diacritics → canonicalise
//!    punctuation → collapse whitespace → trim.
//! 2. **Sort** — `NaturalCollator<AsciiCiCollator>` orders the
//!    keys numerically-aware so `v2 < v10`.
//!
//! Run: `cargo run --example normalize_and_sort`

use stringcheese::collate::{AsciiCiCollator, Collator, NaturalCollator};
use stringcheese::normalize;

fn main() {
    // Mixed input — different capitalisations, diacritics, smart
    // quotes, and version numbers embedded in filenames.
    let raw = [
        "\u{201C}Café Résumé v10\u{201D}",
        "cafe resume v2",
        "CAFE RESUME V1",
        "Naïve file v100",
        "naive file v20",
        "naive file v3",
    ];

    // Step 1: normalise every entry into its search key.
    let with_keys: Vec<(String, &str)> =
        raw.iter().map(|s| (normalize::search_key(s), *s)).collect();

    // Step 2: sort by key with natural collation.
    let collator = NaturalCollator::new(AsciiCiCollator::new());
    let mut sorted = with_keys;
    sorted.sort_by(|(ka, _), (kb, _)| collator.compare(ka, kb));

    println!("Sorted display names with search keys:\n");
    println!("  {:<28}  display name", "search key");
    println!("  {:<28}  ------------", "----------");
    for (key, display) in &sorted {
        println!("  {key:<28}  {display}");
    }

    println!();
    println!("Notes:");
    println!("  • \"Café Résumé\" / \"cafe resume\" / \"CAFE RESUME\" collapse to one key");
    println!("    (case folding + diacritic stripping in search_key).");
    println!("  • v2, v3, v20, v100 sort numerically (NaturalCollator), not lexicographically.");
}
