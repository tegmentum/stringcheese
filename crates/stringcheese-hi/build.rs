//! Build-time codegen for the Hindi pack — mirrors the pattern
//! from `stringcheese-en/build.rs`. Reads `rules/hi.toml` and emits
//! `$OUT_DIR/generated.rs`.

use std::env;
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR set by cargo"));
    let rules = "rules/hi.toml";
    let out = out_dir.join("generated.rs");
    stringcheese_lang_gen::generate(rules, &out)
        .unwrap_or_else(|e| panic!("stringcheese-lang-gen failed on {rules}: {e}"));
    println!("cargo:rerun-if-changed={rules}");
    println!("cargo:rerun-if-changed=build.rs");
}
