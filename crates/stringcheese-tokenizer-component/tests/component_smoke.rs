//! Wasmtime smoke test for the reference `tegmentum:tokenizer@0.1.0`
//! component.
//!
//! Verifies Phase 7's "reference `tokenizer-provider` component built
//! from the character-BPE seed echoes correct encodings across the
//! boundary under wasmtime" acceptance criterion. The test drives the
//! component the same way any external caller would: builds it under
//! `wasm32-wasip1 --features wit-component`, componentizes it via
//! `wasm-tools`, and invokes each of the four exports through
//! `wasmtime run --invoke`, asserting the id sequence and decoded
//! output match the host-side fixture in `src/reference.rs`.
//!
//! # Prerequisites
//!
//! Requires all of these on `$PATH`:
//!
//! * `cargo` with the `wasm32-wasip1` target installed
//! * `wasm-tools` (Bytecode Alliance)
//! * `wasmtime` (Bytecode Alliance)
//!
//! When any of the three is missing the test soft-skips with a
//! diagnostic — matching the "conformance runner" convention used by
//! `stringcheese-tokenizer-hf`'s real-vocab tests. This keeps host
//! `cargo test` green on developer machines that haven't installed
//! the wasm toolchain; CI installs all three explicitly (see the
//! `wasm-tokenizer-component` job).
//!
//! # Why a `--invoke`-driven test instead of the `wasmtime` crate
//!
//! Pulling the `wasmtime` crate in as a dev-dep would add a large
//! transitive-dep tail (cranelift, regalloc2, target-lexicon,
//! wasmparser, wast, …) that is not otherwise on the workspace's
//! dep graph. The CLI-based approach keeps the workspace lock
//! unchanged and matches how a real deployment invokes the
//! component (through a runtime binary loading the `.wasm`, not
//! through embedded-Rust wasmtime bindings).

#![cfg(not(target_family = "wasm"))]

use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

/// One end-to-end round-trip: build, componentize, invoke.
///
/// Split out of the test bodies so each of the four export smoke
/// tests below stays a straight per-export assertion.
struct SmokeHarness {
    component_wasm: PathBuf,
}

/// The four smoke tests run concurrently under `cargo test`'s
/// default thread pool. Each of them calls `SmokeHarness::build_or_skip`;
/// without a guard the four threads would race on the same
/// `target/wasm32-wasip1/release/` directory and `wasm-tools`
/// output path, producing spurious failures when one test's
/// componentize step reads a wasm the other is still writing.
/// This `OnceLock<Option<PathBuf>>` serialises the build-once,
/// componentize-once step across every caller — subsequent
/// invocations get back the cached component path (or the cached
/// skip decision) without re-invoking cargo / wasm-tools.
static SHARED: OnceLock<Option<PathBuf>> = OnceLock::new();

impl SmokeHarness {
    fn build_or_skip() -> Option<Self> {
        let path = SHARED.get_or_init(Self::build_shared_once).clone()?;
        Some(Self {
            component_wasm: path,
        })
    }

    fn build_shared_once() -> Option<PathBuf> {
        // Every one of these tools is optional at test-run time; a
        // missing tool soft-skips rather than failing. Matches the
        // shape of `stringcheese-tokenizer-hf`'s real-vocab
        // conformance runner.
        if !has_tool("cargo") {
            eprintln!("SKIP: cargo not on PATH");
            return None;
        }
        if !has_tool("wasm-tools") {
            eprintln!(
                "SKIP: wasm-tools not on PATH — install from https://github.com/bytecodealliance/wasm-tools"
            );
            return None;
        }
        if !has_tool("wasmtime") {
            eprintln!("SKIP: wasmtime not on PATH — install from https://wasmtime.dev/install.sh");
            return None;
        }
        if !target_installed("wasm32-wasip1") {
            eprintln!(
                "SKIP: wasm32-wasip1 target not installed — `rustup target add wasm32-wasip1`"
            );
            return None;
        }

        // Build the wasm module. `--release` keeps the produced
        // artifact aligned with the CI job's build recipe; the size
        // gate downstream measures the release output.
        let workspace_root = workspace_root();
        let build = Command::new("cargo")
            .current_dir(&workspace_root)
            .args([
                "build",
                "-p",
                "stringcheese-tokenizer-component",
                "--target",
                "wasm32-wasip1",
                "--features",
                "wit-component",
                "--release",
            ])
            .status()
            .expect("cargo build must launch");
        assert!(
            build.success(),
            "cargo build for wasm32-wasip1 must succeed"
        );

        let module_wasm = workspace_root
            .join("target/wasm32-wasip1/release/stringcheese_tokenizer_component.wasm");
        assert!(
            module_wasm.exists(),
            "expected wasm at {} after cargo build",
            module_wasm.display()
        );

        // Locate the WASI preview1 → preview2 adapter. Newer
        // `wasm-tools` and `cargo-component` ship one under
        // `~/.cargo/registry/`, but the reliable path here is to
        // fetch it once alongside the wit-bindgen-rt version this
        // crate depends on. On the CI path the workflow places the
        // adapter under `$RUNNER_TEMP/adapter.wasm`; on developer
        // machines the test soft-skips when nothing is found.
        let Some(adapter) = find_wasi_adapter() else {
            eprintln!(
                "SKIP: no wasi_snapshot_preview1.reactor.wasm adapter found on the search path"
            );
            return None;
        };

        // Componentize.
        let component_wasm = std::env::temp_dir().join("stringcheese_tokenizer_component.wasm");
        let componentize = Command::new("wasm-tools")
            .args([
                "component",
                "new",
                module_wasm.to_str().expect("path is utf8"),
                "--adapt",
                adapter.to_str().expect("adapter path is utf8"),
                "-o",
                component_wasm.to_str().expect("output path is utf8"),
            ])
            .status()
            .expect("wasm-tools must launch");
        assert!(
            componentize.success(),
            "wasm-tools component new must succeed"
        );
        assert!(
            component_wasm.exists(),
            "expected component at {}",
            component_wasm.display()
        );
        Some(component_wasm)
    }

    fn invoke(&self, expr: &str) -> String {
        let out = Command::new("wasmtime")
            .args([
                "run",
                "--dir=.",
                "--invoke",
                expr,
                self.component_wasm.to_str().expect("path is utf8"),
            ])
            .output()
            .expect("wasmtime must launch");
        assert!(
            out.status.success(),
            "wasmtime invoke {expr:?} failed: stderr={}",
            String::from_utf8_lossy(&out.stderr)
        );
        // `wasmtime run --invoke` prints the return value on stdout
        // as a wave-shaped expression. Trim the trailing newline.
        String::from_utf8(out.stdout)
            .expect("wasmtime output is utf8")
            .trim_end()
            .to_string()
    }
}

fn has_tool(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn target_installed(triple: &str) -> bool {
    let out = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .any(|l| l.trim() == triple),
        _ => {
            // `rustup` unavailable — assume the target is present so
            // the caller sees the real failure (build) rather than an
            // erroneous skip.
            true
        }
    }
}

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is `crates/stringcheese-tokenizer-component`;
    // walk up two levels to reach the workspace root.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root is two levels above crate manifest")
        .to_path_buf()
}

fn find_wasi_adapter() -> Option<PathBuf> {
    // Search order:
    //   1. `STRINGCHEESE_WASI_ADAPTER` environment variable — the
    //      CI job sets this to the adapter it downloads into
    //      `$RUNNER_TEMP`.
    //   2. `~/.cache/stringcheese-tokenizer-component/wasi_snapshot_preview1.reactor.wasm`
    //      — a developer convenience path so a one-time download
    //      does not need to be repeated per test run.
    //   3. Any `wasi_snapshot_preview1.reactor.wasm` under
    //      `~/.cargo/registry/`. cargo-component ships one at
    //      known relative paths after `cargo install cargo-component`.
    if let Ok(p) = std::env::var("STRINGCHEESE_WASI_ADAPTER") {
        let path = PathBuf::from(p);
        if path.exists() {
            return Some(path);
        }
    }
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let cache =
        home.join(".cache/stringcheese-tokenizer-component/wasi_snapshot_preview1.reactor.wasm");
    if cache.exists() {
        return Some(cache);
    }
    // Last resort: search cargo-component's install root.
    walk_for_adapter(&home.join(".cargo/registry/src")).next()
}

fn walk_for_adapter(root: &std::path::Path) -> impl Iterator<Item = PathBuf> {
    // Non-recursive one-level scan; cargo-component's adapter lives
    // at a known depth. Kept intentionally shallow to bound test
    // startup cost.
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            let candidate = path.join("cargo-component/wasi_snapshot_preview1.reactor.wasm");
            if candidate.exists() {
                out.push(candidate);
            }
        }
    }
    out.into_iter()
}

#[test]
fn wasmtime_smoke_encode_returns_expected_ids() {
    let Some(h) = SmokeHarness::build_or_skip() else {
        return;
    };
    let out = h.invoke("encode(\"hello\")");
    // The character-BPE reference vocab merges "hello" to id 260;
    // the returned encoding carries a single 0..5 offset and no
    // special-mask entries (the reference tokenizer emits an empty
    // mask when no special token fires, matching the trait
    // crate's "empty when not tracked" convention).
    assert!(
        out.contains("ids: [260]"),
        "encode output missing expected ids [260]: {out}"
    );
    assert!(
        out.contains("start: 0, end: 5"),
        "encode output missing 0..5 offset: {out}"
    );
}

#[test]
fn wasmtime_smoke_decode_round_trips_hello() {
    let Some(h) = SmokeHarness::build_or_skip() else {
        return;
    };
    let out = h.invoke("decode([260])");
    assert_eq!(out.trim(), r#"ok("hello")"#);
}

#[test]
fn wasmtime_smoke_count_matches_encode() {
    let Some(h) = SmokeHarness::build_or_skip() else {
        return;
    };
    let out = h.invoke("count(\"hello\")");
    assert_eq!(out.trim(), "ok(1)");
}

#[test]
fn wasmtime_smoke_get_capabilities_reports_reference_shape() {
    let Some(h) = SmokeHarness::build_or_skip() else {
        return;
    };
    let out = h.invoke("get-capabilities()");
    assert!(
        out.contains(r#"model-type: "bpe""#),
        "capabilities missing model-type: {out}"
    );
    assert!(
        out.contains(r#"variant-id: "reference-character-bpe""#),
        "capabilities missing variant-id: {out}"
    );
    assert!(
        out.contains("vocab-size: 261"),
        "capabilities missing vocab-size 261: {out}"
    );
    assert!(
        out.contains("has-byte-fallback: false"),
        "capabilities missing has-byte-fallback false: {out}"
    );
    assert!(
        out.contains("has-special-tokens: true"),
        "capabilities missing has-special-tokens true: {out}"
    );
}
