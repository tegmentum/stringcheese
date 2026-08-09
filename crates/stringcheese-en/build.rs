//! Build-time codegen for the English pack.
//!
//! Reads `rules/en.toml` via `stringcheese-lang-gen` and emits a
//! self-contained Rust source at `$OUT_DIR/generated.rs`. The runtime
//! library `include!`s that file to bring in the static
//! `CAPABILITIES: LanguageCapabilities` and `STOPWORDS: &[&str]`
//! consts — the algorithm side (Porter, Porter2, contractions,
//! collator) stays hand-written under `src/`.
//!
//! `stringcheese-lang-gen` is a `[build-dependencies]` entry — it
//! runs here at build time and drops out of the shipped binary. A
//! caller pulling `stringcheese-en = "0.1"` pays zero cost for the
//! generator's presence.

use std::env;
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR set by cargo"));
    let rules = "rules/en.toml";
    let out = out_dir.join("generated.rs");
    stringcheese_lang_gen::generate(rules, &out)
        .unwrap_or_else(|e| panic!("stringcheese-lang-gen failed on {rules}: {e}"));
    println!("cargo:rerun-if-changed={rules}");
    println!("cargo:rerun-if-changed=build.rs");
}
