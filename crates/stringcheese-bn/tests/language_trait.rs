//! Integration tests for the Bengali [`Language`] implementation.

use stringcheese_bn::BENGALI;
use stringcheese_lang::Language;

#[test]
fn code_and_name() {
    assert_eq!(BENGALI.code(), "bn");
    assert_eq!(BENGALI.name(), "Bengali");
}

#[test]
fn common_bengali_pronouns_are_stopwords() {
    for w in ["আমি", "তুমি", "আপনি", "সে", "তিনি"] {
        assert!(
            BENGALI.is_stopword(w),
            "expected pronoun {w:?} to be a stopword"
        );
    }
}

#[test]
fn common_bengali_conjunctions_are_stopwords() {
    for w in ["এবং", "বা", "কিন্তু", "যদি"] {
        assert!(
            BENGALI.is_stopword(w),
            "expected conjunction {w:?} to be a stopword"
        );
    }
}

#[test]
fn common_to_be_forms_are_stopwords() {
    for w in ["আছে", "ছিল", "হয়"] {
        assert!(
            BENGALI.is_stopword(w),
            "expected to-be form {w:?} to be a stopword"
        );
    }
}

#[test]
fn negation_particles_are_stopwords() {
    for w in ["না", "নয়", "নেই"] {
        assert!(
            BENGALI.is_stopword(w),
            "expected negation particle {w:?} to be a stopword"
        );
    }
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["বাংলা", "বই", "algorithm", "supercalifragilistic"] {
        assert!(!BENGALI.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_bengali_sentence() {
    let toks: Vec<&str> = BENGALI.tokenize("আমি বাংলা বলি").collect();
    assert_eq!(toks, ["আমি", "বাংলা", "বলি"]);
}

#[test]
fn tokenize_splits_on_danda() {
    // Bengali danda (।) — inherited from Devanagari, used as
    // sentence terminator in Bengali text too.
    let toks: Vec<&str> = BENGALI.tokenize("আমি যাই।").collect();
    assert_eq!(toks, ["আমি", "যাই"]);
}

#[test]
fn tokenize_preserves_matras_and_halant_inside_words() {
    // সত্য = স + ত + ্ + য — the halant and matras must stay
    // word-internal.
    let toks: Vec<&str> = BENGALI.tokenize("সত্য এর জয়").collect();
    assert_eq!(toks, ["সত্য", "এর", "জয়"]);
}

#[test]
fn stem_plural_marker_guli() {
    // বইগুলি → বই (books, formal plural).
    assert_eq!(BENGALI.stem("বইগুলি"), "বই");
}

#[test]
fn stem_plural_marker_gulo() {
    // বইগুলো → বই (books, colloquial plural).
    assert_eq!(BENGALI.stem("বইগুলো"), "বই");
}

#[test]
fn stem_plural_genitive_der() {
    // ছেলেদের → ছেলে (of the boys).
    assert_eq!(BENGALI.stem("ছেলেদের"), "ছেলে");
}

#[test]
fn stem_animate_plural_ra() {
    // ছেলেরা → ছেলে (the boys).
    assert_eq!(BENGALI.stem("ছেলেরা"), "ছেলে");
}

#[test]
fn stem_accusative_ke() {
    // ছেলেকে → ছেলে (to the boy).
    assert_eq!(BENGALI.stem("ছেলেকে"), "ছেলে");
}

#[test]
fn stem_genitive_r() {
    // ছেলের → ছেলে (of the boy).
    assert_eq!(BENGALI.stem("ছেলের"), "ছেলে");
}

#[test]
fn stem_leaves_bare_nouns_alone() {
    assert_eq!(BENGALI.stem("বাংলা"), "বাংলা");
}

#[test]
fn phonetic_encoder_is_phonex_bn() {
    let enc = BENGALI
        .phonetic_encoder()
        .expect("Bengali pack ships a phonetic encoder");
    assert_eq!(enc.name(), "phonex-bn");
    let (primary, alt) = enc.encode("রাম").expect("encodes রাম");
    assert_eq!(primary, "R500");
    assert!(alt.is_none());
}

#[test]
fn phonetic_encoder_handles_consonant_cluster_via_halant() {
    // সত্য → satya → S300 (S seed, A vow, T code=3, Y vow).
    let enc = BENGALI.phonetic_encoder().unwrap();
    let (primary, _) = enc.encode("সত্য").expect("encodes সত্য");
    assert_eq!(primary, "S300");
}

#[test]
fn phonetic_encoder_returns_none_for_no_bengali() {
    let enc = BENGALI.phonetic_encoder().unwrap();
    assert!(enc.encode("hello").is_none());
    assert!(enc.encode("").is_none());
    assert!(enc.encode("123").is_none());
}

#[test]
fn collator_is_none_by_default() {
    assert!(BENGALI.collator().is_none());
}
