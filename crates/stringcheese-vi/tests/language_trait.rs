//! Integration tests for the Vietnamese [`Language`] implementation.

use stringcheese_lang::Language;
use stringcheese_vi::VIETNAMESE;

#[test]
fn code_and_name() {
    assert_eq!(VIETNAMESE.code(), "vi");
    assert_eq!(VIETNAMESE.name(), "Vietnamese");
}

#[test]
fn common_vietnamese_words_are_stopwords() {
    for w in [
        "và", "là", "có", "được", "không", "trong", "của", "với", "để", "tôi", "bạn",
    ] {
        assert!(VIETNAMESE.is_stopword(w), "expected {w:?} to be a stopword");
    }
}

#[test]
fn case_insensitive_stopword_lookup_uses_unicode_fold() {
    // Vietnamese case-fold (Rust default Unicode fold): A → a,
    // Ă → ă, Đ → đ, Ơ → ơ, Ư → ư, Ệ → ệ. Uppercase queries
    // fold correctly to the plain lowercase list.
    assert!(VIETNAMESE.is_stopword("VÀ"));
    assert!(VIETNAMESE.is_stopword("Và"));
    assert!(VIETNAMESE.is_stopword("ĐƯỢC"));
    assert!(VIETNAMESE.is_stopword("Được"));
    assert!(VIETNAMESE.is_stopword("NHỮNG"));
    assert!(!VIETNAMESE.is_stopword("SÁCH")); // Not a stopword.
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["sách", "học", "sinh", "máy", "tính", "phở"] {
        assert!(!VIETNAMESE.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_vietnamese_sentence() {
    let text = "Học sinh đọc sách.";
    let toks: Vec<&str> = VIETNAMESE.tokenize(text).collect();
    assert_eq!(toks, ["Học", "sinh", "đọc", "sách"]);
}

#[test]
fn tokenize_preserves_diacritics_in_tokens() {
    let text = "Nguyễn Văn Đông đi học ở Hà Nội.";
    let toks: Vec<&str> = VIETNAMESE.tokenize(text).collect();
    assert_eq!(
        toks,
        ["Nguyễn", "Văn", "Đông", "đi", "học", "ở", "Hà", "Nội"]
    );
}

#[test]
fn stem_is_identity_on_nfc_input() {
    // Vietnamese is analytic — the "stemmer" is an NFC canonicalizer,
    // not a suffix stripper.
    assert_eq!(VIETNAMESE.stem("được"), "được");
    assert_eq!(VIETNAMESE.stem("học sinh"), "học sinh");
    assert_eq!(VIETNAMESE.stem("nước"), "nước");
}

#[test]
fn stem_recomposes_nfd_input_to_nfc() {
    // NFD `ệ` (e + circumflex + dot-below) → NFC `ệ`.
    assert_eq!(VIETNAMESE.stem("e\u{0302}\u{0323}"), "ệ");
}

#[test]
fn phonetic_encoder_is_phonex_vi() {
    let enc = VIETNAMESE
        .phonetic_encoder()
        .expect("Vietnamese pack ships a phonetic encoder");
    assert_eq!(enc.name(), "phonex-vi");
    assert_eq!(
        enc.encode("Nguyễn"),
        Some((String::from("N500"), None)),
        "PHONEX-VI(Nguyễn) should be N500 with no alternate key"
    );
    assert_eq!(
        enc.encode("Trần"),
        Some((String::from("T500"), None)),
        "PHONEX-VI(Trần) should be T500"
    );
}

#[test]
fn phonetic_encoder_collapses_tone_variants() {
    // All six tone variants of `ban` collapse to the same PHONEX key
    // because every diacritic is stripped in preprocessing.
    let enc = VIETNAMESE.phonetic_encoder().unwrap();
    let baseline = enc.encode("ban").unwrap().0;
    for w in ["bàn", "bán", "bản", "bãn", "bạn"] {
        let (k, _) = enc.encode(w).unwrap();
        assert_eq!(
            k, baseline,
            "tone variant {w:?} did not collapse to {baseline:?}"
        );
    }
}

#[test]
fn phonetic_encoder_folds_ng_and_nh_to_nasal() {
    let enc = VIETNAMESE.phonetic_encoder().unwrap();
    // `Nga` (NG→N) and `Na` produce the same key.
    let (a, _) = enc.encode("Nga").unwrap();
    let (b, _) = enc.encode("Na").unwrap();
    assert_eq!(a, b, "NG-N merger failed");
    // `Nha` (NH→N) and `Na` produce the same key.
    let (c, _) = enc.encode("Nha").unwrap();
    assert_eq!(c, b, "NH-N merger failed");
}

#[test]
fn collator_is_none_by_default() {
    assert!(VIETNAMESE.collator().is_none());
}
