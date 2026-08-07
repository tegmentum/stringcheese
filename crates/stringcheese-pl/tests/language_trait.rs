//! Integration tests for the Polish [`Language`] implementation.

use stringcheese_lang::Language;
use stringcheese_pl::POLISH;

#[test]
fn code_and_name() {
    assert_eq!(POLISH.code(), "pl");
    assert_eq!(POLISH.name(), "Polish");
}

#[test]
fn common_polish_words_are_stopwords() {
    for w in [
        "i", "a", "w", "z", "na", "nie", "że", "do", "ale", "jak", "się", "to",
    ] {
        assert!(POLISH.is_stopword(w), "expected {w:?} to be a stopword");
    }
}

#[test]
fn case_insensitive_stopword_lookup_uses_unicode_fold() {
    // Polish case-fold (Rust default Unicode fold): A → a, Ą → ą, Ć →
    // ć, Ż → ż, etc. all work under default rules. Uppercase queries
    // fold correctly to the plain lowercase list.
    assert!(POLISH.is_stopword("NIE"));
    assert!(POLISH.is_stopword("Nie"));
    assert!(POLISH.is_stopword("ŻE"));
    assert!(POLISH.is_stopword("Że"));
    assert!(POLISH.is_stopword("AŻ"));
    assert!(!POLISH.is_stopword("KSIĄŻKA")); // Not a stopword.
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["książka", "pies", "komputer", "program", "łóżko"] {
        assert!(!POLISH.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_polish_sentence() {
    let text = "Kot śpi na macie.";
    let toks: Vec<&str> = POLISH.tokenize(text).collect();
    assert_eq!(toks, ["Kot", "śpi", "na", "macie"]);
}

#[test]
fn tokenize_preserves_diacritics_in_tokens() {
    let text = "Mąż i żona mieszkają w Krakowie.";
    let toks: Vec<&str> = POLISH.tokenize(text).collect();
    assert_eq!(toks, ["Mąż", "i", "żona", "mieszkają", "w", "Krakowie"]);
}

#[test]
fn stem_a_few_polish_words() {
    // Noun plural + verb infinitive + verb 1pl. See the
    // `snowball_reference` test for the wider set.
    assert_eq!(POLISH.stem("książki"), "książ");
    assert_eq!(POLISH.stem("czytać"), "czyt");
    assert_eq!(POLISH.stem("czytamy"), "czyt");
    assert_eq!(POLISH.stem("dobrego"), "dobr");
}

#[test]
fn phonetic_encoder_is_phonex_pl() {
    let enc = POLISH
        .phonetic_encoder()
        .expect("Polish pack ships a phonetic encoder");
    assert_eq!(enc.name(), "phonex-pl");
    assert_eq!(
        enc.encode("Kowalski"),
        Some((String::from("K147"), None)),
        "PHONEX-PL(Kowalski) should be K147 with no alternate key"
    );
    assert_eq!(
        enc.encode("Nowak"),
        Some((String::from("N120"), None)),
        "PHONEX-PL(Nowak) should be N120 with no alternate key"
    );
}

#[test]
fn phonetic_encoder_conflates_o_kreska_with_u() {
    // The signature Polish PHONEX equivalence: ó and u produce the same
    // phonetic content because they represent the same modern-Polish
    // vowel /u/.
    let enc = POLISH.phonetic_encoder().unwrap();
    let (a, _) = enc.encode("Kraków").unwrap();
    let (b, _) = enc.encode("Krakuw").unwrap();
    assert_eq!(a, b, "ó/u merger failed");
}

#[test]
fn phonetic_encoder_merges_z_kropka_and_z_kreska() {
    // ż and ź both fold to Z (sibilant class).
    let enc = POLISH.phonetic_encoder().unwrap();
    let (a, _) = enc.encode("Żak").unwrap();
    let (b, _) = enc.encode("Zak").unwrap();
    assert_eq!(a, b, "ż/z merger failed");
    let (c, _) = enc.encode("Źle").unwrap();
    let (d, _) = enc.encode("Zle").unwrap();
    assert_eq!(c, d, "ź/z merger failed");
}

#[test]
fn collator_is_none_by_default() {
    assert!(POLISH.collator().is_none());
}
