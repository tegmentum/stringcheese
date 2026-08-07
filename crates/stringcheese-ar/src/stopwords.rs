//! The Arabic stopword list.
//!
//! ~150 common Arabic function words drawn from the intersection of
//! several well-known open lists (the NLTK Arabic stopword list, the
//! Snowball Arabic stopword tradition, and Kareem Darwish's TAWA
//! Arabic-IR pipeline). The list targets particles (حروف), prepositions
//! (حروف الجر), conjunctions (حروف العطف), demonstrative and personal
//! pronouns (الضمائر), and the most-frequent verb forms of *to be* /
//! *to have*; it deliberately omits content words.
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
//! Arabic has no case distinction. The default
//! [`is_stopword`](stringcheese_lang::Language::is_stopword)
//! implementation calls [`str::eq_ignore_ascii_case`], which is a
//! *no-op* on Arabic scalars (they are non-ASCII and pass through the
//! comparison unchanged); the case-insensitivity contract is
//! technically satisfied but vacuously so on Arabic input. This is the
//! right behavior — comparing Arabic function words byte-for-byte
//! against the list is what a stopword filter wants.
//!
//! # Diacritics and orthographic variants
//!
//! Arabic input in the wild comes with three orthographic uncertainties
//! the stopword list does not enumerate:
//!
//! - **Harakat (diacritics).** Fully-vocalized text (Qur'anic verses,
//!   pedagogical corpora) writes short vowels with combining diacritics
//!   (`فَتْحَة`, `كَسْرَة`, `ضَمَّة`, `سُكُون`, `شَدَّة`, `تَنْوِين`, `مَدَّة`); newswire
//!   and web text almost always strips them (`فتحة`, `كسرة`, `ضمة`).
//!   The stopword list uses the *stripped* form only. Callers whose
//!   input may carry diacritics should run
//!   [`crate::normalize::normalize`] before consulting the stopword
//!   list.
//! - **Alef variants.** Word-initial hamzated alef (`أ`, `إ`, `آ`) and
//!   plain alef (`ا`) are distinguished by orthography but are
//!   variously written in the wild. Same rule: normalize before
//!   consulting.
//! - **Alef maqsura vs. yeh.** Terminal `ى` (alef maqsura) and terminal
//!   `ي` (yeh) are conflated across a large fraction of Arabic text.
//!   Same rule: normalize before consulting.
//!
//! [`crate::normalize::normalize`] applies all three folds in one pass;
//! [`crate::normalize::ArabicNormalizer`]'s builder-configurable
//! variant lets callers opt in to teh-marbuta folding as well.
//!
//! # Function words that wear two hats — a note on set semantics
//!
//! Several Arabic function words serve multiple grammatical roles.
//! `لا` is both a coordinating conjunction ("neither…nor…") *and* a
//! negation particle ("no"); `ما` is both an interrogative ("what") and
//! a negation particle ("not"); `لكن` is both a coordinating
//! conjunction ("but") and, homographically, the second-person feminine
//! plural clitic `لَـ + كُنَّ` ("to you-fem-pl"). The stopword list is a
//! **set**, so each such word is listed exactly once; the section
//! headings below name the role that seemed most representative.
//!
//! # Coverage rationale
//!
//! - **Prepositions and case markers** (`في`, `من`, `إلى`, `على`, `عن`,
//!   `مع`, `عند`, `لدى`).
//! - **Coordinating and subordinating conjunctions** (`و`, `أو`, `ثم`,
//!   `لكن`, `لأن`, `أن`, `إن`, `إذا`, `حتى`, `كي`).
//! - **Demonstrative pronouns** (`هذا`, `هذه`, `هؤلاء`, `ذلك`, `تلك`,
//!   `أولئك`, `هنا`, `هناك`).
//! - **Personal pronouns** (`أنا`, `نحن`, `أنت`, `أنتِ`, `أنتم`, `هو`,
//!   `هي`, `هم`, `هن`).
//! - **Interrogatives** (`ما`, `ماذا`, `من`, `متى`, `أين`, `كيف`,
//!   `كم`, `لماذا`, `هل`, `أ`).
//! - **Negation and modality** (`لا`, `لم`, `لن`, `ليس`, `ما`).
//! - **Common "to be" / "to have" surface forms** (`كان`, `كانت`,
//!   `يكون`, `تكون`, `أكون`, `ليس`, `عند`, `لدى`, `له`, `لها`).
//!
//! # Non-goals
//!
//! - **Dialect stopwords.** Egyptian (`مش`, `دلوقتي`, `يعني`), Levantine
//!   (`شو`, `هلق`, `منيح`), and Gulf (`شلون`, `وين`, `متى`) forms are
//!   absent — the list targets Modern Standard Arabic (MSA) only.
//!   Dialect coverage is a follow-up.
//! - **Farsi / Urdu.** These languages use the Arabic script but have
//!   distinct grammars and function-word inventories. They belong in
//!   their own `stringcheese-fa` / `stringcheese-ur` packs.
//! - **Domain-specific stopwords.** IR practice for legal, medical, or
//!   scientific corpora typically extends the general list. Downstream
//!   applications should carry their own.
//! - **Fully-vocalized entries.** The list stores the unvoweled surface
//!   form only; input with harakat should be passed through
//!   [`crate::normalize::normalize`] before the stopword check.

/// The Arabic stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor hands
/// back exactly this slice.
///
/// Entries are stored in *logical* (byte) order; RTL rendering is a
/// display-layer concern.
pub const STOPWORDS: &[&str] = &[
    // -----------------------------------------------------------------
    // Prepositions (حروف الجر) — the core of the case-marking system.
    // -----------------------------------------------------------------
    "في",
    "من",
    "إلى",
    "على",
    "عن",
    "مع",
    "عند",
    "لدى",
    "بين",
    "حول",
    "خلال",
    "بعد",
    "قبل",
    "تحت",
    "فوق",
    "أمام",
    "خلف",
    "وراء",
    "داخل",
    "خارج",
    // -----------------------------------------------------------------
    // Coordinating conjunctions (حروف العطف). `لا` (negation +
    // "neither…nor…") is deferred to the negation section below.
    // -----------------------------------------------------------------
    "و",
    "أو",
    "ثم",
    "ف",
    "أم",
    "بل",
    "لكن",
    "حتى",
    // -----------------------------------------------------------------
    // Subordinating conjunctions and complementizers.
    // -----------------------------------------------------------------
    "أن",
    "إن",
    "أنه",
    "أنها",
    "إذ",
    "إذا",
    "لو",
    "لأن",
    "لكي",
    "كي",
    "بينما",
    "حيث",
    "عندما",
    "طالما",
    "منذ",
    // -----------------------------------------------------------------
    // Demonstrative pronouns (أسماء الإشارة) — the ha- and dha- series.
    // -----------------------------------------------------------------
    "هذا",
    "هذه",
    "هذان",
    "هاتان",
    "هؤلاء",
    "ذلك",
    "تلك",
    "ذاك",
    "أولئك",
    "هنا",
    "هناك",
    "هنالك",
    // -----------------------------------------------------------------
    // Personal pronouns (الضمائر المنفصلة).
    // -----------------------------------------------------------------
    "أنا",
    "نحن",
    "أنت",
    "أنتِ",
    "أنتما",
    "أنتم",
    "أنتن",
    "هو",
    "هي",
    "هما",
    "هم",
    "هن",
    // -----------------------------------------------------------------
    // Interrogatives (أدوات الاستفهام). `ما` (interrogative +
    // negation particle) is deferred to the negation section below.
    // -----------------------------------------------------------------
    "ماذا",
    "متى",
    "أين",
    "كيف",
    "كم",
    "لماذا",
    "هل",
    "أي",
    "أية",
    "أيها",
    "أيتها",
    // -----------------------------------------------------------------
    // Negation and modality particles. `ما` and `لا` land here because
    // negation is their most frequent role in IR-relevant text.
    // -----------------------------------------------------------------
    "لا",
    "ما",
    "لم",
    "لن",
    "لست",
    "ليس",
    "ليست",
    "ليسا",
    "ليسوا",
    // -----------------------------------------------------------------
    // Common surface forms of كان (to be).
    // -----------------------------------------------------------------
    "كان",
    "كانت",
    "كانوا",
    "كن",
    "يكون",
    "تكون",
    "أكون",
    "نكون",
    "يكونون",
    "كائن",
    // -----------------------------------------------------------------
    // "To have" — Arabic has no lexical verb for *have*; possession is
    // expressed with a preposition (`عند`, `لدى`, `ل-`) plus an
    // attached-pronoun clitic. The most common attached forms are
    // listed here. `لكن` (which is also the fem-pl "to you-all"
    // clitic) is elided — already listed as a conjunction above.
    // -----------------------------------------------------------------
    "له",
    "لها",
    "لهم",
    "لهن",
    "لك",
    "لكم",
    "لي",
    "لنا",
    "عنده",
    "عندها",
    "عندهم",
    "لديه",
    "لديها",
    "لديهم",
    // -----------------------------------------------------------------
    // Common adverbs and quantifiers frequently filtered as noise.
    // -----------------------------------------------------------------
    "كل",
    "بعض",
    "غير",
    "سوى",
    "أيضا",
    "كذلك",
    "أيضًا",
    "جدا",
    "جداً",
    "فقط",
    "قد",
    "لقد",
    "سوف",
    "س",
    "قط",
    "أبدا",
    "الآن",
    "اليوم",
    "أمس",
    "غدا",
    "دائما",
    "أحيانا",
    "ربما",
    "لعل",
    "كأن",
    // -----------------------------------------------------------------
    // Relative particles (الأسماء الموصولة).
    // -----------------------------------------------------------------
    "الذي",
    "التي",
    "الذين",
    "اللاتي",
    "اللواتي",
    "اللذان",
    "اللتان",
    // -----------------------------------------------------------------
    // "Yes" / "no" / discourse markers.
    // -----------------------------------------------------------------
    "نعم",
    "بلى",
    "كلا",
    "إذن",
    "إذًا",
    "طبعا",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "~150" — assert we're in that
        // ballpark. The list has grown slightly during hand-curation.
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
    fn stopwords_contain_core_prepositions() {
        // Any pack author reordering the list should trip this test if
        // they lose one accidentally.
        for &w in &["في", "من", "إلى", "على", "عن", "مع"] {
            assert!(STOPWORDS.contains(&w), "core preposition {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_personal_pronouns() {
        for &w in &["أنا", "نحن", "هو", "هي", "هم"] {
            assert!(STOPWORDS.contains(&w), "personal pronoun {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_demonstratives() {
        for &w in &["هذا", "هذه", "ذلك", "تلك"] {
            assert!(STOPWORDS.contains(&w), "demonstrative {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_core_conjunctions() {
        for &w in &["و", "أو", "ثم", "لكن"] {
            assert!(STOPWORDS.contains(&w), "conjunction {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_common_to_be_forms() {
        for &w in &["كان", "كانت", "يكون", "ليس"] {
            assert!(STOPWORDS.contains(&w), "to-be form {w:?} missing");
        }
    }

    #[test]
    fn no_duplicates() {
        // O(n^2) is fine for a static list of ~150.
        for (i, &w) in STOPWORDS.iter().enumerate() {
            for &v in &STOPWORDS[i + 1..] {
                assert_ne!(w, v, "duplicate stopword: {w:?}");
            }
        }
    }
}
