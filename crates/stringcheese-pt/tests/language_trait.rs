//! Integration tests for the Portuguese [`Language`] implementation.

use stringcheese_lang::Language;
use stringcheese_pt::PORTUGUESE;

#[test]
fn code_and_name() {
    assert_eq!(PORTUGUESE.code(), "pt");
    assert_eq!(PORTUGUESE.name(), "Portuguese");
}

#[test]
fn common_portuguese_words_are_stopwords() {
    for w in [
        "o", "a", "os", "as", "e", "ou", "de", "em", "um", "uma", "por", "para",
    ] {
        assert!(PORTUGUESE.is_stopword(w), "expected {w:?} to be a stopword");
    }
}

#[test]
fn case_insensitive_stopword_lookup() {
    // ASCII case folding via `str::eq_ignore_ascii_case`.
    assert!(PORTUGUESE.is_stopword("O"));
    assert!(PORTUGUESE.is_stopword("A"));
    assert!(PORTUGUESE.is_stopword("E"));
    assert!(PORTUGUESE.is_stopword("De"));
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in [
        "queijo",
        "algoritmo",
        "linguística",
        "supercalifragilístico",
    ] {
        assert!(!PORTUGUESE.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_portuguese_sentence() {
    let text = "Como está você? Bem, obrigado.";
    let toks: Vec<&str> = PORTUGUESE.tokenize(text).collect();
    assert_eq!(toks, ["Como", "está", "você", "Bem", "obrigado"]);
}

#[test]
fn stem_a_few_portuguese_words() {
    assert_eq!(PORTUGUESE.stem("falando"), "fal");
    assert_eq!(PORTUGUESE.stem("meninos"), "menin");
    assert_eq!(PORTUGUESE.stem("casa"), "cas");
    assert_eq!(PORTUGUESE.stem("falar"), "fal");
}

#[test]
fn phonetic_encoder_is_phonex_pt() {
    let enc = PORTUGUESE
        .phonetic_encoder()
        .expect("Portuguese pack ships a phonetic encoder");
    assert_eq!(enc.name(), "phonex-pt");
    assert_eq!(
        enc.encode("Silva"),
        Some((String::from("S410"), None)),
        "PHONEX-PT(Silva) should be S410 with no alternate key"
    );
    assert_eq!(
        enc.encode("Santos"),
        Some((String::from("S537"), None)),
        "PHONEX-PT(Santos) should be S537 with no alternate key"
    );
}

#[test]
fn collator_is_none_by_default() {
    assert!(PORTUGUESE.collator().is_none());
}
