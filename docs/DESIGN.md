# StringCheese — Design Document

Status: Design Proposal
Target Version: 0.1 (Foundation)

This document is the north-star vision for StringCheese. It records the
philosophy, scope, algorithm coverage, memory model, and validation strategy
the project is being built against. Where the code has not yet caught up, this
document reflects intent rather than current state.

Subordinate design documents — comparison type system, preprocessing pipeline,
phonetic subsystem, n-gram and fingerprinting subsystem, WebAssembly/WIT
interface — will be added under `docs/` as their scope is fleshed out.

---

## Vision

StringCheese is a comprehensive, high-performance Rust and WebAssembly
toolkit for **string processing** — the full arc from inspecting and
shaping text (comparing, transforming, segmenting, encoding) through to
the language-specific and locale-aware operations that most string work
sooner or later needs.

The umbrella pursues three commitments existing string libraries treat
as tradeoffs, not as coherent whole:

1. **Explicit Unicode semantics at every boundary.** Every operation
   names the level it works at — bytes, Unicode Scalar Values, extended
   grapheme clusters, or display width. Nothing silently picks a
   segmentation.
2. **Allocation-conscious layered APIs.** Where the operation permits,
   borrowed / iterator / into-buffer / owned variants are all exposed;
   the pleasant default doesn't preclude the tight-loop form.
3. **Pluggable, opt-in globalization.** ICU-alternative i18n via the
   WebAssembly Component Model, with locale/capability data loaded
   from compressed data packs rather than the full monolithic ICU
   binary. Callers pay for the languages and features they use.

The library emphasizes:

- Mathematical correctness (for comparison)
- Explicit semantics (for every text-touching API)
- Performance (runtime, allocation count, peak memory, binary size)
- Predictable allocation behavior
- WebAssembly support (Wasm-first, no assumption of host runtime)
- Composability (pipelines, extension traits, configured operations)
- Explainability (algorithm descriptors, inspectable pipelines)
- Multilingual support (language packs; locale-aware Unicode)

Unlike existing libraries, the goal is not simply to expose implementations
of known algorithms. The goal is to provide a coherent, semantically
rigorous string-processing framework where the meaning, properties,
costs, and limitations of every operation are explicit.

## Scope

StringCheese is an **umbrella** — a set of coordinated crates that
together form one coherent string-processing toolkit. Each sub-project
owns a slice of the mission; the umbrella keeps them coherent.

### In scope

**Comparison** — `stringcheese-compare`, `stringcheese-align`.
Given two sequences, produce a distance, similarity, alignment, or
match result whose semantics are precise, whose cost is inspectable,
and whose correctness is testable. Every metric declares its
mathematical properties (metric axioms, bounds, normalization
policy).

**Manipulation** — `stringcheese-manip`. Inspect, trim, case, split,
join, replace, normalize, pad, slice, find, escape, quote, line
handling, and templating. Four API levels (free functions, extension
trait, configured operations, `TextPipeline` IR) so both the
pleasant one-liner and the allocation-controlled hot loop are
first-class. See the [`stringcheese-manip` module docs](../crates/stringcheese-manip/src/lib.rs)
for the module map.

**Preprocessing** — `stringcheese-unicode`. Normalization
(NFC/NFD/NFKC/NFKD), case folding, grapheme-cluster segmentation,
diacritic stripping — the Unicode-aware primitives that comparison,
manipulation, and language layers all consume.

**Phonetic** — `stringcheese-phonetic`. Sound-alike keys (Soundex,
NYSIIS, Double Metaphone) with a `PhoneticEncoder` trait so
language-specific encoders can plug in via the pack crates.

**Fingerprinting & Chunking** — `stringcheese-cdc`. Rolling-hash
fingerprints (Rabin, polynomial, Gear) and FastCDC content-defined
chunking, exposed as a streaming state machine.

**Indexing** — `stringcheese-index`. BK-tree, VP-tree, and q-gram
inverted index for metric-space and set-similarity nearest-neighbor
queries. Metric-space structures enforce metric properties at
construction.

**Language-pack infrastructure** — `stringcheese-lang`. The
`Language` trait, `LanguageProvider` discovery trait, `Stemmer` /
`Collator` / `LanguagePhoneticEncoder` plugin points, shared helper
types (`Stopwords`, `SimpleTokenizer`), and a static `registry`
(linkme `distributed_slice`) each `stringcheese-<lang>` pack opts
into via `register_language!`. Data-only — no per-language
implementations live here. Callers picking a language at runtime
(user locale, config file, `Accept-Language` header) reach for
`registry::language(code)`; callers who name the pack at compile
time keep using the pack's `ENGLISH` / `GERMAN` / `FRENCH`
constant. Full BCP-47 fallback (`"pt-BR" → "pt"`) is a v0.2 follow-up.

**Language packs** — `stringcheese-<language>` (e.g.,
`stringcheese-en`, planned: `stringcheese-de`, `stringcheese-ja`, …).
Data-driven implementations of stemming, stopword lists,
language-specific phonetic encoders, tokenization rules, collation
tailoring, and morphological analysis — one opt-in crate per
supported language. The `stringcheese-en` pack ships in v0.1 with a
~150-word stopword list, the Porter (1980) stemmer, the default
whitespace-and-punctuation tokenizer, and a Soundex phonetic hookup.
Additional language packs land as the algorithm-family coverage
matures.

**Component-model globalization** (planned) — `stringcheese-icu-*`
WIT interfaces and data packs. Callers instantiate just the
interfaces they need (case mapping, collation, plural rules, date
formatting) and load only the locales they support. A proposed
compressed data-pack format (SCUD — StringCheese Unicode Data)
packages CLDR-derived tables at a fraction of ICU's binary size by
composing range deltas, adaptive paging, packed integers, and outer
Brotli/Zstd compression.

**Substrate** — `stringcheese-core`, `stringcheese-corpus`. Traits,
result newtypes, algorithm-variant descriptors, workspace/sequence
abstractions, and the golden-case validation schema every sub-project
uses.

### Not in scope for the umbrella

**Record linkage** — combining per-field comparisons into whole-record
match/non-match decisions, blocking strategies, learned or
probabilistic classifiers that consume per-field scores. StringCheese
supplies the per-field scores and the metric-space blocking indexes;
deciding whether two records refer to the same real-world entity is
a downstream concern.
See [record-linkage](https://github.com/tegmentum/record-linkage)
for the sibling library that implements the Fellegi-Sunter classifier
and sorted-neighborhood blocking on top of StringCheese. The sibling
will be renamed to `stringcheese-linkage` and moved under the
umbrella name in a follow-up wave; the substantive scope split
(compute per-field vs decide per-record) remains.

**Regex engines** — StringCheese's `find` / `replace` accept
`Pattern`s in the `str::find` sense (literals, closures, char sets).
Full regex is a separate library, not an umbrella responsibility.

**I/O and reader-driven pipelines** — manipulation and comparison
operate on in-memory `&str` / `&[u8]`. Streaming from a reader is a
downstream concern.

**Full ICU parity** — the WIT-based i18n interfaces target the
80/90/95 % of locale-aware use cases with pluggable, opt-in data
packs; parity with ICU's every corner (Java-only APIs, historical
calendar edge cases, deep transliteration graphs) is not the goal.
Callers who need that reach for ICU4X directly.

Historically, this repo shipped a `stringcheese-linkage` crate and a
`sorted_neighborhood` module in `stringcheese-index`; both were
extracted to the sibling record-linkage repo when the scope decision
crystallized. See the extraction commit for the migration record.

### Sub-project map

| Crate | Charter |
| --- | --- |
| `stringcheese` | Facade — re-exports every sub-project under one dependency |
| `stringcheese-core` | Traits, result types, descriptors, workspace/sequence abstractions |
| `stringcheese-corpus` | Golden-case schema, oracle framework, differential harness |
| `stringcheese-compare` | Comparison kernels: Levenshtein, Hamming, Jaro/Jaro-Winkler, Damerau/OSA, LCS, n-gram, set-similarity, MinHash, substring search |
| `stringcheese-align` | Pairwise alignment: Needleman-Wunsch, Smith-Waterman, edit scripts |
| `stringcheese-manip` | Manipulation: inspect/trim/case/split/join/replace/normalize/pad/slice/find/escape/quote/lines/template + `TextPipeline` IR |
| `stringcheese-unicode` | Preprocessing: NFC/NFD/NFKC/NFKD, case folding, graphemes, diacritics |
| `stringcheese-phonetic` | Phonetic keys: Soundex, NYSIIS, Double Metaphone (language-neutral core) |
| `stringcheese-cdc` | Rolling-hash fingerprints + FastCDC content-defined chunking |
| `stringcheese-index` | Metric-space and set-similarity indexes: BK-tree, VP-tree, q-gram inverted |
| `stringcheese-bench` | Criterion benchmarks + allocation-counting harness |
| `stringcheese-lang` | Language-pack infrastructure: `Language` trait, `Stemmer` / `Collator` / `LanguagePhoneticEncoder` plugin points, `Stopwords` and `SimpleTokenizer` helpers, plus a static `registry` (linkme distributed slice) each pack self-registers into via `register_language!` |
| `stringcheese-en` | English pack: ~150-word stopword list, Porter (1980) stemmer, simple tokenizer, Soundex phonetic hookup; self-registers into `stringcheese-lang::registry` as `"en"` |
| `stringcheese-de` | German pack: ~200-word stopword list, Snowball German stemmer, simple tokenizer, Kölner Phonetik (Postel 1969) hookup; self-registers into `stringcheese-lang::registry` as `"de"` |
| `stringcheese-fr` | French pack: ~200-word stopword list, Snowball French stemmer, elision-aware tokenizer, PHONEX phonetic hookup; self-registers into `stringcheese-lang::registry` as `"fr"` |
| `stringcheese-es` | Spanish pack: ~200-word stopword list, Snowball Spanish stemmer (attached-pronoun stripping + standard + verb + residual), simple tokenizer, PHONEX-Spanish phonetic hookup (Soundex-shaped 4-char key with Spanish-tuned preprocessing: `v/b` merger, `z/s` merger via *seseo*, silent `h`, `ñ→n`, `ll→l`, `ch→x`, `qu→k`, `rr→r`); self-registers into `stringcheese-lang::registry` as `"es"` |
| `stringcheese-pt` | Portuguese pack: ~200-word stopword list, Snowball Portuguese stemmer (nasal `ã→a~` / `õ→o~` prelude-postlude placeholder mechanism, standard + verb + residual + residual-form steps), simple tokenizer, PHONEX-Portuguese phonetic hookup (Soundex-shaped 4-char key with Portuguese-tuned preprocessing: `ç→s`, `lh→l`, `nh→n`, `ch→x`, `qu→k`, `rr→r`, `v/b` merger, `z/s` merger, silent `h`, nasal-vowel accent fold); European Portuguese as the default with pt-BR / pt-PT specialization deferred; self-registers into `stringcheese-lang::registry` as `"pt"` |
| `stringcheese-nl` | Dutch pack: ~160-word stopword list, Snowball Dutch stemmer (diacritic-fold + glide-mark prelude, R1 adjusted-to-≥3 / R2 regions, four-step cascade: `heden`/`en`/`s` → `e`-ending → `heid` → `end`/`ing`/`ig`/`lijk`/`baar`/`bar`, plus short-vowel `aa`/`ee`/`oo`/`uu` undouble; `gem` guard on `-en` strip; `-cht` cluster preserved by construction), simple tokenizer, PHONEX-Dutch phonetic hookup (Soundex-shaped 4-char key with Dutch-tuned preprocessing: `ij→i`, `sch→sX`, `ch→g`, `rr→r`, silent `h`, diaeresis / acute vowel fold; labial `B/P/F/V/W` merger; velar-palatal `C/K/G/Q/J/X` cluster); Netherlands Dutch as the default with Belgian-Dutch specialization deferred; self-registers into `stringcheese-lang::registry` as `"nl"` |
| `stringcheese-nn` | Norwegian (Nynorsk) pack: sibling to `stringcheese-no` covering the second official Norwegian written standard. ~130-word Nynorsk-tuned stopword list (Nynorsk-specific pronouns `eg`/`ho`/`me`/`dei`/`dykk`/`dykkar`/`deira`/`honom`, articles `ein`/`ei`/`eit`, negation `ikkje`, interrogatives `kva`/`kven`/`kvifor`/`korleis`/`kvar`, adverbs `so`/`difor`/`mykje`, copula/auxiliary paradigms `vera`/`vore`/`har`/`hadde`/`skal`/`skulle`/`vil`/`ville`/`kan`/`kunne`/`verta`/`vert`/`vart`/`vorte`, plus the Bokmål-shared function-word bulk `og`/`eller`/`men`/`i`/`på`/`til`/`med`/`av`/`frå`/`utan`/…); Snowball Norwegian stemmer — the upstream `norwegian.sbl` covers **both** written standards, so this pack ports the same three-step cascade documented in `stringcheese-no` (main-suffix Group A/B/C with the R1 ≥ 3 adjustment, consonant-pair `-dt`/`-vt` trailing-`t` strip, derivational-suffix delete over `leg`/`eleg`/`ig`/`eig`/`lig`/`elig`/`els`/`lov`/`elov`/`slov`/`hetslov`); whitespace-and-punctuation tokenizer (transparent `SimpleTokenizer` wrapper — `æ`/`ø`/`å` are `char::is_alphanumeric`); PHONEX-Norwegian phonetic hookup (algorithmically identical to `stringcheese-no`'s encoder — Nynorsk and Bokmål share their phonological cluster set; adapter name `"phonex-nn"` distinguishes the pack). Registers BCP-47 `"nn"` (never `"no"`, so it can coexist cleanly with `stringcheese-no`'s `"no"`-free `"nb"` registration). Self-registers into `stringcheese-lang::registry` as `"nn"`. |
| `stringcheese-no` | Norwegian (Bokmål) pack: ~170-word stopword list drawn from the Snowball project's `norwegian/stop.txt` (keeps a handful of Nynorsk-flavored high-frequency function words `ikkje`/`eg`/`me`/`dei`/`kva`/`korleis` alongside their Bokmål equivalents for parity with the upstream Snowball stop list), Snowball Norwegian stemmer per Porter/Boulton `norwegian.sbl` — three-step cascade: (1) main-suffix Group A plain-delete (`a`/`e`/`ede`/`ande`/`ende`/`ane`/`ene`/`hetene`/`en`/`heten`/`ar`/`er`/`heter`/`as`/`es`/`edes`/`endes`/`enes`/`hetenes`/`ens`/`hetens`/`ers`/`ets`/`et`/`het`/`ast`) + Group B bare-`s` (deleted when preceded by a valid s-ending `b c d f g h j l m n o p r t v y z`, or `k` provided the char before `k` is non-vowel — note the spec's deliberate inclusion of vocalic `o`/`y` in the s-ending set) + Group C `-erte`/`-ert` → `-er` rewrite; (2) consonant-pair `-dt`/`-vt` trailing-`t` strip in R1; (3) derivational-suffix delete (`leg`/`eleg`/`ig`/`eig`/`lig`/`elig`/`els`/`lov`/`elov`/`slov`/`hetslov`) in R1; R1 adjusted so it never begins before char index 3. Whitespace-and-punctuation tokenizer (transparent wrapper around `SimpleTokenizer`; Norwegian is delimiter-clean and the extra letters `æ`/`ø`/`å` satisfy `char::is_alphanumeric`). PHONEX-Norwegian Soundex-shaped 4-char phonetic hookup with Norwegian-tuned preprocessing: cluster substitutions `skj→S`, `sk` before front vowel (`e`/`i`/`y`, plus fold-derived `E` from `æ`/`ø`) → `S`, `kj→C`, `k` before front vowel → `C`, `ch→S`, silent word-interior `h` drops; Norwegian-vowel folds `å → O` (open back rounded), `æ → E` (open front), `ø → E` (rounded mid front); adapter name `"phonex-no"`. **Bokmål-only** — the pack registers BCP-47 `"nb"` (not the macrolanguage `"no"`) so a future `stringcheese-nn` (Nynorsk) sibling can register `"nn"` cleanly without shadowing this pack; `"no"` is deliberately not resolved by the registry until an application-level fallback layer opts in. Nynorsk (`stringcheese-nn`), Danish (`stringcheese-da`), and Icelandic (`stringcheese-is`) siblings deferred; Métaphone Norwegian, compound-noun splitting, and Norwegian-tailored collator (`æ`/`ø`/`å` sort after `z`) also deferred. Self-registers into `stringcheese-lang::registry` as `"nb"`. |
| `stringcheese-pl` | Polish pack: ~280-word Unicode stopword list carrying the full Polish diacritic inventory (`ą ć ę ł ń ó ś ź ż`), light suffix-stripping stemmer in the Snowball style (there is no stable canonical `polish.sbl` and the community-standard Stempel/Egothor algorithm requires a large trained transducer out of scope for a per-crate pack; the module strips common nominal / adjectival / verbal / adverbial suffixes on `Vec<char>` indices with an RV floor and a 2-char min-stem guard), simple whitespace-and-punctuation tokenizer that treats every Polish letter (including the diacritic-carrying ones and the digraphs `sz`/`cz`/`rz`/`ch`/`dz`/`dź`/`dż`) as word-internal, PHONEX-Polish phonetic hookup (Soundex-shaped 4-char key with Polish-tuned preprocessing: digraphs `sz→S`, `cz→C`, `rz→R`, `ch→K`; `ó→U` conflation because `ó` and `u` are phonetically identical in modern Polish; nasal-vowel fold `ą→a`, `ę→e`; `ż` and `ź` both merge to `Z` in the sibilant class; `ń→N`, `ć→C`, `ś→S`, `ł→L`; silent `h`; adapter name `"phonex-pl"`). Overrides `Language::is_stopword` to apply Unicode case-fold (default `str::eq_ignore_ascii_case` misses `Ą→ą`/`Ć→ć`/`Ż→ż`/etc). Polish-tailored collator, Métaphone-Polish, Morfologik/Stempel dictionary lemmatization, and prefix-aspect stripping deferred. Self-registers into `stringcheese-lang::registry` as `"pl"`. |
| `stringcheese-ja` | Japanese pack: ~120-word stopword list, character-type-based (dictionary-free) tokenizer, Kunrei-shiki (ISO 3602) romanization phonetic hookup, minimal polite/plural stemmer. First non-Latin-script pack — full morphological tokenization deferred (needs kuromoji-scale dictionary outside the wasm-first / offline-first envelope). Self-registers into `stringcheese-lang::registry` as `"ja"`. |
| `stringcheese-ko` | Korean pack (**first Hangul-script pack**; **agglutinative morphology**): ~60-word stopword list covering demonstratives, interrogatives, conjunctions, common adverbs, and dictionary forms of the copula and high-frequency verbs (case particles `-은/-는/-이/-가/…` deliberately omitted from the stopword list because they attach at the syllable end and are stripped by the stemmer); algorithmic Hangul syllable ↔ jamo decomposition / composition per the closed-form Unicode 3.12 formulas (`decompose_syllable(char) -> (L, V, Option<T>)` and `compose_jamo(L, V, T) -> char` — every one of the 11172 precomposed syllables in U+AC00..=U+D7A3 round-trips through the `jamo_decompose` integration test); coarse suffix-stripping stemmer that iteratively peels the closed set of common noun-attached case particles (`-에서`, `-까지`, `-부터`, `-에게`, `-으로`, `-는`, `-은`, `-을`, `-를`, `-이`, `-가`, `-에`, `-로`, `-와`, `-과`, `-의`, `-도`, `-만`, `-다`) with longest-match ordering (so `학교에서` strips `-에서` before ever seeing `-에`) and a min-stem-length ≥ 1 syllable guard — verb / adjective conjugation stripping deferred to a dictionary-driven `stringcheese-ko-morph` sibling because Korean verb endings fuse with stems whose surface form varies with the following vowel (`먹`+`-어요` → `먹어요`; `가`+`-아요` → `가요` after vowel elision) and paradigm-aware analysis needs a real morphological analyzer; whitespace-and-punctuation tokenizer (Korean is space-delimited between orthographic words, unlike Japanese / Chinese — the tokenizer splits on ASCII whitespace, ASCII punctuation, CJK Symbols and Punctuation U+3001..=U+303F, General Punctuation U+2000..=U+206F, and Halfwidth-and-Fullwidth punctuation, while keeping Latin / digit characters glued to any adjacent Hangul to match how Korean readers group borrowed English terms like `iOS앱` and `2025년`); PHONEX-Korean phonetic hookup — a two-step algorithm that first romanizes via Revised Romanization of Korean (RR, per the National Institute of the Korean Language 2000 tables — L jamo values `ᄀ→g`, `ᄁ→kk`, `ᄂ→n`, `ᄃ→d`, `ᄄ→tt`, `ᄅ→r`, `ᄆ→m`, `ᄇ→b`, `ᄈ→pp`, `ᄉ→s`, `ᄊ→ss`, `ᄋ→""` null onset, `ᄌ→j`, `ᄍ→jj`, `ᄎ→ch`, `ᄏ→k`, `ᄐ→t`, `ᄑ→p`, `ᄒ→h`; V jamo values `ᅡ→a`, `ᅢ→ae`, `ᅣ→ya`, `ᅤ→yae`, `ᅥ→eo`, `ᅦ→e`, `ᅧ→yeo`, `ᅨ→ye`, `ᅩ→o`, `ᅪ→wa`, `ᅫ→wae`, `ᅬ→oe`, `ᅭ→yo`, `ᅮ→u`, `ᅯ→wo`, `ᅰ→we`, `ᅱ→wi`, `ᅲ→yu`, `ᅳ→eu`, `ᅴ→ui`, `ᅵ→i`; T jamo reading-form values `ᆨ→k` / `ᆮ→t` / `ᆯ→l` / `ᆸ→p` / `ᆼ→ng` etc.) then reduces to a 4-character Soundex-shape key with Korean-tuned consonant classification (bilabial `B P F V M W → 1`, velar `C G K Q → 2`, dental-alveolar `D T → 3`, liquid `L R → 4` (Korean `ㄹ` is both `r`/`l`), alveolar-nasal `N → 5`, sibilant + affricate `S Z X J → 7`, vowels + `H → 0` dropped); adapter name `"phonex-ko"`. **First Hangul-script pack** — every precomposed syllable is **3 bytes in UTF-8** (U+AC00..=U+D7A3 falls in UTF-8's 3-byte range) so all tokenizer / stemmer / romanization arithmetic runs via `str::chars` and `char` values (byte offsets would silently corrupt syllable boundaries); the jamo model uses the conjoining jamos (U+1100..=U+11FF) exclusively — the legacy Hangul Compatibility Jamo block (U+3130..=U+318F) is a compatibility mapping for glyph rendering and callers that ingest it should NFKC-decompose first. Context-sensitive full RR rules (assimilation, palatalization, liaison across syllable boundaries), McCune-Reischauer romanization, Hangul Compatibility Jamo normalization, compound-word splitting (`대한민국` → `대한` + `민국`), Korean-tailored collator, and North Korean orthography all deferred. Self-registers into `stringcheese-lang::registry` as `"ko"`. |
| `stringcheese-zh` | Chinese (Simplified) pack: ~80-word stopword list, character-level tokenizer (dictionary-free — every CJK Han scalar becomes its own token, Latin/digit runs stay together, matching BERT's Chinese preprocessing philosophy), identity stemmer (Chinese is analytic — no inflection to strip), and a Hanyu Pinyin (tone-mark-stripped) phonetic hookup over a curated ~1000-entry high-frequency Han character table (~85% running-text coverage; unknown Han encodes as `?` to keep the output ASCII); adapter name `"pinyin-zh"`. Targets Simplified Chinese (mainland / Singapore); Traditional support requires a separate `stringcheese-zh-hant` sibling or an S↔T converter (both deferred). Dictionary-driven word segmentation (jieba / thulac / pkuseg parity) is deferred to a `stringcheese-zh-jieba` sibling — the base pack stays wasm-first / offline-first. Cantonese Jyutping, Wade-Giles, Bopomofo, and tone-preserving pinyin all deferred. Self-registers into `stringcheese-lang::registry` as `"zh"` (BCP-47 fallback: `zh-CN`, `zh-Hans`, `zh-Hans-CN` all resolve here). |
| `stringcheese-ar` | Arabic pack: ~150-word stopword list, Larkey ALP light10 stemmer (Larkey, Ballesteros, Connell 2002), diacritic/alef/yeh/teh-marbuta normalizer, Buckwalter transliteration phonetic hookup, whitespace-based tokenizer. First right-to-left-script pack — validates that StringCheese's byte/character-sequence processing model handles RTL scripts without special-case machinery (RTL is a display concern; all processing is on logical UTF-8 order). Full root-and-pattern morphological analysis deferred (needs Buckwalter-scale template lexicon). Self-registers into `stringcheese-lang::registry` as `"ar"`. |
| `stringcheese-he` | Hebrew pack (**second right-to-left-script pack** after Arabic — different Semitic script, same logical-UTF-8-order processing model): ~130-word stopword list, light suffix-and-prefix stemmer (single prefix strip covering the definite article `ה-`, coordinating particles `ו- ב- כ- ל- מ- ש-`, and the eleven common two-letter combined-prefix forms `וה בה כה לה מה שה וב וכ ול ומ וש`; single suffix strip covering plural `-ים` / `-ות`, feminine `-ה`, possessives `-י -ך -ו -ה -נו -כם -הם` and their -יהם / -יהן variants, and past-tense endings `-תי -ת -נו -תם -ו`, with a 2-character over-strip guard), niqqud + cantillation normalizer (strips U+05B0..=U+05BC / U+05BE / U+05BF / U+05C1..=U+05C2 / U+05C4..=U+05C5 / U+05C7 vowel points and the full U+0591..=U+05AF te'amim range by default; opt-in final-form folding `ך→כ / ם→מ / ן→נ / ף→פ / ץ→צ`; opt-in Hebrew-punctuation stripping for maqaf / geresh / gershayim), maqaf-aware tokenizer that treats `־` (U+05BE) as a word-internal joiner so compound words like `בית־ספר` "school" stay one token, simplified ISO 259 style single-character ASCII transliteration phonetic hookup (22 base letters plus 5 final forms all fold to their base form's code; emphatic / pharyngeal / sibilant letters get uppercase-Latin / punctuation stand-ins in Buckwalter style — `H` for ח, `T` for ט, `` ` `` for ע, `c` for צ, `$` for ש; adapter name `"iso-259-he"`). Full root-and-pattern morphological analysis, verb-binyan awareness, Biblical-Hebrew tuning, and Yiddish / Ladino / Judeo-Aramaic packs deferred. Self-registers into `stringcheese-lang::registry` as `"he"`. |
| `stringcheese-hi` | Hindi pack (**first Devanagari-script pack**): ~130-word Devanagari stopword list covering personal / demonstrative / interrogative pronouns, postpositions, conjunctions, particles, high-frequency forms of the copula *होना* and auxiliary *करना*, and common adverbs; light Hindi suffix-stripping stemmer — **there is no canonical Snowball Hindi algorithm** (Snowball's catalogue lists Hindi as "planned" but ships no `hindi.sbl`; the module ships a deliberately conservative Ramanathan-Rao 2003-style subset covering gender / number markers `-ों -ें -ा -ी -े -ि -ु`, fused postpositions `-का -की -के -ने -को -से -में -पर`, and verb tense endings `-ता -ती -ते -या -ई -ए`, all matched on `Vec<char>` with a 2-scalar min-stem guard); Devanagari-aware normalizer (opt-in Devanagari-digit `०-९ → 0-9` folding and opt-in nukta `़` U+093C stripping — both off by default because nukta is semantically meaningful (`ज` "ja" vs. `ज़` "za" are different phonemes) and the two digit blocks serve distinct visual purposes in Hindi typography); Devanagari-aware tokenizer that treats the full Devanagari block U+0900..=U+097F as word-internal (letters plus dependent vowel signs / matras / virama / anusvara / chandrabindu / visarga / nukta) with the three Devanagari punctuation scalars (`।` U+0964 danda "full stop", `॥` U+0965 double danda "end of verse", `॰` U+0970 abbreviation sign) as explicit separators — otherwise the default `is_alphanumeric` splitter would shatter every word at every matra because Unicode classifies dependent vowel signs as Mark not Letter; IAST (International Alphabet of Sanskrit Transliteration) phonetic hookup with **Sanskrit-style inherent-schwa handling** — Devanagari is an abugida where every base consonant carries an implicit `a` (schwa) vowel unless a matra or virama overrides it, so `क` alone encodes to `ka` (not just `k`), `क्` (`क` + virama U+094D) encodes to bare `k`, `कि` (`क` + matra `ि`) encodes to `ki`, `सत्य` encodes to `satya` (virama on त suppresses its schwa, following य carries its own inherent schwa), and `राम` encodes to `rāma` (final म retains inherent schwa — modern colloquial Hindi drops it but the deletion is lexicon-driven and out of scope for this deterministic encoder); IAST mapping honors the classical 33 consonants with retroflex under-dots (`ṭ ṭh ḍ ḍh ṇ`), sibilant diacritics (`ś ṣ`), and velar/palatal nasal marks (`ṅ ñ`); nukta letters handled in both precomposed (`क़ ख़ ग़ ज़ ड़ ढ़ फ़`) and decomposed (base + `़`) forms via a one-scalar-lookahead state machine; adapter name `"iast-hi"`. **First Devanagari-script pack** — every Devanagari letter is **3 bytes in UTF-8** (Cyrillic is 2, Latin is 1) because U+0900..=U+097F falls in UTF-8's 3-byte range; all suffix / tokenizer / stemmer arithmetic runs on `Vec<char>` (byte offsets would silently corrupt 3-byte scalars). Marathi / Sanskrit / Nepali / other Indic-script packs (Bengali / Gurmukhi / Gujarati / Oriya / Tamil / Telugu / Kannada / Malayalam) and schwa-deletion / ITRANS / HK / SLP1 / ISO 15919 romanization adapters and full Snowball Hindi (if one ever appears) deferred. Self-registers into `stringcheese-lang::registry` as `"hi"`. |
| `stringcheese-mr` | Marathi pack (**second Devanagari-script pack** after `stringcheese-hi`; Marathi is Indo-Aryan and shares the Devanagari block U+0900..=U+097F with Hindi, so every Marathi letter is likewise **3 bytes in UTF-8** — all suffix / tokenizer / stemmer arithmetic runs on `Vec<char>` for the same reason): Marathi differs from Hindi in three linguistically-meaningful ways that this pack reflects — (1) Marathi **retains the Old Indo-Aryan neuter gender** (Hindi has only masculine and feminine), so verb / adjective agreement is three-way rather than two-way and the stopword list carries neuter demonstratives (`हे` n. sg. alongside `हा` m. sg. / `ही` f. sg.); (2) Marathi's case marking is **agglutinative** — case suffixes attach directly to the noun stem (`घराला` "to the house" — one orthographic word) rather than Hindi's postpositions written as separate words (`घर को` — two tokens), which means the Marathi stemmer is substantially more useful than Hindi's because Marathi words genuinely inflect at the surface spelling level; (3) Marathi retains the letters **`ळ` (U+0933, retroflex L, /ɭ/)** and **`ऱ` (U+0931, R with lower diagonal)** that Standard Modern Hindi lacks. ~55-word Marathi stopword list covering personal / demonstrative / interrogative pronouns (with the neuter forms and the clusivity-preserving 1pl `आम्ही` "we-exclusive" vs. `आपण` "we-inclusive"), the *independent* postpositions (`साठी` "for", `बद्दल` "about", `बरोबर` "together-with", `पासून` "from", `पर्यंत` "until", `शिवाय` "without" — the agglutinative case markers are handled by the stemmer, not the stopword list), conjunctions, particles, high-frequency forms of the copula *असणे* ("to be") and auxiliary *करणे* ("to do"), and common adverbs; longest-match-wins Marathi suffix stripper — **there is no canonical Snowball Marathi algorithm** (Marathi is not in the Snowball catalogue) — with a 2-scalar min-stem guard and an internal loop that **iterates to convergence** so a stacked suffix like the oblique-plural + case-marker combination in `घरांना` (houses-to) reduces to `घर` in a single external call; the suffix table covers the agglutinative case markers (`-ला`/`-ना` accusative-dative, `-चा`/`-ची`/`-चे`/`-च्या` genitive with head-noun gender agreement, `-ने`/`-नी` instrumental-ergative, `-त`/`-मध्ये` locative, `-ऊन`/`-हून` ablative, `-शी`/`-सह` sociative), the two-form (nom/oblique) plural markers (`-ां` general oblique plural, `-ीं` neut plural variant), the present-habitual verb personal endings (`-तो` 1sg-m / 3sg-m, `-ते` 1sg-f / 3sg-f / 3sg-n, `-तोस` 2sg-m, `-तेस` 2sg-f, `-तात` 3pl), the aorist / perfective endings (`-ला` 3sg-m, `-ली` 3sg-f, `-ले` 3sg-n / 3pl-m, `-ल्या` 3pl-f), the infinitive marker `-णे`, and the single-scalar linking-vowel matras (`-ा`, `-े`, `-ी`) that surface between the noun stem and an agglutinative case suffix; Marathi-aware tokenizer that treats the full Devanagari block U+0900..=U+097F (including the Marathi additions `ळ` U+0933 and `ऱ` U+0931) as word-internal with the three Devanagari punctuation scalars (`।` U+0964 danda, `॥` U+0965 double danda, `॰` U+0970 abbreviation sign) as explicit separators; two-stage ISO 15919 → PHONEX-Marathi phonetic hookup — the [`MarathiIso15919`] transliteration honors the **explicit-schwa** convention (a base consonant with no following matra or virama emits letter + `a`; virama `्` U+094D suppresses; matras override), inherits the classical 33-consonant mapping from Hindi with retroflex under-dots (`ṭ ṭh ḍ ḍh ṇ`) and sibilant diacritics (`ś ṣ`), adds the Marathi-specific letters `ळ → ḷ` and `ऱ → ṟ`, uses the ISO 15919 form `ṁ` for anusvara (rather than Hindi's IAST `ṃ`) for consistency with the Bengali pack, and handles both precomposed nukta letters and decomposed `base + ़` sequences via a one-scalar-lookahead state machine; the ISO output then feeds a Soundex-shape 4-character reduction that folds Latin-with-diacritic scalars to their ASCII base (`ā → A`, `ṭ → T`, `ś → S`, `ḷ → L`, `ṟ → R`, etc.) and applies the standard Soundex classification with vowel-reset; adapter name `"phonex-mr"`, matching the shape of the Bengali pack (`phonex-bn`) and every other Latin-alphabet pack's phonetic hookup. The Devanagari transliteration tables are structurally adapted from the `stringcheese-hi` pack's IAST encoder (the two languages share the identical base script); the two Marathi-specific consonants and the Soundex-shape reduction are the Marathi additions. Konkani sibling (`stringcheese-kok`, Indo-Aryan of Maharashtra / Goa), Marathi-specific normalizer (Devanagari-digit folding and nukta stripping are inherited via `stringcheese-hi::HindiNormalizer` — the folds apply unchanged to Marathi text), full Marathi morphological analysis (past-tense infixes, honorific marking, aspectual auxiliaries, non-finite verb forms), colloquial surface forms, schwa-deletion, ITRANS / HK / SLP1 romanization adapters, and full Snowball Marathi (if one ever appears) deferred. Self-registers into `stringcheese-lang::registry` as `"mr"`. |
| `stringcheese-bn` | Bengali pack (**second Brahmic-script pack** after `stringcheese-hi`; Bengali script U+0980..=U+09FF is a sibling of Devanagari U+0900..=U+097F, another abugida in the Brahmic family, and every Bengali letter is likewise **3 bytes in UTF-8** — all suffix / tokenizer / stemmer arithmetic runs on `Vec<char>` for the same reason): ~65-word Bengali stopword list covering personal / demonstrative / interrogative pronouns, postpositions, conjunctions, particles, high-frequency forms of the copula *হওয়া* ("to be") and auxiliary *করা* ("to do"), and common adverbs; light Bengali suffix-stripping stemmer — **there is no canonical Snowball Bengali algorithm** — the module ships a deliberately conservative rule-based subset covering plural markers (`-গুলি -গুলো -দের -রা`) and the most-common case endings (`-কে -তে -র -রে -য়`), all matched on `Vec<char>` with a 2-scalar min-stem guard; Bengali-aware tokenizer that treats the full Bengali block U+0980..=U+09FF as word-internal (letters plus dependent vowel signs / matras / kars / halant / anusvara / chandrabindu / visarga / nukta) with the Devanagari-inherited danda (`।` U+0964) and double danda (`॥` U+0965) as separators (they sit outside the Bengali block and satisfy the `!is_alphanumeric` rule naturally); two-stage ISO 15919 → PHONEX-Bengali phonetic hookup — the [`BengaliIso15919`] transliteration honors the **explicit-schwa** convention (a base consonant with no following matra or halant emits letter + `a`; halant `্` U+09CD suppresses; matras override), covers the 33 base consonants with retroflex under-dots (`ṭ ṭh ḍ ḍh ṇ`), sibilant diacritics (`ś ṣ`), and the three Bengali-extension nukta letters (`ড়` "ṛ" retroflex flap, `ঢ়` "ṛh" breathy, `য়` "ẏ" palatal glide) handled in both precomposed and decomposed forms, plus the khanda ta (`ৎ` U+09CE) as a schwa-less final `t`, and the ISO output then feeds a Soundex-shape 4-character reduction that folds Latin-with-diacritic scalars to their ASCII base (`ā → A`, `ṭ → T`, `ś → S`, etc.) and applies the standard Soundex classification with vowel-reset; adapter name `"phonex-bn"`, matching the shape of the other Latin-alphabet packs' phonetic hookups. Assamese sibling (`stringcheese-as`, adds `ৰ` U+09F0 and `ৱ` U+09F1), Manipuri and other Bengali-script users, schwa-deletion, ITRANS / HK / SLP1 romanization adapters, and full Snowball Bengali (if one ever appears) deferred. Self-registers into `stringcheese-lang::registry` as `"bn"`. |
| `stringcheese-ml` | Malayalam pack (**second Dravidian pack**, sibling of `stringcheese-ta`, and **fourth Brahmic-script pack** after `stringcheese-hi`, `stringcheese-bn`, and `stringcheese-ta`; first Malayalam-script pack; Malayalam script U+0D00..=U+0D7F is a Brahmic-family abugida, and every Malayalam letter is **3 bytes in UTF-8** — all suffix / tokenizer / stemmer arithmetic runs on `Vec<char>` for the same reason as the earlier Brahmic packs): Malayalam phonology is a hybrid of the classical Sanskrit-inherited system and the Dravidian tradition — the script retains the classical **four-way stop series** (unaspirated-voiceless / aspirated-voiceless / unaspirated-voiced / aspirated-voiced across five places of articulation, like Devanagari and Bengali and unlike Tamil which collapses to one stop per place) plus the three Dravidian-family additions (`ള` retroflex-lateral "ḷ"; `ഴ` retroflex-approximant "ḻ"; `റ` alveolar-tap "ṟ" — the same set Tamil carries); **six atomic "pure consonant" chillu letters** live in U+0D7A..=U+0D7F (`ൺ ൻ ർ ൽ ൾ ൿ`, respectively from base `ണ ന ര ല ള ക`) representing a word-final consonant without any following vowel, so words like `ഞാൻ` "I" end in chillu `ൻ`, `അവർ` "they" ends in chillu `ർ`, `അവൾ` "she" ends in chillu `ൾ` — unique among the Brahmic scripts; **conjunct consonants form via virama** `്` U+0D4D (unlike Tamil, which lacks true conjuncts and writes every cluster with a visible pulli — Malayalam is much closer to Devanagari / Bengali in this regard, with some clusters rendered as ligatures and some as visible virama sequences); Malayalam also uses zero-width joiner U+200D and zero-width non-joiner U+200C to control conjunct formation at typing input, though those sit outside the block; ~55-word Malayalam stopword list covering personal / demonstrative / interrogative pronouns, postpositions, conjunctions, negation and affirmation particles, high-frequency forms of the copula *ആണ്* / *ഉണ്ട്* ("to be"), and common adverbs; longest-match-wins Malayalam suffix stripper — **there is no canonical Snowball Malayalam algorithm** — the module ships a deliberately conservative rule-based subset covering the six canonical case suffixes (accusative `-നെ`; genitive `-ന്റെ` / `-ുടെ`; dative `-ന്`; locative `-ിൽ`; instrumental `-ാൽ` / `-ിനാൽ`; sociative `-ോട്`), the plural markers (`-കൾ` inanimate; `-മാർ` animate; plus the ങ്ങ-linked variant `-ങ്ങൾ`), the highest-frequency verb tense / aspect / adjectival endings (`-ുന്നു` present; `-ുന്ന` present-adjectival; `-ിയ` past-adjectival; `-ുക` infinitive), and the emphatic particles (`-ും` "also" / future; `-ോ` interrogative; `-ല്ലേ` tag question), all matched on `Vec<char>` with a 2-scalar min-stem guard — and with **bare chillu-letter word endings deliberately left in place** so pronouns like `ഞാൻ` / `അവർ` / `അവൾ` survive the stemmer unchanged (the suffix table lists only multi-scalar chillu-carrying endings like `-ിൽ` / `-ന്റെ` / `-ന്` / `-കൾ` / `-മാർ`, never a bare chillu on its own); Malayalam-aware tokenizer that treats the full Malayalam block U+0D00..=U+0D7F as word-internal (letters plus matras plus virama plus anusvara / visarga plus the six chillu letters — every combining mark and every chillu stays inside its token) with the standard ASCII punctuation (`.`, `?`, `!`) as separators (Malayalam, like Tamil and unlike Devanagari / Bengali, does **not** inherit the danda); two-stage ISO 15919 → PHONEX-Malayalam phonetic hookup — the [`MalayalamIso15919`] transliteration honors the **explicit-schwa** convention (a base consonant with no following matra or virama emits letter + `a`; virama `്` suppresses; matras override), covers the 33 base consonants with retroflex under-dots (`ṭ ṭh ḍ ḍh ṇ`), sibilant diacritics (`ś ṣ`), velar / palatal nasal marks (`ṅ ñ`), the three Dravidian additions (`ḷ ḻ ṟ`), and the Malayalam digit block `൦..൯`, plus **chillu letters encoded as bare-consonant / suppressed-schwa forms** (`ൻ → n`, `ർ → r`, `ൽ → l`, `ൾ → ḷ`, `ൺ → ṇ`, `ൿ → k`), and the ISO output then feeds a Soundex-shape 4-character reduction that folds Latin-with-diacritic scalars to their ASCII base (`ā → A`, `ē → E`, `ō → O`, `ṭ → T`, `ḍ → D`, `ṇ → N`, `ḷ → L`, `ḻ → L`, `ṟ → R`, `ṅ → N`, `ñ → N`, `ś → S`, `ṣ → S`, `ṁ → M`, `ḥ → H`) and applies the standard Soundex classification with vowel-reset; adapter name `"phonex-ml"`, matching the shape of the other Latin-alphabet packs' phonetic hookups. Telugu / Kannada sister Dravidian packs, full Malayalam morphological analysis (sandhi-driven consonant alternations at stem boundaries, honorific marking, aspectual auxiliaries, non-finite verb forms), colloquial surface forms, ITRANS / HK / SLP1 romanization adapters, and full Snowball Malayalam (if one ever appears) deferred. Self-registers into `stringcheese-lang::registry` as `"ml"`. |
| `stringcheese-ta` | Tamil pack (**first Dravidian pack** and **third Brahmic-script pack** after `stringcheese-hi` and `stringcheese-bn`; Tamil script U+0B80..=U+0BFF is a Brahmic-family abugida sibling of Devanagari and Bengali, and every Tamil letter is likewise **3 bytes in UTF-8** — all suffix / tokenizer / stemmer arithmetic runs on `Vec<char>` for the same reason): Tamil is Dravidian, not Indo-Aryan, and its script encodes a **single stop-consonant series** per place of articulation (k / c / ṭ / t / p — no voiced-aspirated four-way split like Devanagari's) plus five Grantha loans (`ஜ ஷ ஸ ஹ`), with two Tamil-only nasals and liquids (`ன` alveolar-n distinct from `ந` dental-n; `ற` alveolar-tap distinct from `ர` dental-r; `ழ` retroflex approximant "ḻ" — the famous "ḻ" of "tamiḻ" — distinct from `ள` retroflex-lateral and `ல` dental-lateral); **every Tamil consonant cluster uses an explicit visible pulli** `்` U+0BCD (unlike Devanagari, which stacks consonants into ligature-forming conjuncts and visually elides the virama), so words like `நன்றி` "thanks" carry the pulli word-internally and the tokenizer must keep it inside the token; ~55-word Tamil stopword list covering personal / demonstrative / interrogative pronouns, postpositions, conjunctions, negation and affirmation particles, high-frequency forms of the auxiliary *இரு-* ("to be") and *ஆகு-* ("to become"), and common adverbs; longest-match-wins Tamil suffix stripper — **there is no canonical Snowball Tamil algorithm** — the module ships a deliberately conservative rule-based subset covering the plural marker (`-கள்`), all eight canonical case suffixes with both independent-vowel and matra surface forms (accusative `-ஐ` / `-ை`; instrumental `-ஆல்` / `-ால்`; dative `-கு`; genitive `-இன்` / `-ின்`; locative `-இல்` / `-ில்`; sociative `-ஓடு` / `-ோடு` / `-உடன்`; allative `-வரை`), the six present-tense verb personal endings (`-கிறேன் -கிறாய் -கிறார் -கிறோம் -கிறீர்கள் -கிறார்கள்`), the two present-tense marker infixes that also appear in surface stems (`-கிறு -கின்று`), the two interrogative particles (`-ஆ -ஏ`), and the bare future-tense marker (`-வ`), all matched on `Vec<char>` with a 2-scalar min-stem guard; Tamil-aware tokenizer that treats the full Tamil block U+0B80..=U+0BFF as word-internal (letters plus matras plus pulli) with the standard ASCII punctuation (`.`, `?`, `!`) as separators (Tamil does **not** inherit the Devanagari danda, unlike Bengali); two-stage ISO 15919 → PHONEX-Tamil phonetic hookup — the [`TamilIso15919`] transliteration honors the **explicit-schwa** convention (a base consonant with no following matra or pulli emits letter + `a`; pulli `்` suppresses; matras override), covers the 18 native consonants with retroflex under-dots (`ṭ ṇ ḷ`), alveolar under-bars (`ṉ ṟ`), and the Tamil-only retroflex approximant `ḻ`, plus the 5 Grantha loans and the Tamil digit block `௦..௯`, and the ISO output then feeds a Soundex-shape 4-character reduction that folds Latin-with-diacritic scalars to their ASCII base (`ā → A`, `ē → E`, `ō → O`, `ṭ → T`, `ṇ → N`, `ṉ → N`, `ṟ → R`, `ḷ → L`, `ḻ → L`, etc.) and applies the standard Soundex classification with vowel-reset; adapter name `"phonex-ta"`, matching the shape of the other Latin-alphabet packs' phonetic hookups. Malayalam / Telugu / Kannada sister Dravidian packs, full Tamil morphological analysis (past-tense infixes interacting with sandhi at the stem boundary, honorific marking, aspectual auxiliaries, non-finite verb forms), colloquial (koṭuntamiḻ) surface forms, ITRANS / HK / SLP1 romanization adapters, and full Snowball Tamil (if one ever appears) deferred. Self-registers into `stringcheese-lang::registry` as `"ta"`. |
| `stringcheese-fa` | Persian (Farsi) pack: ~160-word stopword list, light Persian stemmer (nominal-suffix stripper covering plural `-ها` / `-های`, comparative `-تر` / superlative `-ترین`, and the six possessive clitics `-ام / -ای / -اش / -مان / -تان / -شان`, each with optional leading ZWNJ), Persian-specific normalizer (Arabic-yeh → Persian-yeh `ي → ی`, Arabic-kaf → Persian-kaf `ك → ک`, tatweel stripped by default, opt-in Extended-Arabic-Indic `۰-۹ → 0-9` digit fold, opt-in ZWNJ U+200C stripping, opt-in `ۀ → ه + ی` decomposition), ZWNJ-aware tokenizer that treats U+200C as word-internal so compound words like `می‌روم` stay one token, Persian-Buckwalter transliteration phonetic hookup (four Persian additions `پ→p / چ→c / ژ→J / گ→g` on top of the classical Arabic-Buckwalter table; ghain reassigned to capital `G` to break the `g` collision with gaf; Persian yeh / kaf and Arabic yeh / kaf both encode to `y` / `k` and inverse to the Persian form; adapter name `"persian-buckwalter"`). Uses the shared Arabic script — the pack processes strings in logical UTF-8 order and treats RTL as a display concern (same convention as `stringcheese-ar`). Verb morphology, ezafeh detection, compound-verb decomposition, and Dari / Tajik varieties deferred to follow-up packs. Self-registers into `stringcheese-lang::registry` as `"fa"`. |
| `stringcheese-tr` | Turkish pack: ~180-word stopword list, Snowball Turkish stemmer (Eryiğit & Adalı 2004) with vowel-harmony-aware suffix stripping across nominal-verb / noun / derivational passes, Turkic-aware case-fold helper (dotted `İ → i`, dotless `I → ı`), simple tokenizer, light PHONEX-Turkish phonetic hookup (Turkish orthography is already highly phonetic, so a small Soundex-shape key is sufficient). Overrides `Language::is_stopword` to apply the Turkic case-fold before ASCII-insensitive comparison. Self-registers into `stringcheese-lang::registry` as `"tr"`. |
| `stringcheese-et` | Estonian pack (**second Uralic (non-Indo-European) language pack**, sibling of Finnish in the Finnic branch): ~90-word stopword list carrying the Estonian diacritic set (`ä ö ü õ` native vowels — no `å`, unlike Finnish — plus loanword `š ž`), lightweight suffix-stripping stemmer (**Snowball has no official Estonian algorithm** — the shipped module is a hand-audited longest-match suffix stripper inspired by academic references, running a single pass over a length-sorted suffix table with a 2-character multi-char min-stem floor and a stricter 4-character single-char min-stem floor to protect short base words like `kool` / `kass` / `ilus`) covering the fourteen grammatical cases (Estonian dropped Finnish's instructive but retains almost the full case inventory under different names — illative `-sse`, inessive `-s`, elative `-st`, allative `-le`, adessive `-l`, ablative `-lt`, translative `-ks`, terminative `-ni`, essive `-na`, abessive `-ta`, comitative `-ga`), the plural markers (`-d` nominative, `-id` partitive, `-te` / `-de` genitive), common verb inflections (`-me` 1pl, `-te` 2pl, `-vad` 3pl present, `-sin` 1sg past, `-sid` 2sg / 3pl past, `-sime` / `-site` past plural, `-b` 3sg present, `-ma` / `-da` infinitives, `-nud` / `-tud` participles), and the diminutive `-ke` / `-kene`. The `-si-` past-tense forms (`-sid`, `-sime`, `-site`) carry a vowel-preceding context constraint to disambiguate from the noun-plural `-id` (compare `kass + -id → kassid` "cats" vs. `käi + -sid → käisid` "you went"). **Vowel harmony is NOT a factor** — unlike Finnish, modern Standard Estonian lost native vowel harmony centuries ago, so the suffix table lists each suffix exactly once (no back / front harmony variants). Whitespace-and-punctuation tokenizer (transparent `SimpleTokenizer` wrapper — Estonian is delimiter-clean; compound splitting like `raamatukogu → raamatu + kogu` requires a lexicon and is deferred). PHONEX-Estonian phonetic hookup (Soundex-shaped 4-char key with Estonian-tuned preprocessing: long-consonant collapse `kk`/`tt`/`ll`/`pp`/`mm`/`nn`/`ss`/`rr` → single, long-vowel collapse `aa`/`ee`/`ii`/`oo`/`uu`/`õõ`/`ää`/`öö`/`üü` → single, `ä → a`, `ö → o`, `õ → o`, `ü → u`, and loanword `š → s`, `ž → z` folds — note `õ` and `ö` both collapse to the same ASCII `o` for phonetic-key purposes; adapter name `"phonex-et"`). Overrides `Language::is_stopword` to apply Unicode case-fold (Estonian has no locale-specific quirks — unlike Turkish's dotted / dotless `I` distinction — but the default trait method uses ASCII-only case-fold which misses `Ä → ä`/`Ö → ö`/`Ü → ü`/`Õ → õ`/`Š → š`/`Ž → ž`). Lexicon-driven consonant-gradation reversal (`raamat` ↔ `raamatu`, `laps` ↔ `lapse`), vowel-alternation reversal (`käsi` ↔ `käed`), compound-word splitting, and Võro / Seto dialect packs deferred. Self-registers into `stringcheese-lang::registry` as `"et"`. |
| `stringcheese-fi` | Finnish pack (**first Uralic (non-Indo-European) language pack**): ~170-word stopword list carrying the Finnish diacritics (`ä ö å`, `å` only in loanwords / Swedish-origin names), Snowball Finnish stemmer running the six-step cascade documented at <https://snowballstem.org/algorithms/finnish/stemmer.html> — particles (`-kin`/`-kaan`/`-kään`/`-ko`/`-kö`/`-han`/`-hän`/`-pa`/`-pä` clitics, then `-sti` in R2) → possessives (`-ni`/`-si`/`-nsa`/`-nsä`/`-mme`/`-nne` plus context-guarded `-an`/`-än`/`-en`) → cases (Finnish has 15 grammatical cases; the stemmer strips inessive `-ssa`/`-ssä`, elative `-sta`/`-stä`, illative `-hVn` with vowel matching, adessive `-lla`/`-llä`, ablative `-lta`/`-ltä`, allative `-lle`, essive `-na`/`-nä`, translative `-ksi`/`-kse-`, partitive `-ta`/`-tä`/`-a`/`-ä` with R1-strict guard, genitive `-n`, plural illative `-siin`/`-seen`, plural genitive `-tten`/`-den`) → other derivational (`-mpi`/`-mpa`/`-mpä`/`-mmi`/`-mma`/`-mmä` comparative in R2, `-impi`/`-impa`/`-impä`/`-immi`/`-imma`/`-immä` superlative in R2) → plurals (`-i`/`-j`/`-t` conditional on Step 3) → tidy-up (undouble trailing repeated restricted vowel in R1, drop trailing `-j` after `o`/`u`, undouble trailing repeated consonant for `-kk`/`-pp`/`-tt` gradation reversal). Vowel harmony handled by orthography-level enumeration: back-harmony (`a o u`) and front-harmony (`ä ö y`) suffix variants are separately listed in every step's table — the literal-match check IS the harmony check, no runtime predicate needed (unlike `-tr` which uses an explicit harmony guard). Finnish `y` classified as a **front rounded vowel** /y/ (like German ü), not an English-style glide — critical for region computation. R1/R2 computed as the standard Snowball VC-boundary regions on `Vec<char>` (Finnish has three multi-byte scalars `ä ö å` that would corrupt any byte-index arithmetic). Whitespace-and-punctuation tokenizer (transparent `SimpleTokenizer` wrapper — Finnish is delimiter-clean; compound splitting like `kirjakauppa → kirja + kauppa` requires a lexicon and is deferred). PHONEX-Finnish phonetic hookup (Soundex-shaped 4-char key with Finnish-tuned preprocessing: long-consonant collapse `kk`/`tt`/`ll`/`pp`/`mm`/`nn`/`ss`/`rr` → single, long-vowel collapse `aa`/`ee`/`ii`/`oo`/`uu`/`yy`/`ää`/`öö` → single, `ä → a`, `ö → o`, `å → o`; `y` treated as a vowel; adapter name `"phonex-fi"`). Overrides `Language::is_stopword` to apply Unicode case-fold (Finnish has no locale-specific quirks — unlike Turkish's dotted / dotless `I` distinction — but the default trait method uses ASCII-only case-fold which misses `Ä → ä`/`Ö → ö`/`Å → å`). Estonian sibling (`stringcheese-et`), Northern Sami pack (`stringcheese-se`), full lexicon-driven consonant-gradation reversal (`jalka` ↔ `jalan`, `käsi` ↔ `käden`), and compound-word splitting deferred. Self-registers into `stringcheese-lang::registry` as `"fi"`. |
| `stringcheese-sr` | Serbian pack (**first dual-script pack**): dual-script stopword lists (~120 entries per script, ~240 total) covering personal / possessive / demonstrative pronouns, prepositions, conjunctions, particles, high-frequency forms of the copula *biti / бити* and auxiliary *imati / имати*, and common adverbs; bijective Vukovica (Cyrillic) <-> Gaj's Latin transliteration (`љ ↔ lj`, `њ ↔ nj`, `џ ↔ dž`, `ђ ↔ đ`, `ж ↔ ž`, `ћ ↔ ć`, `ч ↔ č`, `ц ↔ c`, `ш ↔ š`, `ј ↔ j`, plus 22 single-letter pairs); Snowball-family light stemmer that **normalizes Cyrillic input to Latin** via the transliteration helper, runs a single Latin suffix table (`-ovima`, `-ijim`, `-ijem`, `-ijeg`, `-ovi`, `-ove`, `-ova`, `-ovu`, `-ovom`, `-ama`, `-ima`, `-oga`, `-ome`, `-iji`, `-ije`, `-ali`, `-alo`, `-ila`, `-ilo`, `-ati`, `-iti`, `-uti`, `-eti`, `-ost`, single-char `-a` / `-e` / `-i` / `-o` / `-u`, min-stem 3), then transliterates the stem back if the input was Cyrillic (option (a): one suffix table, no dual-table drift); whitespace-and-punctuation tokenizer that treats both scripts as word characters (every letter of both alphabets satisfies `char::is_alphanumeric`); `to_latin`-backed phonetic hookup (adapter name `"sr-latin"`) that unifies records filed under either script under a single lowercase Latin key. Ekavian vs. ijekavian handled as distinct opaque forms (`vek` and `vijek` stem to themselves; the stopword lists carry both `gde` / `gdje`, `uvek` / `uvijek` variants). Croatian / Bosnian / Montenegrin packs deferred — they share the dual-script base but diverge in vocabulary. Overrides `Language::is_stopword` to dispatch on the input's script. Self-registers into `stringcheese-lang::registry` as `"sr"`. |
| `stringcheese-ru` | Russian pack: ~170-word Cyrillic stopword list, Snowball Russian stemmer (Porter/Boulton `russian.sbl`) with `ё → е` precomputation and the four-step cascade (perfective-gerund / reflexive / adjectival-verb-noun → trailing-`и` in RV → derivational `ост`/`ость` in R2 → undouble-`нн` / superlative-`ейш` / trailing soft-sign), whitespace-and-punctuation tokenizer, GOST 7.79-2000 System B transliteration phonetic hookup (deterministic ASCII-only Cyrillic → Latin: `ж → zh`, `ч → ch`, `ш → sh`, `щ → shh`, `ц → cz`, `ъ → ''`, `ь → '`, `э → e'`; adapter name `"gost-7.79-b"`). First Cyrillic-script pack — all suffix / region / stopword arithmetic runs on `Vec<char>` because every Cyrillic scalar is 2 bytes in UTF-8 (byte offsets would silently corrupt boundaries). Overrides `Language::is_stopword` to apply Unicode case-fold plus `ё → е` before comparison. Slavic-Metaphone / Ukrainian / Belarusian packs and ISO 9 System A transliteration deferred. Self-registers into `stringcheese-lang::registry` as `"ru"`. |
| `stringcheese-uk` | Ukrainian pack: ~220-word Cyrillic stopword list, light suffix-stripping stemmer (there is no canonical Snowball Ukrainian; the module ships a single-pass longest-match stemmer over reflexive / verb / adjective / noun tables with an RV region guard, plus a trailing soft-sign strip — rather than a non-canonical Russian port that would inherit `нн`-undoublement and `ость`-derivational rules that do not apply to Ukrainian), apostrophe-aware tokenizer (preserves the ASCII `'` (U+0027) as a word-internal character in words like `сім'я`, `п'ять`, `об'єкт` where it marks a hard consonant / iotated vowel boundary), GOST 7.79-2000 System B transliteration phonetic hookup tailored to the Ukrainian letter set (`г → h`, `ґ → g` distinct — Russian collapses both to `g`; `є → ye`, `і → i`, `ї → yi`, `и → y`, `х → kh`, `щ → shch`, `ь → '`; adapter name `"gost-7.79-b-uk"`). Second Cyrillic-script pack — carries the extended Cyrillic letters `ґ` (U+0491), `є` (U+0454), `і` (U+0456), `ї` (U+0457), and does NOT carry Russian's `ъ`, `ы`, `ё`, `э`; all suffix / region / stopword arithmetic runs on `Vec<char>`. Overrides `Language::is_stopword` to apply Unicode case-fold (no `ё → е` fold — Ukrainian has no `ё`). Canonical Snowball parity, verb-aspect prefix stripping, Ukrainian government 2010 transliteration, typographic apostrophe (U+2019) recognition, and Belarusian / Serbian / Bulgarian / Macedonian packs deferred. Self-registers into `stringcheese-lang::registry` as `"uk"`. |
| `stringcheese-be` | Belarusian pack: ~85-word Cyrillic stopword list, light suffix-stripping Belarusian stemmer (there is no canonical Snowball Belarusian; the module ships a single-pass globally-longest-match stemmer over reflexive `-ся` and unified noun / adjective / verb tables with an RV region guard and a theme-vowel context guard on the past-tense endings `-ў`, `-ла`, `-ло`, `-лі` — rather than a non-canonical Russian port), apostrophe-aware tokenizer (preserves the ASCII `'` (U+0027) as a word-internal character in words like `сям'я`, `аб'ект`, `пад'езд` where it plays the role Russian's hard sign `ъ` plays), and a **PHONEX-Belarusian** Soundex-shape 4-character phonetic hookup with Belarusian-tuned preprocessing (short-u `ў → W` in the labial class alongside `в`/`б`/`п`/`ф`; digraph rewrites `дж → J`, `дз → Z` collapse a two-scalar Cyrillic digraph into a single class-7 grapheme in the key; adapter name `"phonex-be"`). Third Cyrillic-script pack — carries the Belarusian-specific letters `ў` (U+045E), `і` (U+0456), and does NOT carry Russian's `и`, `щ`, `ъ`; all suffix / region / stopword arithmetic runs on `Vec<char>` because every Cyrillic scalar is 2 bytes in UTF-8. Overrides `Language::is_stopword` to apply Unicode case-fold (no `ё → е` fold — Belarusian carries `ё` as a distinct vowel). Canonical Snowball parity, verb-aspect prefix stripping, Slavic-Metaphone alternate encoder, GOST 7.79-B transliteration adapter, Narkamaŭka / Taraškievič orthography toggle, typographic apostrophe (U+2019) recognition, and Macedonian pack deferred. Self-registers into `stringcheese-lang::registry` as `"be"`. |
| `stringcheese-bg` | Bulgarian pack: ~236-word Cyrillic stopword list, Snowball Bulgarian stemmer (Nakov 2003) with the four-step cascade specialized for Bulgarian's analytic morphology — **definite-article stripping first** (`-ият`/`-ия` masc long-adj, `-ата`/`-ото` fem/neut long-adj, `-ите` plural long-adj, `-ът`/`-ят` masc noun, `-та` fem noun, `-то` neut noun, `-те` plural noun; this signature Bulgarian step collapses `книгата → книг` and `човекът → человек` to the same forms as `книга`/`човек`, because Bulgarian's article is a postposed suffix rather than a separate word like English `the`), then plural markers (`-ове`/`-еве` for monosyllabic-root masc plurals), then verb/l-participle endings (aorist `-вах`/`-ах`/`-ох`/`-ех`, imperfect `-аше`/`-еше`/`-иеше`/`-яше`, present `-еш`/`-иш`/`-ат`/`-ят`/`-им`/`-ем`/`-ете`/`-ите`, l-participle `-л`/`-ла`/`-ло`/`-ли`/`-ъл`), then final bare-vowel strip in R1 (`а е и о у я ю ъ` — Bulgarian's vowel set, note `ъ` is a vowel /ɤ/ not a hard-sign glyph as in Russian); whitespace-and-punctuation tokenizer (transparent wrapper around `SimpleTokenizer`); GOST 7.79-2000 System B transliteration phonetic hookup tailored to Bulgarian phonology (**`щ → sht`** to reflect Bulgarian's /ʃt/ cluster where Russian's `щ` is a long /ʃː/ rendered `shh`, **`ъ → a`** because ъ is a full vowel in Bulgarian, **`х → h`** and **`ц → ts`** per Bulgarian romanization convention; adapter name `"gost-7.79-b-bg"`). Third Cyrillic-script pack — carries the 30-letter Bulgarian alphabet (drops Russian-only `ё`/`ы`/`э`; repurposes `ъ` as a vowel); all suffix / region / stopword arithmetic runs on `Vec<char>`. Overrides `Language::is_stopword` to apply Unicode case-fold. Palatal alternation reversal (`к`↔`ц`, `г`↔`з`, `х`↔`с`, stressed `я`↔unstressed `е`), Macedonian pack (`-ѓ ќ ѕ ј љ њ џ`, three-way proximal/medial/distal article system), Old Church Slavonic (`-ѣ ѫ ѧ ѩ ѭ ѱ ѳ ѵ`), Church Slavic Snowball variant, Belarusian, ISO 9 System A adapter, and Slavic-Metaphone deferred. Self-registers into `stringcheese-lang::registry` as `"bg"`. |
| `stringcheese-mk` | Macedonian pack: ~155-word Cyrillic stopword list, lightweight rule-based Macedonian stemmer (there is no canonical Snowball Macedonian; the module ships a four-step cascade shaped after the Bulgarian Snowball algorithm — Macedonian is Bulgarian's closest linguistic sibling, both analytic South Slavic languages with postposed definite articles) with **three-way definite-article stripping first** covering all twelve forms across the proximity contrast Bulgarian lacks (proximal `-ов`/`-ва`/`-во`/`-ве` "this-here", medial `-от`/`-та`/`-то`/`-те` neutral, distal `-он`/`-на`/`-но`/`-не` "that-yonder"; this signature Macedonian step collapses `градот`/`градов`/`градон` to the same stem as `град`), then plural markers (`-ови`/`-еви` for monosyllabic-root masc plurals, `-ња` for neut `-ње` derivations), then verb personal endings (`-ам`/`-аш`/`-ат`/`-ме`/`-те` present, `-ав` aorist), then final bare-vowel strip in R1 (`-а`/`-и`/`-о`/`-у` — `-е` deliberately left as-is to protect neut nouns like `дете` and the copula `е`); whitespace-and-punctuation tokenizer (transparent wrapper around `SimpleTokenizer`); PHONEX-Macedonian Soundex-shape 4-char-count phonetic hookup with the seven Macedonian-specific letters folded to their nearest Slavic-Soundex class (`ѓ`/`ќ`/`ј` → guttural class 2, `љ` → lateral class 4, `њ` → nasal class 5, `ѕ`/`џ` → sibilant class 7); the seed slot preserves the Cyrillic letter verbatim (a word starting with `Ѓ` produces a key whose first char-count-1 scalar is `ѓ`, not `г`), so `ѓавол` and `гавол` are not collapsed at the seed. Adapter name `"phonex-mk"`. Fourth Cyrillic-script pack — carries the 31-letter Macedonian alphabet with seven Macedonian-specific letters (`ѓ` U+0453, `ќ` U+045C, `љ` U+0459, `њ` U+045A, `џ` U+045F, `ѕ` U+0455, `ј` U+0458) and drops Russian's `ё`/`ы`/`э`/`ъ`/`ь`/`й`/`щ`/`ю`/`я`; all suffix / region / stopword arithmetic runs on `Vec<char>`. Overrides `Language::is_stopword` to apply Unicode case-fold. GOST 7.79-B Macedonian adapter, Slavic-Metaphone Macedonian, palatal-alternation reversal (`к`↔`ц`, `г`↔`з`, `х`↔`с`), and full-vocabulary cross-verification deferred. Self-registers into `stringcheese-lang::registry` as `"mk"`. |
| `stringcheese-cs` | Czech pack: ~275-word Czech stopword list (with proper diacritics for the extended letter set `á č ď é ě í ň ó ř š ť ú ů ý ž`), light Czech suffix-stripping stemmer — **there is no canonical Snowball Czech algorithm** — with an RV region guard and a hand-audited longest-match table covering noun / adjective / possessive endings (`-ovi`, `-ova`, `-ovy`, `-ové`, `-ami`, `-emi`, `-ám`, `-ým`, `-ého`, `-ých`, plus bare `-a` / `-e` / `-i` / `-o` / `-u` / `-y`), verb inflections (the -ovat family `-oval` / `-ovala` / `-ovalo` / `-ovali` / `-ovaly` / `-ovat` / `-uji` / `-uje` / `-uješ`, and the -at / -it / -ět families' past-tense and infinitive endings), whitespace-and-punctuation tokenizer (Czech is delimiter-clean; the `ch` digraph stays intact as two ASCII letters inside a token), and a PHONEX-Czech Soundex-shaped 4-char phonetic hookup with Czech-tuned preprocessing (haček folds `č → C`, `š → S`, `ž → Z`, `ř → R`, `ď/ť/ň → D/T/N`; long-vowel folds `á/é/í/ó/ú/ý → A/E/I/O/U/Y` and `ů → U`; `ě → E`; `ch → X` digraph; silent `h`; adapter name `"phonex-cs"` chosen for consistency with the other Latin-alphabet packs). Deliberately conservative — over-stemming Czech is easy without a lexicon (velar / palatal alternation like `ruka → ruce` is not reversed; the light stemmer strips the suffix only). Overrides `Language::is_stopword` to apply Unicode case-fold. Aggressive Dolamic-Savoy derivational stripping (`-ost`, `-ství`), consonant-alternation reversal, ISO 9-cs transliteration adapter, and Croatian / Bosnian / Montenegrin packs deferred. Self-registers into `stringcheese-lang::registry` as `"cs"`. |
| `stringcheese-sk` | Slovak pack (mutually intelligible with Czech; ~90% morphology overlap but different function-word inventory and Slovak-only letter set): ~240-word Slovak stopword list (with proper diacritics for the Slovak-specific extended letter set `á ä č ď é í ĺ ľ ň ó ô ŕ š ť ú ý ž`), light Slovak suffix-stripping stemmer — **there is no canonical Snowball Slovak algorithm** — with an RV region guard and a hand-audited longest-match table shaped after the Czech pack's but with Slovak morphology's differences encoded explicitly: infinitive suffix is `-ť` not Czech's `-t` (`-ovať`, `-ať`, `-iť`, `-ieť`, `-núť`); present tense of `-ovať` verbs follows the Slovak paradigm `-ujem` / `-uješ` / `-uje` / `-ujeme` / `-ujete` / `-ujú` (not Czech's `-uji` / `-ují`); past-tense plural is `-ovali` only (Slovak has no gender split like Czech's `-ovali` / `-ovaly`); masculine-noun instrumental singular is `-om` (Slovak) not `-em` (Czech); RV vowel set adds `ä`, `ô`, `ĺ`, `ŕ` and drops `ě`, `ů` (Slovak lacks these Czech letters). Whitespace-and-punctuation tokenizer (Slovak is delimiter-clean; the `ch` digraph stays intact as two ASCII letters inside a token). PHONEX-Slovak Soundex-shaped 4-char phonetic hookup with Slovak-tuned preprocessing (haček folds `č → C`, `š → S`, `ž → Z`, `ď/ť/ň → D/T/N`, Slovak-only `ľ → L`; long-vowel folds `á/é/í/ó/ú/ý → A/E/I/O/U/Y` and Slovak-only syllabic `ĺ → L`, `ŕ → R`; Slovak-only `ä → E` (open-front vowel phonetically closer to `e` than `a`) and `ô → O` (diphthong marker folding to base vowel); `ch → X` digraph; silent `h`; adapter name `"phonex-sk"` chosen for consistency with the other Latin-alphabet packs). Deliberately conservative — same over-stemming risk as Czech; velar / palatal alternation not reversed. Overrides `Language::is_stopword` to apply Unicode case-fold. Aggressive derivational stripping (`-osť`, `-stvo`, `-izmus`), consonant-alternation reversal, and diacritic-strip ISO 9-sk transliteration adapter deferred. Self-registers into `stringcheese-lang::registry` as `"sk"`. |
| `stringcheese-da` | Danish pack: ~120-word stopword list drawn from the Snowball project's `danish/stop.txt` (ranked head plus full paradigms of the copula `være` / auxiliary `have` / modals `kunne` / `ville` / `skulle` / `måtte` / `burde`; carries the three Danish extra letters `æ ø å` in the accented forms `på`/`være`/`også`/`må`/…), Snowball Danish stemmer per Porter/Boulton `danish.sbl` — four-step cascade: (1) main-suffix longest match over Group A plain-delete (`hed`/`ethed`/`ered`/`e`/`erede`/`ende`/`erende`/`ene`/`erne`/`ere`/`en`/`heden`/`eren`/`er`/`heder`/`erer`/`heds`/`es`/`endes`/`erendes`/`enes`/`ernes`/`eres`/`ens`/`hedens`/`erens`/`ers`/`ets`/`erets`/`et`/`eret`) plus Group B bare-`s` (deleted when preceded by a valid s-ending `a b c d f g h j k l m n o p r t v y z å` — the sbl's `s_ending` set includes vocalic `a` and the extended letter `å` deliberately); (2) consonant-pair `-gd`/`-dt`/`-gt`/`-kt` trailing-letter strip in R1; (3) other-suffix in R1: `-igst` prelude strips final `-st` (leaves `-ig`), then longest match `ig`/`lig`/`elig`/`els` → delete (then re-run Step 2) plus `løst → løs` replacement; (4) undouble a trailing repeated consonant when the doubled pair sits in R1. R1 adjusted so it never begins before char index 3. Whitespace-and-punctuation tokenizer (transparent wrapper around `SimpleTokenizer`; Danish is delimiter-clean and the extra letters `æ`/`ø`/`å` satisfy `char::is_alphanumeric`). PHONEX-Danish Soundex-shaped 4-char phonetic hookup with Danish-tuned preprocessing: cluster substitutions `sj → S` (voiceless postalveolar /ɕ/), `sk` before front vowel (`e`/`i`/`y`, plus fold-derived `E` from `æ`/`ø`) → `S`, `ch → S`, silent `h` drops; Danish-vowel folds `å → O` (open back rounded), `æ → E` (open front), `ø → E` (rounded mid front); adapter name `"phonex-da"`. Registers BCP-47 `"da"`. Métaphone Danish, compound-noun splitting (`børne + have → børnehave`), Danish-tailored collator (`æ`/`ø`/`å` sort after `z`), historical `aa → å` normalization pass, and Icelandic sibling (`stringcheese-is`) deferred. Self-registers into `stringcheese-lang::registry` as `"da"`. |
| `stringcheese-is` | Icelandic pack — **completes the Nordic quintet** alongside `stringcheese-sv` / `stringcheese-no` / `stringcheese-nn` / `stringcheese-da`. ~90-word stopword list (ranked head of Icelandic function words plus the full paradigms of the copula `vera` "to be", the auxiliary `hafa` "have", and the modals `skulu` / `vilja` / `geta` / `mega`; carries the Icelandic letters `þ ð æ ö` and the long-vowel scalars `á é í ó ú ý` in the accented forms `þú`/`það`/`í`/`á`/`ég`/`eða`/…). **Rule-based lightweight stemmer** — Icelandic has **no official Snowball algorithm** (Icelandic morphology is fusional with rich noun/adjective declension × 4 cases × sg/pl × 3 genders, strong/weak verb inflection, and a definite article that agglutinates as a suffix); this pack ships a longest-match suffix stripper with a MIN_STEM_CHARS ≥ 3 guard covering the definite-article suffix inventory (`-inum`/`-inni` masc/fem dat sg def, `-inn` masc nom sg def, `-nir` masc nom pl def, `-nar` fem/masc acc pl def, `-num` dat pl def, `-nni` fem dat sg def alt, `-nu` fem acc/dat sg def, `-ið` neut sg def, `-in` fem nom sg / neut nom pl def), the noun case inventory (`-ur` masc nom sg, `-ar` nom pl / fem gen sg, `-ir` fem/masc weak nom pl, `-um` dat pl universal, `-s` masc/neut gen sg, `-i` dat sg universal, `-a` gen pl / weak neut), verb personal endings (`-um` 1pl, `-uð` 2pl archaic past, `-ir` 2sg, `-ið` 2pl, `-a` inf/3pl), and adjective agreement (`-ur`/`-ir`/`-um`/`-a`/`-t` weak/strong). The internal loop iterates to convergence so `hesturinn → hestur → hest` completes in a single external call. Whitespace-and-punctuation tokenizer (transparent wrapper around `SimpleTokenizer`; the four Icelandic-specific letters `þ`/`ð`/`æ`/`ö` and the six long-vowel scalars all satisfy `char::is_alphanumeric`). PHONEX-Icelandic Soundex-shaped 4-char phonetic hookup with Icelandic-tuned preprocessing: letter-to-digraph rewrites `þ → th` (voiceless dental fricative /θ/), `ð → dh` (voiced dental fricative /ð/), `æ → ae` (front-open diphthong), `ö → oe` (rounded-mid front vowel); cluster rewrite `hv → kv` (the Modern Icelandic hv-/kv- merger — historical /hw/ pronounced /kʰv/); silent `h` word-initial and word-interior; long-vowel accent folds `á/é/í/ó/ú/ý → A/E/I/O/U/Y`; adapter name `"phonex-is"`. All suffix/preprocessing arithmetic runs on `Vec<char>` because every Icelandic-specific scalar is multi-byte in UTF-8 (byte offsets would silently corrupt boundaries). Registers BCP-47 `"is"`. Métaphone Icelandic, lexicon-backed lemmatization (needed to collapse u-umlaut alternations like `hafa` / `höfum` to a single form), compound-noun splitting (`bókasafn = bóka + safn`), preaspiration encoding, `ll`/`nn` fortition after long vowels in the phonetic encoder, and an Icelandic-tailored collator (traditional order runs `... x y z þ æ ö`) all deferred. Self-registers into `stringcheese-lang::registry` as `"is"`. |
| `stringcheese-sv` | Swedish pack: ~140-word stopword list (ranked head plus paradigms of `vara`/`ha`/`bli`/`kunna`/`skola`/`vilja`/`måste`/`göra`, all three Swedish extras `å ä ö` represented in the accented forms `är`/`så`/`där`/`här`/`över`/…), Snowball Swedish stemmer (Porter/Boulton `swedish.sbl`) with the German-style R1 adjusted-to-≥3 region and the three-step cascade (main-suffix longest match over the 36-entry unconditional-delete group `a`/`arna`/`erna`/`heterna`/`orna`/`ad`/`e`/`ade`/`ande`/`arne`/`are`/`aste`/`en`/`anden`/`aren`/`heten`/`ern`/`ar`/`er`/`heter`/`or`/`as`/`arnas`/`ernas`/`ornas`/`es`/`ades`/`andes`/`ens`/`arens`/`hetens`/`erns`/`at`/`andet`/`het`/`ast` plus conditional `s` (with the sbl's 16-char valid-s-ending set `bcdfghjklmnoprtvy` OR the `ets` sub-form gated by the et-condition) and conditional `et` (gated by the et-condition and its 21-entry exclusion list `h`/`iet`/`uit`/`fab`/`cit`/`dit`/`alit`/`ilit`/`mit`/`nit`/`pit`/`rit`/`sit`/`tit`/`ivit`/`kvit`/`xit`/`kom`/`rak`/`pak`/`stak` that protects `paket`/`alfabet`/`raket`/`societet`/…), consonant-pair reduction on `dd`/`gd`/`nn`/`dt`/`gt`/`kt`/`tt` in R1, and other-suffix longest match over `lig`/`ig`/`els` delete plus `öst → ös` with ost-ending guard `iklnprtuv` plus `fullt → full` replacement), whitespace-and-punctuation tokenizer (transparent wrapper around `SimpleTokenizer`; the three Swedish-specific letters stay word-internal), PHONEX-Swedish phonetic hookup (Soundex-shaped 4-char key with Swedish-tuned preprocessing: sj-family cluster fold `sj → S`, `stj → S`, `skj → S`, `sch → S`, `sk` before front vowels `e i y` → `S`; tj-family palatal fold `tj → C`, `kj → C`, and `k` before front vowels → `C`; `ch → S`; vowel folds `å → o`, `ä → e`, `ö → e`; adapter name `"phonex-sv"`). Sverigesvenska ("Sweden Swedish") as the default with Finland-Swedish specialization deferred; Norwegian and Danish sibling packs deferred to their own `stringcheese-no` / `stringcheese-da` crates. Self-registers into `stringcheese-lang::registry` as `"sv"`. |
| `stringcheese-hu` | Hungarian pack (**Uralic** — non-Indo-European, related to Finnish / Estonian, first Uralic pack in the workspace): ~180-word stopword list (with proper diacritics for the long / umlaut vowel inventory `á é í ó ö ő ú ü ű`), Snowball Hungarian stemmer (per <https://snowballstem.org/algorithms/hungarian/stemmer.html>) implemented as an iterated longest-match strip over a unified surface-form table that merges the reference algorithm's instrumental / case / owned / owner / plural / verb-suffix steps into a single pass (rationale: the phased approach can over-strip a shorter cross-category match; the unified longest-match resolves the ambiguity uniformly), with an R1 region guard (position after the first vowel-then-consonant transition) and a 2-character min-stem floor — every case-ending, plural, and possessive suffix is listed in each of its harmony variants (`-ban`/`-ben` inessive, `-ba`/`-be` illative, `-ra`/`-re` sublative, `-nak`/`-nek` dative, `-nál`/`-nél` adessive, `-ból`/`-ből` elative, `-ról`/`-ről` delative, `-tól`/`-től` ablative, `-hoz`/`-hez`/`-höz` allative triplet, `-vá`/`-vé` translative, `-val`/`-vel` instrumental with the sixteen doubled-consonant assimilation variants, `-ért` causal-final, `-ig` terminative, `-kor` temporal, `-ként` essive-formal, `-ul`/`-ül` essive-modal, `-t`/`-at`/`-et`/`-ot`/`-öt` accusative), whitespace-and-punctuation tokenizer (Hungarian is delimiter-clean; every ASCII scalar of every Hungarian digraph is alphabetic so `cs`, `sz`, `zs`, `gy`, `ny`, `ty`, `ly`, `dz`, `dzs` all stay inside tokens), PHONEX-Hungarian Soundex-shaped 4-char phonetic hookup with Hungarian-tuned preprocessing (long-vowel folds `á/é/í/ó/ú → A/E/I/O/U` and umlaut/rounded folds `ö/ő → O` and `ü/ű → U`; digraph rewrites `cs → C`, `sz → S`, `zs → Z'`, `gy → G'`, `ny → N'`, `ty → T'`, `ly → J`, `dz → Z`, and trigraph `dzs → J` — primed placeholders use an ASCII apostrophe joiner that the encoder treats as transparent, so a primed pair encodes as its base letter's class once; silent `h` dropped in preprocess; adapter name `"phonex-hu"` chosen for consistency with the other Latin-alphabet packs). **Vowel harmony encoded in the suffix table**, not as a runtime predicate — every surface variant of every suffix is its own literal entry, so the stemmer's runtime job is a longest-match search over concrete surface forms (same design choice as the Turkish pack, minus Turkish's runtime harmony-class check). Overrides `Language::is_stopword` to apply Unicode case-fold. Verb-conjugation lemmatization, definite/indefinite conjugation awareness, compound-word decomposition, post-strip vowel-length restoration, Hungarian-tailored CLDR collator, and full-corpus cross-verification against Snowball's `voc.txt` / `output.txt` deferred to a follow-up wave. Self-registers into `stringcheese-lang::registry` as `"hu"`. |
| `stringcheese-vi` | Vietnamese pack: ~180-syllable stopword list (Vietnamese orthography writes every syllable as a whitespace-separated word so entries are single-syllable — multi-syllable compounds like `chúng tôi` are covered by their component syllables), configurable Vietnamese normalizer (NFC canonicalization by default; opt-in `with_strip_tone_marks(true)` removes the five tone marks — grave `à` / acute `á` / hook-above `ả` / tilde `ã` / dot-below `ạ` — while **preserving letter modifiers** `ă â đ ê ô ơ ư`; opt-in `with_strip_all_diacritics(true)` folds every diacritic to plain ASCII, including the `đ → d` fold that has no NFD decomposition), identity-style "stemmer" (Vietnamese is analytic — no inflection to strip — so the `Language::stem` slot is filled by an NFC canonicalizer via `unicode_normalization::is_nfc` fast-path that returns `Cow::Borrowed` on already-NFC input), whitespace-and-punctuation tokenizer (Vietnamese is space-delimited — unlike Chinese / Japanese / Thai — so the `SimpleTokenizer` wrapper suffices; multi-syllable compound joining is deferred to a future dictionary-backed pack), PHONEX-Vietnamese phonetic hookup (Soundex-shaped 4-char key over the diacritic-stripped ASCII form with Vietnamese-tuned digraph rewrites `ng → N`, `nh → N`, `ph → F`, `kh → K`, `tr → T`, `ch → X`, `qu → K`, `gi → Y`, `gh → G`, and silent `H`; adapter name `"phonex-vi"`). Design choices tied to Vietnamese linguistics: (1) NFC as the default composition — the web overwhelmingly delivers Vietnamese in NFC and every Vietnamese input method (Telex, VNI, VIQR) produces NFC output; (2) tone marks and letter modifiers as **linguistically distinct** categories — letter modifiers change the segmental phoneme (`a`/`ă`/`â` are different vowels; `d`/`đ` are different consonants) and letter-modified vowels can carry a tone mark on top of the modifier (`ằ` = ă + grave, three scalars in NFD); (3) analytic morphology — Vietnamese verbs / nouns / adjectives do not inflect for tense / number / case / gender / person, so `Language::stem` is a canonicalizer, not a suffix stripper. Overrides `Language::is_stopword` to apply Unicode case-fold (`ă → Ă`, `đ → Đ`, `ệ → Ệ` etc). Multi-syllable word segmentation, compound-word lemmatization, regional-variant handling (Northern / Central / Southern dialect PHONEX rules), Métaphone-Vietnamese, and Vietnamese-tailored collator deferred. Self-registers into `stringcheese-lang::registry` as `"vi"`. |
| `stringcheese-th` | Thai pack (**first Thai-script pack**): ~55-word Thai-script stopword list (pronouns, demonstratives, interrogatives, conjunctions, prepositions, copula/auxiliary/negator/TAM markers, motion light verbs, sentence-final particles, quantifiers); near-identity Thai stemmer (Thai is fully **analytic** — no case, no plural, no verb-tense inflection — so the stemmer only strips a small closed set of nominalizer / agent prefixes `การ- ความ- ผู้- นัก- เครื่อง-` with a 2-scalar min-stem guard and folds exact word-level reduplication `XX → X`; everything else passes through as identity, matching the design of `stringcheese-zh`'s identity stemmer); syllable-cluster tokenizer (Thai is written **without spaces between words** — like Chinese/Japanese, unlike Vietnamese — so **true word segmentation requires a dictionary + statistical / neural model** (ICU's Thai break iterator, PyThaiNLP's `newmm`, `attacut`, `deepcut`) and is **deferred** to a future `stringcheese-th-newmm` sibling; the base pack ships a naive syllable-cluster segmenter that emits a cluster per leading consonant + attached vowel / tone / sign marks — coarser than word-level (`ไทย` splits as `ไท` + `ย` because the naive rule ends the cluster at the second consonant), but deterministic, dictionary-free, and dramatically cheaper); PHONEX-Thai phonetic hookup — a two-step algorithm that drops every non-consonant scalar (tone marks, vowel marks, pre-vowels, signs) and maps each Thai consonant to a Royal-Thai-Romanization-family letter (all velars `ก ข ฃ ค ฅ ฆ → K` collapsing high/low class historical distinctions to the modern /k/-/kʰ/ phone; all sibilants `ซ ศ ษ ส → S`; all labials `ผ พ ภ ป → P`; all dentals `ต ถ ท ธ ฐ ฑ ฒ ฏ → T`; nasals `ง ณ น → N`; liquid `ร → R`, `ล ฬ → L`; glide `ญ ย → Y`; glottal `ห ฮ → H` (dropped as class 0); zero-onset `อ → A` (dropped)) then runs the standard Soundex-shape 4-character reduction (class digits `B P F V W = 1`, `C K G Q J X = 2`, `D T = 3`, `L = 4`, `M N = 5`, `R = 6`, `S Z = 7`, vowels + H = 0 dropped, consecutive equal codes collapse, pad to 4 with `'0'`); adapter name `"phonex-th"`. **First Thai-script pack** — every Thai scalar (U+0E00..=U+0E7F) is **3 bytes in UTF-8** (the block falls in UTF-8's 3-byte range U+0800..=U+FFFF, same as Devanagari / Bengali / Hangul), so all tokenizer / stemmer / phonex arithmetic runs on `Vec<char>` or `str::chars` iteration — never raw byte offsets — because byte arithmetic would silently corrupt scalar boundaries; the tokenizer specifically models Thai's pre-vowel convention (five leading vowels `เ แ โ ใ ไ` U+0E40..=U+0E44 are typed **before** the consonant they modify in Unicode order) so a cluster like `เป็น` is captured as pre-vowel + consonant + vowel-mark rather than five isolated scalars. Dictionary-driven word segmentation (`stringcheese-th-newmm` or `stringcheese-th-dict`), tone-preserving encoder, RTGS-faithful romanization (`stringcheese-th-rtgs`), ISO 11940 scholarly transliteration, sara-am decomposition, and Khmer / Lao / Burmese Southeast-Asian-script siblings all deferred. Self-registers into `stringcheese-lang::registry` as `"th"`. |
| `stringcheese-id` | Indonesian (Bahasa Indonesia) pack (**first Malayo-Polynesian / Austronesian pack in the workspace** — every prior pack is Indo-European, Sino-Tibetan, Japonic, Koreanic, Semitic, Uralic, Turkic, or Austroasiatic): ~90-word ASCII stopword list (coordinating / subordinating conjunctions, prepositions, personal / demonstrative / interrogative pronouns, copular / existential / auxiliary verbs, common adverbs, negations, numerals up to `sepuluh`), simplified **Nazief-Adriani stemmer** (Bobby Nazief & Mirna Adriani, Universitas Indonesia 1996 — the canonical Indonesian IR stemmer reference; the shipped variant ships the algorithm's rule structure **without** the reference algorithm's root-word dictionary lookup, calibrated to over-stem rarely rather than under-stem) implemented as a five-step ordered cascade — stopword short-circuit → particle suffix (`-lah` imperative, `-kah` interrogative, `-tah` rhetorical, `-pun` concessive) → possessive suffix (`-ku` 1sg, `-mu` 2sg, `-nya` 3sg/definite) → derivational suffix (`-kan` causative/benefactive, `-an` nominalizer, `-i` locative/applicative with a "consonant-before-i" guard) → derivational prefix with `me-`/`pe-` consonant restoration reversing the nasal-assimilation rules (`mem-`+vowel restore `p` for `memilih` → `pilih`; `men-`+vowel restore `t` for `menulis` → `tulis`; `meny-`+vowel restore `s` for `menyapu` → `sapu`; `meng-` ambiguous — no-elision reading wins under the shipped rules so `mengambil` → `ambil` and `mengirim` → `irim` are the outcomes; `me-`+sonorant strips bare so `melihat` → `lihat`; parallel `pe-` allomorphs for the agent nominalizer so `penulis` → `tulis`; special `bel-` allomorph on `belajar` → `ajar`). Confix-inhibition rules protect against known over-strips: `ber-`/`di-`/`ter-` + `-an` refuses the `-an` strip (so `berjalan` → `jalan`, not `jal`); possessive already stripped in step 3 refuses the `-an` strip (so `tanganku` → `tangan`, not `tang`); `me-`/`pe-`/`di-`/`ber-`/`ter-` prefix refuses the `-i` strip (so `menari` → `tari`, not `tar`; `berlari` → `lari`, not `lar`); commit-on-shape rule for the 3-letter `ber-`/`per-`/`ter-`/`bel-` prefixes prevents the bare `me-`/`pe-`+sonorant handler from firing on words like `pergi` where the 3-letter prefix's shape is present but its residue is too short. 3-character minimum-stem floor on every strip. Simple ASCII Latin tokenizer (Indonesian uses the modern 26-letter Latin alphabet with **no diacritics** — every scalar in an Indonesian word is ASCII-alphabetic, so the default `SimpleTokenizer` wrapper suffices; reduplication like `buku-buku` "books" splits at the hyphen and the two halves stem to `buku` independently). PHONEX-Indonesian Soundex-shape 4-char phonetic hookup with Indonesian-tuned digraph rewrites (`ny → N` /ɲ/ nasal, `ng → G` /ŋ/ — folds to `G` class 2 rather than `N` class 5 to preserve the `bunga` vs. `bunda` distinction, `sy → S` /ʃ/ sibilant, `kh → K` /x/ velar; silent `H` dropped after the digraph pass so `hotel` → `OTEL`; adapter name `"phonex-id"`). Uses the default `Language::is_stopword` (ASCII case fold suffices since Indonesian's orthography is entirely inside `[a-zA-Z]`); default Unicode collation (Indonesian sorts under the plain Latin order — no locale tailoring required). Malaysian Malay sibling (`stringcheese-ms` — the two languages share ~80 % of core vocabulary and identical morphology; the algorithm and phonetic encoder would carry over unchanged, only the stopword list would differ), dictionary-backed root confirmation via a Sastrawi-style 30 000-word lexicon, Métaphone-shaped variable-length phonetic encoder, colloquial / SMS-register stopword additions (`gw` / `lu` / `bgt`), and reduplication canonicalization all deferred. Self-registers into `stringcheese-lang::registry` as `"id"`. |
| `stringcheese-hy` | Armenian (Eastern Armenian, Republic of Armenia standard) pack (**first Armenian-script pack**; **Indo-European isolate branch** — a family-of-one within IE, in the same typological company as `stringcheese-el` (Greek) and Albanian): ~55-entry lowercase Armenian-script stopword list (pronouns, three-way demonstratives `այս`/`այդ`/`այն`, interrogatives, conjunctions including the ligature-spelled `և` and the two-letter `եւ`, prepositions, copula `եմ`/`ես`/`է`/`ենք`/`եք`/`են`, negator `չէ`/`չեմ`/`չես`/`չենք`/`չեք`/`չեն`, high-frequency adverbs / quantifiers); hand-audited longest-match suffix stripper stemmer (Armenian has no widely-published Snowball algorithm; the shipped stemmer iterates to convergence over a curated table covering all seven Eastern Armenian singular case suffixes — genitive `-ի`, dative `-ին`, ablative `-ից`, instrumental `-ով`, locative `-ում` (also the imperfective-participle marker), postposed definite article `-ը` after consonant / `-ն` after vowel — the two plural markers `-եր` (monosyllabic base) / `-ներ` (polysyllabic base), their plural + case combinations `-ների`/`-ներով`/`-ներում`/`-ներից`/`-ներին`/`-երի`/`-երով`/`-երում`/`-երից`/`-երին`, and the aorist personal endings `-եցի` 1sg / `-եցիր` 2sg / `-եց` 3sg / `-եցինք` 1pl / `-եցիք` 2pl / `-եցին` 3pl; 2-scalar min-stem guard on every strip prevents over-stripping of short base words); whitespace-and-punctuation tokenizer (transparent wrapper around `SimpleTokenizer` — Armenian letters satisfy `is_alphanumeric` and stay inside tokens; Armenian punctuation `։` U+0589 full stop, `՝` U+055D comma, `՞` U+055E question mark, `՜` U+055C exclamation mark, and `֊` U+058A hyphen are all Unicode `Po`/`Pd` and split under the default rule); PHONEX-Armenian phonetic hookup — a two-stage encoder that first folds each Armenian scalar to a Hübschmann-Meillet-family Latin letter (aspiration-collapsing consonant folds: labial stops `պ / փ / բ → P`, dental stops `տ / թ / դ → T`, velar stops `կ / ք / գ → K`, dental affricates `ծ / ց / ձ → C`, palato-alveolar affricates `ճ / չ / ջ → J`, velar fricatives `խ / ղ → X`, sibilants `զ / ժ → Z` and `ս / շ → S`, nasals `մ → M` / `ն → N`, liquids `լ → L` and `ր / ռ → R`, fricatives `վ → V` / `ֆ → F` / `հ → H`, glide `յ → Y`, classical `ւ → V`; vowels `ա → A`, `ե/է/ը → E`, `ի → I`, `ո/օ → O`; the two-scalar digraph `ու → U` (Armenian writes /u/ as `o + w`); the ligature `և → EV` (its two component sounds `y + ev`)) then runs the standard Soundex-shape 4-character reduction (class digits `B P F V W = 1`, `C K G Q J X = 2`, `D T = 3`, `L = 4`, `M N = 5`, `R = 6`, `S Z = 7`, vowels + `H` = 0 dropped, consecutive equal codes collapse, pad to 4 with `'0'`); adapter name `"phonex-hy"`. **First Armenian-script pack** — carries the full 39-letter Armenian alphabet (`Ա-Ֆ` uppercase and `ա-ֆ` lowercase, plus the ligature `և`) which sits inside U+0530..=U+058F; every Armenian scalar is 2 bytes in UTF-8, so all suffix / stopword / phonex arithmetic runs on `Vec<char>` or `str::chars` iteration — never raw byte offsets — because byte arithmetic would silently corrupt scalar boundaries. Rust's default `to_lowercase` fold handles Armenian's case pairs correctly (no locale-specific quirks — Armenian is not Turkish). The two-letter `եւ` and the single-scalar ligature `և` are normalized to a single form at every entry point (stopword lookup, stemmer, phonex) so both spellings stem / tokenize / encode identically. Overrides `Language::is_stopword` to apply Unicode case-fold and the `եւ → և` normalization before comparison. Western Armenian (`stringcheese-hyw` — distinct phonology: Western reads `բ` as /pʰ/ where Eastern reads it as /b/; distinct present-tense verb morphology using `կը` + finite form), Classical Armenian / Grabar (`stringcheese-xcl` — 5th-century literary language with 7 cases + distinct sg/pl forms + aorist / imperfect / perfect distinction + case-inflecting participles), lexicon-driven lemmatization, and ISO 9985 / BGN-PCGN scholarly-transliteration adapters all deferred. Self-registers into `stringcheese-lang::registry` as `"hy"`. |
| `stringcheese-el` | Greek pack (**first Greek-script pack**): ~220-entry Greek stopword list (accent-stripped, non-final-sigma form), Snowball-family Greek stemmer modeled after Ntais (2006) with a preprocessing pass that folds monotonic accents (`ά → α`, `έ → ε`, `ή → η`, `ί → ι`, `ό → ο`, `ύ → υ`, `ώ → ω`, plus dialytika `ϊ → ι` / `ϋ → υ`) and folds the positional final sigma (`ς → σ`) — followed by a three-step suffix cascade (long-compound step for superlative `-οτεροσ`/`-οτατοσ`, comparative, passive participle `-μενοσ`, abstract-noun `-οτητα`, passive aorist `-θηκαμε`/`-τηκαμε`, mediopassive `-ιονται`/`-ιομαστε`; medium step for the nominal / adjectival case endings `-οσ`, `-ου`, `-ον`, `-οι`, `-ων`, `-ουσ`, `-ασ`, `-ησ`, `-εσ`, `-ια`, and common verb endings `-ουμε`, `-ετε`, `-ουν`, `-εται`, `-ονται`, `-ουσα`, `-αμε`, `-ατε`; bare final vowel `-α`/`-ε`/`-η`/`-ι`/`-ο`/`-υ`/`-ω`) with a min-stem-length-3 guard on every strip; whitespace-and-punctuation tokenizer (transparent wrapper around `SimpleTokenizer`); ISO 843 Type 1 Greek → Latin transliteration phonetic hookup (24-letter base mapping — `α→a`, `β→v`, `γ→g`, `δ→d`, `ε→e`, `ζ→z`, `η→i`, `θ→th`, `ι→i`, `κ→k`, `λ→l`, `μ→m`, `ν→n`, `ξ→x`, `ο→o`, `π→p`, `ρ→r`, `σ/ς→s`, `τ→t`, `υ→y`, `φ→f`, `χ→ch`, `ψ→ps`, `ω→o` — plus the one context-sensitive `γγ → ng` double-gamma nasalization; diphthongs `αι`/`ει`/`οι` fall out letter-by-letter as `ai`/`ei`/`oi`; accented and diaeresis vowels fold to their base letter before lookup; adapter name `"iso-843-el"`). First Greek-script pack — carries the 24-letter Modern Greek alphabet plus the 7 accented forms and the two dialytika forms; all suffix / region / stopword arithmetic runs on `Vec<char>` because every Greek scalar in U+0370..=U+03FF is 2 bytes in UTF-8 (byte offsets would silently corrupt boundaries). Sigma's positional variants (`σ` non-final vs. `ς` final) are handled by folding `ς → σ` at every entry point (stemmer, phonetic encoder, stopword lookup) — the stopword list stores non-final `σ` and queries with either form match. Overrides `Language::is_stopword` to apply Unicode case-fold, accent-fold, and final-sigma-fold before comparison. Ancient Greek (`stringcheese-grc` — polytonic accents, richer morphology), Katharevousa (archaic Modern Greek), polytonic Modern Greek normalization, Coptic sibling (`stringcheese-cop`), ELOT 743 transliteration alongside ISO 843, and full canonical Snowball parity all deferred. Self-registers into `stringcheese-lang::registry` as `"el"`. |
| `stringcheese-ro` | Romanian pack (**first Balkan Romance pack** — Romanian is genealogically Romance (sibling of Spanish / French / Portuguese / Italian, descended from Vulgar Latin) but geographically Balkan, having spent centuries inside the **Balkan Sprachbund** alongside Bulgarian / Macedonian / Albanian / Greek and picked up several signature Balkan features its Romance cousins lack): ~130-word stopword list (free-standing articles, prepositions, coordinating / subordinating conjunctions, personal / possessive / demonstrative pronouns, negation particles, and the high-frequency conjugations of `a fi` "to be", `a avea` "to have", `a face` "to do/make"; postposed definite articles are NOT listed here — they are morphological endings handled by the stemmer's step 0, not free words to filter), Snowball Romanian stemmer per Porter's `romanian.sbl` — five-step cascade: (1) preprocess (lowercase + Unicode-aware) with an **entry-point cedilla-to-comma-below fold** (`ş → ș` U+015F → U+0219, `ţ → ț` U+0163 → U+021B) so a corpus authored on older systems that emit cedilla still aligns with modern comma-below-form queries; (2) glide-marking pass (i / u between two vowels marked as consonantal `I`/`U` for region computation, reverted in the postlude); (3) region computation (`R1`/`R2` standard Snowball, Romanian-specific `RV` matching the sbl spec — after 2nd letter if consonant, after next consonant if first two are vowels, position 3 otherwise); (4) **step 0 — postposed article strip in R1** (the signature Balkan feature — Romanian writes definite articles as noun-suffixes rather than free words: `omul` = "the man" collapses to `om`, `omului` = "of/to the man" collapses to `om`; longest-match table `'ul' 'ului'` → delete, `'aua'` → `-a`, `'ea' 'ele' 'elor'` → `-e`, `'ii' 'iua' 'iei' 'iile' 'iilor' 'ilor'` → `-i`, `'atei'` → `-ație`, `'ație' 'ația'` → `-ați`); (5) step 1 — standard suffix replacement in R1 (`'abilitate' 'abilitati' 'abilităţi'` → `abil`, `'ibilitate'` → `ibil`, `'ivitate' 'ivitati' 'ivităţi'` → `iv`, `'icitate' 'icator' 'icatoare' 'icatori'` → `ic`, `'ativ' 'ativa' 'ative' 'ativi' 'ativă'` → `at` — iterated to fix-point per the sbl's `do repeat` clause); step 2 — combining suffix delete in R2 (`abil`/`ibil`/`iv`/`ic`/`at`/`it`/`ut`/`ant`/`ător`/`ătoare`/`ători`/`are`/`ere`/`ire`/`inţ`/`ism`/`ist` family); step 3 — verb personal endings in RV (four-conjugation-class paradigm covering the `-ăm`/`-ați` present, `-esc`/`-ești`/`-ește` `-i`-class present, `-am`/`-ai`/`-au` imperfect, `-eam`/`-eați` `-ea`-class, `-âm`/`-âi`/`-ând` `-â` class, aorist `-arăm`/`-arăţi`, pluperfect `-esem`/`-eseși`/`-eseră`, gerund `-ind`/`-ând`); step 4 — final vowel drop in RV (`a`/`e`/`i`/`o`/`u`/`ă` — `â`/`î` deliberately preserved because they mark historical central-vowel `/ɨ/` and rarely appear word-finally); postlude — fold glide markers `I`/`U` back to `i`/`u`. Whitespace-and-punctuation tokenizer (Romanian is delimiter-clean; the five Romanian-specific diacritic letters `ă â î ș ț` (plus the two legacy cedilla forms `ş ţ`) all satisfy `char::is_alphanumeric` and stay word-internal; the ASCII hyphen is a separator, so enclitic pronoun forms like `dă-mi` "give me" and auxiliary contractions like `l-am` "I-have-him" split at the hyphen — the intended behavior for IR because the clitics are stopwords anyway). PHONEX-Romanian Soundex-shape 4-character phonetic hookup with Romanian-tuned preprocessing: cedilla-to-comma-below fold (`ş`/`ţ` → `ș`/`ț`), diacritic folds (`ă`/`â` → `A`, `î` → `I`, `ș` → `S`, `ț` → `T`), Romance-family digraph rewrites (`CH` before front vowel `E`/`I` → `K` for hard `/k/` — Romanian's `chibrit` "match" spelling convention; `GH` → `G` for hard `/g/` — `ghid` "guide"; `PH` → `F` for imports; `TZ` → `T` for legacy transliteration of `ț`), silent intervocalic `H` (kept word-initially so `Horia` seeds with `H` but `Mihai` drops the H), and the standard Soundex classification (`B P F V W = 1`, `C K G Q J X = 2`, `D T = 3`, `L = 4`, `M N = 5`, `R = 6`, `S Z = 7`, vowels + `H` + `Y` = 0 dropped); adapter name `"phonex-ro"`. First Balkan Romance pack — the shared postposed-definite-article feature makes Romanian's morphology look more like Bulgarian's / Macedonian's (`книгата` "the book" in Bulgarian, `cartea` "the book" in Romanian — both suffix the article) than its Romance cousins' (`el libro` / `le livre` / `il libro`), so the shipped stemmer's step 0 is more elaborate than the Spanish / French / Portuguese packs' step 0 (which handle enclitic pronouns, not articles). Retains Latin case marking (gen/dat `-lui` masc singular, `-i`/`-ii` fem singular, `-lor` genitive plural) — every other Romance language lost the Latin case system. Overrides `Language::is_stopword` to fold cedilla → comma-below before comparison so `şi` (cedilla) and `și` (comma-below) both match the stopword list's canonical comma-below-form entry. Registers BCP-47 `"ro"`; `ro-MD` (Moldovan-Latin) falls back to `ro` automatically through the registry's BCP-47 subtag walk. Cyrillic Moldovan (pre-1989 orthography), Métaphone Român, `ci`/`ce`/`gi`/`ge` context-sensitive `/tʃ/` / `/dʒ/` encoding, palatal-alternation reversal (`obosesc` → `obos` currently over-stems), CLDR-tailored Romanian collator (`... a ă â b … i î … s ș t ț … z`), and full-vocabulary cross-verification against Snowball's `voc.txt` / `output.txt` deferred to a follow-up wave. Self-registers into `stringcheese-lang::registry` as `"ro"`. |
| `stringcheese-<lang>` | Additional language-specific implementations (planned; one opt-in crate per language) |
| `stringcheese-tokenizer` | Tokenizer/segmenter trait crate + built-in tokenizers (whitespace, delimiter, identifier, grapheme, n-gram). Foundation for downstream subword algorithm crates and model packs. See [docs/design/tokenizers.md](./design/tokenizers.md). |
| `stringcheese-tokenizer-bpe` | Data-neutral Byte-Pair Encoding (Sennrich, Haddow, Birch 2016) algorithm crate — caller supplies merge table and vocabulary. Substrate for the `stringcheese-tokenizer-tiktoken` model pack (shipped) and the planned `-huggingface` pack. |
| `stringcheese-tokenizer-tiktoken` | OpenAI tiktoken model tokenizer pack — `cl100k_base` (default feature), `p50k_base`, `r50k_base`, `o200k_base` shipped as SCUD-lite BPE data on top of `stringcheese-tokenizer-bpe`. Each variant behind its own Cargo feature; lazy-decode via `OnceLock`. Real OpenAI `mergeable_ranks` blobs are not committed for licence + repo-bloat reasons; the crate's `build.rs` synthesises a small stand-in tokenizer per variant and transcodes contributor-supplied plaintext blobs from `data/<variant>.tiktoken` into SCUD-lite deflate when present. See [docs/design/tokenizers.md § 6](./design/tokenizers.md#6-tiktoken-pack--stringcheese-tokenizer-tiktoken). |
| `stringcheese-tokenizer-*` | Additional subword-tokenizer algorithm crates (planned: `-wordpiece`, `-sentencepiece`) and pre-configured model packs (planned: `-huggingface`) |
| `stringcheese-icu-*` | WIT interfaces + SCUD data packs for i18n (planned) |

The scope boundary is a coherent commitment, not a fence against
convenience. Utilities that drift outside these charters belong in
downstream libraries. Utilities that would sit awkwardly across two
sub-projects (e.g., "manip needs to know something a lang pack knows")
are handled through explicit dependency edges, not by expanding a
crate's scope.

## Philosophy

Most comparison algorithms have existed for decades.

The innovation is not implementing Levenshtein yet again.

The innovation is providing:

- complete coverage
- excellent engineering
- consistent APIs
- explicit semantics
- performance transparency
- reusable infrastructure
- language awareness
- WebAssembly-first implementation

The library should become the canonical Rust toolkit for sequence comparison.

## Design Principles

### Preserve Semantics

The API should never erase semantic differences simply to create a uniform
interface.

- Distance is not similarity.
- Similarity is not probability.
- Scores are not metrics.
- Metric properties matter.
- Normalization policies matter.

Everything should remain explicit.

### Performance Is a Feature

Performance includes:

- runtime
- memory usage
- allocation count
- peak memory
- binary size
- WebAssembly footprint
- cache locality
- SIMD utilization

### WebAssembly First

The library is intended to be a core component within WasmOS, DuckLink,
SQLink, and future Tegmentum projects.

Every design decision should consider:

- browser
- WASI
- Component Model
- embedded
- no_std

## Architecture

The umbrella is a set of coordinated Rust crates in one workspace,
plus a WIT component-model surface and (planned) opt-in language and
i18n data packs.

```
stringcheese/                       — the workspace root
├── crates/
│   ├── stringcheese                — facade (re-exports every sub-project)
│   ├── stringcheese-core           — traits, result types, descriptors,
│   │                                 workspace/sequence abstractions
│   ├── stringcheese-corpus         — golden-case schema, oracle framework
│   │
│   ├── stringcheese-compare        — comparison kernels (edit distance,
│   │                                 similarity, n-gram, MinHash, search)
│   │     src/
│   │       ├── levenshtein/        — module per algorithm family
│   │       ├── hamming/            — (was 9 sibling crates before consolidation)
│   │       ├── jaro/
│   │       ├── damerau/
│   │       ├── lcs/
│   │       ├── ngram/
│   │       ├── search/
│   │       ├── set_similarity/
│   │       └── minhash/
│   │
│   ├── stringcheese-align          — pairwise alignment (NW, SW, edit scripts)
│   ├── stringcheese-manip          — inspect/trim/case/split/…/pipeline
│   │                                 (scaffold in v0.1; populates in
│   │                                 subsequent releases)
│   │
│   ├── stringcheese-unicode        — normalization, case folding, graphemes
│   ├── stringcheese-phonetic       — Soundex, NYSIIS, Double Metaphone
│   ├── stringcheese-cdc            — rolling-hash + FastCDC chunking
│   ├── stringcheese-index          — BK-tree, VP-tree, q-gram inverted
│   │
│   └── stringcheese-bench          — criterion + allocation-counting harness
│
├── component/                      — WebAssembly Component Model surface
│   ├── wit/stringcheese.wit        — interface definition
│   └── rust-host/                  — reference host binding
│
├── fuzz/                           — cargo-fuzz differential + axiom targets
├── bench-adapters/                 — head-to-head vs strsim, rapidfuzz, …
└── docs/                           — design docs, references, publish runbook

# Planned, not shipped in v0.1
crates/
  ├── stringcheese-en, -de, -fr, -ja, …   — one crate per supported language
  └── stringcheese-icu-*                    — WIT interfaces for i18n

data/
  └── *.scud                                — compressed CLDR-derived data
                                              packs (StringCheese Unicode Data)
```

Sub-projects depend upward, not sideways. `stringcheese-manip` uses
`stringcheese-unicode` and (for `find`/`replace`) `stringcheese-compare`;
`stringcheese-index` uses `stringcheese-compare` for the metrics it
indexes; the language packs use `stringcheese-phonetic` /
`stringcheese-unicode` / `stringcheese-manip`. The facade
`stringcheese` re-exports the public surface of every sub-project so
callers who don't need fine-grained dependency selection can add one
crate to `Cargo.toml`.

## Core Sequence Model

The library fundamentally compares sequences.

Possible sequence types include:

- bytes
- Unicode scalar values
- grapheme clusters
- tokens
- phonemes
- generic slices

Strings are simply one specialization.

## Comparison Categories

The library recognizes multiple categories.

### Distance

Lower is better.

Examples: Levenshtein, Hamming, Damerau, edit distance.

### Similarity

Higher is better.

Examples: Jaro, Jaro-Winkler, cosine, Dice, Jaccard similarity.

### Score

Neither distance nor similarity.

Examples: Smith-Waterman, Needleman-Wunsch, probabilistic linkage,
learned scoring models.

### Predicate

Examples: phonetic key equality, exact equality, prefix/suffix matching.

## Mathematical Properties

Algorithms should expose their mathematical guarantees.

- Metric
- Semimetric
- Pseudometric
- Quasimetric
- Divergence
- Similarity
- Kernel
- Score

Each implementation exposes:

- symmetry
- identity preservation
- triangle inequality
- boundedness
- normalization

This information is usable by indexing structures. Example: a BK-tree should
only accept true metrics.

## Result Types

The library avoids returning anonymous floating-point values.

Instead: `Distance<T>`, `Similarity<T>`, `Score<T>`, `NormalizedDistance`,
`NormalizedSimilarity`.

Conversions are explicit. No global rule such as `distance = 1 - similarity`
exists. Normalization policy must be specified.

### Normalization Policies

Examples for Levenshtein:

- divide by max length
- divide by total length
- custom

Normalization becomes an explicit strategy.

## Representation Layers

Algorithms should work over multiple representations:

- bytes
- Unicode scalars
- graphemes
- words
- tokens
- phonemes

The API should never silently choose.

## Algorithms

### Edit Distance
- Levenshtein
- Weighted Levenshtein
- Damerau-Levenshtein
- Optimal String Alignment
- Hamming
- Longest Common Subsequence
- Longest Common Substring

### Alignment
- Needleman-Wunsch
- Smith-Waterman
- Affine gap alignment
- Edit script reconstruction

### Similarity
- Jaro
- Jaro-Winkler
- Dice
- Jaccard
- Overlap coefficient
- Cosine similarity

### N-Gram Measures
- Dice
- Jaccard
- Cosine
- Weighted Jaccard
- Containment similarity

### Phonetic Matching

Phonetics is a first-class subsystem — not merely another comparison
function. Supported algorithms include:

- Soundex
- Refined Soundex
- Metaphone
- Double Metaphone
- NYSIIS
- Match Rating
- Cologne Phonetics
- Caverphone
- Daitch-Mokotoff
- Beider-Morse

### Multilingual Support

The library supports as many languages as practical. Language support is
modular:

- phonetic-germanic
- phonetic-romance
- phonetic-slavic
- phonetic-semitic
- phonetic-indic
- phonetic-cjk

Support includes language, script, and region. The API distinguishes native
script, transliteration, and pronunciation rules.

### Phoneme-Level Comparison

Long-term goal. Rather than comparing phonetic hashes, compare phoneme
sequences with phoneme edit distance. Supports multilingual matching.

## Unicode

Unicode is modular:

- NFC / NFD / NFKC / NFKD
- case folding
- grapheme segmentation
- diacritic removal
- transliteration

## Preprocessing Pipeline

Comparison is rarely performed on raw strings. Pipeline objects are
reusable:

    normalize -> case fold -> remove punctuation -> collapse whitespace
        -> tokenize -> phonetic encoding -> comparison

## N-Grams

N-grams are a representation layer — not merely a comparison algorithm.

Supported representations: character, byte, grapheme, token, phoneme,
skip-grams.

Policies: boundary markers, multiplicity, weighting, fixed N, variable N.

Representations: set, multiset, weighted vector.

## Fingerprinting

Separate subsystem:

- Rabin fingerprints
- Polynomial rolling hash
- Buzhash
- Gear hash

## Search Algorithms

- Rabin-Karp
- KMP
- Boyer-Moore
- Horspool
- Two-way search
- Aho-Corasick

## Content Defined Chunking

Support Rabin CDC and FastCDC. Streaming interfaces. Reusable boundaries.
No unnecessary allocation.

## Index Structures

Future subsystem:

- BK-tree
- VP-tree
- N-gram inverted index
- Prefix filtering
- Length filtering
- MinHash
- Locality-sensitive hashing

## Memory Philosophy

Memory is explicit. Every algorithm documents:

- runtime
- auxiliary memory
- allocation behavior
- workspace requirements

### Workspace Reuse

Essential for entity resolution, databases, and WebAssembly.

### Streaming APIs

Many algorithms support streaming: FastCDC, rolling hashes, Rabin-Karp,
tokenization, fingerprinting.

## SIMD

Optional. Supported backends: scalar, native SIMD, wasm SIMD.

SIMD must never change observable behavior.

## WebAssembly

Primary deployment target. Requirements:

- no_std core
- alloc optional
- deterministic memory
- streaming
- reusable workspaces
- feature-gated Unicode
- feature-gated phonetics

### Component Model

Future WIT interface. Supports comparison, prepared objects, reusable
preprocessing, workspace reuse.

## Explainability

Comparison results should explain themselves. Example:

    Normalization:  NFKC
    Representation: Grapheme
    Algorithm:      Jaro-Winkler
    Similarity:     0.94
    Language:       German
    Phonetic:       Double Metaphone
    Threshold:      Passed

Entity resolution benefits enormously from explainability.

## Benchmark Philosophy

Benchmark more than runtime:

- runtime
- allocations
- peak memory
- binary size
- Wasm size
- SIMD improvement
- throughput
- cold start
- warm performance

## Feature Flags

- core
- distance
- alignment
- phonetic
- phonetic-germanic
- phonetic-slavic
- unicode
- unicode-full
- fingerprint
- search
- chunking
- indexing
- simd
- parallel
- std
- alloc

## Public Goals

StringCheese should become:

- the definitive Rust comparison library
- the reference implementation for sequence comparison
- suitable for production-scale entity resolution
- usable in databases
- usable in browsers
- usable in Wasm components
- usable in embedded systems
- suitable for DuckLink and SQLink integration
- suitable for WasmOS infrastructure

## Version 0.1 Scope

Core infrastructure:

- Comparison abstractions
- Result types
- Mathematical property system
- Normalization framework
- Unicode preprocessing
- Levenshtein
- Damerau
- Hamming
- Jaro
- Jaro-Winkler
- Dice
- Jaccard
- Character and token n-grams
- Soundex
- Double Metaphone
- NYSIIS
- Workspace reuse
- SIMD where appropriate
- no_std core
- WebAssembly support
- Comprehensive benchmark suite

## Future Roadmap

### Version 0.2
- Smith-Waterman
- Needleman-Wunsch
- affine gaps
- phoneme representations
- multilingual phonetic packs
- BK-trees
- VP-trees
- FastCDC
- Rabin fingerprints
- Gear hash
- Rabin-Karp
- Buzhash
- streaming APIs

### Version 0.3
- probabilistic linkage primitives
- MinHash
- locality-sensitive hashing
- learned similarity models
- Component Model bindings
- database integration
- SQL operators
- DuckLink integration
- SQLink integration

## Guiding Principle

The defining characteristic of StringCheese is semantic precision. Existing
libraries generally expose algorithms. StringCheese exposes algorithms and their
meaning. Every comparison carries explicit information about:

- what was compared
- how it was normalized
- what mathematical guarantees apply
- what computational cost was incurred
- why two sequences matched

The library should be known not simply for the breadth of algorithms it
implements, but for making sequence comparison correct, explainable,
performant, multilingual, and practical across native and WebAssembly
environments.

---

# Validation, Golden Datasets, and Comparative Benchmarking

## Purpose

StringCheese must provide objective evidence that its implementations are:

- mathematically correct
- semantically well-defined
- compatible with published algorithm definitions
- consistent across native and WebAssembly targets
- competitive with existing libraries
- efficient in both runtime and memory usage

Correctness and performance validation are first-class deliverables. The
validation system should be substantial enough that it can independently
serve as a reference corpus for string-comparison implementations.

## Validation Strategy

Validation uses several complementary methods. No single method is
sufficient.

### Validation Layers

- Hand-authored canonical examples
- Exhaustive small-domain testing
- Property-based testing
- Differential testing against independent implementations
- Golden datasets
- Metamorphic testing
- Cross-backend consistency testing
- Performance and memory benchmarking
- Fuzzing
- Specification and paper conformance tests

### Canonical Test Vectors

Each algorithm includes canonical examples derived from original papers,
standards, widely cited textbook examples, authoritative reference
implementations, and manually verified edge cases.

Examples cover empty strings, identical strings, one empty string,
one-character differences, repeated symbols, transpositions, prefixes and
suffixes, Unicode, normalization-sensitive strings, asymmetric inputs,
maximum-distance cutoffs, integer overflow boundaries, long inputs.

Canonical vectors record the expected result and its derivation.

### Exhaustive Small-Domain Oracles

For algorithms where a straightforward implementation is practical, maintain
an intentionally simple oracle implementation. The oracle prioritizes
clarity and correctness over performance.

Then exhaustively generate all strings over small alphabets (e.g. `{a, b}`
lengths 0–8 or `{a, b, c}` lengths 0–6). Every optimized implementation must
agree with the oracle.

This is particularly important for banded edit distance, cutoff-aware
implementations, bit-parallel algorithms, SIMD implementations, compact
integer-cell variants, streaming implementations, and hashed n-gram
representations.

### Independent Oracle Implementations

Optimized implementations should not validate themselves. For important
algorithms, maintain at least two structurally independent implementations.
Agreement among implementations written from different formulations provides
stronger evidence than agreement among minor variants of the same code.

The oracle implementation resides in a validation-only crate and is not
compiled into normal library builds.

### Property-Based Testing

Metric properties (for algorithms declared as metrics):

    d(x, y) >= 0
    d(x, y) = 0 iff x = y
    d(x, y) = d(y, x)
    d(x, z) <= d(x, y) + d(y, z)

These are tested over generated sequences. Where properties depend on
configuration, tests generate only valid configurations or verify that
invalid configurations are rejected.

### Metamorphic Testing

Validates relationships between transformed inputs when exact expected
outputs are difficult to enumerate:

- Identity-preserving transformations (case folding, normalization)
- Prefix and suffix effects: `d(prefix + x, prefix + y) = d(x, y)`
- Symbol renaming (equality-only algorithms)
- Representation equivalence (prepared vs. unprepared)
- Backend equivalence (scalar = native SIMD = wasm SIMD)

### Differential Testing

Compares outputs against multiple independent libraries and language
ecosystems. The objective is not to blindly match every implementation — it
is to identify genuine defects, semantic ambiguities, normalization
differences, variant mismatches, and undocumented edge-case behavior.

Disagreement must not automatically cause StringCheese to imitate the majority
result. The implementation must follow its declared semantics and source
definition.

### Algorithm Variant Registry

Many algorithms have multiple incompatible definitions under the same name:

- restricted vs. unrestricted Damerau-Levenshtein
- optimal string alignment vs. full Damerau-Levenshtein
- several Levenshtein normalization formulas
- different Jaro matching-window definitions
- Jaro-Winkler prefix limits
- set vs. multiset Dice
- cosine distance vs. angular distance
- Soundex variants
- language-specific phonetic variants
- FastCDC normalization levels and masks

Each implementation has a stable variant identifier (`AlgorithmDescriptor`).
Golden datasets refer to the variant identifier rather than only the common
algorithm name.

## Golden Dataset Design

Golden datasets are versioned, machine-readable, and independently
consumable. Recommended formats: JSON Lines for readability, CBOR or
MessagePack for compact test execution, Parquet for large analytical
datasets, plain text manifests for provenance and licensing.

Each case includes: id, algorithm, variant, left, right, expected,
representation, normalization, source, and tags.

### Golden Dataset Categories

- Core edit-distance corpus (unit-cost, weighted, transpositions, unequal
  lengths, threshold boundaries, Unicode scalar and grapheme cases)
- Similarity corpus (Jaro/Jaro-Winkler examples, symmetry tests, prefix-boost
  boundaries, floating-point tolerances)
- N-gram corpus (all combinations of representation × n × padding × set/multiset)
- Phonetic corpus (multilingual, curated by algorithm applicability)
- Search corpus (Rabin-Karp/KMP/Boyer-Moore edge cases)
- Fingerprint corpus (known fingerprints, window transitions, rolling updates)
- Chunking corpus (FastCDC exact boundaries, chunk lengths, streaming
  vs. contiguous)
- Real-world corpora (personal names, company names, addresses,
  bibliographic records, multilingual text, OCR-like corruption)
- Regression corpus (every discovered bug becomes a permanent golden case)

### Dataset Provenance

Every dataset includes source, license, retrieval date, transformation
history, filtering rules, version, and cryptographic digest. Generated
datasets include random seed, generator version, and generator configuration.

### Floating-Point Validation

Floating-point algorithms require explicit comparison policy. Each algorithm
defines one of: exact bitwise equality, absolute tolerance, relative
tolerance, or ULP tolerance. Golden records store both the expected value
and comparison policy.

### Cross-Target Validation

Every release validates at least native scalar, native SIMD, wasm32-wasip1,
wasm32-unknown-unknown, WebAssembly SIMD, debug and release builds, and
32-bit and 64-bit targets where practical.

### Fuzzing

Fuzz targets include all public comparison functions, UTF-8 boundaries,
malformed byte-sequence APIs, custom cost tables, normalization pipelines,
prepared representations, streaming chunk boundaries, rolling hash state
transitions, and workspace sizing.

Important differential fuzz targets: optimized vs. oracle; scalar vs. SIMD;
contiguous vs. streaming; prepared vs. direct; native vs. Wasm.

## Performance Benchmarks

Correctness benchmarks and performance benchmarks remain distinct.

### Benchmark Dimensions

- latency, throughput, CPU time, wall-clock time
- allocations, total bytes allocated, peak resident memory
- scratch-memory requirement
- Wasm linear-memory growth
- compiled binary size, component size, instantiation time
- cold first-call latency vs. steady-state performance

### Input Dimensions

Benchmark across input length, alphabet size, edit distance, percentage
similarity, ASCII vs. multilingual Unicode, repeated symbols, random inputs,
natural-language inputs, short names vs. long documents, batch size,
threshold value, prepared vs. unprepared operation.

### Workload Modes

Single pair; one query against many candidates; all-pairs comparison;
thresholded filtering; top-k ranking; streaming input; prepared corpus;
index-assisted lookup.

### Comparative Library Benchmarking

Adapters live under `bench-adapters/{rust,python,java,javascript,cpp,go}/`.

Each adapter performs no unnecessary conversion inside timed regions,
preloads or prepares inputs consistently, separates startup cost from
steady-state cost, exposes allocation metrics where possible, and emits
results in a common machine-readable format.

Do not compare differently defined algorithms under the same label. Results
clearly identify non-equivalent variants.

### Pareto Analysis

Report Pareto frontiers rather than optimize for runtime alone. Dimensions:
latency, throughput, memory, allocations, binary size, implementation
capability, Unicode support, cutoff support, edit-script support, streaming
support.

## Continuous Integration Requirements

Every pull request runs: unit tests, canonical golden tests, property
tests, differential tests against internal oracles, regression corpus,
scalar/SIMD equivalence, native/Wasm equivalence, fuzz smoke tests,
benchmark compilation checks.

Nightly or scheduled CI runs: full external differential suite, large
golden datasets, long-running fuzzing, full comparative benchmarks, memory
benchmarks, binary-size tracking.

## Release Gates

A release does not proceed unless:

1. All golden datasets pass.
2. All declared mathematical properties pass generated tests.
3. All optimized implementations agree with their independent oracle.
4. Native and WebAssembly results agree.
5. Scalar and SIMD implementations agree.
6. No unresolved differential discrepancy is classified as a StringCheese defect.
7. Performance regressions beyond defined thresholds are reviewed.
8. Binary-size and memory regressions are reviewed.
9. Dataset and benchmark versions are recorded in the release manifest.

## Public Correctness Report

Each release publishes a machine-generated correctness report:

- Algorithms tested
- Variants tested
- Golden cases executed
- Generated cases executed
- External implementations compared
- Agreements
- Known semantic differences
- Known external discrepancies
- Fuzzing duration
- Targets tested
- Dataset versions

## Golden Dataset as a Project Asset

The golden corpus is a standalone deliverable. Structure:

    stringcheese-corpus/
        schema/
        edit-distance/
        similarity/
        ngram/
        phonetic/
        search/
        fingerprint/
        chunking/
        unicode/
        regression/
        tools/
        manifests/

The corpus is versioned independently from the Rust library.

## Implementation Sequence

### Phase 1
1. Define golden-case schema.
2. Build full-matrix edit-distance oracles.
3. Add exhaustive small-alphabet generators.
4. Add canonical examples.
5. Add property-based tests.
6. Add scalar vs. optimized differential tests.

### Phase 2
1. Build external benchmark adapter protocol.
2. Compare against selected Rust, Python, Java, and JavaScript implementations.
3. Add automated discrepancy classification.
4. Publish initial correctness report.

### Phase 3
1. Add multilingual phonetic corpora.
2. Add Unicode normalization corpus.
3. Add n-gram representation corpus.
4. Add native/Wasm equivalence harness.

### Phase 4
1. Add fingerprint and chunking datasets.
2. Add streaming split enumeration.
3. Add comparative performance dashboards.
4. Publish the corpus as a separately versioned project.

## Design Principle

StringCheese should never ask users to trust that an implementation is correct
because it is fast, widely used, or resembles a textbook implementation.
Correctness must be demonstrated through:

- independent derivation
- exhaustive testing
- differential comparison
- mathematical properties
- cross-platform consistency
- permanent regression datasets

The benchmark and golden-data infrastructure is part of the product, not
incidental test code. This gives the project a second defensible asset: not
just the Rust implementation, but a substantial, versioned sequence-comparison
conformance corpus that other libraries can test against.
