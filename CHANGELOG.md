# Changelog

All notable changes to StringCheese are documented in this file.

The format is based on [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html).
Until the first `0.1.0` release, expect breaking changes on any minor
bump; `0.x` versions are pre-stability.

## [Unreleased]

### Added

- **`stringcheese-am` — Amharic language pack.** New workspace crate,
  registered as BCP-47 `"am"`. **First Ge'ez-script pack** in
  StringCheese (Ethiopic syllabary — an abugida where each character
  represents a consonant+vowel syllable, unlike Devanagari/Bengali
  which use base+matra). First Ethiopian/Eritrean language pack; Semitic
  sibling of Arabic (`ar`) and Hebrew (`he`) but written in Ge'ez
  instead of the Arabic/Hebrew abjads. Dedicated `geez` module with
  `decompose(char) → Option<(family_head, order 0..7)>` and
  `compose(base, order) → Option<char>` — pure arithmetic on the
  48-row × 8-column main-block layout at U+1200..=U+137F (supplement
  U+1380..=U+139F and extended U+2D80..=U+2DDF return `None` — layout
  differs). Rule-based iterate-to-convergence longest-match stripper
  covering the definite article `-ው`/`-ዋ` (masc/fem), plural `-ኦች`,
  8 possessive suffixes, 8 object suffixes. 2-scalar min-stem guard;
  `Vec<char>` arithmetic throughout since every Ge'ez scalar is 3
  bytes in UTF-8. Tokenizer splits on Ge'ez punctuation `፡`/`።`/`፣`/
  `፤`/`፥`/`፦`/`፧`/`፨` (U+1361..=U+1368) plus ASCII whitespace. Two-
  stage phonex: BGN/PCGN-style Latin transliteration (48-family table
  with Buckwalter-inspired stand-ins for pharyngeals/emphatics/
  sibilants/palatals/glottals/ayn) then Soundex-shape 4-char
  reduction; adapter name `"phonex-am"`. ~70-entry stopword list.

- **`stringcheese-ka` — Georgian language pack.** New workspace crate,
  registered as BCP-47 `"ka"`. **First Kartvelian-family pack** — a
  small Caucasian family (Georgian, Mingrelian, Laz, Svan) unrelated
  to every prior family in the workspace. **First Georgian-script
  pack** — Mkhedruli `U+10D0..=U+10FF` (3 bytes/scalar), all
  arithmetic on `Vec<char>`. Longest-match stemmer covering all 7
  grammatical cases (nominative `-ი`, dative-accusative `-ს`,
  ergative `-მა`, genitive `-ის`, instrumental `-ით`, adverbial
  `-ად`, vocative unmarked), plural markers (contemporary `-ები`,
  archaic `-ნი`/`-თა`), 5 agglutinated postpositions (`-ში` "in",
  `-ზე` "on", `-თან` "at", `-გან` "from", `-კენ` "toward", plus
  `-თვის` "for"), plural+case/postposition compounds, and common
  verb personal/tense endings. 2-scalar min-stem guard; longest-match
  wins so `-ისთვის` (6 chars) beats bare `-ის` (2 chars). Tokenizer
  handles Mkhedruli, Mtavruli (Unicode 11 capitalized), Asomtavruli,
  and Nuskhuri as alphabetic; `჻` (U+10FB) as punctuation. PHONEX
  case-folds Mtavruli → Mkhedruli via `char::to_lowercase`, then maps
  each scalar to **ISO 9984 (1996)** Latin form (33 modern letters
  including the distinctive **glottalized/ejective consonants**
  `კ`/`პ`/`ტ`/`წ`/`ჭ`/`ყ` marked with apostrophe). The apostrophe
  drops before Soundex classification, so ejective/aspirate pairs
  (`კ`/`ქ`, `პ`/`ფ`, `ტ`/`თ`, `წ`/`ც`, `ჭ`/`ჩ`) collapse to the
  same key by design. Adapter name `"phonex-ka"`. ~65-entry
  Mkhedruli-lowercase stopword list.

- **`stringcheese-hy` — Armenian (Eastern Armenian) language pack.**
  New workspace crate, registered as BCP-47 `"hy"`. **First
  Armenian-script pack** — Armenian `U+0530..=U+058F`, 2 bytes/scalar.
  **Indo-European isolate branch** — a family-of-one within IE, in
  the same typological company as Greek (`el`) and Albanian.
  Longest-match iterate-to-convergence stripper covering all 7
  Eastern Armenian singular case suffixes (genitive `-ի`, dative
  `-ին`, ablative `-ից`, instrumental `-ով`, locative `-ում`
  which is also the imperfective-participle marker, postposed
  definite article `-ը`/`-ն`), the two plural markers `-եր`
  (monosyllabic base) / `-ներ` (polysyllabic base), their
  plural+case combinations, and aorist personal endings
  (`-եցի`/`-եցիր`/`-եց`/`-եցինք`/`-եցիք`/`-եցին`). 2-scalar
  min-stem guard. Documented over-strip: `տանում` "in house" stems
  to `տա` because the cascade first strips `-ում` then strips the
  `-ն` definite article. PHONEX-hy (`"phonex-hy"`) with
  aspiration-collapsing consonant folds (labial stops
  `պ`/`փ`/`բ → P`, dental stops `տ`/`թ`/`դ → T`, velar stops
  `կ`/`ք`/`գ → K`, dental affricates `ծ`/`ց`/`ձ → C`,
  palato-alveolar affricates `ճ`/`չ`/`ջ → J`); digraph handling
  for `ու → U` and ligature `և → EV`; two-form spelling `եւ` and
  `և` normalized to a single form at every entry point. Overrides
  `Language::is_stopword` with Unicode case-fold + `եւ → և`
  normalization.

- **`stringcheese-ml` — Malayalam language pack.** New workspace
  crate, registered as BCP-47 `"ml"`. **Second Dravidian pack**
  (sibling of Tamil), **fourth Brahmic-script pack** (after
  Devanagari, Bengali, Tamil). First Malayalam-script pack —
  `U+0D00..=U+0D7F`, 3 bytes/scalar, `Vec<char>` throughout.
  Malayalam is a hybrid — retains the classical Sanskrit
  **four-way stop series** across five places of articulation (like
  Devanagari, unlike Tamil which collapses to one stop per place)
  PLUS the three Dravidian additions (`ള`/`ഴ`/`റ`). Signature
  feature: **six atomic chillu letters** at U+0D7A..=U+0D7F
  (`ൺ ൻ ർ ൽ ൾ ൿ`) representing word-final consonants without
  vowels — unique among Brahmic scripts. Conjunct consonants form
  via virama `്` (U+0D4D), unlike Tamil which uses only explicit
  pulli. Longest-match stripper covering 6 case suffixes
  (accusative `-നെ`, genitive `-ന്റെ`/`-ുടെ`, dative `-ന്`,
  locative `-ിൽ`, instrumental `-ാൽ`/`-ിനാൽ`, sociative `-ോട്`),
  plural markers (`-കൾ` inanimate, `-മാർ` animate, `-ങ്ങൾ`
  linked variant), verb tense/aspect/adjectival endings, and
  emphatic particles. **Bare chillu word-endings deliberately left
  in place** so pronouns like `ഞാൻ`/`അവർ`/`അവൾ` survive the
  stemmer unchanged. PHONEX-ml (`"phonex-ml"`) via ISO 15919
  transliteration with chillu letters encoded as bare-consonant
  suppressed-schwa forms.

- **`stringcheese-pa` — Punjabi (Eastern) language pack.** New
  workspace crate, registered as BCP-47 `"pa"` (defaults to
  Gurmukhi/`pa-Guru`; Shahmukhi variant `pa-Arab` deferred). **First
  Gurmukhi-script pack** — Gurmukhi `U+0A00..=U+0A7F`,
  3 bytes/scalar. **Fifth Brahmic-script pack** (hi/bn/ta/ml/pa).
  Signature feature: **Punjabi is a tonal language** — the
  historical Sanskrit-inherited voiced-aspirate letters
  `ਘ`/`ਝ`/`ਢ`/`ਧ`/`ਭ` have lost their voicing and aspiration in
  modern Punjabi and now encode a **tone contour** on the adjacent
  vowel (low tone at syllable start, high tone at syllable end),
  while the phone matches the corresponding voiceless-unaspirated
  stop. Longest-match stripper covering case markers, plural
  markers (`-ਾਂ`/`-ਆਂ`/`-ੀਆਂ`/`-ਿਆਂ`), imperfective participles
  (`-ਦਾ`/`-ਦੀ`/`-ਦੇ`), and perfective/aorist endings. Tippi (`ੰ`),
  bindi (`ਂ`), and addak (`ੱ`) never stripped alone. Two-stage
  phonex via `PunjabiIso15919` then **Punjabi-specific tone-
  collapse pre-pass** — folds historical voiced-aspirate digrams to
  voiceless-unaspirated counterparts (`gh → k`, `jh → c`,
  `ḍh → ṭ`, `dh → t`, `bh → p`) so tone-marked and unmarked
  spellings of the same word (e.g. `ਘਰ` and `ਕਰ`) share a key.
  Handles addak-driven gemination (`ਪੱਕਾ` → "pakkā"), tippi/bindi
  nasalization, and Perso-Arabic nukta letters `ਖ਼ ਗ਼ ਜ਼ ਫ਼`.

- **`stringcheese-mr` — Marathi language pack.** New workspace
  crate, registered as BCP-47 `"mr"`. **Second Devanagari-script
  pack** (after Hindi). Marathi differs from Hindi in three
  linguistically-meaningful ways: (1) retains the **OIA neuter
  gender** (Hindi has only masc/fem) — three-way verb/adjective
  agreement; (2) case marking is **agglutinative** — case suffixes
  attach directly to the noun stem (`घराला` "to the house" — one
  orthographic word) rather than Hindi's postposition-based system
  (`घर को` — two tokens), which means the Marathi stemmer is
  substantially more useful than Hindi's because Marathi words
  genuinely inflect at the surface spelling level; (3) retains the
  letters `ळ` (U+0933, retroflex L /ɭ/) and `ऱ` (U+0931) that
  Standard Modern Hindi lacks. Longest-match stripper (iterated to
  convergence per idempotence requirement) covering agglutinative
  case markers (`-ला`/`-ना` dative, `-चा`/`-ची`/`-चे`/`-च्या`
  genitive with head-noun agreement, `-ने`/`-नी` instrumental,
  `-त`/`-मध्ये` locative, `-ऊन`/`-हून` ablative, `-शी`/`-सह`
  sociative), plural markers, verb personal endings, aorist/
  perfective endings, infinitive `-णे`, and linking-vowel matras
  (`-ा`/`-े`/`-ी`). Documented tradeoff: over-strips a small class
  of surface stems (`मुलाने → मु`) — unavoidable without a lexicon.
  Two-stage ISO 15919 → PHONEX-mr (`"phonex-mr"`). Devanagari
  transliteration tables structurally adapted from Hindi pack.

- **`stringcheese-ro` — Romanian language pack.** New workspace
  crate, registered as BCP-47 `"ro"`; `ro-MD` (Moldovan-Latin)
  falls back to `ro` via BCP-47 subtag walk. **First Balkan Romance
  pack** — Romanian is genealogically Romance (sibling of Spanish/
  French/Portuguese/Italian, descended from Vulgar Latin) but
  geographically Balkan, having spent centuries inside the **Balkan
  Sprachbund** alongside Bulgarian/Macedonian/Albanian/Greek and
  picked up several signature Balkan features its Romance cousins
  lack. Signature: **postposed definite articles** (like
  Bulgarian/Macedonian — `omul` "the man") and **retained Latin
  case marking** (`-lui` gen/dat masc sg, `-i`/`-ii` fem sg, `-lor`
  gen pl) — every other Romance language lost the Latin case system.
  Snowball Romanian stemmer per Porter's `romanian.sbl` — five-step
  cascade: (0) preprocess with **cedilla-to-comma-below fold**
  (`ş → ș` U+015F → U+0219, `ţ → ț` U+0163 → U+021B) at every entry
  point, glide-marking pass (i/u between vowels marked
  consonantal); step 0 postposed-article strip in R1; step 1
  standard-suffix replacement (iterated to fix-point per sbl `do
  repeat`); step 2 combining-suffix delete in R2; step 3 verb
  personal endings in RV (gated `!step1 && !step2`); step 4 final
  vowel drop in RV. Documented divergence: `steaua` stems to `steau`
  rather than `stea` because word-final glide marking blocks the
  literal `aua` suffix match. PHONEX-ro (`"phonex-ro"`) with
  `ch → K` / `gh → G` before front vowel (Romance hard-`k`/hard-`g`
  spelling convention), silent intervocalic `h`, cedilla fold.

- **HuggingFace `Metaspace` pre-tokenizer + `Precompiled` normalizer
  passthrough in `stringcheese-tokenizer-bpe`.** Completes the missing
  SentencePiece pieces that pair with wave 12's Unigram model support,
  making Llama, Mistral, T5, and XLM-RoBERTa tokenizer.json files fully
  loadable end-to-end. `Metaspace` implements the HF-canonical rule set:
  replace ASCII space with `▁` (U+2581, "lower one-eighth block"), then
  optional prepend controlled by `prepend_scheme` (`always` /
  `first` / `never`), then optional `split` with SentencePiece
  `MergedWithNext` semantics so each output piece begins with `▁`. New
  typed `HfPreTokenizer::Metaspace { replacement, prepend_scheme, split }`
  variant with HF-canonical serde defaults replaces the previous
  `serde_json::Value` catch-all; `HfPrependScheme` deserializes with
  `#[serde(rename_all = "lowercase")]`. `Precompiled` charsmap is
  parsed into `Normalizer::Precompiled { charsmap_base64: String }` so
  real Llama tokenizer.json files stop erroring; the runtime apply is a
  documented passthrough with a TODO for full SentencePiece charsmap
  execution. `Precompiled` inside a normalizer `Sequence` (the real
  SentencePiece shape, e.g. Prepend + Replace + Precompiled) parses
  correctly. 13 Metaspace tests + 6 Precompiled tests. Public
  `to_runtime_metaspace` helper exported.

- **`stringcheese-is` — Icelandic language pack.** New workspace crate,
  registered as BCP-47 `"is"`. **Completes the Nordic quintet** (sv,
  nb, nn, da, is). Since Icelandic has no official Snowball algorithm,
  ships a rule-based longest-match suffix stripper with
  `MIN_STEM_CHARS ≥ 3` (per-suffix override to `≥ 4` on the bare `-t`
  neuter suffix — `hest → hes` over-strip fixed) covering: definite-
  article suffix inventory (`-inum`/`-inni` masc/fem dat sg def, `-inn`
  masc nom sg def, `-nir` masc nom pl def, `-nar` fem/masc acc pl def,
  `-num` dat pl def, `-nni` fem dat sg def alt, `-nu` fem acc/dat sg
  def, `-ið` neut sg def, `-in` fem nom sg / neut nom pl def), noun
  case inventory (`-ur` masc nom sg, `-ar` nom pl / fem gen sg, `-ir`
  fem/masc weak nom pl, `-um` dat pl universal, `-s` masc/neut gen sg,
  `-i` dat sg universal, `-a` gen pl / weak neut), verb personal
  endings (`-um` 1pl, `-uð` 2pl archaic past, `-ir` 2sg, `-ið` 2pl,
  `-a` inf/3pl), adjective agreement (`-ur`/`-ir`/`-um`/`-a`/`-t`).
  Iterates to convergence: `hesturinn → hestur → hest` in one call.
  PHONEX-is (`"phonex-is"`) with `þ → th`, `ð → dh`, `æ → ae`,
  `ö → oe`, `hv → kv` (Modern Icelandic hv-/kv- merger), silent `h`,
  long-vowel accent folds. `Vec<char>` arithmetic throughout since
  every Icelandic-specific scalar is multi-byte. ~90-word stopword
  list (paradigms of `vera`/`hafa`/`skulu`/`vilja`/`geta`/`mega`).
  Documented over-strips: `hestunum → hestu` (would need `-unum`),
  `konunni → konu` — full lemma-quality needs a lexicon.

- **`stringcheese-be` — Belarusian language pack.** New workspace crate,
  registered as BCP-47 `"be"`. Belarusian is **East Slavic** (Narkamaŭka
  orthography targeted) — carries the letter `ў` (short u) distinctive
  from Russian/Ukrainian. 87-word Cyrillic stopword list. Apostrophe-
  aware tokenizer preserving `сям'я`/`аб'ект`/`пад'езд`. Light suffix-
  stripping stemmer over `Vec<char>` with RV region guard + theme-vowel
  guard on past-tense `-ў`, `-ла`, `-ло`, `-лі`. Suffix table
  deliberately omits `-аў`, `-аць`, `-яць` — they would race against
  the theme-vowel-guarded past-tense `-ў` and infinitive `-ць` on words
  like `чытаў`/`чытаць` and steal the longer match, over-stemming verbs
  to `чыт` instead of `чыта`. PHONEX-be (`"phonex-be"`) with digraph
  rewrites `дж → J`, `дз → Z` and short-u `ў → W` grouped with the
  labial class alongside `в`/`б`/`п`/`ф`. Belarusian carries `ё` as a
  distinct vowel (no `ё → е` fold, unlike Russian conventions). PHONEX
  gates on Cyrillic-block presence — returns `None` for ASCII-only
  input or lone soft-sign `ь`.

- **`stringcheese-mk` — Macedonian language pack.** New workspace crate,
  registered as BCP-47 `"mk"`. **Fourth Cyrillic-script pack** (ru, uk,
  bg, sr, mk). Macedonian is **South Slavic** — closest relative to
  Bulgarian, sharing the near-total loss of noun declension. Signature
  feature: **three-way postposed definite article** with proximity
  distinction — proximal `-ов`/`-ва`/`-во`/`-ве` (near speaker), medial
  `-от`/`-та`/`-то`/`-те` (neutral), distal `-он`/`-на`/`-но`/`-не`
  (distant / far from speaker). All 12 forms stripped in the first
  cascade step, followed by plural (`-ови`/`-еви`/`-ња`), verb personal
  endings (`-ам`/`-аш`/`-ат`/`-ме`/`-те`/`-ав`), and bare-vowel
  (`-а`/`-и`/`-о`/`-у`) passes. `Vec<char>` arithmetic throughout.
  155-word stopword list. PHONEX-mk (`"phonex-mk"`) folds the seven
  Macedonian-specific letters (`ѓ`/`ќ`/`љ`/`њ`/`џ`/`ѕ`/`ј`) into
  Slavic-Soundex consonant classes.

- **`stringcheese-th` — Thai language pack.** New workspace crate,
  registered as BCP-47 `"th"`. **First Thai-script pack** — Thai
  scalars U+0E00..=U+0E7F are 3 bytes in UTF-8, all arithmetic on
  `Vec<char>`. Thai is fully **analytic** (no case, no plural, no
  verb-tense inflection) — the stemmer is near-identity: strips a
  small closed set of nominalizer/agent prefixes
  `การ-`/`ความ-`/`ผู้-`/`นัก-`/`เครื่อง-` (2-scalar min-stem guard)
  and folds exact word-level reduplication `XX → X`; everything else
  passes through, matching `stringcheese-zh`'s design. **Word
  segmentation is heuristic-only** — true Thai word segmentation
  requires a dictionary + statistical/neural model (ICU's Thai break
  iterator, PyThaiNLP's `newmm`, `attacut`, `deepcut`) and is
  **deferred** to a future `stringcheese-th-newmm` sibling; the base
  pack ships a naive syllable-cluster segmenter (11-variant `CharType`
  covering consonants, pre-vowels U+0E40..=U+0E44, vowel marks, tone
  marks, signs, digits) that ends clusters at the second consonant —
  so `ไทย` splits as `ไท` + `ย`. Tokenizer models Thai's pre-vowel
  convention: the five leading vowels `เ`/`แ`/`โ`/`ใ`/`ไ` are typed
  **before** the consonant they modify, so `เป็น` captures as
  pre-vowel + consonant + vowel-mark. PHONEX-th (`"phonex-th"`) drops
  tone marks/vowel marks/signs, folds consonants by RTGS families
  (velars `ก ข ฃ ค ฅ ฆ → K`, sibilants `ซ ศ ษ ส → S`, dentals
  `ต ถ ท ธ ฐ ฑ ฒ ฏ → T`, nasals `ง ณ น → N`, liquid `ร → R`,
  `ล ฬ → L`, glide `ญ ย → Y`, glottal `ห ฮ → H` dropped), then
  standard Soundex-shape 4-char reduction. ~55-word stopword list.

- **`stringcheese-id` — Indonesian (Bahasa Indonesia) language pack.**
  New workspace crate, registered as BCP-47 `"id"`. **First Malayo-
  Polynesian / Austronesian pack in the workspace** — every prior
  pack is Indo-European, Sino-Tibetan, Japonic, Koreanic, Semitic,
  Uralic, Turkic, Dravidian, or Austroasiatic. Simplified
  **Nazief-Adriani stemmer** (Nazief & Adriani, Universitas Indonesia
  1996 — the canonical Indonesian IR stemmer) shipped WITHOUT the
  reference algorithm's root-word dictionary, calibrated to over-stem
  rarely rather than under-stem. Five-step ordered cascade: stopword
  short-circuit → particle suffix (`-lah`/`-kah`/`-tah`/`-pun`) →
  possessive (`-ku`/`-mu`/`-nya`) → derivational suffix
  (`-kan`/`-an`/`-i`) → derivational prefix with `me-`/`pe-` consonant
  restoration reversing nasal assimilation (`mem-`+vowel restore `p`:
  `memilih → pilih`; `men-`+vowel restore `t`: `menulis → tulis`;
  `meny-`+vowel restore `s`: `menyapu → sapu`; `me-`+sonorant strips
  bare: `melihat → lihat`). `meng-` ambiguity resolved as **no-elision**:
  `mengambil → ambil` (correct), `mengirim → irim` (documented
  over-strip, locked into a `documented_over_strips_are_locked_in`
  test). Confix-inhibition rules protect against over-strips:
  `ber-`/`di-`/`ter-` + `-an` refuses `-an` strip (so `berjalan →
  jalan`, not `jal`); possessive-stripped state refuses `-an` (so
  `tanganku → tangan`, not `tang`); prefix-stripped state refuses `-i`
  (so `menari → tari`, not `tar`); commit-on-shape rule prevents bare
  `me-`/`pe-`+sonorant handler firing on words like `pergi` where the
  3-letter prefix shape is present but residue is too short. 3-char
  minimum-stem floor. PHONEX-id (`"phonex-id"`) with digraph rewrites
  `ny → N`, `ng → G` (folded to `G` class 2 rather than `N` class 5 —
  preserves `bunga` vs. `bunda` distinction), `sy → S`, `kh → K`.
  ~90-word ASCII stopword list.

- **`stringcheese-ta` — Tamil language pack.** New workspace crate,
  registered as BCP-47 `"ta"`. **First Dravidian pack** (non-Indo-
  European, non-Uralic, non-Sino-Tibetan) — third Brahmic-script pack
  (after Hindi/Devanagari and Bengali). Tamil scalars U+0B80..=U+0BFF
  are 3 bytes in UTF-8, all arithmetic on `Vec<char>`. Tamil is
  agglutinative like Finnish/Korean with rich case/verb morphology.
  Longest-match suffix stripper (2-scalar min-stem guard) covering:
  plural `-கள்`, all 8 case suffixes (`-ஐ` accusative, `-ஆல்`
  instrumental, `-கு` dative, `-இன்` genitive, `-இல்` locative,
  `-ஓடு`/`-உடன்` sociative, `-வரை` allative), verb personal endings
  (`-கிறேன்`/`-கிறாய்`/`-கிறார்`/`-கிறோம்`/`-கிறீர்கள்`/`-கிறார்கள்`),
  tense markers before personal ending (`-கிறு`/`-கின்று`/`-வ`),
  interrogative particles (`-ஆ`/`-ஏ`). Suffix table lists both
  independent-vowel forms (`-ஆல்`) and matra surface variants
  (`-ால்`) that appear after consonant stems. Property test:
  convergence within 16 iterations. PHONEX-ta (`"phonex-ta"`) via
  ISO 15919 transliteration → 4-char Soundex-shape key with
  Tamil-appropriate consonant classes; folds Tamil-specific
  alveolar/retroflex distinctions (`ன → N` alveolar-n, `ற → R`
  alveolar-tap, `ழ → L` retroflex approximant — the famous "ḻ" of
  "tamiḻ"). Tamil has a **single stop-consonant series** (unlike
  Devanagari's 4-way voiced/aspirated distinction), simplifying the
  phonetic map. ~50-word stopword list.

- **HuggingFace `Unigram` model support in `stringcheese-tokenizer-bpe`.**
  Adds `HfUnigramModel` + `HfModel::Unigram` variant and a new
  `UnigramTokenizer` runtime executing Viterbi forward-DP over Unicode
  character positions. Best path = `max over j<i of best_prev[j] +
  log_prob(input[j..i])`; UNK fallback via `unk_id` with a fixed 10.0
  log-space penalty so vocab-only paths always win when one exists.
  New dispatch variant `HfTokenizer::Unigram` in `to_tokenizer`, new
  conversion `to_unigram_tokenizer`, new error variants
  `UnsupportedModelForUnigram` and `UnigramUnkIdOutOfRange`. Unlocks
  end-to-end loading of Llama, Mistral, T5, and XLM-RoBERTa
  tokenizer.json files (Unigram/SentencePiece model family). 12
  integration tests cover whole-word preference, two-piece splits,
  char-by-char fallback, OOV/`unk` fallback, multibyte character
  boundaries, empty input, out-of-range `unk_id`, and non-Unigram
  rejection.

- **HuggingFace `BertNormalizer` support in `stringcheese-tokenizer-bpe`.**
  Extends `HfNormalizer` with the `BertNormalizer { clean_text,
  handle_chinese_chars, strip_accents, lowercase }` variant, all with
  HF-matching serde defaults (`true`/`true`/`None`/`true`) so bare
  `{"type": "BertNormalizer"}` deserialises correctly. Passes applied
  in HF order: `clean_text` (strip C0 controls + `\u{FFFD}`, replace
  `\t`/`\n`/`\r` with space) → `handle_chinese_chars` (pad Han-block
  codepoints with spaces so downstream WordPiece treats each as its own
  token) → `strip_accents` (NFD then drop combining marks via
  `canonical_combining_class != 0`; defaults to `lowercase` when not
  set) → `lowercase`. Full 24-combination toggle sweep asserts no field
  silently regresses. With Unigram + BertNormalizer + WordPiece +
  BertPreTokenizer now landed, BERT, DistilBERT, ELECTRA, Llama,
  Mistral, T5, and XLM-RoBERTa tokenizer.json files are all parseable
  and executable end-to-end.

- **`stringcheese-da` — Danish language pack.** New workspace crate,
  registered as BCP-47 `"da"`. Nordic sibling to `sv`/`no`. Four-step
  Snowball Danish stemmer per `danish.sbl`: Group A main-suffix
  unconditional delete + Group B bare-`s` guarded by valid-s-ending set
  `abcdfghjklmnoprtvyzå`; consonant-pair `-gd`/`-dt`/`-gt`/`-kt`
  trailing-letter strip; `-ig`/`-lig`/`-elig`/`-els`/`-løst` other-suffix
  processing with `-igst` prelude and `løst → løs` rewrite; undouble
  trailing repeated consonant when the pair sits in R1. R1 floor
  adjusted to ≥3 per the reference. ~120-word stopword list. PHONEX-da
  encoder (`"phonex-da"`): Soundex-shape 4-char key with `sj → S`,
  `sk`-before-front-vowel → `S`, `ch → S`, silent `H`, `å → O`,
  `æ`/`ø → E`. Documented in tests: unlike Norwegian, Danish's Snowball
  suffix table does not include `-ede`, so `elskede → elsked` (only the
  bare `-e` strips). 30-pair Snowball + 20-pair PHONEX reference tables.

- **`stringcheese-nn` — Norwegian Nynorsk language pack.** New
  workspace crate, registered as BCP-47 `"nn"` — sibling to
  `stringcheese-no` (registered as `"nb"` for Bokmål). Snowball
  Norwegian stemmer ported verbatim since the upstream `norwegian.sbl`
  covers both Bokmål and Nynorsk per the Snowball project. ~130-word
  Nynorsk-tuned stopword list with Nynorsk-specific pronouns
  (`eg`/`ho`/`me`/`dei`/`dykk`/`dykkar`/`deira`/`honom`), articles
  (`ein`/`ei`/`eit`), negation (`ikkje`), kv-interrogatives
  (`kva`/`kven`/`kvifor`/`korleis`/`kvar`), adverbs (`so`/`difor`/
  `mykje`), and Nynorsk verb paradigms (`vera`/`vore`/`verta`/`vert`/
  `vart`/`vorte`). PHONEX-nn encoder (`"phonex-nn"`) shares the same
  phonological cluster set as PHONEX-no.

- **`stringcheese-et` — Estonian language pack.** New workspace crate,
  registered as BCP-47 `"et"`. **Second Uralic pack** (sibling of
  Finnish, both non-Indo-European). Since Snowball has no official
  Estonian stemmer, ships a single-pass longest-match suffix stripper
  covering all 14 grammatical cases (`-le`/`-lt`/`-ks`/`-ni`/`-na`/
  `-ga`/`-ta`/`-sse`/`-st`/`-s`/`-de`/…), plural markers
  (`-d`/`-id`/`-te`/`-de`), verb inflections
  (`-me`/`-te`/`-vad`/`-sin`/`-sid`/`-sime`/`-site`/`-sid`), and the
  diminutive (`-ke`/`-kene`). Estonian has **no vowel harmony** (unlike
  Finnish) — the suffix table lists each suffix exactly once. `-si-`
  past-tense forms carry a vowel-preceding-stem check to disambiguate
  from noun-plural `-id` (`kass + -id → kassid` "cats" vs. `käi +
  -sid → käisid` "you went"). Single-char strips (`-s`/`-l`/`-d`/`-b`)
  carry a stricter 4-char min-stem floor to protect short base words
  (`kool`, `kass`, `ilus`). PHONEX-et encoder (`"phonex-et"`) folds
  `ä→a`, `ö→o`, `õ→o`, `ü→u`, `š→s`, `ž→z`. ~80 stopwords.

- **`stringcheese-bn` — Bengali language pack.** New workspace crate,
  registered as BCP-47 `"bn"`. **Second Brahmic-script pack** (sibling
  of Hindi/Devanagari). Bengali script `U+0980..=U+09FF`, 3 bytes per
  scalar in UTF-8; all string operations use `Vec<char>` arithmetic,
  never byte indexing. Word tokenizer covers the full Bengali block
  including matras/halant/anusvara as word-internal; danda `।`/`॥`
  separate. `LightBengaliStemmer` covering `-গুলি`/`-গুলো`/`-দের`/
  `-রা`/`-রে`/`-কে`/`-তে`/`-র`/`-য়` with 2-scalar min-stem guard.
  Two-stage phonex: `BengaliIso15919` (schwa-aware transliteration,
  virama suppresses inherent vowel, matras override, nukta variants
  both precomposed and decomposed, khanda ta `ৎ` → final `t`) then
  `BengaliPhonex` (Soundex-shape 4-char key with ISO-diacritic
  folding); adapter name `"phonex-bn"`. 65-entry stopword list. 20-pair
  stemmer + 17-pair PHONEX reference tables.

- **`stringcheese-ko` — Korean language pack.** New workspace crate,
  registered as BCP-47 `"ko"`. **First Hangul-script pack.** Ships a
  dedicated `jamo` module with `decompose_syllable(char) →
  Option<(char, char, Option<char>)>` and `compose_jamo(L, V, T) →
  Option<char>` implementing the closed-form Unicode `U+AC00..=U+D7A3`
  formulas: `SIndex = C - 0xAC00`, `L = 0x1100 + SIndex / (21*28)`,
  `V = 0x1161 + (SIndex % (21*28)) / 28`, `T = 0x11A7 + SIndex % 28`.
  `tests/jamo_decompose.rs` enumerates **all 11172 precomposed
  syllables** and asserts `compose(decompose(s)) == s`. Stemmer
  iteratively peels the closed particle set (`-에서`, `-까지`, `-부터`,
  `-에게`, `-으로`, `-는`, `-은`, `-을`, `-를`, `-이`, `-가`, `-에`,
  `-로`, `-와`, `-과`, `-의`, `-도`, `-만`, `-다`) with longest-match
  ordering so `학교에서` strips `-에서` (not `-에`). Space-delimited
  word tokenizer (unlike JA/ZH, Korean uses spaces between words) that
  keeps Latin/digit runs glued to adjacent Hangul (`iOS앱`, `2025년`).
  Two-step phonex: full Revised Romanization jamo tables → 4-char
  Soundex-family key with Korean-tuned classification; adapter name
  `"phonex-ko"`. ~60-entry stopword list (case particles deliberately
  omitted — the stemmer handles those).

- **`stringcheese-sv` — Swedish language pack.** New workspace crate.
  ~145 stopwords (ranked head + `vara`/`ha`/`bli`/`kunna`/`skola`/`vilja`
  paradigms). Snowball Swedish stemmer (Porter/Boulton `swedish.sbl`) —
  faithful to the reference spec, no postlude (contrary to some common
  descriptions). German-style R1 adjusted to ≥3. Three-step cascade:
  36-entry main-suffix unconditional delete + conditional `s` (16-char
  valid-s-ending set `bcdfghjklmnoprtvy`) + conditional `et` (with
  21-entry exclusion list protecting `paket`/`alfabet`/`raket`/
  `societet`); consonant-pair reduction on `dd`/`gd`/`nn`/`dt`/`gt`/
  `kt`/`tt` in R1; other-suffix `lig`/`ig`/`els` delete + `öst → ös` +
  `fullt → full`. **`y` is both a vowel and an s-ending consonant** per
  Snowball's spec — tested explicitly. PHONEX-sv encoder (`"phonex-sv"`)
  with sj-family cluster fold (`sj`/`stj`/`skj`/`sch`/`sk` before front
  vowels → `S`), tj-family palatal (`tj`/`kj`/`k` before front vowels
  → `C`), vowel folds `å → o`, `ä → e`, `ö → e`. 37 Snowball pairs +
  20 PHONEX pairs. 80 tests total. Finland-Swedish, Norwegian/Danish
  siblings deferred to their own packs.

- **`stringcheese-no` — Norwegian (Bokmål) language pack.** New workspace
  crate. **Registered as `"nb"`** (Bokmål-specific), NOT `"no"`
  (macrolanguage) — leaves room for a future `stringcheese-nn` (Nynorsk)
  sibling to register `"nn"` without either pack shadowing the other.
  A `macrolanguage_no_is_not_registered_by_this_pack` test locks this
  in. ~177 stopwords (Bokmål core + retained Nynorsk-flavored high-
  frequency function words for Snowball parity). Snowball Norwegian
  stemmer per official spec: R1 rules; suffix cascades `-heter`/`-arna`/
  `-etene`/`-ande`/`-a`/`-e`/`-et`/`-en`/`-ene`/`-hetens`/`-ens`/`-ers`;
  `-s` genitive with specific consonant preceding. PHONEX-no encoder
  (`"phonex-no"`) with `skj`/`sk` before front vowels → `S`, `kj`/`k`
  before front vowels → `C`, `å → o`, `æ → e`, `ø → e`. 35 Snowball
  pairs + 20 PHONEX pairs. Nynorsk (`stringcheese-nn`), Danish
  (`stringcheese-da`), Icelandic (`stringcheese-is`) deferred.

- **`stringcheese-fi` — Finnish language pack.** New workspace crate.
  **First Uralic (non-Indo-European) pack.** Finnish morphology is famously
  agglutinative — a single word can carry 5-10 morphemes with vowel
  harmony rules. ~172 stopwords. Snowball Finnish stemmer per official
  spec — the longest Snowball algorithm in the workspace. Steps: mark
  R1/R2; remove particles (`-kin`/`-kaan`/`-han`/`-ko`/`-ni`/`-si`/
  `-mme`/`-nne`); possessive-suffix removal; case-ending removal
  (Finnish has 15 grammatical cases); verb-personal-ending removal;
  consonant-gradation restoration; undouble-consonant. **Vowel harmony
  encoded in the suffix table** (not as a runtime predicate) — every
  harmony-sensitive suffix listed in both back (`-ssa`/`-lla`/`-kaan`/
  `-ko`/`-han`/`-pa`/`-nsa`) and front (`-ssä`/`-llä`/`-kään`/`-kö`/
  `-hän`/`-pä`/`-nsä`) variants across all six steps' tables. Longest-
  literal-match check IS the harmony check. Finnish `y` treated as a
  front rounded vowel /y/ (critical for R1/R2 region computation).
  PHONEX-fi encoder (`"phonex-fi"`) with long-consonant collapse
  (`ll → L`, `kk → K`) and long-vowel collapse. Traced longest sample:
  `yliopistossanikin` (17 chars, "in my university also") → `yliopisto`
  (9 chars) — inessive + 1sg possessive + clitic particle cascade.
  33 Snowball pairs + 20 PHONEX pairs. Estonian (`et`), Sami packs,
  full-lexicon consonant-gradation reversal, compound-word splitting
  deferred.

- **`stringcheese-hu` — Hungarian language pack.** New workspace crate.
  Second Uralic pack (after Finnish); related but structurally distinct.
  ~196 stopwords. Snowball Hungarian stemmer implemented as an iterated
  longest-match strip over a unified surface-form table that merges the
  reference algorithm's instrumental/case/owned/owner/plural/verb-suffix
  steps into a single pass (rationale: phased approach can over-strip
  a shorter cross-category match; unified longest-match resolves
  ambiguity uniformly). R1 region guard + 2-char min-stem floor.
  **Vowel harmony encoded in the suffix table** — every case-ending
  listed in each harmony variant: `-ban`/`-ben` inessive, `-ba`/`-be`
  illative, `-nak`/`-nek` dative, `-hoz`/`-hez`/`-höz` allative triplet,
  `-val`/`-vel` instrumental with 16 doubled-consonant assimilation
  variants (`-Xal`/`-Xel` for X ∈ {b,c,d,f,g,h,j,k,l,m,n,p,r,s,t,z}),
  `-ért`/`-ig`/`-kor`/`-ként` non-harmonizing cases, `-t`/`-at`/`-et`/
  `-ot`/`-öt` accusative. **`MAX_STRIP_ITERATIONS = 1`** prevents
  cascade over-stemming. Bare `-t`/`-tek`/`-tok`/`-tök` deliberately
  excluded to prevent loanword over-stripping. PHONEX-hu encoder
  (`"phonex-hu"`) with Hungarian digraphs `cs → C`, `dz → Z`, `dzs → J`,
  `gy → G'`, `ly → J`, `ny → N'`, `sz → S`, `ty → T'`, `zs → Z'`
  (primed placeholders keep related sounds distinguishable). 34 Snowball
  pairs (covering inessive/illative/sublative/dative/adessive/elative/
  delative/ablative harmony pairs) + 19 PHONEX pairs.

- **`stringcheese-el` — Greek language pack.** New workspace crate.
  **First Greek-script pack.** 222 stopwords (205 unique, accent-stripped,
  non-final-sigma form). Snowball Greek stemmer per official spec, with
  a preprocessing step that strips accents from vowels (ά→α/έ→ε/ή→η/
  ί→ι/ό→ο/ύ→υ/ώ→ω) and folds final sigma (ς→σ) before the multi-step
  suffix cascade. **Final sigma fold at every entry point** — stemmer
  preprocessing, phonetic encoder, `is_stopword` — rationale: `ς` and
  `σ` are positional variants of the same letter. All arithmetic on
  `Vec<char>` (Greek letters are 2 bytes per UTF-8 char). Phonetic
  encoder: **ISO 843 transliteration** to Latin (adapter `"iso-843-el"`):
  `α→a`, `β→v`, `γ→g`, `δ→d`, `ζ→z`, `η→i`, `θ→th`, `κ→k`, `λ→l`,
  `μ→m`, `ν→n`, `ξ→x`, `π→p`, `ρ→r`, `σ/ς→s`, `τ→t`, `υ→y`, `φ→f`,
  `χ→ch`, `ψ→ps`, `ω→o`. Handles diphthongs (`αι→ai`, `ει→ei`, `οι→oi`,
  `ου→ou`, `γγ→ng`) and dialytika (`ϊ`, `ϋ`, `ΰ`). Registered as
  `"el"`. 32 Snowball pairs + 33 transliteration pairs (all 24 letters
  + final sigma + all 7 accented vowels). Ancient Greek (`stringcheese-grc`),
  Katharevousa, polytonic support, Coptic sibling, ELOT 743 alternative
  transliteration deferred.

- **`stringcheese-hi` — Hindi language pack.** New workspace crate.
  **First Devanagari-script pack** (3 bytes per UTF-8 char — the first
  script wider than Cyrillic's 2 bytes). ~149 stopwords in Devanagari.
  `HindiNormalizer` with `with_devanagari_digit_folding` (fold `०-९` to
  `0-9`) and `with_nukta_stripping`. Light stemmer with gender/number
  markers, verb tense endings. Bare postpositions (`-का`/`-की`/`-के`)
  and single-scalar matras (`ि`, `ु`, `ई`, `ए`) deliberately **excluded**
  from the stemmer table — they cause real over-stemming
  (`लड़का → लड़` instead of `लड़क`; `बच्चे → बच्` instead of `बच्च`),
  documented as a conservative choice. **IAST transliteration** (International
  Alphabet of Sanskrit Transliteration, adapter `"iast-hi"`) with
  **one-scalar-lookahead state-machine schwa handling**: every base
  consonant queued rather than emitted; the encoder inspects the next
  scalar to decide — virama `्` drops the schwa (`क्` → `k`), matra
  produces vowel replacement (`कि` → `ki`), combining mark keeps the
  schwa (`हैं` → `haiṃ`), nukta swaps the base to its nukta variant
  (`ज़` → `za`). Convention: Sanskrit-style explicit-schwa retention
  (`राम` → `rāma`, `कमल` → `kamala`); Modern-Hindi context-dependent
  schwa deletion is out of scope, deferred to a future `stringcheese-hi-morph`.
  Tokenizer handles danda `।` (U+0964). 20 stemmer pairs + 52 transliteration
  pairs (all 33 classical consonants + 10 primary vowels + digits +
  combining marks + word examples). Marathi (`mr`), Sanskrit (`sa`),
  Nepali (`ne`), Bengali/Gurmukhi/Gujarati/Oriya/Tamil/Telugu/Kannada/
  Malayalam sibling packs deferred; ITRANS/Harvard-Kyoto/SLP1/ISO 15919
  alternate romanization adapters deferred.

- **`stringcheese-tokenizer-bpe`: HuggingFace WordPiece model support.**
  Adds the second-largest deferred model type from the wave-9 HF parser.
  Unlocks BERT / DistilBERT / RoBERTa / ALBERT / MobileBERT tokenizer.json
  parsing end-to-end. New `wordpiece` module: `WordPieceTokenizer` with
  `vocab: HashMap<String, u32>`, `unk_token_id`, `continuing_subword_prefix`
  (default `"##"`), `max_input_chars_per_word` (default 100). Encoding is
  greedy longest-match from left with `##` continuation prefix; first
  subword unprefixed, subsequent subwords prefixed. Whole word becomes
  `[UNK]` if any subword fails to match. New `HfTokenizer` enum
  (`#[non_exhaustive]`, boxed BpeTokenizer variant to keep footprint):
  ```
  pub enum HfTokenizer {
      Bpe(Box<BpeTokenizer>),
      WordPiece(WordPieceTokenizer),
  }
  ```
  New `to_tokenizer` dispatcher + `to_wordpiece_tokenizer` sibling.
  **Backwards-compatible**: existing `to_bpe_tokenizer` callers still
  work; a non-BPE model now surfaces `HfConversionError::UnsupportedModelForBpe`.
  **`BertPreTokenizer` implemented inline** — whitespace split followed
  by per-word punctuation split. Reference test: `"unaffable"` → `["un",
  "##aff", "##able"]`. BERT-shape synthetic tokenizer.json integration
  test with BertPreTokenizer + TemplateProcessing + `[CLS]`/`[SEP]`.
  33 new tests. Deferred: `Unigram`, `WordLevel` models; `BertNormalizer`
  (lower-case + accent-strip + Chinese-char handling); `Punctuation`/
  `Metaspace`/`CharDelimiterSplit`/`Digits`/`UnicodeScripts`/`FixedLength`
  standalone pre-tokenizers; `TemplateProcessing` splice on the WordPiece
  runtime path (config parses, callers can splice manually).

- **`stringcheese-vi` — Vietnamese language pack.** New workspace crate.
  ~181 single-syllable stopwords (Vietnamese orthography writes each
  syllable as a whitespace-separated word; multi-syllable compounds like
  `chúng tôi` are covered by their component syllables). `VietnameseNormalizer`
  builder with three opt-in flags: NFC canonicalization (default via
  `unicode-normalization::is_nfc` fast-path returning `Cow::Borrowed`),
  `with_strip_tone_marks(bool)` (remove grave/acute/hook/tilde/dot-below
  but **preserve letter modifiers** `ă â đ ê ô ơ ư`), `with_strip_all_diacritics(bool)`
  (fold everything to ASCII including `đ → d`). **Linguistic distinction**:
  tone marks are suprasegmental (pitch); letter modifiers change the
  segmental phoneme (`a`/`ă`/`â` are different vowels, `d`/`đ` are different
  consonants). NFC default because the web overwhelmingly delivers Vietnamese
  in NFC and every input method (Telex, VNI, VIQR) produces NFC output.
  Identity stemmer (Vietnamese is analytic — no inflection). PHONEX-vi
  encoder (`"phonex-vi"`) with digraph rewrites `ng → N`, `nh → N`,
  `ph → F`, `kh → K`, `tr → T`, `ch → X`, `qu → K`, `gi → Y`, `gh → G`.
  22 PHONEX pairs + 33 normalizer pairs (7 NFC + 17 tone-strip + 15 full-strip).

- **`stringcheese-he` — Hebrew language pack.** New workspace crate.
  **Second RTL pack** (after Arabic). ~130 stopwords in Hebrew script.
  `HebrewNormalizer` builder with four opt-in flags: `with_strip_niqqud`
  (default true, removes vowel points U+05B0..=U+05BC/U+05BE/U+05BF/etc.),
  `with_strip_cantillation` (default true, removes te'amim U+0591..=U+05AF),
  `with_final_form_folding` (default false — final letters ך/ם/ן/ף/ץ
  are semantically meaningful position markers), `with_strip_hebrew_punctuation`
  (default false, maqaf/geresh/gershayim). Light suffix-stripping stemmer:
  7 single-letter prefixes (`ה ו ב כ ל מ ש`) + common suffixes (`-ים`,
  `-ות`, `-ה`, possessives, past-tense endings). **Aggressive** — no
  lexicon awareness, so `בית` (starts with `ב`) and `כתבתי` (starts
  with `כ`) can over-strip; documented. Simplified ISO 259 transliteration
  encoder (`"iso-259-he"`) — single-character ASCII bijective mapping.
  22 base + 5 finals covered by dedicated coverage assertions. Root-and-
  pattern morphological analysis, verb-binyan awareness, Yiddish (`yi`),
  Ladino (`lad`), Biblical-vs-Modern-Hebrew tuning deferred.

- **`stringcheese-sk` — Slovak language pack.** New workspace crate.
  Sibling to Czech; ~90% morphology overlap with distinct function-word
  inventory and Slovak-specific letters `ä ĺ ľ ŕ ô` (drops Czech's
  `ř ě ů`). ~237 stopwords. Light stemmer — no canonical Snowball Slovak.
  Divergences from Czech: infinitive `-ť` not `-t` (`-ovať`/`-ať`/`-iť`/
  `-ieť`/`-núť`); Slovak present-tense paradigm `-ujem`/`-uješ`/`-uje`
  vs Czech's `-uji`/`-uješ`; past-tense plural is `-ovali` only (no
  gender split like Czech's `-ovali`/`-ovaly`); masculine instrumental
  `-om` not `-em`; RV vowel set adds `ä`/`ô`/`ĺ`/`ŕ`, drops `ě`/`ů`.
  PHONEX-sk encoder (`"phonex-sk"`) with Slovak-tuned folds: `ľ → L`
  (Slovak-only palatal), `ĺ → L`/`ŕ → R` (Slovak-only long syllabics),
  `ä → E` (open-front /æ/ closer to `e` phonetically), `ô → O` (diphthong
  marker). 40 stemmer pairs + 17 PHONEX pairs. 83 tests total.

- **`stringcheese-bg` — Bulgarian language pack.** New workspace crate.
  **Third Cyrillic pack** (after Russian and Ukrainian). ~236 stopwords.
  Snowball Bulgarian stemmer (Nakov 2003) with **definite-article
  stripping as the signature first step** — Bulgarian's article is a
  postposed suffix (`-ият`/`-ия` masc long-adj, `-ата`/`-ото` fem/neut
  long-adj, `-ите` plural long-adj, `-ът`/`-ят` masc noun, `-та` fem
  noun, `-то` neut noun, `-те` plural noun), not a separate word like
  English "the". `книгата → книг` and `човекът → човек` collapse to the
  bare forms. Then plural markers (`-ове`/`-еве`), verb/l-participle
  endings, and final bare-vowel strip in R1 (`а е и о у я ю ъ` —
  **`ъ` is a vowel /ɤ/ in Bulgarian**, not a hard-sign glyph as in
  Russian). GOST 7.79-B transliteration tailored to Bulgarian phonology:
  **`щ → sht`** (Bulgarian /ʃt/ cluster; Russian's `щ` is long /ʃː/
  rendered `shh`), **`ъ → a`** (Bulgarian's full vowel), `х → h`,
  `ц → ts` per convention. Adapter `"gost-7.79-b-bg"`. 33 Snowball
  pairs (12 exercising the article suffixes) + 22 transliteration pairs
  (all 30 letters covered).

- **`stringcheese-zh` — Chinese language pack (minimal).** New workspace
  crate. **Character-based tokenizer only** — no dictionary, no jieba-style
  word segmentation. Documented design commitment matching the Japanese
  pack's ethos: dictionary-driven segmentation is out of the offline-first
  / wasm-friendly envelope. Every CJK character becomes its own token
  (matches BERT's Chinese preprocessing philosophy: `你好` → `["你","好"]`,
  `中文hello123` → `["中","文","hello","123"]` — Han split, Latin/digit
  runs stay together). ~93 stopwords (single-character function words:
  的, 了, 在, 是, ...). Identity stemmer (Chinese is fully analytic —
  no inflection). **Pinyin encoder** (`"pinyin-zh"`) with a curated
  ~1092-character subset covering ~85%+ running-text coverage; unknown
  Han encodes as `?`. Simplified Chinese target; Traditional characters
  tokenize the same but stopword/pinyin lookups miss (`stringcheese-zh-hant`
  sibling deferred). 22 tokenizer pairs + 25 pinyin pairs. Dictionary-based
  `stringcheese-zh-jieba`, Simplified↔Traditional converter, Cantonese
  Jyutping, Wade-Giles/Yale/Tongyong/Bopomofo, tone-preserving pinyin
  deferred.

- **Slavic-Metaphone wiring in `ru`/`uk`/`sr` language packs.** Wires
  the wave-9 `stringcheese-phonetic::SlavicMetaphone` encoder into the
  three existing Slavic packs as an opt-in alternate. New `slavic-metaphone`
  feature (default off) per pack; new `RussianPhoneticChoice` /
  `UkrainianPhoneticChoice` / `SerbianPhoneticChoice` enum field;
  new `with_slavic_metaphone_encoder()` / `with_default_encoder()` const
  constructors; new sibling constants `RUSSIAN_WITH_SLAVIC_METAPHONE`,
  `UKRAINIAN_WITH_SLAVIC_METAPHONE`, `SERBIAN_WITH_SLAVIC_METAPHONE`.
  **Default behavior preserved** — `RUSSIAN`, `UKRAINIAN`, `SERBIAN`
  constants still return their existing transliteration encoders.
  Cross-pack equivalence test: 7 pairs (Chekhov, Petar, Ivan, Nikola,
  Milan, Volkov, Bratislava) × 6 equalities each = 42 assertions verify
  all three pack encoders agree across Cyrillic and Latin spellings.
  Uniform adapter name `"slavic-metaphone-2026"` across packs for
  cross-pack introspection. Pack types are no longer strictly zero-sized
  (small enum field added) but stay `Copy`/`Clone`/`Debug`/`Default`/
  `PartialEq`/`Eq`/`Hash` via derive-with-`#[default]`.

- **`stringcheese-tokenizer-bpe`: HuggingFace Normalizers and
  TemplateProcessing.** The wave-9-flagged "highest-impact next unlock"
  for tokenizer.json byte-parity. **Normalizer support**: NFC, NFD,
  NFKC, NFKD (via `unicode-normalization`), Lowercase, `Replace{String
  pattern}`, `Strip{left,right}`, `Prepend`, `Sequence` composition.
  New `normalizer` module exposes `Normalizer` enum + `normalize(text,
  normalizer) -> String`. New `hf-normalizer` sub-feature that
  `hf-tokenizer` now implies (JSON callers get NFC/NFD out of the box).
  Pipeline composition: normalizer runs BEFORE pre-tokenizer (HF's
  semantic order). Deferred: `Bert`, `Nmt`, `Precompiled` (SentencePiece),
  `Replace{Regex pattern}`. **TemplateProcessing support**: recognizes
  the post-processor variant with `single`/`pair` templates and
  `special_tokens` map. `encode_with_special(text, add_special_tokens)`
  provides opt-out. Llama-family tokenizers with `<|begin_of_text|>` /
  `<|eot_id|>` BOS/EOS injection now work end-to-end. Deferred:
  `BertProcessing`, `RobertaProcessing`, `ByteLevel` post-processor,
  `Sequence` multi-processor. Synthetic Llama-3-shape config test
  verifies `encode("hello") == [128000, 7]` with `special_mask [true,
  false]`; `encode_with_special(_, false) == [7]`. 36 new tests.

- **`stringcheese-pl` — Polish language pack.** New workspace crate.
  ~282 stopwords (`być / mieć / móc / chcieć` paradigms + full pronoun /
  preposition / conjunction inventory; carries both ASCII-typed and
  diacritic-correct variants like `az` / `aż`). Light stemmer — no
  canonical Snowball Polish (Stempel/Egothor requires a large trained
  transducer, outside the per-crate offline envelope). Single-pass
  longest-match stripper over a unified noun/adjective/verb/adverb
  table with RV region floor and 2-char min-stem guard. `się` left to
  the tokenizer (free-standing orthographic word, not a suffix).
  Polish-specific letters `ą ć ę ł ń ó ś ź ż` handled. PHONEX-pl
  encoder (`"phonex-pl"`): nasal-vowel folding `ą→a`/`ę→e`, `ó→u`
  conflation (same modern /u/ phoneme), `ż`/`ź` merger (both → `Z`),
  digraph handling (`sz→S`, `cz→C`, `rz→R`, `ch→K` before silent-H
  strip). Case-fold override uses Unicode `to_lowercase` (default
  `eq_ignore_ascii_case` would miss `Ą → ą`). 36 Snowball reference
  pairs + 19 PHONEX pairs. 83 tests total.

- **`stringcheese-cs` — Czech language pack.** New workspace crate.
  ~275 stopwords (`být` copula's ~30 forms with `ne-` negation, plus
  gender/number variants of possessives and demonstratives — higher
  than the ~150-200 target because Czech function-word morphology is
  rich). Light stemmer — no canonical Snowball Czech. Covers common
  noun/adjective endings (`-ovi`, `-ova`, `-ovy`, `-ové`, `-ami`,
  `-emi`, `-ám`, `-ým`, `-ého`, `-ých`), single-vowel case endings,
  and verb endings (`-oval`, `-ovala`, `-ovat`, `-uji`, `-uje`).
  Deliberately conservative to avoid over-stemming without a lexicon.
  Czech-specific letters `á č ď é ě í ň ó ř š ť ú ů ý ž` handled.
  PHONEX-cs encoder (`"phonex-cs"`): haček folds `č/š/ž/ř/ď/ť/ň →
  C/S/Z/R/D/T/N`, long-vowel folds `á/é/í/ó/ú/ý → A/E/I/O/U/Y`,
  `ů → U`, `ě → E`, `ch → X`, silent `h`. 33 stemmer reference pairs
  + 17 PHONEX pairs. 99 tests total. Consonant alternation
  (`ruka → ruce` — velar/palatal `k/c/č`, `h/z/ž`) stripped-but-not-
  reversed; reversing needs a lexicon. Slovak sibling pack, aggressive
  Dolamic-Savoy derivational variant, ISO 9-cs alternate adapter deferred.

- **BCP-47 subtag fallback in `stringcheese-lang::registry`.** Extends
  `registry::language(code)` to walk the fallback chain right-to-left
  on hyphen boundaries: `"pt-BR"` → `"pt"`, `"sr-Cyrl-RS"` → `"sr-Cyrl"`
  → `"sr"`, `"pt-BR-x-informal"` → all the way down to `"pt"`.
  Case-insensitive throughout (BCP-47 comparisons are case-insensitive
  regardless of script/region conventions). New
  `registry::language_exact(code)` preserves pre-existing exact-match
  semantics for callers who need them. **Fully backwards-compatible**:
  every code that used to resolve still resolves to the same pack; codes
  that used to return `None` either still return `None` (no primary-
  language match) or now succeed via fallback. Grandfathered irregular
  tags (`i-klingon`, `en-GB-oed`), private-use `-x-` subtags, and
  extension `-u-` subtags all degrade to the same right-to-left walk
  (no IANA-registry consultation). 10 new unit tests + 2 proptests
  (exact-match wins over fallback; impl agrees with hand-rolled strip
  loop).

- **`stringcheese-phonetic`: Slavic-Metaphone encoder.** New shared
  cross-Slavic sound-alike encoder covering 12 Slavic languages across
  both Cyrillic and Latin scripts (Russian, Ukrainian, Serbian,
  Bulgarian, Belarusian, Macedonian on the Cyrillic side; Polish,
  Czech, Slovak, Croatian, Bosnian, Slovenian on the Latin side).
  **Both scripts land directly in a shared 19-class ASCII alphabet**
  (14 consonant classes + 5 vowel classes) via a single-pass classifier
  with digraph lookahead — no intermediate Latin transliteration step.
  Handles Polish `sz/cz/rz/dź/dż`, ASCII transliteration `sh/zh/kh/ts`,
  Czech/Polish `ch`, and Serbian Latin `dž` at the classifier level.
  **Deliberate cross-Slavic collapses**: `g → H` class (Prague ↔ Прага
  match despite the g/h dialect divide), `ch → C` (Russian /tʃ/
  convention wins; `kh` remains for /x/), sibilant voice pairs
  `š/ž → X`, `s/z → S`, `č/dž → C` (classic Metaphone), palatalization
  diacritics fold to base (`ń→N`, `ť→T`, `ď→T`, `đ→C`, `ł→L`), long
  vowels fold to short, Polish nasals `ą/ę` decompose to `A+N`/`E+N`,
  Russian and Ukrainian `щ` both fold to `X`. Default `max_length` 8;
  vowels dropped after the initial by default (Metaphone convention),
  opt-in `include_vowels`. `SlavicMetaphone` type + `SlavicMetaphoneOptions`
  builder + `slavic_metaphone()` free function. 24 cross-language
  reference pairs verified (`Чехов`/`Chekhov`, `Kraków`/`Краків`,
  `Praha`/`Прага`, `Београд`/`Beograd`, ...). Beider-Morse-Slavic
  (much larger rule engine) and language-tailored variants deferred.
  Language packs deliberately not rewired — a future wave can expose
  Slavic-Metaphone as an alternate encoder alongside their existing
  transliterations.

- **`stringcheese-manip`: `wrap_at_width` and `reflow` via UAX #14.**
  New `wrap` module using the wave-8 UAX #14 line-break iterators from
  `stringcheese-unicode::line_breaks`. Public API: `wrap_at_width(text,
  width) -> Vec<String>`, `wrap_at_width_borrowed(text, width) ->
  Vec<&str>` (zero-copy), `fill(text, width) -> String`, `reflow(text,
  width) -> String` (paragraph-aware). `WrapOptions` builder with
  `.width()`, `.break_words()`, `.initial_indent()`, `.subsequent_indent()`.
  **Width unit: `unicode-width`** — CJK ideographs = 2 cols, combining
  marks / ZWJ = 0 cols (fixed-cell terminal renderer convention).
  Feature-gated on new `line-breaking` feature (default on), forwarding
  through to `stringcheese-unicode/line-breaking` + pulling in
  `unicode-width` (`no_std`, pure data). Wasm-size baseline unchanged
  (probe doesn't reference `wrap`, so LTO strips both deps from the
  measured build); documented transitive opt-in cost ~22 KB
  (unicode-linebreak) + ~10-15 KB (unicode-width). Default matches
  Python `textwrap`: oversized words overflow their line; `break_words(true)`
  opts into character-level force-splits on valid UTF-8 boundaries.
  `reflow` consumes single terminators as spaces and treats 2+
  consecutive newlines as paragraph boundaries. 34 unit tests covering
  the full golden matrix (empty / short / exact-width / oversized /
  mandatory-break preservation / CRLF / multibyte UTF-8 boundary
  safety / CJK double-width / combining marks / paragraph reflow).

- **`stringcheese-tokenizer-bpe`: HuggingFace ByteLevel pre-tokenizer
  and decoder.** Adds the largest single deferred feature from the
  wave-8 HF parser landing — GPT-2 and Llama-family tokenization now
  reachable end-to-end from `tokenizer.json`. New `byte_level` module
  with `BYTES_TO_CHARS: [char; 256]` bijective mapping (printable ASCII
  0x21..=0x7E, plus 0xA1..=0xAC / 0xAE..=0xFF unchanged; unprintable
  bytes → Unicode 256..=(256+n)); `CHARS_TO_BYTES` inverse; `encode_bytes`
  / `decode_chars` functions. Extends `HfTokenizerConfig` to recognize
  `ByteLevel` pre-tokenizer variants and compose them with Split(Regex)
  in the correct semantic order (ByteLevel encoding BEFORE regex split
  for GPT-2 semantics). Extends `HfDecoder` to apply ByteLevel inverse
  in decode paths. **On the ByteLevel path, BPE pieces are seeded per
  char (not per byte)** so multi-byte encoded chars like `Ġ` stay
  atomic — matches HF's own char-based BPE seeding; non-ByteLevel
  paths (tiktoken, raw-byte BPE) keep per-byte seeding unchanged.
  GPT-2 `tokenizer.json` now fully parseable end-to-end: `"hello"` →
  `[Ġhello]`, `"hello world"` → `[Ġhello, Ġworld]`; canonical HF
  `["ĠHello", "Ġworld"]` example matches byte-for-byte. 39 new tests
  (10 byte_level, 7 hf ByteLevel-focused, plus config integration).
  Deferred: WordPiece / Unigram / WordLevel models; all Normalizers
  (NFC/NFD is the next highest-impact unlock); Post-processors
  (`TemplateProcessing` for BOS/EOS injection in Llama-family);
  `trim_offsets` on ByteLevel.

- **`stringcheese-uk` — Ukrainian language pack.** New workspace crate.
  **Second Cyrillic-script pack** (after Russian). ~218 stopwords in
  Cyrillic. Light suffix-stripping stemmer (no canonical Snowball
  Ukrainian exists) with an RV region guard and a theme-vowel context
  predicate on past-tense endings (`-в`, `-ла`, `-ло`, `-ли`) to prevent
  noun `столи` from misfiring as verbal. Handles extended letters `ґ`
  (U+0491), `є` (U+0454), `і` (U+0456), `ї` (U+0457) that Russian lacks;
  documents that Ukrainian also lacks Russian's `ъ`/`ы`/`ё`/`э`.
  All suffix arithmetic runs on `Vec<char>` per the wave-7 Cyrillic
  pattern. Transliteration: **GOST 7.79-2000 System B, Ukrainian
  adaptation** (adapter `"gost-7.79-b-uk"`). Ukrainian-specific
  divergences from the Russian mapping: `г → h`, `ґ → g` (Russian
  collapses both), `є → ye`, `ї → yi`, `и → y` (Ukrainian и is /ɪ/),
  `х → kh`, `щ → shch` (vs Russian's `shh`). Registered as `"uk"`.
  39 Snowball reference pairs + 21 transliteration pairs (all 33 letters
  exercised). Verb-aspect prefix stripping, ISO 9 System A / Ukrainian
  government 2010 transliterations, typographic apostrophe U+2019
  recognition, and Belarusian/Bulgarian/Macedonian sibling packs deferred.

- **`stringcheese-sr` — Serbian dual-script language pack.** New workspace
  crate. **First dual-script pack** — Serbian is written in both Cyrillic
  (Vukovica) and Latin (Gaj's), and both scripts are equally valid.
  ~200 stopwords across two lists (`STOPWORDS_CYR` + `STOPWORDS_LAT`);
  `is_stopword` looks up either script. Bijective `to_latin()` /
  `to_cyrillic()` transliteration in a new `scripts` module: `нј → nj`,
  `љ → lj`, `џ → dž`, `ђ → đ`, `ч → č`, `ц → c`, `ш → š`, `ž → ž`,
  `ћ → ć`, `ј → j` (and reverse, correctly handling `nj`/`lj`/`dž`
  digraph reassembly). **Snowball approach: normalize Cyrillic → Latin
  via `to_latin`, run one Latin suffix table, transliterate back.**
  Documented in `snowball.rs`. None of the ~65 shipped suffixes begin
  with `j`/`ž`, so stripping cannot split `lj`/`nj`/`dž`. Ekavian vs.
  ijekavian treated as distinct opaque forms; both variants of divergent
  stopwords (`gde`/`gdje`, `uvek`/`uvijek`) live in each list.
  Phonetic encoder: ships `to_latin` as `"sr-latin"` — unifies records
  under either script by lowercase Latin key. Registered as `"sr"`.
  30 Snowball pairs + 22 script conversion pairs (all digraphs, both
  cases). Bijection caveat on plain-Cyrillic `лј`/`нј`/`дж` non-digraph
  sequences captured as a proptest regression seed. Croatian, Bosnian,
  Montenegrin (adds `с́`/`з́`) sibling packs, explicit ijekavian → ekavian
  fold adapter, consonant alternation modelling (`k → č`, `g → ž`) deferred.

- **`stringcheese-fa` — Persian (Farsi) language pack.** New workspace
  crate. Arabic-script but Persian-tuned. ~177 stopwords. Handles Persian
  additions to the Arabic alphabet: **پ چ ژ گ**. `PersianNormalizer`
  builder with six opt-in flags: `with_arabic_yeh_to_persian` (default
  true, Arabic `ي` U+064A → Persian `ی` U+06CC), `with_arabic_kaf_to_persian`
  (default true, `ك` U+0643 → `ک` U+06A9), `with_western_digits` (default
  false, Extended Arabic-Indic `۰-۹` U+06F0..=U+06F9 → Western `0-9`),
  `with_strip_zwnj` (default false — ZWNJ U+200C is semantic in Persian
  compounds), `with_strip_tatweel` (default true), `with_heh_yeh_normalization`
  (default false, `ۀ` U+06C0). **Tokenizer treats ZWNJ as word-internal
  by default** — compound words like `می‌روم` stay as one token; caller
  opts into ZWNJ-stripping normalization for search contexts. Light
  suffix-stripping stemmer for nominal suffixes: `-ها`, `-های`, `-تر`,
  `-ترین`, `-ام/-ای/-اش/-مان/-تان/-شان`. Phonetic encoder:
  **Persian-Buckwalter** — the classical Buckwalter Arabic mapping
  extended with the four Persian consonants (`پ→p`, `چ→c`, `ژ→J`, `گ→g`).
  Because `g` collides with Arabic-Buckwalter's ghain, ghain is
  reassigned to `G` for invertibility. Persian yeh/kaf and Arabic yeh/kaf
  both encode to `y`/`k`; inverse decodes to Persian forms so
  `inverse(encode(x))` normalizes Arabic-yeh/kaf to Persian on round-trip.
  Adapter `"persian-buckwalter"`. Registered as `"fa"`. 20 stemmer
  reference pairs + 34 transliteration pairs (one per Persian letter).
  Dari (Afghan Persian), Tajik-Cyrillic, ezafeh detection, verb morphology
  (`می-`/`ب-` prefixes), compound-verb decomposition deferred.

- **UAX #14 line breaking.** New `line_breaks` module in
  `stringcheese-unicode`, feature-gated as `line-breaking` (default on,
  individually toggleable). `LineBreak` enum (`Mandatory` for `\n`/`\r\n`/
  paragraph separators; `Allowed` for soft-wrap opportunities).
  `LineBreakSequence<'a>` iterator yielding `(byte_offset, LineBreak)`
  pairs; `line_breaks(text)` helper. Complements the wave-7 UAX #29
  word + sentence segmentation. Dep chosen: **`unicode-linebreak 0.1`**
  over `xi-unicode` — 1:1 `BreakOpportunity::{Mandatory, Allowed}` enum
  map to the public API, `#![no_std]`-clean, and actively maintained
  (`xi-unicode` is a byproduct of the archived xi-editor). Wasm-size:
  tracked baseline unchanged (default probe doesn't enable the new
  feature); new opt-in `unicode-with-line-breaking` probe measures
  +22 KB standalone cost. Downstream wiring deliberately deferred —
  this is groundwork for a future `stringcheese-manip::wrap` helper.
  Locale-tailored break rules (`line-break: strict/normal/loose`),
  Southeast Asian dictionary-based break detection (Thai/Lao/Khmer/Myanmar),
  and `word-break: break-all`-style forced-break iterator deferred.

- **`stringcheese-tokenizer-bpe`: HuggingFace `tokenizer.json` parser.**
  New optional `hf-tokenizer` feature (adds `serde 1` + `serde_json 1`,
  gated behind the feature). New `hf` module with:
  * `HfTokenizerConfig` — serde-derived top-level struct matching the
    tokenizer.json shape (version, truncation, padding, added_tokens,
    normalizer, pre_tokenizer, post_processor, decoder, model).
  * `parse_tokenizer_json(json: &str) -> Result<HfTokenizerConfig, HfParseError>`
    with structured errors.
  * `to_bpe_tokenizer(config: &HfTokenizerConfig) -> Result<BpeTokenizer, HfConversionError>`.
  Supported: **BPE model** (vocab + merges in both shipped shapes — pair
  form `[["a","b"]]` and space-joined `["a b"]`), **Split pre-tokenizer
  with Regex pattern** (routed through the wave-6 `RegexPreTokenizer`),
  single-child `Sequence` pre-tokenizer wrappers, **added special
  tokens** (`special: true` → BPE special tokens; non-special → base
  vocab). Deferred (returned as `HfConversionError::Unsupported` with
  named feature): `WordPiece`/`Unigram`/`WordLevel` models; `ByteLevel`
  (GPT-2 style byte remapping — requires a whole post-processing layer),
  `Whitespace`, `WhitespaceSplit`, `Punctuation`, `Metaspace`,
  `CharDelimiterSplit`, `BertPreTokenizer`, `Digits`, `UnicodeScripts`,
  `FixedLength`, `Split(String)`, ambiguous multi-child `Sequence`;
  normalizer/post_processor/decoder preserved on the parsed config for
  caller inspection but not applied. Incidental change to `bpe.rs`:
  `PreTokenizerRegex` grows a `Regex(RegexPreTokenizer)` variant (gated
  on `std`) so the compiled regex from the Split(Regex) branch can be
  threaded through the encoder. Tests parse both GPT-2-shape and
  Llama-3-shape synthetic blobs (real ones deferred — GPT-2 uses
  `ByteLevel`, which is the largest single deferred feature).

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

- **`stringcheese-mr`: Marathi stemmer idempotence.** The two-phase
  design (single primary strip + iterated cleanup) shipped in the
  initial Marathi pack was not idempotent — an input like `कओतत`
  would stem to `कओत` on the first call and `कओ` on the second,
  breaking `stem(stem(w)) == stem(w)`. Fixed by iterating primary +
  cleanup together to convergence. Accepts a small class of
  over-strips (`मुलाने → मु` instead of the linguistically-correct
  `मुल`) as an unavoidable tradeoff for a lexicon-free light stemmer.

- **`stringcheese-tokenizer-bpe`: backtick doc-markdown identifiers in
  `hf.rs`.** Rust 1.97's `clippy::doc_markdown` lint fires on bare
  identifiers like `WordPiece`, `WordLevel`, and `DeepSeek` in the HF
  parser's module docstring. The HF agent ran clippy on an older
  toolchain where the lint was more permissive; the identifiers now
  carry backticks so `-D warnings` stays clean.

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

- **`stringcheese-cdc`: real vectorized Polynomial SIMD kernel via
  block reformulation.** Closes the CDC SIMD trilogy (Gear wave 7,
  Buzhash + Rabin wave 8, Polynomial now). Block reformulation:
  `state_{k+16} = state_k * PK_BLOCK + Σ bytes[i]*pk[15-i]` (mod PRIME),
  with `pk[j] = BASE^j mod PRIME` const-evaluated. Effective-slice
  truncation (shared with Rabin): for `window > 0`, only last `window`
  bytes affect the digest. Coefficient split: each 61-bit `pk` splits
  into `pk_hi ≤ 2^29` and `pk_lo = 32 bits` so `byte * pk_half` fits
  in a u64 lane; `hi_acc` and `lo_acc` accumulate independently across
  the 16-byte block, then reassemble as `hi_sum * 2^32 + lo_sum` in
  u128 for a single per-block Mersenne reduction. **All four arches
  got real kernels**: AVX2 (`_mm256_mul_epu32` VPMULUDQ, 4-lane u64),
  SSE2 (`_mm_mul_epu32` PMULUDQ — no gather needed, SSE2 baseline
  suffices, 2-lane u64), NEON (`vmull_u32` widening 32×32→64, 2-lane
  u64), wasm SIMD128 (`i64x2_mul` low-64; inputs bounded ≤32 bits so
  low-64 == widening product). Verified byte-identical to scalar on
  aarch64 host (98 CDC tests pass) and x86_64 under Rosetta (23
  polynomial tests, both AVX2 and SSE2 kernels bit-identical). AVX-512
  IFMA (`_mm256_madd52lo/hi_epu64`, fused 52-bit multiply-add for the
  61-bit Mersenne coefficient) noted as future work.

- **`stringcheese-cdc`: real vectorized Buzhash SIMD kernel.** Replaces
  the wave-6 Buzhash scaffolding with a true block-reformulation kernel
  via rotate-XOR. AVX2 backend: 4-lane u64, 16 iterations per 64-byte
  block, per-lane variable rotate via `_mm256_sllv_epi64` +
  `_mm256_srlv_epi64` + `_mm256_or_si256` with counts `[3,2,1,0]` /
  `[61,62,63,0]`, 4-bit Horner advance, `_mm256_xor_si256` fold,
  horizontal XOR via extract/xor. NEON backend: 2-lane u64, 32 iterations,
  per-lane variable rotate via `vshlq_u64` with signed count vectors
  (identity lane realised as `x|x` to keep counts in the unambiguous
  `[-63,63]` band). wasm SIMD128 backend: 2-lane u64; `u64x2_shl` is
  uniform-across-lanes so lane 0's pre-rotate factors to scalar
  `g0.rotate_left(1)` before the `u64x2(_,_)` pack. SSE2 stays scalar
  (no gather until SSE4.1, no per-lane variable shift until AVX2).
  Boundary-differential-tested at sizes 63/64/65/127/128/129 across
  windows 1/4/8/32/63/64/65/100/200 per backend + dispatcher.

- **`stringcheese-cdc`: real vectorized Rabin SIMD kernel via
  `pclmulqdq`.** Replaces the wave-6 Rabin scaffolding with a true
  block-folding kernel using carry-less multiplication in GF(2). x86_64
  SSE2 + PCLMULQDQ backend: 8-byte block folding via `_mm_clmulepi64_si128`,
  runtime-detected via `is_x86_feature_detected!("pclmulqdq")`. AVX2
  backend delegates into the SSE2 pclmul kernel (a 2-way VPCLMULQDQ path
  is future work — the intrinsic stabilized in Rust 1.89 but the
  workspace MSRV is 1.85). aarch64 NEON + AES (PMULL) backend: 8-byte
  block folding via `vmull_p64`, gated on
  `is_aarch64_feature_detected!("aes")`. wasm SIMD128 stays scalar
  (no PMULL equivalent). Verified end-to-end on native aarch64 (PMULL
  green) and under Rosetta 2 x86_64 (PCLMULQDQ green, VPCLMULQDQ absent
  as expected). Boundary tests at sizes `{0,1,7,8,9,15,16,17,31,32,33,63,
  64,65,127,128,129,511,512,513,4096}` × windows `{1,8,32,64,100,128,
  512,1024}` per backend + dispatcher.

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
