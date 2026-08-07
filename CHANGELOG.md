# Changelog

All notable changes to StringCheese are documented in this file.

The format is based on [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html).
Until the first `0.1.0` release, expect breaking changes on any minor
bump; `0.x` versions are pre-stability.

## [Unreleased]

### Added

- **`stringcheese-nl` — Dutch language pack.** New workspace crate. ~174
  stopwords, Snowball Dutch stemmer per the official spec: R1/R2 regions,
  `ij` digraph handling (only marked as consonant-like `I` when strictly
  between two vowels — `keien → kei`, `mijn → mijn`), `-en`/`-e` cascade
  with `gem` guard on `-en`, undouble step scoped to `kk`/`dd`/`tt`,
  Step 3b derivational suffixes that preserve `-cht`. PHONEX-Dutch
  encoder (`"phonex-nl"`) — Soundex-shape with Dutch-tuned preprocessing
  (`ij → I` collapse so `mijn` and `min` share a key; `sch → sX`
  sibilant+velar; `ch → g` velar; labial merger `B/P/F/V/W`). Registered
  as `"nl"`. 37 Snowball reference pairs + 20 PHONEX pairs. Belgian-Dutch
  (nl-BE), Afrikaans, Métaphone Dutch, compound-noun splitting, CLDR-tailored
  Dutch collator deferred.

- **`stringcheese-ru` — Russian language pack.** New workspace crate.
  **First Cyrillic-script pack.** ~235 stopwords (Cyrillic, plain-е form).
  Snowball Russian stemmer per the official spec — RV region, four steps
  (perfective gerund, reflexive, adjectival, verb + noun), `ё → е`
  precomputation runs before every stem call and every stopword
  comparison so `ёлка`/`елка` and `ЁЖ`/`еж` collapse identically. **All
  suffix/region/stopword arithmetic runs on `Vec<char>`** (never raw
  bytes) since every Cyrillic scalar is 2 bytes in UTF-8 and byte offsets
  would silently corrupt boundaries — this is the pattern the next
  Cyrillic pack (Ukrainian, Serbian, Bulgarian, ...) should copy. Case-fold
  uses `char::to_lowercase` (no Turkic-style tailoring needed).
  Transliteration encoder: **GOST 7.79-2000 System B** (deterministic
  Cyrillic → ASCII: `ж → zh`, `ч → ch`, `ш → sh`, `щ → shh`, `ц → cz`,
  `ъ → ''`, `ь → '`, `э → e'`, `ы → y`, `ё → yo`, `ю → yu`, `я → ya`).
  Adapter `"gost-7.79-b"`. Registered as `"ru"`. 45 Snowball reference
  pairs + 24 transliteration pairs (all 33 modern letters exercised).
  Ukrainian/Belarusian/Serbian/Bulgarian/Macedonian packs, Slavic-Metaphone,
  ISO 9 System A transliteration, pre-1918 orthography deferred.

- **UAX #29 word and sentence segmentation.** New `words` and `sentences`
  modules in `stringcheese-unicode`, feature-gated as `word-segmentation`
  and `sentence-segmentation` (both default-on, individually toggleable
  so wasm-size-conscious callers can shed either). `WordSequence` /
  `SentenceSequence` types wrapping `unicode-segmentation`'s iterators;
  `SplitWordBoundsBehavior` enum (`WordsOnly` vs `AllBoundaries`);
  `words(text)`, `word_bounds(text)`, `word_indices(text)`,
  `word_bound_indices(text)`, `sentences(text)`, `sentence_indices(text)`
  helpers. `IndexableSequence` impls included. Wires up four downstream
  stubs that were blocked on this API: `stringcheese-manip::split_words`,
  `stringcheese-manip::split_sentences`, tokenizer `WordSegmenter`,
  tokenizer `SentenceSegmenter`. Baseline wasm-size probe unchanged
  (default probe doesn't enable the new features); a new
  `unicode-with-segmentation` probe measures +59 KB for callers who
  turn them on. UAX #14 line breaking and locale-tailored break rules
  (`ja`, `ar`, Thai lexicon-based) deferred.

- **`stringcheese-ja`: Hepburn romanization and kana normalization.**
  Extends the Japanese pack with the two follow-ups the crate docs
  explicitly named. `HepburnRomaji` — Modified Hepburn (macrons `ō`/`ū`
  on long vowels, `ei` for `えい`, `oo → ō` for same-vowel doubles,
  small-tsu geminates, small-y digraphs, shibboleths `shi`/`chi`/`tsu`/
  `fu`/`ja`). `JapaneseWithHepburn` sibling type, `Japanese::with_hepburn_encoder()`
  const factory, `JAPANESE_WITH_HEPBURN` const. Kunrei-shiki remains
  the default `Language::phonetic_encoder`. `KanaNormalizer` builder
  with 4 opt-in flags: full-width↔half-width ASCII, half-width→full-width
  katakana (incl. dakuten/handakuten combining), katakana↔hiragana fold,
  dakuten canonicalization (`<base> + U+3099 → precomposed`). Default
  preset applies only the two lossless passes (half-width widening +
  dakuten canonicalization). 46 Hepburn reference pairs. Word-boundary
  macrons (requires morphological analyzer), Nihon-shiki as a third
  romanization, full NFKC integration deferred.

- **DIN 5007 German collators.** New `GermanCollator` in
  `stringcheese-de` with two DIN 5007 presets — variant 1 (dictionary,
  umlauts fold to base: `ä = a`, `ö = o`, `ü = u`, `ß = ss`; "Bär" sorts
  with "Bar") and variant 2 (phonebook, umlauts expand to digraphs:
  `ä = ae`, `ö = oe`, `ü = ue`, `ß = ss`; "Bär" sorts as "Baer"). Preset
  consts `GermanCollator::DIN_5007_DICTIONARY`, `::DIN_5007_PHONEBOOK`,
  `::ASCII`. `GermanCollatorChoice` enum field on `German` with
  `with_din5007_variant1()`, `::with_din5007_variant2()`, `::with_ascii_collator()`
  const constructors; new `GERMAN_WITH_DIN5007_DICTIONARY` and
  `GERMAN_WITH_DIN5007_PHONEBOOK` consts. **On by default** (compile-time
  tables, no ICU4X, zero new deps) — unlike the English CLDR collator
  which is opt-in behind a feature. Reference orderings covered:
  Müller/Muller/Munk/Muster divergence, Straße/Strasse identity,
  Bär/Bar/Baer three-way. Austrian ordering variants, Swiss no-ß
  ordering, CLDR-de opt-in feature deferred.

- **`stringcheese-ar`: digit normalization and tatweel handling.**
  Three new opt-in flags on `ArabicNormalizer::builder()`:
  `with_western_digits(bool)` (Arabic-Indic `٠-٩` U+0660..=U+0669 and
  Extended Arabic-Indic `۰-۹` U+06F0..=U+06F9 → Western `0-9`),
  `with_eastern_digits(bool)` (reverse direction), `with_strip_tatweel(bool)`
  (remove `ـ` U+0640). New `ArabicNormalizer::DEFAULT_FOR_SEARCH` preset
  turns tatweel stripping on. **No default behavior change** — free
  `normalize()`, `::new()`, `::default()`, and all `Language` paths on
  `ARABIC` remain byte-for-byte identical. Read-back accessors added.
  Alef-hamza on waw/yeh/standalone, yeh carrier variants (Persian yeh
  U+06CC, yeh barree U+06D2), Persian-kaf (`ک` U+06A9 → `ك` U+0643),
  ZWNJ handling deferred to a future `stringcheese-fa` pack.

- **`stringcheese-pt` — Portuguese language pack.** New workspace crate.
  ~203 stopwords (SER/ESTAR/HAVER/TER paradigms plus Snowball's ranked
  head). Full Snowball Portuguese stemmer per the official `.sbl` spec —
  three-branch RV computation, all suffix cascades (steps 1–5), postlude
  accent-fold. Uses a placeholder mechanism `ã → a~` / `õ → o~` during
  the cascade so nasals can't be chewed by unrelated `-o` suffixes; the
  postlude walks the buffer forward and folds them back. PHONEX-Portuguese
  encoder (`"phonex-pt"`) — Soundex-shaped 4-char key with Portuguese-tuned
  preprocessing (Ç→S, LH→L, NH→N, CH→X, QU→K, `ão` collapse, accent fold).
  Cedilla is intentionally lossless for the sibilant class: `Coração` and
  `Coracao` produce different keys (the cedilla marks the /s/
  pronunciation). Registered as `"pt"`. 40 Snowball reference pairs +
  17 PHONEX pairs. pt-BR variant, Métaphone Português, Beider-Morse
  Portuguese, CLDR-tailored Portuguese collator, verb-conjugation
  lemmatization deferred.

- **`stringcheese-tr` — Turkish language pack.** New workspace crate.
  ~192 stopwords, Snowball Turkish stemmer with vowel-harmony rules.
  First language pack with **Turkic case-fold semantics**: an internal
  `case_fold` module implements `'I' → 'ı'`, `'İ' → 'i'` (distinct from
  Unicode default). Rationale: pulling in `stringcheese-unicode`'s
  ICU4X-backed Turkic path roughly doubles the pack's wasm footprint;
  the internal path is a single scalar rewrite. Stemmer, phonetic
  encoder, and `Language::is_stopword` all route through the Turkic
  fold. Idempotence property-tested. `TurkishPhonex` encoder
  (`"phonex-tr"`) — 4-char Soundex-shape with Turkic pre-fold and
  ASCII narrowing (ç→c, ğ→g, ı→i, ö→o, ş→s, ü→u). 31 Snowball reference
  pairs. **Deliberate deviation from Snowball spec:** the stemmer uses
  a unified longest-match across all suffix tables per iteration rather
  than the strict three-phase pipeline — better on ambiguous inputs
  (`tuzsuz` picks `-suz` derivational over `-uz` nominal-verb), and
  documented as a practical variant. Full-corpus Snowball cross-verify,
  consonant-alternation restoration (kitab → kitap), and Métaphone-shaped
  variable-length encoder deferred.

- **`stringcheese-tokenizer-bpe`: full regex pre-tokenizer (Phase 2b).**
  Replaces the `PreTokenizerRegex::Literal` stub with a real regex-backed
  `RegexPreTokenizer`. Chose **`fancy-regex 0.14`** over `regex` (no
  look-around) and `regex-lite` (no Unicode categories) — the tiktoken
  canonical pattern needs both `\p{L}`/`\p{N}` and a `\s+(?!\S)`
  negative-lookahead. Same crate upstream tiktoken's own Rust impl uses.
  Compiles clean on `wasm32-wasip1`. New optional `dep:fancy-regex` gated
  under the `std` feature. Exposes `RegexPreTokenizer::tiktoken_canonical()`,
  `::gpt2()`, `split(text)`, `split_ranges(text)`. 21 new unit tests
  covering word/punctuation/contraction/digit-group/Unicode/whitespace
  splits and multi-byte byte-offset correctness. Byte-identical tiktoken
  IDs still require real `mergeable_ranks` blobs plus the O(n log n)
  encoder — this landing removes the last algorithmic blocker.

- **`stringcheese-tokenizer-bpe`: O(n log n) linked-list + min-heap
  encoder.** Replaces the naive O(n²) merge loop with an arena-based
  linked list + `BinaryHeap<Reverse<HeapEntry>>` min-heap
  (`(rank, left_idx, right_idx)` ordering). Lazy stale-entry deletion on
  heap pop — verify both endpoints alive, `left.next == right`, and
  merge-table rank unchanged; discard otherwise. Zero new deps (uses
  `alloc::collections::BinaryHeap`). Naive form retained as
  `#[cfg(test)] fn merge_loop_naive` and used as the oracle: exhaustive
  agreement over all 127 strings of length 0..=6 over `{a,b}` and all
  121 strings of length 0..=4 over `{a,b,c}` with dense merge tables,
  plus a 512-case proptest with random inputs × random tables, plus a
  UTF-8 round-trip proptest. Measured speedup (release, `a^n` chained
  merges): 3.8× at n=50, 7.4× at n=100, 14.2× at n=200, 27.3× at n=400
  — doubles per doubling of n, matches asymptotic delta. Downstream
  `stringcheese-tokenizer-tiktoken` still passes 24/24 tests.

- **Wide-block SIMD Jaro and OSA on wasm32 (simd128).** Closes the SIMD
  matrix on wasm32 — extends the wave-5 wasm Levenshtein pattern to the
  two other SIMD-dispatched families. Jaro: 16-byte-block via
  `u8x16_splat` + `u8x16_eq` + `u8x16_bitmask` (which returns `u16`
  directly on wasm, unlike SSE2's i32). OSA: Hyyrö 2003 bit-parallel
  wide-block for `64 < m ≤ 128` on v128, with cross-lane carry via
  `u64x2_extract_lane` / `u64x2_replace_lane` scalar hop (wasm SIMD has
  no whole-register byte-shift on the u64 lane dimension). 128-bit
  big-integer add via lane-extract → scalar `overflowing_add` → lane-replace,
  same shape as SSE2's `add128`. `v128_andnot(a, b) = a & ~b` (opposite of
  SSE2's `_mm_andnot_si128(a, b) = ~a & b`), so operand order is reversed
  relative to the SSE2 code. 27 new SIMD-gated tests; wasmtime-driven full
  test run reports 616 passed. Block-form OSA for `m > 128` and relaxed-simd
  extensions deferred.

- **CDC SIMD scaffolding for all four rolling hashes.** New `simd` feature
  in `stringcheese-cdc`. Adds `<hash>/simd/{mod, scalar, x86_sse2,
  x86_avx2, aarch64_neon, wasm_simd128}.rs` scaffolding for Gear, Buzhash,
  Polynomial, and Rabin. Runtime dispatcher (`is_x86_feature_detected!`)
  routes `digest_of_slice` through the best available backend. **Present
  landing ships the scalar core under `#[target_feature(enable = "...")]`
  gating on every arch** — rolling hashes are strictly sequential
  (`state_{n+1}` depends on `state_n`), so real per-byte SIMD lifts need
  algorithm reformulation (Gear's block form `state_k = state_0 << k +
  Σ G[b_i] << (k-1-i)` for k ≥ 64; Buzhash v128 rotate-XOR; `pclmulqdq`
  GF(2) reduction for Rabin; AVX-512 IFMA Polynomial). Scaffolding + API
  + differential-test harness (short random, chunk boundaries 1..129,
  window-sized, 512 B–16 KiB blobs) is stable so real kernels drop in
  without churn. Crate root relaxed `forbid(unsafe_code)` → `deny` for
  the target_feature attribute. 87 crate tests pass; wasm build clean
  under `RUSTFLAGS="-C target-feature=+simd128"`.

- **Java bench adapter.** `bench-adapters/java/` — fifth non-Rust adapter
  after Python (wave 3), JavaScript (wave 4), Go (wave 5). Uses
  **Chicory 1.7.3** (dylibso pure-Java wasm runtime, no JNI) — chosen to
  match the wazero-on-Go ethos: `mvn test` works on any JDK 17+ with no
  platform-native library. Component-model workaround mirrors the Go
  adapter: shell out to `wasm-tools component unbundle` on first
  construction, extract the inner core module, run in Chicory with its
  built-in `WasiPreview1` shim. Cache-once. `STRINGCHEESE_CORE_WASM` env
  override. Five JMH benchmark classes (`LevenshteinBenchmark`,
  `HammingBenchmark`, `JaroBenchmark`, `DamerauBenchmark`, `LcsBenchmark`)
  crossing `length ∈ {8, 32, 128, 512, 2048} × regime ∈ {random, similar,
  identical}`. SplitMix64 corpus generator ports byte-for-byte from the
  Rust/Go/Python/JS side (via `Long.remainderUnsigned` for u64 semantics)
  so cross-adapter datapoints share exact corpora. Compared against
  `apache-commons-text` (Levenshtein, Jaro-Winkler, LongestCommonSubsequence)
  and `info.debatty/java-string-similarity` (10+ metrics including OSA,
  Jaro-Winkler, Damerau, LCS). `SmokeTest`: 14 subtests all pass.
  Chicory-native Component Model, GraalVM native-image path, and
  rapidfuzz-equivalent Java binding deferred.

- **`stringcheese-tokenizer-tiktoken` — OpenAI tiktoken model tokenizer
  pack (Phase 3).** New workspace crate. Feature-gated variants:
  `cl100k_base` (default), `p50k_base`, `r50k_base`, `o200k_base`.
  SCUD-lite BPE data format with `miniz_oxide` deflate (pure-Rust,
  wasm-portable — chosen over Brotli which requires a C shim or a
  substantially heavier pure-Rust decoder). `TiktokenPack::get()` /
  `try_get()` return `&'static BpeTokenizer`; one `pub static` per
  enabled variant. Build script synthesises deterministic small
  stand-in packs into `OUT_DIR` when contributors haven't dropped
  real `data/<variant>.tiktoken` blobs (OpenAI's ~5 MB
  mergeable_ranks tables are not shipped in-tree per license +
  bloat). Facade deliberately unlinked — the umbrella `stringcheese`
  crate stays opt-in. Uses the placeholder whitespace pre-tokenizer
  from Phase 2; real regex pre-tokenization for tiktoken-identical
  IDs remains Phase 2b.

- **`stringcheese-es` — Spanish language pack.** ~269 stopwords
  (canonical accent-preserving form, NLTK-scale). Snowball Spanish
  stemmer per `docs/design/snowball-es.md` — full three-branch RV
  computation, pronoun-strip cascades (iéndo/ándo/ár/ér/ír, ando/
  iendo/ar/er/ir, uyendo), postlude accent-fold. **Phonetic
  encoder: PHONEX-Spanish** (option 3), a Soundex-shaped 4-char key
  tuned for Spanish phonology (Ñ→N, LL→L, CH→X, QU→K, RR→R, PH→F,
  GN→N, silent H, Z→S seseo, V→B betacismo, accent fold).
  Registered as `"es"`. Reference-tested against 52 Snowball pairs
  and 17 PHONEX pairs. Kondrak/Metaphone-Español/Beider-Morse
  deferred (no single dominant published spec).

- **`stringcheese-ar` — Arabic language pack.** ~148 stopwords, ISRI
  Larkey light10 morphological stemmer, Buckwalter transliteration
  as the phonetic encoder (bijective on the mapped subset —
  `Buckwalter::inverse()` round-trips). ~106 tests total. First
  right-to-left-script pack — the crate processes strings in
  **logical UTF-8 order** (first-consonant-first); RTL rendering is
  a display-layer concern. Stemmer preserves classical single-pass
  semantics (avoids over-stripping `الوقت → قت`); property test
  checks bounded convergence rather than strict idempotence. Teh
  marbuta folding is opt-in via `ArabicNormalizer` builder.
  Registered as `"ar"`. Root-and-pattern morphology, dialect
  stopwords (Egyptian/Levantine/Gulf), AraSoundex/ISRI phonetic
  deferred.

- **`stringcheese-en`: opt-in CLDR-tailored collator via icu_collator.**
  New feature `icu-collator` (requires `std`, adds `icu_collator` +
  `icu_locid` + `icu_provider/sync` dependencies). `EnglishCldrCollator`
  wraps ICU4X's collator behind a `OnceLock` for lazy init;
  `CldrCollatorOptions` exposes `strength` (default Tertiary),
  `numeric` (natural-number digit-run ordering), and `case_level`.
  New `EnglishCollatorChoice` enum field on `English` with
  `with_ascii_collator()` (always available) and (feature-gated)
  `with_cldr_collator()` const constructors. New
  `ENGLISH_WITH_CLDR_COLLATOR` const. `Language::collator` dispatches
  on the choice. Feature isolation verified — default builds pull
  no ICU4X. Observed size cost: ≈+52 KB in the rlib.

- **Wide-block SIMD for Jaro and OSA.** Extends the Levenshtein
  wide-block pattern to two more distance algorithms across all
  three host arches:
  * **Jaro**: SSE2 (16-byte blocks via `_mm_cmpeq_epi8` +
    `_mm_movemask_epi8`), AVX2 (32-byte blocks via
    `_mm256_cmpeq_epi8` + `_mm256_movemask_epi8`), NEON (16-byte
    blocks via `vceqq_u8` + `vshrn_n_u16::<4>` narrow-mask idiom).
    New shared `jaro/simd/common.rs` with a packed word-backed
    `Bitmap` supporting straddling 16/32-bit window reads.
  * **OSA**: Hyyrö 2003 bit-parallel with cross-lane carry. SSE2
    and NEON wide-block cover `64 < m ≤ 128`, AVX2 wide-block
    covers `128 < m ≤ 256`. Algorithm was validated against a
    Python-form prototype (0 failures over ~1M random pairs at
    both 128-bit and 256-bit) before intrinsic port.

  Measured perf on aarch64: OSA at n=128 jumps from ~48 µs
  (scalar rolling-rows) to ~1.7 µs (NEON wide-block) — ~28×.
  Jaro at n=128 sees ~10 % improvement on random inputs; similar/
  identical inputs regress slightly because the algorithm
  early-exits, so per-block SIMD setup dominates on short match
  paths (correctness is exact; perf tuning is a follow-up).

- **wasm SIMD (simd128) Levenshtein backend.** Closes the SIMD
  matrix on the wasm32 target. New
  `crates/stringcheese-compare/src/levenshtein/simd/wasm_simd128.rs`
  using `core::arch::wasm32` v128 intrinsics with 128-bit
  wide-block Myers for `64 < m ≤ 128`. Compile-time gated on
  `target_feature = "simd128"` (no runtime detection on wasm).
  Cross-lane carry via `u64x2_extract_lane` / `u64x2_replace_lane`
  scalar hop — wasm SIMD has no `_mm_slli_si128` equivalent that
  cleanly expresses a whole-register 1-bit shift. Verified
  end-to-end under wasmtime 47.0.2 (588 wasm tests pass including
  9 new boundary + differential cases). No default-build size
  impact — the module is `#[cfg(feature = "simd")]`-gated and the
  wasm-size probes don't enable `simd`. Bench extension deferred.

- **Go bench adapter (wazero).** `bench-adapters/go/` — fourth
  non-Rust adapter after Python and JavaScript. Uses **wazero
  v1.9.0** (pure-Go, no CGO — chosen over wasmtime-go, whose
  upstream is archived and adds a heavy CGO dep). Wazero doesn't
  yet run Component Model, so the adapter shells out to
  `wasm-tools component unbundle` on first construction to extract
  the inner core module; cached under
  `component/rust-host/target/.../unbundled/`. Canonical ABI wired
  by hand: `cabi_realloc`, return-area pointers for
  `result<u32, string>` and `variant bounded-distance`,
  `cabi_post_hamming` for free. Compared against
  `agnivade/levenshtein` (Levenshtein-only) and `hbollon/go-edlib`
  (kitchen-sink pure-Go distance library — no rapidfuzz for Go).
  `TestSmoke` covers 13 subtests across all 8 exposed algorithms;
  full bench matrix (5 lengths × 3 regimes × 6 competitors) runs
  end-to-end.

- **`stringcheese-tokenizer` + `stringcheese-tokenizer-bpe` — tokenizer
  subsystem, Phases 1+2.** Two new workspace crates per
  `docs/design/tokenizers.md`:
  * `stringcheese-tokenizer` — `Segmenter` (GAT-based, non-round-trip)
    and `Tokenizer` (round-trip; `encode`/`decode`/`count`) traits;
    `Encoding<Token>` with offsets + special_mask; built-in
    tokenizers: `WhitespaceTokenizer`, `DelimiterTokenizer`,
    `IdentifierTokenizer` (5 modes: SnakeCase/KebabCase/DottedPath/
    CamelCase/Auto), `GraphemeSegmenter` (wraps
    `stringcheese-unicode`), `NgramSegmenter`. Facade re-exports as
    `stringcheese::tokenizer`.
  * `stringcheese-tokenizer-bpe` — data-neutral BPE algorithm
    (Sennrich et al. 2016). `BpeMergeTable` + `BpeVocabulary` +
    `BpeTokenizer::from_parts()`. Special-token handling
    (longest-first, literal match). Round-trip verified: `decode(encode(x)) == x`.
    Regex pre-tokenization stubbed as `PreTokenizerRegex::Literal`
    (full regex is Phase 2b). Naive O(n²) merge loop — linked-list +
    min-heap is Phase 2 optimization.

- **`stringcheese-ja` — Japanese language pack.** First non-Latin-script
  pack. ~141 stopwords (particles, auxiliaries, demonstratives,
  pronouns, adverbs), character-type tokenizer (range-based
  Hiragana/Katakana/Kanji/Latin/digit classification with
  Kanji+Hiragana okurigana merging), Kunrei-shiki romanization
  (ISO 3602) as the phonetic encoder (chosen over Hepburn for
  key-stability: `し → si`, `じ → zi`), minimal polite-form /
  plural-marker stemmer. Public `JAPANESE` constant. Registers into
  `stringcheese-lang::registry` as `"ja"`. Full morphological
  tokenization deliberately deferred (kuromoji-scale dictionary
  is outside the wasm-first / offline-first envelope).

- **JavaScript bench adapter.** `bench-adapters/js/` — third non-Rust
  adapter after Python. Uses `@bytecodealliance/jco@1.27.0` to
  transpile the WIT component to ES modules that Node.js can
  import. Compares StringCheese vs `fastest-levenshtein`,
  `js-levenshtein`, `natural`, `string-similarity`. Uses
  `tinybench` (lightweight ESM-native harness). Verified end-to-end
  with an actual bench run. `damerauDistance` throws
  `NotImplementedError` (upstream WIT surface gap — full Damerau
  kernel isn't wasm-portable yet).

- **`stringcheese-en`: English collation + contraction tokenization.**
  Two additive features:
  * `EnglishCollator` implementing `stringcheese_lang::Collator` with
    three flags — `ignore_leading_articles` (a/an/the with
    whitespace separator), `case_insensitive` (ASCII case-fold),
    `digits_after_letters` (digit lift into 0x80..=0x89 slot).
    Presets `EnglishCollator::DICTIONARY` and `::ASCII`.
    `English.collator()` now returns `Some(&ENGLISH_DICTIONARY_COLLATOR)`
    (was `None`).
  * `ContractionTokenizer` handles English contractions ("don't" →
    ["do", "n't"], "won't" → ["will", "n't"], "shan't" →
    ["shall", "n't"], plus `'ll` / `'ve` / `'re` / `'d` / `'s` / `'m`
    suffixes). Two presets: `STANDARD` (preserves fragments),
    `NORMALIZED` (expands to full-word forms). Handles ASCII `'`
    and typographic `'`. Both owned `String` and borrowed `&'a str`
    tokenization surfaces.
  * New `ENGLISH_WITH_CONTRACTIONS` constant. Backwards-compat:
    `ENGLISH`, `PORTER_STEMMER`, `PORTER2_STEMMER`, `ENGLISH_PORTER2`
    unchanged.

- **`stringcheese-lang`: static language-pack registry via linkme.**
  New `registry` module with `language(code) -> Option<&'static dyn
  Language>` and `languages()` iteration. Language packs opt in via
  the new `register_language!` macro. Uses `linkme`
  (`#[distributed_slice]`) for compile-time collection with zero
  runtime constructor cost. BCP-47 codes matched case-insensitively;
  full BCP-47 fallback (`pt-BR → pt`) deferred. All four language
  packs (`stringcheese-en`, `-de`, `-fr`, `-ja`) self-register.
  `#![forbid(unsafe_code)]` relaxed to `#![deny]` in the affected
  crates (linkme emits an `unsafe fn __typecheck`); the sole
  registration site carries an explicit `#[allow(unsafe_code)]`.

- **`stringcheese-compare::levenshtein::simd`: wide-block Myers.**
  Replaces the delegation-to-scalar in the three SIMD arch backends
  with actual vector-intrinsic wide-block Myers:
  * SSE2 — 128-bit (2 × u64 lanes) — m ≤ 128
  * NEON — 128-bit (2 × u64 lanes) — m ≤ 128
  * AVX2 — 256-bit (4 × u64 lanes) — m ≤ 256

  Cross-lane carry via `_mm_slli_si128` / `vextq_u64` /
  `_mm256_permute4x64_epi64`; multi-lane integer add uses
  per-lane vector add plus scalar carry chain. Dispatcher picks
  widest available; falls back to scalar Myers for m ≤ 64 and to
  rolling-rows for m > register-width. Differential tests confirm
  bit-for-bit agreement with scalar reference across every
  arch × m combination. **Benchmarks**: 17-20× speedup on NEON at
  m=96-128; 6-10× on SSE2 at m=96-128. m > 256 still uses
  rolling-rows (block-form Hyyrö deferred).

- **`stringcheese-manip`: `pipeline` module ships — all 15 modules real.**
  `TextPipeline` stages `Operation` trait objects into an ordered
  transformation IR that applies each in one pass over a ping-pong
  buffer pair (two heap allocations regardless of stage count;
  `apply_into` writes the final stage direct into the caller's
  buffer with no post-hoc copy). Concrete operations wrap the shipping
  modules — `Trim`, `Normalize`, `CaseFold`, `CollapseWhitespace`,
  `Remove`, `Replace`, `Escape`, `Truncate`. Operations expose
  `name()` for introspection; budget-limited ops (`Truncate`)
  short-circuit. `Truncate` is byte-budget with scalar-aligned cut
  (never splits a UTF-8 scalar). Adds 59 unit + 8 property + 15
  doctests.

- **`stringcheese-de` — German language pack.** New workspace crate.
  ~245 stopwords (drawn from Snowball's German stoplist), Snowball
  German stemmer (full 6-step spec: R1/R2 regions, u/i-between-two-vowels
  isolation, standard and rare-suffix cascades, undouble, un-accent),
  Kölner Phonetik encoder (Postel 1969) with Wikipedia's H-in-next-letter
  interpretation for the C rule. Public `GERMAN` constant. Snowball
  cross-verified against 46 hand-traced reference pairs; Kölner Phonetik
  against 15 well-known German surnames. `Freiheit → freiheit` (stem
  unchanged because R2 is past `-heit`) is explicitly tested — matches
  the spec, not a bug. Compound-noun splitting deferred (needs a
  dictionary).

- **`stringcheese-fr` — French language pack.** New workspace crate.
  246 stopwords (both apostrophe-suffixed and stripped clitic forms),
  Snowball French stemmer (full 6-step spec: R1/R2/RV with the
  `par`/`col`/`tap` exception, 15 rule groups with cascading
  precede-by rules, verb suffix passes 2a/2b with ment-family
  dispatch, script/residual cleanup, undouble, un-accent), PHONEX
  phonetic encoder (Soundex-shaped 4-character key with French-tuned
  preprocessing — accent folding, `PH → F`, `GN → N`, `CH → X`,
  `QU → K`, `Y → I`, `W → V`, `Ç → S`), elision-aware tokenizer
  (splits `l'`, `d'`, `qu'`, `jusqu'`, `lorsqu'`, `puisqu'`,
  `quoiqu'` case-insensitively; preserves `aujourd'hui` as one token;
  handles ASCII `'` and typographic `\u{2019}`). Public `FRENCH`
  constant. Snowball cross-verified against 46 pairs; PHONEX against
  22 pairs. Snowball French intentionally not universally idempotent
  (per spec, `dangereux → danger → dang`); property tests verify
  convergence in ≤5 iterations. Full-corpus verification, Métaphone
  Français alternative, soft-C/soft-G detection deferred.

- **`stringcheese-en`: Porter2 (Snowball 2001) stemmer companion.**
  Adds the revised Porter2 stemmer as `PORTER2_STEMMER` alongside the
  existing `PORTER_STEMMER` (Porter 1980). Both remain available.
  Public `ENGLISH_PORTER2` constant; new `English::with_porter()` and
  `English::with_porter2()` const constructors. The `ENGLISH`
  constant continues to use Porter 1980 for backwards compatibility.
  Full 5+ step spec with exception table (`skies → ski`, `sky → sky`,
  `dying → die`, `lying → lie`, `tying → tie`, `vying → vye`,
  `idly`, `gently`, `ugly`, `early`, `only`, `singly`, `news`,
  `atlas`, `cosmos`, `bias`, `andes`, `inning`, `outing`, `canning`,
  `herring`, `earring`, `evening`, `proceed`, `exceed`, `succeed`).
  R1/R2 markers, special prefix handling (`gener-`, `commun-`,
  `arsen-`), Y-vowel prelude, short-syllable predicate, double
  predicate. Cross-verified against **498 Snowball reference pairs**
  from the canonical `voc.txt` / `output.txt`.

- **`stringcheese-compare`: SIMD dispatch for Jaro and OSA.** Extends
  the SIMD dispatch pattern established for Levenshtein to
  `stringcheese_compare::jaro::simd` and
  `stringcheese_compare::damerau::osa::simd`. Both provide
  `similarity_bytes_with_workspace` / `distance_bytes_with_workspace`
  entry points that check the `simd` feature + byte-amenability and
  dispatch to AVX2 / SSE2 / NEON / scalar fallback. Arch-specific
  backends currently delegate to the scalar SIMD-shape for
  correctness scaffolding; wide-block true vector intrinsics are
  documented follow-up work. Full unrestricted Damerau-Levenshtein
  stays scalar (its HashMap-backed algorithm doesn't fit the Myers
  pattern). Hoisted `is_byte_amenable` into a shared
  `simd_dispatch` module used by all three SIMD sub-trees.
  Differential and property tests confirm bit-for-bit agreement with
  the existing scalar kernels; a proptest-caught bug in the Jaro
  SIMD scan's slice bounds (start > len_b when i > len_b + window)
  was fixed and pinned as a regression.

- **Python bench adapter.** `bench-adapters/python/` — pytest-benchmark
  head-to-head comparing StringCheese (loaded via `wasmtime-py` as the
  component-model `.wasm`) against `python-Levenshtein`, `jellyfish`,
  `rapidfuzz`. Establishes the non-Rust adapter pattern for future
  language adapters. wasmtime-py 41.0.0's `wasmtime.component` surface
  handles the nested exports and `result<T, E>` returns cleanly.
  Verified end-to-end with an actual benchmark run — StringCheese-via-
  wasm shows the expected FFI overhead vs native C-extension libraries
  for short strings. `damerau_distance` skipped because the full
  Damerau kernel isn't yet exposed at the WIT boundary (needs a
  wasm-portable hash story).

- **`docs/design/tokenizers.md` — tokenizer subsystem design.** New
  ~5,800-word design doc covering the tokenizer subsystem:
  `Tokenizer` vs `Segmenter` trait taxonomy with GAT-based signatures,
  three-tier crate layout (`stringcheese-tokenizer` for abstractions +
  built-ins; `-bpe` / `-wordpiece` / `-sentencepiece` for algorithms;
  `-tiktoken` / `-huggingface` for pre-configured models), SCUD
  extension for BPE data packs, ~65-line WIT interface with
  offset-preserving `encoding` record and configurable
  `special-policy`, integration sketches for `compare` / `cdc` /
  `manip` / `lang` / `index`, seven-phase implementation plan.
  Nine open questions flagged (trait residency, concurrent-tokenizer
  negotiation, SCUD compression measurement, loader sharing with
  wit-i18n, default special-token policy, borrowed vs owned segmenter
  output, ...). Design only — no implementation.

### Fixed

- **`stringcheese-en`: contraction tokenizer idempotence on
  multi-apostrophe words.** `split_word` now short-circuits raw
  words with two or more apostrophes to atomic (unchanged) output.
  Earlier behavior on inputs like `"A'M'm"` yielded `["A'M", "'m"]`,
  which re-tokenized to `["A", "'m", "'m"]` after join — violating
  the property test's idempotence claim. Every real English
  contraction carries exactly one apostrophe, so recognized
  contractions (`don't`, `won't`, `I'll`, etc.) are unaffected.
  Surfaced by a new proptest seed; regression pinned in
  `crates/stringcheese-en/proptest-regressions/properties.txt`.

### Changed

- **`stringcheese-cdc`: real vectorized Gear SIMD kernel.** Replaces
  the wave-6 Gear scaffolding (scalar-under-`target_feature`) with a
  true block-reformulation kernel per the invariant `state_k = state_0 << k
  + Σ G[b_i] << (k-1-i)` (for k=64, `state_0 << 64 = 0` in u64, so each
  64-byte block hashes independently). AVX2 backend: 16 iterations × 4
  bytes via `_mm256_i32gather_epi64` from `GEAR_TABLE`,
  `_mm256_sllv_epi64` per-lane pre-shift `[3,2,1,0]`,
  `_mm256_slli_epi64::<4>` Horner advance, `_mm256_add_epi64` fold. NEON
  backend: 32 iterations × 2 bytes via scalar-side gather packed with
  `vsetq_lane_u64`, `vshlq_u64` pre-shift, `vshlq_n_u64::<2>` Horner,
  `vaddq_u64` fold. wasm SIMD128 backend: 32 iterations × 2 bytes via
  scalar-side `(g0<<1, g1)` pack, `u64x2_shl(_, 2)` Horner, `u64x2_add`
  fold. SSE2 stays scalar (no gather, no per-lane variable shift before
  SSE4.1). Non-64-byte-aligned tail falls back to the scalar recurrence.
  Measured on Apple M-series: **2.09× at 16 KiB, 1.48× at 1 MiB** vs
  the scalar baseline. Byte-identical output vs scalar reference over
  all boundary sizes (63/64/65/127/128/129 + larger blobs). Buzhash,
  Polynomial, Rabin real kernels remain scaffolding — deferred to
  wave 8.

- **`stringcheese-unicode`: wasm baseline shrunk from 213 KB to 190 KB
  (11 %).** New `case-fold` feature (default on) and
  `compiled-case-data` feature (default on) let callers opt out of
  icu_casemap entirely. New `case_fold_with_mapper` /
  `simple_case_fold_with_mapper` / `case_fold_turkic_with_mapper`
  entry points accept a caller-supplied `CaseMapper` when the tables
  aren't baked in; re-exports `icu_casemap::CaseMapper` for that.
  The 40-60 % target wasn't achievable through feature-gating alone —
  LTO was already stripping most of icu_casemap when the calling
  code doesn't reach `case_fold`; the residual ~145 KB is in
  unicode-normalization's NFC / NFD / NFKC / NFKD tables (needs an
  upstream `unicode-normalization` patch to feature-gate the compat
  tables). `.wasm-size-limits.toml` re-baselined at 189567 B;
  `docs/wasm-binary-size.md` updated with the new number + twiggy
  breakdown. No downstream crate Cargo.toml changes required
  (nothing in the workspace uses `case_fold`).

- **Project renamed from Comparand to StringCheese.** Every crate is renamed
  Second wave fills in `split`, `join`, `replace`, `normalize`, `slice`,
  `find`, `pad`, `lines`, `escape`, `quote`, and `template`. Only
  `pipeline` (the transformation IR) remains a scaffold. 14 of 15
  modules now ship. Every module names the boundary each function
  works at (bytes / USVs / graphemes / display width), delegates to
  `stringcheese-unicode` for Unicode-aware work and to
  `stringcheese-compare::search` for substring-search. See the crate's
  module map for the full surface.

- **`stringcheese-lang` — language pack infrastructure.** New crate
  defining the `Language` trait (`code` / `name` / `stopwords` /
  `is_stopword` / `stem` / `tokenize` / `phonetic_encoder` /
  `collator`), plus companion traits (`Stemmer`, `Collator`,
  `LanguageProvider`) and shared helper types (`Stopwords`,
  `SimpleTokenizer`, `LanguagePhoneticEncoder`,
  `Soundex`/`Nysiis`/`DoubleMetaphone` adapters). `no_std + alloc`
  compatible. Enables opt-in per-language packs.

- **`stringcheese-en` — English language pack.** First reference
  language pack. Ships the full 5-step Porter (1980) stemmer,
  ~150 stopwords, whitespace-and-punctuation tokenizer, Soundex as
  phonetic encoder. Public `ENGLISH` constant so callers write
  `stringcheese_en::ENGLISH.stem("caresses")` without construction
  ceremony. Porter cross-verified against 65 reference pairs from
  the original paper.

- **`stringcheese-compare`: Ristad-Yianilos (1998) learned string-edit
  distance.** Memoryless stochastic transducer over source/target
  alphabets, trained from labeled pairs via EM in log space (log-sum-exp
  throughout for numerical stability). `LearnedEditModel` +
  `LearnedEdit` (implements `DistanceMetric`) + `RistadYianilosEstimator`
  (builder-pattern EM with configurable iterations and convergence
  threshold). New `AlgorithmFamily::RistadYianilos` variant in
  `stringcheese-core`. Semimetric class — the model is symmetric only
  if the trained insert/delete/substitute costs are symmetric.
  `no_std + alloc` compatible for the query surface; training is
  std-gated (needs `f64::ln`/`exp`). 33 tests total (unit + property +
  golden).

- **`stringcheese-compare::levenshtein::simd` — SIMD-dispatched Myers
  kernel.** Opt-in `simd` feature swaps the Levenshtein kernel for
  Myers (1999) bit-parallel bit-vector formulation on hosts where it
  wins. Runtime dispatch picks AVX2 → SSE2 → NEON → scalar Myers →
  falls back to rolling-rows on non-viable input (too short, or
  unicode-heavy). Scalar Myers alone delivers ~1.9-12× speedup for
  m ≤ 64; wide-block true-SIMD kernels behind the same dispatch is
  documented follow-up work. `#![forbid(unsafe_code)]` softened to
  `#![deny]` in `stringcheese-compare` with an inline exception
  comment pointing at the SIMD sub-tree (every `unsafe fn` / block
  in the sub-tree carries a `SAFETY:` comment naming its CPU-feature
  precondition). 20 SIMD-specific tests (differential vs oracle +
  arch wrapper agreement).

- **`docs/design/wit-i18n.md` — SCUD + WIT i18n design doc.** ~5,300
  word design for the umbrella's ICU-alternative direction. Covers
  the six capability WIT interfaces (case / collation / plural /
  number / datetime / break) with a ~80-line illustrative WIT for
  `stringcheese-icu-case`; the SCUD compressed data-pack binary
  format (seven compression primitives — RangeDelta, AdaptivePages,
  PackedIntegers, SequencePool, StringPool, LoudsTrie,
  FiniteStateTable — plus outer Brotli/Zstd; loader API sketch);
  runtime discovery / fallback / composition / versioning; language
  pack integration; CLDR licensing threat model; six-phase
  implementation plan. Design only — no implementation touched.

- **Wasm binary-size CI gate.** New `wasm-size` GitHub Actions job
  runs on every PR: builds each crate's minimal-surface release
  wasm through a shared `wasm-size-probes` cdylib wrapper, runs
  `wasm-opt -Oz`, compares against per-crate thresholds in
  `.wasm-size-limits.toml` (default ±5 %, ±20 % for
  `stringcheese-core` whose 724 B baseline sits inside wasm-opt
  noise). `scripts/measure-wasm-size.sh` is the contributor-facing
  local reproducer. Baseline documented in
  `docs/wasm-binary-size.md` with per-crate size + twiggy top-N
  breakdowns. Not addressed by this gate: `stringcheese-unicode`
  weighs 213 KB (icu_casemap + unicode-normalization data);
  documented as instrumentation-only and a future size-shrink
  opportunity.

- **`stringcheese-compare` crate.** Consolidates the nine sibling
  comparison crates (`stringcheese-levenshtein`, `stringcheese-hamming`,
  `stringcheese-jaro`, `stringcheese-damerau`, `stringcheese-lcs`,
  `stringcheese-ngram`, `stringcheese-search`,
  `stringcheese-set-similarity`, and `stringcheese-minhash`) into one
  crate with a top-level module per family (`levenshtein`, `hamming`,
  `jaro`, `damerau`, `lcs`, `ngram`, `search`, `set_similarity`,
  `minhash`). Every load-bearing type each source crate re-exported
  at its own root is also re-exported at `stringcheese-compare`'s root,
  so `use stringcheese_compare::Levenshtein` and
  `use stringcheese_compare::levenshtein::Levenshtein` both resolve.
  Public API is preserved.

- **`stringcheese-manip` crate.** New sub-project — the manipulation
  half of the StringCheese charter. Scaffold only in v0.1: every
  module (`inspect`, `trim`, `case`, `split`, `join`, `replace`,
  `normalize`, `pad`, `slice`, `find`, `escape`, `quote`, `lines`,
  `template`, `pipeline`) is declared with module-level docs
  describing its scope, but no items ship yet. Depending on
  `stringcheese-manip` today is safe — items will only be added at
  this pre-1.0 stage, never removed. Also re-exported from the
  facade as `stringcheese::manip`.

- **`docs/DESIGN.md` Charter/Scope/Architecture rewrite.** The Vision,
  Scope, and Architecture sections now reflect the umbrella charter
  (string processing, not just comparison) — sub-project map, the
  planned language packs (`stringcheese-<language>`), and the planned
  WIT-based i18n direction (SCUD data packs) are documented. Record
  linkage stays a sibling library (planned rename to
  `stringcheese-linkage`); the substantive scope split is unchanged.

- **`stringcheese-cdc`: Buzhash rolling hash.** Uzgalis (1983)
  cyclic-polynomial rolling hash with a 256-entry byte-substitution
  table (generated at compile time from a fixed `SplitMix64` seed for
  cross-target reproducibility; variant slug `splitmix64-seed-buzz`
  pins the table). Implements the `RollingHash` trait alongside the
  existing Rabin, polynomial, and Gear implementations. New public
  types: `stringcheese_cdc::Buzhash` and
  `stringcheese_cdc::fingerprint::buzhash::{Buzhash, BUZ_TABLE}`.
  Windows larger than 64 bytes are supported by folding the eviction
  rotate through `window mod 64` (unlike Gear's 64-byte natural
  horizon). Adds 4 golden cases + 12 unit tests + 3 property tests
  (17 new tests).

- **`stringcheese-manip`: `inspect`, `trim`, and `case` modules.**
  First three scaffold-status modules become real implementations
  (11 of the 15 modules still ship as scaffold-only stubs — the
  charter is unchanged; those land in follow-on waves).
  * `inspect` — `is_empty`, `byte_len`, `scalar_count`,
    `grapheme_count`, `first_char` / `last_char`, `first_grapheme` /
    `last_grapheme`. Every function names its boundary in its doc
    comment (byte vs USV vs grapheme); all zero-allocation.
    Grapheme-inspect functions gated on `feature = "alloc"`
    (delegating to `stringcheese-unicode` which is `alloc`-gated).
  * `trim` — `trim` / `trim_start` / `trim_end` (whitespace),
    `trim_matches` (predicate), `trim_chars` (char-set) plus
    start-only / end-only variants; and the reusable `Trim`
    configured-operation type. Zero-allocation for all trim
    functions; `Trim` value type is `alloc`-gated (holds
    `Box<dyn Fn>`).
  * `case` — `to_lowercase` / `to_uppercase` / `to_title_case` /
    `capitalize`, both owned (`-> String`) and buffer-appending
    (`*_into(&str, &mut String)`) variants, plus ASCII fast paths.
    Delegates to `stringcheese-unicode` for Unicode-aware case
    folding. Title-case buffers each word's tail into a `String`
    and calls `str::to_lowercase()` on it so Greek final-sigma is
    honored. Word boundary is "grapheme whose first scalar is
    `char::is_alphabetic()`, preceded by a non-alphabetic
    grapheme"; full UAX #29 word segmentation deferred.
  * Adds 76 unit tests + 24 property tests + 29 doctests (133 new
    tests). Extension-trait API (`s.stringcheese_trim()` style)
    deferred to a follow-up wave.

- **`stringcheese-phonetic`: Double Metaphone rule sets completed.**
  All four remaining rule families from Philips (1999) now land in
  the encoder:
  * **Slavo-Germanic modifications** — the paper's standard heuristic
    (detected by presence of `W`, `K`, `CZ`, or `WITZ` in the
    normalized name) enables three conditional rules: initial `S`
    before `L`/`M`/`N`/`W` diverges the alternate to `X`
    (*Sniepis*, *Slavik*); `CZ` anywhere emits `S` in primary, `X`
    in alternate (*Czajka*); word-final `-WITZ` has the alternate
    emit `F` and consume the whole cluster (*Rabinowitz*,
    *Horowitz*).
  * **SC-before-IEY** — `SC` (not `SCH`) followed by `I`/`E`/`Y`
    collapses to `S` in both branches (*Scientific*, *Scenic*,
    *Ascension*); `SCH` followed by `ER`/`EN`/`OO`/`UY`/`ED`/`EM`
    emits `SK` (the German consonantal reading — *Schenker*,
    *Schooner*, *Schuyler*).
  * **French silent-terminal endings** — word-final `-GN` skips the
    `G` (*Reign*, *Coign*); word-final `-MB` emits `M` and skips
    the silent `B` (*Lamb*, *Thumb*, *Coulomb*); the `-MPT-`
    cluster silences the `P` (*Compton*, *Hampton*, *Empty*,
    *Symptom*), with `-MPS` (Thompson) intentionally preserved.
  * **Surname exceptions** — chemistry `CH` at word start followed
    by `IA` / `YS` / `EM` emits `K` (*Chianti*, *Chemistry*),
    overriding the default `X`. Scots/Irish `MC` and `MAC`
    patronymic prefixes force hard `K` on their `C` even when the
    following letter would otherwise soften it (*McIver*,
    *MacBride*).

  Primary-key stability is preserved: the module's contract that the
  primary-only variant's primary key equals the full variant's
  primary key byte-for-byte for every input is honored, verified
  case-by-case against all pre-existing goldens (including surnames
  that now count as Slavo-Germanic under the new heuristic). Adds
  24 inline unit tests + 40 primary-only goldens + 10 full-variant
  goldens + 5 property tests.

- **`stringcheese-compare::minhash`: three new sibling sketches.**
  * **SimHash** (Charikar 2002) — signed random projections for
    cosine LSH. `SimHashSketch` exposes `from_iter`, `signature()`,
    `hamming_distance()`, and `estimated_cosine_similarity()`
    (std-gated). Fixed 64-bit signature; empty accumulators
    tie-break to non-negative producing `u64::MAX` so two empty
    sketches have Hamming 0 and cosine 1.0. Multiset (signed-sum)
    semantics rather than the set-invariance of regular MinHash —
    callers who want set semantics dedupe upstream.
  * **One-permutation MinHash** (Li-Owen-Zhang 2012 with
    Shrivastava-Li 2014 rotation densification) — single-permutation
    approximation with cleaner densification for empty bins.
    Bucket assignment uses Lemire top-bits multiplication
    (`((h as u128 * k as u128) >> 64) as usize`) not `hash % k` —
    top-bits preserves item-hash order, which is the condition
    Li-Owen-Zhang's unbiasedness proof requires; the low-bits
    variant fails a golden case empirically. Densification stores
    `splitmix64(source_value XOR splitmix64(hop_distance))` rather
    than a raw copy so long runs of empty bins with a shared
    source don't collapse to a single value on both sketches
    (which would inflate the estimator).
  * **p-stable LSH** (Datar-Immorlica-Indyk-Mirrokni 2004) — LSH
    families for L_p distances. `PStableLshSketch` +
    `PStableFamily::{L1, L2}`. Adds `AlgorithmFamily::PStableLsh`
    to `stringcheese-core`. Std-gated (needs `sqrt`/`ln` for
    inversion sampling — Box-Muller for Gaussian; direct inverse
    for Cauchy). Exposes `bucket() -> i64` and
    `collide_with(other) -> bool`; callers compose multiple
    sketches for LSH amplification.

  Adds 8 golden cases + 13 property tests (57 new tests total
  spanning the three sketches).

### Changed

- **`proptest` gated off wasm; wasm-runtime CI matrix expanded to the
  full workspace.** Every crate that used `proptest` in
  `[dev-dependencies]` (`stringcheese-compare`, `-align`, `-cdc`,
  `-index`, `-manip`, `-phonetic`, `-unicode`) now declares it under
  `[target.'cfg(not(target_family = "wasm"))'.dev-dependencies]`, and
  every property-test module (`mod property_tests;` or inline
  `mod properties { ... }`) is gated on the matching
  `#[cfg(not(target_family = "wasm"))]` predicate. Reason: `proptest`
  transitively depends on `wait-timeout`, which is `#[cfg(unix)]` /
  `#[cfg(windows)]` only with no wasm branch; leaving it unconditional
  broke the wasm-runtime CI job's `cargo test --target wasm32-wasip1`
  at LINK time. Host `cargo test` runs are unchanged — proptest is
  still picked up for every non-wasm target and the property tests
  continue to run.

  With the gate in place, the wasm-runtime CI job now runs
  `cargo test --workspace --exclude stringcheese-bench
  --target wasm32-wasip1` — 10 crates, was 3. Locally, 942 tests
  pass under wasmtime on `wasm32-wasip1` (host `cargo test` remains
  1,233 including the property tests). `stringcheese-bench` stays
  excluded because criterion depends on host-only timing/IO.

- **Import paths.** `use stringcheese_<family>::X` becomes
  `use stringcheese_compare::<family>::X` (with `set-similarity`
  spelled `set_similarity` on the Rust side). The umbrella `stringcheese`
  facade re-exports the same nine module names as before, so
  `use stringcheese::levenshtein::Levenshtein` keeps working unchanged.

- **Project renamed from Comparand to StringCheese.** Every crate is renamed
  from `comparand-*` to `stringcheese-*`; the umbrella facade is `stringcheese`
  (was `comparand`); the WIT package is `stringcheese:core` (was `comparand:core`)
  and the interface file is `component/wit/stringcheese.wit`; the produced
  component binary is `stringcheese_component_host.wasm`. The `Comparand` name
  is retired — a comparison sub-project stays under the StringCheese umbrella
  as `stringcheese-compare` in a follow-up wave. Repository is
  `https://github.com/tegmentum/stringcheese`. Maintainer email is
  `zachary.whitley@tegmentum.ai`.
- The `DifferenceClassification::ComparandDefect` variant is now
  `DifferenceClassification::StringCheeseDefect`. Callers that named the
  variant by path in match arms must update.

### Deprecated

### Removed

### Fixed

### Security

## [0.1.0-alpha] — unreleased

Retroactive entry documenting everything on `main` up to the ship-rehearsal
wave. No published release has been cut yet. Section grouped by capability
rather than by commit; consult `git log` for the per-commit narrative.

### Added

#### Substrate

- Initial workspace and type-system substrate: result types, metric
  traits, mathematical-property descriptors, algorithm-variant registry,
  workspace and sequence traits, and the golden-case validation schema
  (`stringcheese-core`, `stringcheese-corpus`, `stringcheese`, and the
  placeholder crates `stringcheese-unicode`, `stringcheese-phonetic`,
  `stringcheese-search`, `stringcheese-cdc`, `stringcheese-index`,
  `stringcheese-bench`).
- `stringcheese-corpus` oracle framework, exhaustive generators, and
  differential vocabulary.

#### Algorithm crates

- **Edit distance and similarity.** `stringcheese-levenshtein` (full-matrix
  oracle, rolling-rows production kernel, Ukkonen-style banded cutoff),
  `stringcheese-hamming` (metric distance for equal-length sequences),
  `stringcheese-jaro` (Jaro and Jaro-Winkler similarity family),
  `stringcheese-damerau` (Optimal String Alignment and full
  Damerau-Levenshtein), and `stringcheese-lcs` (Longest Common Subsequence
  and LCS distance).
- **N-gram and set similarity.** `stringcheese-ngram` (character, byte,
  and token n-gram representations) and `stringcheese-set-similarity`
  (Dice, Jaccard, Overlap, Cosine over n-gram representations).
- **Alignment.** `stringcheese-align`: Needleman-Wunsch global alignment
  and Smith-Waterman local alignment with linear and affine gap
  penalties.
- **Phonetic.** `stringcheese-phonetic`: Soundex, NYSIIS, and the
  single-key Double Metaphone encoder, followed by the full two-key
  Double Metaphone variant.
- **Unicode preprocessing.** `stringcheese-unicode`: NFC/NFD/NFKC/NFKD
  normalization, Unicode case folding (via `icu_casemap`, including
  multi-character expansions), grapheme-cluster segmentation, and
  diacritic stripping.
- **Substring search.** `stringcheese-search`: Rabin-Karp, KMP, Boyer-Moore
  (bad-character), Aho-Corasick, followed by Horspool, Two-way, the
  full Boyer-Moore with good-suffix rule, and streaming wrappers over
  the single-pattern algorithms.
- **Content-defined chunking.** `stringcheese-cdc`: rolling-hash
  fingerprints (Rabin, polynomial, Gear) and FastCDC chunking.
- **Indexes.** `stringcheese-index`: BK-tree, VP-tree (with bulk build and
  sorted-neighborhood blocking added in a follow-up), and a q-gram
  inverted index for large-scale nearest-neighbor and blocking queries.
- **MinHash / LSH.** `stringcheese-minhash`: MinHash sketches and LSH
  banding for approximate Jaccard-similarity search at scale.

#### WebAssembly Component Model

- `component/`: WIT interface definition and a Rust host demonstrating
  StringCheese consumed as a Component-Model component, plus a reference
  guest and matching integration tests.

#### Benchmarks, fuzzing, and cross-comparison

- `stringcheese-bench`: criterion benchmark suite over the algorithm
  surface (Levenshtein, Hamming, Jaro, Damerau, n-gram, batch).
- Allocation-counting harness in `stringcheese-bench` backed by `dhat-rs`,
  gated behind the opt-in `alloc-tracking` feature so a default
  `cargo bench` never inherits the profiler's global allocator.
- `bench-adapters/`: Rust head-to-head adapters against `strsim` and
  `rapidfuzz` so criterion runs report StringCheese alongside the
  established crates.
- `fuzz/`: `cargo-fuzz` targets covering both differential comparisons
  against known-good implementations and metric-axiom checks
  (non-negativity, identity, symmetry, triangle inequality).
- Scheduled nightly fuzz workflow (`.github/workflows/fuzz-nightly.yml`)
  driving the `cargo-fuzz` corpus and reporting regressions.

#### Documentation

- `docs/DESIGN.md`: full project vision, algorithm coverage, validation
  strategy, CI requirements, and release gates.
- Five subordinate design documents under `docs/design/` covering the
  detailed design of the substrate and first-wave algorithms.
- `docs/wasm-build-recipes.md`: definitive per-crate matrix of the
  `wasm32-unknown-unknown` and `wasm32-wasip1` feature combinations
  StringCheese supports.
- `docs/references.md`: consolidated bibliography citing the primary
  papers behind every algorithm shipped in the workspace.
- Per-crate paper references added to the edit-distance / similarity,
  phonetic / unicode / align, and search / cdc / index / minhash
  families, cross-linked into `docs/references.md`.

### Changed

- Hardened the wasm CI matrix: `continue-on-error` removed from the
  `wasm` job, so `wasm32-unknown-unknown` and `wasm32-wasip1` failures
  now fail the workflow instead of being merely reported.

### Removed

- The record-linkage scope (Fellegi-Sunter probabilistic record
  linkage), briefly landed in-tree, has been extracted into a separate
  sibling library. StringCheese stays focused on sequence comparison; the
  record-linkage crate depends on StringCheese rather than the other way
  around. See the sibling repository for the extracted code and its
  own history.

### Fixed

- `stringcheese-damerau`: dropped intra-doc links that pointed at the
  test-only `property_tests` module, which produced rustdoc warnings on
  a non-test build.

[Unreleased]: https://github.com/tegmentum/stringcheese/compare/HEAD...HEAD
[0.1.0-alpha]: https://github.com/tegmentum/stringcheese/commits/main
