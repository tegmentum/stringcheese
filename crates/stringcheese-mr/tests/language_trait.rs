//! Integration tests for the Marathi [`Language`] implementation.

use stringcheese_lang::Language;
use stringcheese_mr::MARATHI;

#[test]
fn code_and_name() {
    assert_eq!(MARATHI.code(), "mr");
    assert_eq!(MARATHI.name(), "Marathi");
}

#[test]
fn common_marathi_pronouns_are_stopwords() {
    // Marathi's clusivity-preserving 1pl (आम्ही exclusive vs. आपण
    // inclusive) is one of the ways Marathi differs from Hindi, so
    // both variants deserve entries.
    for w in ["मी", "तू", "तो", "ती", "ते", "आम्ही", "आपण", "तुम्ही"]
    {
        assert!(
            MARATHI.is_stopword(w),
            "expected pronoun {w:?} to be a stopword"
        );
    }
}

#[test]
fn neuter_demonstrative_is_a_stopword() {
    // Marathi retains the Old Indo-Aryan neuter gender that Hindi has
    // lost. `हे` is the neuter singular / plural demonstrative.
    assert!(
        MARATHI.is_stopword("हे"),
        "neuter demonstrative हे must be a stopword"
    );
}

#[test]
fn common_marathi_conjunctions_are_stopwords() {
    for w in ["आणि", "किंवा", "पण", "जर", "की"] {
        assert!(
            MARATHI.is_stopword(w),
            "expected conjunction {w:?} to be a stopword"
        );
    }
}

#[test]
fn common_to_be_forms_are_stopwords() {
    for w in ["आहे", "आहेत", "होता", "होती", "होते"] {
        assert!(
            MARATHI.is_stopword(w),
            "expected to-be form {w:?} to be a stopword"
        );
    }
}

#[test]
fn negation_particles_are_stopwords() {
    for w in ["नाही", "नको"] {
        assert!(
            MARATHI.is_stopword(w),
            "expected negation particle {w:?} to be a stopword"
        );
    }
}

#[test]
fn independent_postpositions_are_stopwords() {
    // These are Marathi's *independent* postpositions — the ones
    // written as their own orthographic words. Marathi's agglutinative
    // case markers (-ला, -चा, -ने) are attached to the noun stem and
    // handled by the stemmer, not the stopword list.
    for w in ["साठी", "बद्दल", "बरोबर"] {
        assert!(
            MARATHI.is_stopword(w),
            "expected independent postposition {w:?} to be a stopword"
        );
    }
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["पुस्तक", "मराठी", "algorithm", "supercalifragilistic"] {
        assert!(!MARATHI.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_marathi_sentence() {
    let toks: Vec<&str> = MARATHI.tokenize("मी मराठी बोलतो").collect();
    assert_eq!(toks, ["मी", "मराठी", "बोलतो"]);
}

#[test]
fn tokenize_splits_on_danda() {
    // Marathi inherits the Devanagari danda (।) as its sentence
    // terminator (same convention as Hindi).
    let toks: Vec<&str> = MARATHI.tokenize("मी जातो।").collect();
    assert_eq!(toks, ["मी", "जातो"]);
}

#[test]
fn tokenize_preserves_matras_and_virama_inside_words() {
    // पुस्तक = प + ु + स + ् + त + क — the virama and matras must
    // stay word-internal.
    let toks: Vec<&str> = MARATHI.tokenize("पुस्तक हे चांगले").collect();
    assert_eq!(toks, ["पुस्तक", "हे", "चांगले"]);
}

#[test]
fn tokenize_preserves_marathi_specific_retroflex_l() {
    // शाळा "school" carries the Marathi-specific ळ (U+0933). Falls
    // inside the Devanagari block, so the tokenizer captures it as
    // word-internal.
    let toks: Vec<&str> = MARATHI.tokenize("शाळा आणि घर").collect();
    assert_eq!(toks, ["शाळा", "आणि", "घर"]);
}

#[test]
fn stem_agglutinative_dative_marker() {
    // घराला → घर (to the house). The stemmer strips -ला and the
    // convergence loop then peels off the exposed linking-vowel -ा.
    assert_eq!(MARATHI.stem("घराला"), "घर");
}

#[test]
fn stem_agglutinative_instrumental_marker() {
    // मुलाने → मुल (by the boy).
    assert_eq!(MARATHI.stem("मुलाने"), "मुल");
}

#[test]
fn stem_agglutinative_genitive_marker() {
    // मुलाचा → मुल (of the boy, m. sg. agreement).
    assert_eq!(MARATHI.stem("मुलाचा"), "मुल");
    // मुलाची → मुल (of the boy, f. sg. agreement).
    assert_eq!(MARATHI.stem("मुलाची"), "मुल");
}

#[test]
fn stem_verb_present_habitual() {
    // बोलतो → बोल (I / he speak(s) — m.).
    assert_eq!(MARATHI.stem("बोलतो"), "बोल");
    // बोलते → बोल (I / she speak(s) — f./n.).
    assert_eq!(MARATHI.stem("बोलते"), "बोल");
    // बोलतात → बोल (they speak).
    assert_eq!(MARATHI.stem("बोलतात"), "बोल");
}

#[test]
fn stem_verb_aorist() {
    // बोलला → बोल (he spoke).
    assert_eq!(MARATHI.stem("बोलला"), "बोल");
    // बोलल्या → बोल (they-f spoke).
    assert_eq!(MARATHI.stem("बोलल्या"), "बोल");
}

#[test]
fn stem_infinitive_marker() {
    // करणे → कर (to do).
    assert_eq!(MARATHI.stem("करणे"), "कर");
}

#[test]
fn stem_stacked_plural_plus_case_iterates_to_convergence() {
    // घरांना → घर (to the houses). The convergence loop reduces the
    // -ना case marker and the -ां plural marker in one external call.
    assert_eq!(MARATHI.stem("घरांना"), "घर");
}

#[test]
fn stem_leaves_bare_nouns_alone() {
    assert_eq!(MARATHI.stem("पुस्तक"), "पुस्तक");
}

#[test]
fn phonetic_encoder_is_phonex_mr() {
    let enc = MARATHI
        .phonetic_encoder()
        .expect("Marathi pack ships a phonetic encoder");
    assert_eq!(enc.name(), "phonex-mr");
    let (primary, alt) = enc.encode("राम").expect("encodes राम");
    assert_eq!(primary, "R500");
    assert!(alt.is_none());
}

#[test]
fn phonetic_encoder_handles_consonant_cluster_via_virama() {
    // सत्य → satya → S300 (S seed, A vow, T code=3, Y vow).
    let enc = MARATHI.phonetic_encoder().unwrap();
    let (primary, _) = enc.encode("सत्य").expect("encodes सत्य");
    assert_eq!(primary, "S300");
}

#[test]
fn phonetic_encoder_handles_marathi_retroflex_l() {
    // कमळ "lotus" → kamaḷa → KAMALA → K seed, A vow, M code=5, A vow,
    //   L code=4, A vow reset → "K54" → "K540".
    let enc = MARATHI.phonetic_encoder().unwrap();
    let (primary, _) = enc.encode("कमळ").expect("encodes कमळ");
    assert_eq!(primary, "K540");
}

#[test]
fn phonetic_encoder_returns_none_for_no_devanagari() {
    let enc = MARATHI.phonetic_encoder().unwrap();
    assert!(enc.encode("hello").is_none());
    assert!(enc.encode("").is_none());
    assert!(enc.encode("123").is_none());
}

#[test]
fn collator_is_none_by_default() {
    assert!(MARATHI.collator().is_none());
}
