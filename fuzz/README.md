# stringcheese-fuzz

Cargo-fuzz targets for the StringCheese workspace. Deliberately kept
outside the main `[workspace]` (see the sentinel `[workspace]` block at
the top of `Cargo.toml`) because `cargo fuzz` requires a nightly
toolchain and the workspace root pins stable.

## Layout

```
fuzz/
  Cargo.toml                — standalone workspace root
  fuzz_targets/             — one `.rs` per registered [[bin]]
  seed_corpus/<target>/     — hand-crafted seeds, checked in
  corpus/<target>/          — libFuzzer's working corpus (gitignored)
  artifacts/<target>/       — libFuzzer's crash reproducers (gitignored)
```

Seed inputs live under `seed_corpus/<target>/` so they can be reviewed
in-tree; libFuzzer's evolving corpus accumulates under
`corpus/<target>/` and is gitignored (recovered from GitHub Actions
cache on the nightly job).

## Targets

### Differential / property targets (edit-distance kernels)

| Target                          | Kind         | What it checks                                                                                |
| ------------------------------- | ------------ | --------------------------------------------------------------------------------------------- |
| `levenshtein_differential`      | Differential | Full-matrix vs rolling-rows vs banded Levenshtein agree.                                      |
| `osa_differential`              | Differential | Optimal-string-alignment kernels agree on all inputs.                                         |
| `damerau_differential`          | Differential | Damerau-Levenshtein kernels agree on all inputs.                                              |
| `hamming_symmetry`              | Property     | Hamming distance is symmetric and identity-satisfying.                                        |
| `jaro_range`                    | Property     | Jaro similarity stays within `[0, 1]`.                                                        |
| `ngram_count_agreement`         | Property     | Two n-gram counters agree per gram.                                                           |
| `metric_axioms_levenshtein`     | Property     | Levenshtein satisfies identity, symmetry, and triangle inequality.                            |
| `align_score_traceback_agree`   | Property     | NW / SW `score()`, `align().score`, and sum-over-edit-script all agree under linear gaps.     |

### Parser-robustness targets (binary / JSON loaders)

| Target                     | Surface                                            | Invariant                                                                 |
| -------------------------- | -------------------------------------------------- | ------------------------------------------------------------------------- |
| `scud_load`                | `stringcheese_scud::ScudFile::from_slice`          | Arbitrary bytes → `Ok(ScudFile)` or typed `ScudError`, never panic.       |
| `scud_roundtrip`           | `ScudWriter` + `ScudFile::from_slice`              | Structured input → serialize → deserialize preserves case pairs, collation primary overrides + options blob, plural cardinal rules. |
| `hf_tokenizer_json`        | `stringcheese_tokenizer_hf::hf::parse_tokenizer_json` | UTF-8 bytes → `Ok(HfTokenizerConfig)` or typed `HfParseError`, never panic. |
| `regex_compile_and_match`  | `stringcheese_pattern_regex::Regex::{new,bytes,case_insensitive,literal}` + `Pattern::is_match` | Arbitrary bytes → `Ok(Regex)` or typed `RegexError`, never panic; `is_match` never panics on a compiled `Regex`. |
| `tiktoken_merges_parse`    | `stringcheese_tokenizer_tiktoken::builder::build_scud_from_tiktoken` | Arbitrary bytes → `Ok((vocab, merges))` or typed `String` error, never panic. |
| `escape_decode_roundtrip`  | `stringcheese_escape::{escape, unescape}` for URI / HTML / JSON / shell targets | For URI / HTML / JSON: `unescape(escape(x), g) == Ok(x)` for all valid UTF-8 `x`. Shell is encode-only (round-trip is not defined over arbitrary bytes). Neither call may panic. |
| `wit_parse`                | `wit_parser::Resolve::push_str`                        | Arbitrary bytes → `Ok(())` (well-formed WIT) or `Err(_)` (malformed), never panic. |

The parser-robustness targets are the highest-value because their
inputs (SCUD binary blobs, `tokenizer.json`) come from external
producers — corrupt or adversarial bytes have to surface as typed
errors, not panics.

Seed corpus per parser target:

* `scud_load`
  * `01_minimal_case.bin` — minimal valid SCUD (CAP_CASE, no locale, no sections)
  * `02_case_with_one_section.bin` — CAP_CASE + `sLwr` section (empty payload) + locale
  * `03_case_all_sections.bin` — CAP_CASE + every case section id (empty payloads) + locale
  * `04_magic_only.bin` — truncated to the four magic bytes
  * `05_zeros_64.bin` — 64 bytes of zeros (invalid magic, catches early rejection)
* `scud_roundtrip`
  * `01_empty_input.bin` — empty (`arbitrary` returns `IncorrectFormat`; target no-ops)
  * `02_one_case_pair.bin` — decodes to CLDR "44.1" + one `(A, a)` case pair
  * `03_all_zero.bin` — 32 zero bytes (minimal-values variant)
  * `04_max_values.bin` — 64 bytes of `0xFF` (max u32 fields, edge case)
  * `05_alternating.bin` — 96 alternating `0x5A`/`0xA5` bytes (mixed picks)
* `hf_tokenizer_json`
  * `01_minimal_bpe.json` — smallest well-formed BPE tokenizer config
  * `02_empty_object.json` — `{}` (parses; every optional field defaulted)
  * `03_invalid_json.json` — `{` (parser must return `HfParseError`, not panic)
  * `04_null.json` — `null` (top-level shape mismatch)
  * `05_small_bpe_with_merges.json` — BPE config with normalizer, pre-tokenizer, and one merge
* `regex_compile_and_match`
  * Input layout: `byte0 = mode (mod 4)` (0=`new`, 1=`bytes`, 2=`case_insensitive`, 3=`literal`), then pattern bytes, `0x00` separator, haystack bytes.
  * `01_literal_match.bin` — mode=`literal` + `hello` + `hello world` (auto-escape path, guaranteed hit)
  * `02_simple_match.bin` — mode=`new` + `^foo` + `foobar` (anchor path)
  * `03_unicode_class.bin` — mode=`new` + `\p{Nd}+` + `abc123def` (Unicode property class)
  * `04_malformed_bracket.bin` — mode=`new` + `[a-` + `abc` (compile-error path — `RegexError`, no panic)
  * `05_huge_repeat.bin` — mode=`new` + `a{999999}` + `aaa` (size-limit path — `RegexError`, no panic)
  * `06_recursive_backreference.bin` — mode=`new` + `(?P<a>foo)(?P=a)` + `foofoo` (backreferences unsupported — `RegexError`, no panic)
* `align_score_traceback_agree`
  * Input decoded via `arbitrary::Unstructured` into a `FuzzPair { a, b }` with each length bounded to `[0, 64]`.
  * `01_identical.bin` — `a == b == "ACGT"` (all-match path, score == length).
  * `02_all_different.bin` — `a = "AAAA"`, `b = "TTTT"` (all-mismatch or gap paths).
  * `03_one_empty.bin` — `a = "AAAA"`, `b = ""` (boundary column path).
  * `04_gap_heavy.bin` — `a = "AAAABBBB"`, `b = "AABBBBAA"` (out-of-phase, gap-preferring).
  * `05_short_dna.bin` — `a = "GATTACA"`, `b = "GCATGCU"` (canonical mixed-op pair).
* `escape_decode_roundtrip`
  * Input layout: `byte0 = mode (mod 4)` (0=uri, 1=html, 2=json, 3=shell), then a UTF-8 payload.
  * `01_uri_ascii.bin` — URI + `"hello world"` (space triggers the `%20` path).
  * `02_html_xss.bin` — HTML + the classic `<script>alert(1)</script>` payload.
  * `03_json_controls.bin` — JSON + `"quote:"\\newline\nend"` (mix of `"`, `\`, and newline escapes).
  * `04_shell_meta.bin` — Shell + `"it's a; rm -rf /"` (embedded quote + shell metachars; encode-only).
  * `05_empty_payload.bin` — JSON with empty payload (encode-only edge case).
  * `06_utf8_multibyte.bin` — URI + `"cafe\u{e9} \u{2603}"` (2-byte + 3-byte scalars for the multibyte path).
* `wit_parse`
  * `01_minimal_package.bin` — `package example:pkg@0.1.0;` (smallest well-formed WIT source).
  * `02_shipped_plural.bin` — an actual shipped WIT (`component/wit/plural/stringcheese-icu-plural.wit`).
  * `03_empty.bin` — zero bytes (edge case).
  * `04_brace_soup.bin` — `{{{}}}` (unbalanced braces, must return `Err`).
  * `05_malformed_ident.bin` — `package 1invalid:name;` (identifier starting with digit; must return `Err`).

**Note on vocab bytes.** The `hf_tokenizer_json` seeds ship
hand-crafted synthetic examples only. Real-vocab tokenizer.json fixtures
live under `crates/stringcheese-tokenizer-hf/tests/conformance/vocabs/`
and are deliberately kept out of the fuzz corpus — a session-standing
constraint on this repo. libFuzzer will grow the vocab-shape coverage
on its own from the synthetic seeds.

## Running locally

Cargo-fuzz needs a nightly toolchain and the `cargo-fuzz` cargo
subcommand:

```bash
rustup toolchain install nightly
cargo install cargo-fuzz --locked
```

Run a target for a fixed number of iterations. The first positional
argument is libFuzzer's *writable* corpus (findings go here); the
second is a read-only input directory it merges in on start:

```bash
mkdir -p fuzz/corpus/scud_load
cargo +nightly fuzz run scud_load \
    fuzz/corpus/scud_load fuzz/seed_corpus/scud_load -- -runs=1000

mkdir -p fuzz/corpus/hf_tokenizer_json
cargo +nightly fuzz run hf_tokenizer_json \
    fuzz/corpus/hf_tokenizer_json fuzz/seed_corpus/hf_tokenizer_json -- -runs=1000
```

Or for a fixed time budget:

```bash
cargo +nightly fuzz run scud_load \
    fuzz/corpus/scud_load fuzz/seed_corpus/scud_load -- -max_total_time=30
```

Only the writable corpus dir accumulates state across runs — the seed
corpus stays fixed. Omit the seed dir after the first run if you like;
libFuzzer will have copied the interesting seeds into the writable
corpus on merge.

Never pass `seed_corpus/<target>` as the *first* positional argument —
libFuzzer will treat it as the writable primary and write discovered
inputs into the checked-in directory.

## Continuous fuzzing

The GitHub Actions workflow `.github/workflows/fuzz-nightly.yml` runs
every target for 30 minutes each on the `nightly` cron (`0 6 * * *`
UTC) and on manual dispatch. Crashes upload as run artifacts under
`crashes-<target>/`; promote reproducers into the regression corpus
under `crates/*/proptest-regressions/` (for cross-checked properties)
or extend the seed corpus with the reduced input (for parser
robustness bugs). See `docs/DESIGN.md` § Fuzzing for the discipline.

## Adding a new target

1. Add `fuzz_targets/<name>.rs` with a `fuzz_target!(|data: &[u8]| { ... })`
   body.
2. Register it as `[[bin]] name = "<name>", path = "fuzz_targets/<name>.rs"`
   in `fuzz/Cargo.toml`.
3. Add matching `matrix.target` entry to
   `.github/workflows/fuzz-nightly.yml`.
4. Optionally, hand-craft a small seed corpus under
   `fuzz/seed_corpus/<name>/`.
5. Verify locally with `cargo +nightly fuzz run <name> -- -runs=1000`.
