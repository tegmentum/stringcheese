# WIT-based i18n and the SCUD Data-Pack Format

Status: Design
Applies to: StringCheese 0.2 and later (design only; nothing here ships in 0.1)
Related: [DESIGN.md](../DESIGN.md), [wasm-and-wit-interface.md](./wasm-and-wit-interface.md), [type-system.md](./type-system.md), [preprocessing-pipeline.md](./preprocessing-pipeline.md), [phonetic-subsystem.md](./phonetic-subsystem.md)

The design of StringCheese's ICU-alternative internationalisation substrate — the `stringcheese-icu-*` family of WIT packages, the compressed SCUD (StringCheese Unicode Data) data-pack format, the runtime loader and locale fallback rules, and the way per-language packs (`stringcheese-en`, `stringcheese-fr`, …) plug into both. **No `stringcheese-icu-*` crates exist yet; this document fleshes out the umbrella charter's [pluggable, opt-in globalization](../DESIGN.md) commitment.**

---

## 1. Motivation

### 1.1. Why ICU is a poor fit for Wasm

International Components for Unicode (ICU4C / ICU4J) is the industry-standard i18n library and the ground truth for CLDR-derived operations. For the deployment shape StringCheese cares about, it is the wrong tool:

- **Binary size.** A full ICU4C build with default data is roughly 30 MB on disk; the data file (`icudt*.dat`) is roughly 27 MB on its own — more than a browser page's entire JavaScript budget.
- **Monolithic API surface.** Thousands of entry points across collation, formatting, transliteration, break iteration, calendars, currencies, resource bundles — with no "just do case mapping" subset. The API is Java-flavoured (a legacy of ICU's Taligent origins), which sits awkwardly across a WIT boundary.
- **Data inseparability.** ICU's data is one aggregate. Splitting it requires ICU's data-tools toolchain and still yields artifacts assuming the ICU loader's binary shape. There is no "give me only Portuguese" build path that produces a small standalone artifact.
- **Licence-aggregate propagation.** ICU4C is under the Unicode License (permissive, MIT-compatible), but its binary distribution embeds CLDR as an opaque aggregate; downstream projects inherit code + data licences together and must reproduce ICU's attribution notices verbatim.

### 1.2. Why ICU4X only partially solves it

ICU4X — the Unicode Consortium's Rust re-implementation — improves matters substantially: `no_std`, pluggable data providers, one to two orders of magnitude smaller than ICU4C in typical Wasm configurations. It is where a caller who needs *ICU semantics* should look. But it is not the whole answer:

- **Still substantial.** A full-locale ICU4X blob is single-digit MB; the code artifact for common operations is hundreds of KB to low MB. Better than ICU4C, still large for a browser tab or size-critical edge worker.
- **Still monolithic in API contract.** `icu_collator`, `icu_datetime`, … present rich Rust APIs; they are not designed to be swapped at a WIT boundary. A caller who wants "just plural rules, delivered through a wasm component the host substitutes at deploy time" is not the ICU4X target user.
- **Data-provider surface leaks ICU shape.** `DataProvider<M>` is a general abstraction, but the concrete `DataMarker` types encode ICU4X-specific assumptions about how locale data is keyed and framed. A pack that satisfies ICU4X's providers is close to committing to ICU4X's data layout.

### 1.3. What StringCheese wants that neither addresses

Three properties simultaneously:

1. **Pluggable interfaces.** Each i18n capability is a *separate* WIT package. A caller who needs only pluralisation depends on one interface. Nothing else is linked.
2. **Opt-in data packs.** Locale data is one SCUD file per (capability × locale) tuple. A caller supporting German and Japanese loads exactly six SCUD files, not a bundled aggregate.
3. **Component-model boundaries as substitution points.** Every capability is a wasm component behind a stable WIT interface, so callers can *substitute* an implementation — a bespoke collation for a domain-specific alphabet, an ICU4X-backed adapter for a specific locale — without changing calling code.

### 1.4. Non-goals

ICU parity (the 80/90/95 % is the target; callers needing historical calendar edge cases, deep transliteration graphs, or every ICU corner reach for ICU4X); translation (localising operations *on* text, not translating text itself); transliteration graphs (ICU's ruled transliterator); complex script shaping (OpenType shaping, bidi visual runs, and display-time concerns handled by HarfBuzz); regex with Unicode properties (already out of umbrella scope).

---

## 2. Which ICU capabilities we expose

The first wave is six capabilities, one crate each. Function names below use WIT `kebab-case`.

### 2.1. Case mapping — `stringcheese-icu-case`

Locale-sensitive to-lower, to-upper, to-title per UAX #21 (<https://www.unicode.org/reports/tr21/>) with CLDR tailorings (Turkish/Azerbaijani dotted/dotless *I*, Lithuanian dot-above, Greek final sigma, Dutch `ij` titlecasing).

Interface shape: `to-lower(input, locale)`, `to-upper(input, locale)`, `to-title(input, locale, options)`, `fold(input, mode)` where `fold` is locale-independent (default / Turkic).

### 2.2. Collation — `stringcheese-icu-collation`

Locale-sensitive comparison implementing the Unicode Collation Algorithm (UTS #10, <https://www.unicode.org/reports/tr10/>) with CLDR per-locale tailorings.

Interface shape: `compare(a, b, options) -> ordering`, `sort-key(input, options) -> list<u8>` (bytewise-comparable). `collation-options` selects strength (`primary`/`secondary`/`tertiary`/`quaternary`/`identical`), case-first, and numeric mode. Tailorings are CLDR's default per-locale ordering (Swedish `å ä ö` after `z`, German `ß` at `ss`, traditional Spanish `ñ` between `n` and `o`).

### 2.3. Plural rules — `stringcheese-icu-plural`

CLDR-derived plural-class classification for cardinals and ordinals (UTS #35 § 5, <https://www.unicode.org/reports/tr35/tr35-numbers.html#Language_Plural_Rules>).

Interface shape: `cardinal(operand, locale) -> plural-class`, `ordinal(operand, locale) -> plural-class`, where `plural-class = enum { zero, one, two, few, many, other }` and `plural-operand` is a record of the CLDR `n / i / v / w / f / t / c` operands.

Deliberately narrow: this interface *classifies*, it does not format. MessageFormat-style helpers live in caller code.

### 2.4. Number formatting — `stringcheese-icu-number`

Grouping separators, decimals, currency, percent, and unit formatting per CLDR.

Interface shape: `format-decimal(value, locale, options)`, `format-currency(value, currency, locale, options)`, `format-percent(value, locale, options)`. `decimal-input = variant { integer(s64), unsigned(u64), fixed(record { mantissa: s64, scale: u8 }), decimal-string(string) }` — no IEEE-754 for currency; `f64` is offered only as an explicit lossy variant. CLDR numbering systems in the first wave: `latn`, `arab`, `arabext`, `beng`, `deva`, `thai`, `hanidec`.

### 2.5. Date/time formatting — `stringcheese-icu-datetime`

CLDR date/time skeletons and patterns per locale. First wave is **calendar-neutral**: underlying calendar is Proleptic Gregorian; Islamic, Hebrew, Japanese, and Buddhist calendars ship as follow-up SCUD supplements (`stringcheese-icu-datetime-islamic`, …). Time-zone-aware operations are separate — see §2.7.

### 2.6. Break iteration — `stringcheese-icu-break`

Grapheme, word, sentence, and line boundaries per UAX #29 (<https://www.unicode.org/reports/tr29/>) and UAX #14 (<https://www.unicode.org/reports/tr14/>) with CLDR-supplied dictionaries for Thai / Lao / Khmer / Japanese / Chinese word segmentation.

Interface shape: `iterate-graphemes(input) -> list<u32>`, `iterate-words(input, locale) -> list<u32>`, `iterate-sentences(input, locale) -> list<u32>`, `iterate-lines(input, locale) -> list<u32>` — all return boundary byte offsets.

Relationship to [`stringcheese-unicode`](../../crates/stringcheese-unicode/src/lib.rs): language-neutral grapheme break rules already live there. This crate's grapheme iterator delegates when no locale-specific dictionary applies; SCUD is consulted only for word/sentence boundaries in dictionary-based locales.

### 2.7. Deferred capabilities

Explicitly out of first-wave scope: transliteration (ruled transforms), regex with Unicode properties, bidi (UAX #9), deep locale display names, time-zone-aware operations (IANA TZDB, its release cadence, calendar arithmetic across zone transitions). Each is an *addable* crate; nothing forecloses them.

---

## 3. WIT package layout

Each capability lives in its own crate that exports a WIT world, mirroring the existing [`stringcheese.wit`](../../component/wit/stringcheese.wit):

```
component/wit/
├── stringcheese.wit                   # umbrella (already exists)
├── stringcheese-icu-case.wit
├── stringcheese-icu-collation.wit
├── stringcheese-icu-plural.wit
├── stringcheese-icu-number.wit
├── stringcheese-icu-datetime.wit
└── stringcheese-icu-break.wit
```

Each `.wit` declares a package (`package stringcheese:icu-case@0.1.0;`), one or more interfaces, and a world that exports them. A caller who uses case mapping and pluralisation depends on `stringcheese-icu-case` and `stringcheese-icu-plural`, and pays for nothing else.

### 3.1. Illustrative WIT: `stringcheese-icu-case`

`stringcheese-icu-case` is the simplest capability and works well as a shape reference for the others. **Illustrative — the exact IDL will be tuned during implementation.**

```wit
// Proposed — not yet implemented.
// component/wit/icu-case/stringcheese-icu-case.wit

package stringcheese:icu-case@0.1.0;

/// Shared types. Split into its own interface so `mapping` and
/// `capabilities` can `use` the same definitions.
interface types {
    /// BCP 47 locale tag: "en", "pt-BR", "zh-Hant-HK", "az-Cyrl-AZ".
    /// The empty string denotes the CLDR root locale — no tailoring.
    type locale = string;

    /// Errors surfaced by the case-mapping interface. Case mapping is
    /// total on valid Unicode input; failure modes are structural.
    variant case-error {
        invalid-locale(string),
        locale-unavailable(string),
        unsupported-title-mode(string),
    }

    /// How aggressively to fold case. See UAX #21 § 1.3.
    enum fold-mode {
        /// Common + Simple (1:1 code point).
        simple,
        /// Common + Full (may expand: German ß -> ss).
        full,
        /// Full + Turkic tailorings in a locale-neutral context.
        full-turkic,
    }

    enum title-boundary {
        /// UAX #29 grapheme breaks: stateless, imperfect.
        graphemes,
        /// UAX #29 word breaks: correct for most Latin.
        words,
        /// Only sentence-initial words are candidates.
        sentences,
    }

    record title-options {
        boundary: title-boundary,
        lowercase-tail: bool,
    }
}

/// The core case-mapping surface. Passing locale "" requests root.
interface mapping {
    use types.{locale, case-error, fold-mode, title-options};

    to-lower: func(input: string, locale: locale) -> result<string, case-error>;
    to-upper: func(input: string, locale: locale) -> result<string, case-error>;

    to-title: func(
        input: string,
        locale: locale,
        options: title-options,
    ) -> result<string, case-error>;

    /// Locale-*independent* case folding for case-insensitive matching.
    /// Deterministic across packs.
    fold: func(input: string, mode: fold-mode) -> string;
}

/// Introspection: which tailorings does the linked component actually
/// cover? Callers use this to warn end-users that their locale falls
/// back to root behaviour.
interface capabilities {
    use types.{locale};
    supported-locales: func() -> list<locale>;
    supports: func(loc: locale) -> bool;
}

/// The world every case-mapping component exports.
world case {
    export types;
    export mapping;
    export capabilities;
}
```

The same shape — a small set of interfaces, a `capabilities` interface for introspection, a `world` binding them together — recurs across all six capability packages.

### 3.2. Composition

A caller assembles a working component by linking the capability component (`stringcheese-icu-case.wasm`) with one or more data-supplying components — usually a language pack (`stringcheese-en.wasm`) embedding the compiled SCUD blob for its locale. Composition is a `wasm-tools compose` operation (see <https://github.com/bytecodealliance/wasm-tools>); the result is one linked `.wasm` binary the host loads.

```
                +-------------------------+
                |  stringcheese-icu-case  |    exports `case` world
                |    (algorithms only)    |    imports `case-data` world
                +-----------+-------------+
                            |
                    imports | case-data
                            |
             +--------------+--------------+
             |                             |
      +------+------+              +-------+------+
      | stringch-en |              | stringch-tr  |    each exports
      |   (SCUD)    |              |    (SCUD)    |    `case-data`
      +-------------+              +--------------+
```

The algorithm component contains no locale data; it imports `case-data`. Each language pack *exports* `case-data`, backed by its embedded SCUD blob. Composition is deterministic and reproducible; a caller supporting three locales links three language-pack components alongside one algorithm component.

---

## 4. SCUD data-pack format spec

**SCUD** — StringCheese Unicode Data. An on-disk format designed for four properties:

1. **Small.** CLDR-derived tables at a fraction of ICU's size. Structural primitives + outer stream compression stack against real Unicode-table redundancy.
2. **Memory-mappable.** Fixed offsets in the header; capability data at aligned offsets in the body. A loader mmaps the file (or reads a `Vec<u8>`), then hands out zero-copy slices.
3. **Per-locale.** One SCUD file per (capability × locale) tuple. Callers pay only for the locales they load.
4. **Versioned.** Magic bytes, format version, CLDR version at fixed offsets. Loaders reject files they cannot interpret rather than misread them.

### 4.1. File layout

Byte layout, little-endian throughout (matching WebAssembly linear-memory byte order):

```
+---------+---------+---------+---------+---------+---------+
|  magic  | fmt-maj | fmt-min |  flags  |  cap-id | header- |
|  4 B    |  2 B    |  2 B    |  4 B    |  4 B    |  len 4 B|
+---------+---------+---------+---------+---------+---------+
|                     header (var-len)                      |
+-----------------------------------------------------------+
|                     body   (var-len)                      |
+-----------------------------------------------------------+
```

- **`magic`** — four ASCII bytes `S`, `C`, `U`, `D` (`0x53 0x43 0x55 0x44`). Any other prefix is not a SCUD file (`ScudError::NotScud`).
- **`fmt-maj`, `fmt-min`** — format version. Minor bumps are backward-compatible; a `fmt-maj` bump is a hard incompatibility.
- **`flags`** — bitfield. Bit 0: outer payload is Brotli. Bit 1: outer payload is Zstd. Bit 2: header is 8-byte-aligned (default). Bit 3: BCP 47 locale key follows the header. Bits 4-31: reserved, must be zero.
- **`cap-id`** — 4-byte capability tag: `CASE`, `COLL`, `PLUR`, `NUMB`, `DTFM`, `BRKI`. Extensible.
- **`header-len`** — length of the capability-specific header region.
- **Header** — capability-specific fixed structure. For case data: offsets and lengths of the range-delta tables (lower / upper / title / tailoring exceptions) plus the CLDR version string. For collation: DUCET root table offset, tailoring rules offset, reordering block offset. Offsets are relative to body start.
- **Body** — capability-specific compressed content. When bit 0 or bit 1 of `flags` is set the body is a single Brotli or Zstd stream decompressing to the "raw" body layout. When neither is set the body is the raw layout directly (useful for tests and higher-layer compression).

CLDR version is stored as an ASCII string in the header (e.g., `"44.1"`), never bit-packed, so forward compatibility with CLDR's version numbering is robust.

### 4.2. Compression primitives

The structural primitives — plus outer Brotli/Zstd — are chosen because Unicode-derived tables have highly redundant structure at several distinct scales. Each primitive targets one kind of redundancy; the outer stream compressor cleans up whatever byte-level redundancy remains.

**RangeDelta.** Unicode data is largely "range of code points → property". Stores an ascending sequence of `(start, len, value)` triples where `start` is delta-encoded against the previous range's end. For the ASCII case mapping (52 code points, two ranges) RangeDelta compresses to ~12 bytes vs 208 bytes for `[u32; 52]`; for full Unicode `White_Space` (26 code points, 11 ranges) it produces ~50 bytes vs ~28 KiB for a sparse table over the BMP.

**AdaptivePages.** Unicode is a 21-bit space (17 planes × 65 536). Most planes are almost entirely absent for most properties; where a property does live, it typically covers most of a 4 KiB page uniformly. AdaptivePages partitions the code space into 4 096-code-point pages; each page carries one of `EMPTY` (property absent), `UNIFORM(v)` (property equals `v` throughout), `BITMAP(len)` (low-cardinality boolean/enum, bit-packed), or `INDIRECT(offset)` (unusual page; body is a RangeDelta or raw table at the given body offset). An 8 KiB overhead plus per-page storage covers the whole 21-bit space.

**PackedIntegers.** Variable-width integer encoding tuned for Unicode-property distributions. LEB128 (<https://webassembly.github.io/spec/core/binary/values.html#binary-int>) is the general fallback; SCUD extends it with a two-byte fast path for the common BMP range and a grouped run-length option so a table of general categories doesn't spend 8 bits per code point. The wire format identifies which packing was used so a decoder never has to guess.

**SequencePool.** Interns fixed-width integer sequences (typically `Vec<u32>` of Unicode scalars). Case mapping is the canonical example: dozens of locales share the same lowercase-of-`Ω` mapping. Entries referenced by byte offset. Within a single SCUD file the pool deduplicates within-file redundancy; across files, a "root" SCUD's pool covers ubiquitous mappings and per-locale files reference it by well-known offsets.

**StringPool.** UTF-8 payloads (locale display names, currency symbols, month names) with prefix compression in blocks of 16, front-coded — borrows from FST-style string dictionaries (Lucene, <https://lucene.apache.org/core/8_0_0/core/org/apache/lucene/util/fst/package-summary.html>) and *Managing Gigabytes* (Witten, Moffat, & Bell 1999). Block granularity trades a small amount of decompression for random-access binary search: locate the block, decompress it, walk to the offset. The loader never decompresses the whole pool.

**LoudsTrie.** Locale keys, property names, and enum labels are strings with high prefix redundancy. LoudsTrie stores them as a LOUDS-encoded trie (Jacobson 1989, <https://doi.org/10.1109/SFCS.1989.63533>): trie shape is one bit per node (level-order unary degree sequence), labels one byte per edge, terminator bitmap marks final states. Typical shrink factor over `HashMap<String, T>` is 10-100× at the cost of `O(k)` navigation per query, both fitting in-place in the mmap.

**FiniteStateTable.** For morphology-shaped mappings (case folding with contextual tailorings like Greek final sigma; UAX #29 word-break state tables) SCUD uses a finite-state transducer — the flat-transition-table + terminal-state-bitmap layout used by Lucene's `FST<T>` and Aoe (1989) double-array tries (<https://doi.org/10.1109/32.31365>). Load `O(1)`; step `O(1)`; match `O(k)` in input length.

**Outer stream compression.** After the structural primitives shape the body, a final byte-level pass with **Brotli** (RFC 7932, <https://datatracker.ietf.org/doc/html/rfc7932>) or **Zstd** (RFC 8878, <https://datatracker.ietf.org/doc/html/rfc8878>) recovers byte-level redundancy. Both have excellent compression *and* small decoder implementations (Brotli's decoder is under 100 KiB compiled; `ruzstd` comparable). Brotli is the default; the choice lives in file flags.

### 4.3. Loader API

The loader is one crate (`stringcheese-scud`) shared by all six capability crates.

```rust
// Proposed — not yet implemented.
// crates/stringcheese-scud/src/lib.rs

pub struct ScudFile {
    bytes: ScudBytes,
    header: ScudHeader,   // parsed once at open time
}

enum ScudBytes {
    #[cfg(feature = "std")]
    Mmap(memmap2::Mmap),
    Owned(alloc::vec::Vec<u8>),
    /// Wasm callers pass `include_bytes!` output. This variant
    /// avoids a copy.
    Static(&'static [u8]),
}

#[derive(Debug)]
pub enum ScudError {
    NotScud,
    UnsupportedMajorVersion { file: u16, supported: u16 },
    UnsupportedCapability { got: [u8; 4] },
    HeaderTruncated,
    BodyTruncated,
    OuterDecompressionFailed(&'static str),
    ChecksumMismatch,
}

impl ScudFile {
    #[cfg(feature = "std")]
    pub fn open(path: &std::path::Path) -> Result<Self, ScudError> { todo!() }
    pub fn from_bytes(bytes: &'static [u8]) -> Result<Self, ScudError> { todo!() }

    pub fn magic(&self) -> [u8; 4] { *b"SCUD" }
    pub fn format_version(&self) -> (u16, u16) { todo!() }
    pub fn cldr_version(&self) -> &str { todo!() }
    pub fn capability(&self) -> [u8; 4] { todo!() }
    pub fn locale(&self) -> Option<&str> { todo!() }

    /// Zero-copy view; returns None if this file is a different capability.
    pub fn as_case_data(&self) -> Option<CaseDataView<'_>> { todo!() }
    pub fn as_collation_data(&self) -> Option<CollationDataView<'_>> { todo!() }
    pub fn as_plural_data(&self) -> Option<PluralDataView<'_>> { todo!() }
    pub fn as_number_data(&self) -> Option<NumberDataView<'_>> { todo!() }
    pub fn as_datetime_data(&self) -> Option<DateTimeDataView<'_>> { todo!() }
    pub fn as_break_data(&self) -> Option<BreakDataView<'_>> { todo!() }
}
```

Header parsing is `O(1)`; capability-projection views are zero-copy references into the mmap. Data access — a case-mapping lookup, a collation-key computation — pays only the CPU cost of walking the encoded structure, never an allocation for setup.

The type accepts both mmap-backed and `include_bytes!`-embedded input, because language packs typically embed SCUD bytes as a `&'static [u8]` constant. That eliminates the "where is the data file on disk?" problem in Wasm and in single-file host binaries; the trade-off is the compressed pack lives in the binary rather than beside it.

---

## 5. Runtime library

`stringcheese-scud` (loader + wire format) plus a thin runtime crate `stringcheese-icu-runtime` compose into what the capability crates depend on.

### 5.1. Discovery

A caller finds its locale packs through a fixed lookup order:

1. **Explicit configuration.** The caller passes an `IcuRuntime` builder that names SCUD files directly (path or `&'static [u8]`). The only path in `no_std` builds.
2. **Environment variable.** `STRINGCHEESE_SCUD_PATH`, colon-separated on Unix / semicolon on Windows. Each entry is a directory searched for `<capability>-<locale>.scud`.
3. **XDG data dirs.** On Unix, `$XDG_DATA_HOME/stringcheese/scud/` then each `$XDG_DATA_DIRS` entry appended with `stringcheese/scud/`. On Windows the analogue is `%APPDATA%\stringcheese\scud\`.
4. **Compiled-in fallbacks.** Each language-pack crate ships an embedded SCUD blob; enabling the corresponding Cargo feature (`stringcheese/i18n-en`) links the blob into the binary. Recommended path for Wasm.

### 5.2. Fallback chain

Locale queries fall back through the CLDR-defined chain (matching ICU's `Locale::getFallback`, <https://unicode-org.github.io/icu-docs/apidoc/dev/icu4c/classicu_1_1Locale.html>):

```
pt-BR   ->   pt   ->   ""    (root)
zh-Hant-HK  ->  zh-Hant  ->  zh  ->  ""
en-US   ->   en   ->   ""
```

The chain is walked at load time (populated as a small `Vec<(Locale, ScudFile)>` per capability) and at query time (a `to-lower(input, "pt-BR")` call checks `pt-BR`'s pack, then `pt`'s pack, then root). The first pack with a mapping for the queried code point wins; missing entries do not abort the chain. The runtime holds N SCUD files open per locale-chain depth; each is small (kilobytes to low hundreds) and mmap keeps the per-file cost minimal.

### 5.3. Composition of a WIT world instance

Given loaded packs, constructing a component-model world instance is: the algorithm component (`stringcheese-icu-case.wasm`) is instantiated with its `case-data` import satisfied by an in-memory adapter that answers queries out of the loaded SCUD files. In pure-Rust host contexts (no component-model runtime), the same shape appears as an `IcuCaseRuntime::new(scud_files)` type implementing the case-mapping trait directly. The WIT layer is a thin adapter over this; each capability crate exposes both surfaces.

### 5.4. Versioning

Three orthogonal version numbers:

- **SCUD format version** (`fmt-maj.fmt-min` in the file header). Loaders enforce `file.fmt_maj == loader.supported_major`. A major bump is a hard break requiring a coordinated release.
- **CLDR version** (`"44.1"` in the header). Callers can read it to know what locale data was compiled in; the runtime does not enforce a specific CLDR version but the correctness report (see [DESIGN.md § Public Correctness Report](../DESIGN.md)) does.
- **WIT interface version** (`stringcheese:icu-case@0.1.0` in the package declaration). Semver applies; WIT tooling detects incompatibility at link time.

A caller with a `fmt-maj = 2` SCUD file but a component expecting `fmt-maj = 1` receives `ScudError::UnsupportedMajorVersion` at load, not a silent corruption. Interface mismatches surface at `wasm-tools compose` or component-instantiation time.

---

## 6. Language pack integration

The umbrella charter treats `stringcheese-<lang>` crates as the delivery vehicle for both the code-driven pieces of a language (stopword lists, language-specific phonetic encoders, tokenisation rules — described elsewhere) and the CLDR-derived locale data. Both live in the same crate because a caller who says "I support German" wants one dependency, not two.

### 6.1. Per-crate structure

Sketch, using `stringcheese-en`:

```
stringcheese-en/
├── Cargo.toml
├── src/
│   ├── lib.rs               // trait impls, feature gates, blob adapters
│   ├── phonetic.rs          // English-specific encoder
│   ├── stopwords.rs         // stopword list
│   └── tokenize.rs          // English tokenisation rules
└── data/
    ├── case-en.scud
    ├── collation-en.scud
    ├── plural-en.scud
    ├── number-en.scud
    ├── datetime-en.scud
    └── break-en.scud
```

The `data/*.scud` files are checked into the crate. Each is under 100 KB for a typical Latin-script locale, well below the size at which build-time regeneration becomes attractive.

The `lib.rs` uses `include_bytes!` to embed each blob under a Cargo feature:

```rust
// Proposed — not yet implemented.
// crates/stringcheese-en/src/lib.rs

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

/// Embedded SCUD blobs. Each is a `&'static [u8]` `stringcheese-scud`
/// wraps in a `ScudFile` at zero copy cost. Only enabled capabilities
/// are compiled in.
pub mod scud {
    #[cfg(feature = "case")]
    pub const CASE: &[u8] = include_bytes!("../data/case-en.scud");

    #[cfg(feature = "collation")]
    pub const COLLATION: &[u8] = include_bytes!("../data/collation-en.scud");

    #[cfg(feature = "plural")]
    pub const PLURAL: &[u8] = include_bytes!("../data/plural-en.scud");

    // ... one per capability.
}

/// Registration entrypoint into the shared `LanguageProvider` registry.
/// The `LanguageProvider` trait lives in `stringcheese-lang` (companion
/// design doc).
#[cfg(feature = "std")]
pub fn register(provider: &mut stringcheese_lang::LanguageProvider) {
    #[cfg(feature = "case")]
    provider.provide_case("en", scud::CASE);
    #[cfg(feature = "collation")]
    provider.provide_collation("en", scud::COLLATION);
    // ...
}
```

`Cargo.toml` declares one feature per capability so callers with tight size budgets pick à la carte:

```toml
# Proposed — not yet implemented.
[features]
default   = ["case", "collation", "plural"]
case      = []
collation = []
plural    = []
number    = []
datetime  = []
break     = []
```

Building with `default-features = false, features = ["case"]` compiles in *only* the English case-mapping blob — nothing else from the crate participates in the final binary. This matches the umbrella's feature-gate discipline described in [wasm-and-wit-interface.md § Feature-gate strategy](./wasm-and-wit-interface.md#feature-gate-strategy).

### 6.2. Which capabilities each pack ships

Packs are not required to cover every capability. `stringcheese-en` will ship all six; `stringcheese-tr` might ship only case, collation, and plural (Turkish's headline tailoring is the dotted/dotless *I*). A pack declares its coverage through the feature set it *offers*; callers see the missing capabilities as `ScudError::LocaleUnavailable` when they attempt to load a locale a pack does not cover, and fall back through the chain.

---

## 7. Threat model / caveats

### 7.1. SCUD files are trusted input

SCUD files are treated as trusted input, produced from CLDR by the project's build tool (see §10). The loader does not defend against maliciously crafted files. This is acceptable because language packs ship SCUD as `include_bytes!` (as trusted as the crate they came from) and filesystem discovery reads from XDG data dirs (user-controlled, trust-equivalent to the calling binary). If SCUD ever crosses a trust boundary (a service accepting user-uploaded packs), a hardened loader is needed in a separate crate.

### 7.2. CLDR licence and attribution

CLDR is licensed under the Unicode License (<https://www.unicode.org/license.txt>) — permissive, MIT-compatible, with attribution requirements. StringCheese's source is dual-licensed MIT / Apache-2.0. The licences are compatible but the derived-data attribution requirement propagates:

- Every `stringcheese-<lang>` crate shipping a SCUD blob derived from CLDR must reproduce the Unicode License notice and attribute the CLDR version. `Cargo.toml`'s `license-file` field points at `LICENSE-CLDR` alongside `LICENSE-MIT` and `LICENSE-APACHE`.
- The SCUD file embeds the CLDR version in its header (§4.1), so downstream can trace the data provenance.
- The umbrella's `NOTICE` file lists CLDR's copyright and licence text.

**Revisit in review:** the exact wording of `NOTICE` needs a legal read-through before the first pack ships.

### 7.3. Third-party language-pack contributions

Community `stringcheese-<their-language>` crates inherit the same CLDR attribution obligation. The project needs a lightweight per-locale contribution guide covering: (a) how to regenerate SCUD from current CLDR, (b) the attribution / notice checklist, (c) the golden-case format for locale-specific test vectors (see [DESIGN.md § Golden Dataset Design](../DESIGN.md)).

A conservative reading of the Unicode Licence's attribution requirement may prefer that third-party packs *do not* redistribute CLDR data verbatim but instead invoke the build tool at install time — uglier to consume but eliminates ambiguity. **Revisit in review.**

---

## 8. Phased implementation plan

Delivery is per capability, one phase per new WIT + first pack + SCUD supplement.

**Phase 1 — SCUD format + case mapping (foundation).** Done when: `crates/stringcheese-scud` loads a well-formed file and extracts magic / version / CLDR version / capability views; `component/wit/icu-case/stringcheese-icu-case.wit` parses under `wit-parser` and passes `wit-bindgen` smoke; `crates/stringcheese-icu-case` implements the algorithm side and delegates to `CaseDataView<'_>`; `crates/stringcheese-en` ships `case-en.scud`, registers via `LanguageProvider`, and passes ≥ 100 golden vectors (ASCII, common Latin extended, German ß, Turkish `i` via the `tr` pack loaded alongside); `crates/stringcheese-tr` exists as the second pack so cross-locale composition is exercised; size measurement infrastructure ([wasm-and-wit-interface.md § Measurement](./wasm-and-wit-interface.md#measurement)) reports the composed component's size in CI.

**Phase 2 — Collation.** Done when `stringcheese-icu-collation` WIT + Rust implementation lands; `compare` and `sort-key` pass the UCA conformance subset (<https://www.unicode.org/reports/tr10/CollationTest.html>) at primary/secondary/tertiary strength; `collation-en.scud` and `collation-de.scud` ship as the first two locales; footprint measured and reported.

**Phase 3 — Plural rules + number formatting.** Done when `stringcheese-icu-plural` returns the correct CLDR plural class for cardinals and ordinals across the top 20 locales by CLDR coverage class; `stringcheese-icu-number` formats decimals, currency, and percent for those locales with CLDR default patterns; golden vectors derived from ICU4X's testdata rather than freshly authored.

**Phase 4 — Date/time (Gregorian) + calendar interfaces.** Done when `stringcheese-icu-datetime` formats dates and times against Proleptic Gregorian for a first tranche of locales; the calendar interface is designed so adding an Islamic pack later does not require a WIT break; time-zone-free semantics documented explicitly (all times treated as local-to-input; no zone conversion).

**Phase 5 — Break iteration.** Done when grapheme/word/sentence/line iterators pass UAX #29 and UAX #14 conformance for language-neutral cases; word segmentation dictionaries for Thai, Japanese, and Chinese ship as SCUD supplements; interoperation with `stringcheese-unicode`'s existing grapheme iteration is verified (no double-implementation).

**Phase 6 — Multi-language coverage.** Done when `stringcheese-en`, `stringcheese-fr`, `stringcheese-de`, `stringcheese-es`, `stringcheese-ja`, `stringcheese-zh`, `stringcheese-ar` all ship with the full six capabilities each; the published correctness report includes an i18n section listing locale coverage, capability coverage, and cross-pack fallback tests.

Each phase is independently releasable; a caller who needs only case mapping never has to wait for the plural or date/time phases.

### 8.6. Phase 6 progress (2026-08)

**Landed.** The Phase 6 multi-language rollout ships case + collation packs across the six-locale roster (**{en, de, fr, tr, ru, zh}**) and closes the Turkish-i locale-tailoring algorithm hook:

- **Case-mapping locales.** The Phase 1 shipped set of `{en, tr}` grows to `{en, de, fr, tr, ru, zh}`. Each new pack ships an ASCII a-z ↔ A-Z table, the Latin-1 supplement (À-Þ ↔ à-þ excluding × ÷), the ß → SS full uppercase / ß → ss full fold rows, and language-specific letters (fr adds Ÿ / Œ / Æ; ru adds the Cyrillic upper/lower pairs; zh ships ambient ASCII since Han is case-neutral; de adds ẞ → ß simple-lower). `case-de.scud` is the new pack landing in this wave (~1.5 KB); `case-{fr,ru,zh}.scud` landed in the Phase 6 partial rollout (Wave 14).
- **Turkish-i algorithm hook.** `stringcheese_icu_case::CaseEngine::pack_for` grows a locale-alias fallback that fires when an exact tag-match misses. The Phase 6 table maps `az` and `az-Latn` → `tr` so a caller shipping only the Turkish pack still handles the Turkic dotted/dotless-I rules under Azerbaijani queries (same rules per [UAX #21 § 1.2](https://www.unicode.org/reports/tr21/tr21-7.html)). Exposed as the standalone `case_locale_alias(tag)` helper for pack-authoring introspection. +7 new unit tests exercising the four Turkish-i rules (I → ı, i → İ, İ → i, ı → I) plus az / az-Latn / az-AZ fallback and a control that English keeps default rules.
- **Collation locales.** The Phase 2 shipped set of `{en, de}` grows to `{en, de, fr, tr, ru, zh}`. The fr / ru / zh packs shipped in Wave 14 (each ships options + shared ß→ss expansion + Latin-1 supplement rows so the composed engine has uniform pack-hit ratios); the tr pack adds Turkish alphabet options. Locale-specific ordering tailorings — French backwards-secondary and Turkish primary-distinct dotless-ı — landed in this wave via the new `SECT_PRIMARY_OVERRIDES` section (bytes `PriW`, `u32 count` + sorted `(u32 cp, u32 primary, u32 secondary, u32 tertiary)` records) plus a `backwards_secondary` bit in `SECT_COLLATION_OPTIONS` byte 2. Russian case-second and Chinese pinyin/stroke remain documented deferrals (see the red-flag list below).
- **Component embedding.** `stringcheese-icu-case-component` grows from embedding `{en, tr}` to embedding `{en, de, fr, tr, ru, zh}`; `stringcheese-icu-collation-component` grows from `{en, de}` to `{en, de, fr, tr, ru, zh}`. Each smoke-test suite gains one-or-two per-locale wasmtime tests exercising the new packs end-to-end, plus an Azerbaijani-alias smoke on the case component.
- **CI.** `wasm-i18n-case` and `wasm-i18n-collation` jobs extended from two-locale coverage to six-locale coverage. Each job runs the per-locale `case_golden_<lang>` / `collation_golden_<lang>` suite plus a size-reporting step listing all six SCUD blobs.

**Locales covered vs the Phase 6 charter.** Phase 6's completion charter (§ 8, above) lists `{en, fr, de, es, ja, zh, ar}` as the seven languages that must ship the full six capabilities each. Case + collation are now covered for `{en, de, fr, tr, ru, zh}` — six of the seven; the missing pieces are:

- `es`, `ja`, `ar` — no case or collation packs yet. The Phase 3 plural + number packs for these landed in Wave 3.
- `tr`, `ru` are Phase 6 additions over the charter's baseline seven, giving richer coverage for Turkic and Cyrillic scripts.
- Full-capability coverage (adding datetime + break for every locale) remains a follow-up wave.

**Red flags.** A couple worth noting for reviewers:

- ~~`az-Cyrl` walks up to `az` and inherits Turkic-I incorrectly.~~ **Landed** (2026-08). `CaseEngine::pack_for_with_origin` now consults the origin locale's script subtag before firing the `az → tr` alias — a bare `az` origin still fires (Latin is the CLDR default script for Azerbaijani), but any `az-Cyrl-*` origin suppresses the alias at every rung of the fallback walk. New helper `case_locale_alias_for_origin(tag, origin)` exposes the gate for direct callers. +3 unit tests (`case_locale_alias_for_origin_gates_on_script_tag`, `az_cyrl_does_not_inherit_turkish_tailorings`, updated `case_locale_alias_returns_expected_partners`).
- ~~Turkish collation is still primary-equal on ı vs i.~~ **Landed** (2026-08). New SCUD section `SECT_PRIMARY_OVERRIDES` (bytes `PriW`) carries sorted `(u32 cp, u32 primary_weight, u32 secondary_weight, u32 tertiary_weight)` records; the tr collation pack ships primary-weight override rows for the full Turkish lowercase alphabet, assigning dotless-ı (U+0131) a primary weight between h (U+0068) and i (U+0069). `CollationEngine::compare` / `sort_key` switch to a weight-tuple comparator when the active pack carries overrides — characters outside the table use their ASCII-lowercased codepoint as a primary-weight approximation. +5 unit tests in `stringcheese-icu-collation` + 8-vector `primary_distinct_dotless_i` golden in `stringcheese-tr`.
- ~~French backwards-secondary is a documented deferral.~~ **Landed** (2026-08). `SECT_COLLATION_OPTIONS` grows a `backwards_secondary` bit (byte 2); the fr collation pack sets it. `CollationEngine::compare_with_backwards_secondary` decomposes precomposed accented Latin letters (via a hand-written table covering the Latin-1 supplement), strips combining marks for the level-1 primary compare, then extracts a per-position secondary sequence and reverses it for the level-2 tie-break. Result: `cote < côte < coté < côté` — the classical French dictionary order. +5 unit tests in `stringcheese-icu-collation` + `backwards_secondary_classic_quartet` + 21 more golden assertions in `stringcheese-fr`.

**Remaining collation deferrals.** Two locale-specific tailorings still ride on default UCA:

- **Chinese pinyin / stroke ordering.** Neither of the Han-script ordering conventions has landed. Both require large per-character weight tables (~20k CJK ideographs); adding rows to the existing `SECT_PRIMARY_OVERRIDES` section is the natural extension point but the data-shipping story is a follow-up wave.
- **Russian case-second.** The ru pack currently ships default UCA (case-tertiary). CLDR Russian moves case to the secondary level so `Аа < аА` at secondary strength; landing this needs a new options-blob bit and a small `CollationEngine` code path shift. Sized as a small follow-up.

### 8.5. Phase 5 progress (2026-08)

**Landed.** The Phase 5 foundation ships across a single wave:

- `stringcheese-scud` — extended with the `CAP_BREAK` capability tag and six new section ids (`SECT_GRAPHEME_CLASSES` / `SECT_WORD_CLASSES` / `SECT_SENTENCE_CLASSES` / `SECT_GRAPHEME_RULES` / `SECT_WORD_RULES` / `SECT_SENTENCE_RULES`) with `GraphemeClass` / `WordClass` / `SentenceClass` enums, a `BreakDataView` zero-copy view, and a `BreakSectionBuilder` writer. Class-override wire format is a sorted `(cp, run_len, class)` table so a pack can tailor the built-in UAX #29 classifier without shipping a full Unicode-wide table. `RULES_UAX29_DEFAULT` marker + `set_default_rules()` builder helper wire the "just use the algorithm's default rules" path.
- `stringcheese-icu-segment` — the WIT interface at `component/wit/segment/stringcheese-icu-segment.wit` (package `tegmentum:i18n-segment@0.1.0`, parses cleanly under `wit-parser` with smoke tests) plus a `BreakEngine` algorithm side that consumes an optional `BreakPack` and drives the UAX #29 grapheme (§ 3) / word (§ 4) / sentence (§ 5) rules from the built-in classification tables in `classes.rs`. GB1–GB13, WB1–WB16, SB1–SB11 implemented; line breaking (UAX #14) is a follow-up crate `stringcheese-icu-linebreak`. 40 unit tests + 25 hand-authored UAX #29 vectors covering RI parity, ZWJ emoji sequences, CRLF, ExtendedPictographic + Extend + ZWJ interactions, Hangul syllable composition, ATerm-vs-STerm sentence-tail state machine.
- `stringcheese-icu-segment-component` — the WASM component wrapper that carries the `segment-world` world across the wasm boundary, mirroring the `stringcheese-icu-datetime-component` template's shape (dual `cdylib` + `rlib`, `wit-component` feature gate on `wit-bindgen-rt` 0.44, pre-generated `src/bindings.rs` from `wit-bindgen rust --runtime-path wit_bindgen_rt`, in-process `wasmtime::component::bindgen!` smoke tests, `AtomicPtr`-backed `shared_engine()` singleton). Unlike the collation / datetime component wrappers, the segment component embeds **no** SCUD packs — the algorithm crate's `BreakEngine::new()` runs pure-algorithm-driven UAX #29 rules from the built-in tables, so the reference engine has zero external data to bundle. `supported-locales` returns `[""]` (the root-locale marker) per the WIT contract's Phase 5 note; `supports(loc)` returns `true` for every input for forward compatibility with future locale tailorings. 8 wasmtime smoke tests cover grapheme boundaries (ASCII, empty input, CRLF, family emoji ZWJ sequence), word boundaries (mixed ASCII + punctuation), sentence boundaries (two-sentence split), and both capabilities exports.
- CI — two new jobs `wasm-i18n-segment` and `wasm-i18n-segment-component` in `.github/workflows/ci.yml`. The segment-component crate is added to the wasm-runtime `--exclude` list for the same duplicate-export-symbol reason the case-component / collation-component / datetime-component crates are excluded.

**Wasm component sizes** (measured locally, aarch64 macOS, `--release`, wasmtime 26 / wasm-tools 1.254): raw module 82 448 B, componentised 104 854 B. Smaller than the datetime component (~114 KiB componentised) because no SCUD packs are embedded — the entire ~82 KiB is UAX #29 classification tables + rule state machines plus the wit-bindgen guest scaffolding.

**Locales covered vs deferred.** Phase 5 ships the locale-neutral default only. UAX #29 grapheme rules are language-independent; word / sentence rules are locale-neutral for every script except:

- **Japanese + Chinese** — CJK word segmentation without whitespace requires a dictionary-based segmenter (MeCab-style). Deferred to a follow-up SCUD pack extension; the WIT `locale` argument is already accepted for forward compatibility.
- **Thai / Lao / Khmer / Burmese** — Syllable segmentation requires either a dictionary or a rule-based segmenter (ICU's `BreakIterator` ships one of each). Deferred alongside the CJK dictionaries.
- **Portuguese / French** — Elision-aware word segmentation (`l'homme` → `l'` + `homme`) is a per-locale tailoring; the shipped UAX #29 default keeps them together as one word.

**Deferred.** Documented in the crate roots and left for the follow-up wave:

- **UAX #14 line breaking.** A separate crate `stringcheese-icu-linebreak` — the LB rules ship large enough table state that combining both would balloon the pack.
- **Dictionary-based CJK word segmentation.** MeCab-style morphological analyzer for Japanese; forward-maximum-match segmenter for Chinese. Both need multi-MB dictionaries that belong in per-locale SCUD packs rather than the algorithm crate.
- **Interop with `stringcheese-unicode`'s existing grapheme iterator.** Phase 5 does not consolidate the two implementations; a future wave should either delegate `stringcheese-unicode::grapheme_indices` to `BreakEngine::segment_graphemes` or vice versa to eliminate the duplicate table shipping.
- **Extended grapheme cluster tailoring for Indic scripts (`GB9c`).** The current `GraphemeIter` approximates `GB9c` as `GB9` (Extend after any base). InCB=Consonant handling needs a per-scalar `InCB` classification table which is not yet shipped. Impact: minimal — the `GB9` approximation over-glues by at most one Extend character per Indic sequence.

**Red flags.** A couple worth noting for reviewers:

- **RI parity semantics.** GB12/GB13 (Regional Indicator pair) track the count of RIs in the current run; the parity is checked at the point of comparison. `GraphemeIter` resets the count on any non-RI scalar. Cross-checked against the "🇬a🇬🇧" test case in the algorithm crate's `ri_run_length_is_reset_after_non_ri` unit test.
- **WB4 fold-through of Extend/Format/ZWJ.** The word iterator's `fold_extend_format_zwj` matches UAX #29's "these classes inherit the previous effective class, except after CR/LF/Newline". This introduces a subtle asymmetry: `prev_effective` is folded, but `prev_raw` (the class of the immediately-preceding scalar) is kept separately so WB3c (`ZWJ × ExtPict`) can consult the actual ZWJ rather than what it folded into. Verified by `wb_zwj_pictographic_no_break`.
- **Sentence-break state machine simplification.** SB11's "break unless curr is `Lower | Extend | Format`" path folds SB8 into a simpler "assume any non-lowercase continuation ends the sentence" heuristic. Matches CLDR reference behaviour on the shipped test vectors; a full SB8 implementation would need OLetter/Upper/Lower lookahead through unlimited `Numeric | SContinue | SATerm` runs and is deferred.
- **No pack was authored for the shipped component.** Unlike the collation / datetime components (which each bundle two or three locale packs so the shipped `.wasm` is drivable end-to-end for its full CLDR pattern surface), the segment component's reference engine is just `BreakEngine::new()`. Any future pack-driven tailoring (CJK dictionaries, Thai syllable segmentation) can be added to the component's `reference_engine()` without breaking the WIT surface — `BreakEngine::with_pack(pack)` and multi-pack composition are already supported in the algorithm crate.

### 8.4. Phase 4 progress (2026-08)

**Landed.** The Phase 4 foundation ships across a single wave:

- `stringcheese-scud` — extended with `CAP_DATETIME` (already reserved in Phase 1) plus eight new section ids (`SECT_DATE_PATTERNS` / `SECT_TIME_PATTERNS` / `SECT_MONTH_NAMES` / `SECT_MONTH_ABBR` / `SECT_WEEKDAY_NAMES` / `SECT_WEEKDAY_ABBR` / `SECT_AM_PM` / `SECT_ERA_NAMES`) with a `DateTimeLength` enum (`Short | Medium | Long | Full`), a `DateTimeDataView` zero-copy view, and a `DateTimeSectionBuilder` writer. Section wire formats are index-based (dates/times: four consecutive length-prefixed strings) or `u16`-counted (months/weekdays), matching the small-table shapes Phase 4 needs. +4 new unit tests.
- `stringcheese-icu-datetime` — the WIT interface at `component/wit/datetime/stringcheese-icu-datetime.wit` (package `tegmentum:i18n-datetime@0.1.0`, parses cleanly under `wit-parser` with 3 smoke tests) plus a `DateTimeEngine` algorithm side that consumes one or more `DateTimePack`s and walks the BCP 47 fallback chain (`fr-CA → fr → ""`) at query time. Ships a self-contained CLDR pattern interpreter recognising `y`/`yy`/`yyyy`/`M`/`MM`/`MMM`/`MMMM`/`d`/`dd`/`E`/`EEEE`/`H`/`HH`/`h`/`hh`/`m`/`mm`/`s`/`ss`/`a`/`G` plus CLDR quoted literals (`'text'`, `''`). Date math is self-contained: leap-year via the classic Gregorian rule, weekday via Zeller's congruence. No `chrono` / `jiff` / `time` dep — the wasm component stays lean. 24 unit tests + 3 wit-parser smoke tests.
- `stringcheese-icu-datetime-component` — the WASM component wrapper that carries the `datetime-world` world across the wasm boundary, mirroring the `stringcheese-icu-collation-component` template's shape (dual `cdylib` + `rlib`, `wit-component` feature gate on `wit-bindgen-rt`, pre-generated `src/bindings.rs` from `wit-bindgen rust --runtime-path wit_bindgen_rt`, in-process `wasmtime::component::bindgen!` smoke tests). The shipped `.wasm` embeds `datetime-{en,de,fr}.scud` so the componentised binary is drivable end-to-end. 7 wasmtime smoke tests cover `format-date` (en/de/fr short/medium/full), `format-time` (English 12-hour AM/PM), `format-datetime` (combined), `supported-locales`, and `supports` (BCP 47 fallback via `fr-CA → fr`).
- `stringcheese-en` — `datetime-en.scud` with the CLDR default patterns (`M/d/y` short, `MMM d, y` medium, `MMMM d, y` long, `EEEE, MMMM d, y` full for dates; `h:mm a` short + `h:mm:ss a` medium/long/full for times), full + abbreviated month/weekday names, `AM`/`PM` markers, `BC`/`AD` eras. Exposed via `datetime_data::datetime_pack()` under a new `datetime-scud` feature (default-on). 10 golden test functions covering short/medium/long/full dates, 12-hour AM/PM edge cases (midnight, noon, 12:30 PM), combined datetime, BCP 47 fallback, and out-of-range input rejection.
- `stringcheese-de` — `datetime-de.scud` (`dd.MM.y` short/medium, `d. MMMM y` long, `EEEE, d. MMMM y` full; 24-hour throughout; German month + weekday names with `März`, `Sonntag`, …; `v. Chr.`/`n. Chr.` eras). 8 golden test functions.
- `stringcheese-fr` — `datetime-fr.scud` (`dd/MM/y` short, `d MMM y` medium, `d MMMM y` long, `EEEE d MMMM y` full; 24-hour throughout; French month + weekday names all lowercase per CLDR; `av. J.-C.`/`ap. J.-C.` eras). 8 golden test functions.
- CI — two new jobs `wasm-i18n-datetime` and `wasm-i18n-datetime-component` in `.github/workflows/ci.yml`. The datetime-component crate is added to the wasm-runtime `--exclude` list for the same duplicate-export-symbol reason the case-component and collation-component crates are excluded.

**Measured sizes** (release builds of the SCUD blobs, uncompressed):

| Pack               | Bytes | Notes                                                              |
| ------------------ | -----:| ------------------------------------------------------------------ |
| `datetime-en.scud` |   415 | 4 date + 4 time patterns, 12 full + 12 abbr months, 7 + 7 weekdays |
| `datetime-de.scud` |   436 | Same shape plus German umlauts + `März` / `Chr.` eras              |
| `datetime-fr.scud` |   445 | Same shape plus French lowercase month/weekday names               |

**Wasm component sizes** (measured locally, aarch64 macOS, `--release`, wasmtime 26 / wasm-tools 1.254): raw module 91 586 B, componentised 114 442 B. Larger than the icu-case component (~112 KiB componentised) by roughly 2 KiB, entirely accounted for by the three embedded SCUD packs.

**Locales covered vs deferred.** Phase 4 ships packs for `en`, `de`, and `fr` (matching Phase 3's initial locale set). The remaining ~200 CLDR locales are follow-ups that add per-locale packs but not new algorithm code — the pattern interpreter and Gregorian arithmetic already cover any Gregorian-calendar locale that uses the shipped token set.

**Deferred.** Documented in the crate roots and left for the follow-up wave:

- **Non-Gregorian calendars** (Buddhist, Hebrew, Islamic, Japanese). The WIT interface is calendar-agnostic — a future capability tag could ship parallel `SECT_MONTH_NAMES` variants under an Islamic pack — but Phase 4's engine hardcodes Gregorian arithmetic. Adding a calendar is a new algorithm-side crate plus a new SCUD capability tag; the WIT interface does not need to break.
- **Timezone conversion.** Phase 4 is time-zone-naive: the input ISO string's offset (or lack thereof) is discarded by the formatter. Zone-aware formatting is a separate WIT interface (`stringcheese-icu-datetime-tz@0.1.0`) that takes an IANA TZDB name and returns the offset-adjusted rendering; the release cadence of TZDB makes this a distinct maintenance surface.
- **Skeleton-based formatting** — CLDR skeletons (`"yMMMd"` → auto-pick the closest pattern) fall out of a per-locale skeleton table, which is not shipped here. Phase 4 uses only the fixed short/medium/long/full lengths.
- **Relative time formatting** ("3 hours ago") and **interval formatting** ("Mar 3 – Mar 5") are separate concerns handled by different WIT interfaces.
- **Localised era-style alternatives** — CLDR's `GGGGG` (narrow era) and `GGGG` (era name in full: `"Anno Domini"` vs `"AD"`) are not shipped; Phase 4 renders `G`/`GG`/`GGG` as the abbreviated form and leaves narrow / wide variants for a follow-up expansion of `SECT_ERA_NAMES`.

**Red flags.** A couple worth noting for reviewers:

- **Zeller's congruence handles the Gregorian calendar directly.** The formula in `weekday()` uses the "Sunday = 0" convention baked in via a fixed offset from Zeller's own "Saturday = 0" output. Cross-checked against five hand-verified reference dates (2024-09-22 Sunday, 2024-01-01 Monday, 2000-02-29 Tuesday, 1969-07-20 Sunday, 1776-07-04 Thursday) plus the leap-year matrix (`is_leap_year` accepts 2024, 2000, 2400; rejects 1900, 2100). The formula is textbook (Wikipedia's [Zeller's congruence](https://en.wikipedia.org/wiki/Zeller%27s_congruence) page); the only subtlety is the January/February shift into months 13/14 of the previous year, which is done inline.
- **Epoch boundaries.** The `year` field is a signed `i32`, so the calendar covers everything from year -2 147 483 648 to year 2 147 483 647 — but Gregorian is *proleptic*: it extends the Gregorian rules backwards past 1582 into the pre-Gregorian world. Callers formatting historical dates before 1582 get output that would not agree with a period-appropriate Julian rendering; this is the standard Proleptic Gregorian trade-off and is called out in the crate docs. The era token `G` renders `BC` for `year ≤ 0` (astronomical convention: year 0 = 1 BC, year -1 = 2 BC).
- **Timezone-naive assumption.** The `iso-datetime` parser accepts (and silently discards) `Z`, `+HH:MM`, and `-HH:MM` suffixes. A caller who forgets that the formatter is zone-naive may see wall-clock output that differs from what they expected — most obviously, `"2024-09-22T23:00:00Z"` formatted for a locale a user reads in `America/Los_Angeles` renders as `"2024-09-22 11:00 PM"` (UTC wall-clock) rather than the local `"2024-09-22 4:00 PM"`. This is the documented Phase 4 behaviour; a future zone-aware crate is the correct fix for callers who need it.
- **CLDR pattern token subset.** Phase 4 recognises the tokens the shipped short/medium/long/full patterns actually use. Unknown letters (`Q` quarter, `L` standalone month, `c` standalone weekday, `w` week-of-year, `z` timezone name, `Z` timezone offset, `X`/`x` ISO-8601 timezone, `V` timezone id) fall through as literals — a conservative choice for forward compatibility. A pack that ships a pattern using an unrecognised token gets that token emitted verbatim in the output; a maintainer adding a new locale should validate the output against a reference implementation before assuming the token is covered.

### 8.3. Phase 3 progress (2026-08)

**Landed.** The Phase 3 foundation ships across a single wave:

- `stringcheese-scud` — extended with `CAP_PLURAL` and `CAP_NUMBER` capability tags (both already reserved in Phase 1). Adds `PluralDataView` + `PluralSectionBuilder` over two section ids (`SECT_CARDINAL_RULES` / `SECT_ORDINAL_RULES`) with a `PluralCategory` enum plus a `PluralRuleIter`; adds `NumberDataView` + `NumberSectionBuilder` over three section ids (`SECT_DECIMAL_PATTERN` / `SECT_CURRENCY_TABLE` / `SECT_PERCENT_PATTERN`) with decoded `DecimalPattern` / `CurrencyRecord` / `PercentPattern` records. +5 new unit tests.
- `stringcheese-icu-plural` — the WIT interface at `component/wit/plural/stringcheese-icu-plural.wit` (package `tegmentum:i18n-plural@0.1.0`, parses cleanly under `wit-parser` with 3 smoke tests) plus a `PluralEngine` algorithm side that consumes one or more `PluralPack`s and walks the BCP 47 fallback chain (`pt-BR → pt → ""`) at query time. Hand-encodes ~11 CLDR plural predicates in the `PluralRuleId` enum (English cardinal `IEq1AndVEq0`; English ordinal `NMod10Eq1NotMod100Eq11` + `NMod10Eq2NotMod100Eq12` + `NMod10Eq3NotMod100Eq13`; French cardinal `IIn01`; Spanish/Italian ordinal `NEq1`; Russian cardinal `RuOne` + `SlavFew` + `RuMany`; Polish cardinal `PlMany`; Arabic full set `NEq0` + `NEq2` + `ArFew` + `ArMany`). 11 unit tests + 3 wit-parser smoke tests.
- `stringcheese-icu-number` — the WIT interface at `component/wit/number/stringcheese-icu-number.wit` (package `tegmentum:i18n-number@0.1.0`) plus a `NumberEngine` algorithm side that implements `format-decimal` / `format-currency` / `format-percent` against SCUD-supplied patterns. Formats through Rust's default `{:.N}` (banker's / half-to-even rounding) matching CLDR reference implementations; currency defaults to 2 fraction digits, percent to 0 (both overridable via `FormattingOptions`). Negative currencies place the sign outside the symbol (`-$1.00`) matching CLDR's `-¤#,##0.00`. 15 unit tests + 3 wit-parser smoke tests.
- `stringcheese-en` — `plural-en.scud` covering English cardinals + ordinals with the CLDR teens exception, and `number-en.scud` covering group `,` / decimal `.` / USD-EUR-GBP-JPY-CAD-AUD currencies (all symbol-before-value) / percent `%` (no space). Exposed via `plural_data::plural_pack()` and `number_data::number_pack()` under new `plural-scud` and `number-scud` cargo features (both default-on). 47 golden-vector assertions across 17 test functions.
- `stringcheese-de` — `plural-de.scud` (cardinal `one` when `i=1 v=0`; every ordinal is `other`) plus `number-de.scud` (group `.` / decimal `,` / EUR-USD-GBP-CHF placed after value with a space / percent `%` with a space). 8 golden test functions for numbers + 6 for plurals, ~60 assertions total.
- `stringcheese-fr` — `plural-fr.scud` (cardinal `one` for `i in 0..1` — 0 and 1 both singular; ordinal `one` only for `n = 1`) plus `number-fr.scud` (group NBSP U+00A0 / decimal `,` / EUR-USD-GBP-CAD-CHF placed after with a space / percent `%` with a space). 7 golden test functions for numbers + 7 for plurals. New `build.rs` (fr previously had none) emits the SCUD packs.
- CI — two new jobs `wasm-i18n-plural` and `wasm-i18n-number` in `.github/workflows/ci.yml` build both crates, run unit + golden tests, and print per-locale SCUD sizes to the run log.

**Wave 2 — 5 additional SCUD packs (2026-08).** A data-only round extending the Phase 3 packs from {en, de, fr} to {en, de, fr, ru, pl, ar, zh, ja} without changing algorithms. The algorithm crate's `PluralRuleId` opcode set already covered every one of these; this round wires per-locale `plural_data` / `number_data` modules, `build.rs` codegen, and golden-vector suites into the existing `stringcheese-<lang>` packs behind opt-in `plural-scud` / `number-scud` features (matching en/de/fr). The `stringcheese-icu-plural::builder` module grows helper functions (`russian_cardinals`, `polish_cardinals`, `arabic_cardinals`, `chinese_cardinals`, `japanese_cardinals`, plus matching `_ordinals` counterparts) so every pack's `build.rs` uses the same declarative surface.

**Wave 3 — Latin-adjacent trio (2026-08).** A follow-up data-only round bringing the shipped set from `{en, de, fr, ru, pl, ar, zh, ja}` (8 locales) to `{en, de, fr, es, pt, ru, pl, ar, zh, ja}` (10 locales). Wave 3 also grows the algorithm crate's `PluralRuleId` enum with two new opcodes — `EsItPtCardinalMany` (rule 16, `v = 0 and i != 0 and i % 1000000 = 0`) and `ItOrdinalMany` (rule 17, `n in {8, 11, 80, 800}`) — needed for Spanish/Italian/Portuguese cardinal `many` (CLDR 42+ added the compact-large-number bucket) and Italian ordinal `many` respectively. New builder helpers `spanish_cardinals`, `spanish_ordinals`, `italian_cardinals`, `italian_ordinals`, `portuguese_cardinals`, `portuguese_ordinals` land in `stringcheese-icu-plural::builder`; the Italian pair ships for future use, since `stringcheese-it` does not yet exist in the workspace.

- `stringcheese-es` — `plural-es.scud` (three-way: `one` when `n = 1`, `many` when `v = 0 and i != 0 and i % 1000000 = 0`, `other` otherwise; ordinals are `other`-only) plus `number-es.scud` (Spain conventions: group `.` / decimal `,` / EUR-USD-GBP-MXN after value with space / percent `%` with space). New `build.rs`, `plural_data`, `number_data` modules; the SCUD features are off by default (matching the `ru`/`pl`/`ar`/`zh`/`ja` cadence, not the always-on en/de/fr defaults). 11 plural + 10 number golden test functions.
- `stringcheese-pt` — `plural-pt.scud` matching CLDR 44 pt-PT rules (`one` when `n = 1 and v = 0`, `many` at exact million-multiples, `other` otherwise; ordinals `other`-only) plus `number-pt.scud` (group `.` / decimal `,` / EUR-BRL-USD-GBP after value with space / percent `%` with space). BRL included alongside EUR so a caller servicing either variety has the primary regional currency present. 11 plural + 11 number golden test functions.
- CI — the `wasm-i18n-plural` and `wasm-i18n-number` jobs grow `-p stringcheese-es` / `-p stringcheese-pt` entries and per-locale golden-test steps; the SCUD-size reporter enumerates the two new packs.

**Wave 4 — Italian closes the last Phase 3 gap (2026-08).** A single-locale data-only round bringing the shipped set from `{en, de, fr, es, pt, ru, pl, ar, zh, ja}` (10 locales) to `{en, de, fr, es, pt, it, ru, pl, ar, zh, ja}` (11 locales). No new algorithm code — the `EsItPtCardinalMany` / `ItOrdinalMany` opcodes and the `italian_cardinals` / `italian_ordinals` builder helpers already shipped in Wave 3. The wave's actual work is scaffolding the missing base language-pack crate.

- `stringcheese-it` — a **new base language-pack crate**, the Latin-adjacent trio's third member. Ships a minimum-viable `Language` trait impl: `code = "it"`, `name = "Italian"`, a ~30-word MVP hand-curated ASCII-only stopword list (articles, prepositions, coordinating conjunctions, high-frequency personal pronouns, negation, and the bare-infinitive forms of the common auxiliary and modal verbs), the `ItalianTokenizer` transparent wrapper around `SimpleTokenizer`, and an **identity stemmer** (`Cow::Borrowed`). A pure-Rust Snowball Italian port and an Italian PHONEX phonetic encoder are documented follow-ups — the sibling Latin-script packs each hand-port their Snowball algorithm and the workspace has no shared Snowball binding to lean on. `phonetic_encoder()` returns `None`; `collator()` returns `None` (default Unicode Latin ordering suffices). Registered via `stringcheese_lang::register_language!(ITALIAN)`.
- `stringcheese-it` — `plural-it.scud` (60 bytes; three-way cardinal `one`/`many`/`other` with `i = 1 and v = 0` for `one` — Italian differs from Spanish here, which uses `n = 1` — plus the `EsItPtCardinalMany` sub-clause; ordinal `many` bucket via `ItOrdinalMany` for `n ∈ {8, 11, 80, 800}` — one of the very few CLDR locales with a distinct ordinal `many` bucket, matching `ottavo → 8º` / `undicesimo → 11º` / `ottantesimo → 80º` / `ottocentesimo → 800º`) plus `number-it.scud` (104 bytes; Italy conventions: group `.` / decimal `,` / EUR-USD-GBP-CHF after value with space / percent `%` with space; CHF included for `it-CH` Italian-Switzerland relevance, with a dedicated `it-CH` pack a documented follow-up). 16 plural + 14 number golden test functions.
- CI — the `wasm-i18n-plural` and `wasm-i18n-number` jobs grow a `-p stringcheese-it` entry, a per-locale `plural_golden_it` / `number_golden_it` step, and the SCUD-size reporter enumerates `plural-it.scud` / `number-it.scud`.

- `stringcheese-ru` — `plural-ru.scud` (Slavic three-way: `one` when `v=0 and i%10=1 and i%100!=11`, `few` when `v=0 and i%10 in 2..4 and i%100 not in 12..14`, `many` when `v=0 and (i%10=0 or i%10 in 5..9 or i%100 in 11..14)`) plus `number-ru.scud` (group NBSP U+00A0 / decimal `,` / RUB-USD-EUR-GBP after value with space / percent `%` with space). 8 plural + 10 number golden test functions, 100+ assertions total.
- `stringcheese-pl` — `plural-pl.scud` (three-way: `one` only for `i=1 v=0`, `few` shares Russian's `SlavFew`, `many` uses `PlMany` predicate) plus `number-pl.scud` (group NBSP / decimal `,` / PLN-USD-EUR-GBP after value with space / percent `%` **without** space — Polish is the odd shipped locale here). 8 plural + 10 number golden test functions.
- `stringcheese-ar` — `plural-ar.scud`, the **maximum-category-count** pack: exercises every one of the six CLDR plural categories (`zero`, `one`, `two`, `few`, `many`, `other`); `PluralCategory::Zero` end-to-end through the `PluralEngine` verified. `number-ar.scud` uses the `latn` numbering-system default (Western digits): group `,` / decimal `.` / SAR-USD-EUR-AED before value with space / percent `%` with no space. 11 plural + 10 number golden test functions (Arabic hits the task's 30-vector floor for six-bucket locales).
- `stringcheese-zh` — `plural-zh.scud` (empty rule table; every input classifies as `other` — Chinese lacks grammatical number in CLDR) plus `number-zh.scud` (group `,` / decimal `.` / CNY-USD-EUR-HKD before value with no space / percent `%` with no space). 8 plural + 10 number golden test functions.
- `stringcheese-ja` — `plural-ja.scud` (empty rule table like `zh`) plus `number-ja.scud` (group `,` / decimal `.` / JPY-USD-EUR-CNY before value with no space / percent `%` with no space). 8 plural + 11 number golden test functions; the Japanese suite adds an explicit `currency_jpy_zero_fraction_override` case documenting how a caller opts into culturally-correct 0-fraction yen output.

**Measured sizes** (release builds of the SCUD blobs, uncompressed):

| Pack             | Bytes | Notes                                                      |
| ---------------- | -----:| ---------------------------------------------------------- |
| `plural-en.scud` |    62 | 1 cardinal rule + 3 ordinal rules                          |
| `plural-de.scud` |    56 | 1 cardinal rule, no ordinals                               |
| `plural-fr.scud` |    58 | 1 cardinal rule + 1 ordinal rule                           |
| `plural-ru.scud` |    60 | 3 cardinal rules, no ordinals                              |
| `plural-pl.scud` |    60 | 3 cardinal rules, no ordinals                              |
| `plural-ar.scud` |    64 | 5 cardinal rules (max-category), no ordinals               |
| `plural-zh.scud` |    54 | 0 rules — every input falls through to `other`             |
| `plural-ja.scud` |    54 | 0 rules — every input falls through to `other`             |
| `plural-es.scud` |    58 | 2 cardinal rules (one + many), no ordinals                 |
| `plural-pt.scud` |    58 | 2 cardinal rules (one + many), no ordinals                 |
| `plural-it.scud` |    60 | 2 cardinal rules (one + many) + 1 ordinal rule (many)      |
| `number-en.scud` |   120 | 6 currencies + decimal + percent patterns                  |
| `number-de.scud` |   104 | 4 currencies + decimal + percent patterns                  |
| `number-fr.scud` |   114 | 5 currencies + decimal + percent patterns                  |
| `number-ru.scud` |   105 | 4 currencies + decimal + percent patterns                  |
| `number-pl.scud` |   105 | 4 currencies + decimal + percent patterns                  |
| `number-ar.scud` |   119 | 4 currencies + decimal + percent patterns                  |
| `number-zh.scud` |   106 | 4 currencies + decimal + percent patterns                  |
| `number-ja.scud` |   107 | 4 currencies + decimal + percent patterns                  |
| `number-es.scud` |   104 | 4 currencies + decimal + percent patterns (Spain defaults) |
| `number-pt.scud` |   105 | 4 currencies + decimal + percent patterns                  |
| `number-it.scud` |   104 | 4 currencies + decimal + percent patterns (Italy defaults) |

(Measurements taken from `find target -name '<pack>' -exec ls -l {} \;` after a `cargo build --features <pack>-scud`.)

**Locales covered vs deferred.** Phase 3's rule table lets any locale whose CLDR rules happen to be a subset of the encoded predicates ship without further work. Wave 4 above brings the shipped set to **eleven** locales: `en`, `de`, `fr`, `es`, `pt`, `it`, `ru`, `pl`, `ar`, `zh`, `ja`. Italian shipped its base language-pack crate (`stringcheese-it`) in Wave 4 with a minimum-viable `Language` trait impl (~30-word MVP stopword list, identity stemmer, tokenizer wrapper, no phonetic encoder); a full Snowball Italian stemmer, an Italian PHONEX phonetic encoder, and an `it-CH` regional-variant SCUD pack are the documented follow-ups. The full ~200-locale CLDR plural table remains a Phase-3-scope deferral: hand-encoding the remaining predicates (locale-specific reorderings, additional operand combinations) is a follow-up wave.

**Deferred.** Documented in the crate roots and left for the follow-up wave:

- Standalone WASM component builds for `stringcheese-icu-plural` and `stringcheese-icu-number`. Both WIT files parse cleanly under `wit-parser`; the `wit-bindgen` `Guest` implementations and `cargo build --target wasm32-wasip1 --features wit-component` recipes land in a follow-up (same shape as Phase 1's / Phase 2's deferrals).
- Full ~200-locale CLDR plural rules (Phase 3 targets ~13 covered predicates across the shipped `en`, `de`, `fr`, `es`, `pt`, `it`, `ru`, `pl`, `ar`, `zh`, `ja` packs).
- **Snowball Italian stemmer** (`stringcheese-it`). Ships an identity stemmer in Wave 4; a pure-Rust port of Martin Porter's Italian algorithm (matching the sibling Latin-script packs' pattern) is the follow-up. The full ~280-entry `italian/stop.txt`-derived stopword list ships alongside that work.
- **Italian PHONEX phonetic encoder** (`stringcheese-it`). The base pack ships `phonetic_encoder() → None`; a PHONEX-Italian variant with double-consonant collapse (`piatto → PIATO`), palatal digraph handling (`gn`, `gl`, `sc + i/e`), and vowel-length sensitivity is the follow-up.
- **Regional variant `it-CH`** (Italian-Switzerland). The base `stringcheese-it` pack ships CHF in its currency table so a fallback resolves cleanly, but Swiss group / decimal separators differ from Italy conventions and warrant a dedicated SCUD pack — a follow-up.
- `pt-BR` cardinal variant. The shipped `pt` pack matches pt-PT rules (`n = 1 and v = 0` for `one`); CLDR default `pt` (used for pt-BR) has the looser `i = 0..1` rule (both 0 and 1 → `one`). A pt-BR-specific SCUD pack or a runtime override is a follow-up.
- Latin American Spanish variants (`es-MX`, `es-AR`, `es-419`). The shipped `es` pack matches Spain conventions (group `.`, decimal `,`); Latin American variants differ (Mexico uses group `,`, decimal `.`). Regional overrides ship as separate packs in a follow-up.
- CLDR `c` / `e` (compact / exponent) operands — Phase 3 evaluates only `n / i / v / w / f / t`. Compact notation (`1.2K`, `3.5M`) needs the `e ≠ 0` handling.
- Named-currency database — ISO 4217 → symbol lookup for every locale-currency pair. This crate reads whatever currencies the pack ships; a shipping locale-by-locale symbol table lives separately.
- Compact number formatting ("1.2K", "3.5M") — CLDR "short" pattern.
- Scientific notation and accounting-style negative currency.
- Percent output with the localized percent sign U+066A (Arabic-Indic) or U+FF05 (fullwidth) — Phase 3 packs only ASCII `%`.
- **Arabic RTL bidi handling.** `number-ar.scud` emits logical-order strings; a downstream shaper is responsible for the visual reversal Arabic contexts expect.
- **Arabic-Indic (`arab` U+0660..U+0669) / Persian-extended (`arabext` U+06F0..U+06F9) digit shapes.** The Arabic pack ships the `latn` numbering system (Western digits `0..9`); alternate digit tables need a SCUD extension or a per-numbering-system pack. Additional numbering systems (`deva`, `thai`, etc.) share the same deferral.
- **Per-currency fraction-digit defaults.** JPY-style zero-fraction currencies (JPY, KRW, ISK, …) format as `¥1,234.56` by default under the current engine because `format_currency` forces 2 fraction digits regardless of the currency code. Callers who want the culturally-correct output pass `FormattingOptions { min_fraction: Some(0), max_fraction: Some(0), … }`. The Japanese pack docs and the `currency_jpy_zero_fraction_override` golden test document the workaround.

**Red flags.** A couple worth noting for reviewers:

- **Half-even rounding of banker's ties.** CLDR's reference `NumberFormat` defaults to `HALF_EVEN`; Rust's default `{:.N}` also uses half-even. Golden vector `0.125 → "12%"` and `0.135 → "14%"` reflect this — a reviewer used to `HALF_UP` (JavaScript's `toFixed`) will find these surprising, but they match CLDR + Rust and are the documented behaviour.
- **French `many` cardinal deferred.** French CLDR ships a `many` bucket triggered by `e = 0 and i != 0 and i % 1000000 = 0 and v = 0 or e not in 0..5` — the compact-large-number case. Phase 3 packs `fr` cardinal as `one` (0..1) → `other`. Callers formatting compact numbers may see wrong CLDR classes; the deferral is documented in the FR pack docs.
- **French group separator.** CLDR 42+ uses NARROW NO-BREAK SPACE (U+202F) as the French group separator. Phase 3 ships U+00A0 (NO-BREAK SPACE) for downstream-compatibility with tools that predate the CLDR 42 change. The rendering difference is subvisual; the byte-level difference matters for any consumer that string-matches the separator.
- **`f64` fraction extraction is lossy for edge-case inputs.** `PluralOperands::from_f64` uses Rust's default `{}` formatting to recover the visible-fraction shape. `1.0` and `1.00` are indistinguishable as `f64`; we report v=0 for both. This is a documented Phase 3 simplification; callers who care about the visible-fraction distinction should compute operands from their text representation.

### 8.2. Phase 2 progress (2026-08)

**Landed.** The Phase 2 foundation ships across a single wave:

- `stringcheese-scud` — extended with the `CAP_COLLATION` capability tag (already reserved in Phase 1), a `CollationDataView` zero-copy view over the body, and the `CollationSectionBuilder` writer alongside the existing `CaseSectionBuilder`. Two new section ids: `SECT_EXPANSIONS` (character-expansion table sharing the wire format of `SECT_FULL_UPPER`) and `SECT_COLLATION_OPTIONS` (a 4-byte options blob carrying the pack's default strength + case-insensitivity bit). +5 new unit tests.
- `stringcheese-icu-collation` — the WIT interface at `component/wit/collation/stringcheese-icu-collation.wit` (parses cleanly under `wit-parser`; 3 smoke tests assert package name / version / interfaces / world), plus the `CollationEngine` algorithm side that consumes one or more `CollationPack`s and delegates to the existing `stringcheese-collate::UcaCollator` (feruca) for the UCA compare. The engine walks the BCP 47 fallback chain (`de-DE → de → ""`) at query time. 14 unit tests.
- `stringcheese-en` — `collation-en.scud` shipping DUCET-root-plus-ligature expansions (Æ/æ/Œ/œ → AE/ae/OE/oe). Exposed via `collation_data::COLLATION_EN_SCUD` and `collation_data::collation_pack()` under the `collation-scud` cargo feature (default on). 14 golden-vector test functions totaling ≥ 101 assertions covering primary/secondary/tertiary strengths, sort-key consistency, and empty-input boundaries.
- `stringcheese-de` — `collation-de.scud` shipping the DIN 5007-2 (phonebook) tailoring: `ß → ss`, `ẞ → SS`, `ä → ae`, `Ä → AE`, `ö → oe`, `Ö → OE`, `ü → ue`, `Ü → UE`. The design commits to phonebook ordering as the shipped default because it is the more distinctive convention (dictionary ordering agrees with English tertiary compare on the same inputs); DIN 5007-1 dictionary ordering remains available via the native `stringcheese_de::GermanCollator::DIN_5007_DICTIONARY` preset. 14 golden-vector test functions totaling ≥ 52 assertions.
- UCA conformance subset — 100 hand-authored ordered pairs at primary/secondary/tertiary strengths, tested for compare-passes-through-DUCET, antisymmetry, and sort-key/compare consistency at every strength.
- CI — new `wasm-i18n-collation` job in `.github/workflows/ci.yml` builds both packs, runs the golden + UCA subset suites, and prints per-locale SCUD sizes to the run log.

**Measured sizes** (release builds of the SCUD blobs, uncompressed):

| Pack                | Bytes | Notes                                                          |
| ------------------- | -----:| -------------------------------------------------------------- |
| `collation-en.scud` |   126 | DUCET root + Æ/æ/Œ/œ ligature expansions                        |
| `collation-de.scud` |   194 | DIN 5007-2 (phonebook): ß, ẞ, and the six umlauts               |
| Composed total      |   320 | Both packs loaded into one `CollationEngine`                    |

**Strength implementation.** Phase 2 approximates the UCA strength ladder by pre-folding before delegating to feruca (which does its own full CLDR-root walk internally at tertiary + shifted mode):

- **Primary** — pack-expand + strip ASCII case + strip combining marks (U+0300..U+036F and adjacent ranges).
- **Secondary** — pack-expand + strip ASCII case.
- **Tertiary** — pack-expand only; delegate to feruca for weight table walk.
- **Quaternary** — currently the same as Tertiary (feruca's shifted mode is already quaternary-aware for variable-weight punctuation).
- **Identical** — Tertiary with a full-input tiebreak on equal.

The `sort_key` implementation composes level-1 (primary-folded), level-2 (case-folded), and level-3 (case marker) sub-keys so a bytewise compare of two sort keys agrees with `compare` at the same strength on ASCII input.

**Deferred.** Documented in the crate roots and left for the follow-up wave:

- Full ~200 000-entry `CollationTest.txt` conformance. Phase 2 ships the hand-authored 100-entry subset; the full run is a follow-up.
- Cross-locale composition (loading `en` + `de` and switching per-string). The engine's fallback chain already accepts multiple packs; the "which pack does this per-string query use?" story is a Phase 3 concern.
- Precomposed accented-character decomposition. Phase 2's primary_fold strips combining marks only for the decomposed form (`e` + U+0301). Precomposed `é` (U+00E9) survives to feruca unchanged; that layer NFD-decomposes internally so the compare is correct, but the `sort_key` for such input reflects the precomposed byte and does not fold to the base letter. Fixing this requires either pulling in `unicode-normalization` at the icu-collation layer or shipping an NFD-decomposition SCUD section — deferred.
- SCUD support for empty-expansion entries (Thai / Lao / hyphen ignorables). The current writer's `encode_full_table` panics on empty target sequences; adding "collapse to empty" as a valid entry is a backwards-compatible loader upgrade.

**Landed after Phase 2 close.** Post-close follow-up commits fill in the deferred deliverables that did not gate the phase:

- Standalone WASM component build for `stringcheese-icu-collation`. The `stringcheese-icu-collation-component` sibling crate wraps the `CollationEngine` behind the `stringcheese:icu-collation@0.1.0` WIT `collation-world` world, mirroring the `stringcheese-icu-case-component` template's shape (dual `cdylib` + `rlib`, `wit-component` feature gate on `wit-bindgen-rt`, pre-generated `src/bindings.rs` from `wit-bindgen rust --runtime-path wit_bindgen_rt`, in-process `wasmtime::component::bindgen!` smoke tests). The shipped `.wasm` embeds the `collation-en.scud` and `collation-de.scud` packs so the componentised binary is drivable end-to-end without a separate pack component; a new `wasm-i18n-collation-component` CI job builds, componentises, reports sizes, and runs the wasmtime smoke test. Reference sizes measured locally (aarch64 macOS, `--release`, wasmtime 47 / wasm-tools 1.254): raw module 1 858 133 B, componentised 1 881 004 B; with `wasm-opt -Oz` applied pre-componentise: 1 755 452 B / 1 778 323 B respectively — larger than the icu-case component because the algorithm side pulls in `feruca` and its embedded UCA weight table (unlike case mapping which is table-lookup only). The 7 wasmtime smoke tests cover `compare` (English primary + case fold, German phonebook `Bär`/`Baer` and `Straße`/`Strasse`), `sort-key` monotonicity vs `compare`, and the `capabilities` introspection surface (`get-capabilities` returning `[en, de]` at max-strength Identical and CLDR version 44.1, plus BCP 47 fallback via `de-CH → de`).

### 8.1. Phase 1 progress (2026-08)

**Landed.** The Phase 1 foundation ships across four commits:

- `stringcheese-scud` — the SCUD file-format loader (wire format v1.0). Parses the header (magic / version / flags / capability tag / CLDR + locale annotations) and hands out per-capability views; the first capability, `CaseDataView`, covers simple + full lower/upper/fold plus contextual (locale-override, final-sigma) tables. Ships `ScudWriter` for build-time SCUD generation. 12 unit tests + 1 doctest.
- `stringcheese-icu-case` — the WIT interface at `component/wit/icu-case/stringcheese-icu-case.wit` (parses cleanly under `wit-parser`; 3 smoke tests assert package name / version / interfaces / world), plus the `CaseEngine` algorithm side that consumes one or more `CasePack`s and walks the CLDR fallback chain (`pt-BR → pt → ""`) at query time. 13 unit tests.
- `stringcheese-en` case-scud pack — a hand-verified, CLDR-44.1-derived `case-en.scud` blob (~2.3 KiB) covering ASCII, Latin-1 supplement, common Latin Extended-A, German ß full expansion (→ "SS" / "ss"), capital sharp S, and Œ/Æ ligatures. Exposed via `case_data::CASE_EN_SCUD` and `case_data::case_pack()` under the `case-scud` cargo feature (default on). 12 golden-vector test functions totaling ≥ 195 assertions.
- `stringcheese-tr` case-scud pack — a `case-tr.scud` blob (~200 B) carrying Turkish's dotted / dotless-I contextual overrides (`I → ı`, `i → İ`) plus symmetric simple pairs and the Turkish alphabet letters. Cross-locale composition exercised in `tests/case_cross_locale.rs`: an `[en_pack, tr_pack]` engine yields different output for the same input under `"en"` vs `"tr"`. 10 golden-vector test functions + 8 composition tests.
- CI — new `wasm-i18n-case` job in `.github/workflows/ci.yml` builds both packs, runs the golden + composition suites, and prints per-locale SCUD sizes to the run log.

**Measured sizes** (release builds of the SCUD blobs, uncompressed):

| Pack           | Bytes | Notes                                                          |
| -------------- | -----:| -------------------------------------------------------------- |
| `case-en.scud` | 2 277 | ASCII + Latin-1 + Latin-A + ß + ligatures                       |
| `case-tr.scud` |   201 | Turkish contextual overrides + alphabet + belt-and-braces ß     |
| Composed total | 2 478 | Both packs loaded into one `CaseEngine`                         |

**Deferred.** Documented in the crate roots and left for the follow-up wave:

- Outer stream compression (Brotli, Zstd). SCUD flag bits 0 and 1 are reserved; the loader currently rejects a compressed body with `ScudError::UnsupportedCompression` and shipped packs write the raw layout. A decompression pass is a backwards-compatible loader upgrade.
- Structural compression primitives (`RangeDelta`, `AdaptivePages`, `PackedIntegers`, `SequencePool`, `StringPool`, `LoudsTrie`, `FiniteStateTable`). Phase 1 uses plain sorted `(u32, u32)` tables — enough for the Latin + Turkish subset without pulling in an FST library. The ~2.5 KiB total is well under the design's per-locale budget.
- Final-sigma tailoring for Greek. `stringcheese_scud::ContextKind::FinalSigma` is reserved in the SCUD format; the algorithm wires it in when the Greek pack ships case data.
- Full CLDR title-casing (Dutch `ij` digraph, UAX #29 word-break integration). Phase 1 `to_title` handles the ASCII common case; the full logic lands alongside the `stringcheese-icu-break` capability crate.

**Landed after Phase 1 close.** Post-close follow-up commits fill in the deferred deliverables that did not gate the phase:

- Standalone WASM component build for `stringcheese-icu-case`. The `stringcheese-icu-case-component` sibling crate wraps the `CaseEngine` behind the `stringcheese:icu-case@0.1.0` WIT `case` world, mirroring the `stringcheese-tokenizer-component` template's shape (dual `cdylib` + `rlib`, `wit-component` feature gate on `wit-bindgen-rt`, pre-generated `src/bindings.rs` from `wit-bindgen rust --runtime-path wit_bindgen_rt`, in-process `wasmtime::component::bindgen!` smoke tests). The shipped `.wasm` embeds the `case-en.scud` and `case-tr.scud` packs so the componentised binary is drivable end-to-end without a separate pack component; a new `wasm-i18n-case-component` CI job builds, componentises, reports sizes, and runs the wasmtime smoke test. Reference sizes measured locally (aarch64 macOS, `--release`, wasmtime 26 / wasm-tools 1.254): raw module 83 562 B, componentised 112 305 B; with `wasm-opt -Oz` applied pre-componentise: 57 426 B / 86 169 B respectively — under the design's 40 KB pure-algorithm floor once the two locale packs are stripped, and comfortably within the reference component's ~100 KB budget with them embedded.

---

## 9. Alternatives considered

- **Just use ICU4X.** Discussed in §1.2. The right choice for callers who need ICU semantics; StringCheese is not an ICU replacement but a smaller, WIT-native alternative for callers who need capability-negotiated composition. Where ICU4X is what the caller needs, the recommendation is unambiguous: use ICU4X directly and skip this layer.
- **Build on ICU4X's `DataProvider`s rather than SCUD.** Tempting because well-designed; rejected because `DataMarker` types encode ICU-adjacent binary shapes. A SCUD-that-satisfies-ICU4X would either bloat to match every marker or be an incomplete implementation callers hit rough edges on. Better to own the format end-to-end and offer an ICU4X interop adapter for callers who want both.
- **Static-linked ICU for Wasm.** Rejected: 30 MB defeats the "under 40 KB Wasm floor" commitment in [wasm-and-wit-interface.md § Binary size targets](./wasm-and-wit-interface.md#binary-size-targets). Even ICU4C's "minimal" build is disproportionate.
- **Server-side i18n only.** Rejected because the umbrella's component-model story is explicitly client-side too: browser tabs, edge workers, embedded Wasm. Deferring to the server forfeits half the deployment surface StringCheese exists to serve.
- **A single monolithic `stringcheese-icu` crate.** Simpler to consume; violates per-capability pluggability. Rejected.
- **Ad-hoc data format per capability.** Simpler to bootstrap; the shared loader is the payoff — a bug fix in RangeDelta decoding fixes every capability at once. Rejected.

---

## 10. Open questions

Explicitly unresolved; each is a "revisit in review" bullet.

- **Measured compression ratios.** The design leans on the structural primitives + outer Brotli/Zstd achieving 10-100× shrink vs a naive representation. That needs to be *measured*, per capability per locale, and reported. The number the charter should commit to is the *worst* ratio across shipped packs, not a hero number.
- **Build-tool distribution.** Is `stringcheese-scud-build` (regenerates SCUD from CLDR JSON) shipped as a crate, a binary, or both? Callers who want to rebuild their own packs need it; the project's CI needs it. A hosted build service would be nice but out of scope for the initial phases.
- **Multi-language capability negotiation.** When a caller composes `stringcheese-en` and `stringcheese-ja` alongside `stringcheese-icu-case`, both packs export `case-data`. Composition must merge, not conflict. Possibly a `stringcheese-icu-registry` component that fans in per-locale packs and re-exports the merged interface.
- **Explicit WIT authorship vs codegen.** Do pack authors write WIT for their locale, or does a proc-macro generate the pack's exports from its Cargo features? Macro is nicer for contributors; explicit WIT is nicer for auditors. Probably: generate by default, allow override.
- **ICU4X interop.** For callers who need SCUD's small footprint for the common case but ICU4X's depth for one particular locale or operation, is there a bridge crate? Sketch: `stringcheese-icu-x-bridge` adapts an ICU4X `DataProvider` into a `LanguageProvider` back-end so the same runtime dispatches to whichever source has data.
- **Streaming interfaces at the WIT boundary.** Break iteration on a large document is naturally streaming, but `func(input: string) -> list<u32>` materialises the whole boundary list. A follow-up may want the same chunked pattern used for rolling hashes and CDC (see [wasm-and-wit-interface.md § Streaming across component boundaries](./wasm-and-wit-interface.md#streaming-across-component-boundaries)).
- **Root-pool sharing.** §4.2 mentions a "root" SCUD whose SequencePool is referenced by per-locale files. Distribution mechanism unresolved — in `stringcheese-scud`, a separate `stringcheese-icu-root` crate, or something else. `wasm-tools compose` semantics may or may not make cross-file pool references practical.

---

## 11. Cross-references

- Umbrella charter for the ICU-alternative direction: [DESIGN.md § Vision](../DESIGN.md), [DESIGN.md § Scope](../DESIGN.md), [DESIGN.md § Sub-project map](../DESIGN.md).
- WIT interface conventions (byte-oriented types, resource lifetimes, streaming): [wasm-and-wit-interface.md](./wasm-and-wit-interface.md).
- Feature-gate discipline informing per-capability à-la-carte features: [wasm-and-wit-interface.md § Feature-gate strategy](./wasm-and-wit-interface.md#feature-gate-strategy).
- Result-type conventions (why `case-error` is a `variant` rather than a `string`): [type-system.md](./type-system.md).
- Preprocessing-pipeline consumers of case mapping and folding: [preprocessing-pipeline.md](./preprocessing-pipeline.md).
- Existing `stringcheese-unicode` primitives that overlap (and delegate to, for graphemes): [`crates/stringcheese-unicode/src/lib.rs`](../../crates/stringcheese-unicode/src/lib.rs).

## 12. Prior art and references

- **ICU4C** — <https://icu.unicode.org/> — reference C/C++ implementation.
- **ICU4X** — <https://github.com/unicode-org/icu4x> — Rust re-implementation.
- **CLDR** — <https://cldr.unicode.org/> — Common Locale Data Repository.
- **Unicode License** — <https://www.unicode.org/license.txt>.
- **UAX #21 (Case Mappings)** — <https://www.unicode.org/reports/tr21/>.
- **UAX #29 (Text Segmentation)** — <https://www.unicode.org/reports/tr29/>.
- **UAX #14 (Line Breaking)** — <https://www.unicode.org/reports/tr14/>.
- **UTS #10 (Unicode Collation Algorithm)** — <https://www.unicode.org/reports/tr10/>.
- **UTS #35 (LDML, incl. plural rules)** — <https://www.unicode.org/reports/tr35/>.
- **BCP 47 (language tags)** — Phillips & Davis (2009), RFC 5646, <https://datatracker.ietf.org/doc/html/rfc5646>.
- **WebAssembly Component Model** — <https://github.com/WebAssembly/component-model>.
- **WIT format** — <https://github.com/WebAssembly/component-model/blob/main/design/mvp/WIT.md>.
- **wasm-tools compose** — <https://github.com/bytecodealliance/wasm-tools>.
- **Brotli** — Alakuijala & Szabadka (2016), RFC 7932, <https://datatracker.ietf.org/doc/html/rfc7932>.
- **Zstandard** — Collet & Kucherawy (2021), RFC 8878, <https://datatracker.ietf.org/doc/html/rfc8878>.
- **LOUDS** — Jacobson (1989), "Space-efficient static trees and graphs", *30th Annual Symposium on Foundations of Computer Science*, pp. 549-554, <https://doi.org/10.1109/SFCS.1989.63533>. Engineering treatment: Delpratt, Rahman, & Raman (2006), "Engineering the LOUDS Succinct Tree Representation", *WEA 2006*, LNCS 4007, pp. 134-145, <https://doi.org/10.1007/11764298_12>.
- **Front-coded string dictionaries** — Witten, Moffat, & Bell (1999), *Managing Gigabytes* (2nd ed.), Morgan Kaufmann, ISBN 978-1-55860-570-1, Chapter 4.
- **Double-array tries** — Aoe (1989), "An Efficient Digital Search Algorithm by Using a Double-Array Structure", *IEEE TSE* 15(9), pp. 1066-1077, <https://doi.org/10.1109/32.31365>.
- **FSTs in text indexing** — Mihov & Maurel (2001), "Direct Construction of Minimal Acyclic Subsequential Transducers", *CIAA 2000*, LNCS 2088, pp. 217-229, <https://doi.org/10.1007/3-540-44674-5_18>.

Citations of primary Unicode / CLDR / RFC sources are preferred over pre-trained knowledge; where a citation is a book without a DOI, the ISBN is given.
