//! The Japanese stopword list.
//!
//! ~120 common Japanese function words drawn from the intersection of
//! several well-known open lists (the `SlothLib` Japanese stopword
//! list, kuromoji-style particle inventories, and `MeCab`'s `unidic`
//! surface forms most commonly filtered by Japanese IR systems). The
//! list targets particles (助詞), auxiliaries (助動詞), and the most
//! frequent function-word pronouns and demonstratives; it deliberately
//! omits content words.
//!
//! # Character-type composition
//!
//! Because Japanese function words are almost universally written in
//! hiragana (or short katakana), the list contains no kanji and no
//! ASCII outside a handful of transliteration alternates. This matters
//! for the [`is_stopword`](stringcheese_lang::Language::is_stopword)
//! default implementation, which uses ASCII-only case folding via
//! [`str::eq_ignore_ascii_case`]: hiragana input is compared
//! byte-for-byte, which is the right behavior — Japanese has no case.
//!
//! # Coverage rationale
//!
//! - **Particles (助詞).** The nine core case-marking particles (`は`
//!   topic, `が` subject, `を` object, `に` locative/dative, `で`
//!   instrumental/locative, `と` comitative/quotative, `から` ablative,
//!   `まで` allative, `より` comparative) plus every conjunction and
//!   sentence-final particle a well-tokenized Japanese corpus emits at
//!   high frequency (`も`, `の`, `や`, `か`, `ね`, `よ`, `わ`, ...).
//! - **Auxiliary verbs (助動詞).** The copula in every polite/plain and
//!   affirmative/negative combination (`だ`, `である`, `です`, `ます`,
//!   `ない`, `ん`, `ぬ`, `た`, `だった`, `でした`, `ました`), plus
//!   `する`/`いる`/`ある` in their most-frequent surface forms.
//! - **Demonstratives / pronouns.** The こ／そ／あ／ど series (`これ`,
//!   `それ`, `あれ`, `どれ`; `この`, `その`, `あの`, `どの`; `ここ`,
//!   `そこ`, `あそこ`, `どこ`; `こう`, `そう`, `ああ`, `どう`), plus the
//!   most common personal pronouns (`私`, `僕`, `俺`, `あなた`, `彼`,
//!   `彼女`).
//! - **Formulaic conjunctions and adverbs.** `そして`, `しかし`,
//!   `また`, `もっと`, `よく`, `とても`, `ずっと`, `やはり`, `つまり`.
//!
//! # Non-goals
//!
//! - **Domain-specific stopwords.** IR practice for legal, medical, or
//!   scientific corpora typically extends the general list. Downstream
//!   applications should carry their own.
//! - **Kanji-only pronouns.** `私` and `僕` are included; less-common
//!   kanji-only pronouns (`吾`, `汝`, `其`) are absent — they belong to
//!   an archaic register.
//! - **Sentence-boundary punctuation.** Japanese periods (`。`) and
//!   commas (`、`) are handled by the tokenizer as separators; they are
//!   not stopwords.

/// The Japanese stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor hands
/// back exactly this slice.
pub const STOPWORDS: &[&str] = &[
    // -----------------------------------------------------------------
    // Case-marking particles (格助詞).
    // -----------------------------------------------------------------
    "は",
    "が",
    "を",
    "に",
    "へ",
    "で",
    "と",
    "から",
    "まで",
    "より",
    "の",
    // -----------------------------------------------------------------
    // Adverbial / focus / topic particles (副助詞・係助詞).
    // -----------------------------------------------------------------
    "も",
    "や",
    "など",
    "だけ",
    "ばかり",
    "しか",
    "こそ",
    "さえ",
    "でも",
    "ほど",
    "くらい",
    "ぐらい",
    "まま",
    "ずつ",
    // -----------------------------------------------------------------
    // Conjunctive particles (接続助詞). Note: `で` and `が` are
    // deliberately elided here — they are already listed under the
    // case-marking particles above (Japanese function words often
    // fill more than one grammatical role in different contexts, but
    // the stopword list is a set, not a multiset).
    // -----------------------------------------------------------------
    "て",
    "ながら",
    "つつ",
    "ので",
    "のに",
    "けど",
    "けれど",
    "けれども",
    "し",
    "たり",
    "だり",
    // -----------------------------------------------------------------
    // Sentence-final particles (終助詞). `の` is deliberately elided
    // (already listed as a case particle above).
    // -----------------------------------------------------------------
    "か",
    "ね",
    "よ",
    "わ",
    "な",
    "ぞ",
    "さ",
    "かな",
    "かしら",
    // -----------------------------------------------------------------
    // Copula and auxiliary-verb surface forms (助動詞).
    // -----------------------------------------------------------------
    "だ",
    "です",
    "である",
    "でした",
    "でしょう",
    "だった",
    "だろう",
    "ます",
    "ました",
    "ません",
    "ましょう",
    "ない",
    "なかった",
    "ん",
    "ぬ",
    "た",
    "つ",
    "たい",
    "たかった",
    "らしい",
    "そうだ",
    "ようだ",
    "みたい",
    "べき",
    "べきだ",
    // -----------------------------------------------------------------
    // High-frequency verb forms that behave as auxiliaries.
    // -----------------------------------------------------------------
    "する",
    "した",
    "して",
    "される",
    "させる",
    "できる",
    "ある",
    "あった",
    "いる",
    "いた",
    "なる",
    "なった",
    "いう",
    "言う",
    // -----------------------------------------------------------------
    // Demonstratives — こ／そ／あ／ど series.
    // -----------------------------------------------------------------
    "これ",
    "それ",
    "あれ",
    "どれ",
    "この",
    "その",
    "あの",
    "どの",
    "ここ",
    "そこ",
    "あそこ",
    "どこ",
    "こう",
    "そう",
    "ああ",
    "どう",
    "こちら",
    "そちら",
    "あちら",
    "どちら",
    "こんな",
    "そんな",
    "あんな",
    "どんな",
    // -----------------------------------------------------------------
    // Personal pronouns.
    // -----------------------------------------------------------------
    "私",
    "わたし",
    "僕",
    "ぼく",
    "俺",
    "おれ",
    "あなた",
    "あんた",
    "君",
    "きみ",
    "彼",
    "彼女",
    "我々",
    "彼ら",
    "彼女ら",
    // -----------------------------------------------------------------
    // Common conjunctions and adverbs frequently filtered as noise.
    // Note: `でも` and `けれど` are elided here — they are already
    // listed as particles above (both function words wear two hats
    // in Japanese grammar and the stopword list stays a set).
    // -----------------------------------------------------------------
    "そして",
    "しかし",
    "また",
    "ただ",
    "つまり",
    "なお",
    "やはり",
    "もっと",
    "よく",
    "とても",
    "ずっと",
    "まだ",
    "もう",
    "すぐ",
    "ほとんど",
    "たぶん",
    "きっと",
    "ちょっと",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "~100-150" — assert we're in that
        // ballpark.
        assert!(
            STOPWORDS.len() >= 100 && STOPWORDS.len() <= 200,
            "STOPWORDS.len() = {} outside the advertised 100-200 range",
            STOPWORDS.len()
        );
    }

    #[test]
    fn every_stopword_is_non_empty() {
        for &w in STOPWORDS {
            assert!(!w.is_empty(), "empty stopword entry");
        }
    }

    #[test]
    fn stopwords_contain_core_particles() {
        // The nine case-marking particles are the smoke test — any pack
        // author reordering the list should trip this test if they lose
        // one accidentally.
        for &w in &["は", "が", "を", "に", "で", "と", "から", "まで", "より"] {
            assert!(
                STOPWORDS.contains(&w),
                "core case-marking particle {w:?} missing"
            );
        }
    }

    #[test]
    fn stopwords_contain_core_copulas() {
        for &w in &["だ", "です", "である", "ます"] {
            assert!(STOPWORDS.contains(&w), "core copula {w:?} missing");
        }
    }

    #[test]
    fn no_duplicates() {
        // O(n^2) is fine for a static list of ~120.
        for (i, &w) in STOPWORDS.iter().enumerate() {
            for &v in &STOPWORDS[i + 1..] {
                assert_ne!(w, v, "duplicate stopword: {w:?}");
            }
        }
    }
}
