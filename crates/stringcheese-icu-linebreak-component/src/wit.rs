//! WIT `Guest` implementations for the reference line-break
//! component.
//!
//! Bridges the crate's native API ([`crate::reference_engine`],
//! [`crate::reference_supported_locales`]) to the shared
//! `tegmentum:i18n-linebreak@0.1.0` interface so this crate can
//! ship as a standalone WebAssembly component that any
//! component-model-capable host loads without linking Rust.
//!
//! Only compiled on wasm targets with the `wit-component` feature.
//! See the crate-level docs and `stringcheese-icu-segment-component`
//! for the same pattern.

use alloc::string::String;
use alloc::vec::Vec;

use stringcheese_icu_linebreak::{BreakKind, LineBreakEngine, Strictness};

// -----------------------------------------------------------------------
// Shared engine
// -----------------------------------------------------------------------
//
// The `linebreak-world` world is stateless in the WIT sense — the
// guest is re-instantiated fresh on every component instantiation.
// The Phase 5 `LineBreakEngine` carries no per-call mutable state
// and no loaded pack (locale-neutral default), so we can hand out a
// borrow of a single `static`ally-materialised engine cheaply. Same
// shape as `stringcheese-icu-segment-component`'s `shared_engine`.

fn shared_engine() -> &'static LineBreakEngine<'static> {
    use core::sync::atomic::{AtomicPtr, Ordering as AtomicOrdering};

    static ENGINE_PTR: AtomicPtr<LineBreakEngine<'static>> = AtomicPtr::new(core::ptr::null_mut());

    let existing = ENGINE_PTR.load(AtomicOrdering::Acquire);
    if !existing.is_null() {
        // SAFETY: the pointer was published by the initialising
        // thread via a `Release` store, and the pointee lives for
        // the lifetime of the component.
        return unsafe { &*existing };
    }

    let engine = alloc::boxed::Box::new(crate::reference_engine());
    let raw = alloc::boxed::Box::into_raw(engine);
    match ENGINE_PTR.compare_exchange(
        core::ptr::null_mut(),
        raw,
        AtomicOrdering::AcqRel,
        AtomicOrdering::Acquire,
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
/// implemented on.
///
/// The WIT `types` interface has no functions, so wit-bindgen
/// emits no `Guest` trait for it — only `linebreak` and
/// `capabilities` need Guest impls below.
pub struct Component;

// -----------------------------------------------------------------------
// The `linebreak` interface — find-breaks /
// find-breaks-with-strictness.
// -----------------------------------------------------------------------

use crate::bindings::exports::tegmentum::i18n_linebreak::linebreak::{
    BreakOpportunity as WitBreakOpportunity, Guest as LinebreakGuest,
    LineBreakStrictness as WitStrictness,
};
use crate::bindings::exports::tegmentum::i18n_linebreak::types::BreakKind as WitBreakKind;

impl LinebreakGuest for Component {
    fn find_breaks(text: String) -> Vec<WitBreakOpportunity> {
        shared_engine()
            .find_breaks(&text)
            .into_iter()
            .map(to_wit_break_opportunity)
            .collect()
    }

    fn find_breaks_with_strictness(
        text: String,
        strictness: WitStrictness,
    ) -> Vec<WitBreakOpportunity> {
        // Cheap: `LineBreakEngine` is `Copy`.
        let engine = shared_engine().with_strictness(from_wit_strictness(strictness));
        engine
            .find_breaks(&text)
            .into_iter()
            .map(to_wit_break_opportunity)
            .collect()
    }
}

// -----------------------------------------------------------------------
// The `capabilities` interface — introspection.
// -----------------------------------------------------------------------

use crate::bindings::exports::tegmentum::i18n_linebreak::capabilities::{
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

fn to_wit_break_opportunity(
    opp: stringcheese_icu_linebreak::BreakOpportunity,
) -> WitBreakOpportunity {
    WitBreakOpportunity {
        offset: opp.offset,
        kind: match opp.kind {
            BreakKind::Mandatory => WitBreakKind::Mandatory,
            BreakKind::Allowed => WitBreakKind::Allowed,
        },
    }
}

fn from_wit_strictness(s: WitStrictness) -> Strictness {
    match s {
        WitStrictness::Loose => Strictness::Loose,
        WitStrictness::Normal => Strictness::Normal,
        WitStrictness::Strict => Strictness::Strict,
    }
}

// Register the `Component` type as the guest implementation for
// every trait the `linebreak-world` world exports.
crate::bindings::export!(Component with_types_in crate::bindings);
