//! WIT `Guest` implementations for the reference collation
//! component.
//!
//! Bridges the crate's native API ([`crate::reference_engine`],
//! [`crate::reference_supported_locales`],
//! [`crate::REFERENCE_CLDR_VERSION`]) to the shared
//! `stringcheese:icu-collation@0.1.0` interface so this crate can
//! ship as a standalone WebAssembly component that any
//! component-model-capable host loads without linking Rust.
//!
//! Only compiled on wasm targets with the `wit-component` feature.
//! Native builds skip the bindings module so `cargo test` doesn't
//! pull in `wit-bindgen-rt`, and wasm builds without the feature
//! also skip it — otherwise two backends `export!`-ing the same
//! `stringcheese:icu-collation` interface into one binary would
//! collide at link time with duplicate exported symbols. See the
//! crate-level docs and `stringcheese-icu-case-component` for the
//! same pattern.

use alloc::string::String;
use alloc::string::ToString as _;
use alloc::vec::Vec;
use core::cmp::Ordering as NativeOrdering;

use stringcheese_icu_collation::{CollationEngine, CollationStrength as NativeStrength};

// -----------------------------------------------------------------------
// Shared engine
// -----------------------------------------------------------------------
//
// The `collation-world` world is stateless in the WIT sense — the
// guest is re-instantiated fresh on every component instantiation.
// Within a single instantiation, though, the two loaded SCUD packs
// never change, so we build the `CollationEngine` once on first
// access and hand out borrows from an `AtomicPtr`-backed singleton.
// This matches what a long-lived host would do naturally; the
// guest just makes it explicit. Same shape as
// `stringcheese-icu-case-component`'s `shared_engine`.

fn shared_engine() -> &'static CollationEngine<'static> {
    use core::sync::atomic::{AtomicPtr, Ordering as AtomicOrdering};

    static ENGINE_PTR: AtomicPtr<CollationEngine<'static>> = AtomicPtr::new(core::ptr::null_mut());

    let existing = ENGINE_PTR.load(AtomicOrdering::Acquire);
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
/// implemented on. Zero sized; the shared `CollationEngine` lives
/// behind an `AtomicPtr`-backed singleton in this module rather
/// than on the struct itself. A future component that took
/// per-instance configuration (e.g. a caller-supplied pack list)
/// would carry it here or expose it via a WIT `resource`.
///
/// The WIT `types` interface has no functions, so wit-bindgen
/// emits no `Guest` trait for it — only `collation` and
/// `capabilities` need Guest impls below.
pub struct Component;

// -----------------------------------------------------------------------
// The `collation` interface — compare + sort-key.
// -----------------------------------------------------------------------

use crate::bindings::exports::stringcheese::icu_collation::collation::{
    CollationError as WitCollationError, CollationStrength as WitStrength, Guest as CollationGuest,
    Locale as WitLocale, Ordering as WitOrdering,
};

impl CollationGuest for Component {
    fn compare(
        a: String,
        b: String,
        locale: WitLocale,
        strength: WitStrength,
    ) -> Result<WitOrdering, WitCollationError> {
        // The Phase 2 `compare` never returns an error — the WIT
        // signature carries `result<ordering, collation-error>` to
        // reserve room for a future strict-mode validator that
        // rejects malformed BCP 47 tags. Here we always succeed.
        let ord = shared_engine().compare(&a, &b, &locale, to_native_strength(strength));
        Ok(to_wit_ordering(ord))
    }

    fn sort_key(
        text: String,
        locale: WitLocale,
        strength: WitStrength,
    ) -> Result<Vec<u8>, WitCollationError> {
        Ok(shared_engine().sort_key(&text, &locale, to_native_strength(strength)))
    }
}

// -----------------------------------------------------------------------
// The `capabilities` interface — introspection.
// -----------------------------------------------------------------------

use crate::bindings::exports::stringcheese::icu_collation::capabilities::{
    CapabilitiesRecord as WitCapabilitiesRecord, Guest as CapabilitiesGuest, Locale as CapLocale,
};

impl CapabilitiesGuest for Component {
    fn get_capabilities() -> WitCapabilitiesRecord {
        // `max_strength` reports the deepest strength the algorithm
        // implements. Phase 2 exposes the full ladder up to
        // `identical` (the tertiary compare with a full-input
        // tiebreak). See `docs/design/wit-i18n.md` § 8.2 for the
        // strength implementation notes.
        WitCapabilitiesRecord {
            supported_locales: crate::reference_supported_locales(),
            max_strength: WitStrength::Identical,
            cldr_version: crate::REFERENCE_CLDR_VERSION.to_string(),
        }
    }

    fn supports(loc: CapLocale) -> bool {
        shared_engine().supports(&loc)
    }
}

// -----------------------------------------------------------------------
// WIT <-> native type bridges.
// -----------------------------------------------------------------------

fn to_native_strength(s: WitStrength) -> NativeStrength {
    match s {
        WitStrength::Primary => NativeStrength::Primary,
        WitStrength::Secondary => NativeStrength::Secondary,
        WitStrength::Tertiary => NativeStrength::Tertiary,
        WitStrength::Quaternary => NativeStrength::Quaternary,
        WitStrength::Identical => NativeStrength::Identical,
    }
}

fn to_wit_ordering(o: NativeOrdering) -> WitOrdering {
    match o {
        NativeOrdering::Less => WitOrdering::Less,
        NativeOrdering::Equal => WitOrdering::Equal,
        NativeOrdering::Greater => WitOrdering::Greater,
    }
}

// Register the `Component` type as the guest implementation for
// every trait the `collation-world` world exports.
crate::bindings::export!(Component with_types_in crate::bindings);
