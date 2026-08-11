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
//! * **Capability views (raw body)** — [`CaseDataView`] over the body
//!   bytes. The full design allows outer Brotli/Zstd compression
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
        CAP_CASE | CAP_COLLATION | CAP_PLURAL | CAP_NUMBER | CAP_DATETIME | CAP_BREAK => {}
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

    #[test]
    fn owned_scudfile_round_trips() {
        let bytes = build_ascii_pack();
        let owned = OwnedScudFile::new(bytes.clone()).unwrap();
        let view = owned.as_view();
        assert_eq!(view.capability(), CAP_CASE);
        assert_eq!(view.locale(), Some("en"));
        assert_eq!(owned.len(), bytes.len());
    }
}
