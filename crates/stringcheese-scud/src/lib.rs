//! StringCheese Unicode Data (SCUD) file format.
//!
//! SCUD is the on-disk data-pack format shared by every
//! `stringcheese-icu-*` capability crate. One SCUD file carries the
//! data for one *(capability, locale)* tuple — a Turkish case-mapping
//! pack, an English collation pack, a Japanese break-iteration pack —
//! so a caller loading two locales for one capability pays for exactly
//! two files.
//!
//! # Position in the WIT-i18n subsystem
//!
//! Phase 1 (foundation) of the WIT-i18n design
//! (`docs/design/wit-i18n.md` § 8). The design commits to a
//! `stringcheese-scud` crate that parses well-formed SCUD files and
//! extracts magic / version / CLDR version / capability views; this
//! crate is that reference.
//!
//! # What Phase 1 ships (and what it defers)
//!
//! Phase 1 targets the *minimum* SCUD subset needed to unblock the
//! `stringcheese-icu-case` capability crate and its two reference
//! packs (`stringcheese-en`, `stringcheese-tr`):
//!
//! * **File header parsing** — magic, format version, flags,
//!   capability tag, embedded header. Loader rejects mismatched magic,
//!   unsupported major versions, unknown capabilities, and truncated
//!   bytes.
//! * **Capability views (raw body)** — [`CaseDataView`] (Phase 1) and
//!   [`CollationDataView`] (Phase 2) over the body bytes.
//!   The full design allows outer Brotli/Zstd compression
//!   (§ 4.2 flag bits 0 and 1); Phase 1's writer emits the *raw*
//!   variant (flag bits clear) so the reference implementation stays
//!   dependency-free. The header layout is forward-compatible with a
//!   later flag-driven decompression pass.
//! * **Section-oriented body** — the body is a sequence of
//!   `(section-id: [u8;4], length: u32, bytes: [u8; length])` frames
//!   read via [`SectionReader`] / written via [`ScudWriter`]. A
//!   new capability adds a new section id without touching the loader.
//!
//! Deferred to later phases (see [`docs/design/wit-i18n.md`](../../docs/design/wit-i18n.md)
//! § 4.2):
//!
//! * Structural compression primitives (`RangeDelta`, `AdaptivePages`,
//!   `PackedIntegers`, `SequencePool`, `StringPool`, `LoudsTrie`,
//!   `FiniteStateTable`). Phase 1 encodes case data as plain sorted
//!   `(u32, u32)` tables and multi-scalar mapping tables — enough for
//!   the shipped ASCII + Latin-1 + Turkish tailoring subset without
//!   pulling in an FST library.
//! * Outer Brotli / Zstd stream compression. Bit 0 (Brotli) and
//!   bit 1 (Zstd) of [`ScudFlags`] are reserved; the loader currently
//!   rejects a file whose body is compressed with either. Adding a
//!   decompression pass is a backwards-compatible loader upgrade.
//! * mmap-backed loading. The `std` feature exposes [`ScudFile::open`],
//!   which reads the whole file into a `Vec<u8>` for now. Swapping
//!   the storage backend for `memmap2::Mmap` is another
//!   backwards-compatible loader upgrade.
//!
//! # Wire format (v1.0)
//!
//! ```text
//!   offset  size  meaning
//!    0      4     magic         b"SCUD" (0x53 0x43 0x55 0x44)
//!    4      2     fmt-maj       u16 le
//!    6      2     fmt-min       u16 le
//!    8      4     flags         u32 le (see ScudFlags)
//!   12      4     cap-id        [u8; 4]  e.g. b"CASE"
//!   16      4     header-len    u32 le
//!   20      hdr   header        length = header-len bytes
//!   ...    body   body          length = file-len - 20 - hdr
//! ```
//!
//! The header carries capability-agnostic metadata:
//!
//! ```text
//!    0     4     cldr-len       u32 le
//!    4     cl    cldr-version   UTF-8 (e.g. "44.1")
//!    ..    4     locale-len     u32 le
//!    ..    ll    locale-tag     UTF-8 BCP 47 (e.g. "tr")
//! ```
//!
//! Everything is little-endian to match the WebAssembly linear-memory
//! byte order. Section-oriented body framing is documented on
//! [`SectionReader`].
//!
//! # Trust model
//!
//! SCUD files are **trusted input** (see
//! [`docs/design/wit-i18n.md`](../../docs/design/wit-i18n.md) § 7.1).
//! Language packs ship their SCUD as `include_bytes!` constants and
//! callers load them through [`ScudFile::from_static`]; the loader
//! validates structural invariants (magic, length bounds, UTF-8) so a
//! corrupt file surfaces as a typed [`ScudError`] rather than a
//! panic, but does not defend against maliciously crafted input.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use core::fmt;

/// The four-byte magic that every SCUD file starts with.
///
/// ASCII `S` `C` `U` `D` (0x53 0x43 0x55 0x44). A file whose first
/// four bytes do not match is rejected with
/// [`ScudError::NotScud`].
pub const MAGIC: [u8; 4] = *b"SCUD";

/// The SCUD wire format's major version. The loader accepts a file
/// only when its `fmt-maj` equals this constant; a bump is a hard
/// incompatibility that requires a coordinated release.
pub const SUPPORTED_FORMAT_MAJOR: u16 = 1;

/// The SCUD wire format's minor version emitted by [`ScudWriter`].
/// Loaders accept any minor version whose major matches
/// [`SUPPORTED_FORMAT_MAJOR`]; newer minor versions may add fields
/// after the last known one but do not change existing layouts.
pub const CURRENT_FORMAT_MINOR: u16 = 0;

/// Fixed byte length of the outer file header prefix (magic through
/// `header-len`) — the offset at which the capability-specific
/// header begins.
pub const HEADER_PREFIX_LEN: usize = 20;

/// Capability tag for the case-mapping capability (bytes `C A S E`).
pub const CAP_CASE: [u8; 4] = *b"CASE";

/// Capability tag for collation. Reserved; not consumed in Phase 1.
pub const CAP_COLLATION: [u8; 4] = *b"COLL";

/// Capability tag for plural rules. Reserved.
pub const CAP_PLURAL: [u8; 4] = *b"PLUR";

/// Capability tag for number formatting. Reserved.
pub const CAP_NUMBER: [u8; 4] = *b"NUMB";

/// Capability tag for date/time formatting. Reserved.
pub const CAP_DATETIME: [u8; 4] = *b"DTFM";

/// Capability tag for break iteration. Reserved.
pub const CAP_BREAK: [u8; 4] = *b"BRKI";

/// Capability tag for line-break iteration (UAX #14). Reserved for
/// Phase 5's follow-up `stringcheese-icu-linebreak` crate; distinct
/// from [`CAP_BREAK`] so a caller only interested in line-break
/// classification does not have to load the (larger) UAX #29
/// grapheme / word / sentence class tables. Bytes `L` `B` `R` `K`.
pub const CAP_LINEBREAK: [u8; 4] = *b"LBRK";

/// Bitfield of file-level flags stored at offset 8.
///
/// Only bits 0-3 are defined; bits 4-31 are reserved and must be zero
/// on write. The loader tolerates unknown bits so a forward-compatible
/// minor-version bump can introduce new flags.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ScudFlags(u32);

impl ScudFlags {
    /// Bit 0 — the body is a single Brotli stream that decompresses to
    /// the raw body layout. Not yet supported by this loader; a file
    /// with this bit set is rejected.
    pub const BROTLI: u32 = 1 << 0;
    /// Bit 1 — the body is a single Zstd stream. Not yet supported.
    pub const ZSTD: u32 = 1 << 1;
    /// Bit 2 — the header prefix is 8-byte-aligned. Always set in
    /// the wire format; kept as a flag for future variance.
    pub const HEADER_ALIGNED: u32 = 1 << 2;
    /// Bit 3 — the header carries a locale tag after the CLDR version.
    /// Always set for Phase 1 packs.
    pub const HAS_LOCALE: u32 = 1 << 3;

    /// Wrap a raw `u32` as a [`ScudFlags`] view.
    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// The raw `u32` behind this flags value.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// True when this flags value has *any* body compression bit
    /// (Brotli or Zstd) set. Phase 1's loader rejects such files.
    #[must_use]
    pub const fn is_body_compressed(self) -> bool {
        (self.0 & (Self::BROTLI | Self::ZSTD)) != 0
    }
}

/// The typed failure modes of the SCUD loader.
///
/// Structural only — the loader treats SCUD files as trusted input
/// (see the crate-level *Trust model* section) so failure modes are
/// bounded to "this file is not a SCUD file we can interpret".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScudError {
    /// The first four bytes were not `b"SCUD"`.
    NotScud,
    /// The file's major format version is not
    /// [`SUPPORTED_FORMAT_MAJOR`].
    UnsupportedMajorVersion {
        /// The `fmt-maj` value read from the file.
        file: u16,
        /// The loader's supported major version.
        supported: u16,
    },
    /// The `cap-id` field did not match any known capability tag.
    UnsupportedCapability {
        /// The four-byte capability tag as it appeared in the file.
        got: [u8; 4],
    },
    /// The file ended before the outer header prefix was fully read,
    /// or before the declared `header-len` bytes were available.
    HeaderTruncated,
    /// The file ended before the declared body length was fully read,
    /// or before a body section's declared length was available.
    BodyTruncated,
    /// A [`ScudFlags::BROTLI`] or [`ScudFlags::ZSTD`] bit was set;
    /// Phase 1 supports only raw (uncompressed) body layouts.
    UnsupportedCompression,
    /// The header carried a UTF-8-encoded string (CLDR version or
    /// locale tag) that was not well-formed.
    InvalidUtf8,
    /// The header's declared field lengths overflowed its bounded
    /// region.
    InvalidHeader,
    /// A capability-projection view was requested but the file's
    /// `cap-id` names a different capability.
    CapabilityMismatch {
        /// The four-byte capability tag the caller expected.
        expected: [u8; 4],
        /// The four-byte capability tag actually present in the file.
        got: [u8; 4],
    },
    /// A section-oriented body reader encountered an unexpected end
    /// of input, a duplicate section id, or an ill-formed section
    /// header.
    InvalidSection,
    /// An error occurred while reading the file from disk. Only
    /// possible under the `std` feature.
    #[cfg(feature = "std")]
    Io(std::io::ErrorKind),
}

impl fmt::Display for ScudError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotScud => f.write_str("not a SCUD file (magic mismatch)"),
            Self::UnsupportedMajorVersion { file, supported } => write!(
                f,
                "unsupported SCUD major version: file={file}, supported={supported}"
            ),
            Self::UnsupportedCapability { got } => write!(
                f,
                "unsupported SCUD capability: {got:?}",
                got = core::str::from_utf8(got).unwrap_or("<non-utf8>")
            ),
            Self::HeaderTruncated => f.write_str("SCUD header truncated"),
            Self::BodyTruncated => f.write_str("SCUD body truncated"),
            Self::UnsupportedCompression => f.write_str("SCUD body compression not yet supported"),
            Self::InvalidUtf8 => f.write_str("SCUD header contained invalid UTF-8"),
            Self::InvalidHeader => f.write_str("SCUD header field lengths out of range"),
            Self::CapabilityMismatch { expected, got } => write!(
                f,
                "SCUD capability mismatch: expected {:?}, got {:?}",
                core::str::from_utf8(expected).unwrap_or("<non-utf8>"),
                core::str::from_utf8(got).unwrap_or("<non-utf8>"),
            ),
            Self::InvalidSection => f.write_str("SCUD section framing invalid"),
            #[cfg(feature = "std")]
            Self::Io(kind) => write!(f, "SCUD I/O error: {kind:?}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ScudError {}

#[cfg(feature = "std")]
impl From<std::io::Error> for ScudError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.kind())
    }
}

/// A parsed SCUD file backed by borrowed bytes.
///
/// Constructed via [`ScudFile::from_static`] (the primary path for
/// language packs that embed their SCUD as an `include_bytes!`
/// constant) or [`ScudFile::from_slice`] (borrowing from a caller-owned
/// buffer). The `std` feature adds [`ScudFile::open`] and
/// [`ScudFile::from_bytes`] for on-disk loading with owned storage.
///
/// The type is `Copy`-cheap to clone (it holds a small header plus a
/// byte-slice reference into the underlying bytes) and safe to reuse
/// across queries; every accessor is `O(1)` after construction.
#[derive(Debug, Clone, Copy)]
pub struct ScudFile<'a> {
    bytes: &'a [u8],
    header: ScudHeader<'a>,
}

/// A parsed SCUD file that owns its byte buffer.
///
/// Distinct from [`ScudFile`] because owning the bytes changes the
/// lifetime story — a [`ScudFile<'a>`] borrows from external storage,
/// while an [`OwnedScudFile`] carries its own `Vec<u8>` and hands out
/// borrowed [`ScudFile`] views via [`OwnedScudFile::as_view`].
///
/// Only available under `alloc`.
#[cfg(feature = "alloc")]
#[derive(Debug, Clone)]
pub struct OwnedScudFile {
    bytes: Vec<u8>,
}

#[cfg(feature = "alloc")]
impl OwnedScudFile {
    /// Wrap `bytes` as an [`OwnedScudFile`], parsing the header once
    /// to validate structure. The parsed header is discarded; call
    /// [`as_view`](Self::as_view) to project a [`ScudFile`] view.
    pub fn new(bytes: Vec<u8>) -> Result<Self, ScudError> {
        // Validate the header at construction time so a malformed
        // file surfaces here rather than in a downstream `as_view`.
        let _ = ScudFile::from_slice(&bytes)?;
        Ok(Self { bytes })
    }

    /// Borrow the owned bytes as a [`ScudFile`].
    ///
    /// Cheap; every call re-parses the outer header, which is `O(1)`.
    ///
    /// # Panics
    ///
    /// Never — the header was validated at construction, so this call
    /// unwraps a `Result` that cannot fail unless the owned buffer
    /// was mutated behind the type's back (which is not possible via
    /// the safe API).
    #[must_use]
    pub fn as_view(&self) -> ScudFile<'_> {
        ScudFile::from_slice(&self.bytes).expect("bytes validated at construction")
    }

    /// The raw byte length of the owned SCUD blob.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// True iff the owned SCUD blob is empty (structurally impossible
    /// for a valid SCUD file; kept as a matter of `Vec`-style API
    /// hygiene).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

/// The parsed outer header of a SCUD file.
///
/// Not part of the public API surface — callers reach for
/// [`ScudFile`]'s accessors instead. Public inside the crate so the
/// writer / reader can share the layout definitions.
#[derive(Debug, Clone, Copy)]
struct ScudHeader<'a> {
    fmt_maj: u16,
    fmt_min: u16,
    flags: ScudFlags,
    cap_id: [u8; 4],
    cldr_version: &'a str,
    locale: Option<&'a str>,
    body: &'a [u8],
}

impl<'a> ScudFile<'a> {
    /// Parse a SCUD file whose bytes live in `'static` storage — the
    /// primary path for language packs.
    ///
    /// This is the shape [`include_bytes!`] emits: a `&'static [u8]`
    /// constant embedded in the binary. The returned view borrows for
    /// `'static`, so the value can be stored in a `static` slot or
    /// handed out through a `'static`-lifetime trait method.
    pub fn from_static(bytes: &'static [u8]) -> Result<ScudFile<'static>, ScudError> {
        ScudFile::from_slice(bytes)
    }

    /// Parse a SCUD file from an arbitrary byte slice.
    ///
    /// The returned view borrows for `'a`, so its lifetime is tied to
    /// the caller-owned buffer.
    pub fn from_slice(bytes: &'a [u8]) -> Result<ScudFile<'a>, ScudError> {
        let header = parse_header(bytes)?;
        Ok(ScudFile { bytes, header })
    }

    /// Read a SCUD file from disk into an owned buffer. Delegates to
    /// [`OwnedScudFile`] under the hood.
    ///
    /// Only available under the `std` feature.
    #[cfg(feature = "std")]
    pub fn open(path: &std::path::Path) -> Result<OwnedScudFile, ScudError> {
        let bytes = std::fs::read(path)?;
        OwnedScudFile::new(bytes)
    }

    /// Wrap an owned `Vec<u8>` as an [`OwnedScudFile`]. Convenience
    /// wrapper around [`OwnedScudFile::new`].
    ///
    /// Only available under the `alloc` feature.
    #[cfg(feature = "alloc")]
    pub fn from_bytes(bytes: Vec<u8>) -> Result<OwnedScudFile, ScudError> {
        OwnedScudFile::new(bytes)
    }

    /// The literal magic bytes at the start of the file. Always
    /// `b"SCUD"` — the loader would have rejected the file otherwise.
    #[must_use]
    pub fn magic(&self) -> [u8; 4] {
        MAGIC
    }

    /// The `(major, minor)` format version pair.
    #[must_use]
    pub fn format_version(&self) -> (u16, u16) {
        (self.header.fmt_maj, self.header.fmt_min)
    }

    /// The file's flags word.
    #[must_use]
    pub fn flags(&self) -> ScudFlags {
        self.header.flags
    }

    /// The four-byte capability tag (e.g. [`CAP_CASE`]).
    #[must_use]
    pub fn capability(&self) -> [u8; 4] {
        self.header.cap_id
    }

    /// The CLDR version string carried in the header (e.g. `"44.1"`).
    #[must_use]
    pub fn cldr_version(&self) -> &'a str {
        self.header.cldr_version
    }

    /// The BCP 47 locale tag carried in the header, if any.
    ///
    /// Returns `None` when the [`ScudFlags::HAS_LOCALE`] bit is clear
    /// — reserved for root-locale packs that ship without a locale
    /// annotation.
    #[must_use]
    pub fn locale(&self) -> Option<&'a str> {
        self.header.locale
    }

    /// The raw body bytes (post-header). Callers who want a typed view
    /// project through [`as_case_data`](Self::as_case_data) or the
    /// forthcoming per-capability accessors.
    #[must_use]
    pub fn body(&self) -> &'a [u8] {
        self.header.body
    }

    /// The total byte length of the SCUD file.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// True iff the SCUD file is zero-length. Never for a valid file
    /// (header parsing would fail); kept for API symmetry.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Project the body as a case-mapping view.
    ///
    /// Returns `Ok(view)` when the file's capability tag is
    /// [`CAP_CASE`]; returns [`ScudError::CapabilityMismatch`]
    /// otherwise. The returned [`CaseDataView`] borrows into this
    /// [`ScudFile`]'s bytes.
    pub fn as_case_data(&self) -> Result<CaseDataView<'a>, ScudError> {
        if self.capability() != CAP_CASE {
            return Err(ScudError::CapabilityMismatch {
                expected: CAP_CASE,
                got: self.capability(),
            });
        }
        CaseDataView::parse(self.body())
    }

    /// Project the body as a collation view.
    ///
    /// Returns `Ok(view)` when the file's capability tag is
    /// [`CAP_COLLATION`]; returns [`ScudError::CapabilityMismatch`]
    /// otherwise. The returned [`CollationDataView`] borrows into
    /// this [`ScudFile`]'s bytes.
    pub fn as_collation_data(&self) -> Result<CollationDataView<'a>, ScudError> {
        if self.capability() != CAP_COLLATION {
            return Err(ScudError::CapabilityMismatch {
                expected: CAP_COLLATION,
                got: self.capability(),
            });
        }
        CollationDataView::parse(self.body())
    }

    /// Project the body as a plural-rules view.
    ///
    /// Returns `Ok(view)` when the file's capability tag is
    /// [`CAP_PLURAL`]; returns [`ScudError::CapabilityMismatch`]
    /// otherwise. The returned [`PluralDataView`] borrows into this
    /// [`ScudFile`]'s bytes.
    pub fn as_plural_data(&self) -> Result<PluralDataView<'a>, ScudError> {
        if self.capability() != CAP_PLURAL {
            return Err(ScudError::CapabilityMismatch {
                expected: CAP_PLURAL,
                got: self.capability(),
            });
        }
        PluralDataView::parse(self.body())
    }

    /// Project the body as a number-formatting view.
    ///
    /// Returns `Ok(view)` when the file's capability tag is
    /// [`CAP_NUMBER`]; returns [`ScudError::CapabilityMismatch`]
    /// otherwise. The returned [`NumberDataView`] borrows into this
    /// [`ScudFile`]'s bytes.
    pub fn as_number_data(&self) -> Result<NumberDataView<'a>, ScudError> {
        if self.capability() != CAP_NUMBER {
            return Err(ScudError::CapabilityMismatch {
                expected: CAP_NUMBER,
                got: self.capability(),
            });
        }
        NumberDataView::parse(self.body())
    }

    /// Project the body as a date/time-formatting view.
    ///
    /// Returns `Ok(view)` when the file's capability tag is
    /// [`CAP_DATETIME`]; returns [`ScudError::CapabilityMismatch`]
    /// otherwise. The returned [`DateTimeDataView`] borrows into this
    /// [`ScudFile`]'s bytes.
    pub fn as_datetime_data(&self) -> Result<DateTimeDataView<'a>, ScudError> {
        if self.capability() != CAP_DATETIME {
            return Err(ScudError::CapabilityMismatch {
                expected: CAP_DATETIME,
                got: self.capability(),
            });
        }
        DateTimeDataView::parse(self.body())
    }

    /// Project the body as a break-iteration view.
    ///
    /// Returns `Ok(view)` when the file's capability tag is
    /// [`CAP_BREAK`]; returns [`ScudError::CapabilityMismatch`]
    /// otherwise. The returned [`BreakDataView`] borrows into this
    /// [`ScudFile`]'s bytes.
    pub fn as_break_data(&self) -> Result<BreakDataView<'a>, ScudError> {
        if self.capability() != CAP_BREAK {
            return Err(ScudError::CapabilityMismatch {
                expected: CAP_BREAK,
                got: self.capability(),
            });
        }
        BreakDataView::parse(self.body())
    }

    /// Project the body as a line-break (UAX #14) view.
    ///
    /// Returns `Ok(view)` when the file's capability tag is
    /// [`CAP_LINEBREAK`]; returns [`ScudError::CapabilityMismatch`]
    /// otherwise. The returned [`LineBreakDataView`] borrows into
    /// this [`ScudFile`]'s bytes.
    pub fn as_linebreak_data(&self) -> Result<LineBreakDataView<'a>, ScudError> {
        if self.capability() != CAP_LINEBREAK {
            return Err(ScudError::CapabilityMismatch {
                expected: CAP_LINEBREAK,
                got: self.capability(),
            });
        }
        LineBreakDataView::parse(self.body())
    }
}

/// Parse the outer file header (magic through the CLDR/locale
/// annotation), returning a [`ScudHeader`] borrowing into `bytes`.
fn parse_header(bytes: &[u8]) -> Result<ScudHeader<'_>, ScudError> {
    if bytes.len() < HEADER_PREFIX_LEN {
        return Err(ScudError::HeaderTruncated);
    }
    let magic = <[u8; 4]>::try_from(&bytes[0..4]).unwrap();
    if magic != MAGIC {
        return Err(ScudError::NotScud);
    }
    let fmt_maj = read_u16(&bytes[4..6]);
    let fmt_min = read_u16(&bytes[6..8]);
    if fmt_maj != SUPPORTED_FORMAT_MAJOR {
        return Err(ScudError::UnsupportedMajorVersion {
            file: fmt_maj,
            supported: SUPPORTED_FORMAT_MAJOR,
        });
    }
    let flags = ScudFlags::from_bits(read_u32(&bytes[8..12]));
    if flags.is_body_compressed() {
        return Err(ScudError::UnsupportedCompression);
    }
    let cap_id = <[u8; 4]>::try_from(&bytes[12..16]).unwrap();
    match cap_id {
        CAP_CASE | CAP_COLLATION | CAP_PLURAL | CAP_NUMBER | CAP_DATETIME | CAP_BREAK
        | CAP_LINEBREAK => {}
        other => return Err(ScudError::UnsupportedCapability { got: other }),
    }
    let header_len = read_u32(&bytes[16..20]) as usize;
    let header_start = HEADER_PREFIX_LEN;
    let header_end = header_start
        .checked_add(header_len)
        .ok_or(ScudError::InvalidHeader)?;
    if header_end > bytes.len() {
        return Err(ScudError::HeaderTruncated);
    }
    let header_bytes = &bytes[header_start..header_end];
    let (cldr_version, locale) = parse_header_annotations(header_bytes, flags)?;
    let body = &bytes[header_end..];
    Ok(ScudHeader {
        fmt_maj,
        fmt_min,
        flags,
        cap_id,
        cldr_version,
        locale,
        body,
    })
}

fn parse_header_annotations(
    hdr: &[u8],
    flags: ScudFlags,
) -> Result<(&str, Option<&str>), ScudError> {
    let (cldr, rest) = read_length_prefixed_str(hdr)?;
    if flags.bits() & ScudFlags::HAS_LOCALE != 0 {
        let (loc, _tail) = read_length_prefixed_str(rest)?;
        Ok((cldr, Some(loc)))
    } else {
        Ok((cldr, None))
    }
}

fn read_length_prefixed_str(bytes: &[u8]) -> Result<(&str, &[u8]), ScudError> {
    if bytes.len() < 4 {
        return Err(ScudError::InvalidHeader);
    }
    let len = read_u32(&bytes[0..4]) as usize;
    let start = 4usize;
    let end = start.checked_add(len).ok_or(ScudError::InvalidHeader)?;
    if end > bytes.len() {
        return Err(ScudError::InvalidHeader);
    }
    let s = core::str::from_utf8(&bytes[start..end]).map_err(|_| ScudError::InvalidUtf8)?;
    Ok((s, &bytes[end..]))
}

/// Read a little-endian `u16` from a two-byte slice.
///
/// # Panics
/// If `bytes.len() < 2` — a caller-side invariant checked at every
/// call site in this crate before reaching this helper.
fn read_u16(bytes: &[u8]) -> u16 {
    u16::from_le_bytes([bytes[0], bytes[1]])
}

/// Read a little-endian `u32` from a four-byte slice.
///
/// # Panics
/// If `bytes.len() < 4`.
fn read_u32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

// -----------------------------------------------------------------------
// Section framing
// -----------------------------------------------------------------------

/// A capability body's sequence of `(section-id, length, bytes)`
/// frames.
///
/// Each capability defines a set of four-byte section ids the reader
/// looks up by name. New sections can be added in a
/// backwards-compatible way — an unknown section id simply doesn't
/// contribute data, and the reader skips over it by consulting the
/// preceding length.
///
/// # Frame layout
///
/// ```text
///    0     4     section-id     [u8; 4]
///    4     4     length         u32 le
///    8     len   bytes          [u8; len]
/// ```
///
/// The sequence is terminated by the byte-slice's end; there is no
/// framing sentinel.
#[derive(Debug, Clone, Copy)]
pub struct SectionReader<'a> {
    remaining: &'a [u8],
}

/// Payload of a decoded section frame — `(section-id, section-bytes,
/// trailing-bytes)`.
type SectionFrame<'a> = ([u8; 4], &'a [u8], &'a [u8]);

impl<'a> SectionReader<'a> {
    /// Wrap a body byte-slice as a section reader.
    #[must_use]
    pub fn new(body: &'a [u8]) -> Self {
        Self { remaining: body }
    }

    /// Find the first section with the given id, returning its bytes.
    ///
    /// Section ids are expected to be unique within a body; a
    /// duplicate is not an error but the first match wins.
    pub fn find(&self, id: [u8; 4]) -> Result<Option<&'a [u8]>, ScudError> {
        let mut cursor = self.remaining;
        while !cursor.is_empty() {
            let (frame_id, payload, rest) = read_section_frame(cursor)?;
            if frame_id == id {
                return Ok(Some(payload));
            }
            cursor = rest;
        }
        Ok(None)
    }

    /// Iterate every section as `(id, bytes)`.
    ///
    /// Errors surface once and terminate iteration; a corrupt file
    /// yields the good frames read so far followed by `None`.
    ///
    /// Equivalent to `(&reader).into_iter()`; provided as an inherent
    /// method so callers do not have to reach for [`IntoIterator`].
    #[must_use]
    pub fn iter(&self) -> SectionIter<'a> {
        SectionIter {
            remaining: self.remaining,
        }
    }
}

impl<'a> IntoIterator for &SectionReader<'a> {
    type Item = ([u8; 4], &'a [u8]);
    type IntoIter = SectionIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        SectionIter {
            remaining: self.remaining,
        }
    }
}

/// Iterator adapter over a [`SectionReader`]'s frames.
///
/// Cloneable but *not* `Copy` — a `Copy` iterator would let a caller
/// silently duplicate iteration state, which clippy flags as an
/// error-prone pattern.
#[derive(Debug, Clone)]
pub struct SectionIter<'a> {
    remaining: &'a [u8],
}

impl<'a> Iterator for SectionIter<'a> {
    type Item = ([u8; 4], &'a [u8]);
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.is_empty() {
            return None;
        }
        if let Ok((id, payload, rest)) = read_section_frame(self.remaining) {
            self.remaining = rest;
            Some((id, payload))
        } else {
            self.remaining = &[];
            None
        }
    }
}

fn read_section_frame(bytes: &[u8]) -> Result<SectionFrame<'_>, ScudError> {
    if bytes.len() < 8 {
        return Err(ScudError::InvalidSection);
    }
    let id = <[u8; 4]>::try_from(&bytes[0..4]).unwrap();
    let len = read_u32(&bytes[4..8]) as usize;
    let start = 8usize;
    let end = start.checked_add(len).ok_or(ScudError::InvalidSection)?;
    if end > bytes.len() {
        return Err(ScudError::BodyTruncated);
    }
    Ok((id, &bytes[start..end], &bytes[end..]))
}

// -----------------------------------------------------------------------
// Case-mapping view
// -----------------------------------------------------------------------

/// Section id for the simple lowercase table (single scalar → single
/// scalar). Bytes `s` `L` `w` `r`.
pub const SECT_SIMPLE_LOWER: [u8; 4] = *b"sLwr";
/// Section id for the simple uppercase table.
pub const SECT_SIMPLE_UPPER: [u8; 4] = *b"sUpr";
/// Section id for the simple case-fold table.
pub const SECT_SIMPLE_FOLD: [u8; 4] = *b"sFld";
/// Section id for the *full* uppercase expansion table (single scalar
/// → up to N scalars — the `ß → SS` shape).
pub const SECT_FULL_UPPER: [u8; 4] = *b"fUpr";
/// Section id for the full lowercase expansion table (rare; kept for
/// symmetry with [`SECT_FULL_UPPER`]).
pub const SECT_FULL_LOWER: [u8; 4] = *b"fLwr";
/// Section id for the full case-fold expansion table (`ß → ss`).
pub const SECT_FULL_FOLD: [u8; 4] = *b"fFld";
/// Section id for the locale-tailored context table.
///
/// Each entry is `(from, kind, to)` where `kind` picks a context
/// selector understood by the algorithm crate (e.g. `0 =
/// unconditional locale override`, `1 = final-sigma`).
pub const SECT_CONTEXT: [u8; 4] = *b"Ctx0";

/// Zero-copy view into a SCUD file's case-mapping body.
///
/// Every accessor is `O(log n)` — the simple tables are binary-searched
/// by source scalar. Callers cache lookups per input if they need
/// microsecond-scale throughput.
///
/// The view carries the parsed section byte-slices; each lookup
/// re-decodes the fixed-width record at the matched offset.
#[derive(Debug, Clone, Copy)]
pub struct CaseDataView<'a> {
    /// Sorted list of `(u32 src, u32 dst)` pairs — the simple
    /// lowercase table.
    simple_lower: &'a [u8],
    /// Sorted list of `(u32 src, u32 dst)` pairs — the simple
    /// uppercase table.
    simple_upper: &'a [u8],
    /// Sorted list of `(u32 src, u32 dst)` pairs — the simple
    /// case-fold table.
    simple_fold: &'a [u8],
    /// Sorted list of `(u32 src, u8 n, [u32; n])` records — full
    /// uppercase.
    full_upper: &'a [u8],
    /// Sorted list of `(u32 src, u8 n, [u32; n])` records — full
    /// lowercase.
    full_lower: &'a [u8],
    /// Sorted list of `(u32 src, u8 n, [u32; n])` records — full
    /// case-fold.
    full_fold: &'a [u8],
    /// Sorted list of `(u32 src, u8 kind, u32 to)` — contextual
    /// tailorings.
    context: &'a [u8],
}

/// A contextual case-mapping selector.
///
/// Each contextual entry pairs a `from` scalar with one of these
/// selectors to disambiguate which of several mappings applies. The
/// algorithm crate consults the selector at query time.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ContextKind {
    /// The mapping applies whenever the current locale matches the
    /// SCUD pack's locale (unconditional locale override, e.g.
    /// Turkish `I → ı`).
    LocaleOverrideLower = 0,
    /// The mapping applies to uppercasing under the current locale
    /// (Turkish `i → İ`).
    LocaleOverrideUpper = 1,
    /// Greek final sigma — the mapping applies when the scalar is at
    /// a word-final position under the target locale's rules.
    FinalSigma = 2,
}

impl ContextKind {
    /// Round-trip a `u8` context kind back into the typed enum.
    ///
    /// Returns `None` for unknown values so a forward-compatible SCUD
    /// pack (a newer minor version that defined a new kind) doesn't
    /// panic in an older loader.
    #[must_use]
    pub fn from_u8(kind: u8) -> Option<Self> {
        match kind {
            0 => Some(Self::LocaleOverrideLower),
            1 => Some(Self::LocaleOverrideUpper),
            2 => Some(Self::FinalSigma),
            _ => None,
        }
    }
}

impl<'a> CaseDataView<'a> {
    /// Parse a case-mapping body into a section-projected view.
    fn parse(body: &'a [u8]) -> Result<Self, ScudError> {
        let reader = SectionReader::new(body);
        Ok(Self {
            simple_lower: reader.find(SECT_SIMPLE_LOWER)?.unwrap_or(&[]),
            simple_upper: reader.find(SECT_SIMPLE_UPPER)?.unwrap_or(&[]),
            simple_fold: reader.find(SECT_SIMPLE_FOLD)?.unwrap_or(&[]),
            full_upper: reader.find(SECT_FULL_UPPER)?.unwrap_or(&[]),
            full_lower: reader.find(SECT_FULL_LOWER)?.unwrap_or(&[]),
            full_fold: reader.find(SECT_FULL_FOLD)?.unwrap_or(&[]),
            context: reader.find(SECT_CONTEXT)?.unwrap_or(&[]),
        })
    }

    /// Look up the simple lowercase mapping for `src` (a Unicode
    /// scalar's `u32` value). Returns `None` when the source has no
    /// tailored mapping in this pack.
    #[must_use]
    pub fn simple_lower(&self, src: u32) -> Option<u32> {
        binary_search_pair(self.simple_lower, src)
    }

    /// Look up the simple uppercase mapping for `src`.
    #[must_use]
    pub fn simple_upper(&self, src: u32) -> Option<u32> {
        binary_search_pair(self.simple_upper, src)
    }

    /// Look up the simple case-fold mapping for `src`.
    #[must_use]
    pub fn simple_fold(&self, src: u32) -> Option<u32> {
        binary_search_pair(self.simple_fold, src)
    }

    /// Look up the full uppercase mapping for `src`. Returns a slice
    /// of `(u32, ...)` scalars.
    ///
    /// The returned slice's lifetime is tied to the underlying pack;
    /// callers copy its contents if they need owned storage.
    ///
    /// The scalars are packed into a temporary fixed buffer to avoid
    /// borrow-checker friction with the byte-oriented table; callers
    /// receive a stack-allocated array plus its length.
    #[must_use]
    pub fn full_upper(&self, src: u32) -> Option<FullMapping> {
        binary_search_full(self.full_upper, src)
    }

    /// Look up the full lowercase mapping for `src`.
    #[must_use]
    pub fn full_lower(&self, src: u32) -> Option<FullMapping> {
        binary_search_full(self.full_lower, src)
    }

    /// Look up the full case-fold mapping for `src`.
    #[must_use]
    pub fn full_fold(&self, src: u32) -> Option<FullMapping> {
        binary_search_full(self.full_fold, src)
    }

    /// Enumerate every contextual mapping in the pack whose source
    /// matches `src`.
    ///
    /// Contextual entries are keyed by `src` and disambiguated by a
    /// [`ContextKind`]; a pack may carry multiple entries for the
    /// same `src` (e.g. Turkish carries `I → ı` under
    /// [`ContextKind::LocaleOverrideLower`] plus `i → İ` under
    /// [`ContextKind::LocaleOverrideUpper`]).
    pub fn contextual(&self, src: u32) -> ContextIter<'a> {
        ContextIter {
            remaining: self.context,
            src,
        }
    }
}

/// A full-mapping payload — up to eight scalars produced by a single
/// input scalar's mapping.
///
/// The Unicode `SpecialCasing.txt` table's longest expansion is three
/// scalars; a bound of eight is generous to accommodate any future
/// extension without requiring a `Vec` on the query path.
#[derive(Debug, Clone, Copy)]
pub struct FullMapping {
    buf: [u32; 8],
    len: u8,
}

impl FullMapping {
    /// The mapped scalars, in emission order.
    #[must_use]
    pub fn as_slice(&self) -> &[u32] {
        &self.buf[..usize::from(self.len)]
    }

    /// Iterate the mapped scalars as `char` values.
    ///
    /// Silently skips a scalar that is not a valid `char`
    /// (`0xD800..=0xDFFF` or `> 0x10FFFF`) — SCUD writers should never
    /// emit those, but the reader is defensive.
    pub fn chars(&self) -> impl Iterator<Item = char> + '_ {
        self.as_slice().iter().filter_map(|&s| char::from_u32(s))
    }
}

/// Iterator over the contextual mappings whose source scalar matches
/// a query. Yields `(ContextKind, dst-scalar)` pairs.
///
/// Cloneable but *not* `Copy` — same reasoning as [`SectionIter`].
#[derive(Debug, Clone)]
pub struct ContextIter<'a> {
    remaining: &'a [u8],
    src: u32,
}

impl Iterator for ContextIter<'_> {
    type Item = (ContextKind, u32);
    fn next(&mut self) -> Option<Self::Item> {
        // Records are (u32 src, u8 kind, u32 to) = 9 bytes each.
        while self.remaining.len() >= 9 {
            let entry_src = read_u32(&self.remaining[0..4]);
            let kind_raw = self.remaining[4];
            let entry_to = read_u32(&self.remaining[5..9]);
            self.remaining = &self.remaining[9..];
            if entry_src == self.src {
                if let Some(kind) = ContextKind::from_u8(kind_raw) {
                    return Some((kind, entry_to));
                }
            }
        }
        None
    }
}

/// Binary-search a byte slice of `(u32 src, u32 dst)` pairs for `src`.
/// Returns the matching `dst`, or `None` if no entry matches.
fn binary_search_pair(bytes: &[u8], src: u32) -> Option<u32> {
    if !bytes.len().is_multiple_of(8) {
        return None;
    }
    let n = bytes.len() / 8;
    let mut lo = 0;
    let mut hi = n;
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let entry_src = read_u32(&bytes[mid * 8..mid * 8 + 4]);
        match entry_src.cmp(&src) {
            core::cmp::Ordering::Less => lo = mid + 1,
            core::cmp::Ordering::Greater => hi = mid,
            core::cmp::Ordering::Equal => {
                return Some(read_u32(&bytes[mid * 8 + 4..mid * 8 + 8]));
            }
        }
    }
    None
}

/// Binary-search a byte slice of `(u32 src, u8 n, [u32; n])` records.
///
/// Because records are variable-length, the table is laid out with a
/// fixed 8-byte-per-entry *index* prefix followed by the payload
/// region. On-disk shape:
///
/// ```text
///    0     4     count           u32 le
///    4     ..    [(u32 src, u32 payload_offset); count]  // sorted by src
///    ..    ..    payload region: [u8 n, u32 s0, u32 s1, ...]* per record
/// ```
///
/// The payload region starts immediately after the index.
fn binary_search_full(bytes: &[u8], src: u32) -> Option<FullMapping> {
    if bytes.len() < 4 {
        return None;
    }
    let count = read_u32(&bytes[0..4]) as usize;
    let index_start = 4usize;
    let index_end = index_start.checked_add(count.checked_mul(8)?)?;
    if index_end > bytes.len() {
        return None;
    }
    let index = &bytes[index_start..index_end];
    let payloads = &bytes[index_end..];

    let mut lo = 0;
    let mut hi = count;
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let entry_src = read_u32(&index[mid * 8..mid * 8 + 4]);
        match entry_src.cmp(&src) {
            core::cmp::Ordering::Less => lo = mid + 1,
            core::cmp::Ordering::Greater => hi = mid,
            core::cmp::Ordering::Equal => {
                let off = read_u32(&index[mid * 8 + 4..mid * 8 + 8]) as usize;
                return decode_full_record(payloads, off);
            }
        }
    }
    None
}

fn decode_full_record(payloads: &[u8], off: usize) -> Option<FullMapping> {
    if off >= payloads.len() {
        return None;
    }
    let n = payloads[off] as usize;
    if n == 0 || n > 8 {
        return None;
    }
    let start = off + 1;
    let end = start.checked_add(n.checked_mul(4)?)?;
    if end > payloads.len() {
        return None;
    }
    let mut buf = [0u32; 8];
    for i in 0..n {
        buf[i] = read_u32(&payloads[start + i * 4..start + i * 4 + 4]);
    }
    // `n` was bounded to <= 8 immediately above, so the cast to
    // `u8` is exact — clippy doesn't see the guard, so allow the
    // truncation lint at this call site only.
    #[allow(clippy::cast_possible_truncation)]
    let len_u8 = n as u8;
    Some(FullMapping { buf, len: len_u8 })
}

// -----------------------------------------------------------------------
// Collation view
// -----------------------------------------------------------------------

/// Section id for the collation-tailoring **expansion** table
/// (single-scalar → up-to-N-scalar replacements applied to both
/// operands before UCA comparison). Bytes `E` `x` `p` `n`.
///
/// The exact wire shape mirrors [`SECT_FULL_UPPER`] — a 4-byte count
/// followed by `(u32 src, u32 payload_offset)` index entries in
/// ascending `src` order, followed by a packed payload region where
/// each record is `u8 n, [u32; n]`. Entries with `n == 0` are
/// forbidden by the writer.
pub const SECT_EXPANSIONS: [u8; 4] = *b"Expn";

/// Section id for a compact collation-options blob carried in the
/// pack header. Bytes `O` `p` `t` `0`.
///
/// The layout is a fixed 4-byte record: `u8 default_strength, u8
/// case_insensitive, u8 backwards_secondary, u8 reserved`.
/// `default_strength` values map onto the wire encoding of the
/// collation strength enum (0 = Primary, 1 = Secondary,
/// 2 = Tertiary, 3 = Quaternary, 4 = Identical). Unknown / absent →
/// the algorithm's compile-time default (Tertiary).
/// `backwards_secondary` is a bool (0 = false, 1 = true) used by the
/// French backwards-secondary tailoring — when true, the collation
/// engine reverses the secondary-weight comparison so accents
/// tie-break right-to-left within a word.
pub const SECT_COLLATION_OPTIONS: [u8; 4] = *b"Opt0";

/// Section id for a per-locale primary-weight override table.
/// Bytes `P` `r` `i` `W`.
///
/// The wire shape is a fixed 16-byte record per entry:
/// `u32 cp, u32 primary_weight, u32 secondary_weight,
/// u32 tertiary_weight`, sorted by `cp`, prefixed with a `u32 count`.
///
/// A pack ships primary-override rows to tailor UCA weights for a
/// specific set of code points. The canonical use is Turkish's
/// primary-distinct dotless-ı (U+0131) ordering — CLDR Turkish places
/// `ı` between `h` and `i` at the primary level while default UCA
/// treats them as primary-equal, tertiary-distinct.
///
/// The [`stringcheese-icu-collation`](../../../stringcheese-icu-collation)
/// engine consults this table at compare-time before falling back to
/// the UCA weight table (feruca / DUCET root). When at least one
/// override row is present, the engine switches to a
/// weight-tuple-based comparator that ranks characters by their
/// override entry — chars without an override use the ASCII-lowercased
/// codepoint as an approximation.
pub const SECT_PRIMARY_OVERRIDES: [u8; 4] = *b"PriW";

/// Zero-copy view into a SCUD file's collation body.
///
/// Every accessor is `O(log n)` — the expansion table is
/// binary-searched by source scalar. The view carries the parsed
/// section byte-slices; each lookup re-decodes the fixed-width
/// record at the matched offset.
///
/// The Phase 2 wire format is deliberately minimal: one
/// character-expansion table (used to encode DE-phonebook
/// `ä → ae`, both DE variants' `ß → ss`, and future locale
/// tailorings) plus a 4-byte options blob carrying the pack's
/// default strength. Later phases can add DUCET-root-reweight
/// sections without touching the loader — a new section id ships
/// alongside its consumer.
#[derive(Debug, Clone, Copy)]
pub struct CollationDataView<'a> {
    /// Sorted list of `(u32 src, u8 n, [u32; n])` records — the
    /// character-expansion table.
    expansions: &'a [u8],
    /// Optional 4-byte options blob (see [`SECT_COLLATION_OPTIONS`]).
    /// Empty when the pack ships no options blob (algorithm defaults
    /// apply).
    options: &'a [u8],
    /// Optional primary-weight override table (see
    /// [`SECT_PRIMARY_OVERRIDES`]). Sorted list of `(u32 cp, u32 pw,
    /// u32 sw, u32 tw)` records. Empty when the pack ships no
    /// primary-override rows.
    primary_overrides: &'a [u8],
}

impl<'a> CollationDataView<'a> {
    /// Parse a collation body into a section-projected view.
    fn parse(body: &'a [u8]) -> Result<Self, ScudError> {
        let reader = SectionReader::new(body);
        Ok(Self {
            expansions: reader.find(SECT_EXPANSIONS)?.unwrap_or(&[]),
            options: reader.find(SECT_COLLATION_OPTIONS)?.unwrap_or(&[]),
            primary_overrides: reader.find(SECT_PRIMARY_OVERRIDES)?.unwrap_or(&[]),
        })
    }

    /// Look up the character-expansion for `src`. Returns a
    /// [`FullMapping`] of one or more replacement scalars, or `None`
    /// when this pack has no expansion for the source.
    #[must_use]
    pub fn expansion(&self, src: u32) -> Option<FullMapping> {
        binary_search_full(self.expansions, src)
    }

    /// True iff the pack carries at least one character expansion.
    ///
    /// A caller consulting the pack for pre-normalization can skip
    /// the expansion walk when this is false, saving one binary
    /// search per input scalar.
    #[must_use]
    pub fn has_expansions(&self) -> bool {
        // Layout: u32 count followed by index entries; empty when
        // `expansions` is under 4 bytes or its count field is 0.
        if self.expansions.len() < 4 {
            return false;
        }
        read_u32(&self.expansions[0..4]) != 0
    }

    /// Number of expansion entries in the pack. Useful for reporting
    /// pack coverage in test logs.
    #[must_use]
    pub fn expansion_count(&self) -> usize {
        if self.expansions.len() < 4 {
            return 0;
        }
        read_u32(&self.expansions[0..4]) as usize
    }

    /// The pack's default collation strength, if any. Returns `None`
    /// when the pack ships no options blob; callers apply their own
    /// compile-time default (typically Tertiary).
    #[must_use]
    pub fn default_strength(&self) -> Option<u8> {
        if self.options.len() < 4 {
            return None;
        }
        Some(self.options[0])
    }

    /// The pack's case-insensitive flag, if the options blob carries
    /// one. Interpretation is caller-side (typically applied only at
    /// Tertiary strength or below).
    #[must_use]
    pub fn case_insensitive(&self) -> Option<bool> {
        if self.options.len() < 4 {
            return None;
        }
        Some(self.options[1] != 0)
    }

    /// The pack's backwards-secondary flag, if the options blob
    /// carries one. When true, the collation engine reverses the
    /// secondary-weight comparison so accents tie-break
    /// right-to-left within a word. Used by French collation.
    #[must_use]
    pub fn backwards_secondary(&self) -> Option<bool> {
        if self.options.len() < 4 {
            return None;
        }
        Some(self.options[2] != 0)
    }

    /// Look up the primary-weight override for `cp`. Returns
    /// `Some((primary, secondary, tertiary))` when the pack ships an
    /// override row for the code point, `None` otherwise.
    ///
    /// See [`SECT_PRIMARY_OVERRIDES`] for the wire format and
    /// intended interpretation.
    #[must_use]
    pub fn primary_override(&self, cp: u32) -> Option<(u32, u32, u32)> {
        binary_search_primary_override(self.primary_overrides, cp)
    }

    /// True iff the pack carries at least one primary-weight
    /// override row.
    #[must_use]
    pub fn has_primary_overrides(&self) -> bool {
        if self.primary_overrides.len() < 4 {
            return false;
        }
        read_u32(&self.primary_overrides[0..4]) != 0
    }

    /// Number of primary-override entries in the pack. Useful for
    /// reporting pack coverage in test logs.
    #[must_use]
    pub fn primary_override_count(&self) -> usize {
        if self.primary_overrides.len() < 4 {
            return 0;
        }
        read_u32(&self.primary_overrides[0..4]) as usize
    }
}

/// Binary-search the primary-override table for `cp`. Returns
/// `Some((primary, secondary, tertiary))` when found.
fn binary_search_primary_override(bytes: &[u8], cp: u32) -> Option<(u32, u32, u32)> {
    if bytes.len() < 4 {
        return None;
    }
    let count = read_u32(&bytes[0..4]) as usize;
    // Each record is 16 bytes (cp + 3 * u32 weights).
    let expected_len = 4usize.checked_add(count.checked_mul(16)?)?;
    if bytes.len() < expected_len {
        return None;
    }
    let records = &bytes[4..expected_len];
    let mut lo = 0;
    let mut hi = count;
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let entry_cp = read_u32(&records[mid * 16..mid * 16 + 4]);
        match entry_cp.cmp(&cp) {
            core::cmp::Ordering::Less => lo = mid + 1,
            core::cmp::Ordering::Greater => hi = mid,
            core::cmp::Ordering::Equal => {
                let pw = read_u32(&records[mid * 16 + 4..mid * 16 + 8]);
                let sw = read_u32(&records[mid * 16 + 8..mid * 16 + 12]);
                let tw = read_u32(&records[mid * 16 + 12..mid * 16 + 16]);
                return Some((pw, sw, tw));
            }
        }
    }
    None
}

// -----------------------------------------------------------------------
// Plural-rules view
// -----------------------------------------------------------------------

/// Section id for the cardinal plural-rules table. Bytes `P` `l` `C` `a`.
///
/// Wire layout: a `u16 count` prefix followed by `count` fixed-width
/// `(u8 category, u8 rule_id)` entries in the order the runtime should
/// evaluate them. Rule ids are opaque to the loader — the algorithm
/// crate interprets them against a hand-encoded predicate table.
/// See [`PluralCategory`] for the category encoding.
pub const SECT_CARDINAL_RULES: [u8; 4] = *b"PlCa";
/// Section id for the ordinal plural-rules table. Same wire layout as
/// [`SECT_CARDINAL_RULES`]. Bytes `P` `l` `O` `r`.
pub const SECT_ORDINAL_RULES: [u8; 4] = *b"PlOr";

/// CLDR plural categories per UTS #35 § 5. The wire encoding matches
/// the order in which CLDR lists them so a `u8` round-trips through
/// [`PluralCategory::from_u8`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PluralCategory {
    /// The `zero` category — used by Arabic, Latvian, and a handful of
    /// other locales for `n == 0`.
    Zero = 0,
    /// The `one` category — the singular in most European languages.
    One = 1,
    /// The `two` category — used by Arabic, Hebrew, Welsh, etc.
    Two = 2,
    /// The `few` category — Slavic languages typically use this for
    /// small-count paucals.
    Few = 3,
    /// The `many` category — used by French for large numbers and by
    /// Polish/Russian/etc. for the "many" bucket.
    Many = 4,
    /// The `other` category — the fallback bucket every locale
    /// defines.
    Other = 5,
}

impl PluralCategory {
    /// Round-trip a `u8` category back into the typed enum.
    #[must_use]
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Zero),
            1 => Some(Self::One),
            2 => Some(Self::Two),
            3 => Some(Self::Few),
            4 => Some(Self::Many),
            5 => Some(Self::Other),
            _ => None,
        }
    }

    /// The wire-encoded `u8` value for this category.
    #[must_use]
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// The CLDR string name of this category (`"zero"`, `"one"`,
    /// `"two"`, `"few"`, `"many"`, `"other"`).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Zero => "zero",
            Self::One => "one",
            Self::Two => "two",
            Self::Few => "few",
            Self::Many => "many",
            Self::Other => "other",
        }
    }
}

/// Zero-copy view into a SCUD file's plural-rules body.
///
/// Every accessor is `O(1)` — cardinal and ordinal tables are looked
/// up by iterating their small entry list (at most a few entries per
/// locale). The runtime interprets each `(category, rule_id)` pair
/// against a hand-encoded predicate table; the wire format keeps
/// SCUD itself independent of the predicate implementation so a
/// future format change to richer rule expressions is
/// backwards-compatible.
#[derive(Debug, Clone, Copy)]
pub struct PluralDataView<'a> {
    /// Ordered `(category, rule_id)` entries for cardinals.
    cardinals: &'a [u8],
    /// Ordered `(category, rule_id)` entries for ordinals.
    ordinals: &'a [u8],
}

impl<'a> PluralDataView<'a> {
    /// Parse a plural body into a section-projected view.
    fn parse(body: &'a [u8]) -> Result<Self, ScudError> {
        let reader = SectionReader::new(body);
        Ok(Self {
            cardinals: reader.find(SECT_CARDINAL_RULES)?.unwrap_or(&[]),
            ordinals: reader.find(SECT_ORDINAL_RULES)?.unwrap_or(&[]),
        })
    }

    /// Iterate cardinal `(category, rule_id)` pairs in evaluation
    /// order.
    ///
    /// The caller evaluates each rule against the operand tuple; the
    /// first rule whose predicate matches wins. If no rule matches,
    /// the caller returns [`PluralCategory::Other`].
    pub fn cardinal_rules(&self) -> PluralRuleIter<'a> {
        PluralRuleIter::from_section(self.cardinals)
    }

    /// Iterate ordinal `(category, rule_id)` pairs.
    pub fn ordinal_rules(&self) -> PluralRuleIter<'a> {
        PluralRuleIter::from_section(self.ordinals)
    }

    /// True iff the pack carries at least one cardinal rule.
    #[must_use]
    pub fn has_cardinal_rules(&self) -> bool {
        self.cardinals.len() > 2
    }

    /// True iff the pack carries at least one ordinal rule.
    #[must_use]
    pub fn has_ordinal_rules(&self) -> bool {
        self.ordinals.len() > 2
    }
}

/// Iterator over the `(category, rule_id)` pairs in a plural-rules
/// table. Cloneable but not `Copy` — same reasoning as
/// [`SectionIter`].
#[derive(Debug, Clone)]
pub struct PluralRuleIter<'a> {
    remaining: &'a [u8],
}

impl<'a> PluralRuleIter<'a> {
    /// Wrap a section body (with its `u16 count` prefix) as an
    /// iterator over its `(category, rule_id)` pairs.
    fn from_section(bytes: &'a [u8]) -> Self {
        // Skip the count prefix; the iterator drives from the
        // remaining `(u8, u8)` pairs.
        let remaining = if bytes.len() >= 2 { &bytes[2..] } else { &[] };
        Self { remaining }
    }
}

impl Iterator for PluralRuleIter<'_> {
    type Item = (PluralCategory, u8);
    fn next(&mut self) -> Option<Self::Item> {
        while self.remaining.len() >= 2 {
            let cat_raw = self.remaining[0];
            let rule_id = self.remaining[1];
            self.remaining = &self.remaining[2..];
            if let Some(cat) = PluralCategory::from_u8(cat_raw) {
                return Some((cat, rule_id));
            }
        }
        None
    }
}

// -----------------------------------------------------------------------
// Number-formatting view
// -----------------------------------------------------------------------

/// Section id for the decimal formatting patterns table. Bytes `N` `m` `D` `p`.
///
/// Wire layout: a fixed 8-byte record of `(u8 group_separator_len,
/// [u8; up to 4] group_separator, u8 decimal_separator_len, [u8; up
/// to 4] decimal_separator, u8 min_fraction, u8 max_fraction, u8
/// primary_grouping, u8 secondary_grouping)`. The separators are
/// UTF-8 bytes (up to 4 each) so a wide code point like NBSP (2 UTF-8
/// bytes) or FIGURE SPACE (3 UTF-8 bytes) fits. Practically every
/// CLDR default separator is 1-3 UTF-8 bytes.
pub const SECT_DECIMAL_PATTERN: [u8; 4] = *b"NmDp";

/// Section id for the currency-symbol / format table. Bytes `N` `m` `C` `y`.
///
/// Wire layout: a `u16 count` prefix followed by `count` records
/// of `(u8 code_len, [u8; code_len] iso_code, u8 symbol_len, [u8;
/// symbol_len] symbol_utf8, u8 pattern_flags)`. `pattern_flags`
/// encodes the currency-symbol placement (bit 0: 1 = after value; 0 =
/// before) and the whether a space separates them (bit 1: 1 = yes;
/// 0 = no).
pub const SECT_CURRENCY_TABLE: [u8; 4] = *b"NmCy";

/// Section id for the percent-format pattern. Bytes `N` `m` `P` `c`.
///
/// Wire layout: `(u8 symbol_len, [u8; symbol_len] symbol_utf8, u8
/// pattern_flags)` where `pattern_flags` encodes placement (bit 0)
/// and space (bit 1) like [`SECT_CURRENCY_TABLE`].
pub const SECT_PERCENT_PATTERN: [u8; 4] = *b"NmPc";

/// Zero-copy view into a SCUD file's number-formatting body.
///
/// Every accessor decodes a fixed-shape record on demand from the
/// underlying section bytes.
#[derive(Debug, Clone, Copy)]
pub struct NumberDataView<'a> {
    decimal: &'a [u8],
    currency: &'a [u8],
    percent: &'a [u8],
}

/// A decoded decimal formatting pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecimalPattern<'a> {
    /// Group (thousands) separator, UTF-8 encoded.
    pub group_separator: &'a str,
    /// Decimal separator, UTF-8 encoded.
    pub decimal_separator: &'a str,
    /// Minimum fraction digits to render.
    pub min_fraction: u8,
    /// Maximum fraction digits to render.
    pub max_fraction: u8,
    /// Primary grouping (digits between the decimal point and the
    /// first group separator). Typically 3.
    pub primary_grouping: u8,
    /// Secondary grouping (digits between subsequent separators).
    /// Typically 3; 2 in Indian numbering.
    pub secondary_grouping: u8,
}

/// A decoded currency-symbol record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurrencyRecord<'a> {
    /// ISO 4217 currency code (`"USD"`, `"EUR"`, `"GBP"`).
    pub iso_code: &'a str,
    /// The localised currency symbol, UTF-8 encoded.
    pub symbol: &'a str,
    /// True when the symbol appears after the value (`"1,00 €"`
    /// rather than `"$1.00"`).
    pub symbol_after: bool,
    /// True when a space separates the value from the symbol.
    pub symbol_spaced: bool,
}

/// A decoded percent-format record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PercentPattern<'a> {
    /// The percent symbol, UTF-8 encoded (`"%"` in most locales;
    /// `"٪"` for the Arabic-Indic system).
    pub symbol: &'a str,
    /// True when the symbol appears after the value.
    pub symbol_after: bool,
    /// True when a space separates the value from the symbol.
    pub symbol_spaced: bool,
}

impl<'a> NumberDataView<'a> {
    /// Parse a number body into a section-projected view.
    fn parse(body: &'a [u8]) -> Result<Self, ScudError> {
        let reader = SectionReader::new(body);
        Ok(Self {
            decimal: reader.find(SECT_DECIMAL_PATTERN)?.unwrap_or(&[]),
            currency: reader.find(SECT_CURRENCY_TABLE)?.unwrap_or(&[]),
            percent: reader.find(SECT_PERCENT_PATTERN)?.unwrap_or(&[]),
        })
    }

    /// Decode the pack's decimal formatting pattern. Returns `None`
    /// if the pack ships no decimal pattern.
    #[must_use]
    pub fn decimal_pattern(&self) -> Option<DecimalPattern<'a>> {
        let bytes = self.decimal;
        let (group, rest) = read_len_prefixed_str_u8(bytes)?;
        let (decimal, rest) = read_len_prefixed_str_u8(rest)?;
        if rest.len() < 4 {
            return None;
        }
        Some(DecimalPattern {
            group_separator: group,
            decimal_separator: decimal,
            min_fraction: rest[0],
            max_fraction: rest[1],
            primary_grouping: rest[2],
            secondary_grouping: rest[3],
        })
    }

    /// Look up a currency record by ISO 4217 code (`"USD"`, `"EUR"`,
    /// `"JPY"`). Returns `None` when the pack ships no entry.
    #[must_use]
    pub fn currency(&self, iso_code: &str) -> Option<CurrencyRecord<'a>> {
        self.iter_currencies()
            .find(|record| record.iso_code == iso_code)
    }

    /// Iterate every currency record in the pack.
    pub fn iter_currencies(&self) -> CurrencyIter<'a> {
        // Skip the u16 count prefix; the iterator drives from the
        // remaining bytes.
        let remaining = if self.currency.len() >= 2 {
            &self.currency[2..]
        } else {
            &[]
        };
        CurrencyIter { remaining }
    }

    /// Decode the pack's percent-format pattern.
    #[must_use]
    pub fn percent_pattern(&self) -> Option<PercentPattern<'a>> {
        let bytes = self.percent;
        let (symbol, rest) = read_len_prefixed_str_u8(bytes)?;
        if rest.is_empty() {
            return None;
        }
        let flags = rest[0];
        Some(PercentPattern {
            symbol,
            symbol_after: (flags & 0x01) != 0,
            symbol_spaced: (flags & 0x02) != 0,
        })
    }
}

/// Iterator over the currency records in a [`NumberDataView`].
///
/// Cloneable but not `Copy` — same reasoning as [`SectionIter`].
#[derive(Debug, Clone)]
pub struct CurrencyIter<'a> {
    remaining: &'a [u8],
}

impl<'a> Iterator for CurrencyIter<'a> {
    type Item = CurrencyRecord<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        let (code, rest) = read_len_prefixed_str_u8(self.remaining)?;
        let (symbol, rest) = read_len_prefixed_str_u8(rest)?;
        if rest.is_empty() {
            self.remaining = &[];
            return None;
        }
        let flags = rest[0];
        self.remaining = &rest[1..];
        Some(CurrencyRecord {
            iso_code: code,
            symbol,
            symbol_after: (flags & 0x01) != 0,
            symbol_spaced: (flags & 0x02) != 0,
        })
    }
}

/// Read a `(u8 len, [u8; len])` UTF-8 string from `bytes`. Returns
/// `None` on truncation or invalid UTF-8.
fn read_len_prefixed_str_u8(bytes: &[u8]) -> Option<(&str, &[u8])> {
    if bytes.is_empty() {
        return None;
    }
    let len = usize::from(bytes[0]);
    if bytes.len() < 1 + len {
        return None;
    }
    let s = core::str::from_utf8(&bytes[1..=len]).ok()?;
    Some((s, &bytes[1 + len..]))
}

// -----------------------------------------------------------------------
// Date/time-formatting view
// -----------------------------------------------------------------------

/// Section id for the date-pattern table. Bytes `D` `t` `D` `p`.
///
/// Wire layout: exactly four consecutive `(u8 len, [u8; len] utf8)`
/// records in `Short`, `Medium`, `Long`, `Full` order. Each string is
/// a CLDR pattern using the standard token set (`y`, `yy`, `yyyy`,
/// `M`, `MM`, `MMM`, `MMMM`, `d`, `dd`, `E`, `EEE`, `EEEE`, etc.).
pub const SECT_DATE_PATTERNS: [u8; 4] = *b"DtDp";

/// Section id for the time-pattern table. Bytes `D` `t` `T` `p`.
///
/// Wire layout: exactly four consecutive `(u8 len, [u8; len] utf8)`
/// records in `Short`, `Medium`, `Long`, `Full` order. Each string is
/// a CLDR pattern using `H`, `HH`, `h`, `hh`, `m`, `mm`, `s`, `ss`,
/// `a`.
pub const SECT_TIME_PATTERNS: [u8; 4] = *b"DtTp";

/// Section id for the full month-name table. Bytes `D` `t` `M` `n`.
///
/// Wire layout: a `u16 count` prefix (always 12 for Gregorian)
/// followed by 12 `(u8 len, [u8; len] utf8)` records ordered
/// January .. December (index 1..=12; the reader keys by
/// `month - 1`).
pub const SECT_MONTH_NAMES: [u8; 4] = *b"DtMn";

/// Section id for the abbreviated month-name table. Bytes `D` `t`
/// `M` `a`. Same wire layout as [`SECT_MONTH_NAMES`].
pub const SECT_MONTH_ABBR: [u8; 4] = *b"DtMa";

/// Section id for the full weekday-name table. Bytes `D` `t` `W` `n`.
///
/// Wire layout: a `u16 count` prefix (always 7) followed by 7
/// `(u8 len, [u8; len] utf8)` records ordered Sunday .. Saturday
/// (index 0..=6, matching Zeller's-congruence output modulo the
/// convention shift documented in `stringcheese-icu-datetime`'s
/// docs).
pub const SECT_WEEKDAY_NAMES: [u8; 4] = *b"DtWn";

/// Section id for the abbreviated weekday-name table. Bytes `D` `t`
/// `W` `a`. Same wire layout as [`SECT_WEEKDAY_NAMES`].
pub const SECT_WEEKDAY_ABBR: [u8; 4] = *b"DtWa";

/// Section id for the AM/PM markers. Bytes `D` `t` `A` `p`.
///
/// Wire layout: two consecutive `(u8 len, [u8; len] utf8)` records
/// in AM, PM order.
pub const SECT_AM_PM: [u8; 4] = *b"DtAp";

/// Section id for the era-name table. Bytes `D` `t` `E` `r`.
///
/// Wire layout: two consecutive `(u8 len, [u8; len] utf8)` records
/// in BC, AD order.
pub const SECT_ERA_NAMES: [u8; 4] = *b"DtEr";

/// The CLDR-standard length classes a date or time pattern can carry.
///
/// Matches the enum shipped in the WIT interface's `date-length` /
/// `time-length` types. The wire encoding is `u8` (index into the
/// 4-entry pattern table).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DateTimeLength {
    /// The shortest CLDR pattern (typically `M/d/y` or `HH:mm`).
    Short = 0,
    /// The default "medium" CLDR pattern (typically `MMM d, y`).
    Medium = 1,
    /// The verbose CLDR pattern (typically `MMMM d, y`).
    Long = 2,
    /// The most verbose CLDR pattern (typically `EEEE, MMMM d, y`).
    Full = 3,
}

impl DateTimeLength {
    /// The wire-encoded `u8` value (index into the pattern table).
    #[must_use]
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Round-trip a `u8` back into the typed enum.
    #[must_use]
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Short),
            1 => Some(Self::Medium),
            2 => Some(Self::Long),
            3 => Some(Self::Full),
            _ => None,
        }
    }
}

/// Zero-copy view into a SCUD file's date/time-formatting body.
///
/// Every accessor decodes a fixed-shape record on demand from the
/// underlying section bytes. Lookups are `O(n)` in the number of
/// entries in the section (at most 12 for months, 7 for weekdays,
/// 4 for pattern lengths); the sections are small enough that a
/// linear walk is faster than a binary search's setup overhead.
#[derive(Debug, Clone, Copy)]
pub struct DateTimeDataView<'a> {
    date_patterns: &'a [u8],
    time_patterns: &'a [u8],
    month_names: &'a [u8],
    month_abbr: &'a [u8],
    weekday_names: &'a [u8],
    weekday_abbr: &'a [u8],
    am_pm: &'a [u8],
    era_names: &'a [u8],
}

impl<'a> DateTimeDataView<'a> {
    /// Parse a date/time body into a section-projected view.
    fn parse(body: &'a [u8]) -> Result<Self, ScudError> {
        let reader = SectionReader::new(body);
        Ok(Self {
            date_patterns: reader.find(SECT_DATE_PATTERNS)?.unwrap_or(&[]),
            time_patterns: reader.find(SECT_TIME_PATTERNS)?.unwrap_or(&[]),
            month_names: reader.find(SECT_MONTH_NAMES)?.unwrap_or(&[]),
            month_abbr: reader.find(SECT_MONTH_ABBR)?.unwrap_or(&[]),
            weekday_names: reader.find(SECT_WEEKDAY_NAMES)?.unwrap_or(&[]),
            weekday_abbr: reader.find(SECT_WEEKDAY_ABBR)?.unwrap_or(&[]),
            am_pm: reader.find(SECT_AM_PM)?.unwrap_or(&[]),
            era_names: reader.find(SECT_ERA_NAMES)?.unwrap_or(&[]),
        })
    }

    /// The date pattern at the given length, if the pack ships one.
    #[must_use]
    pub fn date_pattern(&self, length: DateTimeLength) -> Option<&'a str> {
        nth_len_prefixed_str(self.date_patterns, length.as_u8() as usize)
    }

    /// The time pattern at the given length, if the pack ships one.
    #[must_use]
    pub fn time_pattern(&self, length: DateTimeLength) -> Option<&'a str> {
        nth_len_prefixed_str(self.time_patterns, length.as_u8() as usize)
    }

    /// The full month name for `month` (1..=12), if the pack covers
    /// it. Returns `None` for `0` or `> 12`.
    #[must_use]
    pub fn month_name(&self, month: u8) -> Option<&'a str> {
        if month == 0 || month > 12 {
            return None;
        }
        nth_counted_str(self.month_names, usize::from(month - 1))
    }

    /// The abbreviated month name for `month` (1..=12).
    #[must_use]
    pub fn month_abbreviation(&self, month: u8) -> Option<&'a str> {
        if month == 0 || month > 12 {
            return None;
        }
        nth_counted_str(self.month_abbr, usize::from(month - 1))
    }

    /// The full weekday name for `weekday` (0..=6, where 0 is Sunday
    /// and 6 is Saturday — matching Zeller's congruence output shifted
    /// so Sunday-first ordering is the wire convention).
    #[must_use]
    pub fn weekday_name(&self, weekday: u8) -> Option<&'a str> {
        if weekday > 6 {
            return None;
        }
        nth_counted_str(self.weekday_names, usize::from(weekday))
    }

    /// The abbreviated weekday name for `weekday` (0..=6).
    #[must_use]
    pub fn weekday_abbreviation(&self, weekday: u8) -> Option<&'a str> {
        if weekday > 6 {
            return None;
        }
        nth_counted_str(self.weekday_abbr, usize::from(weekday))
    }

    /// The AM marker (`"AM"` in most English packs).
    #[must_use]
    pub fn am(&self) -> Option<&'a str> {
        nth_len_prefixed_str(self.am_pm, 0)
    }

    /// The PM marker (`"PM"` in most English packs).
    #[must_use]
    pub fn pm(&self) -> Option<&'a str> {
        nth_len_prefixed_str(self.am_pm, 1)
    }

    /// The era name for BC (index 0).
    #[must_use]
    pub fn era_bc(&self) -> Option<&'a str> {
        nth_len_prefixed_str(self.era_names, 0)
    }

    /// The era name for AD (index 1).
    #[must_use]
    pub fn era_ad(&self) -> Option<&'a str> {
        nth_len_prefixed_str(self.era_names, 1)
    }
}

// -----------------------------------------------------------------------
// Break-iteration view (Phase 5)
// -----------------------------------------------------------------------

/// Section id for the grapheme-cluster property class table. Bytes
/// `B` `k` `G` `c`.
///
/// Wire layout: a `u32 count` prefix followed by `count` fixed-width
/// `(u32 start, u32 length, u8 class)` records (9 bytes each) sorted
/// by `start`. The `class` byte is one of the [`GraphemeClass`]
/// discriminants. Ranges are half-open in spirit — the range covers
/// scalars `start..start + length`. An empty section means "the
/// algorithm crate applies its built-in default classification"; the
/// Phase 5 default pack ships empty class sections and lets the
/// algorithm crate own the classification tables in code.
pub const SECT_GRAPHEME_CLASSES: [u8; 4] = *b"BkGc";

/// Section id for the word-break property class table. Same wire
/// layout as [`SECT_GRAPHEME_CLASSES`]; the `class` byte is a
/// [`WordClass`] discriminant. Bytes `B` `k` `W` `c`.
pub const SECT_WORD_CLASSES: [u8; 4] = *b"BkWc";

/// Section id for the sentence-break property class table. Same wire
/// layout as [`SECT_GRAPHEME_CLASSES`]; the `class` byte is a
/// [`SentenceClass`] discriminant. Bytes `B` `k` `S` `c`.
pub const SECT_SENTENCE_CLASSES: [u8; 4] = *b"BkSc";

/// Section id for the grapheme-cluster rule table. Bytes `B` `k`
/// `G` `r`.
///
/// Wire layout: a single `u8 rules_id` byte identifying which
/// rule-set the algorithm crate should apply. Phase 5 defines two
/// values:
///
/// * `0` — no rules section present (identical to omitting the
///   section).
/// * `1` — the UAX #29 default rule set built into the algorithm
///   crate.
///
/// Future locale-specific tailorings (Japanese/Chinese word-break
/// dictionaries) will carry additional bytes after the id.
pub const SECT_GRAPHEME_RULES: [u8; 4] = *b"BkGr";

/// Section id for the word-break rule table. Same wire layout as
/// [`SECT_GRAPHEME_RULES`]. Bytes `B` `k` `W` `r`.
pub const SECT_WORD_RULES: [u8; 4] = *b"BkWr";

/// Section id for the sentence-break rule table. Same wire layout as
/// [`SECT_GRAPHEME_RULES`]. Bytes `B` `k` `S` `r`.
pub const SECT_SENTENCE_RULES: [u8; 4] = *b"BkSr";

/// Section id for the CJK word-break dictionary. Bytes `B` `k` `W` `d`.
///
/// Ships a sorted list of dictionary word bytes so the
/// [`stringcheese-icu-segment`](../../../stringcheese-icu-segment)
/// engine can run a forward-maximum-match (FMM) word segmenter over
/// CJK runs where UAX #29 leaves each ideograph as its own token. See
/// the crate's segmenter documentation for the FMM algorithm.
///
/// Wire layout (little-endian throughout):
///
/// ```text
///    0     4     count             u32 le — number of dictionary
///                                  entries
///    4     2     max_word_len_bytes u16 le — the longest word's
///                                  byte length (used to bound the
///                                  FMM inner loop)
///    6     2     reserved          u16 le — must be zero
///    8     4*n   offsets           u32 le * count — offsets into
///                                  the payload region for each
///                                  entry, sorted lexicographically
///                                  by the entry's UTF-8 bytes
///    ...   var   payload           for each entry:
///                                    u16 length + [u8; length]
///                                  words, lexicographically sorted
/// ```
///
/// Entries must be non-empty (`length > 0`) and lexicographically
/// sorted so a binary search over the offset index is well-defined.
/// The writer sorts inputs; readers assume the invariant holds.
pub const SECT_WORD_DICT: [u8; 4] = *b"BkWd";

/// Well-known rule-id byte for "use the UAX #29 default rules".
/// Written by [`BreakSectionBuilder::set_default_rules`].
pub const RULES_UAX29_DEFAULT: u8 = 1;

/// Unicode `Grapheme_Cluster_Break` property values per UAX #29
/// § 3.
///
/// The `Other` value covers every scalar that does not carry an
/// explicit `Grapheme_Cluster_Break` classification (the default
/// class). SCUD packs may omit ranges that map to `Other`; the
/// algorithm crate substitutes `Other` for any lookup that finds no
/// covering range.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum GraphemeClass {
    /// Default class.
    Other = 0,
    /// `CR` — U+000D.
    Cr = 1,
    /// `LF` — U+000A.
    Lf = 2,
    /// `Control` — control characters excluding CR/LF (`Cc`, `Cf`
    /// minus certain exceptions, line/paragraph separators, ZWNJ,
    /// ZWJ carve-out handled elsewhere).
    Control = 3,
    /// `Extend` — combining marks and the like.
    Extend = 4,
    /// `ZWJ` — U+200D.
    Zwj = 5,
    /// `Regional_Indicator` — U+1F1E6..U+1F1FF.
    RegionalIndicator = 6,
    /// `Prepend` — a prepend character (Arabic number sign etc.).
    Prepend = 7,
    /// `SpacingMark` — spacing combining marks that join to the
    /// previous grapheme.
    SpacingMark = 8,
    /// `L` — Hangul leading jamo.
    HangulL = 9,
    /// `V` — Hangul vowel jamo.
    HangulV = 10,
    /// `T` — Hangul trailing jamo.
    HangulT = 11,
    /// `LV` — Hangul precomposed syllable ending in a vowel.
    HangulLv = 12,
    /// `LVT` — Hangul precomposed syllable ending in a trailing jamo.
    HangulLvt = 13,
    /// `Extended_Pictographic` — an emoji or emoji-adjacent
    /// pictographic scalar (per UAX #29 + UTS #51).
    ExtendedPictographic = 14,
}

impl GraphemeClass {
    /// Round-trip a `u8` back into the typed enum. `Other` for
    /// unknown discriminants preserves the "unknown class is Other"
    /// invariant the algorithm crate relies on.
    #[must_use]
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Cr,
            2 => Self::Lf,
            3 => Self::Control,
            4 => Self::Extend,
            5 => Self::Zwj,
            6 => Self::RegionalIndicator,
            7 => Self::Prepend,
            8 => Self::SpacingMark,
            9 => Self::HangulL,
            10 => Self::HangulV,
            11 => Self::HangulT,
            12 => Self::HangulLv,
            13 => Self::HangulLvt,
            14 => Self::ExtendedPictographic,
            _ => Self::Other,
        }
    }

    /// The wire-encoded `u8` discriminant.
    #[must_use]
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Unicode `Word_Break` property values per UAX #29 § 4.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum WordClass {
    /// Default class.
    Other = 0,
    /// `CR`.
    Cr = 1,
    /// `LF`.
    Lf = 2,
    /// `Newline` — line/paragraph separator except CR/LF.
    Newline = 3,
    /// `Extend`.
    Extend = 4,
    /// `ZWJ` — U+200D.
    Zwj = 5,
    /// `Regional_Indicator`.
    RegionalIndicator = 6,
    /// `Format`.
    Format = 7,
    /// `Katakana`.
    Katakana = 8,
    /// `Hebrew_Letter`.
    HebrewLetter = 9,
    /// `ALetter`.
    ALetter = 10,
    /// `Single_Quote` — U+0027.
    SingleQuote = 11,
    /// `Double_Quote` — U+0022.
    DoubleQuote = 12,
    /// `MidNumLet`.
    MidNumLet = 13,
    /// `MidLetter`.
    MidLetter = 14,
    /// `MidNum`.
    MidNum = 15,
    /// `Numeric`.
    Numeric = 16,
    /// `ExtendNumLet`.
    ExtendNumLet = 17,
    /// `WSegSpace`.
    WSegSpace = 18,
    /// `Extended_Pictographic`.
    ExtendedPictographic = 19,
}

impl WordClass {
    /// Round-trip a `u8` back into the typed enum. Unknown → `Other`.
    #[must_use]
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Cr,
            2 => Self::Lf,
            3 => Self::Newline,
            4 => Self::Extend,
            5 => Self::Zwj,
            6 => Self::RegionalIndicator,
            7 => Self::Format,
            8 => Self::Katakana,
            9 => Self::HebrewLetter,
            10 => Self::ALetter,
            11 => Self::SingleQuote,
            12 => Self::DoubleQuote,
            13 => Self::MidNumLet,
            14 => Self::MidLetter,
            15 => Self::MidNum,
            16 => Self::Numeric,
            17 => Self::ExtendNumLet,
            18 => Self::WSegSpace,
            19 => Self::ExtendedPictographic,
            _ => Self::Other,
        }
    }

    /// The wire-encoded `u8` discriminant.
    #[must_use]
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Unicode `Sentence_Break` property values per UAX #29 § 5.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SentenceClass {
    /// Default class.
    Other = 0,
    /// `CR`.
    Cr = 1,
    /// `LF`.
    Lf = 2,
    /// `Extend`.
    Extend = 3,
    /// `Sep` — line/paragraph separator excluding CR/LF.
    Sep = 4,
    /// `Format`.
    Format = 5,
    /// `Sp` — whitespace.
    Sp = 6,
    /// `Lower`.
    Lower = 7,
    /// `Upper`.
    Upper = 8,
    /// `OLetter` — letters that are neither upper nor lower.
    OLetter = 9,
    /// `Numeric`.
    Numeric = 10,
    /// `ATerm` — ambiguous sentence terminator (`.`).
    ATerm = 11,
    /// `STerm` — definite sentence terminator (`!`, `?`, …).
    STerm = 12,
    /// `Close` — closing punctuation.
    Close = 13,
    /// `SContinue` — continuation punctuation (`,`, `;`, …) that
    /// suppresses a break after an `ATerm`.
    SContinue = 14,
}

impl SentenceClass {
    /// Round-trip a `u8` back into the typed enum. Unknown → `Other`.
    #[must_use]
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Cr,
            2 => Self::Lf,
            3 => Self::Extend,
            4 => Self::Sep,
            5 => Self::Format,
            6 => Self::Sp,
            7 => Self::Lower,
            8 => Self::Upper,
            9 => Self::OLetter,
            10 => Self::Numeric,
            11 => Self::ATerm,
            12 => Self::STerm,
            13 => Self::Close,
            14 => Self::SContinue,
            _ => Self::Other,
        }
    }

    /// The wire-encoded `u8` discriminant.
    #[must_use]
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Zero-copy view into a SCUD file's break-iteration body.
///
/// Every accessor is `O(log n)` in the size of the corresponding
/// class-range table; the rule-id accessors are `O(1)`. Callers who
/// see all sections as empty (the Phase 5 default pack) fall back to
/// the algorithm crate's built-in default classification and rule
/// set.
#[derive(Debug, Clone, Copy)]
pub struct BreakDataView<'a> {
    grapheme_classes: &'a [u8],
    word_classes: &'a [u8],
    sentence_classes: &'a [u8],
    grapheme_rules: &'a [u8],
    word_rules: &'a [u8],
    sentence_rules: &'a [u8],
    word_dict: &'a [u8],
}

impl<'a> BreakDataView<'a> {
    /// Parse a break-iteration body into a section-projected view.
    fn parse(body: &'a [u8]) -> Result<Self, ScudError> {
        let reader = SectionReader::new(body);
        Ok(Self {
            grapheme_classes: reader.find(SECT_GRAPHEME_CLASSES)?.unwrap_or(&[]),
            word_classes: reader.find(SECT_WORD_CLASSES)?.unwrap_or(&[]),
            sentence_classes: reader.find(SECT_SENTENCE_CLASSES)?.unwrap_or(&[]),
            grapheme_rules: reader.find(SECT_GRAPHEME_RULES)?.unwrap_or(&[]),
            word_rules: reader.find(SECT_WORD_RULES)?.unwrap_or(&[]),
            sentence_rules: reader.find(SECT_SENTENCE_RULES)?.unwrap_or(&[]),
            word_dict: reader.find(SECT_WORD_DICT)?.unwrap_or(&[]),
        })
    }

    /// True iff the grapheme-class section carries at least one
    /// range.
    #[must_use]
    pub fn has_grapheme_classes(&self) -> bool {
        range_table_count(self.grapheme_classes) > 0
    }

    /// True iff the word-class section carries at least one range.
    #[must_use]
    pub fn has_word_classes(&self) -> bool {
        range_table_count(self.word_classes) > 0
    }

    /// True iff the sentence-class section carries at least one
    /// range.
    #[must_use]
    pub fn has_sentence_classes(&self) -> bool {
        range_table_count(self.sentence_classes) > 0
    }

    /// The rule-id byte for grapheme rules, or `0` (no rules) if the
    /// section is absent.
    #[must_use]
    pub fn grapheme_rules_id(&self) -> u8 {
        first_byte_or_zero(self.grapheme_rules)
    }

    /// The rule-id byte for word rules.
    #[must_use]
    pub fn word_rules_id(&self) -> u8 {
        first_byte_or_zero(self.word_rules)
    }

    /// The rule-id byte for sentence rules.
    #[must_use]
    pub fn sentence_rules_id(&self) -> u8 {
        first_byte_or_zero(self.sentence_rules)
    }

    /// Look up the [`GraphemeClass`] of the given scalar, consulting
    /// only the pack's class table. Returns `None` when the pack
    /// does not carry any classification for `cp` (the caller
    /// substitutes its built-in default in that case).
    #[must_use]
    pub fn grapheme_class(&self, cp: u32) -> Option<GraphemeClass> {
        lookup_range_class(self.grapheme_classes, cp).map(GraphemeClass::from_u8)
    }

    /// Look up the [`WordClass`] of the given scalar.
    #[must_use]
    pub fn word_class(&self, cp: u32) -> Option<WordClass> {
        lookup_range_class(self.word_classes, cp).map(WordClass::from_u8)
    }

    /// Look up the [`SentenceClass`] of the given scalar.
    #[must_use]
    pub fn sentence_class(&self, cp: u32) -> Option<SentenceClass> {
        lookup_range_class(self.sentence_classes, cp).map(SentenceClass::from_u8)
    }

    /// The CJK word-break dictionary carried by this pack, if any.
    ///
    /// Returns `None` when the pack ships no dictionary section (the
    /// Phase 5 default). A [`WordDictView`] wraps the section bytes
    /// and exposes the FMM-friendly
    /// [`WordDictView::longest_prefix_match`] lookup used by the
    /// segment engine.
    #[must_use]
    pub fn word_dict(&self) -> Option<WordDictView<'a>> {
        WordDictView::parse(self.word_dict)
    }
}

// -----------------------------------------------------------------------
// CJK word-break dictionary view (see [`SECT_WORD_DICT`])
// -----------------------------------------------------------------------

/// Zero-copy view into a SCUD file's CJK word-break dictionary.
///
/// Carries a sorted list of dictionary entries so the segment engine
/// can drive a forward-maximum-match (FMM) word segmenter over CJK
/// script runs where UAX #29 leaves each ideograph as its own word.
///
/// Every lookup is `O(log n)` in the entry count plus `O(k)` in the
/// probed word's byte length. See [`SECT_WORD_DICT`] for the wire
/// layout.
///
/// # FMM contract
///
/// [`longest_prefix_match`](Self::longest_prefix_match) returns the
/// **byte length** of the longest dictionary entry that matches the
/// input's leading UTF-8 bytes, or `None` when no entry matches. The
/// caller advances past the match and repeats until the input is
/// exhausted. When no entry matches, the caller emits a single
/// scalar and advances by its UTF-8 length — the standard
/// unknown-word fallback.
#[derive(Debug, Clone, Copy)]
pub struct WordDictView<'a> {
    /// Total entry count.
    count: usize,
    /// Longest word's byte length (bounds the FMM inner loop).
    max_word_len_bytes: usize,
    /// `count * u32` offset table into `payload`.
    index: &'a [u8],
    /// Concatenated `(u16 length, [u8; length])` records.
    payload: &'a [u8],
}

impl<'a> WordDictView<'a> {
    /// Parse a `SECT_WORD_DICT` section into a view.
    ///
    /// Returns `None` when the section is empty (no dictionary
    /// shipped) or its declared layout does not fit inside the
    /// section bytes.
    #[must_use]
    fn parse(bytes: &'a [u8]) -> Option<Self> {
        if bytes.len() < 8 {
            return None;
        }
        let count = read_u32(&bytes[0..4]) as usize;
        if count == 0 {
            return None;
        }
        let max_word_len_bytes = usize::from(read_u16(&bytes[4..6]));
        // `bytes[6..8]` is the reserved u16.
        let index_start = 8usize;
        let index_len = count.checked_mul(4)?;
        let index_end = index_start.checked_add(index_len)?;
        if index_end > bytes.len() {
            return None;
        }
        let index = &bytes[index_start..index_end];
        let payload = &bytes[index_end..];
        Some(Self {
            count,
            max_word_len_bytes,
            index,
            payload,
        })
    }

    /// Total dictionary entry count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.count
    }

    /// True iff the dictionary is empty (unreachable via the private
    /// `parse` constructor, which returns `None` on empty; kept for
    /// API symmetry with `Vec`-style types).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// The longest dictionary entry's byte length. The FMM caller
    /// bounds its inner "try longer prefixes" loop by this value.
    #[must_use]
    pub fn max_word_len_bytes(&self) -> usize {
        self.max_word_len_bytes
    }

    /// Read the entry at position `idx` (`0..self.len()`) as a
    /// byte slice. Returns `None` if `idx` is out of range or the
    /// record is malformed.
    #[must_use]
    fn entry(&self, idx: usize) -> Option<&'a [u8]> {
        if idx >= self.count {
            return None;
        }
        let off = read_u32(&self.index[idx * 4..idx * 4 + 4]) as usize;
        if off + 2 > self.payload.len() {
            return None;
        }
        let len = usize::from(read_u16(&self.payload[off..off + 2]));
        let start = off + 2;
        let end = start + len;
        if end > self.payload.len() {
            return None;
        }
        Some(&self.payload[start..end])
    }

    /// Return the byte length of the longest dictionary entry that
    /// is a prefix of `input`, or `None` if no entry matches.
    ///
    /// # Algorithm
    ///
    /// Binary-searches for the last entry `<=` `input`, then walks
    /// backwards through the sorted table while entries share their
    /// leading bytes with `input`, returning the longest prefix
    /// found. This lets a call fail-fast when `input` sorts far away
    /// from any dictionary entry, but stays correct when the
    /// candidate largest-<= entry is itself not a prefix (e.g. dict
    /// carries `"北京"` and `"北京大学"`, input is `"北京很大"` — the
    /// largest-<= entry `"北京大学"` is not a prefix, but the earlier
    /// entry `"北京"` is).
    ///
    /// # Complexity
    ///
    /// `O(log n)` for the binary search plus `O(k)` for the linear
    /// walk over entries that share a common prefix with `input`.
    /// For a well-distributed 2000-entry dictionary the linear walk
    /// visits only a handful of entries in the worst case.
    #[must_use]
    pub fn longest_prefix_match(&self, input: &[u8]) -> Option<usize> {
        if input.is_empty() || self.count == 0 {
            return None;
        }
        // Binary search for the last entry <= input (lexicographic
        // over raw bytes).
        let mut lo = 0usize;
        let mut hi = self.count;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let entry = self.entry(mid)?;
            if entry <= input {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo == 0 {
            return None;
        }
        // Walk backwards from the candidate — the first entry that
        // is a prefix of `input` is the longest such entry in the
        // sorted table, because any longer-prefix entry would sort
        // strictly after the current candidate.
        //
        // The walk terminates as soon as an entry no longer shares
        // its first byte with `input`: since the table is sorted,
        // no earlier entry can be a prefix once the common-first-
        // byte invariant is broken.
        let first_byte = input[0];
        let mut i = lo;
        while i > 0 {
            i -= 1;
            let entry = self.entry(i)?;
            if entry.is_empty() || entry[0] != first_byte {
                return None;
            }
            if input.starts_with(entry) {
                return Some(entry.len());
            }
        }
        None
    }

    /// True iff the dictionary contains exactly `word` as an entry.
    #[must_use]
    pub fn contains(&self, word: &[u8]) -> bool {
        if word.is_empty() {
            return false;
        }
        let mut lo = 0usize;
        let mut hi = self.count;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let Some(entry) = self.entry(mid) else {
                return false;
            };
            match entry.cmp(word) {
                core::cmp::Ordering::Less => lo = mid + 1,
                core::cmp::Ordering::Greater => hi = mid,
                core::cmp::Ordering::Equal => return true,
            }
        }
        false
    }
}

// -----------------------------------------------------------------------
// Line-break view (Phase 5 follow-up — UAX #14)
// -----------------------------------------------------------------------

/// Section id for the UAX #14 line-break property class table. Bytes
/// `L` `b` `C` `c`.
///
/// Same wire layout as [`SECT_GRAPHEME_CLASSES`]: a `u32 count`
/// prefix followed by `count` fixed-width `(u32 start, u32 length,
/// u8 class)` records sorted by `start`. The `class` byte is one of
/// the [`LineBreakClass`] discriminants. An empty section means "the
/// algorithm crate applies its built-in default classification".
pub const SECT_LB_CLASSES: [u8; 4] = *b"LbCc";

/// Section id for the UAX #14 line-break rule table marker. Bytes
/// `L` `b` `R` `l`.
///
/// Wire layout: a single `u8 rules_id` byte identifying which
/// rule-set the algorithm crate should apply. Values:
///
/// * `0` — no rules section present (identical to omitting the
///   section).
/// * `1` — the UAX #14 default rule set built into the algorithm
///   crate ([`RULES_UAX14_DEFAULT`]).
pub const SECT_LB_RULES: [u8; 4] = *b"LbRl";

/// Section id for optional per-locale line-break tailoring bytes.
/// Bytes `L` `b` `T` `l`.
///
/// Wire layout: a single `u8 strictness` byte selecting the CJK
/// strictness mode ([`LB_STRICTNESS_LOOSE`] / [`LB_STRICTNESS_NORMAL`]
/// / [`LB_STRICTNESS_STRICT`], see UAX #14 § 6.1). Absent section
/// means "normal" (the CLDR default).
pub const SECT_LB_TAILORINGS: [u8; 4] = *b"LbTl";

/// Well-known rule-id byte for "use the UAX #14 default rules".
pub const RULES_UAX14_DEFAULT: u8 = 1;

/// Strictness tag: loose (line-break-strictness = "loose"). CJK
/// tailoring per UAX #14 § 6.1 that expands the small-kana / hyphen
/// break-opportunity set.
pub const LB_STRICTNESS_LOOSE: u8 = 0;

/// Strictness tag: normal (line-break-strictness = "normal"). The
/// CLDR default.
pub const LB_STRICTNESS_NORMAL: u8 = 1;

/// Strictness tag: strict (line-break-strictness = "strict"). CJK
/// tailoring per UAX #14 § 6.1 that contracts the small-kana /
/// hyphen break-opportunity set.
pub const LB_STRICTNESS_STRICT: u8 = 2;

/// Unicode `Line_Break` property values per UAX #14.
///
/// Full 43-value set (the 40 pair-table classes + BK / SP / EOT
/// meta-markers used by the algorithm's tail state machine). The
/// `Xx` value covers scalars whose `Line_Break` property is `XX`
/// (Unknown) — LB1 folds these to `AL` before the pair table is
/// consulted.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
#[allow(missing_docs)]
pub enum LineBreakClass {
    /// Default / unknown class (`XX`). Resolved to `AL` before the
    /// pair table is consulted per LB1.
    Xx = 0,
    /// `OP` — open punctuation.
    Op = 1,
    /// `CL` — closing punctuation.
    Cl = 2,
    /// `CP` — closing parenthesis (behaves like `CL` under LB13).
    Cp = 3,
    /// `QU` — quotation mark.
    Qu = 4,
    /// `GL` — non-breaking glue.
    Gl = 5,
    /// `NS` — non-starter.
    Ns = 6,
    /// `EX` — exclamation / interrogation.
    Ex = 7,
    /// `SY` — symbols allowing break after.
    Sy = 8,
    /// `IS` — infix numeric separator.
    Is = 9,
    /// `PR` — prefix numeric.
    Pr = 10,
    /// `PO` — postfix numeric.
    Po = 11,
    /// `NU` — numeric.
    Nu = 12,
    /// `AL` — alphabetic.
    Al = 13,
    /// `HL` — Hebrew letter.
    Hl = 14,
    /// `ID` — ideographic.
    Id = 15,
    /// `IN` — inseparable.
    In = 16,
    /// `HY` — hyphen.
    Hy = 17,
    /// `BA` — break after.
    Ba = 18,
    /// `BB` — break before.
    Bb = 19,
    /// `B2` — break opportunity before and after.
    B2 = 20,
    /// `ZW` — zero-width space.
    Zw = 21,
    /// `CM` — combining mark. LB9 folds this into the preceding
    /// class before the pair table is consulted.
    Cm = 22,
    /// `WJ` — word joiner.
    Wj = 23,
    /// `H2` — Hangul syllable of shape LV.
    H2 = 24,
    /// `H3` — Hangul syllable of shape LVT.
    H3 = 25,
    /// `JL` — Hangul leading jamo.
    Jl = 26,
    /// `JV` — Hangul vowel jamo.
    Jv = 27,
    /// `JT` — Hangul trailing jamo.
    Jt = 28,
    /// `RI` — Regional Indicator (flag half). Paired per `LB30a`.
    Ri = 29,
    /// `EB` — Emoji base.
    Eb = 30,
    /// `EM` — Emoji modifier.
    Em = 31,
    /// `ZWJ` — Zero-Width Joiner.
    Zwj = 32,
    /// `CJ` — Conditional Japanese starter. LB1 folds this into
    /// `NS` (normal / strict) or `ID` (loose) before the pair table
    /// is consulted.
    Cj = 33,
    /// `SG` — Surrogate. LB1 folds this into `AL`.
    Sg = 34,
    /// `AI` — Ambiguous. LB1 folds this into `AL` (or `ID` under
    /// certain East-Asian-Width tailorings).
    Ai = 35,
    /// `CB` — Contingent break opportunity.
    Cb = 36,
    /// `BK` — mandatory break (paragraph separator U+2028/U+2029
    /// only).
    Bk = 37,
    /// `CR` — mandatory break (U+000D).
    Cr = 38,
    /// `LF` — mandatory break (U+000A).
    Lf = 39,
    /// `NL` — mandatory break (U+0085).
    Nl = 40,
    /// `SP` — space.
    Sp = 41,
    /// `SA` — South-East Asian scripts (Thai, Lao, Khmer, Burmese).
    /// LB1 folds this to `AL` in Phase 5 pending dictionary support.
    Sa = 42,
}

impl LineBreakClass {
    /// Round-trip a `u8` back into the typed enum. Unknown → `Xx`.
    #[must_use]
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Op,
            2 => Self::Cl,
            3 => Self::Cp,
            4 => Self::Qu,
            5 => Self::Gl,
            6 => Self::Ns,
            7 => Self::Ex,
            8 => Self::Sy,
            9 => Self::Is,
            10 => Self::Pr,
            11 => Self::Po,
            12 => Self::Nu,
            13 => Self::Al,
            14 => Self::Hl,
            15 => Self::Id,
            16 => Self::In,
            17 => Self::Hy,
            18 => Self::Ba,
            19 => Self::Bb,
            20 => Self::B2,
            21 => Self::Zw,
            22 => Self::Cm,
            23 => Self::Wj,
            24 => Self::H2,
            25 => Self::H3,
            26 => Self::Jl,
            27 => Self::Jv,
            28 => Self::Jt,
            29 => Self::Ri,
            30 => Self::Eb,
            31 => Self::Em,
            32 => Self::Zwj,
            33 => Self::Cj,
            34 => Self::Sg,
            35 => Self::Ai,
            36 => Self::Cb,
            37 => Self::Bk,
            38 => Self::Cr,
            39 => Self::Lf,
            40 => Self::Nl,
            41 => Self::Sp,
            42 => Self::Sa,
            _ => Self::Xx,
        }
    }

    /// The wire-encoded `u8` discriminant.
    #[must_use]
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Zero-copy view into a SCUD file's line-break body.
///
/// Every accessor is `O(log n)` in the size of the corresponding
/// class-range table; the rule-id / strictness accessors are `O(1)`.
/// Callers who see the section as empty fall back to the algorithm
/// crate's built-in default classification and rule set.
#[derive(Debug, Clone, Copy)]
pub struct LineBreakDataView<'a> {
    classes: &'a [u8],
    rules: &'a [u8],
    tailorings: &'a [u8],
}

impl<'a> LineBreakDataView<'a> {
    fn parse(body: &'a [u8]) -> Result<Self, ScudError> {
        let reader = SectionReader::new(body);
        Ok(Self {
            classes: reader.find(SECT_LB_CLASSES)?.unwrap_or(&[]),
            rules: reader.find(SECT_LB_RULES)?.unwrap_or(&[]),
            tailorings: reader.find(SECT_LB_TAILORINGS)?.unwrap_or(&[]),
        })
    }

    /// True iff the class section carries at least one range.
    #[must_use]
    pub fn has_classes(&self) -> bool {
        range_table_count(self.classes) > 0
    }

    /// The rule-id byte, or `0` (no rules) if the section is absent.
    #[must_use]
    pub fn rules_id(&self) -> u8 {
        first_byte_or_zero(self.rules)
    }

    /// The strictness byte, or [`LB_STRICTNESS_NORMAL`] if absent.
    #[must_use]
    pub fn strictness(&self) -> u8 {
        if self.tailorings.is_empty() {
            LB_STRICTNESS_NORMAL
        } else {
            self.tailorings[0]
        }
    }

    /// Look up the [`LineBreakClass`] of the given scalar. Returns
    /// `None` when the pack does not carry any classification for
    /// `cp`.
    #[must_use]
    pub fn class(&self, cp: u32) -> Option<LineBreakClass> {
        lookup_range_class(self.classes, cp).map(LineBreakClass::from_u8)
    }
}

/// Wire size of one class-range record: `(u32 start, u32 length, u8
/// class)` = 9 bytes.
const CLASS_RANGE_RECORD_BYTES: usize = 9;

fn range_table_count(bytes: &[u8]) -> u32 {
    if bytes.len() < 4 {
        return 0;
    }
    read_u32(&bytes[0..4])
}

fn first_byte_or_zero(bytes: &[u8]) -> u8 {
    if bytes.is_empty() { 0 } else { bytes[0] }
}

/// Binary-search the sorted class-range table encoded at `bytes`
/// for a range containing `cp`. Returns the `class` byte if found.
fn lookup_range_class(bytes: &[u8], cp: u32) -> Option<u8> {
    let count = range_table_count(bytes) as usize;
    if count == 0 {
        return None;
    }
    let records = &bytes[4..];
    if records.len() < count * CLASS_RANGE_RECORD_BYTES {
        return None;
    }
    // Binary search on the ranges sorted by `start`. Because the
    // ranges are non-overlapping we can locate the last range whose
    // start <= cp and then check its length.
    let mut lo = 0usize;
    let mut hi = count;
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let off = mid * CLASS_RANGE_RECORD_BYTES;
        let start = read_u32(&records[off..off + 4]);
        if start <= cp {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    if lo == 0 {
        return None;
    }
    let idx = lo - 1;
    let off = idx * CLASS_RANGE_RECORD_BYTES;
    let start = read_u32(&records[off..off + 4]);
    let length = read_u32(&records[off + 4..off + 8]);
    let class = records[off + 8];
    // length == 0 is nonsensical; treat as no coverage.
    if length == 0 {
        return None;
    }
    // Overflow-safe: start + length may exceed u32::MAX; use u64.
    let end = u64::from(start) + u64::from(length);
    if u64::from(cp) < end {
        Some(class)
    } else {
        None
    }
}

/// Walk `n` consecutive `(u8 len, [u8; len] utf8)` records and return
/// the string at index `idx`. Returns `None` on truncation, invalid
/// UTF-8, or when `idx` runs past the end.
fn nth_len_prefixed_str(bytes: &[u8], idx: usize) -> Option<&str> {
    let mut cursor = bytes;
    for i in 0..=idx {
        let (s, rest) = read_len_prefixed_str_u8(cursor)?;
        if i == idx {
            return Some(s);
        }
        cursor = rest;
    }
    None
}

/// Like [`nth_len_prefixed_str`] but the section leads with a
/// `u16 count` prefix.
fn nth_counted_str(bytes: &[u8], idx: usize) -> Option<&str> {
    if bytes.len() < 2 {
        return None;
    }
    let count = usize::from(read_u16(&bytes[0..2]));
    if idx >= count {
        return None;
    }
    nth_len_prefixed_str(&bytes[2..], idx)
}

// -----------------------------------------------------------------------
// Writer
// -----------------------------------------------------------------------

/// Build a well-formed SCUD blob byte-by-byte.
///
/// Used by language-pack `build.rs` scripts to emit `case-<lang>.scud`
/// alongside the crate's compiled artifacts. Section order is not
/// significant — [`SectionReader`] searches by id.
///
/// # Example
///
/// ```
/// use stringcheese_scud::{
///     CaseSectionBuilder, ScudFile, ScudFlags, ScudWriter, CAP_CASE,
/// };
///
/// let mut case = CaseSectionBuilder::new();
/// case.push_simple_lower('A' as u32, 'a' as u32);
/// case.push_simple_upper('a' as u32, 'A' as u32);
/// case.push_simple_fold('A' as u32, 'a' as u32);
///
/// let mut w = ScudWriter::new(CAP_CASE, "44.1", Some("en"));
/// w.append_section(*b"sLwr", &case.simple_lower_bytes());
/// w.append_section(*b"sUpr", &case.simple_upper_bytes());
/// w.append_section(*b"sFld", &case.simple_fold_bytes());
/// let bytes = w.finish();
///
/// let file = ScudFile::from_slice(&bytes).unwrap();
/// assert_eq!(file.capability(), CAP_CASE);
/// assert_eq!(file.cldr_version(), "44.1");
/// assert_eq!(file.locale(), Some("en"));
/// let view = file.as_case_data().unwrap();
/// assert_eq!(view.simple_lower('A' as u32), Some('a' as u32));
/// assert_eq!(view.simple_upper('a' as u32), Some('A' as u32));
/// let _ = ScudFlags::HEADER_ALIGNED;
/// ```
#[cfg(feature = "alloc")]
pub struct ScudWriter {
    cap_id: [u8; 4],
    cldr_version: alloc::string::String,
    locale: Option<alloc::string::String>,
    body: Vec<u8>,
}

#[cfg(feature = "alloc")]
impl ScudWriter {
    /// Start a fresh writer for a given capability, CLDR version, and
    /// optional locale tag.
    #[must_use]
    pub fn new(cap_id: [u8; 4], cldr_version: &str, locale: Option<&str>) -> Self {
        Self {
            cap_id,
            cldr_version: cldr_version.into(),
            locale: locale.map(alloc::string::String::from),
            body: Vec::new(),
        }
    }

    /// Append one `(section-id, bytes)` frame to the body.
    ///
    /// # Panics
    ///
    /// Panics if `bytes.len()` exceeds `u32::MAX` (the on-wire length
    /// field is a `u32`). SCUD packs are tens-of-kilobytes at most,
    /// so this bound cannot be hit in practice; the panic is a
    /// belt-and-braces guard on the writer's serialisation invariants.
    pub fn append_section(&mut self, id: [u8; 4], bytes: &[u8]) {
        self.body.extend_from_slice(&id);
        self.body
            .extend_from_slice(&u32::try_from(bytes.len()).unwrap().to_le_bytes());
        self.body.extend_from_slice(bytes);
    }

    /// Finalise the writer into a byte buffer.
    ///
    /// # Panics
    ///
    /// Panics if the CLDR-version string, the locale tag, or the
    /// annotated header block exceeds `u32::MAX` bytes — an on-wire
    /// serialisation invariant no realistic pack can violate.
    #[must_use]
    pub fn finish(self) -> Vec<u8> {
        let mut header_body = Vec::new();
        let cldr = self.cldr_version.as_bytes();
        header_body.extend_from_slice(&u32::try_from(cldr.len()).unwrap().to_le_bytes());
        header_body.extend_from_slice(cldr);
        let mut flags_bits: u32 = ScudFlags::HEADER_ALIGNED;
        if let Some(loc) = &self.locale {
            flags_bits |= ScudFlags::HAS_LOCALE;
            header_body.extend_from_slice(&u32::try_from(loc.len()).unwrap().to_le_bytes());
            header_body.extend_from_slice(loc.as_bytes());
        }

        let mut out = Vec::with_capacity(HEADER_PREFIX_LEN + header_body.len() + self.body.len());
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&SUPPORTED_FORMAT_MAJOR.to_le_bytes());
        out.extend_from_slice(&CURRENT_FORMAT_MINOR.to_le_bytes());
        out.extend_from_slice(&flags_bits.to_le_bytes());
        out.extend_from_slice(&self.cap_id);
        out.extend_from_slice(&u32::try_from(header_body.len()).unwrap().to_le_bytes());
        out.extend_from_slice(&header_body);
        out.extend_from_slice(&self.body);
        out
    }
}

// -----------------------------------------------------------------------
// Section-builder helpers (case)
// -----------------------------------------------------------------------

/// Builds the byte-encoded sections that a case-mapping SCUD pack
/// contains. Sorts input entries by source scalar so the reader's
/// binary search stays valid.
#[cfg(feature = "alloc")]
#[derive(Default)]
pub struct CaseSectionBuilder {
    simple_lower: Vec<(u32, u32)>,
    simple_upper: Vec<(u32, u32)>,
    simple_fold: Vec<(u32, u32)>,
    full_upper: Vec<(u32, Vec<u32>)>,
    full_lower: Vec<(u32, Vec<u32>)>,
    full_fold: Vec<(u32, Vec<u32>)>,
    context: Vec<(u32, ContextKind, u32)>,
}

#[cfg(feature = "alloc")]
impl CaseSectionBuilder {
    /// Fresh, empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a `(from, to)` simple lowercase mapping.
    pub fn push_simple_lower(&mut self, from: u32, to: u32) {
        self.simple_lower.push((from, to));
    }

    /// Push a `(from, to)` simple uppercase mapping.
    pub fn push_simple_upper(&mut self, from: u32, to: u32) {
        self.simple_upper.push((from, to));
    }

    /// Push a `(from, to)` simple case-fold mapping.
    pub fn push_simple_fold(&mut self, from: u32, to: u32) {
        self.simple_fold.push((from, to));
    }

    /// Push a `(from, [to...])` full uppercase mapping.
    pub fn push_full_upper(&mut self, from: u32, to: &[u32]) {
        self.full_upper.push((from, to.to_vec()));
    }

    /// Push a `(from, [to...])` full lowercase mapping.
    pub fn push_full_lower(&mut self, from: u32, to: &[u32]) {
        self.full_lower.push((from, to.to_vec()));
    }

    /// Push a `(from, [to...])` full case-fold mapping.
    pub fn push_full_fold(&mut self, from: u32, to: &[u32]) {
        self.full_fold.push((from, to.to_vec()));
    }

    /// Push a contextual mapping keyed by `(from, kind)`.
    pub fn push_context(&mut self, from: u32, kind: ContextKind, to: u32) {
        self.context.push((from, kind, to));
    }

    /// Encode the simple lowercase table as SCUD-format bytes.
    #[must_use]
    pub fn simple_lower_bytes(&self) -> Vec<u8> {
        encode_pair_table(&self.simple_lower)
    }

    /// Encode the simple uppercase table.
    #[must_use]
    pub fn simple_upper_bytes(&self) -> Vec<u8> {
        encode_pair_table(&self.simple_upper)
    }

    /// Encode the simple case-fold table.
    #[must_use]
    pub fn simple_fold_bytes(&self) -> Vec<u8> {
        encode_pair_table(&self.simple_fold)
    }

    /// Encode the full uppercase table.
    #[must_use]
    pub fn full_upper_bytes(&self) -> Vec<u8> {
        encode_full_table(&self.full_upper)
    }

    /// Encode the full lowercase table.
    #[must_use]
    pub fn full_lower_bytes(&self) -> Vec<u8> {
        encode_full_table(&self.full_lower)
    }

    /// Encode the full case-fold table.
    #[must_use]
    pub fn full_fold_bytes(&self) -> Vec<u8> {
        encode_full_table(&self.full_fold)
    }

    /// Encode the contextual mapping table.
    #[must_use]
    pub fn context_bytes(&self) -> Vec<u8> {
        // Fixed-width (u32 src, u8 kind, u32 to) = 9 bytes/record.
        // Sort by (src, kind) for reproducible output; the reader
        // scans linearly for a matching src.
        let mut sorted = self.context.clone();
        sorted.sort_by_key(|(s, k, _)| (*s, *k as u8));
        let mut out = Vec::with_capacity(sorted.len() * 9);
        for (src, kind, to) in sorted {
            out.extend_from_slice(&src.to_le_bytes());
            out.push(kind as u8);
            out.extend_from_slice(&to.to_le_bytes());
        }
        out
    }
}

/// Builds the byte-encoded sections that a collation SCUD pack
/// contains. Sorts input entries by source scalar so the reader's
/// binary search stays valid.
#[cfg(feature = "alloc")]
#[derive(Default)]
pub struct CollationSectionBuilder {
    expansions: Vec<(u32, Vec<u32>)>,
    default_strength: Option<u8>,
    case_insensitive: Option<bool>,
    backwards_secondary: Option<bool>,
    /// Sorted list of `(cp, primary, secondary, tertiary)` weight
    /// overrides. See [`SECT_PRIMARY_OVERRIDES`] for the wire layout.
    primary_overrides: Vec<(u32, u32, u32, u32)>,
}

#[cfg(feature = "alloc")]
impl CollationSectionBuilder {
    /// Fresh, empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a `(from, [to...])` character-expansion mapping.
    ///
    /// Applied to both operands before UCA comparison — the classic
    /// use is DE-phonebook `ä → ae` (`push_expansion('ä' as u32,
    /// &['a' as u32, 'e' as u32])`) and both DE variants' `ß → ss`.
    pub fn push_expansion(&mut self, from: u32, to: &[u32]) {
        self.expansions.push((from, to.to_vec()));
    }

    /// Set the pack's default collation strength (see
    /// [`SECT_COLLATION_OPTIONS`] for the wire encoding).
    pub fn set_default_strength(&mut self, strength: u8) {
        self.default_strength = Some(strength);
    }

    /// Set the pack's case-insensitive flag.
    pub fn set_case_insensitive(&mut self, v: bool) {
        self.case_insensitive = Some(v);
    }

    /// Set the pack's backwards-secondary flag. When true, the
    /// collation engine reverses the secondary-weight comparison so
    /// diacritics tie-break right-to-left within a word — the French
    /// tailoring.
    pub fn set_backwards_secondary(&mut self, v: bool) {
        self.backwards_secondary = Some(v);
    }

    /// Push a primary-weight override for `cp`.
    ///
    /// See [`SECT_PRIMARY_OVERRIDES`] for the semantics. The
    /// canonical use is Turkish's primary-distinct dotless-ı
    /// (U+0131) ordering — `push_primary_override(0x0131, 190, 0, 0)`
    /// tags dotless-ı with a primary weight that sorts it between h
    /// and i in the Turkish alphabet.
    pub fn push_primary_override(&mut self, cp: u32, primary: u32, secondary: u32, tertiary: u32) {
        self.primary_overrides
            .push((cp, primary, secondary, tertiary));
    }

    /// Encode the character-expansion table.
    ///
    /// Wire shape mirrors the case-mapping full tables: a 4-byte
    /// `count` prefix, `count` fixed-width `(u32 src, u32
    /// payload_offset)` index entries sorted by `src`, and a packed
    /// payload region where each record is `u8 n, [u32; n]`.
    #[must_use]
    pub fn expansion_bytes(&self) -> Vec<u8> {
        encode_full_table(&self.expansions)
    }

    /// Encode the options blob. Returns an empty `Vec` when no
    /// options field has been set — callers use
    /// [`is_options_present`](Self::is_options_present) to decide
    /// whether to append the section at all.
    #[must_use]
    pub fn options_bytes(&self) -> Vec<u8> {
        if !self.is_options_present() {
            return Vec::new();
        }
        alloc::vec![
            self.default_strength.unwrap_or(2), // 2 = Tertiary default
            u8::from(self.case_insensitive.unwrap_or(false)),
            u8::from(self.backwards_secondary.unwrap_or(false)),
            0, // reserved
        ]
    }

    /// Encode the primary-weight override table.
    ///
    /// Wire shape: `u32 count` + `count × (u32 cp, u32 primary,
    /// u32 secondary, u32 tertiary)` sorted by `cp`. Returns an
    /// empty `Vec` when no overrides have been pushed.
    #[must_use]
    pub fn primary_overrides_bytes(&self) -> Vec<u8> {
        if self.primary_overrides.is_empty() {
            return Vec::new();
        }
        let mut sorted = self.primary_overrides.clone();
        sorted.sort_by_key(|(cp, _, _, _)| *cp);
        let count = u32::try_from(sorted.len()).unwrap_or(u32::MAX);
        let mut out = Vec::with_capacity(4 + sorted.len() * 16);
        out.extend_from_slice(&count.to_le_bytes());
        for (cp, pw, sw, tw) in sorted {
            out.extend_from_slice(&cp.to_le_bytes());
            out.extend_from_slice(&pw.to_le_bytes());
            out.extend_from_slice(&sw.to_le_bytes());
            out.extend_from_slice(&tw.to_le_bytes());
        }
        out
    }

    /// True iff the builder carries any options-blob content.
    #[must_use]
    pub fn is_options_present(&self) -> bool {
        self.default_strength.is_some()
            || self.case_insensitive.is_some()
            || self.backwards_secondary.is_some()
    }

    /// True iff the builder carries at least one primary-override row.
    #[must_use]
    pub fn has_primary_overrides(&self) -> bool {
        !self.primary_overrides.is_empty()
    }
}

/// Builds the byte-encoded sections that a plural-rules SCUD pack
/// contains. Preserves push order so the algorithm crate can drive
/// evaluation in CLDR-listed order (`zero → one → two → few → many
/// → other`, with locale-specific reordering allowed).
#[cfg(feature = "alloc")]
#[derive(Default)]
pub struct PluralSectionBuilder {
    cardinals: Vec<(PluralCategory, u8)>,
    ordinals: Vec<(PluralCategory, u8)>,
}

#[cfg(feature = "alloc")]
impl PluralSectionBuilder {
    /// Fresh, empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a cardinal `(category, rule_id)` entry.
    pub fn push_cardinal(&mut self, category: PluralCategory, rule_id: u8) {
        self.cardinals.push((category, rule_id));
    }

    /// Push an ordinal `(category, rule_id)` entry.
    pub fn push_ordinal(&mut self, category: PluralCategory, rule_id: u8) {
        self.ordinals.push((category, rule_id));
    }

    /// Encode the cardinal-rules table as SCUD-format bytes.
    #[must_use]
    pub fn cardinal_bytes(&self) -> Vec<u8> {
        encode_plural_table(&self.cardinals)
    }

    /// Encode the ordinal-rules table as SCUD-format bytes.
    #[must_use]
    pub fn ordinal_bytes(&self) -> Vec<u8> {
        encode_plural_table(&self.ordinals)
    }
}

#[cfg(feature = "alloc")]
fn encode_plural_table(entries: &[(PluralCategory, u8)]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + entries.len() * 2);
    let count = u16::try_from(entries.len()).unwrap_or(u16::MAX);
    out.extend_from_slice(&count.to_le_bytes());
    for (cat, rule) in entries {
        out.push(cat.as_u8());
        out.push(*rule);
    }
    out
}

/// Builds the byte-encoded sections that a number-formatting SCUD
/// pack contains.
#[cfg(feature = "alloc")]
#[derive(Default)]
pub struct NumberSectionBuilder {
    decimal: Option<DecimalPatternOwned>,
    currencies: Vec<CurrencyRecordOwned>,
    percent: Option<PercentPatternOwned>,
}

/// Owned form of [`DecimalPattern`], used by [`NumberSectionBuilder`]
/// at build time.
#[cfg(feature = "alloc")]
#[derive(Debug, Clone)]
struct DecimalPatternOwned {
    group_separator: alloc::string::String,
    decimal_separator: alloc::string::String,
    min_fraction: u8,
    max_fraction: u8,
    primary_grouping: u8,
    secondary_grouping: u8,
}

#[cfg(feature = "alloc")]
#[derive(Debug, Clone)]
struct CurrencyRecordOwned {
    iso_code: alloc::string::String,
    symbol: alloc::string::String,
    symbol_after: bool,
    symbol_spaced: bool,
}

#[cfg(feature = "alloc")]
#[derive(Debug, Clone)]
struct PercentPatternOwned {
    symbol: alloc::string::String,
    symbol_after: bool,
    symbol_spaced: bool,
}

#[cfg(feature = "alloc")]
impl NumberSectionBuilder {
    /// Fresh, empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the pack's decimal formatting pattern.
    ///
    /// `group_separator` and `decimal_separator` are stored as UTF-8
    /// bytes; each must fit in 255 bytes (the on-wire length is
    /// `u8`), which is more than any realistic separator needs.
    pub fn set_decimal_pattern(
        &mut self,
        group_separator: &str,
        decimal_separator: &str,
        min_fraction: u8,
        max_fraction: u8,
        primary_grouping: u8,
        secondary_grouping: u8,
    ) {
        self.decimal = Some(DecimalPatternOwned {
            group_separator: group_separator.into(),
            decimal_separator: decimal_separator.into(),
            min_fraction,
            max_fraction,
            primary_grouping,
            secondary_grouping,
        });
    }

    /// Add a currency record.
    pub fn push_currency(
        &mut self,
        iso_code: &str,
        symbol: &str,
        symbol_after: bool,
        symbol_spaced: bool,
    ) {
        self.currencies.push(CurrencyRecordOwned {
            iso_code: iso_code.into(),
            symbol: symbol.into(),
            symbol_after,
            symbol_spaced,
        });
    }

    /// Set the pack's percent format.
    pub fn set_percent(&mut self, symbol: &str, symbol_after: bool, symbol_spaced: bool) {
        self.percent = Some(PercentPatternOwned {
            symbol: symbol.into(),
            symbol_after,
            symbol_spaced,
        });
    }

    /// Encode the decimal pattern section. Returns an empty `Vec` if
    /// no pattern was set.
    #[must_use]
    pub fn decimal_bytes(&self) -> Vec<u8> {
        let Some(d) = &self.decimal else {
            return Vec::new();
        };
        let mut out = Vec::new();
        write_len_prefixed_str_u8(&mut out, &d.group_separator);
        write_len_prefixed_str_u8(&mut out, &d.decimal_separator);
        out.push(d.min_fraction);
        out.push(d.max_fraction);
        out.push(d.primary_grouping);
        out.push(d.secondary_grouping);
        out
    }

    /// Encode the currency table section. Returns an empty `Vec` if
    /// no currencies were pushed.
    #[must_use]
    pub fn currency_bytes(&self) -> Vec<u8> {
        if self.currencies.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        let count = u16::try_from(self.currencies.len()).unwrap_or(u16::MAX);
        out.extend_from_slice(&count.to_le_bytes());
        for c in &self.currencies {
            write_len_prefixed_str_u8(&mut out, &c.iso_code);
            write_len_prefixed_str_u8(&mut out, &c.symbol);
            let mut flags: u8 = 0;
            if c.symbol_after {
                flags |= 0x01;
            }
            if c.symbol_spaced {
                flags |= 0x02;
            }
            out.push(flags);
        }
        out
    }

    /// Encode the percent-format section. Returns an empty `Vec` if
    /// no percent format was set.
    #[must_use]
    pub fn percent_bytes(&self) -> Vec<u8> {
        let Some(p) = &self.percent else {
            return Vec::new();
        };
        let mut out = Vec::new();
        write_len_prefixed_str_u8(&mut out, &p.symbol);
        let mut flags: u8 = 0;
        if p.symbol_after {
            flags |= 0x01;
        }
        if p.symbol_spaced {
            flags |= 0x02;
        }
        out.push(flags);
        out
    }
}

#[cfg(feature = "alloc")]
fn write_len_prefixed_str_u8(out: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    let len = u8::try_from(bytes.len()).unwrap_or(u8::MAX);
    out.push(len);
    out.extend_from_slice(&bytes[..usize::from(len)]);
}

/// Builds the byte-encoded sections that a date/time-formatting SCUD
/// pack contains.
///
/// The builder collects patterns for the four CLDR length classes
/// (`Short`, `Medium`, `Long`, `Full`) plus month / weekday / era /
/// AM-PM name tables, and encodes each into a caller-selected SCUD
/// section. Sections that no caller ever set write out as empty
/// byte vectors, so a partially-populated pack round-trips through
/// the reader as `None` for the missing accessors.
#[cfg(feature = "alloc")]
#[derive(Default)]
pub struct DateTimeSectionBuilder {
    date_patterns: [Option<alloc::string::String>; 4],
    time_patterns: [Option<alloc::string::String>; 4],
    month_names: Option<[alloc::string::String; 12]>,
    month_abbr: Option<[alloc::string::String; 12]>,
    weekday_names: Option<[alloc::string::String; 7]>,
    weekday_abbr: Option<[alloc::string::String; 7]>,
    am: Option<alloc::string::String>,
    pm: Option<alloc::string::String>,
    era_bc: Option<alloc::string::String>,
    era_ad: Option<alloc::string::String>,
}

#[cfg(feature = "alloc")]
impl DateTimeSectionBuilder {
    /// Fresh, empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the date pattern for the given length. Overwrites any
    /// previously-set pattern at that length.
    pub fn set_date_pattern(&mut self, length: DateTimeLength, pattern: &str) {
        self.date_patterns[usize::from(length.as_u8())] = Some(pattern.into());
    }

    /// Set the time pattern for the given length.
    pub fn set_time_pattern(&mut self, length: DateTimeLength, pattern: &str) {
        self.time_patterns[usize::from(length.as_u8())] = Some(pattern.into());
    }

    /// Set the full month-name table (January .. December).
    pub fn set_month_names(&mut self, names: [&str; 12]) {
        self.month_names = Some(names.map(alloc::string::String::from));
    }

    /// Set the abbreviated month-name table (Jan .. Dec).
    pub fn set_month_abbreviations(&mut self, names: [&str; 12]) {
        self.month_abbr = Some(names.map(alloc::string::String::from));
    }

    /// Set the full weekday-name table (Sunday .. Saturday).
    pub fn set_weekday_names(&mut self, names: [&str; 7]) {
        self.weekday_names = Some(names.map(alloc::string::String::from));
    }

    /// Set the abbreviated weekday-name table (Sun .. Sat).
    pub fn set_weekday_abbreviations(&mut self, names: [&str; 7]) {
        self.weekday_abbr = Some(names.map(alloc::string::String::from));
    }

    /// Set the AM and PM markers.
    pub fn set_am_pm(&mut self, am: &str, pm: &str) {
        self.am = Some(am.into());
        self.pm = Some(pm.into());
    }

    /// Set the BC and AD era names.
    pub fn set_eras(&mut self, bc: &str, ad: &str) {
        self.era_bc = Some(bc.into());
        self.era_ad = Some(ad.into());
    }

    /// Encode the date-patterns section. Writes exactly four
    /// consecutive length-prefixed strings — an unset length emits
    /// an empty entry (`u8 0`) so the wire layout stays index-based.
    /// Returns an empty `Vec` when no date patterns were set at all.
    #[must_use]
    pub fn date_patterns_bytes(&self) -> Vec<u8> {
        if self.date_patterns.iter().all(Option::is_none) {
            return Vec::new();
        }
        let mut out = Vec::new();
        for slot in &self.date_patterns {
            let s = slot.as_deref().unwrap_or("");
            write_len_prefixed_str_u8(&mut out, s);
        }
        out
    }

    /// Encode the time-patterns section. Same shape as
    /// [`date_patterns_bytes`](Self::date_patterns_bytes).
    #[must_use]
    pub fn time_patterns_bytes(&self) -> Vec<u8> {
        if self.time_patterns.iter().all(Option::is_none) {
            return Vec::new();
        }
        let mut out = Vec::new();
        for slot in &self.time_patterns {
            let s = slot.as_deref().unwrap_or("");
            write_len_prefixed_str_u8(&mut out, s);
        }
        out
    }

    /// Encode the full month-name section. Returns an empty `Vec` if
    /// [`set_month_names`](Self::set_month_names) was never called.
    #[must_use]
    pub fn month_names_bytes(&self) -> Vec<u8> {
        encode_counted_strings_12(self.month_names.as_ref())
    }

    /// Encode the abbreviated month-name section.
    #[must_use]
    pub fn month_abbr_bytes(&self) -> Vec<u8> {
        encode_counted_strings_12(self.month_abbr.as_ref())
    }

    /// Encode the full weekday-name section.
    #[must_use]
    pub fn weekday_names_bytes(&self) -> Vec<u8> {
        encode_counted_strings_7(self.weekday_names.as_ref())
    }

    /// Encode the abbreviated weekday-name section.
    #[must_use]
    pub fn weekday_abbr_bytes(&self) -> Vec<u8> {
        encode_counted_strings_7(self.weekday_abbr.as_ref())
    }

    /// Encode the AM/PM section as two consecutive length-prefixed
    /// strings.
    #[must_use]
    pub fn am_pm_bytes(&self) -> Vec<u8> {
        let (Some(am), Some(pm)) = (self.am.as_deref(), self.pm.as_deref()) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        write_len_prefixed_str_u8(&mut out, am);
        write_len_prefixed_str_u8(&mut out, pm);
        out
    }

    /// Encode the era section as two consecutive length-prefixed
    /// strings.
    #[must_use]
    pub fn era_names_bytes(&self) -> Vec<u8> {
        let (Some(bc), Some(ad)) = (self.era_bc.as_deref(), self.era_ad.as_deref()) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        write_len_prefixed_str_u8(&mut out, bc);
        write_len_prefixed_str_u8(&mut out, ad);
        out
    }
}

/// Builds the byte-encoded sections that a break-iteration SCUD pack
/// contains.
///
/// Accepts three kinds of input: (a) per-class range tables as
/// `(start, length, class_byte)` triples for graphemes, words, and
/// sentences; (b) optional per-axis rule-id bytes (default: none).
/// A caller that wants the pack's readers to fall through to the
/// algorithm crate's built-in default classification simply omits
/// the ranges and calls [`Self::set_default_rules`], which stamps
/// the well-known [`RULES_UAX29_DEFAULT`] rule id on each axis.
///
/// # Wire format
///
/// Each range table is encoded as `u32 count` + `count * 9` bytes,
/// with the 9 bytes per record being little-endian `(u32 start,
/// u32 length, u8 class)`. The writer sorts ranges by `start` for
/// binary-search correctness on the reader side.
#[cfg(feature = "alloc")]
#[derive(Default)]
pub struct BreakSectionBuilder {
    grapheme_ranges: Vec<(u32, u32, u8)>,
    word_ranges: Vec<(u32, u32, u8)>,
    sentence_ranges: Vec<(u32, u32, u8)>,
    grapheme_rules: Option<u8>,
    word_rules: Option<u8>,
    sentence_rules: Option<u8>,
    word_dict: Vec<alloc::vec::Vec<u8>>,
}

#[cfg(feature = "alloc")]
impl BreakSectionBuilder {
    /// Fresh, empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a `(start, length, class)` grapheme-class range.
    ///
    /// The range covers scalars `start..start + length`. Overlapping
    /// or duplicated ranges are not validated; the writer sorts by
    /// `start` and the reader picks whichever comes first — callers
    /// author their tables so ranges do not overlap.
    pub fn push_grapheme_range(&mut self, start: u32, length: u32, class: GraphemeClass) {
        self.grapheme_ranges.push((start, length, class.as_u8()));
    }

    /// Push a `(start, length, class)` word-class range.
    pub fn push_word_range(&mut self, start: u32, length: u32, class: WordClass) {
        self.word_ranges.push((start, length, class.as_u8()));
    }

    /// Push a `(start, length, class)` sentence-class range.
    pub fn push_sentence_range(&mut self, start: u32, length: u32, class: SentenceClass) {
        self.sentence_ranges.push((start, length, class.as_u8()));
    }

    /// Stamp the well-known UAX #29 default rule id on all three
    /// axes. Callers who want the algorithm crate's built-in
    /// grapheme/word/sentence rules to apply reach for this
    /// convenience.
    pub fn set_default_rules(&mut self) {
        self.grapheme_rules = Some(RULES_UAX29_DEFAULT);
        self.word_rules = Some(RULES_UAX29_DEFAULT);
        self.sentence_rules = Some(RULES_UAX29_DEFAULT);
    }

    /// Set a specific rule-id byte for grapheme rules. Overrides
    /// any prior call to [`Self::set_default_rules`].
    pub fn set_grapheme_rules(&mut self, id: u8) {
        self.grapheme_rules = Some(id);
    }

    /// Set a specific rule-id byte for word rules.
    pub fn set_word_rules(&mut self, id: u8) {
        self.word_rules = Some(id);
    }

    /// Set a specific rule-id byte for sentence rules.
    pub fn set_sentence_rules(&mut self, id: u8) {
        self.sentence_rules = Some(id);
    }

    /// Encode the grapheme-class range section. Returns an empty
    /// `Vec` when no ranges were pushed.
    #[must_use]
    pub fn grapheme_classes_bytes(&self) -> Vec<u8> {
        encode_range_table(&self.grapheme_ranges)
    }

    /// Encode the word-class range section.
    #[must_use]
    pub fn word_classes_bytes(&self) -> Vec<u8> {
        encode_range_table(&self.word_ranges)
    }

    /// Encode the sentence-class range section.
    #[must_use]
    pub fn sentence_classes_bytes(&self) -> Vec<u8> {
        encode_range_table(&self.sentence_ranges)
    }

    /// Encode the grapheme rule id, or an empty `Vec` if none set.
    #[must_use]
    pub fn grapheme_rules_bytes(&self) -> Vec<u8> {
        match self.grapheme_rules {
            Some(id) => alloc::vec![id],
            None => Vec::new(),
        }
    }

    /// Encode the word rule id, or an empty `Vec` if none set.
    #[must_use]
    pub fn word_rules_bytes(&self) -> Vec<u8> {
        match self.word_rules {
            Some(id) => alloc::vec![id],
            None => Vec::new(),
        }
    }

    /// Encode the sentence rule id, or an empty `Vec` if none set.
    #[must_use]
    pub fn sentence_rules_bytes(&self) -> Vec<u8> {
        match self.sentence_rules {
            Some(id) => alloc::vec![id],
            None => Vec::new(),
        }
    }

    /// Push a dictionary entry for the CJK word-break dictionary
    /// (see [`SECT_WORD_DICT`]).
    ///
    /// Duplicate entries are de-duplicated on encode. Empty inputs
    /// are ignored. Each entry must be well-formed UTF-8 in
    /// practice; the SCUD writer does not enforce that (the reader
    /// treats entries as opaque bytes and hands them back to the
    /// segment engine which requires char-boundary alignment against
    /// its input).
    pub fn push_dict_entry(&mut self, word: &str) {
        if word.is_empty() {
            return;
        }
        self.word_dict.push(word.as_bytes().to_vec());
    }

    /// Encode the CJK word-break dictionary section (see
    /// [`SECT_WORD_DICT`]). Returns an empty `Vec` when no entries
    /// were pushed.
    ///
    /// # Panics
    ///
    /// Panics if the dictionary carries more than `u32::MAX` entries
    /// or a single entry exceeds `u16::MAX` bytes. Both bounds are
    /// wire-format serialisation invariants no realistic dictionary
    /// can violate.
    #[must_use]
    pub fn word_dict_bytes(&self) -> Vec<u8> {
        if self.word_dict.is_empty() {
            return Vec::new();
        }
        // Sort + dedup so binary search over the offset table is
        // well-defined and the wire size matches the entry count.
        let mut sorted: Vec<&alloc::vec::Vec<u8>> = self.word_dict.iter().collect();
        sorted.sort();
        sorted.dedup();
        let count = u32::try_from(sorted.len()).expect("dict size fits u32");
        let max_word_len_bytes = sorted.iter().map(|w| w.len()).max().unwrap_or(0);
        let max_word_len =
            u16::try_from(max_word_len_bytes).expect("longest dictionary word fits u16 bytes");

        // Build the offset index and payload region in tandem.
        let mut payload: Vec<u8> = Vec::new();
        let mut index: Vec<u8> = Vec::with_capacity(sorted.len() * 4);
        for w in &sorted {
            let off = u32::try_from(payload.len()).expect("dictionary payload offset fits u32");
            index.extend_from_slice(&off.to_le_bytes());
            let len = u16::try_from(w.len()).expect("dictionary word length fits u16");
            payload.extend_from_slice(&len.to_le_bytes());
            payload.extend_from_slice(w);
        }

        let mut out = Vec::with_capacity(8 + index.len() + payload.len());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&max_word_len.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // reserved
        out.extend_from_slice(&index);
        out.extend_from_slice(&payload);
        out
    }

    /// Number of unique dictionary entries currently held by the
    /// builder (after implicit de-duplication).
    #[must_use]
    pub fn dict_entry_count(&self) -> usize {
        let mut sorted: Vec<&alloc::vec::Vec<u8>> = self.word_dict.iter().collect();
        sorted.sort();
        sorted.dedup();
        sorted.len()
    }
}

/// Builds the byte-encoded sections that a line-break (UAX #14)
/// SCUD pack contains. Sorts input entries by source scalar so the
/// reader's binary search stays valid.
///
/// Wire shape mirrors [`BreakSectionBuilder`]: a `u32 count` prefix
/// followed by `(u32 start, u32 length, u8 class)` fixed-width
/// records. The rule section stores a single `u8` id; the tailoring
/// section stores a single `u8` strictness byte.
#[cfg(feature = "alloc")]
#[derive(Default)]
pub struct LineBreakSectionBuilder {
    ranges: Vec<(u32, u32, u8)>,
    rules: Option<u8>,
    strictness: Option<u8>,
}

#[cfg(feature = "alloc")]
impl LineBreakSectionBuilder {
    /// Fresh, empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a `(start, length, class)` line-break class range.
    pub fn push_range(&mut self, start: u32, length: u32, class: LineBreakClass) {
        self.ranges.push((start, length, class.as_u8()));
    }

    /// Stamp the well-known UAX #14 default rule id.
    pub fn set_default_rules(&mut self) {
        self.rules = Some(RULES_UAX14_DEFAULT);
    }

    /// Set a specific rule-id byte. Overrides [`Self::set_default_rules`].
    pub fn set_rules(&mut self, id: u8) {
        self.rules = Some(id);
    }

    /// Set the pack's CJK strictness tag (see [`LB_STRICTNESS_LOOSE`],
    /// [`LB_STRICTNESS_NORMAL`], [`LB_STRICTNESS_STRICT`]).
    pub fn set_strictness(&mut self, strictness: u8) {
        self.strictness = Some(strictness);
    }

    /// Encode the class range section. Returns an empty `Vec` when
    /// no ranges were pushed.
    #[must_use]
    pub fn classes_bytes(&self) -> Vec<u8> {
        encode_range_table(&self.ranges)
    }

    /// Encode the rule id, or an empty `Vec` if none set.
    #[must_use]
    pub fn rules_bytes(&self) -> Vec<u8> {
        match self.rules {
            Some(id) => alloc::vec![id],
            None => Vec::new(),
        }
    }

    /// Encode the strictness tag, or an empty `Vec` if none set.
    #[must_use]
    pub fn tailorings_bytes(&self) -> Vec<u8> {
        match self.strictness {
            Some(s) => alloc::vec![s],
            None => Vec::new(),
        }
    }
}

#[cfg(feature = "alloc")]
fn encode_range_table(ranges: &[(u32, u32, u8)]) -> Vec<u8> {
    if ranges.is_empty() {
        return Vec::new();
    }
    let mut sorted = ranges.to_vec();
    sorted.sort_by_key(|(s, _, _)| *s);
    let mut out = Vec::with_capacity(4 + sorted.len() * CLASS_RANGE_RECORD_BYTES);
    let count = u32::try_from(sorted.len()).unwrap_or(u32::MAX);
    out.extend_from_slice(&count.to_le_bytes());
    for (start, length, class) in sorted {
        out.extend_from_slice(&start.to_le_bytes());
        out.extend_from_slice(&length.to_le_bytes());
        out.push(class);
    }
    out
}

#[cfg(feature = "alloc")]
fn encode_counted_strings_12(names: Option<&[alloc::string::String; 12]>) -> Vec<u8> {
    let Some(names) = names else {
        return Vec::new();
    };
    let mut out = Vec::new();
    out.extend_from_slice(&12u16.to_le_bytes());
    for n in names {
        write_len_prefixed_str_u8(&mut out, n);
    }
    out
}

#[cfg(feature = "alloc")]
fn encode_counted_strings_7(names: Option<&[alloc::string::String; 7]>) -> Vec<u8> {
    let Some(names) = names else {
        return Vec::new();
    };
    let mut out = Vec::new();
    out.extend_from_slice(&7u16.to_le_bytes());
    for n in names {
        write_len_prefixed_str_u8(&mut out, n);
    }
    out
}

#[cfg(feature = "alloc")]
fn encode_pair_table(pairs: &[(u32, u32)]) -> Vec<u8> {
    let mut sorted: Vec<(u32, u32)> = pairs.to_vec();
    sorted.sort_by_key(|(s, _)| *s);
    let mut out = Vec::with_capacity(sorted.len() * 8);
    for (s, d) in sorted {
        out.extend_from_slice(&s.to_le_bytes());
        out.extend_from_slice(&d.to_le_bytes());
    }
    out
}

#[cfg(feature = "alloc")]
fn encode_full_table(rows: &[(u32, Vec<u32>)]) -> Vec<u8> {
    // Sort by src for binary-search correctness.
    let mut sorted: Vec<&(u32, Vec<u32>)> = rows.iter().collect();
    sorted.sort_by_key(|(s, _)| *s);

    let count = sorted.len();
    let mut index = Vec::with_capacity(4 + count * 8);
    let mut payloads: Vec<u8> = Vec::new();
    index.extend_from_slice(&u32::try_from(count).unwrap().to_le_bytes());
    for (src, to) in &sorted {
        let off = u32::try_from(payloads.len()).unwrap();
        index.extend_from_slice(&src.to_le_bytes());
        index.extend_from_slice(&off.to_le_bytes());
        assert!(!to.is_empty() && to.len() <= 8);
        payloads.push(u8::try_from(to.len()).unwrap());
        for scalar in to {
            payloads.extend_from_slice(&scalar.to_le_bytes());
        }
    }
    index.extend_from_slice(&payloads);
    index
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::*;

    fn build_ascii_pack() -> Vec<u8> {
        let mut case = CaseSectionBuilder::new();
        for c in 'a'..='z' {
            let upper = c.to_ascii_uppercase();
            case.push_simple_lower(upper as u32, c as u32);
            case.push_simple_upper(c as u32, upper as u32);
            case.push_simple_fold(upper as u32, c as u32);
        }
        let mut w = ScudWriter::new(CAP_CASE, "44.1", Some("en"));
        w.append_section(SECT_SIMPLE_LOWER, &case.simple_lower_bytes());
        w.append_section(SECT_SIMPLE_UPPER, &case.simple_upper_bytes());
        w.append_section(SECT_SIMPLE_FOLD, &case.simple_fold_bytes());
        w.finish()
    }

    #[test]
    fn header_roundtrip() {
        let bytes = build_ascii_pack();
        let file = ScudFile::from_slice(&bytes).unwrap();
        assert_eq!(file.magic(), MAGIC);
        assert_eq!(file.format_version(), (1, 0));
        assert_eq!(file.capability(), CAP_CASE);
        assert_eq!(file.cldr_version(), "44.1");
        assert_eq!(file.locale(), Some("en"));
    }

    #[test]
    fn simple_lower_lookup() {
        let bytes = build_ascii_pack();
        let file = ScudFile::from_slice(&bytes).unwrap();
        let view = file.as_case_data().unwrap();
        for c in 'A'..='Z' {
            let low = c.to_ascii_lowercase();
            assert_eq!(view.simple_lower(c as u32), Some(low as u32));
        }
        // Miss.
        assert_eq!(view.simple_lower('a' as u32), None);
        assert_eq!(view.simple_lower(0x2603), None);
    }

    #[test]
    fn simple_upper_lookup() {
        let bytes = build_ascii_pack();
        let file = ScudFile::from_slice(&bytes).unwrap();
        let view = file.as_case_data().unwrap();
        for c in 'a'..='z' {
            let up = c.to_ascii_uppercase();
            assert_eq!(view.simple_upper(c as u32), Some(up as u32));
        }
        assert_eq!(view.simple_upper('A' as u32), None);
    }

    #[test]
    fn full_upper_ss() {
        let mut case = CaseSectionBuilder::new();
        case.push_full_upper(0x00DF, &[0x0053, 0x0053]);
        case.push_full_fold(0x00DF, &[0x0073, 0x0073]);
        let mut w = ScudWriter::new(CAP_CASE, "44.1", Some("de"));
        w.append_section(SECT_FULL_UPPER, &case.full_upper_bytes());
        w.append_section(SECT_FULL_FOLD, &case.full_fold_bytes());
        let bytes = w.finish();
        let file = ScudFile::from_slice(&bytes).unwrap();
        let view = file.as_case_data().unwrap();
        let mapping = view.full_upper(0x00DF).expect("ß has a full-upper");
        let chars: alloc::vec::Vec<char> = mapping.chars().collect();
        assert_eq!(chars, alloc::vec!['S', 'S']);
    }

    #[test]
    fn contextual_lookup_iter() {
        let mut case = CaseSectionBuilder::new();
        case.push_context('I' as u32, ContextKind::LocaleOverrideLower, 0x0131);
        case.push_context('i' as u32, ContextKind::LocaleOverrideUpper, 0x0130);
        let mut w = ScudWriter::new(CAP_CASE, "44.1", Some("tr"));
        w.append_section(SECT_CONTEXT, &case.context_bytes());
        let bytes = w.finish();
        let file = ScudFile::from_slice(&bytes).unwrap();
        let view = file.as_case_data().unwrap();
        let hits: alloc::vec::Vec<(ContextKind, u32)> = view.contextual('I' as u32).collect();
        assert_eq!(
            hits,
            alloc::vec![(ContextKind::LocaleOverrideLower, 0x0131)]
        );
        let hits: alloc::vec::Vec<(ContextKind, u32)> = view.contextual('i' as u32).collect();
        assert_eq!(
            hits,
            alloc::vec![(ContextKind::LocaleOverrideUpper, 0x0130)]
        );
    }

    #[test]
    fn rejects_bad_magic() {
        let mut bad = alloc::vec![0u8; 20];
        bad[0] = b'X';
        assert_eq!(ScudFile::from_slice(&bad).err(), Some(ScudError::NotScud));
    }

    #[test]
    fn rejects_unsupported_major() {
        let mut bytes = build_ascii_pack();
        bytes[4] = 99;
        bytes[5] = 0;
        assert_eq!(
            ScudFile::from_slice(&bytes).err(),
            Some(ScudError::UnsupportedMajorVersion {
                file: 99,
                supported: SUPPORTED_FORMAT_MAJOR,
            }),
        );
    }

    #[test]
    fn rejects_unknown_capability() {
        let mut bytes = build_ascii_pack();
        bytes[12..16].copy_from_slice(b"XXXX");
        assert!(matches!(
            ScudFile::from_slice(&bytes).err(),
            Some(ScudError::UnsupportedCapability { .. }),
        ));
    }

    #[test]
    fn rejects_truncated_header() {
        let bytes = build_ascii_pack();
        assert_eq!(
            ScudFile::from_slice(&bytes[..10]).err(),
            Some(ScudError::HeaderTruncated),
        );
    }

    #[test]
    fn rejects_body_compression() {
        let mut bytes = build_ascii_pack();
        // Set BROTLI flag (bit 0).
        bytes[8] |= 0x01;
        assert_eq!(
            ScudFile::from_slice(&bytes).err(),
            Some(ScudError::UnsupportedCompression),
        );
    }

    #[test]
    fn section_iter_walks_all() {
        let bytes = build_ascii_pack();
        let file = ScudFile::from_slice(&bytes).unwrap();
        let reader = SectionReader::new(file.body());
        let ids: alloc::vec::Vec<[u8; 4]> = reader.iter().map(|(id, _)| id).collect();
        assert!(ids.contains(&SECT_SIMPLE_LOWER));
        assert!(ids.contains(&SECT_SIMPLE_UPPER));
        assert!(ids.contains(&SECT_SIMPLE_FOLD));
    }

    fn build_de_phonebook_collation_pack() -> Vec<u8> {
        let mut c = CollationSectionBuilder::new();
        c.push_expansion(0x00DF, &[0x0073, 0x0073]); // ß → ss
        c.push_expansion(0x00E4, &[0x0061, 0x0065]); // ä → ae
        c.push_expansion(0x00F6, &[0x006F, 0x0065]); // ö → oe
        c.push_expansion(0x00FC, &[0x0075, 0x0065]); // ü → ue
        c.set_default_strength(2); // Tertiary
        c.set_case_insensitive(true);
        let mut w = ScudWriter::new(CAP_COLLATION, "44.1", Some("de"));
        w.append_section(SECT_EXPANSIONS, &c.expansion_bytes());
        w.append_section(SECT_COLLATION_OPTIONS, &c.options_bytes());
        w.finish()
    }

    #[test]
    fn collation_view_round_trips() {
        let bytes = build_de_phonebook_collation_pack();
        let file = ScudFile::from_slice(&bytes).unwrap();
        assert_eq!(file.capability(), CAP_COLLATION);
        assert_eq!(file.locale(), Some("de"));
        let view = file.as_collation_data().unwrap();
        assert!(view.has_expansions());
        assert_eq!(view.expansion_count(), 4);
        let m = view.expansion(0x00DF).expect("ß expansion");
        let chars: alloc::vec::Vec<char> = m.chars().collect();
        assert_eq!(chars, alloc::vec!['s', 's']);
        let m = view.expansion(0x00E4).expect("ä expansion");
        let chars: alloc::vec::Vec<char> = m.chars().collect();
        assert_eq!(chars, alloc::vec!['a', 'e']);
        assert_eq!(view.default_strength(), Some(2));
        assert_eq!(view.case_insensitive(), Some(true));
    }

    #[test]
    fn empty_collation_pack_is_valid() {
        let c = CollationSectionBuilder::new();
        let mut w = ScudWriter::new(CAP_COLLATION, "44.1", Some("en"));
        w.append_section(SECT_EXPANSIONS, &c.expansion_bytes());
        let bytes = w.finish();
        let file = ScudFile::from_slice(&bytes).unwrap();
        let view = file.as_collation_data().unwrap();
        assert!(!view.has_expansions());
        assert_eq!(view.expansion_count(), 0);
        assert!(view.expansion(0x00DF).is_none());
        assert_eq!(view.default_strength(), None);
    }

    #[test]
    fn as_case_data_rejects_collation_file() {
        let bytes = build_de_phonebook_collation_pack();
        let file = ScudFile::from_slice(&bytes).unwrap();
        assert!(matches!(
            file.as_case_data(),
            Err(ScudError::CapabilityMismatch { .. })
        ));
    }

    #[test]
    fn as_collation_data_rejects_case_file() {
        let bytes = build_ascii_pack();
        let file = ScudFile::from_slice(&bytes).unwrap();
        assert!(matches!(
            file.as_collation_data(),
            Err(ScudError::CapabilityMismatch { .. })
        ));
    }

    #[test]
    fn backwards_secondary_flag_round_trips() {
        let mut c = CollationSectionBuilder::new();
        c.set_default_strength(2);
        c.set_backwards_secondary(true);
        let mut w = ScudWriter::new(CAP_COLLATION, "44.1", Some("fr"));
        w.append_section(SECT_EXPANSIONS, &c.expansion_bytes());
        w.append_section(SECT_COLLATION_OPTIONS, &c.options_bytes());
        let bytes = w.finish();
        let file = ScudFile::from_slice(&bytes).unwrap();
        let view = file.as_collation_data().unwrap();
        assert_eq!(view.backwards_secondary(), Some(true));
        assert_eq!(view.default_strength(), Some(2));
        // A pack that doesn't set the flag reads as Some(false) when
        // any option is present.
        assert_eq!(view.case_insensitive(), Some(false));
    }

    #[test]
    fn backwards_secondary_absent_when_no_options() {
        let c = CollationSectionBuilder::new();
        let mut w = ScudWriter::new(CAP_COLLATION, "44.1", Some("fr"));
        w.append_section(SECT_EXPANSIONS, &c.expansion_bytes());
        // No options section written at all.
        let bytes = w.finish();
        let file = ScudFile::from_slice(&bytes).unwrap();
        let view = file.as_collation_data().unwrap();
        assert_eq!(view.backwards_secondary(), None);
    }

    #[test]
    fn primary_overrides_round_trip() {
        let mut c = CollationSectionBuilder::new();
        // Push out-of-order to check the writer sorts by cp.
        c.push_primary_override(0x0131, 190, 0, 0); // dotless-ı
        c.push_primary_override(0x0069, 200, 0, 0); // i
        c.push_primary_override(0x0068, 180, 0, 0); // h
        let mut w = ScudWriter::new(CAP_COLLATION, "44.1", Some("tr"));
        w.append_section(SECT_EXPANSIONS, &c.expansion_bytes());
        w.append_section(SECT_PRIMARY_OVERRIDES, &c.primary_overrides_bytes());
        let bytes = w.finish();
        let file = ScudFile::from_slice(&bytes).unwrap();
        let view = file.as_collation_data().unwrap();
        assert!(view.has_primary_overrides());
        assert_eq!(view.primary_override_count(), 3);
        assert_eq!(view.primary_override(0x0068), Some((180, 0, 0)));
        assert_eq!(view.primary_override(0x0131), Some((190, 0, 0)));
        assert_eq!(view.primary_override(0x0069), Some((200, 0, 0)));
        assert_eq!(view.primary_override(0x006A), None); // not in table
    }

    #[test]
    fn primary_overrides_absent_by_default() {
        let bytes = build_de_phonebook_collation_pack();
        let file = ScudFile::from_slice(&bytes).unwrap();
        let view = file.as_collation_data().unwrap();
        assert!(!view.has_primary_overrides());
        assert_eq!(view.primary_override_count(), 0);
        assert!(view.primary_override(0x0131).is_none());
    }

    fn build_test_plural_en_pack() -> Vec<u8> {
        let mut p = PluralSectionBuilder::new();
        // English cardinals: `one` when i == 1 && v == 0, else other.
        p.push_cardinal(PluralCategory::One, 1);
        // English ordinals: one (n % 10 == 1 && n % 100 != 11),
        //                   two (n % 10 == 2 && n % 100 != 12),
        //                   few (n % 10 == 3 && n % 100 != 13),
        //                   other otherwise.
        p.push_ordinal(PluralCategory::One, 10);
        p.push_ordinal(PluralCategory::Two, 11);
        p.push_ordinal(PluralCategory::Few, 12);
        let mut w = ScudWriter::new(CAP_PLURAL, "44.1", Some("en"));
        w.append_section(SECT_CARDINAL_RULES, &p.cardinal_bytes());
        w.append_section(SECT_ORDINAL_RULES, &p.ordinal_bytes());
        w.finish()
    }

    #[test]
    fn plural_pack_round_trips() {
        let bytes = build_test_plural_en_pack();
        let file = ScudFile::from_slice(&bytes).unwrap();
        assert_eq!(file.capability(), CAP_PLURAL);
        assert_eq!(file.locale(), Some("en"));
        let view = file.as_plural_data().unwrap();
        let cardinals: Vec<_> = view.cardinal_rules().collect();
        assert_eq!(cardinals, alloc::vec![(PluralCategory::One, 1)]);
        let ordinals: Vec<_> = view.ordinal_rules().collect();
        assert_eq!(
            ordinals,
            alloc::vec![
                (PluralCategory::One, 10),
                (PluralCategory::Two, 11),
                (PluralCategory::Few, 12),
            ]
        );
        assert!(view.has_cardinal_rules());
        assert!(view.has_ordinal_rules());
    }

    #[test]
    fn plural_category_round_trips() {
        for c in [
            PluralCategory::Zero,
            PluralCategory::One,
            PluralCategory::Two,
            PluralCategory::Few,
            PluralCategory::Many,
            PluralCategory::Other,
        ] {
            assert_eq!(PluralCategory::from_u8(c.as_u8()), Some(c));
        }
        assert_eq!(PluralCategory::from_u8(99), None);
    }

    fn build_test_number_en_pack() -> Vec<u8> {
        let mut n = NumberSectionBuilder::new();
        n.set_decimal_pattern(",", ".", 0, 3, 3, 3);
        n.push_currency("USD", "$", false, false);
        n.push_currency("EUR", "\u{20AC}", true, true);
        n.set_percent("%", true, false);
        let mut w = ScudWriter::new(CAP_NUMBER, "44.1", Some("en"));
        w.append_section(SECT_DECIMAL_PATTERN, &n.decimal_bytes());
        w.append_section(SECT_CURRENCY_TABLE, &n.currency_bytes());
        w.append_section(SECT_PERCENT_PATTERN, &n.percent_bytes());
        w.finish()
    }

    #[test]
    fn number_pack_round_trips() {
        let bytes = build_test_number_en_pack();
        let file = ScudFile::from_slice(&bytes).unwrap();
        assert_eq!(file.capability(), CAP_NUMBER);
        let view = file.as_number_data().unwrap();
        let d = view.decimal_pattern().unwrap();
        assert_eq!(d.group_separator, ",");
        assert_eq!(d.decimal_separator, ".");
        assert_eq!(d.primary_grouping, 3);
        assert_eq!(d.secondary_grouping, 3);
        assert_eq!(d.max_fraction, 3);
        let usd = view.currency("USD").unwrap();
        assert_eq!(usd.symbol, "$");
        assert!(!usd.symbol_after);
        assert!(!usd.symbol_spaced);
        let eur = view.currency("EUR").unwrap();
        assert_eq!(eur.symbol, "\u{20AC}");
        assert!(eur.symbol_after);
        assert!(eur.symbol_spaced);
        assert!(view.currency("XXX").is_none());
        let p = view.percent_pattern().unwrap();
        assert_eq!(p.symbol, "%");
        assert!(p.symbol_after);
    }

    #[test]
    fn as_plural_data_rejects_case_file() {
        let bytes = build_ascii_pack();
        let file = ScudFile::from_slice(&bytes).unwrap();
        assert!(matches!(
            file.as_plural_data(),
            Err(ScudError::CapabilityMismatch { .. })
        ));
    }

    #[test]
    fn as_number_data_rejects_case_file() {
        let bytes = build_ascii_pack();
        let file = ScudFile::from_slice(&bytes).unwrap();
        assert!(matches!(
            file.as_number_data(),
            Err(ScudError::CapabilityMismatch { .. })
        ));
    }

    fn build_test_datetime_en_pack() -> Vec<u8> {
        let mut d = DateTimeSectionBuilder::new();
        d.set_date_pattern(DateTimeLength::Short, "M/d/y");
        d.set_date_pattern(DateTimeLength::Medium, "MMM d, y");
        d.set_date_pattern(DateTimeLength::Long, "MMMM d, y");
        d.set_date_pattern(DateTimeLength::Full, "EEEE, MMMM d, y");
        d.set_time_pattern(DateTimeLength::Short, "h:mm a");
        d.set_time_pattern(DateTimeLength::Medium, "h:mm:ss a");
        d.set_time_pattern(DateTimeLength::Long, "h:mm:ss a");
        d.set_time_pattern(DateTimeLength::Full, "h:mm:ss a");
        d.set_month_names([
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ]);
        d.set_month_abbreviations([
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ]);
        d.set_weekday_names([
            "Sunday",
            "Monday",
            "Tuesday",
            "Wednesday",
            "Thursday",
            "Friday",
            "Saturday",
        ]);
        d.set_weekday_abbreviations(["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]);
        d.set_am_pm("AM", "PM");
        d.set_eras("BC", "AD");
        let mut w = ScudWriter::new(CAP_DATETIME, "44.1", Some("en"));
        w.append_section(SECT_DATE_PATTERNS, &d.date_patterns_bytes());
        w.append_section(SECT_TIME_PATTERNS, &d.time_patterns_bytes());
        w.append_section(SECT_MONTH_NAMES, &d.month_names_bytes());
        w.append_section(SECT_MONTH_ABBR, &d.month_abbr_bytes());
        w.append_section(SECT_WEEKDAY_NAMES, &d.weekday_names_bytes());
        w.append_section(SECT_WEEKDAY_ABBR, &d.weekday_abbr_bytes());
        w.append_section(SECT_AM_PM, &d.am_pm_bytes());
        w.append_section(SECT_ERA_NAMES, &d.era_names_bytes());
        w.finish()
    }

    #[test]
    fn datetime_pack_round_trips() {
        let bytes = build_test_datetime_en_pack();
        let file = ScudFile::from_slice(&bytes).unwrap();
        assert_eq!(file.capability(), CAP_DATETIME);
        let view = file.as_datetime_data().unwrap();
        assert_eq!(view.date_pattern(DateTimeLength::Short), Some("M/d/y"));
        assert_eq!(view.date_pattern(DateTimeLength::Medium), Some("MMM d, y"));
        assert_eq!(view.date_pattern(DateTimeLength::Long), Some("MMMM d, y"));
        assert_eq!(
            view.date_pattern(DateTimeLength::Full),
            Some("EEEE, MMMM d, y")
        );
        assert_eq!(view.time_pattern(DateTimeLength::Short), Some("h:mm a"));
        assert_eq!(view.month_name(1), Some("January"));
        assert_eq!(view.month_name(12), Some("December"));
        assert_eq!(view.month_abbreviation(3), Some("Mar"));
        assert_eq!(view.month_name(0), None);
        assert_eq!(view.month_name(13), None);
        assert_eq!(view.weekday_name(0), Some("Sunday"));
        assert_eq!(view.weekday_name(6), Some("Saturday"));
        assert_eq!(view.weekday_abbreviation(1), Some("Mon"));
        assert_eq!(view.weekday_name(7), None);
        assert_eq!(view.am(), Some("AM"));
        assert_eq!(view.pm(), Some("PM"));
        assert_eq!(view.era_bc(), Some("BC"));
        assert_eq!(view.era_ad(), Some("AD"));
    }

    #[test]
    fn datetime_length_round_trips() {
        for len in [
            DateTimeLength::Short,
            DateTimeLength::Medium,
            DateTimeLength::Long,
            DateTimeLength::Full,
        ] {
            assert_eq!(DateTimeLength::from_u8(len.as_u8()), Some(len));
        }
        assert_eq!(DateTimeLength::from_u8(9), None);
    }

    #[test]
    fn as_datetime_data_rejects_case_file() {
        let bytes = build_ascii_pack();
        let file = ScudFile::from_slice(&bytes).unwrap();
        assert!(matches!(
            file.as_datetime_data(),
            Err(ScudError::CapabilityMismatch { .. })
        ));
    }

    fn build_test_break_default_pack() -> Vec<u8> {
        let mut b = BreakSectionBuilder::new();
        // Two ranges so binary search is exercised: ASCII 'a'..='z'
        // is ALetter, U+200D is ZWJ.
        b.push_word_range('a' as u32, 26, WordClass::ALetter);
        b.push_word_range(0x200D, 1, WordClass::Zwj);
        b.push_grapheme_range(0x000D, 1, GraphemeClass::Cr);
        b.push_grapheme_range(0x1F1E6, 26, GraphemeClass::RegionalIndicator);
        b.push_sentence_range('.' as u32, 1, SentenceClass::ATerm);
        b.set_default_rules();
        let mut w = ScudWriter::new(CAP_BREAK, "44.1", Some(""));
        w.append_section(SECT_GRAPHEME_CLASSES, &b.grapheme_classes_bytes());
        w.append_section(SECT_WORD_CLASSES, &b.word_classes_bytes());
        w.append_section(SECT_SENTENCE_CLASSES, &b.sentence_classes_bytes());
        w.append_section(SECT_GRAPHEME_RULES, &b.grapheme_rules_bytes());
        w.append_section(SECT_WORD_RULES, &b.word_rules_bytes());
        w.append_section(SECT_SENTENCE_RULES, &b.sentence_rules_bytes());
        w.finish()
    }

    #[test]
    fn break_pack_round_trips() {
        let bytes = build_test_break_default_pack();
        let file = ScudFile::from_slice(&bytes).unwrap();
        assert_eq!(file.capability(), CAP_BREAK);
        let view = file.as_break_data().unwrap();
        assert!(view.has_word_classes());
        assert!(view.has_grapheme_classes());
        assert!(view.has_sentence_classes());
        assert_eq!(view.word_class('a' as u32), Some(WordClass::ALetter));
        assert_eq!(view.word_class('z' as u32), Some(WordClass::ALetter));
        assert_eq!(view.word_class(0x200D), Some(WordClass::Zwj));
        assert_eq!(view.word_class('9' as u32), None);
        assert_eq!(view.grapheme_class(0x000D), Some(GraphemeClass::Cr));
        assert_eq!(
            view.grapheme_class(0x1F1E6),
            Some(GraphemeClass::RegionalIndicator),
        );
        assert_eq!(
            view.grapheme_class(0x1F1FF),
            Some(GraphemeClass::RegionalIndicator),
        );
        assert_eq!(view.grapheme_class(0x1F200), None);
        assert_eq!(view.sentence_class('.' as u32), Some(SentenceClass::ATerm));
        assert_eq!(view.grapheme_rules_id(), RULES_UAX29_DEFAULT);
        assert_eq!(view.word_rules_id(), RULES_UAX29_DEFAULT);
        assert_eq!(view.sentence_rules_id(), RULES_UAX29_DEFAULT);
    }

    #[test]
    fn break_empty_pack_reports_no_data() {
        let mut w = ScudWriter::new(CAP_BREAK, "44.1", Some(""));
        // Empty pack — every accessor returns "no data".
        let empty = BreakSectionBuilder::new();
        w.append_section(SECT_GRAPHEME_CLASSES, &empty.grapheme_classes_bytes());
        w.append_section(SECT_WORD_CLASSES, &empty.word_classes_bytes());
        w.append_section(SECT_SENTENCE_CLASSES, &empty.sentence_classes_bytes());
        w.append_section(SECT_GRAPHEME_RULES, &empty.grapheme_rules_bytes());
        w.append_section(SECT_WORD_RULES, &empty.word_rules_bytes());
        w.append_section(SECT_SENTENCE_RULES, &empty.sentence_rules_bytes());
        let bytes = w.finish();
        let file = ScudFile::from_slice(&bytes).unwrap();
        let view = file.as_break_data().unwrap();
        assert!(!view.has_grapheme_classes());
        assert!(!view.has_word_classes());
        assert!(!view.has_sentence_classes());
        assert_eq!(view.word_class('a' as u32), None);
        assert_eq!(view.grapheme_class(0x000D), None);
        assert_eq!(view.sentence_class('.' as u32), None);
        assert_eq!(view.grapheme_rules_id(), 0);
        assert_eq!(view.word_rules_id(), 0);
        assert_eq!(view.sentence_rules_id(), 0);
    }

    #[test]
    fn break_class_enum_round_trips() {
        for c in [
            GraphemeClass::Other,
            GraphemeClass::Cr,
            GraphemeClass::Lf,
            GraphemeClass::Control,
            GraphemeClass::Extend,
            GraphemeClass::Zwj,
            GraphemeClass::RegionalIndicator,
            GraphemeClass::Prepend,
            GraphemeClass::SpacingMark,
            GraphemeClass::HangulL,
            GraphemeClass::HangulV,
            GraphemeClass::HangulT,
            GraphemeClass::HangulLv,
            GraphemeClass::HangulLvt,
            GraphemeClass::ExtendedPictographic,
        ] {
            assert_eq!(GraphemeClass::from_u8(c.as_u8()), c);
        }
        assert_eq!(GraphemeClass::from_u8(200), GraphemeClass::Other);
        for c in [
            WordClass::Other,
            WordClass::Cr,
            WordClass::Lf,
            WordClass::Newline,
            WordClass::Extend,
            WordClass::Zwj,
            WordClass::RegionalIndicator,
            WordClass::Format,
            WordClass::Katakana,
            WordClass::HebrewLetter,
            WordClass::ALetter,
            WordClass::SingleQuote,
            WordClass::DoubleQuote,
            WordClass::MidNumLet,
            WordClass::MidLetter,
            WordClass::MidNum,
            WordClass::Numeric,
            WordClass::ExtendNumLet,
            WordClass::WSegSpace,
            WordClass::ExtendedPictographic,
        ] {
            assert_eq!(WordClass::from_u8(c.as_u8()), c);
        }
        for c in [
            SentenceClass::Other,
            SentenceClass::Cr,
            SentenceClass::Lf,
            SentenceClass::Extend,
            SentenceClass::Sep,
            SentenceClass::Format,
            SentenceClass::Sp,
            SentenceClass::Lower,
            SentenceClass::Upper,
            SentenceClass::OLetter,
            SentenceClass::Numeric,
            SentenceClass::ATerm,
            SentenceClass::STerm,
            SentenceClass::Close,
            SentenceClass::SContinue,
        ] {
            assert_eq!(SentenceClass::from_u8(c.as_u8()), c);
        }
    }

    #[test]
    fn as_break_data_rejects_case_file() {
        let bytes = build_ascii_pack();
        let file = ScudFile::from_slice(&bytes).unwrap();
        assert!(matches!(
            file.as_break_data(),
            Err(ScudError::CapabilityMismatch { .. })
        ));
    }

    #[test]
    fn linebreak_pack_round_trips() {
        let mut b = LineBreakSectionBuilder::new();
        b.push_range(0x0020, 1, LineBreakClass::Sp);
        b.push_range(0x000A, 1, LineBreakClass::Lf);
        b.push_range(0x000D, 1, LineBreakClass::Cr);
        b.push_range(0x0028, 1, LineBreakClass::Op);
        b.push_range(0x0029, 1, LineBreakClass::Cp);
        b.set_default_rules();
        b.set_strictness(LB_STRICTNESS_STRICT);
        let mut w = ScudWriter::new(CAP_LINEBREAK, "15.1", Some(""));
        w.append_section(SECT_LB_CLASSES, &b.classes_bytes());
        w.append_section(SECT_LB_RULES, &b.rules_bytes());
        w.append_section(SECT_LB_TAILORINGS, &b.tailorings_bytes());
        let bytes = w.finish();
        let file = ScudFile::from_slice(&bytes).unwrap();
        assert_eq!(file.capability(), CAP_LINEBREAK);
        let view = file.as_linebreak_data().unwrap();
        assert!(view.has_classes());
        assert_eq!(view.class(0x0020), Some(LineBreakClass::Sp));
        assert_eq!(view.class(0x0028), Some(LineBreakClass::Op));
        assert_eq!(view.class(0x0029), Some(LineBreakClass::Cp));
        assert_eq!(view.class(0x0041), None); // 'A' — not in pack
        assert_eq!(view.rules_id(), RULES_UAX14_DEFAULT);
        assert_eq!(view.strictness(), LB_STRICTNESS_STRICT);
    }

    #[test]
    fn linebreak_empty_pack_reports_defaults() {
        let mut w = ScudWriter::new(CAP_LINEBREAK, "15.1", Some(""));
        let empty = LineBreakSectionBuilder::new();
        w.append_section(SECT_LB_CLASSES, &empty.classes_bytes());
        w.append_section(SECT_LB_RULES, &empty.rules_bytes());
        w.append_section(SECT_LB_TAILORINGS, &empty.tailorings_bytes());
        let bytes = w.finish();
        let file = ScudFile::from_slice(&bytes).unwrap();
        let view = file.as_linebreak_data().unwrap();
        assert!(!view.has_classes());
        assert_eq!(view.rules_id(), 0);
        assert_eq!(view.strictness(), LB_STRICTNESS_NORMAL);
    }

    #[test]
    fn linebreak_class_enum_round_trips() {
        for c in [
            LineBreakClass::Xx,
            LineBreakClass::Op,
            LineBreakClass::Cl,
            LineBreakClass::Cp,
            LineBreakClass::Qu,
            LineBreakClass::Gl,
            LineBreakClass::Ns,
            LineBreakClass::Ex,
            LineBreakClass::Sy,
            LineBreakClass::Is,
            LineBreakClass::Pr,
            LineBreakClass::Po,
            LineBreakClass::Nu,
            LineBreakClass::Al,
            LineBreakClass::Hl,
            LineBreakClass::Id,
            LineBreakClass::In,
            LineBreakClass::Hy,
            LineBreakClass::Ba,
            LineBreakClass::Bb,
            LineBreakClass::B2,
            LineBreakClass::Zw,
            LineBreakClass::Cm,
            LineBreakClass::Wj,
            LineBreakClass::H2,
            LineBreakClass::H3,
            LineBreakClass::Jl,
            LineBreakClass::Jv,
            LineBreakClass::Jt,
            LineBreakClass::Ri,
            LineBreakClass::Eb,
            LineBreakClass::Em,
            LineBreakClass::Zwj,
            LineBreakClass::Cj,
            LineBreakClass::Sg,
            LineBreakClass::Ai,
            LineBreakClass::Cb,
            LineBreakClass::Bk,
            LineBreakClass::Cr,
            LineBreakClass::Lf,
            LineBreakClass::Nl,
            LineBreakClass::Sp,
            LineBreakClass::Sa,
        ] {
            assert_eq!(LineBreakClass::from_u8(c.as_u8()), c);
        }
        assert_eq!(LineBreakClass::from_u8(250), LineBreakClass::Xx);
    }

    #[test]
    fn as_linebreak_data_rejects_case_file() {
        let bytes = build_ascii_pack();
        let file = ScudFile::from_slice(&bytes).unwrap();
        assert!(matches!(
            file.as_linebreak_data(),
            Err(ScudError::CapabilityMismatch { .. })
        ));
    }

    #[test]
    fn owned_scudfile_round_trips() {
        let bytes = build_ascii_pack();
        let owned = OwnedScudFile::new(bytes.clone()).unwrap();
        let view = owned.as_view();
        assert_eq!(view.capability(), CAP_CASE);
        assert_eq!(view.locale(), Some("en"));
        assert_eq!(owned.len(), bytes.len());
    }

    // -- Word-dict tests -----------------------------------------------

    fn build_break_pack_with_dict(entries: &[&str]) -> Vec<u8> {
        let mut b = BreakSectionBuilder::new();
        b.set_default_rules();
        for e in entries {
            b.push_dict_entry(e);
        }
        let mut w = ScudWriter::new(CAP_BREAK, "44.1", Some("ja"));
        w.append_section(SECT_GRAPHEME_CLASSES, &b.grapheme_classes_bytes());
        w.append_section(SECT_WORD_CLASSES, &b.word_classes_bytes());
        w.append_section(SECT_SENTENCE_CLASSES, &b.sentence_classes_bytes());
        w.append_section(SECT_GRAPHEME_RULES, &b.grapheme_rules_bytes());
        w.append_section(SECT_WORD_RULES, &b.word_rules_bytes());
        w.append_section(SECT_SENTENCE_RULES, &b.sentence_rules_bytes());
        w.append_section(SECT_WORD_DICT, &b.word_dict_bytes());
        w.finish()
    }

    #[test]
    fn word_dict_absent_by_default() {
        let bytes = build_test_break_default_pack();
        let file = ScudFile::from_slice(&bytes).unwrap();
        let view = file.as_break_data().unwrap();
        assert!(view.word_dict().is_none());
    }

    #[test]
    fn word_dict_round_trips() {
        // Deliberately out-of-order push to exercise the writer's sort.
        let bytes = build_break_pack_with_dict(&[
            "\u{5B66}\u{751F}",                 // 学生
            "\u{79C1}",                         // 私
            "\u{6771}\u{4EAC}",                 // 東京
            "\u{6771}\u{4EAC}\u{5927}\u{5B66}", // 東京大学
        ]);
        let file = ScudFile::from_slice(&bytes).unwrap();
        let view = file.as_break_data().unwrap();
        let dict = view.word_dict().expect("dict present");
        assert_eq!(dict.len(), 4);
        // 東京大学 is 12 UTF-8 bytes (4 * 3).
        assert_eq!(dict.max_word_len_bytes(), 12);

        // Longest-prefix match on 東京大学に行きます — should pick
        // 東京大学 (4 chars, 12 bytes).
        let input = "\u{6771}\u{4EAC}\u{5927}\u{5B66}\u{306B}\u{884C}\u{304D}\u{307E}\u{3059}";
        let m = dict.longest_prefix_match(input.as_bytes());
        assert_eq!(m, Some(12));

        // Longest-prefix match on 東京タワー — should pick 東京
        // (6 bytes).
        let input = "\u{6771}\u{4EAC}\u{30BF}\u{30EF}\u{30FC}";
        let m = dict.longest_prefix_match(input.as_bytes());
        assert_eq!(m, Some(6));

        // No match on an unknown word (using a scalar that shares no
        // prefix with any dict entry). U+4E2D 中 is not in the dict.
        let input = "\u{4E2D}\u{56FD}";
        assert!(dict.longest_prefix_match(input.as_bytes()).is_none());

        // Contains lookup.
        assert!(dict.contains("\u{79C1}".as_bytes()));
        assert!(dict.contains("\u{5B66}\u{751F}".as_bytes()));
        assert!(!dict.contains("\u{4E2D}".as_bytes()));
    }

    #[test]
    fn word_dict_dedups_duplicates() {
        let mut b = BreakSectionBuilder::new();
        b.push_dict_entry("\u{79C1}");
        b.push_dict_entry("\u{79C1}");
        b.push_dict_entry("\u{5B66}\u{751F}");
        assert_eq!(b.dict_entry_count(), 2);
    }

    #[test]
    fn word_dict_empty_input_returns_none() {
        let bytes = build_break_pack_with_dict(&["\u{79C1}"]);
        let file = ScudFile::from_slice(&bytes).unwrap();
        let view = file.as_break_data().unwrap();
        let dict = view.word_dict().unwrap();
        assert!(dict.longest_prefix_match(b"").is_none());
    }

    #[test]
    fn word_dict_walks_back_past_non_prefix_neighbour() {
        // Regression: dict has "北京" and "北京大学"; input is
        // "北京很大". The binary-search "largest <=" hits the
        // non-prefix "北京大学" — the walk-back must find "北京"
        // and return 6. Same shape as the ja/zh test cases.
        let bytes = build_break_pack_with_dict(&[
            "\u{5317}\u{4EAC}",                 // 北京
            "\u{5317}\u{4EAC}\u{5927}\u{5B66}", // 北京大学
        ]);
        let file = ScudFile::from_slice(&bytes).unwrap();
        let view = file.as_break_data().unwrap();
        let dict = view.word_dict().unwrap();
        let input = "\u{5317}\u{4EAC}\u{5F88}\u{5927}"; // 北京很大
        assert_eq!(dict.longest_prefix_match(input.as_bytes()), Some(6));
    }

    #[test]
    fn word_dict_no_match_when_first_bytes_differ() {
        let bytes = build_break_pack_with_dict(&["hello", "\u{5317}\u{4EAC}"]);
        let file = ScudFile::from_slice(&bytes).unwrap();
        let view = file.as_break_data().unwrap();
        let dict = view.word_dict().unwrap();
        // Neither "hello" nor "北京" is a prefix of "world".
        assert!(dict.longest_prefix_match(b"world").is_none());
    }

    #[test]
    fn word_dict_prefers_longest() {
        // 私 (3 bytes) and 私たち (9 bytes) both match 私たちは.
        let bytes = build_break_pack_with_dict(&[
            "\u{79C1}",                 // 私
            "\u{79C1}\u{305F}\u{3061}", // 私たち
        ]);
        let file = ScudFile::from_slice(&bytes).unwrap();
        let view = file.as_break_data().unwrap();
        let dict = view.word_dict().unwrap();
        let input = "\u{79C1}\u{305F}\u{3061}\u{306F}"; // 私たちは
        assert_eq!(dict.longest_prefix_match(input.as_bytes()), Some(9));
    }
}
