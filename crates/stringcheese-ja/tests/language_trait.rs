//! Integration tests for the Japanese [`Language`] implementation.

use stringcheese_ja::JAPANESE;
use stringcheese_lang::Language;

#[test]
fn code_and_name() {
    assert_eq!(JAPANESE.code(), "ja");
    assert_eq!(JAPANESE.name(), "Japanese");
}

#[test]
fn common_japanese_particles_are_stopwords() {
    for w in [
        "は", "が", "を", "に", "で", "と", "から", "まで", "より", "の",
    ] {
        assert!(
            JAPANESE.is_stopword(w),
            "expected particle {w:?} to be a stopword"
        );
    }
}

#[test]
fn common_japanese_copulas_are_stopwords() {
    for w in ["だ", "です", "である", "ます"] {
        assert!(
            JAPANESE.is_stopword(w),
            "expected copula {w:?} to be a stopword"
        );
    }
}

#[test]
fn demonstratives_are_stopwords() {
    for w in [
        "これ", "それ", "あれ", "どれ", "この", "その", "あの", "どの",
    ] {
        assert!(
            JAPANESE.is_stopword(w),
            "expected demonstrative {w:?} to be a stopword"
        );
    }
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in [
        "桜",
        "コンピュータ",
        "algorithm",
        "supercalifragilisticexpialidocious",
    ] {
        assert!(!JAPANESE.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_mixed_script_sentence() {
    let toks: Vec<&str> = JAPANESE
        .tokenize("彼はJavaScriptを勉強しています")
        .collect();
    assert_eq!(toks, ["彼は", "JavaScript", "を", "勉強しています"]);
}

#[test]
fn stem_plural_and_polite_forms() {
    // Plural marker.
    assert_eq!(JAPANESE.stem("私たち"), "私");
    // Polite copula.
    assert_eq!(JAPANESE.stem("学生です"), "学生");
    // Polite auxiliary.
    assert_eq!(JAPANESE.stem("食べます"), "食べ");
}

#[test]
fn stem_leaves_bare_nouns_alone() {
    // No suffix matches on a bare noun — the stemmer returns the
    // input unchanged.
    assert_eq!(JAPANESE.stem("桜"), "桜");
    assert_eq!(JAPANESE.stem("日本"), "日本");
}

#[test]
fn phonetic_encoder_is_kunrei_romaji() {
    let enc = JAPANESE
        .phonetic_encoder()
        .expect("Japanese pack ships a phonetic encoder");
    assert_eq!(enc.name(), "kunrei-romaji");
    assert_eq!(
        enc.encode("さくら"),
        Some((String::from("sakura"), None)),
        "Kunrei(さくら) should be sakura with no alternate key"
    );
    assert_eq!(
        enc.encode("シ"),
        Some((String::from("si"), None)),
        "Kunrei(シ) should be si (not shi — Kunrei differs from Hepburn here)"
    );
}

#[test]
fn phonetic_encoder_returns_none_on_kanaless_input() {
    let enc = JAPANESE.phonetic_encoder().unwrap();
    assert!(enc.encode("").is_none());
    assert!(enc.encode("hello").is_none());
    // Kanji-only input has no kana to romanize — no phonetic key.
    assert!(enc.encode("日本").is_none());
}

#[test]
fn collator_is_none_by_default() {
    assert!(JAPANESE.collator().is_none());
}
