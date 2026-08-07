//! Integration tests for the Chinese [`Language`] implementation.

use stringcheese_lang::Language;
use stringcheese_zh::CHINESE;

#[test]
fn code_and_name() {
    assert_eq!(CHINESE.code(), "zh");
    assert_eq!(CHINESE.name(), "Chinese");
}

#[test]
fn common_chinese_function_words_are_stopwords() {
    for w in ["的", "了", "是", "在", "有", "我", "和", "不"] {
        assert!(
            CHINESE.is_stopword(w),
            "expected function word {w:?} to be a stopword"
        );
    }
}

#[test]
fn common_chinese_pronouns_are_stopwords() {
    for w in ["我", "你", "他", "她", "它", "我们", "你们", "他们"] {
        assert!(
            CHINESE.is_stopword(w),
            "expected pronoun {w:?} to be a stopword"
        );
    }
}

#[test]
fn demonstratives_and_interrogatives_are_stopwords() {
    for w in ["这", "那", "这个", "那个", "什么", "哪", "谁"] {
        assert!(
            CHINESE.is_stopword(w),
            "expected demonstrative/interrogative {w:?} to be a stopword"
        );
    }
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in [
        "桜",
        "北京",
        "algorithm",
        "supercalifragilisticexpialidocious",
    ] {
        assert!(!CHINESE.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_mixed_script_sentence() {
    let toks: Vec<&str> = CHINESE.tokenize("中文hello123").collect();
    assert_eq!(toks, ["中", "文", "hello", "123"]);
}

#[test]
fn tokenize_han_only_splits_every_character() {
    let toks: Vec<&str> = CHINESE.tokenize("你好世界").collect();
    assert_eq!(toks, ["你", "好", "世", "界"]);
}

#[test]
fn tokenize_with_chinese_punctuation() {
    // Chinese full-width punctuation acts as a separator.
    let toks: Vec<&str> = CHINESE.tokenize("你好，世界！").collect();
    assert_eq!(toks, ["你", "好", "世", "界"]);
}

#[test]
fn stem_is_identity() {
    // Chinese has no inflection — every input passes through
    // unchanged.
    assert_eq!(CHINESE.stem("我"), "我");
    assert_eq!(CHINESE.stem("我们"), "我们");
    assert_eq!(CHINESE.stem("中国"), "中国");
    assert_eq!(CHINESE.stem("hello"), "hello");
}

#[test]
fn phonetic_encoder_is_pinyin() {
    let enc = CHINESE
        .phonetic_encoder()
        .expect("Chinese pack ships a phonetic encoder");
    assert_eq!(enc.name(), "pinyin-zh");
    assert_eq!(
        enc.encode("你好"),
        Some((String::from("ni hao"), None)),
        "Pinyin(你好) should be `ni hao` with no alternate key"
    );
    assert_eq!(
        enc.encode("中国"),
        Some((String::from("zhong guo"), None)),
        "Pinyin(中国) should be `zhong guo`"
    );
}

#[test]
fn phonetic_encoder_returns_none_on_empty_input() {
    let enc = CHINESE.phonetic_encoder().unwrap();
    assert!(enc.encode("").is_none());
    // Pure whitespace / punctuation has no token content.
    assert!(enc.encode("   ").is_none());
    assert!(enc.encode("。，！").is_none());
}

#[test]
fn collator_is_none_by_default() {
    assert!(CHINESE.collator().is_none());
}

#[test]
fn stopwords_slice_is_non_empty() {
    assert!(!CHINESE.stopwords().is_empty());
    // The specific 50-120 range mirrors the src/stopwords.rs test.
    assert!(CHINESE.stopwords().len() >= 50);
}
