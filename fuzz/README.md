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

### Parser-robustness targets (binary / JSON loaders)

| Target                     | Surface                                            | Invariant                                                                 |
| -------------------------- | -------------------------------------------------- | ------------------------------------------------------------------------- |
| `scud_load`                | `stringcheese_scud::ScudFile::from_slice`          | Arbitrary bytes → `Ok(ScudFile)` or typed `ScudError`, never panic.       |
| `hf_tokenizer_json`        | `stringcheese_tokenizer_hf::hf::parse_tokenizer_json` | UTF-8 bytes → `Ok(HfTokenizerConfig)` or typed `HfParseError`, never panic. |
| `regex_compile_and_match`  | `stringcheese_pattern_regex::Regex::{new,bytes,case_insensitive,literal}` + `Pattern::is_match` | Arbitrary bytes → `Ok(Regex)` or typed `RegexError`, never panic; `is_match` never panics on a compiled `Regex`. |

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
