//! Integration tests for the Tamil [`Language`] implementation.

use stringcheese_lang::Language;
use stringcheese_ta::TAMIL;

#[test]
fn code_and_name() {
    assert_eq!(TAMIL.code(), "ta");
    assert_eq!(TAMIL.name(), "Tamil");
}

#[test]
fn common_tamil_pronouns_are_stopwords() {
    for w in ["நான்", "நீ", "அவன்", "அவள்", "அவர்"] {
        assert!(
            TAMIL.is_stopword(w),
            "expected pronoun {w:?} to be a stopword"
        );
    }
}

#[test]
fn common_tamil_conjunctions_are_stopwords() {
    for w in ["மற்றும்", "அல்லது", "ஆனால்"] {
        assert!(
            TAMIL.is_stopword(w),
            "expected conjunction {w:?} to be a stopword"
        );
    }
}

#[test]
fn common_to_be_forms_are_stopwords() {
    for w in ["உள்ளது", "இருந்தது"] {
        assert!(
            TAMIL.is_stopword(w),
            "expected to-be form {w:?} to be a stopword"
        );
    }
}

#[test]
fn negation_and_affirmation_particles_are_stopwords() {
    for w in ["இல்லை", "ஆம்"] {
        assert!(
            TAMIL.is_stopword(w),
            "expected particle {w:?} to be a stopword"
        );
    }
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["தமிழ்", "புத்தகம்", "algorithm", "supercalifragilistic"] {
        assert!(!TAMIL.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_tamil_sentence() {
    let toks: Vec<&str> = TAMIL.tokenize("நான் தமிழ் பேசுகிறேன்").collect();
    assert_eq!(toks, ["நான்", "தமிழ்", "பேசுகிறேன்"]);
}

#[test]
fn tokenize_splits_on_ascii_period() {
    // Tamil uses ASCII punctuation for sentence-final marking;
    // no danda.
    let toks: Vec<&str> = TAMIL.tokenize("நான் போகிறேன்.").collect();
    assert_eq!(toks, ["நான்", "போகிறேன்"]);
}

#[test]
fn tokenize_preserves_matras_and_pulli_inside_words() {
    // தமிழ் = த + ம + ி + ழ + ் — the matra and pulli must stay
    // word-internal.
    let toks: Vec<&str> = TAMIL.tokenize("தமிழ் என்று கூறினார்").collect();
    assert_eq!(toks, ["தமிழ்", "என்று", "கூறினார்"]);
}

#[test]
fn tokenize_preserves_word_internal_pulli() {
    // Every Tamil consonant cluster uses a visible pulli; the
    // tokenizer must not shatter at it.
    let toks: Vec<&str> = TAMIL.tokenize("நன்றி").collect();
    assert_eq!(toks, ["நன்றி"]);
}

#[test]
fn stem_plural_marker_kal() {
    // மாணவர்கள் → மாணவர் (students).
    assert_eq!(TAMIL.stem("மாணவர்கள்"), "மாணவர்");
}

#[test]
fn stem_locative_il() {
    // வீட்டில் → வீட்ட (in the house).
    assert_eq!(TAMIL.stem("வீட்டில்"), "வீட்ட");
}

#[test]
fn stem_instrumental_aal() {
    // மரத்தால் → மரத்த (by the tree).
    assert_eq!(TAMIL.stem("மரத்தால்"), "மரத்த");
}

#[test]
fn stem_dative_ku() {
    // அவனுக்கு → அவனுக் (to him).
    assert_eq!(TAMIL.stem("அவனுக்கு"), "அவனுக்");
}

#[test]
fn stem_verb_1p_singular_kiren() {
    // பேசுகிறேன் → பேசு (I speak).
    assert_eq!(TAMIL.stem("பேசுகிறேன்"), "பேசு");
}

#[test]
fn stem_verb_3p_plural_kirarkal() {
    // பேசுகிறார்கள் → பேசு (they speak).
    assert_eq!(TAMIL.stem("பேசுகிறார்கள்"), "பேசு");
}

#[test]
fn stem_leaves_bare_nouns_alone() {
    assert_eq!(TAMIL.stem("தமிழ்"), "தமிழ்");
    assert_eq!(TAMIL.stem("நான்"), "நான்");
}

#[test]
fn phonetic_encoder_is_phonex_ta() {
    let enc = TAMIL
        .phonetic_encoder()
        .expect("Tamil pack ships a phonetic encoder");
    assert_eq!(enc.name(), "phonex-ta");
    let (primary, alt) = enc.encode("தமிழ்").expect("encodes தமிழ்");
    // T seed, A vow, M code=5 push, I vow, L code=4 push → "T540".
    assert_eq!(primary, "T540");
    assert!(alt.is_none());
}

#[test]
fn phonetic_encoder_handles_consonant_cluster_via_pulli() {
    // நன்றி → naṉṟi → NANRI → N seed last=5; A vow last=0;
    // N code=5 push '5', out="N5" last=5; R code=6 push '6',
    // out="N56" last=6; I vow → pad "N560".
    let enc = TAMIL.phonetic_encoder().unwrap();
    let (primary, _) = enc.encode("நன்றி").expect("encodes நன்றி");
    assert_eq!(primary, "N560");
}

#[test]
fn phonetic_encoder_returns_none_for_no_tamil() {
    let enc = TAMIL.phonetic_encoder().unwrap();
    assert!(enc.encode("hello").is_none());
    assert!(enc.encode("").is_none());
    assert!(enc.encode("123").is_none());
}

#[test]
fn collator_is_none_by_default() {
    assert!(TAMIL.collator().is_none());
}
