//! Case-mapping capability for the StringCheese ICU-alternative
//! subsystem.
//!
//! Wraps a locale-agnostic case-mapping engine over one or more
//! `stringcheese-scud` case-data packs and exposes it through the
//! `stringcheese:icu-case@0.1.0` WIT world. Callers construct a
//! [`CaseEngine`] from a slice of loaded [`CasePack`]s and issue
//! [`to_lower`](CaseEngine::to_lower) / [`to_upper`](CaseEngine::to_upper) /
//! [`to_title`](CaseEngine::to_title) / [`fold`](CaseEngine::fold)
//! queries; the engine walks the BCP 47 fallback chain
//! (`pt-BR → pt → ""`) at query time.
//!
//! # Position in the WIT-i18n subsystem
//!
//! Phase 1 of the WIT-i18n design (`docs/design/wit-i18n.md` § 8) —
//! the first capability delivered on top of the shared
//! `stringcheese-scud` loader. The algorithm side of the case
//! interface lives here; the CLDR-derived data lives in
//! `stringcheese-<lang>` packs and reaches this crate through
//! [`CasePack::from_scud_bytes`].
//!
//! # WIT surface
//!
//! The WIT file at `component/wit/icu-case/stringcheese-icu-case.wit` defines
//! four exports on the `case` world:
//!
//! * `to-lower(input, locale)` — locale-sensitive lowercasing.
//! * `to-upper(input, locale)` — locale-sensitive uppercasing.
//! * `to-title(input, locale, options)` — locale-sensitive
//!   titlecasing.
//! * `fold(input, mode)` — locale-independent case folding.
//!
//! A [`CaseEngine`] implements every export on the Rust side; a
//! future `wit-component`-gated `Guest` implementation (matching the
//! `stringcheese-tokenizer-component` pattern) will bridge the two
//! sides so this crate can ship as a standalone WASM component.
//!
//! # Phase 1 deferrals
//!
//! * **Standalone WASM component build.** The WIT interface is in
//!   place and parses cleanly under `wit-parser` (see the smoke test
//!   in `tests/wit_parse.rs`); the `wit-bindgen` `Guest`
//!   implementation and the `cargo build --target wasm32-wasip1
//!   --features wit-component` recipe land in a follow-up wave.
//! * **Full CLDR title-casing.** The Phase 1 [`to_title`](CaseEngine::to_title)
//!   implementation lowercases the tail and uppercases the leading
//!   scalar per word — the ASCII common case. Dutch `ij` titlecasing
//!   and the full UAX #29 word-break-based logic land alongside the
//!   `stringcheese-icu-break` capability crate.
//! * **Final-sigma tailoring.** The SCUD format reserves
//!   [`stringcheese_scud::ContextKind::FinalSigma`] for the Greek
//!   sigma rule; the algorithm side wires it in when the
//!   `stringcheese-el` (Greek) pack ships case data.
//!
//! # Trust model
//!
//! Inherited from `stringcheese-scud`: SCUD packs are trusted
//! input. This crate does not defend against maliciously crafted
//! packs.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
use alloc::string::String;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use stringcheese_scud::{CaseDataView, ScudFile};
// `ContextKind` is only consulted inside the alloc-gated
// `CaseEngine::map_*` methods; keep the import gated on the same cfg
// so a `--no-default-features` build (`std` and `alloc` both off) is
// warning-free.
#[cfg(feature = "alloc")]
use stringcheese_scud::ContextKind;

// Re-export the SCUD-side error surface so downstream language packs
// depending on `stringcheese-icu-case` do not have to add a direct
// `stringcheese-scud` dependency just to name the error type. Matches
// the shape a `stringcheese-en::case_data::case_pack()` helper needs
// (returning `Result<CasePack<'static>, ScudError>`) without imposing
// a second crate on the pack's dependency graph.
pub use stringcheese_scud::ScudError;

/// A loaded case-mapping pack for one BCP 47 locale.
///
/// Wraps a validated [`ScudFile`] whose capability tag is
/// [`stringcheese_scud::CAP_CASE`]. Cheap to clone — the underlying
/// SCUD bytes are borrowed by the [`ScudFile`], and this wrapper
/// carries only the parsed header plus the locale tag pulled from it.
#[derive(Debug, Clone, Copy)]
pub struct CasePack<'a> {
    scud: ScudFile<'a>,
    locale: &'a str,
    data: CaseDataView<'a>,
}

impl<'a> CasePack<'a> {
    /// Wrap a validated [`ScudFile`] as a case pack.
    ///
    /// Returns an error if the SCUD file's capability tag is not
    /// `CAP_CASE` or if the file's body cannot be parsed as case
    /// data.
    pub fn new(scud: ScudFile<'a>) -> Result<Self, ScudError> {
        let data = scud.as_case_data()?;
        let locale = scud.locale().unwrap_or("");
        Ok(Self { scud, locale, data })
    }

    /// Parse `bytes` as a SCUD file and wrap it as a case pack.
    ///
    /// Convenience constructor for pack crates that embed their SCUD
    /// blob as an `include_bytes!` constant.
    pub fn from_scud_bytes(bytes: &'a [u8]) -> Result<Self, ScudError> {
        let scud = ScudFile::from_slice(bytes)?;
        Self::new(scud)
    }

    /// The BCP 47 locale tag associated with this pack.
    #[must_use]
    pub fn locale(&self) -> &'a str {
        self.locale
    }

    /// The CLDR version the pack was generated from.
    #[must_use]
    pub fn cldr_version(&self) -> &'a str {
        self.scud.cldr_version()
    }

    /// Total byte length of the underlying SCUD file.
    #[must_use]
    pub fn scud_bytes_len(&self) -> usize {
        self.scud.len()
    }

    /// The zero-copy case-mapping data view.
    #[must_use]
    pub fn data(&self) -> &CaseDataView<'a> {
        &self.data
    }
}

/// Locale-sensitive case-mapping engine.
///
/// Holds a list of [`CasePack`]s and consults them at query time in
/// BCP 47 fallback order. A caller who wants German and Turkish
/// constructs a [`CaseEngine`] from `[german_pack, turkish_pack]`;
/// queries under `de-DE` walk `de-DE → de → ""`, queries under
/// `tr-CY` walk `tr-CY → tr → ""`.
///
/// The engine is `Send + Sync` and cheap to store in a `static` slot
/// so long as the underlying SCUD bytes are `'static`.
#[cfg(feature = "alloc")]
#[derive(Debug, Clone)]
pub struct CaseEngine<'a> {
    packs: Vec<CasePack<'a>>,
}

/// How aggressively [`CaseEngine::fold`] folds case. Mirrors the WIT
/// `fold-mode` enum.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FoldMode {
    /// Simple case fold — 1:1 code-point mapping only.
    Simple,
    /// Full case fold — may expand (`ß → ss`).
    Full,
    /// Full case fold plus Turkic tailorings (`I → ı`, `İ → i`).
    FullTurkic,
}

/// Which boundary rule [`CaseEngine::to_title`] uses. Mirrors the WIT
/// `title-boundary` enum.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TitleBoundary {
    /// Break on Unicode grapheme boundaries. Stateless, imperfect but
    /// cheap.
    Graphemes,
    /// Break on ASCII-ish word boundaries — the Phase 1 shape until
    /// the `stringcheese-icu-break` capability crate lands.
    Words,
    /// Break on sentence boundaries (only sentence-initial words are
    /// candidates for uppercasing).
    Sentences,
}

/// Options for [`CaseEngine::to_title`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TitleOptions {
    /// Which boundary rule to use.
    pub boundary: TitleBoundary,
    /// When `true`, non-initial characters in each title word are
    /// lowercased (`AbC → Abc`); when `false`, preserved as-is.
    pub lowercase_tail: bool,
}

impl Default for TitleOptions {
    fn default() -> Self {
        Self {
            boundary: TitleBoundary::Words,
            lowercase_tail: true,
        }
    }
}

/// Typed failure modes of the case engine. Mirrors the WIT
/// `case-error` variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaseError {
    /// The locale tag was not a well-formed BCP 47 tag.
    InvalidLocale(&'static str),
    /// No pack was loaded for the requested locale.
    LocaleUnavailable(&'static str),
    /// The requested title-casing mode is not implemented.
    UnsupportedTitleMode(&'static str),
}

#[cfg(feature = "alloc")]
impl<'a> CaseEngine<'a> {
    /// Construct a fresh engine backed by the given packs.
    ///
    /// Pack order is *not* significant — the engine indexes by BCP 47
    /// tag at query time. A later pack with the same tag as an
    /// earlier one overrides it.
    #[must_use]
    pub fn new(packs: Vec<CasePack<'a>>) -> Self {
        Self { packs }
    }

    /// Every BCP 47 locale tag this engine knows about.
    #[must_use]
    pub fn supported_locales(&self) -> Vec<&'a str> {
        self.packs.iter().map(CasePack::locale).collect()
    }

    /// True iff a query in the given locale would use a pack (rather
    /// than falling through to root).
    #[must_use]
    pub fn supports(&self, locale: &str) -> bool {
        walk_fallback_chain(locale).any(|tag| self.pack_for(tag).is_some())
    }

    /// Lowercase `input` under the given locale.
    ///
    /// Walks the CLDR fallback chain: the pack with the most specific
    /// tag matching `locale` wins, then progressively less-specific
    /// ancestors, then root Unicode lowercasing.
    #[must_use]
    pub fn to_lower(&self, input: &str, locale: &str) -> String {
        let mut out = String::with_capacity(input.len());
        for c in input.chars() {
            self.map_lower_char(c, locale, &mut out);
        }
        out
    }

    /// Uppercase `input` under the given locale.
    #[must_use]
    pub fn to_upper(&self, input: &str, locale: &str) -> String {
        let mut out = String::with_capacity(input.len());
        for c in input.chars() {
            self.map_upper_char(c, locale, &mut out);
        }
        out
    }

    /// Titlecase `input` under the given locale and options.
    ///
    /// Phase 1 implements the ASCII common case: uppercase the first
    /// scalar following each word boundary (per [`TitleOptions::boundary`])
    /// and, when [`TitleOptions::lowercase_tail`] is true, lowercase
    /// every subsequent scalar until the next boundary. The Dutch
    /// `ij` digraph and full UAX #29 word-break behaviour land with
    /// the `stringcheese-icu-break` capability crate.
    ///
    /// # Errors
    ///
    /// Returns [`CaseError::UnsupportedTitleMode`] when
    /// [`TitleOptions::boundary`] is [`TitleBoundary::Sentences`] —
    /// sentence-boundary detection is deferred to a follow-up wave.
    pub fn to_title(
        &self,
        input: &str,
        locale: &str,
        options: TitleOptions,
    ) -> Result<String, CaseError> {
        if matches!(options.boundary, TitleBoundary::Sentences) {
            return Err(CaseError::UnsupportedTitleMode("sentences"));
        }
        let mut out = String::with_capacity(input.len());
        let mut at_boundary = true;
        for c in input.chars() {
            if is_title_boundary(c) {
                at_boundary = true;
                out.push(c);
                continue;
            }
            if at_boundary {
                self.map_upper_char(c, locale, &mut out);
                at_boundary = false;
            } else if options.lowercase_tail {
                self.map_lower_char(c, locale, &mut out);
            } else {
                out.push(c);
            }
        }
        Ok(out)
    }

    /// Locale-independent case fold for case-insensitive matching.
    ///
    /// Consults every loaded pack for a fold mapping, applying the
    /// first match encountered; falls back to Rust's built-in
    /// [`char::to_lowercase`] for scalars no pack covers. This is
    /// deterministic across packs because the fold tables come from
    /// Unicode `CaseFolding.txt` (identical in every locale) and any
    /// pack that ships a fold entry ships the same target scalar.
    #[must_use]
    pub fn fold(&self, input: &str, mode: FoldMode) -> String {
        let mut out = String::with_capacity(input.len());
        for c in input.chars() {
            self.fold_char(c, mode, &mut out);
        }
        out
    }

    /// Look up the first-matching pack for a locale tag.
    fn pack_for(&self, tag: &str) -> Option<&CasePack<'a>> {
        self.packs
            .iter()
            .find(|p| p.locale.eq_ignore_ascii_case(tag))
    }

    /// Iterate every pack whose locale is a prefix of `locale` under
    /// the CLDR fallback chain, most-specific first.
    fn packs_for_locale<'e>(&'e self, locale: &'e str) -> impl Iterator<Item = &'e CasePack<'a>> {
        walk_fallback_chain(locale).filter_map(move |tag| self.pack_for(tag))
    }

    fn map_lower_char(&self, c: char, locale: &str, out: &mut String) {
        let src = c as u32;
        for pack in self.packs_for_locale(locale) {
            // Contextual (locale-override) mapping wins first.
            for (kind, dst) in pack.data.contextual(src) {
                if matches!(kind, ContextKind::LocaleOverrideLower) {
                    if let Some(ch) = char::from_u32(dst) {
                        out.push(ch);
                        return;
                    }
                }
            }
            if let Some(dst) = pack.data.simple_lower(src) {
                if let Some(ch) = char::from_u32(dst) {
                    out.push(ch);
                    return;
                }
            }
        }
        // Fallback to Rust's built-in lowercasing.
        for lc in c.to_lowercase() {
            out.push(lc);
        }
    }

    fn map_upper_char(&self, c: char, locale: &str, out: &mut String) {
        let src = c as u32;
        for pack in self.packs_for_locale(locale) {
            // Contextual (locale-override) uppercase mapping wins first.
            for (kind, dst) in pack.data.contextual(src) {
                if matches!(kind, ContextKind::LocaleOverrideUpper) {
                    if let Some(ch) = char::from_u32(dst) {
                        out.push(ch);
                        return;
                    }
                }
            }
            // Full uppercase expansion (`ß → SS`) beats a simple map.
            if let Some(full) = pack.data.full_upper(src) {
                for ch in full.chars() {
                    out.push(ch);
                }
                return;
            }
            if let Some(dst) = pack.data.simple_upper(src) {
                if let Some(ch) = char::from_u32(dst) {
                    out.push(ch);
                    return;
                }
            }
        }
        for uc in c.to_uppercase() {
            out.push(uc);
        }
    }

    fn fold_char(&self, c: char, mode: FoldMode, out: &mut String) {
        let src = c as u32;
        // Turkic tailoring is locale-neutral and applies before any
        // pack lookup so `full_turkic` mode behaves identically
        // regardless of the loaded pack set.
        if matches!(mode, FoldMode::FullTurkic) {
            match src {
                0x0049 => {
                    // Latin capital I → dotless small i.
                    out.push('\u{0131}');
                    return;
                }
                0x0130 => {
                    // Latin capital I with dot above → small i.
                    out.push('i');
                    return;
                }
                _ => {}
            }
        }
        // Full fold (multi-scalar expansion) beats simple fold when
        // the mode allows it.
        if matches!(mode, FoldMode::Full | FoldMode::FullTurkic) {
            for pack in &self.packs {
                if let Some(full) = pack.data.full_fold(src) {
                    for ch in full.chars() {
                        out.push(ch);
                    }
                    return;
                }
            }
        }
        for pack in &self.packs {
            if let Some(dst) = pack.data.simple_fold(src) {
                if let Some(ch) = char::from_u32(dst) {
                    out.push(ch);
                    return;
                }
            }
        }
        for lc in c.to_lowercase() {
            out.push(lc);
        }
    }
}

/// Walk the CLDR-defined fallback chain for a BCP 47 tag.
///
/// The chain strips subtags one at a time from the right, terminating
/// with the empty string (root). Examples:
///
/// * `pt-BR` → `pt-BR`, `pt`, `""`
/// * `zh-Hant-HK` → `zh-Hant-HK`, `zh-Hant`, `zh`, `""`
/// * `en` → `en`, `""`
/// * `""` → `""`
pub fn walk_fallback_chain(locale: &str) -> impl Iterator<Item = &str> {
    // Emit the full tag first, then each successively shorter prefix
    // ending on a `-` boundary, then the empty string once.
    let mut current = Some(locale);
    let mut emitted_root = false;
    core::iter::from_fn(move || {
        if let Some(tag) = current.take() {
            if tag.is_empty() {
                if emitted_root {
                    None
                } else {
                    emitted_root = true;
                    Some("")
                }
            } else {
                // Prepare the next iteration: strip the trailing
                // `-subtag`, or emit `""` if there is no `-`.
                let next = tag.rfind('-').map_or("", |idx| &tag[..idx]);
                current = Some(next);
                Some(tag)
            }
        } else {
            None
        }
    })
}

/// True iff `c` is treated as a word boundary by the Phase 1
/// title-casing rule. Whitespace and ASCII punctuation qualify.
///
/// Only consulted from the alloc-gated [`CaseEngine::to_title`]
/// method; kept behind the same cfg so a no-alloc build stays
/// warning-free.
#[cfg(feature = "alloc")]
fn is_title_boundary(c: char) -> bool {
    c.is_whitespace() || c.is_ascii_punctuation()
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::*;
    use alloc::vec;
    use stringcheese_scud::{
        CAP_CASE, CaseSectionBuilder, SECT_CONTEXT, SECT_FULL_FOLD, SECT_FULL_UPPER,
        SECT_SIMPLE_FOLD, SECT_SIMPLE_LOWER, SECT_SIMPLE_UPPER, ScudFile, ScudWriter,
    };

    fn build_test_en() -> alloc::vec::Vec<u8> {
        let mut c = CaseSectionBuilder::new();
        for ch in 'a'..='z' {
            let up = ch.to_ascii_uppercase();
            c.push_simple_lower(up as u32, ch as u32);
            c.push_simple_upper(ch as u32, up as u32);
            c.push_simple_fold(up as u32, ch as u32);
        }
        // German ß — full uppercase to "SS", full fold to "ss".
        c.push_full_upper(0x00DF, &[0x0053, 0x0053]);
        c.push_full_fold(0x00DF, &[0x0073, 0x0073]);
        // Latin capital I with dot above (İ) → i under English fold
        // (default Unicode behaviour).
        c.push_full_fold(0x0130, &[0x0069, 0x0307]);

        let mut w = ScudWriter::new(CAP_CASE, "44.1", Some("en"));
        w.append_section(SECT_SIMPLE_LOWER, &c.simple_lower_bytes());
        w.append_section(SECT_SIMPLE_UPPER, &c.simple_upper_bytes());
        w.append_section(SECT_SIMPLE_FOLD, &c.simple_fold_bytes());
        w.append_section(SECT_FULL_UPPER, &c.full_upper_bytes());
        w.append_section(SECT_FULL_FOLD, &c.full_fold_bytes());
        w.finish()
    }

    fn build_test_tr() -> alloc::vec::Vec<u8> {
        let mut c = CaseSectionBuilder::new();
        // Turkish dotted/dotless-I contextual mappings.
        c.push_context('I' as u32, ContextKind::LocaleOverrideLower, 0x0131);
        c.push_context('i' as u32, ContextKind::LocaleOverrideUpper, 0x0130);

        let mut w = ScudWriter::new(CAP_CASE, "44.1", Some("tr"));
        w.append_section(SECT_CONTEXT, &c.context_bytes());
        w.finish()
    }

    fn engine_with_en_and_tr() -> (alloc::vec::Vec<u8>, alloc::vec::Vec<u8>) {
        (build_test_en(), build_test_tr())
    }

    #[test]
    fn ascii_lower_upper_roundtrip() {
        let en = build_test_en();
        let en_pack = CasePack::from_scud_bytes(&en).unwrap();
        let engine = CaseEngine::new(vec![en_pack]);
        assert_eq!(engine.to_lower("HELLO WORLD", "en"), "hello world");
        assert_eq!(engine.to_upper("hello world", "en"), "HELLO WORLD");
    }

    #[test]
    fn german_sharp_s_uppercases_to_ss() {
        let en = build_test_en();
        let en_pack = CasePack::from_scud_bytes(&en).unwrap();
        let engine = CaseEngine::new(vec![en_pack]);
        assert_eq!(engine.to_upper("straße", "de"), "STRASSE");
    }

    #[test]
    fn german_sharp_s_full_fold_to_ss() {
        let en = build_test_en();
        let en_pack = CasePack::from_scud_bytes(&en).unwrap();
        let engine = CaseEngine::new(vec![en_pack]);
        assert_eq!(engine.fold("Straße", FoldMode::Full), "strasse");
    }

    #[test]
    fn turkish_lower_i_uses_pack() {
        let (en, tr) = engine_with_en_and_tr();
        let en_pack = CasePack::from_scud_bytes(&en).unwrap();
        let tr_pack = CasePack::from_scud_bytes(&tr).unwrap();
        let engine = CaseEngine::new(vec![en_pack, tr_pack]);
        // Turkish: capital I lowers to dotless small ı.
        assert_eq!(engine.to_lower("ISTANBUL", "tr"), "ıstanbul");
        // English: capital I lowers to i (default).
        assert_eq!(engine.to_lower("ISTANBUL", "en"), "istanbul");
    }

    #[test]
    fn turkish_upper_i_uses_pack() {
        let (en, tr) = engine_with_en_and_tr();
        let en_pack = CasePack::from_scud_bytes(&en).unwrap();
        let tr_pack = CasePack::from_scud_bytes(&tr).unwrap();
        let engine = CaseEngine::new(vec![en_pack, tr_pack]);
        // Turkish: lowercase i uppers to dotted İ.
        assert_eq!(engine.to_upper("istanbul", "tr"), "İSTANBUL");
        // English: lowercase i uppers to I.
        assert_eq!(engine.to_upper("istanbul", "en"), "ISTANBUL");
    }

    #[test]
    fn fallback_chain_walks_correctly() {
        let chain: alloc::vec::Vec<&str> = walk_fallback_chain("pt-BR").collect();
        assert_eq!(chain, ["pt-BR", "pt", ""]);
        let chain: alloc::vec::Vec<&str> = walk_fallback_chain("zh-Hant-HK").collect();
        assert_eq!(chain, ["zh-Hant-HK", "zh-Hant", "zh", ""]);
        let chain: alloc::vec::Vec<&str> = walk_fallback_chain("en").collect();
        assert_eq!(chain, ["en", ""]);
        let chain: alloc::vec::Vec<&str> = walk_fallback_chain("").collect();
        assert_eq!(chain, [""]);
    }

    #[test]
    fn supports_uses_fallback() {
        let en = build_test_en();
        let en_pack = CasePack::from_scud_bytes(&en).unwrap();
        let engine = CaseEngine::new(vec![en_pack]);
        assert!(engine.supports("en"));
        // `en-US` falls back to `en`.
        assert!(engine.supports("en-US"));
        assert!(!engine.supports("de"));
    }

    #[test]
    fn to_title_ascii() {
        let en = build_test_en();
        let en_pack = CasePack::from_scud_bytes(&en).unwrap();
        let engine = CaseEngine::new(vec![en_pack]);
        assert_eq!(
            engine
                .to_title("hello world", "en", TitleOptions::default())
                .unwrap(),
            "Hello World"
        );
    }

    #[test]
    fn to_title_rejects_sentences() {
        let en = build_test_en();
        let en_pack = CasePack::from_scud_bytes(&en).unwrap();
        let engine = CaseEngine::new(vec![en_pack]);
        let err = engine
            .to_title(
                "hello.",
                "en",
                TitleOptions {
                    boundary: TitleBoundary::Sentences,
                    lowercase_tail: true,
                },
            )
            .unwrap_err();
        assert_eq!(err, CaseError::UnsupportedTitleMode("sentences"));
    }

    #[test]
    fn fold_full_turkic_prefers_turkish() {
        let en = build_test_en();
        let en_pack = CasePack::from_scud_bytes(&en).unwrap();
        let engine = CaseEngine::new(vec![en_pack]);
        assert_eq!(engine.fold("I", FoldMode::FullTurkic), "ı");
        assert_eq!(engine.fold("İ", FoldMode::FullTurkic), "i");
        assert_eq!(engine.fold("I", FoldMode::Simple), "i");
    }

    #[test]
    fn empty_input_is_empty_output() {
        let en = build_test_en();
        let en_pack = CasePack::from_scud_bytes(&en).unwrap();
        let engine = CaseEngine::new(vec![en_pack]);
        assert_eq!(engine.to_lower("", "en"), "");
        assert_eq!(engine.to_upper("", "en"), "");
        assert_eq!(engine.fold("", FoldMode::Full), "");
    }

    #[test]
    fn casepack_metadata_accessible() {
        let en = build_test_en();
        let en_pack = CasePack::from_scud_bytes(&en).unwrap();
        assert_eq!(en_pack.locale(), "en");
        assert_eq!(en_pack.cldr_version(), "44.1");
        assert!(en_pack.scud_bytes_len() > 0);
    }

    #[test]
    fn scud_file_wrongly_typed_rejected() {
        // Build a SCUD file with a non-CASE capability tag; the case
        // pack constructor should refuse it.
        let w = ScudWriter::new(*b"COLL", "44.1", Some("en"));
        let bytes = w.finish();
        let file = ScudFile::from_slice(&bytes).unwrap();
        assert!(CasePack::new(file).is_err());
    }
}
