//! Integration tests for the Turkish [`Language`] implementation.

use stringcheese_lang::Language;
use stringcheese_tr::TURKISH;

#[test]
fn code_and_name() {
    assert_eq!(TURKISH.code(), "tr");
    assert_eq!(TURKISH.name(), "Turkish");
}

#[test]
fn common_turkish_words_are_stopwords() {
    for w in [
        "ve", "veya", "ile", "için", "ama", "ancak", "bu", "şu", "ben", "sen",
    ] {
        assert!(TURKISH.is_stopword(w), "expected {w:?} to be a stopword");
    }
}

#[test]
fn case_insensitive_stopword_lookup_uses_turkic_fold() {
    // Turkish case-fold rules: `İ → i`, `I → ı`. So the uppercase
    // form of "için" is "İÇİN" (dotted-Is), and *that* folds back to
    // "için" — a stopword hit.
    assert!(TURKISH.is_stopword("İÇİN"));
    // The all-Latin-caps `ANCAK` (no Turkish special letters) also
    // folds correctly under the Turkic path.
    assert!(TURKISH.is_stopword("ANCAK"));
    // Mixed-case `Bu` is recognized.
    assert!(TURKISH.is_stopword("Bu"));
    // `ŞU` folds to `şu`.
    assert!(TURKISH.is_stopword("ŞU"));
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["peynir", "kütüphane", "algoritma", "yazılım"] {
        assert!(!TURKISH.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_turkish_sentence() {
    let text = "Merhaba, dünya! İstanbul çok güzel.";
    let toks: Vec<&str> = TURKISH.tokenize(text).collect();
    assert_eq!(toks, ["Merhaba", "dünya", "İstanbul", "çok", "güzel"]);
}

#[test]
fn stem_a_few_turkish_words() {
    assert_eq!(TURKISH.stem("kitaplar"), "kitap");
    assert_eq!(TURKISH.stem("evden"), "ev");
    assert_eq!(TURKISH.stem("evde"), "ev");
    assert_eq!(TURKISH.stem("evler"), "ev");
    assert_eq!(TURKISH.stem("okuldan"), "okul");
}

#[test]
fn phonetic_encoder_is_phonex_tr() {
    let enc = TURKISH
        .phonetic_encoder()
        .expect("Turkish pack ships a phonetic encoder");
    assert_eq!(enc.name(), "phonex-tr");
    let (primary, alt) = enc.encode("İstanbul").expect("PHONEX-TR encodes İstanbul");
    assert_eq!(primary, "I735");
    assert!(alt.is_none());
}

#[test]
fn collator_is_none_by_default() {
    assert!(TURKISH.collator().is_none());
}
