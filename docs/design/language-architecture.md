# Language architecture

## Purpose

Captures every architectural decision made across the language-pack and
language-detection subsystems. Read alongside
[scope-and-decomposition](./scope-and-decomposition.md), which frames
the broader library boundary.

The material here spans:

1. Per-language-pack restructuring — data-driven, generator-based,
   small per-language footprints preserved.
2. `LanguageDetector` trait — pluggable, explicit, never silent.
3. WebAssembly / WIT components — Rust rlib, WIT component, and
   browser ES-module targets from one source of truth.
4. Detection tier architecture — script-detect (Tier 0), whatlang per
   script (Tier 1), lingua per language (Tier 2).
5. Unified detection WIT contract — one interface across every
   backend, versioned as `tegmentum:lang-detect@0.2.0`.
6. Implementation-status snapshot — what's landed, what's staged.

---

## 1. Per-language pack restructuring

### Original problem

Waves 5–14 of the language-pack expansion filled the workspace with
30+ per-language crates (`stringcheese-en`, `stringcheese-de`,
`stringcheese-hi`, …). Each was hand-authored and looked
substantially like every other: a stopword list as a Rust const, a
suffix-stripping stemmer, a phonex table, ~2 000 lines of module
plumbing, and a `register_language!` invocation. LLM-assisted
implementation of each pack was **token-inefficient**: lots of
nearly-repetitive Unicode tables, locale exceptions, tests, and
documentation with relatively little new reasoning per token.

The observation was that the language layer should become
**data-driven and generated wherever possible**, following the shape:

```
language definition/data → common generator → Rust module + tables + tests
```

The LLM should design the abstraction and maybe implement 2–3
representative languages, then stop writing language implementations.
Subsequent languages should be **data-ingestion tasks** — a
declarative TOML file plus a handful of exceptional rules — rather
than implementation tasks.

### Tiered depth, capabilities-driven

Not every language needs equal depth immediately. The API surfaces
capabilities so callers reason about what each pack provides:

```rust
enum Capability {
    CaseFold,        // via icu::casemap
    TokenBoundaries, // via icu::segmenter
    Collation,       // via icu::collator
    Normalization,   // via icu::normalizer
    LineBreak,       // via icu::segmenter::LineSegmenter
    Properties,      // via icu::properties

    Stopwords,
    Stemming(StemmerKind),
    Phonex(PhonexKind),
    Contractions,
    Transliteration(String),
    Reduplication,
}
```

English is initially rich (contractions, Porter/Porter2, phonex,
collation). Major packs (German, French, Spanish) are moderately
rich. Every other language can be **claimed as supported** the moment
its `rules/xx.toml` names an ICU locale — the ICU-backed capabilities
work immediately; language-specific data (stopwords, stemmer rules,
phonex tables) fills in as the data is authored.

That fits the ICU-alternative story: **Unicode and CLDR should carry
as much of the language burden as possible; StringCheese encodes
only the semantics they don't provide** — otherwise the LLM is being
asked to recreate CLDR one language at a time.

### The dependency-footprint constraint

Consolidating 30+ per-language crates into one bundle-of-everything
crate is the **wrong shape**: end users pulling `stringcheese-en`
would download every language's data plus the union of every engine.
The per-language crate contract exists precisely so a caller who
wants only English writes `stringcheese-en = "0.1"` and pulls a small
tree.

### The build-time generator answer

The **shared piece is a build-time generator, not a runtime library**:

```
stringcheese-lang         # (unchanged) trait + registry + helpers
stringcheese-lang-gen     # NEW: build-dependency ONLY. TOML → Rust codegen.
                          # Never shipped in any consumer's binary.

stringcheese-lang-icu     # OPTIONAL runtime crate wrapping ICU4X.
                          # Per-language crates opt in if they need it
                          # (Turkish dotted-i, German ß, Greek final sigma).

stringcheese-en           # rules/en.toml + build.rs + slim src/.
                          # Runtime deps: stringcheese-lang.
                          # Build deps:   stringcheese-lang-gen.
                          # Does NOT depend on stringcheese-lang-icu — English
                          # case-fold is fine with eq_ignore_ascii_case.

stringcheese-tr           # Same shape as stringcheese-en, but ALSO depends
                          # on stringcheese-lang-icu because Turkish genuinely
                          # needs locale-aware case-fold. User opts in when
                          # pulling the Turkish pack, not when pulling English.
```

A per-language crate's `build.rs` becomes ~5 lines:

```rust
fn main() {
    stringcheese_lang_gen::generate("rules/en.toml", "generated.rs");
}
```

The emitted `generated.rs` is self-contained Rust that only references
`stringcheese-lang` types. The stemmer engines (Porter, Snowball, …)
stay owned by their per-language crate — the generator produces the
*wiring*, not the algorithm.

**What the user pays for `stringcheese-en = "0.1"`:**

- `stringcheese-en` (small)
- `stringcheese-lang` (small — trait + registry)
- `stringcheese-phonetic` (small — Soundex)
- **No ICU4X** unless the pack actually needs it.
- `stringcheese-lang-gen` runs at build time and drops out.

**What changes for the LLM/human authoring a new language:**

- Drop a `rules/xx.toml` file into `crates/stringcheese-xx/rules/`.
- Write a 5-line `build.rs`.
- Write a ~30-line `src/lib.rs` that wires the generated capabilities
  into a pack struct + `register_language!`.
- No hand-writing of Language trait plumbing, no per-crate
  stopword-list-as-Rust-const, no boilerplate.

The token-efficiency win survives; the small-dependency contract
survives.

### TOML schema shape

```toml
[locale]
bcp47 = "en"
icu   = "en"                # optional ICU4X locale (defaults to bcp47)
name  = "English"
script = "Latn"

[stopwords]
list = ["a", "about", "above", …]

[stemmer]
kind = "porter"                # "identity" | "porter" | "porter2" | "snowball-de" | …

[phonex]                       # optional
kind = "soundex-en"

[contractions]                 # optional
"don't" = "do not"
"won't" = "will not"

[transliteration]              # optional
scheme = "iso-15919"
```

The `stemmer.kind` string names an engine the generated `Language`
trait impl dispatches through. Engines that require significant
runtime code (Porter, Snowball) stay in the per-language crate that
owns them; identity / light-suffix / table-driven engines can live
in a shared engines crate the generator points at.

### Explicit non-scope

- **No mega-crate bundling every language.** Every attempt at "one
  crate with all languages behind features" fights the browser story
  (see section 3) and provides no advantage to server callers who can
  already list the exact packs they want in `Cargo.toml`.
- **No fork of the per-language crates that duplicates data.** A
  runtime umbrella crate that copies each pack's stopwords /
  stemmer / phonex data violates single-source-of-truth. Data lives
  in the per-language crate; consumers pull that crate, no umbrella.

---

## 2. LanguageDetector trait

### Design principle: explicit, never silent

Language detection MUST NOT run inside routine string operations.
Short strings (names, addresses, SKUs, ER fields, single words like
`"resume"` or `"Roma"`) are systematically poor detection inputs, and
silently dispatching normalization / tokenization / stemming through
a detected language would introduce hidden compute and
nondeterminism for the pathological cases every real corpus contains.

### Three-layer API model

1. **Explicit language** — `english.stem(text)`,
   `registry::language("de").normalize(text)`. Deterministic,
   fastest, preferred whenever the language is known.
2. **Detected language** — the caller runs
   `LanguageDetector::detect` itself, inspects the
   `LanguagePrediction`, and dispatches to the appropriate pack.
3. **Auto convenience** — higher-level helpers explicitly named
   `..._auto` (or configured with `Language::Detect`) that carry
   the detection call in their contract. Never a hidden fall-through
   in an operation the caller didn't ask to detect.

### Trait surface

Landed in `crates/stringcheese-lang/src/detect.rs`:

```rust
pub trait LanguageDetector: Send + Sync {
    fn detect(&self, text: &str) -> Option<LanguagePrediction>;
}

pub struct LanguagePrediction {
    pub bcp47:      String,   // primary language subtag — feed directly to registry::language
    pub name:       String,
    pub script:     String,   // ISO 15924
    pub confidence: f64,      // [0.0, 1.0]
    pub reliable:   bool,     // backend's own opinion
}
```

Object-safe, backend-neutral. StringCheese ships no built-in
detector. Downstream adapter crates plug in whatlang, CLD3, fastText,
or a WebAssembly component that speaks the same shape.

### The trait as WASM dispatcher

The trait maps 1:1 onto a WIT interface (`tegmentum:lang-detect`,
covered in section 5). Downstream WASM-component authors expose
detection through the WIT interface with a `detect(text) ->
prediction` export whose `prediction` carries the same fields as
`LanguagePrediction`. Keeping the trait synchronous and non-generic
keeps the WIT surface trivial.

Language detection becomes the dispatcher that selects the
appropriate lightweight language component rather than
StringCheese having to ship every language implementation:

```
text → language detector → language profile/component → normalize/tokenize/phonetic/etc.
```

---

## 3. WebAssembly / WIT components

### The three-target build

Each per-language crate is a **dual-target** Rust crate producing
`rlib` + `cdylib`, and a **third target** produced by the tegmentum
JS toolchain:

- **`rlib`** for Rust callers linking directly. `cargo add
  stringcheese-en` gets the fast static-linking path.
- **`.wasm` component** for WIT hosts (Rust hosts using
  `wasmtime-wasi`, other WASM components, Python, etc.) via
  `wasm-tools component new` or `cargo-component`.
- **JS / TS bindings via
  [`wit-js-bindgen`](https://github.com/tegmentum/wit-js-bindgen)** —
  the tegmentum-owned WIT → JS/TS bindings generator, aligned with
  the `wasm-cm` canonical ABI. Emits `.mjs` + `.d.ts` for the
  language-pack WIT world. Browser and Node/Deno callers `import`
  the generated `.mjs`. Not `jco` — the tegmentum toolchain is the
  intended path for every JS-side StringCheese consumer so bindings,
  runtime, and deploy shape all live under one supported stack.

All three from one source (`rules/en.toml` + generated code) and one
shared WIT interface (`wit/language.wit`).

### The runtime — wasmos

Browser execution of the `.wasm` components goes through
[`wasmos`](https://github.com/tegmentum/wasmos) — a portable,
backend-neutral runtime layer whose `wasmos:runtime` WIT contract has
per-backend adapters for wasmtime, WAMR, and V8. The **`adapter-js`**
adapter — `@tegmentum/wasmos-host-js` — is what actually runs a
StringCheese language pack (or detection component) in a browser,
Deno, or a Cloudflare Worker.

The browser deployment shape (also owned by tegmentum):

- **`@tegmentum/wasmos-browser`** — the current wit-js-bindgen
  browser scaffold. Runs the wasm-cm runtime-guest under any
  Web-standard host.
- **Cloudflare Workers lane** — via `wasmcm-decompose` because
  workerd disallows dynamic `WebAssembly.Module(bytes)`. Consumers
  get pre-decomposed components rather than the browser scaffold
  directly.

For StringCheese this means:

- We ship `.wasm` components + `wit/*.wit` sources.
- Downstream JS/TS applications run `wit-js-bindgen` against those
  WIT files to generate their own bindings (or consume prebuilt
  npm packages that already ran the generator).
- The runtime environment is any `wasmos` adapter — the same
  StringCheese component works under wasmtime (server), V8
  (browser/Deno), or workerd (Cloudflare Workers via
  `wasmcm-decompose`) without a rebuild.

Nothing in the StringCheese crates themselves depends on
`wit-js-bindgen` or `wasmos` at compile time — the coupling is
entirely at the deployment/toolchain layer. StringCheese only has
to be **standards-compliant WIT** for these tools to accept it.

### The browser is the constraint that forces the design honest

- **Bundle size is user-visible.** Every KB is a page-load hit.
  "Ship all 40 languages just in case" is off the table — a
  monolithic multi-MB blob doesn't fit a page budget.
- **Dynamic import is the native loading mechanism.** `await
  import("./stringcheese-lang-de.js")` is one line. Language packs
  shouldn't sit above that abstraction — they should map onto it.
- **CDN caching is per-file.** A per-language file means every site
  using German shares one cached download. A monolithic bundle means
  every site pays independently.
- **No filesystem, no dynamic linker, no `dlopen`.** WIT components
  bound via `wit-js-bindgen` and run under `wasmos`'s `adapter-js`
  are the loose-coupling story that survives to the browser
  (Cloudflare Workers included, via `wasmcm-decompose`).

### The lazy-loading pattern falls out

```js
import { detect } from "@tegmentum/lang-detect";

const langCode = detect(userText).bcp47;                       // "de"
const pack     = await import(`@tegmentum/stringcheese-lang-${langCode}`);
const stems    = userText.split(/\s+/).map(w => pack.stem(w));
```

No registry init, no eager bundling, no ceremony. A user typing
English pays for `stringcheese-lang-en`; the same user typing German
pays for `stringcheese-lang-de` **on demand**. If the app never sees
Korean, Korean is never downloaded.

### What survives, what dies

**Survives:**
- Per-language crates (each becomes a WASM component + JS module).
- Build-time generator (TOML → Rust source + WIT Guest impl).
- WIT schema at workspace root, shared across all packs.
- Rust `Language` trait — still valid for the rlib consumption path.
- Language detector as a separate component.

**Dies:**
- Any "one big crate with feature-gated languages" umbrella — a
  server-side compromise that doesn't survive the browser use case.
- Any argument for statically linking multiple languages into one
  artifact.
- The `linkme` distributed-slice registry story for browser targets
  (it's a link-time mechanism; there is no link step for a component
  fetched at runtime). The registry becomes an **application-level**
  map: host code holds `Map<bcp47, PackModule>` populated by
  dynamic import. `linkme` still works for Rust callers who
  statically link their pack set.

### Target workspace shape

```
stringcheese/
├── wit/
│   ├── language.wit               # canonical language-pack interface
│   └── lang-detect.wit            # canonical detection interface
├── crates/
│   ├── stringcheese-lang          # trait + tiny helpers (unchanged)
│   ├── stringcheese-lang-gen      # TOML → Rust + WIT Guest emitter (build-only)
│   ├── stringcheese-lang-icu      # optional ICU4X-backed runtime helpers
│   ├── stringcheese-en            # rules/en.toml + build.rs + rlib + cdylib
│   ├── stringcheese-de            # same shape
│   ├── stringcheese-detect-script      # tier 0 (Unicode-block classifier)
│   ├── stringcheese-detect-whatlang    # tier 1 (per-script whatlang)
│   ├── stringcheese-detect-lingua      # tier 2 (per-language lingua)
│   └── ...
└── npm/                            # wit-js-bindgen outputs, one package per component
    ├── stringcheese-lang-en/        # emitted `.mjs` + `.d.ts` for the WIT world;
    ├── stringcheese-lang-de/        # runs under any wasmos adapter (browser,
    └── ...                          # Deno, workerd via wasmcm-decompose).
```

---

## 4. Detection tier architecture

The browser constraint applies to detection just as much as it
applies to packs: a 300 KB "always fetched" detector eats the
savings from lazy-loading packs. Detection also has to be lazy.

### The tiers

**Tier 0 — always resident, ~5 KB:**
- `stringcheese-detect-script` — Unicode-block classifier.
- Returns ISO 15924 script code.
- No models, no trigrams, no dependencies. Pure byte scan.
- Its job: make Tier 1 lazy by naming the right per-script
  component to fetch.

**Tier 1 — cheap, per-script; lazy-loaded once per script encountered:**
- `stringcheese-detect-whatlang-latn.wasm` (~60 KB)
- `stringcheese-detect-whatlang-cyrl.wasm` (~20 KB)
- `stringcheese-detect-whatlang-arab.wasm` (~10 KB)
- `stringcheese-detect-whatlang-hebr.wasm`, `-grek.wasm`,
  `-deva.wasm`, `-hans.wasm`, `-hant.wasm`, `-hang.wasm`,
  `-jpan.wasm`.
- Each contains only that script's trigram tables (hard-shard
  target; current cut is a soft-shard using whatlang's runtime
  allowlist).
- Wrapped whatlang (MIT-licensed); attribution retained.

**Tier 2 — heavy, per-language; lazy-loaded only when caller demands
high confidence:**
- `stringcheese-detect-lingua-en.wasm` (~1–2 MB gzipped)
- `stringcheese-detect-lingua-de.wasm` (~1–2 MB)
- Each wraps the corresponding `lingua-<language>-language-model`
  crate — lingua ships per-language natively, so no fork needed;
  we re-expose lingua's per-language granularity as WASM components
  under our WIT.

### Host flow

```
text
  ↓
script-detect (Tier 0, ~5 KB, always resident)
  ↓
"Latn"
  ↓
lazy-load stringcheese-detect-whatlang-latn.wasm (Tier 1, ~60 KB, cached after first fetch)
  ↓
{ lang: "de", confidence: 0.72 }
  ↓
if confidence >= caller_threshold: use as-is → done
else (opt-in escalation):
  lazy-load stringcheese-detect-lingua-de.wasm (Tier 2, ~1.5 MB)
  → { lang: "de", confidence: 0.98 }
```

A browser page that only handles English pays: script-detect (5 KB)
+ whatlang-latn (60 KB) + stringcheese-lang-en (~50 KB) = ~115 KB
*ever*, cached CDN-wide.

Escalation to Tier 2 is **caller opt-in**, never automatic. The
detector library exposes a threshold parameter or an explicit
`.escalate()` call; a downstream that only wants cheap detection
never fetches lingua.

### Why lingua-rs works cleanly for Tier 2

Lingua is architected exactly the way we want:

| | whatlang | lingua-rs |
|---|---|---|
| Architecture | Monolithic — one crate | Per-language crates |
| Activation | Always all-in | Cargo feature per language |
| Model data | Rust `const` trigrams (568 KB source total) | FST binaries `include_dir!`-embedded |
| Per-language weight | ~3–5 KB per language | 4.4–5.7 MB per language |
| Accuracy | Lower | Higher |

`lingua-1.8.0`'s `Cargo.toml` has 150+ optional per-language-model
dependencies gated behind features. Enabling `english` pulls exactly
one 4.5 MB crate. That maps 1:1 onto WASM components — one lingua
crate = one WASM component.

### The lingua size trade-off is real

Lingua models are large. A per-language lingua component would be
roughly 1–2 MB gzipped in WASM form (FST binaries compress well but
they're still large). Compared to whatlang's ~3–5 KB per language,
lingua costs 100–500× more per language for meaningfully better
accuracy. That's why lingua is Tier 2 and opt-in: users pay the
size cost only when confidence matters more than bytes.

---

## 5. Unified detection WIT contract

Both tiers speak the same WIT interface so hosts can pipe them
interchangeably. A caller escalating from whatlang to lingua doesn't
relearn an API.

### The versioned schema

`wit/lang-detect.wit`, package `tegmentum:lang-detect@0.2.0`.

Backwards-compatible extension of the earlier 0.1 interface
(`detect(text) -> option<detection>` and the `detection` record
survive verbatim). 0.2 adds:

- `capabilities: func() -> capabilities` — backend advertises
  identity, version, supported ISO 639-3 codes, and which extended
  shapes it implements.
- `detect-ranked: func(text) -> list<ranked-detection>` — ranked
  distribution across enabled languages. Backends without ranking
  return a one-element list.
- `detect-from: func(text, langs) -> option<detection>` —
  constrained detection. Backends without constraint ignore the
  filter; callers key off `capabilities.can-constrain`.
- `detect-mixed: func(text) -> option<list<span-detection>>` —
  mixed-language segmentation. Backends without segmentation return
  `none`.

### Backend-capability shape

```wit
record capabilities {
    backend:       string,           // "whatlang" | "lingua" | "script" | ...
    version:       string,
    supports:      list<string>,     // ISO 639-3 codes; empty = "any"
    can-rank:      bool,
    can-constrain: bool,
    can-segment:   bool,
}
```

Every backend implements at least `capabilities()` and `detect()`.
The extended shapes are optional; the capability flags tell callers
what to trust.

### Where the WIT lives

- `wit/lang-detect.wit` (workspace root) — canonical schema. Every
  detection component (`stringcheese-detect-script`,
  `stringcheese-detect-whatlang-*`, `stringcheese-detect-lingua-*`,
  any third-party backend) points at this file via
  `[package.metadata.component.target.path = "../../wit"]`.
- `wit/language.wit` (workspace root, follow-up) — canonical
  language-pack interface. Same discipline.

### The tier stack in one contract

```
lang-detect (WIT world, one schema, one JS/TS binding, one host adapter)
    ↓ implemented by ↓

stringcheese-detect-script.wasm       [Tier 0]
  capabilities: { backend: "script", supports: [], can-rank: false, ... }
  detect: script-only; lang is "" and script is set

stringcheese-detect-whatlang-{scr}.wasm [Tier 1]
  capabilities: { backend: "whatlang", can-rank: true, can-constrain: true, can-segment: false }
  detect + detect-ranked: real; detect-from: real; detect-mixed: none

stringcheese-detect-lingua-{lang}.wasm  [Tier 2]
  capabilities: { backend: "lingua", supports: ["en"], can-rank: true, can-constrain: true, can-segment: true }
  all four: real
```

One WIT, three backends, uniform dispatch.

---

## 6. Implementation status snapshot

### Landed

- **`stringcheese-lang/src/detect.rs`** — `LanguageDetector` trait +
  `LanguagePrediction`. Backend-neutral. No adapter shipped;
  adapters are opt-in follow-ups.

- **`docs/design/scope-and-decomposition.md`** — companion doc
  covering the broader library boundary, in / out of scope, unified
  comparison model, regex/pattern subsystem.

- **`crates/stringcheese-lang-data/`** — initial data-driven
  foundation with English + German rules. **This crate's shape is
  wrong** for the intended architecture (it's a runtime library with
  ICU deps that would drag ICU into every consumer). Kept as an
  exploratory artifact; the correct architecture (build-time
  generator + optional ICU crate) is queued as a follow-up refactor
  that will repurpose or replace this crate. Documented here so the
  next author doesn't take it as canonical.

- **`wit/lang-detect.wit`** — canonical detection WIT at workspace
  root, `tegmentum:lang-detect@0.2.0`.

- **`crates/stringcheese-detect-script/`** — Tier 0 Unicode-block
  classifier. 20 tests pass. Dual-target (rlib + cdylib) build config
  wired.

- **`crates/stringcheese-detect-whatlang/`** — Tier 1 wrapper.
  Feature per script. **Currently soft-sharded** — the allowlist
  filters at runtime, but whatlang's full trigram data is still
  compiled into every per-script cdylib. 7 tests pass.

### Queued

- **Hard-shard whatlang** — vendor whatlang's source into the
  workspace, split `profiles.rs` (15 278 lines) by script, feature-
  gate the alphabet-detection tables. This actually shrinks the
  per-script binary. `stringcheese-detect-whatlang` today has the
  right API but wrong binary size; hard shard closes the loop.

- **Tier 2 lingua components** — `crates/stringcheese-detect-lingua/`
  with per-language cdylibs wrapping the existing `lingua-<lang>-
  language-model` crates. Same WIT interface as Tier 1. Callers opt
  in for high-confidence detection.

- **`stringcheese-lang-gen`** — build-time generator. TOML rules →
  Rust source + WIT Guest impl. Replaces the runtime-crate shape of
  the initial `stringcheese-lang-data`.

- **`stringcheese-lang-icu`** — optional runtime ICU4X wrapper.
  Per-language crates opt in when they need locale-aware
  operations (Turkish, German ß, Greek final sigma, …).

- **Per-language crate migration** — each existing per-language
  crate flips to using `rules/<lang>.toml` + `build.rs` + slim
  `src/lib.rs`. Small mechanical work per crate once the generator
  is proven on 2–3 representatives.

- **`cargo-component` pipeline** — currently the crates carry
  cdylib targets and WIT metadata but the actual `.wasm` component
  artifacts haven't been built. Wiring `cargo component build` into
  CI (or a Makefile) produces the components hosts consume.

- **`wit-js-bindgen`-emitted npm packages** — one per WASM
  component, published as `@tegmentum/stringcheese-lang-<code>` and
  `@tegmentum/lang-detect-<backend>-<scope>`. Each package ships the
  emitted `.mjs` + `.d.ts` (no `jco` involvement) and expects a
  `wasmos` runtime adapter — `@tegmentum/wasmos-host-js` for
  browser / Deno / workerd.

- **Host-side dispatcher** — small JS + Rust helper that walks the
  tiers based on caller-supplied confidence threshold. Handles
  script-detect → whatlang-{script} → lingua-{lang} escalation and
  the "load pack for detected language" step.

### Explicit non-goals for the architecture

Restated from the surrounding design docs so nothing gets lost:

- No silent language detection inside ordinary string operations.
- No entity resolution, NER, embeddings, semantic similarity, spell-
  check based on large dictionaries, morphology, MT, document
  classification.
- No PCRE compatibility for the regex subsystem — regular languages
  only.
- No mega-crate bundling every language.
- No forced ICU4X dependency for callers who don't need it.
