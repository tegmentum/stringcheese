//! Integration tests for the Malayalam [`Language`] implementation.

use stringcheese_lang::Language;
use stringcheese_ml::MALAYALAM;

#[test]
fn code_and_name() {
    assert_eq!(MALAYALAM.code(), "ml");
    assert_eq!(MALAYALAM.name(), "Malayalam");
}

#[test]
fn common_malayalam_pronouns_are_stopwords() {
    for w in ["ഞാൻ", "നീ", "അവൻ", "അവൾ", "അവർ"] {
        assert!(
            MALAYALAM.is_stopword(w),
            "expected pronoun {w:?} to be a stopword"
        );
    }
}

#[test]
fn common_malayalam_conjunctions_are_stopwords() {
    for w in ["ഒപ്പം", "അല്ലെങ്കിൽ", "പക്ഷേ"] {
        assert!(
            MALAYALAM.is_stopword(w),
            "expected conjunction {w:?} to be a stopword"
        );
    }
}

#[test]
fn common_to_be_forms_are_stopwords() {
    for w in ["ആണ്", "ഉണ്ട്"] {
        assert!(
            MALAYALAM.is_stopword(w),
            "expected to-be form {w:?} to be a stopword"
        );
    }
}

#[test]
fn negation_and_affirmation_particles_are_stopwords() {
    for w in ["ഇല്ല", "അല്ല"] {
        assert!(
            MALAYALAM.is_stopword(w),
            "expected particle {w:?} to be a stopword"
        );
    }
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["മലയാളം", "പുസ്തകം", "algorithm", "supercalifragilistic"] {
        assert!(!MALAYALAM.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_malayalam_sentence() {
    let toks: Vec<&str> = MALAYALAM.tokenize("ഞാൻ മലയാളം സംസാരിക്കുന്നു").collect();
    assert_eq!(toks, ["ഞാൻ", "മലയാളം", "സംസാരിക്കുന്നു"]);
}

#[test]
fn tokenize_splits_on_ascii_period() {
    // Malayalam uses ASCII punctuation for sentence-final marking;
    // no danda.
    let toks: Vec<&str> = MALAYALAM.tokenize("ഞാൻ പോകുന്നു.").collect();
    assert_eq!(toks, ["ഞാൻ", "പോകുന്നു"]);
}

#[test]
fn tokenize_preserves_matras_and_virama_inside_words() {
    // സത്യം = സ + ത + ് + യ + ം — the virama and anusvara must stay
    // word-internal.
    let toks: Vec<&str> = MALAYALAM.tokenize("സത്യം എന്ന് പറഞ്ഞു").collect();
    assert_eq!(toks, ["സത്യം", "എന്ന്", "പറഞ്ഞു"]);
}

#[test]
fn tokenize_preserves_word_internal_chillu() {
    // Chillu letters (word-final atomic pure consonants) must stay
    // inside their token, not be treated as separators.
    let toks: Vec<&str> = MALAYALAM.tokenize("അവർ").collect();
    assert_eq!(toks, ["അവർ"]);
    let toks: Vec<&str> = MALAYALAM.tokenize("ഞാൻ അവർ അവൾ").collect();
    assert_eq!(toks, ["ഞാൻ", "അവർ", "അവൾ"]);
}

#[test]
fn stem_plural_marker_ngal() {
    // പുസ്തകങ്ങൾ → പുസ്തക (books).
    assert_eq!(MALAYALAM.stem("പുസ്തകങ്ങൾ"), "പുസ്തക");
}

#[test]
fn stem_plural_marker_kal() {
    // വീടുകൾ → വീടു (houses).
    assert_eq!(MALAYALAM.stem("വീടുകൾ"), "വീടു");
}

#[test]
fn stem_animate_plural_maar() {
    // ടീച്ചർമാർ → ടീച്ചർ (teachers).
    assert_eq!(MALAYALAM.stem("ടീച്ചർമാർ"), "ടീച്ചർ");
}

#[test]
fn stem_locative_il() {
    // വീട്ടിൽ → വീട്ട (in the house).
    assert_eq!(MALAYALAM.stem("വീട്ടിൽ"), "വീട്ട");
}

#[test]
fn stem_genitive_nte() {
    // രാമന്റെ → രാമ (of Rama).
    assert_eq!(MALAYALAM.stem("രാമന്റെ"), "രാമ");
}

#[test]
fn stem_present_tense() {
    // പഠിക്കുന്നു → പഠിക്ക (studies).
    assert_eq!(MALAYALAM.stem("പഠിക്കുന്നു"), "പഠിക്ക");
}

#[test]
fn stem_infinitive() {
    // പഠിക്കുക → പഠിക്ക (to study).
    assert_eq!(MALAYALAM.stem("പഠിക്കുക"), "പഠിക്ക");
}

#[test]
fn stem_leaves_bare_chillu_words_alone() {
    // Chillu-ending pronouns are stems in their own right.
    assert_eq!(MALAYALAM.stem("ഞാൻ"), "ഞാൻ");
    assert_eq!(MALAYALAM.stem("അവർ"), "അവർ");
    assert_eq!(MALAYALAM.stem("അവൾ"), "അവൾ");
    // Non-chillu bare noun.
    assert_eq!(MALAYALAM.stem("മലയാളം"), "മലയാളം");
}

#[test]
fn phonetic_encoder_is_phonex_ml() {
    let enc = MALAYALAM
        .phonetic_encoder()
        .expect("Malayalam pack ships a phonetic encoder");
    assert_eq!(enc.name(), "phonex-ml");
    let (primary, alt) = enc.encode("ഞാൻ").expect("encodes ഞാൻ");
    // ISO 15919: ñān → NAN. Phonex: N seed last=5; A vow last=0;
    // N code=5 push '5', out="N5" → pad "N500".
    assert_eq!(primary, "N500");
    assert!(alt.is_none());
}

#[test]
fn phonetic_encoder_handles_conjunct_via_virama() {
    // സത്യം → satyaṁ → SATYAM. Phonex: S seed last=7; A vow;
    // T code=3 push '3', out="S3" last=3; Y vow-like last=0;
    // A vow; M code=5 push '5', out="S35" last=5 → pad "S350".
    let enc = MALAYALAM.phonetic_encoder().unwrap();
    let (primary, _) = enc.encode("സത്യം").expect("encodes സത്യം");
    assert_eq!(primary, "S350");
}

#[test]
fn phonetic_encoder_returns_none_for_no_malayalam() {
    let enc = MALAYALAM.phonetic_encoder().unwrap();
    assert!(enc.encode("hello").is_none());
    assert!(enc.encode("").is_none());
    assert!(enc.encode("123").is_none());
}

#[test]
fn collator_is_none_by_default() {
    assert!(MALAYALAM.collator().is_none());
}
