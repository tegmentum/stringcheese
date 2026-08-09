//! Build-time codegen for the German pack — mirrors the pattern
//! established by `stringcheese-en/build.rs`. Reads `rules/de.toml`
//! and emits `$OUT_DIR/generated.rs`; the rest of the crate wires
//! that generated code into its `Language` trait impl.

use std::env;
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR set by cargo"));
    let rules = "rules/de.toml";
    let out = out_dir.join("generated.rs");
    stringcheese_lang_gen::generate(rules, &out)
        .unwrap_or_else(|e| panic!("stringcheese-lang-gen failed on {rules}: {e}"));
    println!("cargo:rerun-if-changed={rules}");
    println!("cargo:rerun-if-changed=build.rs");
}
