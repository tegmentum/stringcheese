//! The Persian stopword list.
//!
//! ~160 common Persian function words drawn from the intersection of
//! several well-known open lists (the Hazm Persian NLP toolkit's
//! stopword list, the Parsivar preprocessing pipeline, and the widely
//! reproduced Kharazi Persian stopwords). The list targets prepositions
//! (حروف اضافه), conjunctions (حروف ربط), personal and demonstrative
//! pronouns (ضمایر), interrogatives, the most-frequent surface forms
//! of *to be* / *to have*, common adverbs, and the low numerals; it
//! deliberately omits content words.
//!
//! # Right-to-left script — a note on storage order
//!
//! Every entry here is written in **logical order** — the order in
//! which the scalars appear in the UTF-8 encoded byte stream, which is
//! *first-consonant-first* regardless of how the string is displayed.
//! Rust source files, `str::eq_ignore_ascii_case`, and the default
//! [`is_stopword`](stringcheese_lang::Language::is_stopword) all work
//! on logical order; RTL rendering is a display-layer concern that
//! affects only what a human sees on screen, not what the byte
//! comparison sees.
//!
//! # Case folding
//!
//! Persian has no case distinction. The default
//! [`is_stopword`](stringcheese_lang::Language::is_stopword)
//! implementation calls [`str::eq_ignore_ascii_case`], which is a
//! *no-op* on Persian scalars (they are non-ASCII and pass through the
//! comparison unchanged); the case-insensitivity contract is
//! technically satisfied but vacuously so on Persian input.
//!
//! # Orthographic variants
//!
//! Persian input in the wild carries three orthographic uncertainties
//! the stopword list does not enumerate:
//!
//! - **Arabic yeh (`ي` U+064A) vs. Persian yeh (`ی` U+06CC).** The two
//!   scalars display identically in most fonts; keyboards and content
//!   management systems substitute one for the other silently. The
//!   stopword list stores the **Persian** form (`ی` U+06CC). Callers
//!   whose input may carry the Arabic form should run
//!   [`crate::normalize::normalize`] (which folds `ي → ی` by default)
//!   before consulting the stopword list.
//! - **Arabic kaf (`ك` U+0643) vs. Persian kaf (`ک` U+06A9).** Same
//!   story. The stopword list stores the **Persian** form (`ک`);
//!   normalize before consulting.
//! - **Zero-width non-joiner (ZWNJ, U+200C).** Persian compound words
//!   are written with a ZWNJ between joining morphemes (`می‌رود` "he
//!   goes"). Entries that would carry a ZWNJ *do* include it — the
//!   stopword list stores the surface form the input contains. If the
//!   caller has stripped ZWNJs with
//!   [`crate::normalize::PersianNormalizer::with_strip_zwnj`], the
//!   pre-normalization form of the entry is what will match.
//!
//! # Non-goals
//!
//! - **Dari / Tajik stopwords.** Dari (Afghan Persian) shares most
//!   function words with Iranian Persian but has some vocabulary
//!   divergence; Tajik uses Cyrillic script entirely. Neither is
//!   covered here.
//! - **Colloquial / spoken forms.** Written Persian and spoken
//!   Persian differ in the surface form of many function words
//!   (`می‌روم` "I go" formal vs. `می‌رم` colloquial). The list targets
//!   the written / newswire register.
//! - **Domain-specific stopwords.** IR practice for legal, medical, or
//!   scientific corpora typically extends the general list. Downstream
//!   applications should carry their own.

/// The Persian stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor hands
/// back exactly this slice.
///
/// Entries are stored in *logical* (byte) order; RTL rendering is a
/// display-layer concern.
pub const STOPWORDS: &[&str] = &[
    // -----------------------------------------------------------------
    // Prepositions (حروف اضافه).
    // -----------------------------------------------------------------
    "در",
    "از",
    "به",
    "با",
    "بر",
    "برای",
    "بی",
    "تا",
    "جز",
    "سوی",
    "پس",
    "پیش",
    "نزد",
    "روی",
    "زیر",
    "بالای",
    "پایین",
    "کنار",
    "میان",
    "بین",
    "درباره",
    "توسط",
    "مانند",
    "مثل",
    "غیر",
    "بدون",
    // -----------------------------------------------------------------
    // Coordinating and subordinating conjunctions (حروف ربط).
    // -----------------------------------------------------------------
    "و",
    "یا",
    "اما",
    "ولی",
    "لکن",
    "ولیکن",
    "اگر",
    "چون",
    "زیرا",
    "چراکه",
    "هرچند",
    "اگرچه",
    "بنابراین",
    "لذا",
    "چه",
    "همچنین",
    "همچنان",
    "همان‌طور",
    "همانطور",
    "که",
    "وقتی",
    "هنگامی",
    "زمانی",
    // -----------------------------------------------------------------
    // Personal pronouns (ضمایر شخصی).
    // -----------------------------------------------------------------
    "من",
    "تو",
    "او",
    "وی",
    "ما",
    "شما",
    "ایشان",
    "آنها",
    "آن‌ها",
    "اینها",
    "این‌ها",
    "خود",
    "خودم",
    "خودت",
    "خودش",
    "خودمان",
    "خودتان",
    "خودشان",
    // -----------------------------------------------------------------
    // Demonstrative pronouns (اسم اشاره).
    // -----------------------------------------------------------------
    "این",
    "آن",
    "همین",
    "همان",
    "چنین",
    "چنان",
    "اینجا",
    "آنجا",
    "اینک",
    "اکنون",
    // -----------------------------------------------------------------
    // Interrogatives (کلمات پرسشی). `چه` is a duplicate of the
    // conjunctions section — kept a set-oriented single entry there.
    // -----------------------------------------------------------------
    "چرا",
    "چگونه",
    "چطور",
    "کجا",
    "کی",
    "کی‌ست",
    "چند",
    "چقدر",
    "کدام",
    "کیست",
    "آیا",
    // -----------------------------------------------------------------
    // Negation particles.
    // -----------------------------------------------------------------
    "نه",
    "نی",
    "هرگز",
    "هیچ",
    // -----------------------------------------------------------------
    // "To be" (بودن) — the most-common surface forms.
    // -----------------------------------------------------------------
    "است",
    "هست",
    "نیست",
    "هستم",
    "هستی",
    "هستیم",
    "هستید",
    "هستند",
    "بود",
    "بودم",
    "بودی",
    "بودیم",
    "بودید",
    "بودند",
    "باشد",
    "باشم",
    "باشی",
    "باشیم",
    "باشید",
    "باشند",
    "بوده",
    "شد",
    "شده",
    "شدن",
    "می‌شود",
    "می‌شد",
    // -----------------------------------------------------------------
    // "To have" (داشتن) — the most-common surface forms.
    // -----------------------------------------------------------------
    "دارد",
    "دارم",
    "داری",
    "داریم",
    "دارید",
    "دارند",
    "داشت",
    "داشتم",
    "داشتی",
    "داشتیم",
    "داشتید",
    "داشتند",
    "داشته",
    // -----------------------------------------------------------------
    // Common adverbs and quantifiers.
    // -----------------------------------------------------------------
    "همه",
    "هر",
    "دیگر",
    "دیگری",
    "بعد",
    "قبل",
    "حالا",
    "الان",
    "امروز",
    "دیروز",
    "فردا",
    "همیشه",
    "هنوز",
    "خیلی",
    "بسیار",
    "کاملا",
    "کاملاً",
    "تقریبا",
    "تقریباً",
    "شاید",
    "حتی",
    "فقط",
    "تنها",
    "نیز",
    "هم",
    "بلکه",
    "یعنی",
    "مثلا",
    "مثلاً",
    "البته",
    // -----------------------------------------------------------------
    // Object marker and other grammatical particles.
    // -----------------------------------------------------------------
    "را",
    "ای",
    // -----------------------------------------------------------------
    // Low numerals — often filtered as noise in IR pipelines.
    // -----------------------------------------------------------------
    "یک",
    "دو",
    "سه",
    "چهار",
    "پنج",
    "شش",
    "هفت",
    "هشت",
    // "نه" as a numeral collides with "نه" as a negation particle
    // (listed above); the negation form is the more IR-relevant sense,
    // so this cell is intentionally omitted.
    "ده",
    // -----------------------------------------------------------------
    // Discourse markers.
    // -----------------------------------------------------------------
    "بله",
    "آری",
    "خیر",
    "طبعا",
    "طبعاً",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "~160" — assert we're in that
        // ballpark. The list has grown slightly during hand-curation.
        assert!(
            STOPWORDS.len() >= 100 && STOPWORDS.len() <= 250,
            "STOPWORDS.len() = {} outside the advertised 100-250 range",
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
    fn stopwords_contain_core_prepositions() {
        for &w in &["در", "از", "به", "با", "بر", "برای"] {
            assert!(STOPWORDS.contains(&w), "core preposition {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_personal_pronouns() {
        for &w in &["من", "تو", "او", "ما", "شما"] {
            assert!(STOPWORDS.contains(&w), "personal pronoun {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_demonstratives() {
        for &w in &["این", "آن", "همین", "همان"] {
            assert!(STOPWORDS.contains(&w), "demonstrative {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_core_conjunctions() {
        for &w in &["و", "یا", "اما", "ولی", "که"] {
            assert!(STOPWORDS.contains(&w), "conjunction {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_common_to_be_forms() {
        for &w in &["است", "هست", "بود", "باشد"] {
            assert!(STOPWORDS.contains(&w), "to-be form {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_common_to_have_forms() {
        for &w in &["دارد", "دارم", "داشت"] {
            assert!(STOPWORDS.contains(&w), "to-have form {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_object_marker() {
        assert!(STOPWORDS.contains(&"را"), "object marker را missing");
    }

    #[test]
    fn no_duplicates() {
        // O(n^2) is fine for a static list of ~160.
        for (i, &w) in STOPWORDS.iter().enumerate() {
            for &v in &STOPWORDS[i + 1..] {
                assert_ne!(w, v, "duplicate stopword: {w:?}");
            }
        }
    }
}
