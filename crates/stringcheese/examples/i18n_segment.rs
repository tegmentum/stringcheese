//! # WIT-i18n Japanese word segmentation via a CJK dictionary pack
//!
//! Japanese text has no whitespace between words, so the UAX #29
//! default word-break rules give one segment per ideograph. Load the
//! `word-dict-ja` SCUD pack (a hand-curated starter dictionary of
//! ~500 common Japanese terms) into a `BreakEngine`, then run
//! forward-maximum-match segmentation over a short sentence:
//!
//! * Input: "私は学生です" (romaji: "watashi wa gakusei desu")
//! * Expected segments: [私, は, 学生, です]
//!
//! SCUD pack loaded: `word-dict-ja`
//! (`stringcheese-ja::word_dict_data`).
//!
//! Run: `cargo run --example i18n_segment -p stringcheese --all-features`
use stringcheese_icu_segment::BreakEngine;

fn main() {
    let pack = stringcheese_ja::word_dict_data::break_pack().expect("word-dict-ja pack loads");
    let engine = BreakEngine::with_pack(pack);

    let text = "\u{79C1}\u{306F}\u{5B66}\u{751F}\u{3067}\u{3059}";
    println!("Input : {text}");
    println!("Locale: ja");
    println!();

    let segments = engine.segment_words(text, "ja");
    println!("Forward-maximum-match segments ({} total):", segments.len());
    for (idx, seg) in segments.iter().enumerate() {
        let start = seg.start as usize;
        let end = seg.end as usize;
        let slice = &text[start..end];
        let kind = if seg.is_word_like { "word" } else { "non-word" };
        println!("  #{idx}  {start:>2}..{end:<2}  {kind:<8}  {slice}");
    }
    println!();

    // Show that WITHOUT the pack the same input segments per-ideograph
    // — the whole point of the dictionary tailoring.
    let default_engine = BreakEngine::new();
    let default_segments = default_engine.segment_words(text, "ja");
    println!(
        "Default UAX #29 (no pack) segments: {} (one per ideograph, no dictionary tailoring)",
        default_segments.len()
    );
}
