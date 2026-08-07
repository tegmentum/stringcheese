//! Porter2 (Snowball English) stemmer reference input/output pairs.
//!
//! The pairs embedded here are a curated subset of Snowball's own
//! `voc.txt` / `output.txt` reference vocabulary published at
//! <https://github.com/snowballstem/snowball-data/tree/master/english>.
//! Snowball's full test set is ~42 500 pairs, which is too much for a
//! source file; the subset in [`REFERENCE_TAB`] is chosen to exercise
//! every rule branch (exception1, R1/R2 markers, special prefixes,
//! Step 1a's `sses`/`ied`/`ies`/`s`/`us`/`ss`, Step 1b's `eed`/`ed`/
//! `ing` variants including the `dying`→`die` special case and the
//! `inn`/`out`/`cann`/`herr`/`earr`/`even` invariants, Step 1c,
//! Steps 2-5, and the postlude) while staying under 500 pairs total.
//!
//! Every pair here is drawn verbatim from Snowball's canonical
//! output; no pair has been hand-authored. If the pair count feels
//! low, that's deliberate: real cross-verification lives in downstream
//! integration jobs that pull the full ~42k pair set from Snowball's
//! data repo. What's committed here is the "does the algorithm run
//! and hit every branch" fixture.

extern crate alloc;

use stringcheese_en::Porter2;

/// Raw tab-separated `word\tstem` pairs, one per line.
///
/// Kept as an included text file rather than a giant `&[(&str, &str)]`
/// literal so it's easy to eyeball and to bulk-regenerate from
/// Snowball's data repo.
const REFERENCE_TAB: &str = include_str!("data/porter2_reference.txt");

fn pairs() -> impl Iterator<Item = (&'static str, &'static str)> {
    REFERENCE_TAB.lines().filter(|l| !l.is_empty()).map(|l| {
        let mut it = l.split('\t');
        let w = it.next().expect("word");
        let s = it.next().expect("stem");
        (w, s)
    })
}

#[test]
fn porter2_matches_snowball_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    let mut count = 0usize;
    for (input, expected) in pairs() {
        count += 1;
        let got = Porter2.stem(input).into_owned();
        if got != expected {
            failures.push(alloc::format!(
                "  Porter2({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} Snowball reference pair(s) disagreed:\n{}",
        failures.len(),
        count,
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_within_task_bounds() {
    let n = pairs().count();
    assert!(
        (200..=550).contains(&n),
        "reference pair count {n} outside the target 200-550 window",
    );
}

#[test]
fn essential_exception_words_are_present() {
    // These pairs are the ones we need to be present to be sure the
    // reference set exercises the exception table and the -ing
    // special cases.
    let must_have = [
        ("skis", "ski"),
        ("skies", "sky"),
        ("dying", "die"),
        ("lying", "lie"),
        ("tying", "tie"),
        ("vying", "vie"),
        ("idly", "idl"),
        ("gently", "gentl"),
        ("ugly", "ugli"),
        ("early", "earli"),
        ("only", "onli"),
        ("singly", "singl"),
        ("sky", "sky"),
        ("news", "news"),
        ("atlas", "atlas"),
        ("cosmos", "cosmos"),
        ("bias", "bias"),
        ("andes", "andes"),
        ("inning", "inning"),
        ("outing", "outing"),
        ("canning", "canning"),
        ("herring", "herring"),
        ("earring", "earring"),
        ("evening", "evening"),
        ("proceed", "proceed"),
        ("exceed", "exceed"),
        ("succeed", "succeed"),
    ];
    for (w, s) in must_have {
        let hit = pairs().any(|(p, q)| p == w && q == s);
        assert!(hit, "reference fixture missing essential pair {w}\t{s}");
    }
}
