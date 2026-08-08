//! Integration tests for the Georgian [`Language`] implementation.

use stringcheese_ka::GEORGIAN;
use stringcheese_lang::Language;

#[test]
fn code_and_name() {
    assert_eq!(GEORGIAN.code(), "ka");
    assert_eq!(GEORGIAN.name(), "Georgian");
}

#[test]
fn common_georgian_words_are_stopwords() {
    for w in [
        "და",
        "ან",
        "მაგრამ",
        "მე",
        "შენ",
        "ის",
        "ჩვენ",
        "თქვენ",
        "არის",
    ] {
        assert!(GEORGIAN.is_stopword(w), "expected {w:?} to be a stopword");
    }
}

#[test]
fn stopword_lookup_folds_mtavruli_to_mkhedruli() {
    // Mtavruli is Unicode 11's capitalized-Mkhedruli block. Rust's
    // default Unicode lowercase folds every Mtavruli scalar to its
    // Mkhedruli counterpart, so a Mtavruli-cased query should match
    // the Mkhedruli-cased stopword list.
    // ᲓᲐ is Mtavruli "და" ("and"). If this assertion fails on your
    // Rust version, the Unicode 11 Mtavruli case pairs are missing
    // from the standard library's tables.
    assert!(
        GEORGIAN.is_stopword("ᲓᲐ"),
        "Mtavruli ᲓᲐ should case-fold to Mkhedruli და"
    );
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["წიგნი", "სახლი", "კომპიუტერი", "პროგრამა"]
    {
        assert!(!GEORGIAN.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_georgian_sentence() {
    let text = "გამარჯობა, მსოფლიო! თბილისი — დედაქალაქი საქართველოსი.";
    let toks: Vec<&str> = GEORGIAN.tokenize(text).collect();
    assert_eq!(
        toks,
        [
            "გამარჯობა",
            "მსოფლიო",
            "თბილისი",
            "დედაქალაქი",
            "საქართველოსი"
        ]
    );
}

#[test]
fn stem_a_few_georgian_words() {
    // See the `stemmer_reference` test for the wider set.
    assert_eq!(GEORGIAN.stem("წიგნები"), "წიგნ");
    assert_eq!(GEORGIAN.stem("წიგნის"), "წიგნ");
    assert_eq!(GEORGIAN.stem("სახლში"), "სახლ");
    assert_eq!(GEORGIAN.stem("კაცმა"), "კაც");
}

#[test]
fn phonetic_encoder_is_phonex_ka() {
    let enc = GEORGIAN
        .phonetic_encoder()
        .expect("Georgian pack ships a phonetic encoder");
    assert_eq!(enc.name(), "phonex-ka");
    let (primary, alt) = enc.encode("თბილისი").expect("encodes თბილისი");
    assert_eq!(primary, "T147");
    assert!(alt.is_none());
}

#[test]
fn phonetic_encoder_returns_none_for_no_georgian() {
    let enc = GEORGIAN.phonetic_encoder().unwrap();
    assert!(enc.encode("hello").is_none());
    assert!(enc.encode("").is_none());
    assert!(enc.encode("123").is_none());
}

#[test]
fn collator_is_none_by_default() {
    assert!(GEORGIAN.collator().is_none());
}
