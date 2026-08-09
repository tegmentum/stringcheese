#!/usr/bin/env bash
#
# Reproducible build for the StringCheese language-detection WASM
# components. Emits three `.wasm` component files under
# `target/wasm32-wasip1/release/`:
#
#   stringcheese_detect_script.wasm      (tier 0, always-resident)
#   stringcheese_detect_whatlang.wasm    (tier 1, one build per script)
#   stringcheese_detect_lingua.wasm      (tier 2, one build per lang set)
#
# The three implement the same WIT world (tegmentum:lang-detect@0.2.0
# in `wit/lang-detect.wit`) so the browser-side dispatcher can load
# whichever set the caller requested and speak to all of them through
# the same generated bindings.
#
# Toolchain expectations:
#   - `cargo-component` >= 0.21.1  (`cargo install cargo-component`)
#   - rustup target wasm32-wasip1  (`rustup target add wasm32-wasip1`)
#
# Overriding what gets built:
#   SCRIPTS="latn cyrl"            # tier-1 shards to build (default: latn)
#   LANGS="en de"                  # tier-2 languages (default: en+de)
#   SKIP_TIER0=1                   # skip the script-detect build
#   SKIP_TIER1=1                   # skip all tier-1 shards
#   SKIP_TIER2=1                   # skip the tier-2 build

set -euo pipefail

cd "$(dirname "$0")/.."

SCRIPTS="${SCRIPTS:-latn}"
LANGS="${LANGS:-en,de}"

CARGO_COMPONENT=(cargo component build --release --target wasm32-wasip1)

if [ -z "${SKIP_TIER0:-}" ]; then
    echo "==> tier 0: stringcheese-detect-script"
    "${CARGO_COMPONENT[@]}" -p stringcheese-detect-script
fi

if [ -z "${SKIP_TIER1:-}" ]; then
    for script in $SCRIPTS; do
        echo "==> tier 1: stringcheese-detect-whatlang --features $script"
        "${CARGO_COMPONENT[@]}" -p stringcheese-detect-whatlang \
            --no-default-features --features "$script"
        # Rename per-script so subsequent builds don't clobber the
        # previous shard's cdylib output.
        mv target/wasm32-wasip1/release/stringcheese_detect_whatlang.wasm \
           "target/wasm32-wasip1/release/stringcheese_detect_whatlang-${script}.wasm"
    done
fi

if [ -z "${SKIP_TIER2:-}" ]; then
    echo "==> tier 2: stringcheese-detect-lingua --features $LANGS"
    # Lingua requires >= 2 languages — `LanguageDetectorBuilder`
    # asserts on this at construction time. `en,de` is the workspace-
    # default anchor pair; downstream builds override LANGS.
    "${CARGO_COMPONENT[@]}" -p stringcheese-detect-lingua \
        --no-default-features --features "$LANGS"
    # Stamp the language set into the filename for the same reason.
    lang_tag="${LANGS//,/-}"
    mv target/wasm32-wasip1/release/stringcheese_detect_lingua.wasm \
       "target/wasm32-wasip1/release/stringcheese_detect_lingua-${lang_tag}.wasm"
fi

echo
echo "Built components:"
ls -la target/wasm32-wasip1/release/stringcheese_detect_*.wasm 2>/dev/null \
    | awk '{printf "  %-70s %10d bytes\n", $NF, $5}'
