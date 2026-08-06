//! The [`Language`] trait and the [`LanguageProvider`] discovery trait.
//!
//! Every `stringcheese-<lang>` pack implements [`Language`] on a
//! zero-sized value (a unit struct) and exposes a `pub const` instance
//! of it (`ENGLISH`, `GERMAN`, `JAPANESE`, …) so callers can grab the
//! pack without any construction ceremony.
//!
//! [`LanguageProvider`] is the runtime-discovery half of the story —
//! callers that build a `HashMap<&str, &'static dyn Language>` (or the
//! equivalent for `no_std`) can implement this trait to make their
//! collection queryable by BCP-47 code.

use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::{Collator, LanguagePhoneticEncoder, SimpleTokenizer};

/// The core language-pack contract.
///
/// A [`Language`] is a data-driven description of a language's rules:
/// its stopword list, its stemmer, its tokenizer, and (optionally) its
/// phonetic encoder and locale collator. The trait is object-safe;
/// callers routinely pass `&dyn Language` around.
///
/// Implementations should be **zero-sized value types** where possible
/// — the shipped `stringcheese-<lang>` packs all follow this pattern
/// so their `ENGLISH` / `GERMAN` / … `pub const` instances can live in
/// static storage without any runtime setup.
///
/// # Default methods
///
/// The trait ships defaults for [`is_stopword`](Self::is_stopword) and
/// [`tokenize`](Self::tokenize) so a bare-bones pack only has to
/// implement [`code`](Self::code), [`name`](Self::name),
/// [`stopwords`](Self::stopwords), and [`stem`](Self::stem). Packs
/// that need faster stopword lookup or a bespoke tokenizer override the
/// default.
///
/// # Example
///
/// ```ignore
/// use stringcheese_en::ENGLISH;
/// use stringcheese_lang::Language;
///
/// assert_eq!(ENGLISH.code(), "en");
/// assert!(ENGLISH.is_stopword("the"));
/// assert_eq!(ENGLISH.stem("running"), "run");
/// ```
pub trait Language: Send + Sync {
    /// BCP-47 language subtag identifying the language.
    ///
    /// Examples: `"en"` (English), `"fr"` (French), `"ja"` (Japanese),
    /// `"zh-Hans"` (Simplified Chinese). Follow BCP-47's rules — the
    /// [`LanguageProvider`]'s lookup is case-sensitive.
    fn code(&self) -> &'static str;

    /// Human-readable name for the language (in English), e.g.
    /// `"English"`, `"French"`, `"Japanese"`.
    fn name(&self) -> &'static str;

    /// The stopword list for the language.
    ///
    /// May be empty for languages without a settled stopword tradition
    /// (many East and Southeast Asian languages fall into this bucket).
    /// Callers should treat an empty list as "the pack author declined
    /// to ship one", not as "nothing is a stopword" — a downstream
    /// system that needs stopword filtering should carry its own list
    /// or reach for a domain-specific one.
    fn stopwords(&self) -> &'static [&'static str];

    /// Returns `true` if `word` is a stopword under this language.
    ///
    /// The default implementation performs an ASCII-case-insensitive
    /// linear scan of [`stopwords`](Self::stopwords). Language packs
    /// whose stopwords live outside ASCII (Turkish, German ß, …) or
    /// that need faster lookup should override this method.
    fn is_stopword(&self, word: &str) -> bool {
        self.stopwords()
            .iter()
            .any(|s| s.eq_ignore_ascii_case(word))
    }

    /// Returns the stem of `word` — an equivalence-class representative
    /// that collapses inflectional variants into one form.
    ///
    /// The return type is [`Cow`] so a pack whose stemmer chose to
    /// leave the word unchanged can borrow the input instead of
    /// allocating.
    ///
    /// Implementations should be deterministic and idempotent
    /// (`stem(stem(w)) == stem(w)`); see [`Stemmer`](crate::Stemmer)'s
    /// contract for the shared expectations.
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str>;

    /// Splits `text` into words.
    ///
    /// The default implementation returns
    /// [`SimpleTokenizer`]'s output boxed as a trait object.
    /// Language packs that need contraction handling, compound
    /// splitting, morpheme segmentation, or any other bespoke rule
    /// should override this method with their own tokenizer.
    fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
        Box::new(SimpleTokenizer::new().tokenize(text))
    }

    /// Language-specific phonetic encoder, if the pack ships one.
    ///
    /// Returns `None` when the language has no established phonetic
    /// encoding (or the pack author chose to omit it). The default
    /// implementation returns `None`; packs override this by
    /// constructing a [`LanguagePhoneticEncoder`] adapter (see
    /// [`crate::phonetic::SoundexAdapter`] for the English shape).
    fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
        None
    }

    /// Language-specific collator, if the pack ships one.
    ///
    /// Returns `None` when the pack accepts the default Unicode
    /// code-point ordering. Only override this if the language's
    /// canonical sort order actually diverges from code-point ordering
    /// (Swedish, German ß, Turkish i-family, etc.).
    fn collator(&self) -> Option<&dyn Collator> {
        None
    }
}

/// Runtime-discovery trait: look up a language by BCP-47 code.
///
/// Callers that manage a collection of language packs (a
/// `Vec<&'static dyn Language>`, a `HashMap`, or a hand-rolled table)
/// implement this trait to expose the collection through a single
/// query interface. The trait is deliberately unopinionated about
/// storage — this crate ships no built-in registry (see the
/// discussion in [`crate`]'s module docs).
///
/// # Example
///
/// ```
/// use stringcheese_lang::{Language, LanguageProvider};
///
/// struct StaticProvider(&'static [&'static dyn Language]);
///
/// impl LanguageProvider for StaticProvider {
///     fn language(&self, code: &str) -> Option<&dyn Language> {
///         self.0
///             .iter()
///             .copied()
///             .find(|l| l.code() == code)
///     }
///     fn supported_languages(&self) -> Vec<&'static str> {
///         self.0.iter().map(|l| l.code()).collect()
///     }
/// }
/// ```
pub trait LanguageProvider {
    /// Look up a language by its BCP-47 code. Returns `None` if the
    /// code is not known to this provider.
    ///
    /// Lookup is case-sensitive — BCP-47 codes are conventionally
    /// lowercase; providers that want to accept mixed case should
    /// normalize their input before delegating.
    fn language(&self, code: &str) -> Option<&dyn Language>;

    /// Returns the BCP-47 codes of every language this provider
    /// supports.
    fn supported_languages(&self) -> Vec<&'static str>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Stopwords;
    use alloc::string::String;

    // A minimal test Language used to exercise the trait's default
    // methods without depending on the English pack.
    struct TestLang;

    const TEST_STOPWORDS: &[&str] = &["the", "and", "of"];

    impl Language for TestLang {
        fn code(&self) -> &'static str {
            "xx"
        }
        fn name(&self) -> &'static str {
            "Test"
        }
        fn stopwords(&self) -> &'static [&'static str] {
            TEST_STOPWORDS
        }
        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            Cow::Borrowed(word) // identity stemmer for the test
        }
    }

    #[test]
    fn default_is_stopword_uses_case_insensitive_scan() {
        let l = TestLang;
        assert!(l.is_stopword("the"));
        assert!(l.is_stopword("THE"));
        assert!(l.is_stopword("The"));
        assert!(!l.is_stopword("cheese"));
    }

    #[test]
    fn default_tokenize_yields_simple_tokens() {
        let l = TestLang;
        let toks: Vec<&str> = l.tokenize("hello, world!").collect();
        assert_eq!(toks, ["hello", "world"]);
    }

    #[test]
    fn defaults_for_phonetic_and_collator_are_none() {
        let l = TestLang;
        assert!(l.phonetic_encoder().is_none());
        assert!(l.collator().is_none());
    }

    #[test]
    fn code_and_name_are_stable_static_strings() {
        let l = TestLang;
        assert_eq!(l.code(), "xx");
        assert_eq!(l.name(), "Test");
    }

    #[test]
    fn stem_identity_borrows_input() {
        let l = TestLang;
        let s = String::from("running");
        assert_eq!(l.stem(&s), "running");
    }

    #[test]
    fn stopwords_wrapper_and_trait_agree() {
        let l = TestLang;
        let sw = Stopwords::new(l.stopwords());
        for w in ["the", "and", "of"] {
            assert_eq!(l.is_stopword(w), sw.contains(w));
        }
    }

    // A tiny static-slice-backed LanguageProvider used to exercise the
    // trait shape.
    struct StaticProvider(&'static [&'static dyn Language]);

    impl LanguageProvider for StaticProvider {
        fn language(&self, code: &str) -> Option<&dyn Language> {
            self.0.iter().copied().find(|l| l.code() == code)
        }
        fn supported_languages(&self) -> Vec<&'static str> {
            self.0.iter().map(|l| l.code()).collect()
        }
    }

    static TEST_LANG: TestLang = TestLang;

    #[test]
    fn provider_finds_registered_language() {
        static LANGS: &[&dyn Language] = &[&TEST_LANG];
        let p = StaticProvider(LANGS);
        assert!(p.language("xx").is_some());
        assert!(p.language("zz").is_none());
        assert_eq!(p.supported_languages(), ["xx"]);
    }
}
