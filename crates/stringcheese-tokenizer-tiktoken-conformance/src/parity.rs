//! The parity engine: build `BpeTokenizer` from real vocab bytes,
//! encode the corpus, diff against tiktoken-rs, and report.
//!
//! Only compiled when `parity-real-vocab` is enabled. Everything else
//! in the crate still builds without the oracle so callers can wire
//! up cache/fetch/corpus utilities in their own harness.

use std::collections::HashSet;

use stringcheese_tokenizer::Tokenizer;
use stringcheese_tokenizer_hf::{
    BpeMergeTable, BpeTokenizer, BpeVocabulary, PreTokenizerRegex, RegexPreTokenizer,
    TIKTOKEN_CANONICAL_PATTERN,
};
use stringcheese_tokenizer_tiktoken::builder;

/// The `o200k_base` pre-tokenizer pattern (verbatim from
/// `openai/tiktoken`'s `openai_public.py`). Differs from
/// [`TIKTOKEN_CANONICAL_PATTERN`] (which is the `cl100k_base`
/// pattern) in the letter-run alternatives — o200k uses an
/// upper-case-cluster / lower-case-cluster split that lets
/// `CamelCase` and `snake_case` identifiers tokenise more
/// naturally.
///
/// Kept in this crate rather than upstream in
/// `stringcheese-tokenizer-hf` deliberately: the pattern is a
/// harness-scoped test input, not a production API. Promoting it
/// upstream is a task for the `stringcheese-tokenizer-hf` maintainer
/// after this harness proves both patterns yield parity against
/// their respective variants.
pub const O200K_BASE_PATTERN: &str = concat!(
    r"[^\r\n\p{L}\p{N}]?[\p{Lu}\p{Lt}\p{Lm}\p{Lo}\p{M}]*[\p{Ll}\p{Lm}\p{Lo}\p{M}]+(?i:'s|'t|'re|'ve|'m|'ll|'d)?",
    r"|[^\r\n\p{L}\p{N}]?[\p{Lu}\p{Lt}\p{Lm}\p{Lo}\p{M}]+[\p{Ll}\p{Lm}\p{Lo}\p{M}]*(?i:'s|'t|'re|'ve|'m|'ll|'d)?",
    r"|\p{N}{1,3}",
    r"| ?[^\s\p{L}\p{N}]+[\r\n/]*",
    r"|\s*[\r\n]+",
    r"|\s+(?!\S)",
    r"|\s+",
);

/// Pick the pre-tokenizer pattern that matches `variant_name`. The
/// two shipped variants map to distinct regexes; a caller adding a
/// third variant needs to extend this dispatch.
fn pattern_for(variant_name: &str) -> &'static str {
    match variant_name {
        "o200k_base" => O200K_BASE_PATTERN,
        // cl100k_base and any future variant fall back to the
        // canonical cl100k pattern until the harness is taught
        // otherwise.
        _ => TIKTOKEN_CANONICAL_PATTERN,
    }
}

use crate::corpus;
use crate::fetch;
use crate::variant::Variant;

/// A single input on which `stringcheese-tokenizer-hf` and
/// tiktoken-rs disagreed.
///
/// The three fields are enough for a triage engineer to reproduce the
/// bug in isolation: pick the input string, encode it under both
/// engines, diff the id vectors.
#[derive(Debug, Clone)]
pub struct Divergence {
    /// Corpus index (`corpus::CORPUS[input_index]`). Useful when
    /// grepping the report against the source of truth in
    /// `src/corpus.rs`.
    pub input_index: usize,
    /// The offending input verbatim.
    pub input: &'static str,
    /// The category `input` belongs to — see [`corpus::category_for`].
    pub category: &'static str,
    /// tiktoken-rs's output. The oracle.
    pub expected_ids: Vec<u32>,
    /// `stringcheese-tokenizer-hf`'s output.
    pub actual_ids: Vec<u32>,
    /// First index at which the two vectors differ. `None` iff one
    /// vector is a strict prefix of the other (in which case
    /// `expected_ids.len() != actual_ids.len()` and the divergence is
    /// at the end of the shorter one).
    pub first_diff_at: Option<usize>,
}

/// Summary of one variant's parity run. `passed + divergences.len()`
/// always equals the corpus size.
#[derive(Debug, Clone)]
pub struct ParityReport {
    /// The variant this report covers (`"cl100k_base"`, etc.).
    pub variant_name: &'static str,
    /// Number of corpus inputs that produced byte-identical id
    /// sequences across both engines.
    pub passed: usize,
    /// Every input on which the engines disagreed. Truncated at 20
    /// so a broken run does not overflow the log; the truncation
    /// count is reported in [`Self::truncated_divergences`].
    pub divergences: Vec<Divergence>,
    /// How many further divergences were dropped after the
    /// truncation cutoff. `0` iff [`Self::divergences`] is complete.
    pub truncated_divergences: usize,
    /// Total corpus size the report covers.
    pub total: usize,
}

impl ParityReport {
    /// `true` iff every corpus input encoded identically under both
    /// engines. Convenience for `#[test]` assertions.
    #[must_use]
    pub fn is_perfect(&self) -> bool {
        self.divergences.is_empty() && self.truncated_divergences == 0
    }

    /// Divergences grouped by their category label — useful for a
    /// human-readable report that groups "all the emoji failures" so
    /// a reviewer can see a systemic issue at a glance rather than
    /// scanning per-input rows.
    #[must_use]
    pub fn divergences_by_category(&self) -> Vec<(&'static str, usize)> {
        use std::collections::BTreeMap;
        let mut m: BTreeMap<&'static str, usize> = BTreeMap::new();
        for d in &self.divergences {
            *m.entry(d.category).or_default() += 1;
        }
        m.into_iter().collect()
    }
}

/// Truncation limit for [`ParityReport::divergences`]. A run with
/// more than this many divergences records the excess in
/// [`ParityReport::truncated_divergences`] rather than blowing up
/// the log.
pub const DIVERGENCE_CAP: usize = 20;

/// Construct a `stringcheese-tokenizer-hf` `BpeTokenizer` from the
/// raw tiktoken plaintext bytes (`<base64(bytes)> <rank>` format).
///
/// The returned tokenizer:
///
/// * Uses the pre-tokenizer regex that matches `variant_name` (see
///   [`pattern_for`]). `cl100k_base` uses
///   [`TIKTOKEN_CANONICAL_PATTERN`]; `o200k_base` uses
///   [`O200K_BASE_PATTERN`].
/// * Does *not* register any special tokens — the parity harness
///   compares against tiktoken-rs's `encode_ordinary`, which
///   likewise skips special-token extraction.
/// * Reconstructs the merge table from the vocabulary using the
///   same "best split by (lr, rr) key" heuristic that
///   [`stringcheese_tokenizer_tiktoken::builder::build_scud_from_tiktoken`]
///   applies for the embedded SCUD packs.
///
/// # Errors
///
/// Returns the plaintext parse error verbatim if the tiktoken blob
/// is malformed.
pub fn build_stringcheese_tokenizer(
    variant_name: &str,
    plaintext: &[u8],
) -> Result<BpeTokenizer, String> {
    let (vocab_entries, merge_entries) = builder::build_scud_from_tiktoken(plaintext)?;

    let mut vocab = BpeVocabulary::new();
    // Vocab first — merges reference these ids. Sort by id so the
    // insert order matches the on-wire SCUD-lite writer, which
    // matters for tests that compare against the same tokenizer
    // constructed via the SCUD-lite loader.
    let mut sorted = vocab_entries;
    sorted.sort_by_key(|e| e.id);
    for entry in sorted {
        vocab
            .insert(entry.id, entry.bytes)
            .map_err(|e| format!("vocabulary insert failed: {e:?}"))?;
    }

    let mut merges = BpeMergeTable::new();
    for m in merge_entries {
        // Look up both ids in the vocab we just built so we can
        // insert the (bytes, bytes, rank) triple `BpeMergeTable`
        // expects.
        let left_bytes = vocab
            .bytes(m.left_id)
            .ok_or_else(|| format!("merge references unknown left id {}", m.left_id))?
            .to_vec();
        let right_bytes = vocab
            .bytes(m.right_id)
            .ok_or_else(|| format!("merge references unknown right id {}", m.right_id))?
            .to_vec();
        merges.insert(left_bytes, right_bytes, m.rank);
    }

    let pattern = pattern_for(variant_name);
    let pre = RegexPreTokenizer::new(pattern)
        .map_err(|e| format!("failed to compile pre-tokenizer regex for {variant_name}: {e}"))?;

    Ok(BpeTokenizer::from_parts(merges, vocab).with_pre_tokenizer(PreTokenizerRegex::regex(pre)))
}

/// Load the tiktoken-rs oracle for `variant`.
///
/// # Errors
///
/// Returns a message if tiktoken-rs does not know the requested
/// variant (a new-variant landing here needs a matching arm added
/// to this function).
pub fn load_oracle(variant: &Variant) -> Result<tiktoken_rs::CoreBPE, String> {
    match variant.name {
        "cl100k_base" => tiktoken_rs::cl100k_base()
            .map_err(|e| format!("tiktoken-rs failed to load cl100k_base: {e}")),
        "o200k_base" => tiktoken_rs::o200k_base()
            .map_err(|e| format!("tiktoken-rs failed to load o200k_base: {e}")),
        other => Err(format!(
            "no tiktoken-rs oracle registered for variant {other:?} — add a \
             match arm in parity::load_oracle"
        )),
    }
}

/// Encode `text` under the oracle. The oracle's `encode_ordinary`
/// is deliberately used so special-token extraction stays out of
/// the parity picture (we compare pure BPE output).
fn oracle_encode(oracle: &tiktoken_rs::CoreBPE, text: &str) -> Vec<u32> {
    // `encode_ordinary` returns `Vec<Rank>`; `Rank` is `u32` in
    // tiktoken-rs 0.6+. The cast is idempotent — it's here so a
    // future version that widens Rank to u64 still compiles.
    #[allow(clippy::useless_conversion)]
    oracle
        .encode_ordinary(text)
        .into_iter()
        .map(|r| u32::try_from(r).expect("token id fits in u32 for tiktoken-published variants"))
        .collect()
}

/// Encode `text` under the crate-under-test. Skips the trait method
/// so we can call `encode_with_special_policy` with an empty
/// allowed set — matches tiktoken-rs's `encode_ordinary` semantics
/// exactly (no special tokens registered, none matched, none
/// errored).
fn stringcheese_encode(tok: &BpeTokenizer, text: &str) -> Result<Vec<u32>, String> {
    // The tokenizer has no special tokens registered so this reduces
    // to the plain BPE encode loop; using `encode` (the trait method)
    // is equivalent and simpler.
    let enc = tok.encode(text).map_err(|e| format!("{e:?}"))?;
    Ok(enc.ids)
}

/// Run the parity harness against `corpus` for `variant`.
///
/// Fetches (or reads from cache) the plaintext blob, builds the
/// stringcheese and tiktoken-rs tokenizers, encodes every corpus
/// input under both, and returns a [`ParityReport`].
///
/// # Errors
///
/// Returns an error if:
/// * The plaintext blob cannot be fetched or fails SHA-256
///   verification (see [`fetch::load_variant`]).
/// * The stringcheese tokenizer construction fails.
/// * The tiktoken-rs oracle for the variant does not load.
pub fn run_parity(variant: &Variant, corpus: &[&'static str]) -> Result<ParityReport, String> {
    let plaintext = fetch::load_variant(variant)?;
    let tok = build_stringcheese_tokenizer(variant.name, &plaintext)?;
    let oracle = load_oracle(variant)?;

    let mut report = ParityReport {
        variant_name: variant.name,
        passed: 0,
        divergences: Vec::new(),
        truncated_divergences: 0,
        total: corpus.len(),
    };

    // Pre-compute the set of special-token surface strings the oracle
    // knows about. tiktoken-rs treats these as ordinary text in
    // `encode_ordinary`; stringcheese does too (no specials
    // registered). Both should agree; the set is only kept for
    // diagnostic reference if a divergence lands on such a string.
    let _oracle_specials: HashSet<&str> = HashSet::new();

    for (i, input) in corpus.iter().enumerate() {
        let expected = oracle_encode(&oracle, input);
        let actual = stringcheese_encode(&tok, input)?;
        if expected == actual {
            report.passed += 1;
            continue;
        }
        let first_diff_at = expected
            .iter()
            .zip(actual.iter())
            .position(|(a, b)| a != b)
            .or(if expected.len() == actual.len() {
                None
            } else {
                Some(expected.len().min(actual.len()))
            });
        let div = Divergence {
            input_index: i,
            input,
            category: corpus::category_for(i),
            expected_ids: expected,
            actual_ids: actual,
            first_diff_at,
        };
        if report.divergences.len() < DIVERGENCE_CAP {
            report.divergences.push(div);
        } else {
            report.truncated_divergences += 1;
        }
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_stringcheese_tokenizer_handles_minimal_blob() {
        // A minimal blob covering byte alphabet + one merge. Uses
        // the same base64 shape the real tiktoken plaintext uses.
        // "a" (byte 0x61) at rank 97; "b" (byte 0x62) at rank 98;
        // "ab" (bytes 0x61 0x62) at rank 300. The parser should
        // synthesise a merge ("a", "b", 300).
        let plaintext = "YQ== 97\nYg== 98\nYWI= 300\n";
        let tok = build_stringcheese_tokenizer("cl100k_base", plaintext.as_bytes()).unwrap();
        // With the canonical tiktoken regex, "ab" is a single word
        // (letters run), then BPE merges (a, b) → 300.
        let enc = tok.encode("ab").unwrap();
        assert_eq!(enc.ids, vec![300u32]);
    }
}
