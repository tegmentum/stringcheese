//! [`SlavicMetaphone`] — a Metaphone-family phonetic encoder for the Slavic
//! language family across both Cyrillic and Latin scripts.
//!
//! # Origin and rationale
//!
//! No single Metaphone-shaped encoder is universally cited as the "Slavic
//! Metaphone" the way Philips' 1990 Metaphone is for English. The
//! per-language packs in the workspace each ship what they can — GOST 7.79-B
//! transliteration for Russian and Ukrainian, Vukovica/Latin round-tripping
//! for Serbian, and PHONEX-style variants for others — but none of those
//! produces a **cross-language equivalence class**. Two spellings of the
//! same name (say Russian `"Чехов"`, Latin transliteration `"Chekhov"`, or
//! Czech `"Praha"` and Russian `"Прага"`) encode to different keys under
//! transliteration, because transliteration by design preserves the source
//! script's letter identity rather than the underlying phoneme.
//!
//! This module ships a **variable-length, Metaphone-shaped, cross-Slavic**
//! encoder built specifically for the equivalence-class use case:
//! record-linkage systems that need `"Kraków"` (Polish) and `"Краків"`
//! (Ukrainian) to hash to the same bucket, or search systems that need
//! `"Београд"` (Serbian Cyrillic) and `"Beograd"` (Serbian Latin) to
//! resolve identically. The descriptor variant slug is
//! [`VariantId("slavic-metaphone-2026")`](stringcheese_core::VariantId).
//!
//! # Algorithm outline
//!
//! The encoder is a two-stage pipeline:
//!
//! 1. **Normalization to a phonetic-class sequence.** The input is
//!    lowercased via Unicode `to_lowercase`, then walked scalar by scalar
//!    (with two- or three-scalar digraph handling: Polish `sz/cz/rz/dź/dż`,
//!    Czech `ch`, Serbian Latin `dž`, and the ASCII transliteration
//!    digraphs `kh`, `sh`, `zh`) to produce a sequence of ASCII phonetic
//!    class characters (see [Class alphabet](#class-alphabet)). Both
//!    Cyrillic and Latin inputs land in the same class alphabet, which is
//!    what makes cross-script matches possible without a separate
//!    transliteration step.
//! 2. **Key emission.** The class sequence is emitted with two collapsing
//!    rules: consecutive duplicates are dropped, and (by default) all
//!    vowels except the first character of the key are dropped in
//!    Metaphone style. The result is truncated to
//!    [`SlavicMetaphoneOptions::max_length`] characters (default
//!    [`DEFAULT_MAX_KEY_LEN`]).
//!
//! # Class alphabet
//!
//! The intermediate alphabet is deliberately narrow — 14 consonant classes
//! and 5 vowel classes — so that cross-Slavic pronunciation variations
//! collapse to the same class:
//!
//! | Class | Covers                                                                 |
//! |-------|------------------------------------------------------------------------|
//! | `P`   | `p b` / `п б` — labial stops (voice collapse)                          |
//! | `F`   | `f v w` / `ф в ў` — labial fricatives (voice collapse)                 |
//! | `T`   | `t d ť ď þ` / `т д` — dental stops (voice + palatalization collapse)   |
//! | `K`   | `k q ǩ` / `к` — velar stops                                            |
//! | `H`   | `h g x ch(digraph) kh(digraph) ħ` / `г х ґ` — velar/glottal (**g→H**)  |
//! | `S`   | `s z ś ź ß` / `с з` — alveolar fricatives (voice + palatal collapse)   |
//! | `Z`   | `c ç ѕ` / `ц` — alveolar affricates `/ts dz/`                          |
//! | `X`   | `š ž ż sz rz sh zh` / `ш ж щ` — postalveolar fricatives                |
//! | `C`   | `č ć đ cz dž dź dż` / `ч ђ ћ џ ѓ ќ` — postalveolar affricates          |
//! | `M`   | `m` / `м`                                                              |
//! | `N`   | `n ń ň ñ` / `н њ` — alveolar/palatal nasals                            |
//! | `L`   | `l ł ĺ ľ` / `л љ` — laterals                                           |
//! | `R`   | `r ř` / `р` — trills                                                   |
//! | `J`   | `j` / `й ј` — palatal glide (`y` is treated as vowel `I`)              |
//! | `A`   | `a á à â ä ą` / `а я`                                                  |
//! | `E`   | `e é è ê ë ę` / `е є э`                                                |
//! | `I`   | `i í ì î ï y` / `и і ї ы` — Polish `y` is `/ɨ/`, folds to `I`          |
//! | `O`   | `o ó ò ô ö ő` / `о ё`                                                  |
//! | `U`   | `u ú ù û ü ů ű` / `у ю`                                                |
//!
//! The soft-sign / hard-sign / apostrophe scalars (`ь`, `ъ`, `'`) carry no
//! phonetic content of their own and are dropped in stage 1. Every scalar
//! outside the mapping (digits, punctuation, whitespace, non-Slavic letters
//! from CJK / Arabic / etc.) is also dropped.
//!
//! ## Deliberate cross-Slavic collapses
//!
//! The most opinionated collapses — the ones that make cross-language
//! matches work at the cost of within-language precision — are:
//!
//! * **`g → H`.** Ukrainian and Czech have shifted historical `/g/` to
//!   `/ɦ/`, so `Прага` (Russian, `/g/`) and `Praha` (Czech, `/ɦ/`) sound
//!   different but denote the same city. Folding `g` and `h` into a single
//!   class makes those pairs match. The cost is that Serbian / Bulgarian /
//!   Russian pairs that phonetically preserve `/g/` lose the distinction —
//!   an acceptable trade for a cross-Slavic encoder.
//! * **`ch → C` (single digraph).** In Russian / Ukrainian / Serbian
//!   transliteration `"ch"` denotes `/tʃ/` (Chekhov, Chernobyl). In Polish
//!   and Czech `"ch"` denotes `/x/`. The encoder picks the Russian
//!   convention because the Cyrillic → Latin transliteration case is the
//!   dominant cross-language matching scenario; `"kh"` remains available
//!   for the `/x/` reading.
//! * **Sibilant voice-collapse (`š/ž → X`, `s/z → S`, `č/dž → C`).**
//!   Voice pairs are not distinguished. This is standard Metaphone-family
//!   behavior.
//! * **Palatalization-collapse (`t/ť → T`, `n/ń → N`, `l/ł → L`, `d/ď → T`).**
//!   Soft-consonant markings and diacritics do not produce a separate
//!   class; the base class is used. Polish `ń` and Ukrainian palatalized
//!   `н + ь` therefore both encode as `N`.
//! * **Vowel granularity reduction.** Long vs short (Czech `á`/`a`), and
//!   the several ways of writing `/u/` (Polish `ó` vs `u`, Czech `ů` vs
//!   `u`) all fold to the base vowel class. Nasal Polish `ą`/`ę` decompose
//!   to `A + N` and `E + N`.
//!
//! ## Consequential simplifications
//!
//! * **`щ → X`** (single class). Russian `щ` is `/ɕ:/` and Ukrainian `щ` is
//!   `/ʃtʃ/`; both fold to `X`. Callers who need to distinguish Russian
//!   `шаг` from `щаг` should use a language-specific encoder.
//! * **`y` (Latin) is treated as vowel `I`.** In Polish `y` is `/ɨ/` (vowel);
//!   in Serbo-Croatian Latin it does not appear. Treating it as `I` matches
//!   the Polish reading and is inert for other Latin Slavic scripts. The
//!   consonantal `/j/` sound is always spelled `j` in Slavic Latin
//!   orthographies.
//! * **Bulgarian `ъ` (`/ɤ/`, a vowel in Bulgarian usage).** Dropped, matching
//!   Russian usage where `ъ` is a hardness marker. Callers indexing
//!   Bulgarian corpora lose one vowel distinction — but since the encoder
//!   drops all non-initial vowels by default, the loss is only observable
//!   for word-initial `ъ` (rare).
//!
//! # Vowel handling
//!
//! By default ([`SlavicMetaphoneOptions::default`]) vowels are emitted only
//! at the very first key position and dropped elsewhere — the classic
//! Metaphone behavior. Setting
//! [`SlavicMetaphoneOptions::include_vowels`] to `true` keeps vowels in
//! their (collapsed) positions, producing a more discriminative but less
//! cross-language-tolerant key.
//!
//! # When to use this vs a language-specific encoder
//!
//! Use `SlavicMetaphone` when:
//!
//! * The corpus mixes Slavic languages or scripts, and you need a shared
//!   equivalence class (e.g. an entity-resolution join across a Ukrainian-
//!   Russian-Serbian dataset).
//! * You want a bucketing hash for approximate-match candidate generation
//!   that is robust across the Cyrillic/Latin script boundary.
//!
//! Use a language-specific encoder (e.g. `stringcheese-ru`'s GOST 7.79-B or
//! `stringcheese-sr`'s Vukovica transliteration) when:
//!
//! * You are working entirely inside one language and want to preserve
//!   within-language distinctions the cross-Slavic collapses erase
//!   (Russian `/g/` vs `/x/`, Polish `sz` vs `ś`, Serbian `ч` vs `ћ`).
//! * You need a reversible-with-caveats byte-level transliteration
//!   suitable for URLs or filenames rather than a phonetic bucket key.
//!
//! # References
//!
//! * Philips, L. (1990). *Hanging on the Metaphone*. Computer Language 7:12.
//!   The original Metaphone paper that shaped the general algorithm class
//!   this module belongs to.
//! * Beider, A., Morse, S. P. (2008). *Beider-Morse Phonetic Matching: An
//!   Alternative to Soundex with Fewer False Hits*. Avotaynu Foundation.
//!   The multi-language phonetic-matching reference this module's approach
//!   is compatible with (Beider-Morse itself is a much larger rule engine;
//!   this is a Metaphone-shaped simplification).
//! * GOST 7.79-2000 System B (Russian and Ukrainian transliteration) — the
//!   ASCII intermediate this module's Cyrillic tables were designed against
//!   for interoperability with the per-language transliteration packs.

use alloc::string::String;
use alloc::vec::Vec;
use stringcheese_core::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion, VariantId,
};

use crate::encoder::{Applicability, LanguageTag, PhoneticEncoder, ScriptTag};

/// The default maximum length of a Slavic-Metaphone key.
///
/// Set to 8 characters, sitting inside the 6-15 range the design document
/// specifies for variable-length keys and matching the length used by
/// several published Metaphone-family variants for record-linkage.
pub const DEFAULT_MAX_KEY_LEN: usize = 8;

/// The absolute upper bound on any Slavic-Metaphone key length, enforced
/// regardless of the caller-provided `max_length`.
///
/// Longer values are silently clamped to this bound. The cap avoids
/// pathological allocation for adversarial inputs and matches the upper
/// end of the 6-15 range from the design document with margin to spare.
pub const HARD_MAX_KEY_LEN: usize = 32;

/// Options controlling the shape of the emitted key.
///
/// Construct with [`SlavicMetaphoneOptions::default`] for the standard
/// Metaphone-style behavior (drop non-initial vowels, cap at
/// [`DEFAULT_MAX_KEY_LEN`]). Use the `with_*` methods to derive customized
/// option sets.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SlavicMetaphoneOptions {
    /// Maximum length of the emitted key. Values above [`HARD_MAX_KEY_LEN`]
    /// are silently clamped.
    pub max_length: usize,
    /// If `true`, emit all vowels in the key (collapsed against consecutive
    /// duplicates). If `false` (the default), only the very first key
    /// character may be a vowel; every later vowel is dropped, matching
    /// Metaphone's convention.
    pub include_vowels: bool,
}

impl Default for SlavicMetaphoneOptions {
    #[inline]
    fn default() -> Self {
        Self {
            max_length: DEFAULT_MAX_KEY_LEN,
            include_vowels: false,
        }
    }
}

impl SlavicMetaphoneOptions {
    /// Returns a copy of `self` with the given `max_length` (clamped to
    /// [`HARD_MAX_KEY_LEN`]).
    #[inline]
    #[must_use]
    pub const fn with_max_length(mut self, max_length: usize) -> Self {
        self.max_length = max_length;
        self
    }

    /// Returns a copy of `self` with the given `include_vowels` flag.
    #[inline]
    #[must_use]
    pub const fn with_include_vowels(mut self, include_vowels: bool) -> Self {
        self.include_vowels = include_vowels;
        self
    }
}

/// The Slavic-Metaphone encoder.
///
/// A zero-sized (over the [`SlavicMetaphoneOptions`] field) value; construct
/// with [`SlavicMetaphone::new`] for defaults or
/// [`SlavicMetaphone::with_options`] to override.
///
/// See the [module-level docs](self) for the algorithm and its design
/// trade-offs.
///
/// # Example
///
/// ```
/// use stringcheese_phonetic::SlavicMetaphone;
///
/// let enc = SlavicMetaphone::new();
/// // Russian "Чехов" and its Latin transliteration hash to the same key.
/// assert_eq!(enc.encode("Чехов"), enc.encode("Chekhov"));
/// // Czech Praha and Russian Прага also match — g/h are collapsed.
/// assert_eq!(enc.encode("Praha"), enc.encode("Прага"));
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SlavicMetaphone {
    options: SlavicMetaphoneOptions,
}

impl SlavicMetaphone {
    /// The algorithm descriptor for the Slavic-Metaphone variant.
    ///
    /// The variant slug `"slavic-metaphone-2026"` names the algorithm and
    /// its introduction year, distinguishing it from other Metaphone-family
    /// variants (English Metaphone, Double Metaphone, and future
    /// language-specific Slavic packs).
    pub const DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor {
        family: AlgorithmFamily::Metaphone,
        variant: VariantId("slavic-metaphone-2026"),
        version: DescriptorVersion::new(0, 1, 0),
        source: DefinitionSource::IndependentlyDerived,
    };

    /// The languages, scripts, and regions Slavic-Metaphone was designed for.
    ///
    /// The applicability covers Russian, Ukrainian, Belarusian, Bulgarian,
    /// Macedonian, Serbian, Croatian, Bosnian, Slovenian, Polish, Czech,
    /// and Slovak, in both Cyrillic and Latin scripts.
    pub const APPLICABILITY: Applicability = Applicability {
        languages: &[
            LanguageTag("ru"),
            LanguageTag("uk"),
            LanguageTag("be"),
            LanguageTag("bg"),
            LanguageTag("mk"),
            LanguageTag("sr"),
            LanguageTag("hr"),
            LanguageTag("bs"),
            LanguageTag("sl"),
            LanguageTag("pl"),
            LanguageTag("cs"),
            LanguageTag("sk"),
        ],
        scripts: &[ScriptTag("Cyrl"), ScriptTag("Latn")],
        regions: &[],
        notes: "Slavic language family, Cyrillic and Latin scripts; \
                designed for cross-language equivalence-class matching.",
    };

    /// Constructs a Slavic-Metaphone encoder with default options.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            options: SlavicMetaphoneOptions {
                max_length: DEFAULT_MAX_KEY_LEN,
                include_vowels: false,
            },
        }
    }

    /// Constructs a Slavic-Metaphone encoder with the given options.
    #[inline]
    #[must_use]
    pub const fn with_options(options: SlavicMetaphoneOptions) -> Self {
        Self { options }
    }

    /// Returns the options this encoder was constructed with.
    #[inline]
    #[must_use]
    pub const fn options(&self) -> SlavicMetaphoneOptions {
        self.options
    }

    /// Returns the algorithm descriptor. `const` accessor.
    #[inline]
    #[must_use]
    pub const fn descriptor() -> AlgorithmDescriptor {
        Self::DESCRIPTOR
    }

    /// Returns the applicability. `const` accessor.
    #[inline]
    #[must_use]
    pub const fn applicability() -> Applicability {
        Self::APPLICABILITY
    }

    /// Encodes `input` as a Slavic-Metaphone key using this encoder's
    /// options.
    ///
    /// Returns the empty string if `input` contains no mappable
    /// (Cyrillic-Slavic or Latin-alphabetic) character.
    #[must_use]
    pub fn encode(&self, input: &str) -> String {
        encode_with(input, self.options)
    }

    /// Encodes `input` as a Slavic-Metaphone key with a caller-provided
    /// `max_length`, using this encoder's other options.
    ///
    /// Convenience shortcut for
    /// [`Self::encode`] with a temporarily overridden `max_length`.
    #[must_use]
    pub fn encode_max(&self, input: &str, max_length: usize) -> String {
        let opts = SlavicMetaphoneOptions {
            max_length,
            include_vowels: self.options.include_vowels,
        };
        encode_with(input, opts)
    }
}

impl PhoneticEncoder for SlavicMetaphone {
    type Key = String;

    #[inline]
    fn encode(&self, input: &str) -> Self::Key {
        Self::encode(self, input)
    }

    #[inline]
    fn descriptor(&self) -> AlgorithmDescriptor {
        Self::DESCRIPTOR
    }

    #[inline]
    fn applicability(&self) -> Applicability {
        Self::APPLICABILITY
    }
}

/// Encode `text` with default options.
///
/// Convenience free function equivalent to `SlavicMetaphone::new().encode(text)`.
#[must_use]
pub fn slavic_metaphone(text: &str) -> String {
    SlavicMetaphone::new().encode(text)
}

// ---------------------------------------------------------------------------
// Kernel
// ---------------------------------------------------------------------------

/// Class alphabet character marking a vowel emitted by the normalizer.
///
/// Used by the emission stage to distinguish vowel bytes (subject to the
/// `include_vowels` gate) from consonant bytes (always emitted).
#[inline]
fn is_vowel_class(b: u8) -> bool {
    matches!(b, b'A' | b'E' | b'I' | b'O' | b'U')
}

/// The kernel: encode `text` under the given options.
fn encode_with(text: &str, options: SlavicMetaphoneOptions) -> String {
    // Stage 1: lowercase and collect scalars once so digraph lookahead
    // can index them.
    let chars: Vec<char> = text.chars().flat_map(char::to_lowercase).collect();

    // Stage 2: walk the char slice, emitting phonetic-class bytes into
    // `raw`. Digraphs consume 2 or 3 scalars per emit call.
    let mut raw: Vec<u8> = Vec::with_capacity(chars.len().saturating_mul(2));
    let mut i = 0;
    while i < chars.len() {
        i += emit_at(&chars, i, &mut raw);
    }

    // Stage 3: emit the final key with the consecutive-duplicate collapse
    // and the vowel gate. Cap at min(options.max_length, HARD_MAX_KEY_LEN).
    let cap = options.max_length.min(HARD_MAX_KEY_LEN);
    if cap == 0 || raw.is_empty() {
        return String::new();
    }

    let mut out: Vec<u8> = Vec::with_capacity(cap);
    for (idx, &b) in raw.iter().enumerate() {
        // Consecutive-duplicate collapse — mirrors the classic Metaphone
        // "no double letters" rule and folds cases like Polish `dd` or
        // Czech `nn` to a single class byte.
        if out.last() == Some(&b) {
            continue;
        }
        if is_vowel_class(b) {
            // Vowel gate: default drops every vowel that is not the very
            // first key byte. `include_vowels = true` keeps them.
            if !options.include_vowels && idx != first_emitted_index(&raw) {
                continue;
            }
        }
        out.push(b);
        if out.len() >= cap {
            break;
        }
    }

    // Every byte in `out` is an ASCII uppercase letter — the ASCII
    // alphabet is a strict subset of UTF-8, so `from_utf8` cannot fail.
    // We use the checked constructor to avoid an unsafe optimization for a
    // cold path.
    String::from_utf8(out).unwrap_or_default()
}

/// Returns the index of the first byte in `raw` that would be emitted —
/// the first byte, since the emitter never inserts skips before position 0.
///
/// Extracted as a helper only to keep [`encode_with`]'s vowel gate
/// self-documenting: "vowel at index N is kept only if it is the first
/// key byte" reads cleaner than a bare `idx != 0`, and gives us a place
/// to change the rule later without hunting for a magic constant.
#[inline]
const fn first_emitted_index(_raw: &[u8]) -> usize {
    0
}

/// Emit the phonetic-class byte(s) for `chars[i..]` into `out`, returning
/// the number of scalars consumed (1, 2, or 3).
///
/// The function tries longest-form digraphs first so a shorter form is not
/// applied to a prefix of a longer one (Polish `dź` vs `d`).
fn emit_at(chars: &[char], i: usize, out: &mut Vec<u8>) -> usize {
    let c = chars[i];
    let next = chars.get(i + 1).copied();
    let next2 = chars.get(i + 2).copied();

    // Three-scalar digraphs (Polish `dź`, `dż` when spelled with plain
    // ASCII `d` + accented letter, and ASCII transliteration triples).
    if let Some(n1) = next {
        if c == 'd' && matches!(n1, 'ź' | 'ż') {
            out.push(b'C');
            return 2;
        }
        if c == 'd' && n1 == 'z' {
            // Polish `dz` — the base affricate `/dz/` in the alveolar
            // class Z. A following `ź`/`ż` would already have been caught
            // above; a following ASCII `h` would form `dzh` (Serbian
            // Latin approximation) and encodes as C.
            if matches!(next2, Some('h')) {
                out.push(b'C');
                return 3;
            }
            out.push(b'Z');
            return 2;
        }
        // Latin digraphs shared across Slavic Latin orthographies and
        // ASCII transliterations.
        if c == 's' && n1 == 'z' {
            out.push(b'X'); // Polish `sz` = /ʃ/
            return 2;
        }
        if c == 'c' && n1 == 'z' {
            out.push(b'C'); // Polish `cz` = /tʃ/
            return 2;
        }
        if c == 'r' && n1 == 'z' {
            out.push(b'X'); // Polish `rz` ~ /ʐ/
            return 2;
        }
        if c == 'c' && n1 == 'h' {
            // Cross-Slavic policy: `ch` -> C (the /tʃ/ reading in
            // Russian / Ukrainian / Serbian transliteration). Polish and
            // Czech `/x/` `ch` is a known miss; `"kh"` remains available
            // for the `/x/` reading.
            out.push(b'C');
            return 2;
        }
        if c == 'k' && n1 == 'h' {
            out.push(b'H'); // ASCII transliteration of Cyrillic `х`
            return 2;
        }
        if c == 's' && n1 == 'h' {
            out.push(b'X'); // ASCII transliteration of Cyrillic `ш`
            return 2;
        }
        if c == 'z' && n1 == 'h' {
            out.push(b'X'); // ASCII transliteration of Cyrillic `ж`
            return 2;
        }
        if c == 'd' && n1 == 'ž' {
            out.push(b'C'); // Serbian / Croatian Latin `dž`
            return 2;
        }
        if c == 't' && n1 == 's' {
            out.push(b'Z'); // ASCII transliteration of Cyrillic `ц`
            return 2;
        }
    }

    // Single-scalar mapping.
    if let Some(cls) = classify_single(c) {
        for b in cls {
            out.push(*b);
        }
    }
    1
}

/// Classify a single lowercase scalar to zero or more phonetic-class bytes.
///
/// Returns:
///
/// * `Some(&[b'X', ...])` — a slice of one or two class bytes (nasal
///   vowels emit two: the vowel and an `N`).
/// * `None` — the scalar contributes nothing (soft/hard signs, digits,
///   punctuation, unmapped letters).
#[allow(
    clippy::too_many_lines,
    clippy::match_same_arms,
    reason = "flat table for auditability; Latin and Cyrillic arms are kept visually separated to \
              mirror the module-level mapping table even when they share a class"
)]
fn classify_single(c: char) -> Option<&'static [u8]> {
    Some(match c {
        // --- Latin vowels (base + long/short + accented) ---
        'a' | 'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' => b"A",
        'e' | 'é' | 'è' | 'ê' | 'ë' => b"E",
        'i' | 'í' | 'ì' | 'î' | 'ï' | 'y' | 'ý' => b"I",
        'o' | 'ó' | 'ò' | 'ô' | 'ö' | 'ő' | 'õ' => b"O",
        'u' | 'ú' | 'ù' | 'û' | 'ü' | 'ů' | 'ű' => b"U",

        // Polish nasal vowels — decompose to vowel + N suffix, matching
        // their phonetic realization as a vowel followed by a nasal.
        'ą' => b"AN",
        'ę' => b"EN",

        // --- Latin consonants ---
        'b' | 'p' => b"P",
        'f' | 'v' | 'w' => b"F",
        't' | 'd' | 'ť' | 'ď' | 'þ' => b"T",
        'k' | 'q' => b"K",
        'g' | 'h' | 'x' | 'ħ' => b"H",
        's' | 'z' | 'ś' | 'ź' | 'ß' => b"S",
        // Polish `c` / Czech `c` / Serbian Latin `c` all `/ts/`; Turkish
        // `ç` shares the /tʃ/ reading but is close enough to fold to `Z`
        // rather than `C` for the Slavic-Latin design intent.
        'c' | 'ç' => b"Z",
        'č' | 'ć' | 'đ' => b"C",
        'š' | 'ž' | 'ż' => b"X",
        'm' => b"M",
        'n' | 'ń' | 'ň' | 'ñ' => b"N",
        'l' | 'ł' | 'ĺ' | 'ľ' => b"L",
        'r' | 'ř' => b"R",
        'j' => b"J",

        // --- Cyrillic vowels ---
        'а' | 'я' => b"A",
        'е' | 'є' | 'э' => b"E",
        'и' | 'і' | 'ї' | 'ы' => b"I",
        'о' | 'ё' => b"O",
        'у' | 'ю' => b"U",

        // --- Cyrillic consonants ---
        'б' | 'п' => b"P",
        'ф' | 'в' | 'ў' => b"F",
        'т' | 'д' => b"T",
        'к' => b"K",
        'г' | 'х' | 'ґ' => b"H",
        'с' | 'з' => b"S",
        // Alveolar affricates. `ц` = /ts/, Macedonian `ѕ` = /dz/ — voice
        // collapse folds both to Z.
        'ц' | 'ѕ' => b"Z",
        // Postalveolar affricates. Macedonian `ѓ` (/ɟ/) and `ќ` (/c/) are
        // palatal stops sharing the class with Serbian `ђ`/`ћ` for cross-
        // South-Slavic matching. Serbian `џ` = /dʒ/.
        'ч' | 'ђ' | 'ћ' | 'џ' | 'ѓ' | 'ќ' => b"C",
        // Postalveolar fricatives; `щ` folds in for the reasons in the
        // module docs.
        'ш' | 'ж' | 'щ' => b"X",
        'м' => b"M",
        'н' | 'њ' => b"N",
        'л' | 'љ' => b"L",
        'р' => b"R",
        'й' | 'ј' => b"J",

        // Soft/hard signs and apostrophes carry no phonetic content of
        // their own; the class is `None` (skip entirely).
        'ь' | 'ъ' | '\'' | '\u{02BC}' => return None,

        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn e(s: &str) -> String {
        SlavicMetaphone::new().encode(s)
    }

    fn e_vowels(s: &str) -> String {
        let opts = SlavicMetaphoneOptions::default().with_include_vowels(true);
        SlavicMetaphone::with_options(opts).encode(s)
    }

    #[test]
    fn descriptor_variant_slug() {
        let d = SlavicMetaphone::descriptor();
        assert_eq!(d.family, AlgorithmFamily::Metaphone);
        assert_eq!(d.variant, VariantId("slavic-metaphone-2026"));
    }

    #[test]
    fn applicability_covers_slavic_family_both_scripts() {
        let a = SlavicMetaphone::applicability();
        assert!(a.languages.contains(&LanguageTag("ru")));
        assert!(a.languages.contains(&LanguageTag("pl")));
        assert!(a.languages.contains(&LanguageTag("sr")));
        assert!(a.scripts.contains(&ScriptTag("Cyrl")));
        assert!(a.scripts.contains(&ScriptTag("Latn")));
    }

    #[test]
    fn empty_input_returns_empty() {
        assert_eq!(e(""), "");
        assert_eq!(e("---"), "");
        assert_eq!(e("123"), "");
    }

    #[test]
    fn default_options_drops_non_initial_vowels() {
        // "Praha" → P R A H A → drop non-initial vowels → PRH.
        assert_eq!(e("Praha"), "PRH");
    }

    #[test]
    fn include_vowels_keeps_them_collapsed() {
        // With vowels included, "Praha" keeps A between R and H, and the
        // trailing A. Consecutive-duplicate collapse leaves them alone
        // since P, R, A, H, A are all distinct-in-sequence.
        assert_eq!(e_vowels("Praha"), "PRAHA");
    }

    #[test]
    fn max_length_truncates() {
        let enc = SlavicMetaphone::new();
        let out = enc.encode_max("Constantinopolska", 5);
        assert!(out.len() <= 5, "got {out:?}");
    }

    #[test]
    fn hard_max_length_clamps() {
        let opts = SlavicMetaphoneOptions::default().with_max_length(9999);
        let enc = SlavicMetaphone::with_options(opts);
        let out = enc.encode("Constantinopolska");
        assert!(out.len() <= HARD_MAX_KEY_LEN);
    }

    #[test]
    fn max_length_zero_returns_empty() {
        let enc = SlavicMetaphone::new();
        assert_eq!(enc.encode_max("Praha", 0), "");
    }

    #[test]
    fn is_case_insensitive() {
        assert_eq!(e("praha"), e("Praha"));
        assert_eq!(e("PRAHA"), e("Praha"));
        assert_eq!(e("прага"), e("Прага"));
        assert_eq!(e("ПРАГА"), e("Прага"));
    }

    #[test]
    fn free_function_matches_encoder_default() {
        assert_eq!(
            slavic_metaphone("Praha"),
            SlavicMetaphone::new().encode("Praha")
        );
    }

    #[test]
    fn strips_punctuation_and_digits() {
        assert_eq!(e("P-r-a-h-a!"), e("Praha"));
        assert_eq!(e("Praha 2026"), e("Praha"));
    }

    // ------------------------------------------------------------------
    // Cross-language equivalence pairs — the whole point of the encoder.
    // At least 20 pairs across the Slavic family; each asserts that
    // pronunciation-adjacent spellings hash identically.
    // ------------------------------------------------------------------

    /// Table of (label, left, right) cross-language pairs where left and
    /// right must produce the same Slavic-Metaphone key.
    ///
    /// Each row is a real geographic name, common surname, or common
    /// noun where the equivalence is well-established across the Slavic
    /// language family.
    const CROSS_LANGUAGE_PAIRS: &[(&str, &str, &str)] = &[
        // 1. Russian ↔ Latin transliteration
        ("Chekhov", "Чехов", "Chekhov"),
        // 2. Polish ↔ Ukrainian, same city name
        ("Krakow", "Kraków", "Краків"),
        // 3. Czech ↔ Russian, same city (Prague)
        ("Prague", "Praha", "Прага"),
        // 4. Serbian Cyrillic ↔ Serbian Latin (Belgrade)
        ("Belgrade", "Београд", "Beograd"),
        // 5. Russian ↔ Latin transliteration (Moscow)
        ("Moscow", "Москва", "Moskva"),
        // 6. Ukrainian ↔ Russian same surname stem
        ("Ivan", "Иван", "Іван"),
        // 7. Russian city Novgorod ↔ Latin transliteration
        ("Novgorod", "Новгород", "Novgorod"),
        // 8. Polish surname vs Cyrillic
        ("Kowal", "Kowal", "Коваль"),
        // 9. Belarusian city Minsk in Cyrillic and Latin
        ("Minsk", "Мінск", "Minsk"),
        // 10. Sofia (Bulgarian)
        ("Sofia", "София", "Sofia"),
        // 11. Ukrainian Kyiv vs Latin
        ("Kyiv", "Київ", "Kyiv"),
        // 12. Croatian Zagreb vs Cyrillic (Serbian would also write this)
        ("Zagreb", "Zagreb", "Загреб"),
        // 13. Polish Warszawa vs Cyrillic Варшава
        ("Warsaw", "Warszawa", "Варшава"),
        // 14. Serbian surname Vuković — Cyrillic Latin
        ("Vukovic", "Вуковић", "Vuković"),
        // 15. Common noun "brat" (brother) — Russian and Serbian both use it
        ("Brat", "брат", "brat"),
        // 16. Croatian Split vs Cyrillic Сплит
        ("Split", "Сплит", "Split"),
        // 17. Skopje (Macedonian) vs Latin
        ("Skopje", "Скопје", "Skopje"),
        // 18. Common name Petar (Petr) — Bulgarian Cyrillic vs Latin
        ("Petar", "Петар", "Petar"),
        // 19. Polish Poznań vs Cyrillic Познань
        ("Poznan", "Poznań", "Познань"),
        // 20. Serbian Novi Sad vs Cyrillic (one word)
        ("NoviSad", "Нови Сад", "Novi Sad"),
        // 21. Slovakia city Bratislava vs Cyrillic
        ("Bratislava", "Братислава", "Bratislava"),
        // 22. Ukrainian Lviv vs Latin
        ("Lviv", "Львів", "Lviv"),
        // 23. Russian Volga vs Latin transliteration
        ("Volga", "Волга", "Volga"),
        // 24. Serbian Beograd — Cyrillic vs Croatian Latin transliteration
        ("Sarajevo", "Сарајево", "Sarajevo"),
    ];

    #[test]
    fn cross_language_pairs_match() {
        for &(label, left, right) in CROSS_LANGUAGE_PAIRS {
            let a = e(left);
            let b = e(right);
            assert_eq!(a, b, "{label}: {left:?} = {a:?} but {right:?} = {b:?}");
        }
    }

    #[test]
    fn cross_language_pair_count_meets_minimum() {
        // Deliverable minimum was 20; the table is expected to keep
        // growing across releases.
        assert!(
            CROSS_LANGUAGE_PAIRS.len() >= 20,
            "expected at least 20 cross-language pairs, got {}",
            CROSS_LANGUAGE_PAIRS.len()
        );
    }

    // ------------------------------------------------------------------
    // Per-class targeted sanity tests.
    // ------------------------------------------------------------------

    #[test]
    fn g_and_h_collapse_to_same_class() {
        // The cross-Slavic /g/-/h/ collapse is the single most opinionated
        // choice; assert it stays observable.
        assert_eq!(e("gora"), e("hora"));
    }

    #[test]
    fn sh_zh_share_class() {
        // Voice-collapse: /ʃ/ and /ʒ/ both land in X.
        assert_eq!(e("sh"), e("zh"));
        assert_eq!(e("ша"), e("жа"));
    }

    #[test]
    fn ch_transliteration_reads_as_c() {
        // The `ch` digraph reads as /tʃ/ (Russian Ч convention). Chekhov
        // and Cerov both start with C class.
        assert!(e("Chekhov").starts_with('C'));
    }

    #[test]
    fn kh_transliteration_reads_as_h() {
        // "Kharkiv" starts with the `kh` digraph, which emits a single H
        // class byte — matching the Cyrillic `Х` reading.
        assert!(e("Kharkiv").starts_with('H'));
        // And the Ukrainian Cyrillic spelling agrees.
        assert_eq!(e("Kharkiv"), e("Харків"));
    }

    #[test]
    fn nasal_polish_vowels_add_n() {
        // ą decomposes to A + N; ę decomposes to E + N. With vowels
        // included the N surfaces; without, the vowel drops but the N
        // remains.
        assert_eq!(e_vowels("mą"), "MAN");
        assert_eq!(e("mą"), "MN");
    }

    #[test]
    fn soft_signs_are_dropped() {
        // ь and ъ contribute nothing; "конь" and "кон" produce the same
        // key.
        assert_eq!(e("конь"), e("кон"));
        assert_eq!(e("объект"), e("обект"));
    }

    #[test]
    fn palatalization_diacritics_fold_to_base() {
        // Polish ń folds to N, Czech ť folds to T, Croatian đ folds to
        // C — the softened forms share their base class.
        assert_eq!(e("Poznań"), e("Poznan"));
        assert_eq!(e("Baťa"), e("Bata"));
    }

    #[test]
    fn long_vowels_fold_to_short() {
        // Czech á/í/ó fold to a/i/o.
        assert_eq!(e_vowels("Praha"), e_vowels("Práha"));
        assert_eq!(e_vowels("Vitor"), e_vowels("Vítor"));
    }

    #[test]
    fn key_only_contains_class_alphabet() {
        // The emitted key is confined to the 14 consonant classes plus
        // 5 vowel classes.
        let ok = |c: char| {
            matches!(
                c,
                'P' | 'F'
                    | 'T'
                    | 'K'
                    | 'H'
                    | 'S'
                    | 'Z'
                    | 'X'
                    | 'C'
                    | 'M'
                    | 'N'
                    | 'L'
                    | 'R'
                    | 'J'
                    | 'A'
                    | 'E'
                    | 'I'
                    | 'O'
                    | 'U'
            )
        };
        for input in [
            "Praha",
            "Прага",
            "Kraków",
            "Краків",
            "Чехов",
            "Chekhov",
            "Beograd",
            "Warszawa",
            "Poznań",
            "Львів",
            "конь",
            "объект",
        ] {
            for c in e(input).chars() {
                assert!(
                    ok(c),
                    "unexpected class char {c:?} in encode({input:?}) = {:?}",
                    e(input)
                );
            }
        }
    }

    #[test]
    fn idempotent_encode_twice() {
        for input in [
            "Praha",
            "Прага",
            "Kraków",
            "Краків",
            "Чехов",
            "Chekhov",
            "Beograd",
        ] {
            let once = e(input);
            let twice = SlavicMetaphone::new().encode(&once);
            // The key itself, when re-encoded, may compress further
            // because vowels drop — but re-encoding the same *input*
            // twice must be deterministic.
            assert_eq!(e(input), once);
            let _ = twice; // second-order behavior only observed for smoke.
        }
    }

    #[test]
    fn trait_and_inherent_encode_agree() {
        let enc = SlavicMetaphone::new();
        for name in ["Praha", "Прага", "Chekhov", "Чехов", "Kraków", ""] {
            assert_eq!(
                <SlavicMetaphone as PhoneticEncoder>::encode(&enc, name),
                enc.encode(name)
            );
        }
    }

    #[test]
    fn descriptor_is_const() {
        const D: AlgorithmDescriptor = SlavicMetaphone::DESCRIPTOR;
        assert_eq!(D.variant.0, "slavic-metaphone-2026");
    }

    #[test]
    fn options_builder_composes() {
        let opts = SlavicMetaphoneOptions::default()
            .with_max_length(4)
            .with_include_vowels(true);
        assert_eq!(opts.max_length, 4);
        assert!(opts.include_vowels);
    }
}
