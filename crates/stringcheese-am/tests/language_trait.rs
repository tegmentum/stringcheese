//! Integration tests for the Amharic [`Language`] implementation.

use stringcheese_am::AMHARIC;
use stringcheese_lang::Language;

#[test]
fn code_and_name() {
    assert_eq!(AMHARIC.code(), "am");
    assert_eq!(AMHARIC.name(), "Amharic");
}

#[test]
fn common_amharic_pronouns_are_stopwords() {
    for w in ["እኔ", "አንተ", "አንቺ", "እሱ", "እርሷ", "እኛ"] {
        assert!(
            AMHARIC.is_stopword(w),
            "expected pronoun {w:?} to be a stopword"
        );
    }
}

#[test]
fn common_amharic_conjunctions_are_stopwords() {
    for w in ["እና", "ወይም", "ግን"] {
        assert!(
            AMHARIC.is_stopword(w),
            "expected conjunction {w:?} to be a stopword"
        );
    }
}

#[test]
fn common_copula_forms_are_stopwords() {
    for w in ["ነው", "ናት", "ናቸው", "ነበር"] {
        assert!(
            AMHARIC.is_stopword(w),
            "expected copula form {w:?} to be a stopword"
        );
    }
}

#[test]
fn negation_particles_are_stopwords() {
    for w in ["አይ", "የለም", "አይደለም"] {
        assert!(
            AMHARIC.is_stopword(w),
            "expected negation particle {w:?} to be a stopword"
        );
    }
}

#[test]
fn common_demonstratives_are_stopwords() {
    for w in ["ይህ", "ያ", "እነዚህ", "እነዚያ"] {
        assert!(
            AMHARIC.is_stopword(w),
            "expected demonstrative {w:?} to be a stopword"
        );
    }
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["አማርኛ", "ኢትዮጵያ", "algorithm", "supercalifragilistic"] {
        assert!(!AMHARIC.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_amharic_sentence() {
    let toks: Vec<&str> = AMHARIC.tokenize("እኔ አማርኛ እወዳለሁ").collect();
    assert_eq!(toks, ["እኔ", "አማርኛ", "እወዳለሁ"]);
}

#[test]
fn tokenize_splits_on_ethiopic_full_stop() {
    // ። U+1362 — sentence terminator.
    let toks: Vec<&str> = AMHARIC.tokenize("እኔ አማርኛ እወዳለሁ።").collect();
    assert_eq!(toks, ["እኔ", "አማርኛ", "እወዳለሁ"]);
}

#[test]
fn tokenize_splits_on_ethiopic_wordspace() {
    // ፡ U+1361 — traditional Ge'ez word separator.
    let toks: Vec<&str> = AMHARIC.tokenize("እኔ፡አማርኛ፡እወዳለሁ").collect();
    assert_eq!(toks, ["እኔ", "አማርኛ", "እወዳለሁ"]);
}

#[test]
fn stem_definite_article_masc() {
    // ቤት + ው → ቤት (the house).
    assert_eq!(AMHARIC.stem("ቤትው"), "ቤት");
}

#[test]
fn stem_definite_article_fem() {
    // ልጅ + ዋ → ልጅ (the daughter).
    assert_eq!(AMHARIC.stem("ልጅዋ"), "ልጅ");
}

#[test]
fn stem_plural() {
    // ልጅ + ኦች → ልጅ (children).
    assert_eq!(AMHARIC.stem("ልጅኦች"), "ልጅ");
}

#[test]
fn stem_possessive_our() {
    // ቤት + ችን → ቤት (our house).
    assert_eq!(AMHARIC.stem("ቤትችን"), "ቤት");
}

#[test]
fn stem_object_them() {
    // ቤት + ኣቸው → ቤት (... them).
    assert_eq!(AMHARIC.stem("ቤትኣቸው"), "ቤት");
}

#[test]
fn stem_iterates_to_convergence() {
    // ልጅ + ኦች + ችን → ልጅ (stacked plural + possessive).
    assert_eq!(AMHARIC.stem("ልጅኦችችን"), "ልጅ");
}

#[test]
fn stem_leaves_bare_nouns_alone() {
    assert_eq!(AMHARIC.stem("አማርኛ"), "አማርኛ");
    assert_eq!(AMHARIC.stem("ኢትዮጵያ"), "ኢትዮጵያ");
}

#[test]
fn phonetic_encoder_is_phonex_am() {
    let enc = AMHARIC
        .phonetic_encoder()
        .expect("Amharic pack ships a phonetic encoder");
    assert_eq!(enc.name(), "phonex-am");
    let (primary, alt) = enc.encode("አማርኛ").expect("encodes አማርኛ");
    assert_eq!(primary, "E565");
    assert!(alt.is_none());
}

#[test]
fn phonetic_encoder_returns_none_for_no_amharic() {
    let enc = AMHARIC.phonetic_encoder().unwrap();
    assert!(enc.encode("hello").is_none());
    assert!(enc.encode("").is_none());
    assert!(enc.encode("123").is_none());
}

#[test]
fn collator_is_none_by_default() {
    assert!(AMHARIC.collator().is_none());
}
