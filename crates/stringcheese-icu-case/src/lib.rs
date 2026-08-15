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
//! A [`CaseEngine`] implements every export on the Rust side. The
//! WIT `Guest` implementations that turn this crate into a
//! standalone `stringcheese:icu-case@0.1.0` WASM component live in
//! the sibling [`stringcheese-icu-case-component`] crate — the
//! pattern established by `stringcheese-tokenizer-component`. That
//! crate embeds the reference `case-en.scud` and `case-tr.scud`
//! packs so the componentised binary is drivable end-to-end without
//! a separate pack component.
//!
//! [`stringcheese-icu-case-component`]: https://docs.rs/stringcheese-icu-case-component
//!
//! # Phase 1 deferrals
//!
//! * **Standalone WASM component build.** Landed in the follow-up
//!   crate [`stringcheese-icu-case-component`], which wraps this
//!   crate's `CaseEngine` behind the WIT `case` world under a
//!   `wit-component` cargo feature and ships a `wasmtime` in-process
//!   smoke test. This crate remains algorithm-only; the WIT-facing
//!   `Guest` implementations live in the component wrapper.
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
    ///
    /// # ASCII fast path
    ///
    /// When `input.is_ascii()` and `locale` does not carry an ASCII
    /// case tailoring (i.e. is not Turkish or Latin-script
    /// Azerbaijani; see [`locale_has_ascii_tailoring`]), the pipeline
    /// reduces to [`str::to_ascii_lowercase`]. Root Unicode
    /// lowercasing on ASCII scalars is byte-identical to
    /// ASCII-lowercasing for every non-Turkic locale, and the
    /// bulk-`is_ascii` scan is SIMD-accelerated on aarch64 / `x86_64`.
    #[must_use]
    pub fn to_lower(&self, input: &str, locale: &str) -> String {
        if input.is_ascii() && !locale_has_ascii_tailoring(locale) {
            return input.to_ascii_lowercase();
        }
        let mut out = String::with_capacity(input.len());
        for c in input.chars() {
            self.map_lower_char(c, locale, &mut out);
        }
        out
    }

    /// Uppercase `input` under the given locale.
    ///
    /// # ASCII fast path
    ///
    /// Same shape as [`to_lower`](Self::to_lower): on ASCII input
    /// with a locale that has no ASCII tailoring
    /// ([`locale_has_ascii_tailoring`] returns `false`), the pipeline
    /// reduces to [`str::to_ascii_uppercase`].
    #[must_use]
    pub fn to_upper(&self, input: &str, locale: &str) -> String {
        if input.is_ascii() && !locale_has_ascii_tailoring(locale) {
            return input.to_ascii_uppercase();
        }
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
        // ASCII fast path — same rationale as `to_lower` / `to_upper`.
        // Word-boundary detection is `is_title_boundary`, which on
        // ASCII chars matches whitespace or ASCII punctuation; both
        // predicates are cheap byte-level checks. Uppercasing /
        // lowercasing each non-boundary byte reduces to the ASCII
        // primitives.
        if input.is_ascii() && !locale_has_ascii_tailoring(locale) {
            let mut out = String::with_capacity(input.len());
            let mut at_boundary = true;
            for &b in input.as_bytes() {
                let c = b as char;
                if is_title_boundary(c) {
                    at_boundary = true;
                    out.push(c);
                } else if at_boundary {
                    out.push(c.to_ascii_uppercase());
                    at_boundary = false;
                } else if options.lowercase_tail {
                    out.push(c.to_ascii_lowercase());
                } else {
                    out.push(c);
                }
            }
            return Ok(out);
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
        // ASCII fast path — case folding on ASCII scalars per
        // CaseFolding.txt is byte-identical to ASCII lowercasing for
        // every non-Turkic mode. `FoldMode::FullTurkic` explicitly
        // remaps ASCII `I` → `ı` (U+0131, non-ASCII) so it cannot
        // fast-path. Simple and Full modes never remap ASCII to
        // non-ASCII, so `to_ascii_lowercase` is the whole answer.
        if input.is_ascii() && !matches!(mode, FoldMode::FullTurkic) {
            return input.to_ascii_lowercase();
        }
        let mut out = String::with_capacity(input.len());
        for c in input.chars() {
            self.fold_char(c, mode, &mut out);
        }
        out
    }

    /// Look up the first-matching pack for a locale tag.
    ///
    /// Consults an exact-match on the tag first, then a small set of
    /// locale-equivalence aliases (see [`case_locale_alias`]) — this
    /// lets a caller who ships only the Turkish `tr` case pack still
    /// pack-hit when the query locale is Azerbaijani (`az` / `az-Latn`),
    /// which shares the Turkic dotted / dotless-I case tailorings.
    /// Alias resolution never re-consults the fallback chain — a match
    /// on `az` returns the `tr` pack directly.
    ///
    /// Callers that already have the original query locale available
    /// should prefer [`pack_for_with_origin`](Self::pack_for_with_origin)
    /// so an `az-Cyrl-*` query does not misfire the `az → tr` alias
    /// at the bare-`az` rung of the fallback chain (Cyrillic-script
    /// Azerbaijani does not share the Turkic-I rules). This bare
    /// `pack_for` variant passes the tag itself as the origin, so it
    /// is safe for single-tag lookups (e.g. `supports("de-DE")`).
    fn pack_for(&self, tag: &str) -> Option<&CasePack<'a>> {
        self.pack_for_with_origin(tag, tag)
    }

    /// Look up a pack for `tag`, using `origin` (the original query
    /// locale before fallback-chain stripping) to gate alias
    /// resolution. Callers walking the fallback chain pass the
    /// original query as `origin` so an `az-Cyrl-*` query does not
    /// incorrectly fire the `az → tr` alias at the bare-`az` rung
    /// of the chain (Cyrillic-script Azerbaijani does not share the
    /// Turkic dotted / dotless-I rules).
    fn pack_for_with_origin(&self, tag: &str, origin: &str) -> Option<&CasePack<'a>> {
        if let Some(pack) = self
            .packs
            .iter()
            .find(|p| p.locale.eq_ignore_ascii_case(tag))
        {
            return Some(pack);
        }
        if let Some(alias) = case_locale_alias_for_origin(tag, origin) {
            return self
                .packs
                .iter()
                .find(|p| p.locale.eq_ignore_ascii_case(alias));
        }
        None
    }

    /// Iterate every pack whose locale is a prefix of `locale` under
    /// the CLDR fallback chain, most-specific first.
    fn packs_for_locale<'e>(&'e self, locale: &'e str) -> impl Iterator<Item = &'e CasePack<'a>> {
        walk_fallback_chain(locale).filter_map(move |tag| self.pack_for_with_origin(tag, locale))
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

/// Map a locale tag to its case-mapping equivalence-class partner,
/// if any.
///
/// Some locales share the same locale-specific case tailorings — the
/// canonical example is Azerbaijani `az` (Latin script), whose
/// dotted/dotless-I rules are identical to Turkish `tr`. Rather than
/// requiring every consumer to ship two nearly-identical SCUD packs,
/// [`CaseEngine`]'s pack-lookup consults this table when an exact
/// tag-match misses so a caller who loaded only the `tr` pack still
/// pack-hits under `az` queries.
///
/// The alias is deliberately narrow: it fires only on the exact tag,
/// not on prefixes. Query `az-Cyrl` (Azerbaijani in Cyrillic script,
/// which does *not* share the Turkic-I rules) is filtered by an
/// explicit deny-list — `case_locale_alias("az-Cyrl")` returns
/// `None`. The higher-level [`case_locale_alias_for_origin`] helper
/// additionally suppresses the `az → tr` alias when the origin query
/// (before fallback-chain stripping) carries a Cyrl script tag, so
/// walking `az-Cyrl-AZ → az-Cyrl → az → ""` does not misfire the
/// alias at the bare-`az` rung.
///
/// The Phase 6 table is intentionally minimal (just `az → tr`); larger
/// tables (Lithuanian's soft-dotted-i, Greek final-sigma tailoring at
/// case boundaries, etc.) land alongside the packs that need them.
#[must_use]
pub fn case_locale_alias(tag: &str) -> Option<&'static str> {
    // Case-insensitive exact match — BCP 47 tags are ASCII so
    // eq_ignore_ascii_case matches locale-agnostic RFC 5646 canonical
    // forms (`AZ` == `az`).
    if tag.eq_ignore_ascii_case("az") || tag.eq_ignore_ascii_case("az-Latn") {
        return Some("tr");
    }
    None
}

/// Origin-aware variant of [`case_locale_alias`].
///
/// Consults [`case_locale_alias`] as usual, but suppresses the
/// `az → tr` alias when the `origin` locale carries a `Cyrl` script
/// tag — Cyrillic-script Azerbaijani (`az-Cyrl-*`) does not share
/// the Turkic dotted / dotless-I case rules with Turkish, so the
/// alias must not fire even after the fallback-chain walk strips
/// the `-Cyrl` subtag down to a bare `az`.
///
/// This is the hook that resolves the Phase 6 red flag around
/// `az-Cyrl-AZ` incorrectly inheriting Turkish tailorings. Bare
/// `az` still fires the alias (default script for Azerbaijani per
/// CLDR is Latin, so aliasing to `tr` is correct). Explicit
/// `az-Latn-*` still fires the alias.
#[must_use]
pub fn case_locale_alias_for_origin(tag: &str, origin: &str) -> Option<&'static str> {
    if locale_has_script(origin, "Cyrl") {
        return None;
    }
    case_locale_alias(tag)
}

/// True iff a locale tag tailors ASCII scalars under this engine's
/// pack lookup.
///
/// This is the gate that ships the [`CaseEngine`]'s ASCII fast paths
/// (`to_lower` / `to_upper` / `to_title` / `fold`). Every locale that
/// might remap an ASCII scalar to something other than the default
/// `char::to_uppercase` / `char::to_lowercase` result must be
/// deny-listed here so those inputs fall through to the pack-walking
/// slow path.
///
/// The Phase 6 deny-list is minimal — exactly the Turkic-I family:
///
/// * **Turkish** (`tr`, `tr-*`) — every `tr-*` tag walks the fallback
///   chain to bare `tr`, which pack-matches the Turkish pack; that
///   pack ships `LocaleOverrideLower` / `LocaleOverrideUpper` context
///   rules that remap ASCII `I` → `ı` and ASCII `i` → `İ`.
/// * **Azerbaijani Latin** (`az`, `az-Latn`, `az-Latn-*`, `az-<region>`)
///   — resolves to the Turkish pack via [`case_locale_alias`]. CLDR's
///   default script for Azerbaijani is Latin, so a region-tagged `az-AZ`
///   (no script subtag) inherits the same rules.
///
/// **Cyrillic-script Azerbaijani** (`az-Cyrl`, `az-Cyrl-*`) is
/// deliberately NOT deny-listed: [`case_locale_alias_for_origin`]
/// suppresses the `az → tr` alias whenever the origin carries a
/// `Cyrl` script tag, so ASCII scalars under `az-Cyrl-*` fall through
/// to the default Rust case rules — byte-identical to
/// `to_ascii_upper` / `to_ascii_lower`.
///
/// The predicate is a byte-level scan of the first one or two
/// subtags; it never allocates.
#[must_use]
pub fn locale_has_ascii_tailoring(locale: &str) -> bool {
    let mut parts = locale.split('-');
    let Some(lang) = parts.next() else {
        return false;
    };
    if lang.eq_ignore_ascii_case("tr") {
        return true;
    }
    if lang.eq_ignore_ascii_case("az") {
        // `az` or `az-<subtag>` — check whether the second subtag is
        // an explicit script tag. A four-letter subtag is a BCP 47
        // script; `Cyrl` is the only script Azerbaijani uses that
        // does NOT share Turkic-I rules. Anything else (no script
        // tag, `Latn` script tag, region tag, etc.) inherits the
        // Turkish tailorings via the `az → tr` alias.
        let Some(next) = parts.next() else {
            return true; // bare `az` — CLDR default script is Latin.
        };
        if next.len() == 4 {
            // Explicit script subtag — deny-list unless it's `Cyrl`.
            return !next.eq_ignore_ascii_case("Cyrl");
        }
        // Non-script second subtag (region etc.) — CLDR default
        // script is Latin, so the alias still fires.
        return true;
    }
    false
}

/// True iff `locale` carries the given four-letter script subtag.
///
/// BCP 47 script subtags are exactly four ASCII letters and appear
/// as the second subtag when present (after the two-or-three-letter
/// language). Matches case-insensitively so `az-cyrl-AZ` and
/// `AZ-Cyrl` both hit.
fn locale_has_script(locale: &str, script: &str) -> bool {
    debug_assert_eq!(script.len(), 4);
    for part in locale.split('-') {
        if part.len() == 4 && part.eq_ignore_ascii_case(script) {
            return true;
        }
    }
    false
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
        // Turkish dotted/dotless-I contextual mappings (Phase 6).
        c.push_context('I' as u32, ContextKind::LocaleOverrideLower, 0x0131);
        c.push_context('i' as u32, ContextKind::LocaleOverrideUpper, 0x0130);
        // Simple round-trip for the dotted / dotless capital pair —
        // default Unicode has no uppercase for U+0131 (ı) and lowers
        // U+0130 (İ) to "i̇" (i + U+0307), which is wrong for Turkish;
        // the pack overrides both via the simple tables.
        c.push_simple_lower(0x0130, 0x0069); // İ → i
        c.push_simple_upper(0x0131, 0x0049); // ı → I

        let mut w = ScudWriter::new(CAP_CASE, "44.1", Some("tr"));
        w.append_section(SECT_CONTEXT, &c.context_bytes());
        w.append_section(SECT_SIMPLE_LOWER, &c.simple_lower_bytes());
        w.append_section(SECT_SIMPLE_UPPER, &c.simple_upper_bytes());
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

    // -----------------------------------------------------------------------
    // Phase 6: Turkish-i locale-tailoring algorithm change
    //
    // The tr pack ships four Turkish-i rules via SECT_CONTEXT
    // (`LocaleOverrideLower` / `LocaleOverrideUpper`) plus symmetric
    // simple-table entries for `İ ↔ i` and `ı ↔ I`. Under the CLDR
    // fallback chain those rules apply when the query locale walks up
    // to `tr`. The `case_locale_alias` table extends the same rules to
    // Azerbaijani `az` (Latin) queries so a caller loading only the
    // `tr` pack still handles both locales — the whole point of a
    // shared-tailoring equivalence class.
    // -----------------------------------------------------------------------

    #[test]
    fn turkish_upper_lowercase_i_maps_to_dotted_capital() {
        // Rule: uppercase("i", "tr") == "İ" (U+0130).
        let (_en, tr) = engine_with_en_and_tr();
        let tr_pack = CasePack::from_scud_bytes(&tr).unwrap();
        let engine = CaseEngine::new(vec![tr_pack]);
        assert_eq!(engine.to_upper("i", "tr"), "\u{0130}");
    }

    #[test]
    fn turkish_lower_capital_i_maps_to_dotless_lowercase() {
        // Rule: lowercase("I", "tr") == "ı" (U+0131).
        let (_en, tr) = engine_with_en_and_tr();
        let tr_pack = CasePack::from_scud_bytes(&tr).unwrap();
        let engine = CaseEngine::new(vec![tr_pack]);
        assert_eq!(engine.to_lower("I", "tr"), "\u{0131}");
    }

    #[test]
    fn az_locale_alias_shares_turkish_tailorings() {
        // The az → tr alias means a caller shipping only the Turkish
        // pack still gets Turkic-I behaviour under Azerbaijani queries.
        let (_en, tr) = engine_with_en_and_tr();
        let tr_pack = CasePack::from_scud_bytes(&tr).unwrap();
        let engine = CaseEngine::new(vec![tr_pack]);
        // az bare tag walks fallback ["az", ""]; alias fires at "az".
        assert_eq!(engine.to_upper("i", "az"), "\u{0130}");
        assert_eq!(engine.to_lower("I", "az"), "\u{0131}");
        // az-Latn also fires the alias directly.
        assert_eq!(engine.to_upper("i", "az-Latn"), "\u{0130}");
        // az-AZ walks ["az-AZ", "az", ""] — alias fires at the "az"
        // rung, so region-tagged Azerbaijani inherits the same rules.
        assert_eq!(engine.to_upper("i", "az-AZ"), "\u{0130}");
    }

    #[test]
    fn turkish_control_english_locale_uses_default_case_rules() {
        // Cross-check: the same input under English lowercasing does
        // NOT apply the Turkish tailoring even when the tr pack is
        // loaded — the alias is scoped to tr / az queries only.
        let (en, tr) = engine_with_en_and_tr();
        let en_pack = CasePack::from_scud_bytes(&en).unwrap();
        let tr_pack = CasePack::from_scud_bytes(&tr).unwrap();
        let engine = CaseEngine::new(vec![en_pack, tr_pack]);
        assert_eq!(engine.to_upper("istanbul", "en"), "ISTANBUL");
        assert_eq!(engine.to_lower("ISTANBUL", "en"), "istanbul");
    }

    #[test]
    fn case_locale_alias_returns_expected_partners() {
        // The alias table is the algorithm-side authority for shared
        // case tailorings; a regression that widens or narrows it is
        // visible here.
        assert_eq!(case_locale_alias("az"), Some("tr"));
        assert_eq!(case_locale_alias("AZ"), Some("tr")); // case-insensitive
        assert_eq!(case_locale_alias("az-Latn"), Some("tr"));
        // Cyrillic-script Azerbaijani is *not* Turkic-I — the exact
        // `az-Cyrl` tag never fires the alias.
        assert_eq!(case_locale_alias("az-Cyrl"), None);
        assert_eq!(case_locale_alias("tr"), None);
        assert_eq!(case_locale_alias("en"), None);
    }

    #[test]
    fn case_locale_alias_for_origin_gates_on_script_tag() {
        // The origin-aware helper honours an explicit `az-Cyrl-*`
        // origin — the `az → tr` alias is suppressed at every rung
        // of the fallback walk (including bare `az`).
        assert_eq!(
            case_locale_alias_for_origin("az", "az-Cyrl-AZ"),
            None,
            "az-Cyrl origin must not inherit Turkish-I tailorings",
        );
        assert_eq!(case_locale_alias_for_origin("az", "az-Cyrl"), None,);
        // Bare `az` (no script tag) still fires the alias — CLDR's
        // default script for Azerbaijani is Latin, so treating it
        // as Turkic-I is correct.
        assert_eq!(case_locale_alias_for_origin("az", "az"), Some("tr"));
        // Explicit `az-Latn-*` still fires the alias.
        assert_eq!(case_locale_alias_for_origin("az", "az-Latn-AZ"), Some("tr"),);
        assert_eq!(
            case_locale_alias_for_origin("az-Latn", "az-Latn"),
            Some("tr"),
        );
    }

    #[test]
    fn az_cyrl_does_not_inherit_turkish_tailorings() {
        // End-to-end: a caller loading only the tr pack and querying
        // under `az-Cyrl-AZ` should NOT get Turkic-I behaviour. The
        // walk-up to bare `az` used to misfire the alias; the
        // origin-aware `pack_for_with_origin` now suppresses it.
        let (_en, tr) = engine_with_en_and_tr();
        let tr_pack = CasePack::from_scud_bytes(&tr).unwrap();
        let engine = CaseEngine::new(vec![tr_pack]);
        // Default Unicode: "i".to_upper() → "I", not "İ".
        assert_eq!(engine.to_upper("i", "az-Cyrl"), "I");
        assert_eq!(engine.to_upper("i", "az-Cyrl-AZ"), "I");
        // Similarly for lowercase — no Turkic-I dotless-fold.
        assert_eq!(engine.to_lower("I", "az-Cyrl"), "i");
        assert_eq!(engine.to_lower("I", "az-Cyrl-AZ"), "i");
    }

    #[test]
    fn turkish_dotless_lowercase_uppers_to_dotless_capital() {
        // Rule (via simple_upper in the tr pack): uppercase("ı", "tr")
        // == "I". Default Unicode has no uppercase mapping for U+0131,
        // so this only works because the tr pack ships the mapping.
        let (_en, tr) = engine_with_en_and_tr();
        let tr_pack = CasePack::from_scud_bytes(&tr).unwrap();
        let engine = CaseEngine::new(vec![tr_pack]);
        assert_eq!(engine.to_upper("\u{0131}", "tr"), "I");
    }

    #[test]
    fn turkish_dotted_capital_lowers_to_dotted_lowercase() {
        // Rule (via simple_lower in the tr pack): lowercase("İ", "tr")
        // == "i" (single scalar). Default Unicode lowercases İ to
        // "i̇" (i + combining dot above); the tr pack overrides that.
        let (_en, tr) = engine_with_en_and_tr();
        let tr_pack = CasePack::from_scud_bytes(&tr).unwrap();
        let engine = CaseEngine::new(vec![tr_pack]);
        assert_eq!(engine.to_lower("\u{0130}", "tr"), "i");
    }

    // -----------------------------------------------------------------------
    // ASCII fast-path differential tests.
    //
    // The four `CaseEngine` operations short-circuit ASCII input under
    // any non-Turkic locale to `str::to_ascii_*` — bypassing the
    // pack-walking loop entirely. The invariant to enforce is byte-
    // for-byte equality with the full pipeline across an ASCII corpus
    // for every locale the deny-list decides for. The deny-list must
    // fire for `tr`, `tr-*`, `az`, `az-Latn`, `az-Latn-*`, `az-<region>`
    // and NOT fire for `az-Cyrl-*` — the test corpus exercises both
    // sides.
    // -----------------------------------------------------------------------

    /// Locales the fast-path should NOT deny-list — must be
    /// byte-identical to the slow path on ASCII.
    const FAST_LOCALES: &[&str] = &[
        "en",
        "en-US",
        "de",
        "de-DE",
        "fr",
        "ru",
        "zh",
        "zh-Hant",
        "az-Cyrl",
        "az-Cyrl-AZ",
    ];

    /// Locales the fast-path MUST deny-list — the slow-path must fire
    /// so the Turkic-I rules apply.
    const TURKIC_LOCALES: &[&str] = &["tr", "tr-TR", "az", "az-Latn", "az-Latn-AZ", "az-AZ"];

    /// The full ASCII corpus every differential test walks.
    fn ascii_corpus() -> alloc::vec::Vec<alloc::string::String> {
        use alloc::string::ToString;
        alloc::vec![
            alloc::string::String::new(),
            "a".to_string(),
            "I".to_string(),
            "i".to_string(),
            "ISTANBUL".to_string(),
            "istanbul".to_string(),
            "Hello, World!".to_string(),
            "MixedCASE_identifier_42".to_string(),
            "  leading and trailing   ".to_string(),
            "\thas\ttabs\nand\nnewlines\r".to_string(),
            "control\x07chars\x1Fembedded".to_string(),
            "punctuation!?.,;:'\"()[]{}<>-_=+*/\\|@#$%^&`~".to_string(),
            "digits 0123456789 and letters".to_string(),
            (0x20u8..=0x7E)
                .map(|b| b as char)
                .collect::<alloc::string::String>(),
            (0x00u8..=0x7F)
                .map(|b| b as char)
                .collect::<alloc::string::String>(),
        ]
    }

    /// Rebuild the pre-fast-path body of `to_lower` — used as the
    /// oracle for the differential tests.
    fn to_lower_slow(engine: &CaseEngine<'_>, input: &str, locale: &str) -> alloc::string::String {
        let mut out = alloc::string::String::with_capacity(input.len());
        for c in input.chars() {
            engine.map_lower_char(c, locale, &mut out);
        }
        out
    }

    fn to_upper_slow(engine: &CaseEngine<'_>, input: &str, locale: &str) -> alloc::string::String {
        let mut out = alloc::string::String::with_capacity(input.len());
        for c in input.chars() {
            engine.map_upper_char(c, locale, &mut out);
        }
        out
    }

    fn to_title_slow(
        engine: &CaseEngine<'_>,
        input: &str,
        locale: &str,
        options: TitleOptions,
    ) -> alloc::string::String {
        let mut out = alloc::string::String::with_capacity(input.len());
        let mut at_boundary = true;
        for c in input.chars() {
            if is_title_boundary(c) {
                at_boundary = true;
                out.push(c);
                continue;
            }
            if at_boundary {
                engine.map_upper_char(c, locale, &mut out);
                at_boundary = false;
            } else if options.lowercase_tail {
                engine.map_lower_char(c, locale, &mut out);
            } else {
                out.push(c);
            }
        }
        out
    }

    fn fold_slow(engine: &CaseEngine<'_>, input: &str, mode: FoldMode) -> alloc::string::String {
        let mut out = alloc::string::String::with_capacity(input.len());
        for c in input.chars() {
            engine.fold_char(c, mode, &mut out);
        }
        out
    }

    #[test]
    fn ascii_fast_path_lower_matches_slow_path_across_locales() {
        let (en, tr) = engine_with_en_and_tr();
        let en_pack = CasePack::from_scud_bytes(&en).unwrap();
        let tr_pack = CasePack::from_scud_bytes(&tr).unwrap();
        let engine = CaseEngine::new(vec![en_pack, tr_pack]);
        for input in ascii_corpus() {
            assert!(input.is_ascii(), "corpus stayed ASCII: {input:?}");
            for &locale in FAST_LOCALES.iter().chain(TURKIC_LOCALES.iter()) {
                let fast = engine.to_lower(&input, locale);
                let slow = to_lower_slow(&engine, &input, locale);
                assert_eq!(
                    fast, slow,
                    "to_lower diverged: locale={locale:?} input={input:?}"
                );
            }
        }
    }

    #[test]
    fn ascii_fast_path_upper_matches_slow_path_across_locales() {
        let (en, tr) = engine_with_en_and_tr();
        let en_pack = CasePack::from_scud_bytes(&en).unwrap();
        let tr_pack = CasePack::from_scud_bytes(&tr).unwrap();
        let engine = CaseEngine::new(vec![en_pack, tr_pack]);
        for input in ascii_corpus() {
            assert!(input.is_ascii(), "corpus stayed ASCII: {input:?}");
            for &locale in FAST_LOCALES.iter().chain(TURKIC_LOCALES.iter()) {
                let fast = engine.to_upper(&input, locale);
                let slow = to_upper_slow(&engine, &input, locale);
                assert_eq!(
                    fast, slow,
                    "to_upper diverged: locale={locale:?} input={input:?}"
                );
            }
        }
    }

    #[test]
    fn ascii_fast_path_title_matches_slow_path_across_locales() {
        let (en, tr) = engine_with_en_and_tr();
        let en_pack = CasePack::from_scud_bytes(&en).unwrap();
        let tr_pack = CasePack::from_scud_bytes(&tr).unwrap();
        let engine = CaseEngine::new(vec![en_pack, tr_pack]);
        for input in ascii_corpus() {
            assert!(input.is_ascii(), "corpus stayed ASCII: {input:?}");
            for &locale in FAST_LOCALES.iter().chain(TURKIC_LOCALES.iter()) {
                for &lowercase_tail in &[true, false] {
                    let options = TitleOptions {
                        boundary: TitleBoundary::Words,
                        lowercase_tail,
                    };
                    let fast = engine.to_title(&input, locale, options).unwrap();
                    let slow = to_title_slow(&engine, &input, locale, options);
                    assert_eq!(
                        fast, slow,
                        "to_title diverged: locale={locale:?} lowercase_tail={lowercase_tail} input={input:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn ascii_fast_path_fold_matches_slow_path_across_modes() {
        // fold takes no locale; walk every FoldMode instead. Simple
        // and Full fast-path, FullTurkic goes through the slow path
        // (I → ı is a non-ASCII expansion).
        let (en, tr) = engine_with_en_and_tr();
        let en_pack = CasePack::from_scud_bytes(&en).unwrap();
        let tr_pack = CasePack::from_scud_bytes(&tr).unwrap();
        let engine = CaseEngine::new(vec![en_pack, tr_pack]);
        for input in ascii_corpus() {
            assert!(input.is_ascii(), "corpus stayed ASCII: {input:?}");
            for &mode in &[FoldMode::Simple, FoldMode::Full, FoldMode::FullTurkic] {
                let fast = engine.fold(&input, mode);
                let slow = fold_slow(&engine, &input, mode);
                assert_eq!(fast, slow, "fold diverged: mode={mode:?} input={input:?}");
            }
        }
    }

    #[test]
    fn ascii_fast_path_preserves_turkish_i_semantics() {
        // The Turkish-i rules are the whole reason for the deny-list.
        // These lock in that the deny-list fires for the two ASCII
        // inputs the tr / az packs remap.
        let (_en, tr) = engine_with_en_and_tr();
        let tr_pack = CasePack::from_scud_bytes(&tr).unwrap();
        let engine = CaseEngine::new(vec![tr_pack]);
        // to_upper("i", "tr") must be dotted-I ("İ" = U+0130), NOT
        // ASCII "I" (which is what the fast path would return).
        assert_eq!(engine.to_upper("i", "tr"), "\u{0130}");
        // to_lower("I", "tr") must be dotless-i ("ı" = U+0131), NOT
        // ASCII "i".
        assert_eq!(engine.to_lower("I", "tr"), "\u{0131}");
        // Same for Azerbaijani (aliased to tr).
        assert_eq!(engine.to_upper("i", "az"), "\u{0130}");
        assert_eq!(engine.to_lower("I", "az-Latn"), "\u{0131}");
        assert_eq!(engine.to_upper("i", "az-AZ"), "\u{0130}");
        // FoldMode::FullTurkic must map I → ı even on ASCII input.
        assert_eq!(engine.fold("I", FoldMode::FullTurkic), "\u{0131}");
    }

    #[test]
    fn locale_has_ascii_tailoring_matches_alias_table() {
        // Turkish + Latin-script Azerbaijani are deny-listed.
        assert!(locale_has_ascii_tailoring("tr"));
        assert!(locale_has_ascii_tailoring("TR"));
        assert!(locale_has_ascii_tailoring("tr-TR"));
        assert!(locale_has_ascii_tailoring("tr-Cyrl")); // still Turkish rules
        assert!(locale_has_ascii_tailoring("az"));
        assert!(locale_has_ascii_tailoring("AZ"));
        assert!(locale_has_ascii_tailoring("az-Latn"));
        assert!(locale_has_ascii_tailoring("az-Latn-AZ"));
        assert!(locale_has_ascii_tailoring("az-AZ")); // no script → default Latin

        // Cyrillic-script Azerbaijani + everything else: NOT deny-listed.
        assert!(!locale_has_ascii_tailoring("az-Cyrl"));
        assert!(!locale_has_ascii_tailoring("az-Cyrl-AZ"));
        assert!(!locale_has_ascii_tailoring("en"));
        assert!(!locale_has_ascii_tailoring("en-US"));
        assert!(!locale_has_ascii_tailoring("de"));
        assert!(!locale_has_ascii_tailoring("fr"));
        assert!(!locale_has_ascii_tailoring("ru"));
        assert!(!locale_has_ascii_tailoring("zh"));
        assert!(!locale_has_ascii_tailoring(""));
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
