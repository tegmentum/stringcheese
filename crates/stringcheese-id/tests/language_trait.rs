//! Integration tests for the Indonesian [`Language`] implementation.

use stringcheese_id::INDONESIAN;
use stringcheese_lang::Language;

#[test]
fn code_and_name() {
    assert_eq!(INDONESIAN.code(), "id");
    assert_eq!(INDONESIAN.name(), "Indonesian");
}

#[test]
fn common_indonesian_words_are_stopwords() {
    for w in [
        "dan", "atau", "yang", "di", "ke", "dari", "pada", "dalam", "untuk", "dengan", "tidak",
        "bukan", "ini", "itu", "adalah", "ada", "sudah", "belum", "akan",
    ] {
        assert!(INDONESIAN.is_stopword(w), "expected {w:?} to be a stopword");
    }
}

#[test]
fn case_insensitive_stopword_lookup_uses_ascii_fold() {
    // Indonesian is ASCII-only; the default trait method (which uses
    // `str::eq_ignore_ascii_case`) handles case fold correctly.
    assert!(INDONESIAN.is_stopword("DAN"));
    assert!(INDONESIAN.is_stopword("YANG"));
    assert!(INDONESIAN.is_stopword("Dengan"));
    assert!(INDONESIAN.is_stopword("aDaLaH"));
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["buku", "rumah", "makan", "perpustakaan", "membaca"] {
        assert!(!INDONESIAN.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_indonesian_sentence() {
    let text = "Saya membaca buku di rumah.";
    let toks: Vec<&str> = INDONESIAN.tokenize(text).collect();
    assert_eq!(toks, ["Saya", "membaca", "buku", "di", "rumah"]);
}

#[test]
fn tokenize_reduplication_splits_at_hyphen() {
    let toks: Vec<&str> = INDONESIAN.tokenize("buku-buku baru").collect();
    assert_eq!(toks, ["buku", "buku", "baru"]);
}

#[test]
fn stem_a_few_indonesian_words() {
    // Assimilating prefixes with restoration.
    assert_eq!(INDONESIAN.stem("membaca"), "baca");
    assert_eq!(INDONESIAN.stem("memilih"), "pilih");
    assert_eq!(INDONESIAN.stem("menulis"), "tulis");
    assert_eq!(INDONESIAN.stem("menari"), "tari");
    assert_eq!(INDONESIAN.stem("menyapu"), "sapu");
    assert_eq!(INDONESIAN.stem("mengambil"), "ambil");
    assert_eq!(INDONESIAN.stem("melihat"), "lihat");
    // Non-assimilating prefixes.
    assert_eq!(INDONESIAN.stem("dibaca"), "baca");
    assert_eq!(INDONESIAN.stem("berjalan"), "jalan");
    assert_eq!(INDONESIAN.stem("terbaik"), "baik");
    // Derivational suffixes.
    assert_eq!(INDONESIAN.stem("makanan"), "makan");
    assert_eq!(INDONESIAN.stem("bacakan"), "baca");
    // Possessive suffixes.
    assert_eq!(INDONESIAN.stem("bukuku"), "buku");
    assert_eq!(INDONESIAN.stem("namanya"), "nama");
    // Circumfix (prefix + suffix).
    assert_eq!(INDONESIAN.stem("perbuatan"), "buat");
    // Special-case `belajar`.
    assert_eq!(INDONESIAN.stem("belajar"), "ajar");
}

#[test]
fn stopwords_are_not_stripped() {
    // Stopwords short-circuit at step 1; the stripper doesn't run.
    assert_eq!(INDONESIAN.stem("dan"), "dan");
    assert_eq!(INDONESIAN.stem("yang"), "yang");
    assert_eq!(INDONESIAN.stem("adalah"), "adalah");
}

#[test]
fn phonetic_encoder_is_phonex_id() {
    let enc = INDONESIAN
        .phonetic_encoder()
        .expect("Indonesian pack ships a phonetic encoder");
    assert_eq!(enc.name(), "phonex-id");
    let (primary, alt) = enc.encode("Jakarta").expect("PHONEX-ID encodes Jakarta");
    assert_eq!(primary, "J263");
    assert!(alt.is_none());
}

#[test]
fn collator_is_none_by_default() {
    assert!(INDONESIAN.collator().is_none());
}
