//! WIT `Guest` implementations for the reference case-mapping
//! component.
//!
//! Bridges the crate's native API ([`crate::reference_engine`],
//! [`crate::reference_supported_locales`]) to the shared
//! `stringcheese:icu-case@0.1.0` interface so this crate can ship
//! as a standalone WebAssembly component that any
//! component-model-capable host loads without linking Rust.
//!
//! Only compiled on wasm targets with the `wit-component` feature.
//! Native builds skip the bindings module so `cargo test` doesn't
//! pull in `wit-bindgen-rt`, and wasm builds without the feature
//! also skip it — otherwise two backends `export!`-ing the same
//! `stringcheese:icu-case` interface into one binary would collide
//! at link time with duplicate exported symbols. See the crate-level
//! docs and `stringcheese-tokenizer-component` for the same pattern.

use alloc::string::String;
use alloc::vec::Vec;

use stringcheese_icu_case::{
    CaseEngine, CaseError as NativeCaseError, FoldMode as NativeFoldMode,
    TitleBoundary as NativeTitleBoundary, TitleOptions as NativeTitleOptions,
};

// -----------------------------------------------------------------------
// Shared engine
// -----------------------------------------------------------------------
//
// The `case` world is stateless in the WIT sense — the guest is
// re-instantiated fresh on every component instantiation. Within a
// single instantiation, though, the two loaded SCUD packs never
// change, so we build the `CaseEngine` once on first access and
// hand out borrows from a `OnceLock`. This matches what a
// long-lived host would do naturally; the guest just makes it
// explicit.

fn shared_engine() -> &'static CaseEngine<'static> {
    // `OnceLock` gives us `Sync` init without pulling in a lazy_static
    // dependency. The engine holds only `&'static` SCUD-byte
    // references, so the returned reference is safe to share across
    // whatever thread structure the host provides (single-threaded,
    // in the wasip1 case).
    use core::sync::atomic::{AtomicPtr, Ordering};

    static ENGINE_PTR: AtomicPtr<CaseEngine<'static>> = AtomicPtr::new(core::ptr::null_mut());

    let existing = ENGINE_PTR.load(Ordering::Acquire);
    if !existing.is_null() {
        // SAFETY: the pointer was published by the initialising
        // thread via a `Release` store, and the pointee lives for
        // the lifetime of the component (an intentional leak —
        // wasm modules do not "unload").
        return unsafe { &*existing };
    }

    let engine = alloc::boxed::Box::new(crate::reference_engine());
    let raw = alloc::boxed::Box::into_raw(engine);
    match ENGINE_PTR.compare_exchange(
        core::ptr::null_mut(),
        raw,
        Ordering::AcqRel,
        Ordering::Acquire,
    ) {
        Ok(_) => {
            // SAFETY: we won the race; the pointee is now the
            // canonical singleton for the rest of the component's
            // lifetime.
            unsafe { &*raw }
        }
        Err(winner) => {
            // Another thread beat us to it; drop our losing box
            // and yield to the winner's pointer.
            // SAFETY: `raw` came from `Box::into_raw`; reclaiming
            // it into a `Box` before it was published is safe.
            drop(unsafe { alloc::boxed::Box::from_raw(raw) });
            // SAFETY: `winner` was published via `AcqRel` in the
            // race we lost.
            unsafe { &*winner }
        }
    }
}

// -----------------------------------------------------------------------
// The unit struct every WIT `Guest` trait is implemented on.
// -----------------------------------------------------------------------

/// The unit struct every WIT `Guest` trait in this component is
/// implemented on. Zero sized; the shared `CaseEngine` lives
/// behind an `AtomicPtr`-backed singleton in this module rather
/// than on the struct itself. A future component that took
/// per-instance configuration (e.g. a caller-supplied pack list)
/// would carry it here or expose it via a WIT `resource`.
///
/// The WIT `types` interface has no functions, so wit-bindgen
/// emits no `Guest` trait for it — only `mapping` and
/// `capabilities` need Guest impls below.
pub struct Component;

// -----------------------------------------------------------------------
// The `mapping` interface — the four core case-mapping exports.
// -----------------------------------------------------------------------

use crate::bindings::exports::stringcheese::icu_case::mapping::{
    CaseError as WitCaseError, FoldMode as WitFoldMode, Guest as MappingGuest, Locale as WitLocale,
    TitleOptions as WitTitleOptions,
};

impl MappingGuest for Component {
    fn to_lower(input: String, locale: WitLocale) -> Result<String, WitCaseError> {
        // The Phase 1 `to_lower` never returns an error — the WIT
        // signature carries `result<string, case-error>` to reserve
        // room for a future strict-mode validator that rejects
        // malformed BCP 47 tags. Here we always succeed.
        Ok(shared_engine().to_lower(&input, &locale))
    }

    fn to_upper(input: String, locale: WitLocale) -> Result<String, WitCaseError> {
        Ok(shared_engine().to_upper(&input, &locale))
    }

    fn to_title(
        input: String,
        locale: WitLocale,
        options: WitTitleOptions,
    ) -> Result<String, WitCaseError> {
        let native_options = NativeTitleOptions {
            boundary: to_native_boundary(options.boundary),
            lowercase_tail: options.lowercase_tail,
        };
        shared_engine()
            .to_title(&input, &locale, native_options)
            .map_err(to_wit_error)
    }

    fn fold(input: String, mode: WitFoldMode) -> String {
        shared_engine().fold(&input, to_native_fold_mode(mode))
    }
}

// -----------------------------------------------------------------------
// The `capabilities` interface — introspection.
// -----------------------------------------------------------------------

use crate::bindings::exports::stringcheese::icu_case::capabilities::{
    Guest as CapabilitiesGuest, Locale as CapLocale,
};

impl CapabilitiesGuest for Component {
    fn supported_locales() -> Vec<CapLocale> {
        crate::reference_supported_locales()
    }

    fn supports(loc: CapLocale) -> bool {
        shared_engine().supports(&loc)
    }
}

// -----------------------------------------------------------------------
// WIT <-> native type bridges.
// -----------------------------------------------------------------------

fn to_native_fold_mode(mode: WitFoldMode) -> NativeFoldMode {
    match mode {
        WitFoldMode::Simple => NativeFoldMode::Simple,
        WitFoldMode::Full => NativeFoldMode::Full,
        WitFoldMode::FullTurkic => NativeFoldMode::FullTurkic,
    }
}

fn to_native_boundary(
    b: crate::bindings::exports::stringcheese::icu_case::types::TitleBoundary,
) -> NativeTitleBoundary {
    use crate::bindings::exports::stringcheese::icu_case::types::TitleBoundary as WitBoundary;
    match b {
        WitBoundary::Graphemes => NativeTitleBoundary::Graphemes,
        WitBoundary::Words => NativeTitleBoundary::Words,
        WitBoundary::Sentences => NativeTitleBoundary::Sentences,
    }
}

fn to_wit_error(e: NativeCaseError) -> WitCaseError {
    // The native error variants carry `&'static str` payloads (fixed
    // discriminator strings baked at the algorithm crate). The WIT
    // variant expects owned strings; convert with a small copy.
    use alloc::string::ToString as _;
    match e {
        NativeCaseError::InvalidLocale(s) => WitCaseError::InvalidLocale(s.to_string()),
        NativeCaseError::LocaleUnavailable(s) => WitCaseError::LocaleUnavailable(s.to_string()),
        NativeCaseError::UnsupportedTitleMode(s) => {
            WitCaseError::UnsupportedTitleMode(s.to_string())
        }
    }
}

// Register the `Component` type as the guest implementation for
// every trait the `case` world exports.
crate::bindings::export!(Component with_types_in crate::bindings);
