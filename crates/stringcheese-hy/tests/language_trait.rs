//! Integration tests for the Armenian [`Language`] implementation.

use stringcheese_hy::ARMENIAN;
use stringcheese_lang::Language;

#[test]
fn code_and_name() {
    assert_eq!(ARMENIAN.code(), "hy");
    assert_eq!(ARMENIAN.name(), "Armenian");
}

#[test]
fn common_armenian_words_are_stopwords() {
    for w in ["և", "կամ", "բայց", "ես", "դու", "նա", "մենք"] {
        assert!(ARMENIAN.is_stopword(w), "expected {w:?} to be a stopword");
    }
}

#[test]
fn case_insensitive_stopword_lookup_uses_armenian_fold() {
    // Armenian case-fold: Ա → ա, Բ → բ, etc.
    // Uppercase queries fold correctly. Testing uppercase variants
    // of `եւ` — `ԵՒ` (Ե + Ւ, upper Yiwn) should case-fold to `եւ`,
    // then normalize to `և`. Note that `ԵՎ` (Ե + Վ, upper Vew) is a
    // *different* two-letter sequence (Vew /v/ vs. Yiwn /w/ are
    // distinct Armenian letters) — case-folded it lands on `եվ`, not
    // `եւ`, so is not recognized as the conjunction.
    assert!(ARMENIAN.is_stopword("ԵՒ"));
    assert!(ARMENIAN.is_stopword("Եւ"));
    assert!(ARMENIAN.is_stopword("ԿԱՄ"));
    assert!(!ARMENIAN.is_stopword("ԵՐԵՒԱՆ")); // Not a stopword (place name).
}

#[test]
fn stopword_lookup_normalizes_eu_spelling() {
    // The stopword list stores `և` (single ligature); a query with
    // the two-letter `եւ` spelling should match too.
    assert!(ARMENIAN.is_stopword("եւ"));
    assert!(ARMENIAN.is_stopword("և"));
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["գիրք", "մարդ", "աշխարհ", "Հայաստան", "Երևան"] {
        assert!(!ARMENIAN.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_armenian_sentence() {
    // Armenian punctuation: `։` (full stop), `՝` (comma).
    let text = "Բարև, աշխարհ։ Երևանը՝ մայրաքաղաքն է։";
    let toks: Vec<&str> = ARMENIAN.tokenize(text).collect();
    assert_eq!(toks, ["Բարև", "աշխարհ", "Երևանը", "մայրաքաղաքն", "է"]);
}

#[test]
fn stem_a_few_armenian_words() {
    // See the `stemmer_reference` test for the wider set.
    assert_eq!(ARMENIAN.stem("մայրը"), "մայր");
    assert_eq!(ARMENIAN.stem("գրքի"), "գրք");
    assert_eq!(ARMENIAN.stem("սիրեցի"), "սիր");
    assert_eq!(ARMENIAN.stem("գրքերով"), "գրք");
}

#[test]
fn phonetic_encoder_is_phonex_hy() {
    let enc = ARMENIAN
        .phonetic_encoder()
        .expect("Armenian pack ships a phonetic encoder");
    assert_eq!(enc.name(), "phonex-hy");
    let (primary, alt) = enc.encode("Երևան").expect("encodes Երևան");
    assert_eq!(primary, "E615");
    assert!(alt.is_none());
}

#[test]
fn phonetic_encoder_returns_none_for_no_armenian() {
    let enc = ARMENIAN.phonetic_encoder().unwrap();
    assert!(enc.encode("hello").is_none());
    assert!(enc.encode("").is_none());
    assert!(enc.encode("123").is_none());
}

#[test]
fn collator_is_none_by_default() {
    assert!(ARMENIAN.collator().is_none());
}
