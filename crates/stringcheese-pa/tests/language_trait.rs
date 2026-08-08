//! Integration tests for the Punjabi [`Language`] implementation.

use stringcheese_lang::Language;
use stringcheese_pa::PUNJABI;

#[test]
fn code_and_name() {
    assert_eq!(PUNJABI.code(), "pa");
    assert_eq!(PUNJABI.name(), "Punjabi");
}

#[test]
fn common_punjabi_pronouns_are_stopwords() {
    for w in ["ਮੈਂ", "ਤੂੰ", "ਤੁਸੀਂ", "ਅਸੀਂ", "ਓਹ"] {
        assert!(
            PUNJABI.is_stopword(w),
            "expected pronoun {w:?} to be a stopword"
        );
    }
}

#[test]
fn common_punjabi_conjunctions_are_stopwords() {
    for w in ["ਅਤੇ", "ਜਾਂ", "ਪਰ", "ਜੇ"] {
        assert!(
            PUNJABI.is_stopword(w),
            "expected conjunction {w:?} to be a stopword"
        );
    }
}

#[test]
fn common_to_be_forms_are_stopwords() {
    for w in ["ਹੈ", "ਹਾਂ", "ਸੀ"] {
        assert!(
            PUNJABI.is_stopword(w),
            "expected to-be form {w:?} to be a stopword"
        );
    }
}

#[test]
fn negation_particles_are_stopwords() {
    for w in ["ਨਹੀਂ", "ਨਾ"] {
        assert!(
            PUNJABI.is_stopword(w),
            "expected negation particle {w:?} to be a stopword"
        );
    }
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["ਪੰਜਾਬ", "ਕਿਤਾਬ", "algorithm", "supercalifragilistic"] {
        assert!(!PUNJABI.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_punjabi_sentence() {
    let toks: Vec<&str> = PUNJABI.tokenize("ਮੈਂ ਪੰਜਾਬੀ ਬੋਲਦਾ ਹਾਂ").collect();
    assert_eq!(toks, ["ਮੈਂ", "ਪੰਜਾਬੀ", "ਬੋਲਦਾ", "ਹਾਂ"]);
}

#[test]
fn tokenize_splits_on_danda() {
    // Gurmukhi inherits the Devanagari danda (।).
    let toks: Vec<&str> = PUNJABI.tokenize("ਮੈਂ ਜਾਂਦਾ ਹਾਂ।").collect();
    assert_eq!(toks, ["ਮੈਂ", "ਜਾਂਦਾ", "ਹਾਂ"]);
}

#[test]
fn tokenize_preserves_tippi_bindi_and_addak_inside_words() {
    // ਪੰਜਾਬੀ carries a tippi, ਮੈਂ carries a bindi, ਪੱਕਾ carries an
    // addak — all three must stay word-internal.
    let toks: Vec<&str> = PUNJABI.tokenize("ਪੰਜਾਬੀ ਮੈਂ ਪੱਕਾ").collect();
    assert_eq!(toks, ["ਪੰਜਾਬੀ", "ਮੈਂ", "ਪੱਕਾ"]);
}

#[test]
fn stem_plural_marker_aan() {
    // ਘਰਾਂ → ਘਰ (houses).
    assert_eq!(PUNJABI.stem("ਘਰਾਂ"), "ਘਰ");
}

#[test]
fn stem_fem_plural_iiaan() {
    // ਕੁੜੀਆਂ → ਕੁੜ (girls, normalizes with singular).
    assert_eq!(PUNJABI.stem("ਕੁੜੀਆਂ"), "ਕੁੜ");
}

#[test]
fn stem_oblique_sg_e() {
    // ਮੁੰਡੇ → ਮੁੰਡ (of the boy).
    assert_eq!(PUNJABI.stem("ਮੁੰਡੇ"), "ਮੁੰਡ");
}

#[test]
fn stem_imperfective_participle_daa() {
    // ਬੋਲਦਾ → ਬੋਲ (speaks, masc sg).
    assert_eq!(PUNJABI.stem("ਬੋਲਦਾ"), "ਬੋਲ");
}

#[test]
fn stem_perfective_3sg_masc_ia() {
    // ਬੋਲਿਆ → ਬੋਲ (spoke, 3sg-m).
    assert_eq!(PUNJABI.stem("ਬੋਲਿਆ"), "ਬੋਲ");
}

#[test]
fn stem_leaves_bare_nouns_alone() {
    assert_eq!(PUNJABI.stem("ਪੰਜਾਬ"), "ਪੰਜਾਬ");
    assert_eq!(PUNJABI.stem("ਘਰ"), "ਘਰ");
}

#[test]
fn phonetic_encoder_is_phonex_pa() {
    let enc = PUNJABI
        .phonetic_encoder()
        .expect("Punjabi pack ships a phonetic encoder");
    assert_eq!(enc.name(), "phonex-pa");
    let (primary, alt) = enc.encode("ਪੰਜਾਬ").expect("encodes ਪੰਜਾਬ");
    // P seed, A vow, M pushes '5', J pushes '2', A vow, B pushes '1'
    // → "P521" (4 chars, break).
    assert_eq!(primary, "P521");
    assert!(alt.is_none());
}

#[test]
fn phonetic_encoder_tone_collapse_groups_ghar_and_kar() {
    // The tone-collapse pre-pass folds ਘ (gh) to ਕ (k) in phonex.
    let enc = PUNJABI.phonetic_encoder().unwrap();
    let (ghar, _) = enc.encode("ਘਰ").expect("encodes ਘਰ");
    let (kar, _) = enc.encode("ਕਰ").expect("encodes ਕਰ");
    assert_eq!(ghar, kar);
    // K seed, A vow, R pushes '6', A vow → "K6" pad → "K600".
    assert_eq!(ghar, "K600");
}

#[test]
fn phonetic_encoder_returns_none_for_no_gurmukhi() {
    let enc = PUNJABI.phonetic_encoder().unwrap();
    assert!(enc.encode("hello").is_none());
    assert!(enc.encode("").is_none());
    assert!(enc.encode("123").is_none());
}

#[test]
fn collator_is_none_by_default() {
    assert!(PUNJABI.collator().is_none());
}
