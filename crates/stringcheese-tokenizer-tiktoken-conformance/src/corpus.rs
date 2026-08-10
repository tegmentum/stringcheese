//! Parity-harness corpus.
//!
//! ~200 diverse inputs organised by category (English prose, source
//! code, JSON, whitespace-heavy, non-Latin scripts, emoji, edge
//! cases). Every input is a `&'static str` embedded in the binary
//! so tests are hermetic — the corpus has no on-disk footprint and
//! callers can run parity offline once the vocabulary blobs are
//! cached.
//!
//! Phase 3's design-doc acceptance criterion is 10 000 diverse
//! inputs. 200 is enough to prove the harness works and to
//! surface any structural divergence between
//! `stringcheese-tokenizer-bpe` and tiktoken; scaling the corpus is
//! a follow-on change that only touches this module.
//!
//! When a new category shows up in a real bug report, extend
//! [`CORPUS`] and (ideally) add a category label to
//! [`categorised_len`] so the pass/fail summary can attribute a
//! divergence to a coverage bucket.

/// The parity corpus.
///
/// Ordered so that "boring" prose comes first — a divergence there
/// tends to point at pre-tokenizer regex issues, which are the
/// widest-blast-radius category. Later categories exercise more
/// specific paths (unicode segmentation, byte-level BPE fallback,
/// contraction handling).
pub const CORPUS: &[&str] = &[
    // ---- English prose (30) --------------------------------------
    "Hello, world!",
    "The quick brown fox jumps over the lazy dog.",
    "She sells seashells by the seashore.",
    "How much wood would a woodchuck chuck if a woodchuck could chuck wood?",
    "To be, or not to be, that is the question.",
    "It was the best of times, it was the worst of times.",
    "All happy families are alike; each unhappy family is unhappy in its own way.",
    "In a hole in the ground there lived a hobbit.",
    "Call me Ishmael.",
    "The past is a foreign country: they do things differently there.",
    "It is a truth universally acknowledged, that a single man in possession of a good fortune, must be in want of a wife.",
    "Happy families are all alike; every unhappy family is unhappy in its own way.",
    "A screaming comes across the sky.",
    "It was a bright cold day in April, and the clocks were striking thirteen.",
    "Mother died today. Or, maybe, yesterday; I can't be sure.",
    "The sky above the port was the color of television, tuned to a dead channel.",
    "Many years later, as he faced the firing squad, Colonel Aureliano Buendia was to remember that distant afternoon when his father took him to discover ice.",
    "It was a pleasure to burn.",
    "Ships at a distance have every man's wish on board.",
    "You don't know about me without you have read a book by the name of The Adventures of Tom Sawyer; but that ain't no matter.",
    "Whether I shall turn out to be the hero of my own life, or whether that station will be held by anybody else, these pages must show.",
    "riverrun, past Eve and Adam's, from swerve of shore to bend of bay, brings us by a commodius vicus of recirculation back to Howth Castle and Environs.",
    "Someone must have slandered Josef K., for one morning, without having done anything wrong, he was arrested.",
    "The story so far: In the beginning the Universe was created. This has made a lot of people very angry and been widely regarded as a bad move.",
    "There was a boy called Eustace Clarence Scrubb, and he almost deserved it.",
    "As Gregor Samsa awoke one morning from uneasy dreams he found himself transformed in his bed into a gigantic insect.",
    "The cat sat on the mat.",
    "I have a dream that one day this nation will rise up and live out the true meaning of its creed.",
    "Ask not what your country can do for you; ask what you can do for your country.",
    "We hold these truths to be self-evident, that all men are created equal.",
    // ---- Contractions and clitics (10) ---------------------------
    // Deliberately exercises the tiktoken canonical regex's
    // `(?i:'s|'t|'re|'ve|'m|'ll|'d)` alternative.
    "I've got what you're looking for; don't be shy.",
    "It's a shame he'd not have known she'll be back.",
    "we're gonna've been where they've're not.",
    "y'all shouldn't've said that; ain't that right?",
    "Don't. Can't. Won't. Shan't.",
    "She'd've told him if he'd've listened.",
    "I'm the one who's here to say it's over.",
    "Let's see what we've got — you'll like it.",
    "You'd better believe I'd have done it if I'd been there.",
    "Who's on first, what's on second, I don't know's on third.",
    // ---- Source code snippets (25) -------------------------------
    "fn main() { println!(\"Hello, world!\"); }",
    "let x: u32 = 42;",
    "for i in 0..10 { println!(\"{i}\"); }",
    "if let Some(v) = maybe_v { do(v); } else { fallback(); }",
    "impl<T: Clone + Send> Foo for Bar<T> where T: 'static {}",
    "pub struct Point3d { pub x: f64, pub y: f64, pub z: f64 }",
    "match tok.kind { TokenKind::Ident => {} TokenKind::Number => {} _ => panic!() }",
    "def fibonacci(n: int) -> int:\n    if n < 2:\n        return n\n    return fibonacci(n - 1) + fibonacci(n - 2)",
    "const arr = [1, 2, 3].map((x) => x * 2).reduce((a, b) => a + b, 0);",
    "class MyClass:\n    def __init__(self, x):\n        self.x = x",
    "SELECT id, name, COUNT(*) FROM users GROUP BY id, name HAVING COUNT(*) > 1 ORDER BY name DESC;",
    "#include <stdio.h>\nint main(int argc, char** argv) { return 0; }",
    "// TODO: refactor this into a proper module\n// FIXME(#42): edge case with zero-length input",
    "type Result<T, E = Box<dyn Error>> = std::result::Result<T, E>;",
    "async fn fetch(url: &str) -> Result<Response> { reqwest::get(url).await }",
    "vec![1u8, 2, 3, 4].iter().copied().collect::<Vec<_>>()",
    "let (a, b) = (1, 2); let (c, d, e) = (3, 4, 5); let sum = a + b + c + d + e;",
    "macro_rules! foo { ($x:expr) => { println!(\"{}\", $x) } }",
    "trait Serialize { fn to_bytes(&self) -> Vec<u8>; }",
    "use std::collections::{HashMap, HashSet, BTreeMap};",
    "fn add<T: std::ops::Add<Output = T>>(a: T, b: T) -> T { a + b }",
    "let _ = (0..1000).map(|x| x * x).sum::<i64>();",
    "print(f\"total: {sum(x for x in range(10) if x % 2)}\")",
    "return {\"status\": \"ok\", \"data\": [1, 2, 3], \"error\": null};",
    "System.out.println(\"Hello, World!\");",
    // ---- JSON (15) -----------------------------------------------
    r#"{"name":"Ada","age":36}"#,
    r"[1,2,3,4,5,6,7,8,9,10]",
    r#"{"nested":{"key":[true,false,null]}}"#,
    r#"{"unicode":"caf\u00e9","emoji":"\ud83d\ude00"}"#,
    r#"{"empty":"","zero":0,"neg":-1.5}"#,
    r#"{"array_of_objects":[{"id":1},{"id":2},{"id":3}]}"#,
    r#"{"large_number":123456789012345,"float":3.14159265358979}"#,
    r#"{"escaped":"line one\nline two\ttab\"quote\"","backslash":"a\\b"}"#,
    r#"{"key with spaces":"value with spaces","key.with.dots":"..."}"#,
    r#"{"emoji_key":"\ud83d\ude80","emoji_val":"\ud83c\udf89"}"#,
    r"[[1,2],[3,4],[5,6]]",
    r#"{"a":1,"b":2,"c":3,"d":4,"e":5,"f":6,"g":7,"h":8,"i":9,"j":10}"#,
    r#"{"config":{"host":"localhost","port":8080,"ssl":true,"timeout":30}}"#,
    r#"{"errors":[{"code":400,"message":"Bad Request"},{"code":500,"message":"Server Error"}]}"#,
    r#"{"tags":["rust","tokenizer","bpe","tiktoken"]}"#,
    // ---- Whitespace-heavy (15) -----------------------------------
    "  leading spaces",
    "trailing spaces   ",
    "\ttabs\tbetween\twords",
    "\n\nblank lines\n\nbetween\n\n",
    "mixed \t\t whitespace \n\n runs",
    "single_word",
    "                                                    ",
    "\n",
    "\t",
    " ",
    "double  spaces  everywhere",
    "\n\n\n\n\n\n\n",
    "line_one\nline_two\nline_three",
    " a  b   c    d     e",
    "\r\ncrlf\r\nline\r\nbreaks\r\n",
    // ---- Unicode / non-Latin (30) --------------------------------
    "café résumé naïve",
    "Ångström Ω μ π φ",
    "Здравствуй, мир!",
    "こんにちは世界",
    "안녕하세요 세계",
    "你好，世界",
    "مرحبا بالعالم",
    "שלום עולם",
    "नमस्ते दुनिया",
    "สวัสดีชาวโลก",
    "Γεια σου κόσμε",
    "Ni hao, shi jie",
    "Bonjour tout le monde — comment ça va aujourd'hui ?",
    "Guten Tag! Wie geht es Ihnen?",
    "こんにちは。今日はいい天気ですね。",
    "北京大学计算机科学与技术系",
    "Ivan Ivanovich lives in Санкт-Петербург",
    "El niño está en la escuela",
    "Kanji: 日本語, hiragana: にほんご, katakana: ニホンゴ",
    "한글은 세종대왕이 만들었어요.",
    "Ελληνικά: Η γλώσσα της αρχαίας Ελλάδας",
    "העברית היא שפה שמית",
    "العربية لغة سامية",
    "ខ្មែរ ជាភាសាមួយក្នុងអាស៊ីអាគ្នេយ៍",
    "བོད་སྐད། Tibetan script",
    "ᏣᎳᎩ (Cherokee)",
    "Ⓜⓘⓧⓔⓓ ⓣⓔⓧⓣ",
    "𝔊𝔬𝔱𝔥𝔦𝔠 𝔉𝔯𝔞𝔨𝔱𝔲𝔯",
    "Íslenska: Halló heimur!",
    "Türkçe: Merhaba dünya!",
    // ---- Emoji + presentation selectors (20) ---------------------
    "😀 😃 😄 😁 😆",
    "👨‍💻 👩‍🚀 👨‍👩‍👧‍👦",
    "🇺🇸 🇬🇧 🇯🇵 🇨🇳 🇩🇪",
    "❤️ 💔 💕 💖 💗",
    "🌍🌎🌏 🌐",
    "\u{1F44D}\u{1F3FF}",
    "\u{2764}\u{FE0F}",
    "\u{1F469}\u{200D}\u{1F52C}",
    "🚀 to the moon",
    "coffee ☕ tea 🍵 water 💧",
    "🎉🎊🥳",
    "The 🐈 sat on the 🪑",
    "🏳️‍🌈 🏳️‍⚧️",
    "🧑‍🤝‍🧑",
    "0️⃣1️⃣2️⃣3️⃣",
    "☺︎ ☹︎ ☠︎",
    "♠♥♦♣ playing cards",
    "★ ☆ ✩ ✪ ✫",
    "→ ← ↑ ↓ ⇒",
    "∀ x ∈ ℝ, ∃ y > 0 s.t. |y - x| < ε",
    // ---- Numbers and mathematical (15) ---------------------------
    "1234567890",
    "3.14159265358979323846",
    "-1.234e-10",
    "0xdeadbeef",
    "0b10101010",
    "42 + 8 = 50",
    "pi ≈ 3.14, e ≈ 2.72, phi ≈ 1.62",
    "10^100 is a googol",
    "$1,234,567.89",
    "€1.234,56",
    "100%",
    "1/2 + 1/4 + 1/8 = 7/8",
    "10:00 AM",
    "2025-01-01T00:00:00Z",
    "+1-555-123-4567",
    // ---- URLs, paths, identifiers (15) ---------------------------
    "https://example.com/path/to/resource?query=value&other=1#fragment",
    "/usr/local/bin/rustc",
    "C:\\Windows\\System32\\cmd.exe",
    "user@example.com",
    "some-kebab-case-identifier",
    "some_snake_case_identifier",
    "SomeCamelCaseIdentifier",
    "SOME_SCREAMING_SNAKE_CASE",
    "com.example.package.ClassName",
    "http://localhost:3000/api/v1/users/42",
    "file:///home/user/document.txt",
    "git@github.com:tegmentum/stringcheese.git",
    "s3://bucket-name/path/to/object.bin",
    "postgres://user:pass@host:5432/dbname",
    "ldap://directory.example.com:389/cn=admin,dc=example,dc=com",
    // ---- Repeated / patterned (10) -------------------------------
    "aaaaaaaaaa",
    "abababababab",
    "abcabcabcabc",
    "0000000000",
    "!!!!!!!!!!",
    "hahahahaha",
    "                    ",
    "ononononon",
    "the the the the the",
    "!@#$%^&*()_+-={}[]|\\:;\"'<>?,./",
    // ---- Edge cases (15) -----------------------------------------
    "",
    "a",
    " ",
    "\0",
    "\x7f",
    "🦀",
    "\"",
    "\\",
    "\n",
    "  ",
    "\t\t\t",
    "\u{FEFF}zero width joiner",
    "\u{200B}\u{200C}\u{200D}",
    "combining: e\u{0301} vs. \u{00E9}",
    "long_word_no_spaces_supercalifragilisticexpialidocious",
];

/// The corpus categories used in the parity report summary. Kept
/// aligned with the section boundaries in [`CORPUS`]. If you extend
/// the corpus in a way that changes the section counts, update these
/// counts too — the sum must equal `CORPUS.len()`.
pub const CATEGORY_COUNTS: &[(&str, usize)] = &[
    ("english_prose", 30),
    ("contractions", 10),
    ("source_code", 25),
    ("json", 15),
    ("whitespace_heavy", 15),
    ("unicode_non_latin", 30),
    ("emoji_and_symbols", 20),
    ("numbers_and_math", 15),
    ("urls_paths_ids", 15),
    ("repeated_patterned", 10),
    ("edge_cases", 15),
];

/// Which category `CORPUS[idx]` belongs to. Used in the parity
/// report so a divergence can be attributed to the right coverage
/// bucket without a caller having to memorise the section
/// boundaries.
#[must_use]
pub fn category_for(idx: usize) -> &'static str {
    let mut running = 0usize;
    for &(name, count) in CATEGORY_COUNTS {
        running += count;
        if idx < running {
            return name;
        }
    }
    "uncategorised"
}

/// Total corpus size. `const` so downstream harnesses can assert
/// against it at compile time when they extend the corpus.
pub const CORPUS_LEN: usize = CORPUS.len();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_counts_sum_to_corpus_len() {
        let sum: usize = CATEGORY_COUNTS.iter().map(|(_, n)| *n).sum();
        assert_eq!(
            sum, CORPUS_LEN,
            "CATEGORY_COUNTS ({sum}) must sum to CORPUS.len() ({CORPUS_LEN}) — \
             update the counts when adding entries"
        );
    }

    #[test]
    fn every_index_has_a_category() {
        for i in 0..CORPUS_LEN {
            let cat = category_for(i);
            assert_ne!(cat, "uncategorised", "index {i} fell off the end");
        }
    }

    #[test]
    fn corpus_covers_all_bytes_in_edge_cases() {
        // Sanity: the edge-case bucket includes at least the
        // pathological single-byte inputs (empty, U+0000, tab).
        assert!(CORPUS.contains(&""));
        assert!(CORPUS.contains(&"\0"));
        assert!(CORPUS.contains(&"\t\t\t"));
    }
}
