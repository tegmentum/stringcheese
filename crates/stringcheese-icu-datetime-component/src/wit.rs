//! WIT `Guest` implementations for the reference datetime
//! component.
//!
//! Bridges the crate's native API ([`crate::reference_engine`],
//! [`crate::reference_supported_locales`]) to the shared
//! `tegmentum:i18n-datetime@0.1.0` interface so this crate can
//! ship as a standalone WebAssembly component that any
//! component-model-capable host loads without linking Rust.
//!
//! Only compiled on wasm targets with the `wit-component` feature.
//! See the crate-level docs and `stringcheese-icu-collation-component`
//! for the same pattern.

use alloc::string::String;
use alloc::vec::Vec;

use stringcheese_icu_datetime::{
    DateTimeEngine, DateTimeError as NativeError, DateTimeLength as NativeLength,
};

// -----------------------------------------------------------------------
// Shared engine
// -----------------------------------------------------------------------
//
// The `datetime-world` world is stateless in the WIT sense — the
// guest is re-instantiated fresh on every component instantiation.
// Within a single instantiation, though, the three loaded SCUD
// packs never change, so we build the `DateTimeEngine` once on
// first access and hand out borrows from an `AtomicPtr`-backed
// singleton. Same shape as
// `stringcheese-icu-collation-component`'s `shared_engine`.

fn shared_engine() -> &'static DateTimeEngine<'static> {
    use core::sync::atomic::{AtomicPtr, Ordering as AtomicOrdering};

    static ENGINE_PTR: AtomicPtr<DateTimeEngine<'static>> = AtomicPtr::new(core::ptr::null_mut());

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
/// emits no `Guest` trait for it — only `datetime` and
/// `capabilities` need Guest impls below.
pub struct Component;

// -----------------------------------------------------------------------
// The `datetime` interface — format-date / format-time /
// format-datetime.
// -----------------------------------------------------------------------

use crate::bindings::exports::tegmentum::i18n_datetime::datetime::{
    DateLength as WitDateLength, DatetimeError as WitError, Guest as DatetimeGuest,
    Locale as WitLocale, TimeLength as WitTimeLength,
};

impl DatetimeGuest for Component {
    fn format_date(
        iso_date: String,
        locale: WitLocale,
        length: WitDateLength,
    ) -> Result<String, WitError> {
        shared_engine()
            .format_date(&iso_date, &locale, to_native_date_length(length))
            .map_err(to_wit_error)
    }

    fn format_time(
        iso_time: String,
        locale: WitLocale,
        length: WitTimeLength,
    ) -> Result<String, WitError> {
        shared_engine()
            .format_time(&iso_time, &locale, to_native_time_length(length))
            .map_err(to_wit_error)
    }

    fn format_datetime(
        iso_datetime: String,
        locale: WitLocale,
        date_length: WitDateLength,
        time_length: WitTimeLength,
    ) -> Result<String, WitError> {
        shared_engine()
            .format_datetime(
                &iso_datetime,
                &locale,
                to_native_date_length(date_length),
                to_native_time_length(time_length),
            )
            .map_err(to_wit_error)
    }
}

// -----------------------------------------------------------------------
// The `capabilities` interface — introspection.
// -----------------------------------------------------------------------

use crate::bindings::exports::tegmentum::i18n_datetime::capabilities::{
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

fn to_native_date_length(l: WitDateLength) -> NativeLength {
    match l {
        WitDateLength::Short => NativeLength::Short,
        WitDateLength::Medium => NativeLength::Medium,
        WitDateLength::Long => NativeLength::Long,
        WitDateLength::Full => NativeLength::Full,
    }
}

fn to_native_time_length(l: WitTimeLength) -> NativeLength {
    match l {
        WitTimeLength::Short => NativeLength::Short,
        WitTimeLength::Medium => NativeLength::Medium,
        WitTimeLength::Long => NativeLength::Long,
        WitTimeLength::Full => NativeLength::Full,
    }
}

fn to_wit_error(e: NativeError) -> WitError {
    use alloc::string::ToString as _;
    match e {
        NativeError::InvalidLocale(s) => WitError::InvalidLocale(s.to_string()),
        NativeError::LocaleUnavailable(s) => WitError::LocaleUnavailable(s.to_string()),
        NativeError::InvalidInput(s) => WitError::InvalidInput(s.to_string()),
        NativeError::OutOfRange(s) => WitError::OutOfRange(s.to_string()),
    }
}

// Register the `Component` type as the guest implementation for
// every trait the `datetime-world` world exports.
crate::bindings::export!(Component with_types_in crate::bindings);
