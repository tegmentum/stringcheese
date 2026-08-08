//! Integration tests for the Hindi [`Language`] implementation.

use stringcheese_hi::HINDI;
use stringcheese_lang::Language;

#[test]
fn code_and_name() {
    assert_eq!(HINDI.code(), "hi");
    assert_eq!(HINDI.name(), "Hindi");
}

#[test]
fn common_hindi_pronouns_are_stopwords() {
    for w in ["मैं", "हम", "तू", "तुम", "आप", "वह", "यह"] {
        assert!(
            HINDI.is_stopword(w),
            "expected pronoun {w:?} to be a stopword"
        );
    }
}

#[test]
fn common_hindi_postpositions_are_stopwords() {
    for w in ["का", "की", "के", "को", "ने", "से", "में", "पर"] {
        assert!(
            HINDI.is_stopword(w),
            "expected postposition {w:?} to be a stopword"
        );
    }
}

#[test]
fn common_hindi_conjunctions_are_stopwords() {
    for w in ["और", "या", "लेकिन", "अगर", "कि"] {
        assert!(
            HINDI.is_stopword(w),
            "expected conjunction {w:?} to be a stopword"
        );
    }
}

#[test]
fn common_to_be_forms_are_stopwords() {
    for w in ["है", "हैं", "था", "थी", "थे"] {
        assert!(
            HINDI.is_stopword(w),
            "expected to-be form {w:?} to be a stopword"
        );
    }
}

#[test]
fn negation_particles_are_stopwords() {
    for w in ["नहीं", "मत", "न"] {
        assert!(
            HINDI.is_stopword(w),
            "expected negation particle {w:?} to be a stopword"
        );
    }
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["किताब", "स्कूल", "algorithm", "supercalifragilistic"] {
        assert!(!HINDI.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_hindi_sentence() {
    let toks: Vec<&str> = HINDI.tokenize("मैं किताब पढ़ता हूँ").collect();
    assert_eq!(toks, ["मैं", "किताब", "पढ़ता", "हूँ"]);
}

#[test]
fn tokenize_splits_on_danda() {
    // Devanagari danda (।) is the primary sentence terminator in
    // Hindi and must be treated as a separator.
    let toks: Vec<&str> = HINDI.tokenize("मैं जाता हूँ।").collect();
    assert_eq!(toks, ["मैं", "जाता", "हूँ"]);
}

#[test]
fn tokenize_preserves_matras_and_virama_inside_words() {
    // सत्य = स + त + ् + य — the virama and dependent vowel signs
    // must stay word-internal.
    let toks: Vec<&str> = HINDI.tokenize("सत्य की जीत").collect();
    assert_eq!(toks, ["सत्य", "की", "जीत"]);
}

#[test]
fn stem_masculine_singular_marker() {
    // लड़का → लड़क (strip ा).
    assert_eq!(HINDI.stem("लड़का"), "लड़क");
}

#[test]
fn stem_feminine_singular_marker() {
    assert_eq!(HINDI.stem("लड़की"), "लड़क");
}

#[test]
fn stem_oblique_plural_marker() {
    assert_eq!(HINDI.stem("लड़कों"), "लड़क");
}

#[test]
fn stem_habitual_verb_marker() {
    assert_eq!(HINDI.stem("जाता"), "जा");
    assert_eq!(HINDI.stem("जाती"), "जा");
    assert_eq!(HINDI.stem("जाते"), "जा");
}

#[test]
fn stem_leaves_bare_nouns_alone() {
    assert_eq!(HINDI.stem("किताब"), "किताब");
}

#[test]
fn phonetic_encoder_is_iast_hi() {
    let enc = HINDI
        .phonetic_encoder()
        .expect("Hindi pack ships a phonetic encoder");
    assert_eq!(enc.name(), "iast-hi");
    let (primary, alt) = enc.encode("राम").expect("encodes राम");
    assert_eq!(primary, "rāma");
    assert!(alt.is_none());
}

#[test]
fn phonetic_encoder_handles_consonant_cluster_via_virama() {
    // सत्य = sat + ya (virama suppresses the schwa on स).
    let enc = HINDI.phonetic_encoder().unwrap();
    let (primary, _) = enc.encode("सत्य").expect("encodes सत्य");
    assert_eq!(primary, "satya");
}

#[test]
fn phonetic_encoder_honors_inherent_schwa() {
    // Bare क encodes with its inherent schwa: क → ka, not just k.
    let enc = HINDI.phonetic_encoder().unwrap();
    let (primary, _) = enc.encode("क").expect("encodes क");
    assert_eq!(primary, "ka");
}

#[test]
fn phonetic_encoder_returns_none_for_no_devanagari() {
    let enc = HINDI.phonetic_encoder().unwrap();
    assert!(enc.encode("hello").is_none());
    assert!(enc.encode("").is_none());
    assert!(enc.encode("123").is_none());
}

#[test]
fn collator_is_none_by_default() {
    assert!(HINDI.collator().is_none());
}
