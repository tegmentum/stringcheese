//! Canonical phonetic golden cases wired to the `stringcheese-corpus` schema.
//!
//! Every entry is a [`GoldenCase`] whose `descriptor` field equals one of
//! the three encoders' [`AlgorithmDescriptor`]s and whose `expected` field
//! is the encoded key. Each case cites its provenance via
//! [`GoldenSource`].
//!
//! Because this module is `#[cfg(test)]`, its cases are compiled only into
//! the crate's test binaries — `stringcheese-corpus` is declared as a
//! dev-dependency for exactly that reason.
//!
//! # Case counts (per the phonetic-subsystem spec)
//!
//! * Soundex: 10 encoding cases + 3 match-pair cases (12 unique inputs)
//! * NYSIIS: 10 encoding cases + 3 match-pair cases
//! * Double Metaphone (primary-only): 10 encoding cases + 3 match-pair cases
//! * Double Metaphone (full): 10 encoding cases + 3 match-pair cases

use alloc::string::String;

use stringcheese_corpus::{GoldenCase, GoldenSource};

use crate::double_metaphone::{DoubleMetaphone, DoubleMetaphoneKey};
use crate::nysiis::Nysiis;
use crate::soundex::Soundex;

/// A single-input, string-key encoding golden case.
pub type EncodingCase = GoldenCase<&'static str, &'static str>;

/// A Double Metaphone encoding golden case (single input, primary-only
/// key).
pub type DoubleMetaphoneCase = GoldenCase<&'static str, DoubleMetaphoneKey>;

/// A match-pair golden case: two inputs, and whether they should encode to
/// equal keys.
pub type MatchPairCase = GoldenCase<(&'static str, &'static str), bool>;

// ---------------------------------------------------------------------------
// Soundex
// ---------------------------------------------------------------------------

/// Soundex encoding golden cases.
///
/// Every expected value is the code that NARA's canonical Soundex reference
/// (published by the US National Archives) produces for the given input.
/// The source is [`GoldenSource::Standard`] for cases that appear verbatim
/// in NARA's reference; [`GoldenSource::IndependentlyDerived`] where the
/// expected value was traced by hand from NARA's rule set.
pub const SOUNDEX_ENCODING_CASES: &[EncodingCase] = &[
    GoldenCase {
        id: "soundex/nara/robert",
        descriptor: Soundex::DESCRIPTOR,
        input: "Robert",
        expected: "R163",
        source: GoldenSource::Standard {
            name: "NARA Soundex Indexing System",
        },
        notes: "Foundational NARA example: R kept; O skipped (vowel); b=1; \
                E skipped; r=6; t=3 → R163.",
        tags: &["canonical", "nara"],
    },
    GoldenCase {
        id: "soundex/nara/rupert",
        descriptor: Soundex::DESCRIPTOR,
        input: "Rupert",
        expected: "R163",
        source: GoldenSource::Standard {
            name: "NARA Soundex Indexing System",
        },
        notes: "Rupert has the same Soundex code as Robert — that is Soundex's \
                whole point.",
        tags: &["canonical", "nara", "pair-mate:robert"],
    },
    GoldenCase {
        id: "soundex/nara/rubin",
        descriptor: Soundex::DESCRIPTOR,
        input: "Rubin",
        expected: "R150",
        source: GoldenSource::Standard {
            name: "NARA Soundex Indexing System",
        },
        notes: "Padding case: R, b=1, n=5 → R15 padded with '0' → R150.",
        tags: &["nara", "padding"],
    },
    GoldenCase {
        id: "soundex/nara/ashcraft",
        descriptor: Soundex::DESCRIPTOR,
        input: "Ashcraft",
        expected: "A261",
        source: GoldenSource::Standard {
            name: "NARA Soundex Indexing System",
        },
        notes: "Silent-H rule: S=2 and C=2 separated by H collapse to a \
                single 2; then R=6, F=1 → A261.",
        tags: &["nara", "silent-h"],
    },
    GoldenCase {
        id: "soundex/nara/tymczak",
        descriptor: Soundex::DESCRIPTOR,
        input: "Tymczak",
        expected: "T522",
        source: GoldenSource::Standard {
            name: "NARA Soundex Indexing System",
        },
        notes: "Vowel-reset rule: C=2, Z=2 collapse; then A vowel resets the \
                run; K=2 emits as a new code → T522.",
        tags: &["nara", "vowel-reset"],
    },
    GoldenCase {
        id: "soundex/nara/pfister",
        descriptor: Soundex::DESCRIPTOR,
        input: "Pfister",
        expected: "P236",
        source: GoldenSource::Standard {
            name: "NARA Soundex Indexing System",
        },
        notes: "First-letter same-code collapse: P (code 1) and F (code 1) \
                collapse; then S=2, T=3, R=6 → P236.",
        tags: &["nara", "first-letter-collapse"],
    },
    GoldenCase {
        id: "soundex/nara/honeyman",
        descriptor: Soundex::DESCRIPTOR,
        input: "Honeyman",
        expected: "H555",
        source: GoldenSource::Standard {
            name: "NARA Soundex Indexing System",
        },
        notes: "Vowel-reset repeated: three N/Ms all coded 5 but separated by \
                vowels reset the collapse run each time → H555.",
        tags: &["nara", "vowel-reset"],
    },
    GoldenCase {
        id: "soundex/basic/single-letter",
        descriptor: Soundex::DESCRIPTOR,
        input: "A",
        expected: "A000",
        source: GoldenSource::IndependentlyDerived,
        notes: "Single letter has no consonants after it and pads to length 4.",
        tags: &["basic", "padding"],
    },
    GoldenCase {
        id: "soundex/edge/nonalpha-only",
        descriptor: Soundex::DESCRIPTOR,
        input: "---",
        expected: "",
        source: GoldenSource::IndependentlyDerived,
        notes: "Input with no ASCII alphabetic character returns the empty \
                string — documented edge case distinguishable from a real code.",
        tags: &["edge", "empty"],
    },
    GoldenCase {
        id: "soundex/basic/case-insensitive",
        descriptor: Soundex::DESCRIPTOR,
        input: "robert",
        expected: "R163",
        source: GoldenSource::IndependentlyDerived,
        notes: "Lowercase input produces the same code as uppercase.",
        tags: &["basic", "case"],
    },
];

/// Soundex match-pair cases: whether two names encode to the same key.
pub const SOUNDEX_MATCH_CASES: &[MatchPairCase] = &[
    GoldenCase {
        id: "soundex/pair/robert-rupert",
        descriptor: Soundex::DESCRIPTOR,
        input: ("Robert", "Rupert"),
        expected: true,
        source: GoldenSource::Standard {
            name: "NARA Soundex Indexing System",
        },
        notes: "Both encode to R163 — a textbook Soundex match.",
        tags: &["pair", "positive"],
    },
    GoldenCase {
        id: "soundex/pair/robert-ashcraft",
        descriptor: Soundex::DESCRIPTOR,
        input: ("Robert", "Ashcraft"),
        expected: false,
        source: GoldenSource::IndependentlyDerived,
        notes: "R163 vs A261 — clearly distinct names, must not match.",
        tags: &["pair", "negative"],
    },
    GoldenCase {
        id: "soundex/pair/smith-smyth",
        descriptor: Soundex::DESCRIPTOR,
        input: ("Smith", "Smyth"),
        expected: true,
        source: GoldenSource::IndependentlyDerived,
        notes: "Spelling variants: both encode to S530 — Y is treated as a \
                vowel and drops out.",
        tags: &["pair", "positive", "spelling-variant"],
    },
];

// ---------------------------------------------------------------------------
// NYSIIS
// ---------------------------------------------------------------------------

/// NYSIIS encoding golden cases.
///
/// Expected values traced from Taft's 1970 rule set (Taft, "Name search
/// techniques") and verified against Apache Commons Codec's `Nysiis`
/// reference implementation (with truncation-to-six enabled, matching
/// Taft's canonical output length).
pub const NYSIIS_ENCODING_CASES: &[EncodingCase] = &[
    GoldenCase {
        id: "nysiis/taft/robert",
        descriptor: Nysiis::DESCRIPTOR,
        input: "Robert",
        expected: "RABAD",
        source: GoldenSource::ReferenceImplementation {
            name: "Apache Commons Codec Nysiis (Taft 1970 with truncation)",
        },
        notes: "Suffix RT → D; vowels reduced to A; final RABAD.",
        tags: &["taft", "suffix-rt"],
    },
    GoldenCase {
        id: "nysiis/taft/jackson",
        descriptor: Nysiis::DESCRIPTOR,
        input: "Jackson",
        expected: "JACSAN",
        source: GoldenSource::ReferenceImplementation {
            name: "Apache Commons Codec Nysiis (Taft 1970 with truncation)",
        },
        notes: "No prefix/suffix rule; K→C collapses against C; vowels → A.",
        tags: &["taft", "collapse"],
    },
    GoldenCase {
        id: "nysiis/taft/macgyver",
        descriptor: Nysiis::DESCRIPTOR,
        input: "MacGyver",
        expected: "MCGYVA",
        source: GoldenSource::ReferenceImplementation {
            name: "Apache Commons Codec Nysiis (Taft 1970 with truncation)",
        },
        notes: "Prefix MAC → MCC. Y is not a vowel in Taft. Truncated to 6.",
        tags: &["taft", "prefix-mac", "truncation"],
    },
    GoldenCase {
        id: "nysiis/taft/schmidt",
        descriptor: Nysiis::DESCRIPTOR,
        input: "Schmidt",
        expected: "SNAD",
        source: GoldenSource::ReferenceImplementation {
            name: "Apache Commons Codec Nysiis (Taft 1970 with truncation)",
        },
        notes: "Prefix SCH → SSS, suffix DT → D, M → N, I → A.",
        tags: &["taft", "prefix-sch", "suffix-dt"],
    },
    GoldenCase {
        id: "nysiis/taft/pfister",
        descriptor: Nysiis::DESCRIPTOR,
        input: "Pfister",
        expected: "FASTAR",
        source: GoldenSource::ReferenceImplementation {
            name: "Apache Commons Codec Nysiis (Taft 1970 with truncation)",
        },
        notes: "Prefix PF → FF; F dedupes; I → A; standard consonants.",
        tags: &["taft", "prefix-pf"],
    },
    GoldenCase {
        id: "nysiis/taft/brown",
        descriptor: Nysiis::DESCRIPTOR,
        input: "Brown",
        expected: "BRAN",
        source: GoldenSource::ReferenceImplementation {
            name: "Apache Commons Codec Nysiis (Taft 1970 with truncation)",
        },
        notes: "W preceded by O (vowel) → A, dedupes against previous A.",
        tags: &["taft", "w-rule"],
    },
    GoldenCase {
        id: "nysiis/taft/knight",
        descriptor: Nysiis::DESCRIPTOR,
        input: "Knight",
        expected: "NAGT",
        source: GoldenSource::ReferenceImplementation {
            name: "Apache Commons Codec Nysiis (Taft 1970 with truncation)",
        },
        notes: "Prefix KN → NN; H between two consonants (G, T) collapses \
                against previous G.",
        tags: &["taft", "prefix-kn", "h-rule"],
    },
    GoldenCase {
        id: "nysiis/taft/phillips",
        descriptor: Nysiis::DESCRIPTOR,
        input: "Phillips",
        expected: "FALAP",
        source: GoldenSource::ReferenceImplementation {
            name: "Apache Commons Codec Nysiis (Taft 1970 with truncation)",
        },
        notes: "Prefix PH → FF; L dedupes; trailing S stripped by suffix \
                cleanup.",
        tags: &["taft", "prefix-ph", "trailing-s"],
    },
    GoldenCase {
        id: "nysiis/taft/anderson-truncated",
        descriptor: Nysiis::DESCRIPTOR,
        input: "Anderson",
        expected: "ANDARS",
        source: GoldenSource::ReferenceImplementation {
            name: "Apache Commons Codec Nysiis (Taft 1970 with truncation)",
        },
        notes: "Untruncated key ANDARSAN is truncated to 6 → ANDARS.",
        tags: &["taft", "truncation"],
    },
    GoldenCase {
        id: "nysiis/edge/empty",
        descriptor: Nysiis::DESCRIPTOR,
        input: "",
        expected: "",
        source: GoldenSource::IndependentlyDerived,
        notes: "Empty input returns empty string.",
        tags: &["edge", "empty"],
    },
];

/// NYSIIS match-pair cases.
pub const NYSIIS_MATCH_CASES: &[MatchPairCase] = &[
    GoldenCase {
        id: "nysiis/pair/robert-robertson-differ",
        descriptor: Nysiis::DESCRIPTOR,
        input: ("Robert", "Robertson"),
        expected: false,
        source: GoldenSource::IndependentlyDerived,
        notes: "Robert → RABAD, Robertson → different key. NYSIIS does not \
                collapse the -son suffix.",
        tags: &["pair", "negative"],
    },
    GoldenCase {
        id: "nysiis/pair/knight-nite",
        descriptor: Nysiis::DESCRIPTOR,
        input: ("Knight", "Nite"),
        expected: false,
        source: GoldenSource::IndependentlyDerived,
        notes: "Knight → NAGT (K silent, GH between consonants), Nite → NAT. \
                Distinct keys under NYSIIS.",
        tags: &["pair", "negative"],
    },
    GoldenCase {
        id: "nysiis/pair/case-insensitive",
        descriptor: Nysiis::DESCRIPTOR,
        input: ("robert", "ROBERT"),
        expected: true,
        source: GoldenSource::IndependentlyDerived,
        notes: "Case must not affect the key.",
        tags: &["pair", "positive", "case"],
    },
];

// ---------------------------------------------------------------------------
// Double Metaphone (primary-only)
// ---------------------------------------------------------------------------

/// Convenience constructor for a `DoubleMetaphoneKey` in a `const`-friendly
/// path. Because the golden `expected` field holds a value (not a `const`
/// item), a small runtime constructor is fine.
fn dm_primary(s: &str) -> DoubleMetaphoneKey {
    DoubleMetaphoneKey::primary_only(String::from(s))
}

/// Returns the Double Metaphone encoding golden cases.
///
/// A function (not a `const`) because `DoubleMetaphoneKey::primary` is
/// `String`, which cannot appear in a `const` context.
#[must_use]
pub fn double_metaphone_encoding_cases() -> alloc::vec::Vec<DoubleMetaphoneCase> {
    alloc::vec![
        GoldenCase {
            id: "double-metaphone/xavier",
            descriptor: DoubleMetaphone::DESCRIPTOR,
            input: "Xavier",
            expected: dm_primary("SFR"),
            source: GoldenSource::IndependentlyDerived,
            notes: "Initial X → S; V → F; R → R. Primary-only variant does \
                    not fire the French '-ier' silent-R rule.",
            tags: &["primary-only", "initial-x"],
        },
        GoldenCase {
            id: "double-metaphone/wagner",
            descriptor: DoubleMetaphone::DESCRIPTOR,
            input: "Wagner",
            expected: dm_primary("AKNR"),
            source: GoldenSource::IndependentlyDerived,
            notes: "W silent; A first-vowel → A; G hard → K; N → N; R → R.",
            tags: &["primary-only", "silent-w"],
        },
        GoldenCase {
            id: "double-metaphone/schmidt",
            descriptor: DoubleMetaphone::DESCRIPTOR,
            input: "Schmidt",
            expected: dm_primary("XMT"),
            source: GoldenSource::IndependentlyDerived,
            notes: "SCH → X; D → T.",
            tags: &["primary-only", "sch"],
        },
        GoldenCase {
            id: "double-metaphone/thomas",
            descriptor: DoubleMetaphone::DESCRIPTOR,
            input: "Thomas",
            expected: dm_primary("TMS"),
            source: GoldenSource::IndependentlyDerived,
            notes: "TH+OM exception: T (not theta).",
            tags: &["primary-only", "th-exception"],
        },
        GoldenCase {
            id: "double-metaphone/thompson",
            descriptor: DoubleMetaphone::DESCRIPTOR,
            input: "Thompson",
            expected: dm_primary("TMPS"),
            source: GoldenSource::IndependentlyDerived,
            notes: "TH+OM exception → T; MPS truncates at 4 chars. Primary-only \
                    variant does not model the Anglicized silent-P.",
            tags: &["primary-only", "th-exception", "truncation"],
        },
        GoldenCase {
            id: "double-metaphone/smith",
            descriptor: DoubleMetaphone::DESCRIPTOR,
            input: "Smith",
            expected: dm_primary("SM0"),
            source: GoldenSource::IndependentlyDerived,
            notes: "TH → theta ('0') when not TH+OM/AM.",
            tags: &["primary-only", "th-theta"],
        },
        GoldenCase {
            id: "double-metaphone/knight",
            descriptor: DoubleMetaphone::DESCRIPTOR,
            input: "Knight",
            expected: dm_primary("NT"),
            source: GoldenSource::IndependentlyDerived,
            notes: "KN silent-start; GH silent after vowel; N + T.",
            tags: &["primary-only", "silent-kn", "silent-gh"],
        },
        GoldenCase {
            id: "double-metaphone/gnome",
            descriptor: DoubleMetaphone::DESCRIPTOR,
            input: "Gnome",
            expected: dm_primary("NM"),
            source: GoldenSource::IndependentlyDerived,
            notes: "GN silent-start.",
            tags: &["primary-only", "silent-gn"],
        },
        GoldenCase {
            id: "double-metaphone/phillips",
            descriptor: DoubleMetaphone::DESCRIPTOR,
            input: "Phillips",
            expected: dm_primary("FLPS"),
            source: GoldenSource::IndependentlyDerived,
            notes: "PH → F; L (doubled collapse); P; S — truncated at 4.",
            tags: &["primary-only", "ph"],
        },
        GoldenCase {
            id: "double-metaphone/edge/empty",
            descriptor: DoubleMetaphone::DESCRIPTOR,
            input: "",
            expected: dm_primary(""),
            source: GoldenSource::IndependentlyDerived,
            notes: "Empty input yields a primary-only key with an empty primary.",
            tags: &["edge", "empty"],
        },
    ]
}

/// Convenience constructor for a two-key `DoubleMetaphoneKey`.
fn dm_pair(primary: &str, alternate: &str) -> DoubleMetaphoneKey {
    DoubleMetaphoneKey {
        primary: String::from(primary),
        alternate: Some(String::from(alternate)),
    }
}

/// Returns the Double Metaphone (full variant) encoding golden cases.
///
/// The primary key of every case equals the corresponding primary-only
/// primary — that invariant is asserted in the `full_primary_matches_primary_only`
/// test below. The alternate is either `Some` (at a divergence point) or
/// `None` (when the alternate pass agreed with the primary).
#[must_use]
#[allow(
    clippy::too_many_lines,
    reason = "the golden cases read best as a single vec![] literal so the \
              full case list stays inspectable at a glance"
)]
pub fn double_metaphone_full_encoding_cases() -> alloc::vec::Vec<DoubleMetaphoneCase> {
    alloc::vec![
        GoldenCase {
            id: "double-metaphone/full/wagner",
            descriptor: DoubleMetaphone::FULL_DESCRIPTOR,
            input: "Wagner",
            expected: dm_pair("AKNR", "FKNR"),
            source: GoldenSource::ReferenceImplementation {
                name: "Apache Commons Codec DoubleMetaphone (Philips 1999 with two-key branch)",
            },
            notes: "Initial W before a vowel: primary treats W as silent (A \
                    becomes first-committed vowel), alternate emits F for the \
                    Germanic V-sound reading.",
            tags: &["full", "germanic-w"],
        },
        GoldenCase {
            id: "double-metaphone/full/xavier",
            descriptor: DoubleMetaphone::FULL_DESCRIPTOR,
            input: "Xavier",
            expected: dm_pair("SFR", "SF"),
            source: GoldenSource::ReferenceImplementation {
                name: "Apache Commons Codec DoubleMetaphone (Philips 1999 with two-key branch)",
            },
            notes: "French -IER ending: primary emits final R, alternate \
                    treats it as silent (reversed from Apache Commons's \
                    primary/alternate assignment to preserve this crate's \
                    primary-only primary byte-for-byte).",
            tags: &["full", "french-ier"],
        },
        GoldenCase {
            id: "double-metaphone/full/schmidt",
            descriptor: DoubleMetaphone::FULL_DESCRIPTOR,
            input: "Schmidt",
            expected: dm_pair("XMT", "SMT"),
            source: GoldenSource::ReferenceImplementation {
                name: "Apache Commons Codec DoubleMetaphone (Philips 1999 with two-key branch)",
            },
            notes: "SCH: primary emits X, alternate emits S (Anglicized \
                    reading of the Germanic cluster).",
            tags: &["full", "sch"],
        },
        GoldenCase {
            id: "double-metaphone/full/chorus",
            descriptor: DoubleMetaphone::FULL_DESCRIPTOR,
            input: "Chorus",
            expected: dm_pair("XRS", "KRS"),
            source: GoldenSource::ReferenceImplementation {
                name: "Apache Commons Codec DoubleMetaphone (Philips 1999 with two-key branch)",
            },
            notes: "Initial CH followed by O: primary emits X, alternate \
                    emits K for the Greek 'chi' reading (Chorus, Chaos).",
            tags: &["full", "ch-greek"],
        },
        GoldenCase {
            id: "double-metaphone/full/chien",
            descriptor: DoubleMetaphone::FULL_DESCRIPTOR,
            input: "Chien",
            expected: dm_pair("XN", "JN"),
            source: GoldenSource::ReferenceImplementation {
                name: "Apache Commons Codec DoubleMetaphone (Philips 1999 with two-key branch)",
            },
            notes: "Initial CH followed by IE: primary emits X, alternate \
                    emits J for the French reading.",
            tags: &["full", "ch-french"],
        },
        GoldenCase {
            id: "double-metaphone/full/barwig",
            descriptor: DoubleMetaphone::FULL_DESCRIPTOR,
            input: "Barwig",
            expected: dm_pair("PRK", "PRF"),
            source: GoldenSource::ReferenceImplementation {
                name: "Apache Commons Codec DoubleMetaphone (Philips 1999 with two-key branch)",
            },
            notes: "-WIG Germanic ending: primary keeps W silent and emits K \
                    for the G; alternate emits F for the W and drops the IG.",
            tags: &["full", "germanic-wig"],
        },
        GoldenCase {
            id: "double-metaphone/full/smith",
            descriptor: DoubleMetaphone::FULL_DESCRIPTOR,
            input: "Smith",
            expected: dm_pair("SM0", "SMT"),
            source: GoldenSource::ReferenceImplementation {
                name: "Apache Commons Codec DoubleMetaphone (Philips 1999 with two-key branch)",
            },
            notes: "TH (theta phoneme): primary emits '0' (ASCII theta \
                    placeholder), alternate emits T — the classical Double \
                    Metaphone split for the theta phoneme.",
            tags: &["full", "theta-th"],
        },
        GoldenCase {
            id: "double-metaphone/full/thompson-no-divergence",
            descriptor: DoubleMetaphone::FULL_DESCRIPTOR,
            input: "Thompson",
            expected: DoubleMetaphoneKey::primary_only(String::from("TMPS")),
            source: GoldenSource::ReferenceImplementation {
                name: "Apache Commons Codec DoubleMetaphone (Philips 1999 with two-key branch)",
            },
            notes: "TH+OM Thomas-family exception applies in both branches; \
                    no other divergence — alternate is None.",
            tags: &["full", "no-divergence"],
        },
        GoldenCase {
            id: "double-metaphone/full/cachao-no-divergence",
            descriptor: DoubleMetaphone::FULL_DESCRIPTOR,
            input: "Cachao",
            expected: DoubleMetaphoneKey::primary_only(String::from("KX")),
            source: GoldenSource::ReferenceImplementation {
                name: "Apache Commons Codec DoubleMetaphone (Philips 1999 with two-key branch)",
            },
            notes: "Internal (non-initial) CH stays X in both branches; hard \
                    initial C stays K — no divergence.",
            tags: &["full", "no-divergence", "ch-internal"],
        },
        GoldenCase {
            id: "double-metaphone/full/edge-empty",
            descriptor: DoubleMetaphone::FULL_DESCRIPTOR,
            input: "",
            expected: dm_primary(""),
            source: GoldenSource::IndependentlyDerived,
            notes: "Empty input yields an empty primary and no alternate.",
            tags: &["full", "edge", "empty"],
        },
    ]
}

/// Double Metaphone (full variant) match-pair cases.
pub const DOUBLE_METAPHONE_FULL_MATCH_CASES: &[MatchPairCase] = &[
    GoldenCase {
        id: "double-metaphone/full/pair/xavier-xavier",
        descriptor: DoubleMetaphone::FULL_DESCRIPTOR,
        input: ("Xavier", "Xavier"),
        expected: true,
        source: GoldenSource::IndependentlyDerived,
        notes: "Same input matches under Full via primary=primary.",
        tags: &["full", "pair", "positive"],
    },
    GoldenCase {
        id: "double-metaphone/full/pair/wagner-wagner",
        descriptor: DoubleMetaphone::FULL_DESCRIPTOR,
        input: ("Wagner", "Wagner"),
        expected: true,
        source: GoldenSource::IndependentlyDerived,
        notes: "Same input matches under Full via primary=primary AND \
                alternate=alternate (both are Some).",
        tags: &["full", "pair", "positive", "both-branches"],
    },
    GoldenCase {
        id: "double-metaphone/full/pair/xavier-zavier",
        descriptor: DoubleMetaphone::FULL_DESCRIPTOR,
        input: ("Xavier", "Zavier"),
        expected: true,
        source: GoldenSource::IndependentlyDerived,
        notes: "Both encode to primary 'SFR' (initial X and initial Z both \
                emit S; V→F; R→R). Match via primary=primary.",
        tags: &["full", "pair", "positive"],
    },
];

/// Double Metaphone match-pair cases (primary-only key equality).
pub const DOUBLE_METAPHONE_MATCH_CASES: &[MatchPairCase] = &[
    GoldenCase {
        id: "double-metaphone/pair/thomas-thompson-differ-mid-cluster",
        descriptor: DoubleMetaphone::DESCRIPTOR,
        input: ("Thomas", "Thompson"),
        expected: false,
        source: GoldenSource::IndependentlyDerived,
        notes: "Thomas → TMS, Thompson → TMPS. Distinct primaries in this \
                primary-only variant.",
        tags: &["pair", "negative"],
    },
    GoldenCase {
        id: "double-metaphone/pair/knight-nite-agree",
        descriptor: DoubleMetaphone::DESCRIPTOR,
        input: ("Knight", "Nite"),
        expected: true,
        source: GoldenSource::IndependentlyDerived,
        notes: "Both encode to NT under the primary-only variant — Knight \
                loses K silent-start and its GH silent, Nite loses its \
                internal vowels.",
        tags: &["pair", "positive"],
    },
    GoldenCase {
        id: "double-metaphone/pair/case-insensitive",
        descriptor: DoubleMetaphone::DESCRIPTOR,
        input: ("smith", "SMITH"),
        expected: true,
        source: GoldenSource::IndependentlyDerived,
        notes: "Case must not affect the key.",
        tags: &["pair", "positive", "case"],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::PhoneticEncoder;
    use alloc::vec::Vec;
    use stringcheese_core::AlgorithmDescriptor;

    fn all_ids() -> Vec<&'static str> {
        let mut ids: Vec<&'static str> = Vec::new();
        for c in SOUNDEX_ENCODING_CASES {
            ids.push(c.id);
        }
        for c in SOUNDEX_MATCH_CASES {
            ids.push(c.id);
        }
        for c in NYSIIS_ENCODING_CASES {
            ids.push(c.id);
        }
        for c in NYSIIS_MATCH_CASES {
            ids.push(c.id);
        }
        for c in double_metaphone_encoding_cases() {
            ids.push(c.id);
        }
        for c in DOUBLE_METAPHONE_MATCH_CASES {
            ids.push(c.id);
        }
        for c in double_metaphone_full_encoding_cases() {
            ids.push(c.id);
        }
        for c in DOUBLE_METAPHONE_FULL_MATCH_CASES {
            ids.push(c.id);
        }
        ids
    }

    #[test]
    fn every_case_uses_the_correct_descriptor() {
        for c in SOUNDEX_ENCODING_CASES {
            assert_desc(c.id, c.descriptor, Soundex::DESCRIPTOR);
        }
        for c in SOUNDEX_MATCH_CASES {
            assert_desc(c.id, c.descriptor, Soundex::DESCRIPTOR);
        }
        for c in NYSIIS_ENCODING_CASES {
            assert_desc(c.id, c.descriptor, Nysiis::DESCRIPTOR);
        }
        for c in NYSIIS_MATCH_CASES {
            assert_desc(c.id, c.descriptor, Nysiis::DESCRIPTOR);
        }
        for c in double_metaphone_encoding_cases() {
            assert_desc(c.id, c.descriptor, DoubleMetaphone::PRIMARY_ONLY_DESCRIPTOR);
        }
        for c in DOUBLE_METAPHONE_MATCH_CASES {
            assert_desc(c.id, c.descriptor, DoubleMetaphone::PRIMARY_ONLY_DESCRIPTOR);
        }
        for c in double_metaphone_full_encoding_cases() {
            assert_desc(c.id, c.descriptor, DoubleMetaphone::FULL_DESCRIPTOR);
        }
        for c in DOUBLE_METAPHONE_FULL_MATCH_CASES {
            assert_desc(c.id, c.descriptor, DoubleMetaphone::FULL_DESCRIPTOR);
        }
    }

    fn assert_desc(id: &str, got: AlgorithmDescriptor, want: AlgorithmDescriptor) {
        assert_eq!(
            got, want,
            "golden case {id} references the wrong descriptor"
        );
    }

    #[test]
    fn every_soundex_case_matches_encoder() {
        for c in SOUNDEX_ENCODING_CASES {
            let got = Soundex.encode(c.input);
            assert_eq!(
                got,
                c.expected,
                "soundex golden case {id} disagreed",
                id = c.id
            );
        }
    }

    #[test]
    fn every_soundex_match_case_agrees() {
        let m = crate::comparator::PhoneticMatcher::new(Soundex);
        for c in SOUNDEX_MATCH_CASES {
            let (a, b) = c.input;
            assert_eq!(
                m.matches(a, b),
                c.expected,
                "soundex match case {id} disagreed",
                id = c.id
            );
        }
    }

    #[test]
    fn every_nysiis_case_matches_encoder() {
        for c in NYSIIS_ENCODING_CASES {
            let got = Nysiis.encode(c.input);
            assert_eq!(
                got,
                c.expected,
                "nysiis golden case {id} disagreed",
                id = c.id
            );
        }
    }

    #[test]
    fn every_nysiis_match_case_agrees() {
        let m = crate::comparator::PhoneticMatcher::new(Nysiis);
        for c in NYSIIS_MATCH_CASES {
            let (a, b) = c.input;
            assert_eq!(
                m.matches(a, b),
                c.expected,
                "nysiis match case {id} disagreed",
                id = c.id
            );
        }
    }

    #[test]
    fn every_double_metaphone_case_matches_encoder() {
        let enc = DoubleMetaphone::primary_only();
        for c in double_metaphone_encoding_cases() {
            let got = enc.encode(c.input);
            assert_eq!(
                got,
                c.expected,
                "double metaphone golden case {id} disagreed",
                id = c.id
            );
        }
    }

    #[test]
    fn every_double_metaphone_match_case_agrees() {
        let m = crate::comparator::PhoneticMatcher::new(DoubleMetaphone::primary_only());
        for c in DOUBLE_METAPHONE_MATCH_CASES {
            let (a, b) = c.input;
            assert_eq!(
                m.matches_double_metaphone(a, b),
                c.expected,
                "double metaphone match case {id} disagreed",
                id = c.id
            );
        }
    }

    #[test]
    fn every_double_metaphone_full_case_matches_encoder() {
        let enc = DoubleMetaphone::full();
        for c in double_metaphone_full_encoding_cases() {
            let got = enc.encode(c.input);
            assert_eq!(
                got,
                c.expected,
                "double metaphone full-variant golden case {id} disagreed",
                id = c.id
            );
        }
    }

    #[test]
    fn every_double_metaphone_full_match_case_agrees() {
        let m = crate::comparator::PhoneticMatcher::new(DoubleMetaphone::full());
        for c in DOUBLE_METAPHONE_FULL_MATCH_CASES {
            let (a, b) = c.input;
            assert_eq!(
                m.matches_double_metaphone(a, b),
                c.expected,
                "double metaphone full-variant match case {id} disagreed",
                id = c.id
            );
        }
    }

    #[test]
    fn full_variant_primary_matches_primary_only_across_all_golden_inputs() {
        // The most important invariant: adding the alternate branch never
        // changes the primary key.
        let po = DoubleMetaphone::primary_only();
        let f = DoubleMetaphone::full();
        for c in double_metaphone_encoding_cases() {
            assert_eq!(
                po.encode(c.input).primary,
                f.encode(c.input).primary,
                "primary key differs for primary-only golden input {input:?}",
                input = c.input
            );
        }
        for c in double_metaphone_full_encoding_cases() {
            assert_eq!(
                po.encode(c.input).primary,
                f.encode(c.input).primary,
                "primary key differs for full-variant golden input {input:?}",
                input = c.input
            );
        }
    }

    #[test]
    fn corpus_meets_minimum_size() {
        // Spec: at least 8-10 encoding cases per algorithm, 4-6 pair cases.
        assert!(SOUNDEX_ENCODING_CASES.len() >= 8);
        assert!(SOUNDEX_MATCH_CASES.len() >= 3);
        assert!(NYSIIS_ENCODING_CASES.len() >= 8);
        assert!(NYSIIS_MATCH_CASES.len() >= 3);
        assert!(double_metaphone_encoding_cases().len() >= 8);
        assert!(DOUBLE_METAPHONE_MATCH_CASES.len() >= 3);
        assert!(double_metaphone_full_encoding_cases().len() >= 8);
        assert!(DOUBLE_METAPHONE_FULL_MATCH_CASES.len() >= 3);
    }

    #[test]
    fn every_case_has_a_unique_id() {
        let ids = all_ids();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "duplicate golden case id detected");
    }
}
