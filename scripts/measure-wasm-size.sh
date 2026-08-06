#!/usr/bin/env bash
# scripts/measure-wasm-size.sh — contributor-facing reproducer for the
# CI `wasm-size` job. Rebuilds every measured crate's size-probe wasm
# and the `component/rust-host` component, applies `wasm-opt -Oz` where
# the artifact is supported (component-model binaries are not — see the
# note below), and prints a size summary that mirrors what CI reports.
#
# Read-only: prints to stdout, writes only to `wasm-size-probes/target`
# (probe builds), `component/rust-host/target` (component build), and
# the scratch directory `${WASM_SIZE_SCRATCH:-/tmp/stringcheese-wasm-size}`.
#
# Compares each measured size against the baseline in
# `.wasm-size-limits.toml` and exits non-zero if any crate exceeds its
# threshold — matching the CI job's fail condition. Use
# `WASM_SIZE_QUIET=1` to suppress the per-crate build lines and only
# print the summary. Use `WASM_SIZE_NO_GATE=1` to skip the comparison
# (measure-only mode; still exits 0 on completion).
#
# Prerequisites:
#   * rustup target add wasm32-unknown-unknown wasm32-wasip1
#   * `wasm-opt` on PATH (from `binaryen` — `brew install binaryen` /
#     `apt install binaryen` / release tarball at
#     https://github.com/WebAssembly/binaryen/releases)
#   * `cargo-component` for the component build
#     (`cargo install cargo-component`; skipped with a warning if
#     absent)
#
# Tools that CI installs but this script does not require locally
# (contributors don't need them for the gate — CI runs them for
# visibility on regressions):
#   * `twiggy` — top-N reports on the biggest symbols; useful when
#     investigating a size regression but not part of the gate.

set -euo pipefail

# Resolve repo root as this script's parent-of-parent so it works from
# any cwd — a contributor may invoke it from a random subdirectory.
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"

SCRATCH="${WASM_SIZE_SCRATCH:-/tmp/stringcheese-wasm-size}"
mkdir -p "$SCRATCH"

QUIET="${WASM_SIZE_QUIET:-}"
NO_GATE="${WASM_SIZE_NO_GATE:-}"

say() {
  # Silenced only for the per-build noise; the summary always prints.
  # Writes to stderr so the `measure` function's stdout stays clean —
  # it is captured with `$(measure ...)` and only the trailing byte-
  # count is meant to survive.
  if [[ -z "$QUIET" ]]; then
    printf '%s\n' "$*" >&2
  fi
}

# Source of truth for the crate list: the [[crate]] entries in
# `.wasm-size-limits.toml`. Parsed with a POSIX-only shell loop below
# so this script has no non-standard dependencies.
LIMITS_FILE="$REPO_ROOT/.wasm-size-limits.toml"
if [[ ! -f "$LIMITS_FILE" ]]; then
  echo "error: $LIMITS_FILE not found" >&2
  exit 2
fi

# Parse the TOML into three parallel arrays: name / limit_bytes /
# tolerance_pct. Simple line-scanner — every `[[crate]]` block is one
# record; the fields we care about are quoted `name = "..."` and
# integer `optimized_bytes = N` / `tolerance_pct = N`.
NAMES=()
LIMITS=()
TOLERANCES=()
KINDS=()

cur_name=""
cur_kind=""
cur_limit=""
cur_tol=""

flush_record() {
  if [[ -n "$cur_name" && -n "$cur_limit" && -n "$cur_tol" ]]; then
    NAMES+=("$cur_name")
    LIMITS+=("$cur_limit")
    TOLERANCES+=("$cur_tol")
    KINDS+=("${cur_kind:-library}")
  fi
  cur_name=""
  cur_kind=""
  cur_limit=""
  cur_tol=""
}

while IFS= read -r line; do
  line="${line%%#*}"                 # strip inline comment
  line="${line#"${line%%[![:space:]]*}"}"  # ltrim
  line="${line%"${line##*[![:space:]]}"}"  # rtrim
  case "$line" in
    "[[crate]]")
      flush_record
      ;;
    name*=*)
      cur_name="${line#*=}"
      cur_name="${cur_name## }"
      cur_name="${cur_name#\"}"
      cur_name="${cur_name%\"}"
      ;;
    kind*=*)
      cur_kind="${line#*=}"
      cur_kind="${cur_kind## }"
      cur_kind="${cur_kind#\"}"
      cur_kind="${cur_kind%\"}"
      ;;
    optimized_bytes*=*)
      cur_limit="${line#*=}"
      cur_limit="${cur_limit## }"
      ;;
    tolerance_pct*=*)
      cur_tol="${line#*=}"
      cur_tol="${cur_tol## }"
      ;;
  esac
done < "$LIMITS_FILE"
flush_record

if [[ "${#NAMES[@]}" -eq 0 ]]; then
  echo "error: no [[crate]] entries parsed from $LIMITS_FILE" >&2
  exit 2
fi

# Per-artifact measurement. Library probes go through the
# `wasm-size-probes` cdylib wrapper described in
# `wasm-size-probes/Cargo.toml`; components go through
# `cargo component build`. `wasm-opt` is skipped on component-model
# binaries because Binaryen does not yet parse them.
measure() {
  local name="$1" kind="$2"
  local raw_wasm="" opt_wasm=""

  case "$kind" in
    library)
      local feature="probe-$name"
      say "-- $name --"
      (
        cd "$REPO_ROOT/wasm-size-probes"
        cargo build --release \
          --target wasm32-unknown-unknown \
          --no-default-features \
          --features "$feature" \
          --quiet
      )
      raw_wasm="$REPO_ROOT/wasm-size-probes/target/wasm32-unknown-unknown/release/wasm_size_probe.wasm"
      opt_wasm="$SCRATCH/$name.opt.wasm"
      if ! wasm-opt -Oz -o "$opt_wasm" "$raw_wasm" >/dev/null 2>&1; then
        echo "error: wasm-opt failed on $raw_wasm" >&2
        return 1
      fi
      ;;
    component)
      # Only one component today, hardcoded path.
      if ! command -v cargo-component >/dev/null 2>&1; then
        say "-- $name (SKIPPED: cargo-component not installed) --"
        return 2
      fi
      say "-- $name --"
      (
        cd "$REPO_ROOT/component/rust-host"
        cargo component build --release --quiet
      )
      # The final component .wasm lands at
      # target/wasm32-wasip1/release/<pkg>.wasm; the deps/ sibling is
      # the raw core wasm before component-model wrapping.
      local candidate="$REPO_ROOT/component/rust-host/target/wasm32-wasip1/release/stringcheese_component_host.wasm"
      if [[ ! -f "$candidate" ]]; then
        echo "error: no component wasm at $candidate" >&2
        return 1
      fi
      raw_wasm="$candidate"
      # Binaryen wasm-opt does not yet parse components (Binaryen
      # #6728). The baseline in `.wasm-size-limits.toml` is the raw
      # `cargo component build --release` output, so no post-
      # processing here either.
      opt_wasm="$raw_wasm"
      ;;
    *)
      echo "error: unknown kind '$kind' for $name" >&2
      return 1
      ;;
  esac

  wc -c < "$opt_wasm" | tr -d ' '
}

# Pretty-print summary table.
printf '\n'
printf '%-30s %12s %12s %8s %s\n' "crate" "measured_B" "limit_B" "delta%" "status"
printf '%-30s %12s %12s %8s %s\n' "------------------------------" "------------" "------------" "--------" "------"

failed=0
for i in "${!NAMES[@]}"; do
  name="${NAMES[$i]}"
  limit="${LIMITS[$i]}"
  tol="${TOLERANCES[$i]}"
  kind="${KINDS[$i]}"

  measured="$(measure "$name" "$kind")" || {
    rc="$?"
    if [[ "$rc" -eq 2 ]]; then
      printf '%-30s %12s %12s %8s %s\n' "$name" "-" "$limit" "-" "SKIP"
      continue
    fi
    failed=$((failed + 1))
    printf '%-30s %12s %12s %8s %s\n' "$name" "?" "$limit" "?" "BUILD_FAIL"
    continue
  }

  # Threshold check: measured <= limit * (1 + tolerance/100), computed
  # in integer bytes (limit * (100 + tol) / 100). The all-integer form
  # keeps the check reproducible across shells without bringing bc in.
  ceiling=$(( limit * (100 + tol) / 100 ))
  # Delta percent, rounded down. Only shown for humans.
  if [[ "$limit" -gt 0 ]]; then
    delta_pct=$(( (measured - limit) * 100 / limit ))
  else
    delta_pct=0
  fi
  status="OK"
  if [[ -z "$NO_GATE" && "$measured" -gt "$ceiling" ]]; then
    status="OVER"
    failed=$((failed + 1))
  fi
  printf '%-30s %12s %12s %+8s %s\n' "$name" "$measured" "$limit" "${delta_pct}%" "$status"
done

echo ""
if [[ "$failed" -gt 0 ]]; then
  cat <<MSG >&2
FAIL: $failed crate(s) exceeded their wasm-size threshold.

If the growth is intentional, update .wasm-size-limits.toml (bump
optimized_bytes to the new measured value; leave tolerance_pct alone
unless the crate has a genuinely different noise profile — do not
widen it to smother a regression that has not been reviewed).

If the growth is not intentional, inspect the wasm with:
  wasm-opt -Oz -o /tmp/x.opt.wasm \\
    wasm-size-probes/target/wasm32-unknown-unknown/release/wasm_size_probe.wasm
  twiggy top /tmp/x.opt.wasm | head -20
to identify the newly-heavy symbols.
MSG
  exit 1
fi

if [[ -n "$NO_GATE" ]]; then
  echo "OK: measurement-only mode (WASM_SIZE_NO_GATE=1); gate not enforced."
else
  echo "OK: every measured crate is within its wasm-size threshold."
fi
