//! # Identifier case conversion
//!
//! Convert identifier strings between `snake_case`, `camelCase`,
//! `PascalCase`, `kebab-case`, `SCREAMING_SNAKE_CASE`, and
//! `Train-Case` using the shared `Case` enum. Also demonstrates
//! `Case::detect` for best-effort classification.
//!
//! Run: `cargo run --example case_conversion -p stringcheese`

use stringcheese::ident::{Case, to_case};

fn main() {
    let inputs = [
        "user_id",
        "getUserName",
        "HTTPRequestParser",
        "kebab-case-name",
    ];

    let targets = [
        ("snake", Case::Snake),
        ("SCREAM_SNAKE", Case::ScreamingSnake),
        ("kebab", Case::Kebab),
        ("Train", Case::Train),
        ("camel", Case::Camel),
        ("Pascal", Case::Pascal),
    ];

    for input in inputs {
        println!("input:  {input}");
        println!("  detected: {:?}", Case::detect(input));
        for (label, target) in &targets {
            println!("  {label:<13} -> {}", to_case(input, *target));
        }
        println!();
    }
}
