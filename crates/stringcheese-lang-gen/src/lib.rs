//! # StringCheese per-language build-time generator
//!
//! Reads a per-language `rules/<bcp47>.toml` and emits a
//! self-contained Rust source file that a per-language crate
//! `include!`s from its `src/lib.rs`. The emitted code declares:
//!
//! - `static STOPWORDS: &[&str] = &[…]`
//! - `pub static CAPABILITIES: stringcheese_lang::LanguageCapabilities = …`
//!
//! Downstream per-language crates hand-write the algorithm surfaces
//! (stemmer, phonex, contraction tokenizer, collator) and wrap them
//! alongside `CAPABILITIES` in their [`Language`] trait impl.
//!
//! [`Language`]: https://docs.rs/stringcheese-lang/latest/stringcheese_lang/trait.Language.html
//!
//! # Build-time only
//!
//! This crate belongs in `[build-dependencies]`, never
//! `[dependencies]`. The generator runs once per build inside the
//! per-language crate's `build.rs`; nothing here reaches the
//! consumer's runtime binary. Callers who pull the per-language
//! crate at runtime pay zero cost for the generator's presence.
//!
//! # Usage
//!
//! In the per-language crate's `Cargo.toml`:
//!
//! ```toml
//! [build-dependencies]
//! stringcheese-lang-gen = "0.1"
//! ```
//!
//! In its `build.rs`:
//!
//! ```no_run
//! use std::env;
//! use std::path::PathBuf;
//!
//! fn main() {
//!     let out = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("generated.rs");
//!     stringcheese_lang_gen::generate("rules/en.toml", &out).unwrap();
//!     println!("cargo:rerun-if-changed=rules/en.toml");
//! }
//! ```
//!
//! And in its `src/lib.rs`:
//!
//! ```ignore
//! include!(concat!(env!("OUT_DIR"), "/generated.rs"));
//! ```
//!
//! # TOML schema
//!
//! ```toml
//! [locale]
//! bcp47 = "en"           # required — BCP-47 primary subtag
//! script = "Latn"        # required — ISO 15924
//! icu = "en"             # optional — ICU4X locale; defaults to bcp47
//! name = "English"       # required — human-readable English name
//!
//! [stopwords]
//! list = ["a", "an", "the", "and", "of"]   # required; may be empty
//! ```
//!
//! Everything else (stemmer choice, phonex kind, contractions,
//! transliteration scheme) is out of scope for the generator — the
//! per-language crate hand-writes the algorithm side and wires it
//! into its `Language` impl.

use std::fs;
use std::io;
use std::path::Path;

use serde::Deserialize;

// ---------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------

/// Read a per-language rules TOML at `rules_path` and write the
/// corresponding generated Rust source to `out_path`.
///
/// Consumers call this from their `build.rs`; see the [module
/// docs](self) for the full usage pattern.
///
/// # Errors
///
/// Returns [`GenerateError`] if the input file can't be read, the
/// TOML is malformed, the schema fails validation, or the output
/// file can't be written.
pub fn generate(
    rules_path: impl AsRef<Path>,
    out_path: impl AsRef<Path>,
) -> Result<(), GenerateError> {
    let toml_src = fs::read_to_string(rules_path.as_ref()).map_err(GenerateError::Read)?;
    let rules: RulesFile = toml::from_str(&toml_src).map_err(GenerateError::Parse)?;
    let generated = emit(&rules);
    fs::write(out_path.as_ref(), generated).map_err(GenerateError::Write)?;
    Ok(())
}

// ---------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------

/// Everything that can go wrong during code generation.
#[derive(Debug)]
pub enum GenerateError {
    /// The rules TOML file couldn't be read.
    Read(io::Error),
    /// The rules TOML file was syntactically or schema-invalid.
    Parse(toml::de::Error),
    /// The generated output file couldn't be written.
    Write(io::Error),
}

impl std::fmt::Display for GenerateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read(e) => write!(f, "reading rules TOML: {e}"),
            Self::Parse(e) => write!(f, "parsing rules TOML: {e}"),
            Self::Write(e) => write!(f, "writing generated source: {e}"),
        }
    }
}

impl std::error::Error for GenerateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(e) | Self::Write(e) => Some(e),
            Self::Parse(e) => Some(e),
        }
    }
}

// ---------------------------------------------------------------------
// Schema — mirrors what the TOML schema documented in the crate docs
// ---------------------------------------------------------------------

#[derive(Deserialize)]
struct RulesFile {
    locale: Locale,
    stopwords: Stopwords,
}

#[derive(Deserialize)]
struct Locale {
    bcp47: String,
    script: String,
    icu: Option<String>,
    name: String,
}

#[derive(Deserialize)]
struct Stopwords {
    list: Vec<String>,
}

// ---------------------------------------------------------------------
// Emitter
// ---------------------------------------------------------------------

fn emit(rules: &RulesFile) -> String {
    use std::fmt::Write as _;

    let icu = rules.locale.icu.as_deref().unwrap_or(&rules.locale.bcp47);
    let mut out = String::new();
    out.push_str("// GENERATED by stringcheese-lang-gen from rules/*.toml.\n");
    out.push_str("// Regenerate by touching the source TOML — build.rs handles the rest.\n\n");

    // Stopword list — one `static &[&str]`.
    out.push_str("static STOPWORDS: &[&str] = &[\n");
    for word in &rules.stopwords.list {
        writeln!(out, "    {},", rust_string_literal(word)).unwrap();
    }
    out.push_str("];\n\n");

    // Capabilities — the `pub static` per-language crates wire into
    // their `Language` trait impl. Doc-string is emitted so the
    // downstream per-language crate's `missing_docs` lint doesn't
    // fire on generated code.
    writeln!(
        out,
        "/// Data-driven capability record for the {} language pack,\n\
         /// generated by `stringcheese-lang-gen` from `rules/{}.toml`.\n\
         /// See [`stringcheese_lang::LanguageCapabilities`] for the field-level docs.",
        rules.locale.name, rules.locale.bcp47,
    )
    .unwrap();
    out.push_str(
        "pub static CAPABILITIES: ::stringcheese_lang::LanguageCapabilities =\n    \
         ::stringcheese_lang::LanguageCapabilities {\n",
    );
    writeln!(
        out,
        "        bcp47:     {},",
        rust_string_literal(&rules.locale.bcp47)
    )
    .unwrap();
    writeln!(
        out,
        "        script:    {},",
        rust_string_literal(&rules.locale.script)
    )
    .unwrap();
    writeln!(out, "        icu:       {},", rust_string_literal(icu)).unwrap();
    writeln!(
        out,
        "        name:      {},",
        rust_string_literal(&rules.locale.name)
    )
    .unwrap();
    out.push_str("        stopwords: STOPWORDS,\n");
    out.push_str("    };\n");

    out
}

/// Quote `s` as a Rust string literal. The rules TOML is
/// author-controlled — a plain escape pass covers every character
/// we've observed in real per-language TOMLs; a word carrying `"`
/// still round-trips correctly via the `\"` escape.
fn rust_string_literal(s: &str) -> String {
    use std::fmt::Write as _;
    let mut out = String::from("\"");
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => write!(out, "\\u{{{:04x}}}", c as u32).unwrap(),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_expected_shape_for_minimal_rules() {
        let rules = RulesFile {
            locale: Locale {
                bcp47: "en".into(),
                script: "Latn".into(),
                icu: None,
                name: "English".into(),
            },
            stopwords: Stopwords {
                list: vec!["a".into(), "the".into()],
            },
        };
        let src = emit(&rules);
        assert!(src.contains("static STOPWORDS"));
        assert!(src.contains("\"a\","));
        assert!(src.contains("\"the\","));
        assert!(src.contains("pub static CAPABILITIES"));
        assert!(src.contains("bcp47:     \"en\""));
        // ICU defaults to BCP-47 when the TOML doesn't override it.
        assert!(src.contains("icu:       \"en\""));
    }

    #[test]
    fn icu_override_is_honored() {
        let rules = RulesFile {
            locale: Locale {
                bcp47: "en".into(),
                script: "Latn".into(),
                icu: Some("en-GB".into()),
                name: "English".into(),
            },
            stopwords: Stopwords { list: vec![] },
        };
        let src = emit(&rules);
        assert!(src.contains("icu:       \"en-GB\""));
    }

    #[test]
    fn rust_literal_escapes_control_chars_and_quotes() {
        assert_eq!(rust_string_literal("a\"b"), "\"a\\\"b\"");
        assert_eq!(rust_string_literal("a\\b"), "\"a\\\\b\"");
        assert_eq!(rust_string_literal("a\nb"), "\"a\\nb\"");
    }

    #[test]
    fn rust_literal_preserves_non_ascii_letters() {
        // Devanagari / Cyrillic / German umlauts round-trip through
        // the literal emitter without escape-encoding — Rust source
        // files are UTF-8 and Devanagari letters are valid `char`
        // literals in a `"…"` string.
        assert_eq!(rust_string_literal("नमस्ते"), "\"नमस्ते\"");
        assert_eq!(rust_string_literal("Привет"), "\"Привет\"");
        assert_eq!(rust_string_literal("über"), "\"über\"");
    }

    #[test]
    fn generate_end_to_end_writes_valid_rust() {
        let tmp = std::env::temp_dir().join("stringcheese-lang-gen-test");
        std::fs::create_dir_all(&tmp).unwrap();
        let rules_path = tmp.join("en.toml");
        let out_path = tmp.join("generated.rs");
        std::fs::write(
            &rules_path,
            r#"
[locale]
bcp47 = "en"
script = "Latn"
name = "English"

[stopwords]
list = ["a", "and", "the"]
"#,
        )
        .unwrap();
        generate(&rules_path, &out_path).unwrap();
        let src = std::fs::read_to_string(&out_path).unwrap();
        assert!(src.contains("pub static CAPABILITIES"));
        assert!(src.contains("bcp47:     \"en\""));
        assert!(src.contains("stopwords: STOPWORDS"));
        // Clean up the temp files so a re-run doesn't accumulate.
        let _ = std::fs::remove_file(&rules_path);
        let _ = std::fs::remove_file(&out_path);
    }

    #[test]
    fn missing_rules_file_is_a_read_error() {
        let out = std::env::temp_dir().join("stringcheese-lang-gen-nowhere.rs");
        match generate("/nonexistent/path/rules.toml", &out) {
            Err(GenerateError::Read(_)) => {}
            other => panic!("expected Read error, got {other:?}"),
        }
    }
}
