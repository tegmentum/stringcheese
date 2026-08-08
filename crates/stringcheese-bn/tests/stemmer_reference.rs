//! Light Bengali stemmer reference input/output pairs.
//!
//! Each pair below has been traced by hand through the stemmer's
//! suffix table (see [`stringcheese_bn::stemmer`] module docs). The
//! stemmer strips one suffix per call, longest-match-wins, with the
//! over-strip safeguard rolling back any strip that would leave
//! fewer than 2 characters (scalars — Bengali letters are 3 bytes
//! each, so "2 characters" is typically 6 bytes).

extern crate alloc;

use stringcheese_bn::LightBengaliStemmer;

/// Reference pairs. 15+ pairs covering the stemmer's rule categories.
const PAIRS: &[(&str, &str)] = &[
    // -----------------------------------------------------------------
    // Plural markers.
    // -----------------------------------------------------------------
    ("বইগুলি", "বই"),   // books (formal) — strip গুলি
    ("বইগুলো", "বই"),   // books (colloquial) — strip গুলো
    ("বাড়িগুলি", "বাড়ি"), // houses (formal)
    ("বাড়িগুলো", "বাড়ি"), // houses (colloquial)
    // -----------------------------------------------------------------
    // Plural genitive.
    // -----------------------------------------------------------------
    ("ছেলেদের", "ছেলে"), // of the boys — strip দের
    ("মেয়েদের", "মেয়ে"), // of the girls
    // -----------------------------------------------------------------
    // Animate plural nominative.
    // -----------------------------------------------------------------
    ("ছেলেরা", "ছেলে"), // the boys — strip রা
    ("মেয়েরা", "মেয়ে"), // the girls
    // -----------------------------------------------------------------
    // Case markers.
    // -----------------------------------------------------------------
    ("ছেলেকে", "ছেলে"), // to the boy — strip কে
    ("বাড়িকে", "বাড়ি"),   // to the house
    ("বাড়িতে", "বাড়ি"),   // at home — strip তে
    // -----------------------------------------------------------------
    // Genitive.
    // -----------------------------------------------------------------
    ("ছেলের", "ছেলে"), // of the boy — strip র
    ("বাড়ির", "বাড়ি"),   // of the house
    // -----------------------------------------------------------------
    // Longest-match wins: দের beats bare র.
    // -----------------------------------------------------------------
    ("মেয়েদের", "মেয়ে"),
    // -----------------------------------------------------------------
    // Bare noun — no suffix, pass through.
    // -----------------------------------------------------------------
    ("বাংলা", "বাংলা"),
    // -----------------------------------------------------------------
    // Over-strip guard / short input.
    // -----------------------------------------------------------------
    ("কে", "কে"), // suffix alone — stripping would leave 0 chars, guarded
    ("র", "র"),   // single-scalar alone — length short-circuit
    // -----------------------------------------------------------------
    // Non-Bengali — no-op.
    // -----------------------------------------------------------------
    ("hello", "hello"),
];

#[test]
fn light_bengali_stemmer_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = LightBengaliStemmer.stem(input).into_owned();
        if got != expected {
            failures.push(alloc::format!(
                "  LightBengaliStemmer({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} Bengali stemmer reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // The task spec asks for "15+ pairs".
    assert!(
        PAIRS.len() >= 15,
        "reference pair count {} is below the 15-pair floor",
        PAIRS.len()
    );
}
