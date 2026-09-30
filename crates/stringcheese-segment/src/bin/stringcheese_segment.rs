//! `stringcheese-segment` — small CLI wrapper for the crate's
//! [`split`] entry point. Reads UTF-8 text from stdin as
//! newline-delimited records. For each input line, applies the
//! chosen [`SegmentUnit`] and writes the tokens to stdout as
//! `<record_id>\t<token>` pairs. Skips empty tokens and (for
//! `Words`) tokens that contain no alphanumeric scalar.
//!
//! Consumers (see `~/git/snomed-ct/tools/rf2-to-canonical.sh`,
//! SLICE-INT-followup-4 D M4a-b) get UAX #29 tokenization
//! parity with the authoring path -- the same
//! [`SegmentUnit::Words`] iterator that populates canonical
//! `description_tokens` in the substrate.
//!
//! Input format (repeatable, one record per line):
//!
//! ```text
//! <record_id>\t<text>\n
//! ```
//!
//! Output format:
//!
//! ```text
//! <record_id>\t<token>\n
//! ```
//!
//! `<record_id>` is passed through verbatim (typically the caller's
//! primary-key column); the CLI does not interpret it. Records
//! without a tab or with an empty text side yield no tokens.
//!
//! Flags:
//!   `--unit <bytes|codepoints|graphemes|words|sentences|lines>`
//!       default: `words`.
//!   `--lowercase`
//!       lowercase the text BEFORE segmentation (mirrors
//!       `~/git/snomed-ct/src/cli/commands/build.rs`'s
//!       `term.to_lowercase()` pass; a single UTF-8 walk vs
//!       one-per-token lower).
//!   `--keep-punct`
//!       for `words`, emit tokens even if they contain no
//!       alphanumeric scalar. Default matches the substrate's
//!       `STRINGCHEESE-PUNCT-FILTER-PIN` behavior.
//!
//! Exit codes: 0 on success, 2 on argument error, 1 on I/O error.

use std::io::{self, BufRead, BufWriter, Write};

use stringcheese_segment::{split, SegmentUnit};

fn parse_unit(s: &str) -> Option<SegmentUnit> {
    match s {
        "bytes" => Some(SegmentUnit::Bytes),
        "codepoints" | "code-points" => Some(SegmentUnit::CodePoints),
        "graphemes" => Some(SegmentUnit::Graphemes),
        "words" => Some(SegmentUnit::Words),
        "sentences" => Some(SegmentUnit::Sentences),
        "lines" => Some(SegmentUnit::Lines),
        _ => None,
    }
}

fn main() {
    let mut unit = SegmentUnit::Words;
    let mut lowercase = false;
    let mut keep_punct = false;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--unit" => {
                let Some(v) = args.next() else {
                    eprintln!("--unit requires a value");
                    std::process::exit(2);
                };
                let Some(parsed) = parse_unit(&v) else {
                    eprintln!("unknown --unit value: {v}");
                    std::process::exit(2);
                };
                unit = parsed;
            }
            "--lowercase" => lowercase = true,
            "--keep-punct" => keep_punct = true,
            "-h" | "--help" => {
                println!(
                    "stringcheese-segment [--unit <bytes|codepoints|graphemes|words|sentences|lines>] [--lowercase] [--keep-punct]"
                );
                println!("  input : <record_id>\\t<text>\\n on stdin");
                println!("  output: <record_id>\\t<token>\\n on stdout");
                return;
            }
            other => {
                eprintln!("unexpected argument: {other}");
                std::process::exit(2);
            }
        }
    }

    let filter_punct = matches!(unit, SegmentUnit::Words) && !keep_punct;

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    let mut had_io_error = false;
    for line in stdin.lock().lines() {
        let Ok(line) = line else {
            had_io_error = true;
            break;
        };
        let Some((id, text)) = line.split_once('\t') else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        let owned;
        let text: &str = if lowercase {
            owned = text.to_lowercase();
            &owned
        } else {
            text
        };
        for token in split(text, unit) {
            if token.is_empty() {
                continue;
            }
            if filter_punct && !token.chars().any(|c| c.is_alphanumeric()) {
                continue;
            }
            if writeln!(out, "{id}\t{token}").is_err() {
                had_io_error = true;
                break;
            }
        }
        if had_io_error {
            break;
        }
    }
    if out.flush().is_err() {
        had_io_error = true;
    }
    if had_io_error {
        std::process::exit(1);
    }
}
