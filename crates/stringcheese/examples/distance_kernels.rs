//! # Distance and similarity kernels side by side
//!
//! The same input string pair fed through four kernels — Levenshtein
//! edit distance, Hamming distance (equal-length only), Jaro, and
//! Jaro-Winkler — so you can see how each answers a different
//! question about "how close are these two strings?".
//!
//! Run: `cargo run --example distance_kernels -p stringcheese`

use stringcheese::compare::{Hamming, Jaro, JaroWinkler, Levenshtein};
use stringcheese::{DistanceMetric, SimilarityMetric};

fn main() {
    let pairs: &[(&str, &str)] = &[
        ("MARTHA", "MARHTA"),
        ("kitten", "sitting"),
        ("robert", "rupert"),
        ("abcdef", "abcxef"),
    ];

    println!(
        "{:<12} {:<12}  {:>4}  {:>7}  {:>7}  {:>7}",
        "left", "right", "lev", "jaro", "jw", "hamm"
    );
    println!(
        "{:<12} {:<12}  {:>4}  {:>7}  {:>7}  {:>7}",
        "----", "-----", "---", "----", "--", "----"
    );
    for (a, b) in pairs {
        let lev = Levenshtein.distance(a.as_bytes(), b.as_bytes());
        let jaro = Jaro.similarity(a.as_bytes(), b.as_bytes());
        // The classic Winkler-1990 parameters (prefix length 4,
        // prefix scale 0.1, no boost threshold).
        let jw = JaroWinkler::classic().similarity(a.as_bytes(), b.as_bytes());
        let hamm = Hamming
            .try_distance(a.as_bytes(), b.as_bytes())
            .ok()
            .map_or_else(|| "n/a".into(), |d| d.into_inner().to_string());
        println!(
            "{:<12} {:<12}  {:>4}  {:>7.3}  {:>7.3}  {:>7}",
            a,
            b,
            lev.into_inner(),
            jaro.into_inner(),
            jw.into_inner(),
            hamm,
        );
    }
    println!();
    println!("Notes:");
    println!("  * lev  = Levenshtein edit distance (insert/delete/replace)");
    println!("  * jaro = Jaro similarity in [0, 1] — 1 is identical");
    println!("  * jw   = Jaro-Winkler (Jaro + prefix bonus)");
    println!("  * hamm = Hamming distance, only defined when lengths match");
}
