//! # Hello, stringcheese
//!
//! The smallest possible use of the umbrella crate: one distance
//! computation, one line of output. If this compiles and prints
//! `distance = 3`, the install is working.
//!
//! Run: `cargo run --example hello -p stringcheese`

use stringcheese::DistanceMetric;
use stringcheese::compare::Levenshtein;

fn main() {
    let d = Levenshtein.distance("kitten".as_bytes(), "sitting".as_bytes());
    println!("Levenshtein(\"kitten\", \"sitting\") distance = {d}");
}
