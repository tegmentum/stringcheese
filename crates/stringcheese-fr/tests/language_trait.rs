//! Integration tests for the French [`Language`] implementation.

use stringcheese_fr::FRENCH;
use stringcheese_lang::Language;

#[test]
fn code_and_name() {
    assert_eq!(FRENCH.code(), "fr");
    assert_eq!(FRENCH.name(), "French");
}

#[test]
fn common_french_words_are_stopwords() {
    for w in [
        "le", "la", "les", "et", "ou", "de", "du", "des", "un", "une",
    ] {
        assert!(FRENCH.is_stopword(w), "expected {w:?} to be a stopword");
    }
}

#[test]
fn elision_clitics_are_stopwords() {
    for w in ["l'", "d'", "qu'", "j'", "n'", "c'", "s'", "m'", "t'"] {
        assert!(
            FRENCH.is_stopword(w),
            "expected clitic {w:?} to be a stopword"
        );
    }
    for w in ["l", "d", "qu", "j", "n", "c", "s", "m", "t"] {
        assert!(
            FRENCH.is_stopword(w),
            "expected bare-clitic {w:?} to be a stopword"
        );
    }
}

#[test]
fn case_insensitive_stopword_lookup() {
    // ASCII case folding via `str::eq_ignore_ascii_case`.
    assert!(FRENCH.is_stopword("LE"));
    assert!(FRENCH.is_stopword("Le"));
    assert!(FRENCH.is_stopword("lE"));
    assert!(FRENCH.is_stopword("QU'"));
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in [
        "fromage",
        "algorithme",
        "linguistique",
        "supercalifragilistique",
    ] {
        assert!(!FRENCH.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_french_sentence_with_elisions() {
    let text = "L'homme qui n'était pas là s'appelle Pierre.";
    let toks: Vec<&str> = FRENCH.tokenize(text).collect();
    assert_eq!(
        toks,
        [
            "L'", "homme", "qui", "n'", "était", "pas", "là", "s'", "appelle", "Pierre",
        ]
    );
}

#[test]
fn tokenize_keeps_aujourdhui_together() {
    let toks: Vec<&str> = FRENCH.tokenize("aujourd'hui").collect();
    assert_eq!(toks, ["aujourd'hui"]);
}

#[test]
fn stem_a_few_french_words() {
    // Common verb-form conjugations of `continuer`.
    assert_eq!(FRENCH.stem("continue"), "continu");
    assert_eq!(FRENCH.stem("continues"), "continu");
    assert_eq!(FRENCH.stem("continuer"), "continu");
}

#[test]
fn phonetic_encoder_is_phonex() {
    let enc = FRENCH
        .phonetic_encoder()
        .expect("French pack ships a phonetic encoder");
    assert_eq!(enc.name(), "phonex");
    assert_eq!(
        enc.encode("Dubois"),
        Some((String::from("D180"), None)),
        "PHONEX(Dubois) should be D180 with no alternate key"
    );
    assert_eq!(
        enc.encode("Martin"),
        Some((String::from("M635"), None)),
        "PHONEX(Martin) should be M635 with no alternate key"
    );
}

#[test]
fn collator_is_none_by_default() {
    assert!(FRENCH.collator().is_none());
}
