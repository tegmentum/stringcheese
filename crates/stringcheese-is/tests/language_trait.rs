//! Integration tests for the Icelandic [`Language`] implementation.

use stringcheese_is::ICELANDIC;
use stringcheese_lang::Language;

#[test]
fn code_and_name() {
    assert_eq!(ICELANDIC.code(), "is");
    assert_eq!(ICELANDIC.name(), "Icelandic");
}

#[test]
fn common_icelandic_words_are_stopwords() {
    for w in [
        "og", "en", "eða", "að", "sem", "í", "á", "af", "til", "frá", "með", "ekki", "ég", "þú",
        "hann", "hún",
    ] {
        assert!(ICELANDIC.is_stopword(w), "expected {w:?} to be a stopword");
    }
}

#[test]
fn case_insensitive_stopword_lookup() {
    // ASCII case folding via `str::eq_ignore_ascii_case`.
    assert!(ICELANDIC.is_stopword("OG"));
    assert!(ICELANDIC.is_stopword("En"));
    assert!(ICELANDIC.is_stopword("SEM"));
    assert!(ICELANDIC.is_stopword("Er"));
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["fiskur", "algrím", "ofurmenni", "bókasafn"] {
        assert!(!ICELANDIC.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_icelandic_sentence() {
    let text = "Hún hefur farið í búðina.";
    let toks: Vec<&str> = ICELANDIC.tokenize(text).collect();
    assert_eq!(toks, ["Hún", "hefur", "farið", "í", "búðina"]);
}

#[test]
fn stem_a_few_icelandic_words() {
    // Rule-based reductions (hand-traced through the suffix table).
    assert_eq!(ICELANDIC.stem("hesturinn"), "hest");
    assert_eq!(ICELANDIC.stem("bókin"), "bók");
    assert_eq!(ICELANDIC.stem("húsið"), "hús");
    assert_eq!(ICELANDIC.stem("konum"), "kon");
    assert_eq!(ICELANDIC.stem("hafa"), "haf");
}

#[test]
fn phonetic_encoder_is_phonex_is() {
    let enc = ICELANDIC
        .phonetic_encoder()
        .expect("Icelandic pack ships a phonetic encoder");
    assert_eq!(enc.name(), "phonex-is");
    assert_eq!(
        enc.encode("Þór"),
        Some((String::from("T600"), None)),
        "PHONEX-IS(Þór) should be T600 with no alternate key"
    );
    assert_eq!(
        enc.encode("Björn"),
        Some((String::from("B265"), None)),
        "PHONEX-IS(Björn) should be B265 with no alternate key"
    );
}

#[test]
fn collator_is_none_by_default() {
    assert!(ICELANDIC.collator().is_none());
}
