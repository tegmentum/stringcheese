//! Reference values for the Korean coarse particle stripper.
//!
//! Every test in this file pins one advertised behavior of
//! [`stringcheese_ko::KoreanStemmer`]. If a rule table entry changes
//! the corresponding test flips red; keep the tests aligned with the
//! module docs.

use stringcheese_ko::KoreanStemmer;

fn s(w: &str) -> String {
    KoreanStemmer.stem(w).into_owned()
}

// -----------------------------------------------------------------
// Single case-particle strips.
// -----------------------------------------------------------------

#[test]
fn topic_marker_neun() {
    assert_eq!(s("나는"), "나");
}

#[test]
fn topic_marker_eun() {
    assert_eq!(s("책은"), "책");
}

#[test]
fn subject_marker_i() {
    assert_eq!(s("사람이"), "사람");
}

#[test]
fn subject_marker_ga() {
    assert_eq!(s("친구가"), "친구");
}

#[test]
fn object_marker_eul() {
    assert_eq!(s("책을"), "책");
}

#[test]
fn object_marker_reul() {
    assert_eq!(s("나를"), "나");
}

#[test]
fn locative_e() {
    assert_eq!(s("집에"), "집");
}

#[test]
fn ablative_eseo_beats_shorter_e() {
    // Longest-match ordering: `-에서` trips before `-에`. Without the
    // ordering, the stemmer would strip `-에` off `학교에서` and leave
    // a stray `서` — a common bug in Korean suffix strippers.
    assert_eq!(s("학교에서"), "학교");
}

#[test]
fn allative_kkaji() {
    assert_eq!(s("여기까지"), "여기");
}

#[test]
fn ablative_time_buteo() {
    assert_eq!(s("지금부터"), "지금");
}

#[test]
fn dative_ege() {
    assert_eq!(s("친구에게"), "친구");
}

#[test]
fn instrumental_euro() {
    assert_eq!(s("연필으로"), "연필");
}

#[test]
fn genitive_ui() {
    assert_eq!(s("나의"), "나");
}

#[test]
fn focus_do() {
    assert_eq!(s("나도"), "나");
}

#[test]
fn focus_man() {
    assert_eq!(s("너만"), "너");
}

// -----------------------------------------------------------------
// Agglutinative fixed-point.
// -----------------------------------------------------------------

#[test]
fn peels_locative_plus_focus_stack() {
    // 학교에서도 = 학교 + -에서 + -도. Two rounds of stripping.
    assert_eq!(s("학교에서도"), "학교");
}

#[test]
fn peels_dative_plus_focus_stack() {
    // 친구에게도 = 친구 + -에게 + -도. Two rounds of stripping.
    assert_eq!(s("친구에게도"), "친구");
}

// -----------------------------------------------------------------
// Contract.
// -----------------------------------------------------------------

#[test]
fn refuses_to_produce_empty_stem() {
    // Stripping `-는` off the bare word `는` would leave "". The
    // `word.len() > suffix.len()` guard prevents it.
    assert_eq!(s("는"), "는");
    assert_eq!(s("을"), "을");
    assert_eq!(s("에서"), "에서");
    assert_eq!(s(""), "");
}

#[test]
fn non_korean_input_is_returned_unchanged() {
    assert_eq!(s("running"), "running");
    assert_eq!(s("hello world"), "hello world");
    assert_eq!(s("사람"), "사람"); // bare noun, no particle
}

#[test]
fn idempotent() {
    for w in ["나는", "학교에서도", "친구가", "친구에게도", "책을"] {
        let once = KoreanStemmer.stem(w).into_owned();
        let twice = KoreanStemmer.stem(&once).into_owned();
        assert_eq!(once, twice, "stem not idempotent on {w:?}");
    }
}
