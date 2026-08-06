//! [`DoubleMetaphone`] — Lawrence Philips' 1999 Double Metaphone encoder,
//! selectable between a primary-only variant and a full two-key variant.
//!
//! # Two variants
//!
//! Double Metaphone can produce **up to two** phonetic keys per input, a
//! primary and an optional alternate, to capture regional-pronunciation
//! variance (e.g. `"Schmidt"` primary `"XMT"` and alternate `"SMT"`). This
//! crate exposes both shapes through [`DoubleMetaphoneVariant`]:
//!
//! * [`DoubleMetaphoneVariant::PrimaryOnly`] — the primary key only, under
//!   the variant slug [`VariantId("philips-1999-primary-only")`][vid]. This
//!   is a single-pass encoder; the [`DoubleMetaphoneKey::alternate`] field
//!   is always `None`. It preserves the crate's initial delivery behavior
//!   byte-for-byte.
//! * [`DoubleMetaphoneVariant::Full`] — both the primary key and (where
//!   warranted) an alternate key, under the variant slug
//!   `"philips-1999-full"`. More expensive (two passes) but reflects
//!   regional pronunciation variance.
//!
//! Construct an encoder with [`DoubleMetaphone::primary_only`] or
//! [`DoubleMetaphone::full`]. [`DoubleMetaphone::default`] returns the
//! primary-only variant so that the crate's `Default` behavior is unchanged
//! from the initial delivery.
//!
//! [phon]: https://github.com/tegmentum/stringcheese/blob/main/docs/design/phonetic-subsystem.md
//! [vid]: stringcheese_core::VariantId
//!
//! # Primary-key stability
//!
//! The **primary key of the full variant equals the primary key of the
//! primary-only variant, byte for byte, for every input.** The full
//! variant's contribution is a second pass that either produces an
//! alternate string different from the primary (returned as `Some`) or one
//! equal to the primary (returned as `None`). Consumers can therefore
//! upgrade `primary_only()` to `full()` without any change to the primary
//! matches they were already getting; they only pick up additional
//! regional-pronunciation matches through the alternate branch.
//!
//! # Rules — primary key
//!
//! The primary key follows the widely mirrored reference structure of
//! Apache Commons Codec's `DoubleMetaphone.java`:
//!
//! * **Silent starts.** Words beginning with `GN`, `KN`, `PN`, `WR`, or
//!   `PS` skip their first letter.
//! * **Initial X.** A word starting with `X` emits `S` (as in *Xavier*).
//! * **Vowels** (`A, E, I, O, U, Y`) emit `A` if they are the first character
//!   contributing to the key, and are otherwise silent.
//! * **`B`** → `P`; doubled `BB` collapses.
//! * **`C`** — the classical soft/hard C branch with digraph handling for
//!   `CH` (→ `X`), `CK` (→ `K`), and `CIA` (→ `X`).
//! * **`D`** → `T`; `DG` before `E/I/Y` → `J`; doubled `DD`/`DT` collapses.
//! * **`F`** → `F`; doubled `FF` collapses.
//! * **`G`** — soft G before `E/I/Y` → `J`; `GH` silent after a vowel,
//!   `K` otherwise; hard G → `K`.
//! * **`H`** — silent except when at the start of the key or between two
//!   vowels.
//! * **`J`** → `J`.
//! * **`K, L, M, N, P, R, T`** — each maps to itself, with doubled forms
//!   collapsing.
//! * **`PH`** → `F`.
//! * **`Q`** → `K`.
//! * **`SH`** → `X`; **`SCH`** → `X`; otherwise `S` → `S` (doubled collapses).
//! * **`TH`** → `0` (theta placeholder, spelled as ASCII `0` in the key)
//!   except in the *Thomas* / *Thames* family (`TH` followed by `OM`/`AM`),
//!   where it emits `T`.
//! * **`V`** → `F`.
//! * **`W`** — silent in the primary key.
//! * **`X`** → `KS` in the middle of a word; silent when at the end
//!   preceded by `AU` or `OU` (French endings). Initial `X` is handled by
//!   the silent-start rule above.
//! * **`Z`** → `S`.
//!
//! Result length is capped at four characters, matching Philips' original
//! specification.
//!
//! # Rules — alternate key (Full variant only)
//!
//! The alternate key is computed by a second pass over the same normalized
//! input. It mirrors the primary key at every rule except the following
//! divergence points:
//!
//! * **Initial `W` before a vowel** (Germanic): primary is silent, alternate
//!   emits `F` — the Germanic `V`-sound reading. `"Wagner"` primary `"AKNR"`,
//!   alternate `"FKNR"`.
//! * **`CH` at start followed by `IE`** (French): primary emits `X` (the
//!   English `ch`), alternate emits `J`. `"Chien"` primary `"XN"`, alternate
//!   `"JN"`.
//! * **`CH` at start followed by `O`, `A`, or `Y`** (Greek/Germanic
//!   patterns): primary emits `X`, alternate emits `K`. `"Chorus"` primary
//!   `"XRS"`, alternate `"KRS"`.
//! * **`SCH`** anywhere: primary emits `X`, alternate emits `S` (the
//!   Anglicized reading). `"Schmidt"` primary `"XMT"`, alternate `"SMT"`.
//! * **`TH`** in the theta context: primary emits `0` (theta), alternate
//!   emits `T`. `"Smith"` primary `"SM0"`, alternate `"SMT"`. `TH+OM/AM`
//!   still emits `T` in both branches (Thomas exception).
//! * **`-WIG` Germanic ending** (`W` at position *n-3* with `IG` at
//!   *n-2..n*): primary keeps `W` silent and emits `K` for the `G`;
//!   alternate emits `F` for the `W` and skips the `IG`. `"Barwig"` primary
//!   `"PRK"`, alternate `"PRF"`.
//! * **`-WICZ` Slavic ending**: alternate emits `F` for the `W` and `X`
//!   for the `CZ` (the *ts-sh* reading), while primary keeps `W` silent
//!   and encodes the `C, Z` per the standard rules.
//! * **Final `R` after `IE`** (French `-IER` endings, excluding `-MER`,
//!   `-MAR`): primary emits `R`, alternate skips it. `"Xavier"` primary
//!   `"SFR"`, alternate `"SF"`.
//!
//! Where the alternate pass produces a string equal to the primary, the
//! [`DoubleMetaphoneKey::alternate`] field is set to `None` — an alternate
//! is only meaningful when it differs.
//!
//! # Rules deferred
//!
//! Apache Commons Codec's `DoubleMetaphone.java` implements a broader set
//! of context-sensitive alternate-key rules than the ones listed above
//! (extensive Slavo-Germanic origin detection driving many local decisions,
//! `-EAU`/`-EAUX` French silent-endings, `SC` before `I/E/Y` diverging to
//! `SK`, `X` at start diverging to `SFR`, and a long tail of surname-family
//! exceptions). The rules we do implement are the ones a typical
//! English-language entity-resolution workload benefits from most; the
//! long tail is deferred to future work behind the same variant slug (a
//! patch-version bump of [`DoubleMetaphone::FULL_DESCRIPTOR`]).
//!
//! Because our full variant is pinned to preserve the primary-only variant
//! byte-for-byte, some rules that Apache Commons applies to the *primary*
//! branch (like `-IER` silent-R, or the `CH` Greek exceptions being *primary*
//! `K`) apply only to our *alternate* branch here — the roles are reversed
//! relative to the classical reference so that the primary key remains
//! stable. This is a deliberate trade: existing consumers upgrade without
//! observing any primary-key change, and the alternate captures the
//! regional reading.
//!
//! # Applicability
//!
//! Double Metaphone was designed for English but includes rules for
//! Germanic, Slavic, and Romance surnames often encountered in English-
//! language records. Its output on non-Latin-script input (Chinese in Han
//! characters, Arabic in Arabic script) is meaningless — the algorithm's
//! ASCII-letter alphabet is not what those languages present.
//!
//! # Non-ASCII input
//!
//! Non-ASCII characters and non-letters are stripped in the first step;
//! callers who want other behavior should transliterate before calling
//! `encode`.
//!
//! # References
//!
//! * Philips, L. (1990). "Hanging on the metaphone." *Computer Language*,
//!   7(12), 39-43. The original Metaphone paper.
//! * Philips, L. (2000). "The double metaphone search algorithm."
//!   *C/C++ Users Journal*, 18(6). — the Double Metaphone specification.
//! * Apache Software Foundation. *Apache Commons Codec* —
//!   `DoubleMetaphone.java`. URL:
//!   <https://commons.apache.org/proper/commons-codec/> — the widely
//!   mirrored reference implementation whose two-key branch structure this
//!   module's alternate-key pass follows.

use alloc::string::String;
use alloc::vec::Vec;
use stringcheese_core::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion, VariantId,
};

use crate::encoder::{Applicability, LanguageTag, PhoneticEncoder, ScriptTag};

/// The maximum length of a Double Metaphone key, per Philips' original
/// specification.
pub const MAX_KEY_LEN: usize = 4;

/// A Double Metaphone key: a primary code and an optional alternate.
///
/// The [`alternate`](Self::alternate) field is always `None` for the
/// [`DoubleMetaphoneVariant::PrimaryOnly`] variant, and either `None` (when
/// the alternate pass produced the same string as the primary) or
/// `Some(non_empty)` for the [`DoubleMetaphoneVariant::Full`] variant.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DoubleMetaphoneKey {
    /// The primary phonetic key. At most four characters.
    pub primary: String,
    /// The optional alternate phonetic key, for regional pronunciation
    /// variance. `None` when the encoder is primary-only or the alternate
    /// pass agreed with the primary.
    pub alternate: Option<String>,
}

impl DoubleMetaphoneKey {
    /// Constructs a key with only a primary code. Convenience for the
    /// primary-only variant.
    #[inline]
    #[must_use]
    pub const fn primary_only(primary: String) -> Self {
        Self {
            primary,
            alternate: None,
        }
    }
}

/// The Double Metaphone variant a [`DoubleMetaphone`] encoder is configured
/// for.
///
/// See the [module-level docs](self) for a discussion of the two variants
/// and their variant slugs.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum DoubleMetaphoneVariant {
    /// Primary key only. [`DoubleMetaphoneKey::alternate`] is always
    /// `None`. Variant slug `"philips-1999-primary-only"`. Cheaper (a
    /// single pass) and preserves the behavior of the initial delivery.
    #[default]
    PrimaryOnly,
    /// Both primary and alternate keys. Variant slug
    /// `"philips-1999-full"`. More expensive (two passes) but reflects
    /// regional pronunciation variance.
    Full,
}

/// The Double Metaphone encoder.
///
/// The encoder is configured for one of two [`DoubleMetaphoneVariant`]s at
/// construction time. Construct with [`DoubleMetaphone::primary_only`] or
/// [`DoubleMetaphone::full`]; [`DoubleMetaphone::default`] returns the
/// primary-only variant.
///
/// See the [module-level docs](self) for the algorithm's rules.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct DoubleMetaphone {
    variant: DoubleMetaphoneVariant,
}

impl DoubleMetaphone {
    /// The algorithm descriptor for the primary-only Double Metaphone
    /// variant.
    ///
    /// The `"primary-only"` suffix in the slug distinguishes this variant
    /// from the [`FULL_DESCRIPTOR`][Self::FULL_DESCRIPTOR] two-key variant,
    /// so golden cases cannot silently be validated against the wrong one.
    pub const PRIMARY_ONLY_DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor {
        family: AlgorithmFamily::DoubleMetaphone,
        variant: VariantId("philips-1999-primary-only"),
        version: DescriptorVersion::new(0, 1, 0),
        source: DefinitionSource::ReferenceImplementation {
            name: "Apache Commons Codec DoubleMetaphone (primary key path)",
        },
    };

    /// The algorithm descriptor for the full two-key Double Metaphone
    /// variant.
    ///
    /// The slug is `"philips-1999-full"`; the primary key remains
    /// byte-identical to
    /// [`PRIMARY_ONLY_DESCRIPTOR`][Self::PRIMARY_ONLY_DESCRIPTOR] but the
    /// encoder additionally computes an alternate key for the divergence
    /// points documented at the [module-level](self#rules--alternate-key-full-variant-only).
    pub const FULL_DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor {
        family: AlgorithmFamily::DoubleMetaphone,
        variant: VariantId("philips-1999-full"),
        version: DescriptorVersion::new(0, 1, 0),
        source: DefinitionSource::ReferenceImplementation {
            name: "Apache Commons Codec DoubleMetaphone (Philips 1999 with two-key branch)",
        },
    };

    /// Alias for [`PRIMARY_ONLY_DESCRIPTOR`][Self::PRIMARY_ONLY_DESCRIPTOR],
    /// retained for backwards compatibility with the initial delivery API
    /// where only the primary-only variant existed.
    pub const DESCRIPTOR: AlgorithmDescriptor = Self::PRIMARY_ONLY_DESCRIPTOR;

    /// The languages, scripts, and regions Double Metaphone was designed for.
    ///
    /// The algorithm's rule set is English-centric with adaptations for
    /// Germanic, Slavic, and Romance surnames found in English-language
    /// records. It is Latin-script only.
    pub const APPLICABILITY: Applicability = Applicability {
        languages: &[LanguageTag("en")],
        scripts: &[ScriptTag("Latn")],
        regions: &[],
        notes: "English-centric with Germanic / Slavic / Romance surname \
                adaptations; Latin script only.",
    };

    /// Constructs a primary-only encoder.
    ///
    /// Backwards compatible with the initial delivery — encoding produces
    /// exactly the same [`DoubleMetaphoneKey::primary`] as before, and
    /// [`DoubleMetaphoneKey::alternate`] is always `None`.
    #[inline]
    #[must_use]
    pub const fn primary_only() -> Self {
        Self {
            variant: DoubleMetaphoneVariant::PrimaryOnly,
        }
    }

    /// Constructs a full two-key encoder.
    ///
    /// Encoding produces the same [`DoubleMetaphoneKey::primary`] as the
    /// primary-only variant, plus an alternate key set to `Some` at the
    /// divergence points documented at the
    /// [module-level](self#rules--alternate-key-full-variant-only) (or `None`
    /// when the alternate agrees with the primary).
    #[inline]
    #[must_use]
    pub const fn full() -> Self {
        Self {
            variant: DoubleMetaphoneVariant::Full,
        }
    }

    /// Returns this encoder's [`DoubleMetaphoneVariant`].
    #[inline]
    #[must_use]
    pub const fn variant(&self) -> DoubleMetaphoneVariant {
        self.variant
    }

    /// Returns the algorithm descriptor for this encoder's variant.
    #[inline]
    #[must_use]
    pub const fn descriptor(&self) -> AlgorithmDescriptor {
        match self.variant {
            DoubleMetaphoneVariant::PrimaryOnly => Self::PRIMARY_ONLY_DESCRIPTOR,
            DoubleMetaphoneVariant::Full => Self::FULL_DESCRIPTOR,
        }
    }

    /// Returns the applicability. Both variants share the same
    /// applicability — the alternate branch does not add new languages or
    /// scripts, only regional-pronunciation coverage inside the same
    /// English/Latin scope.
    #[inline]
    #[must_use]
    pub const fn applicability(&self) -> Applicability {
        Self::APPLICABILITY
    }

    /// Encodes `input` to a [`DoubleMetaphoneKey`].
    ///
    /// For the [`DoubleMetaphoneVariant::PrimaryOnly`] variant the
    /// [`DoubleMetaphoneKey::alternate`] field is always `None`. For the
    /// [`DoubleMetaphoneVariant::Full`] variant, the primary key matches
    /// the primary-only variant byte-for-byte, and the alternate is `Some`
    /// only where the alternate pass diverged from the primary.
    #[must_use]
    pub fn encode(&self, input: &str) -> DoubleMetaphoneKey {
        match self.variant {
            DoubleMetaphoneVariant::PrimaryOnly => double_metaphone_encode(input),
            DoubleMetaphoneVariant::Full => double_metaphone_encode_full(input),
        }
    }
}

impl PhoneticEncoder for DoubleMetaphone {
    type Key = DoubleMetaphoneKey;

    #[inline]
    fn encode(&self, input: &str) -> Self::Key {
        Self::encode(self, input)
    }

    #[inline]
    fn descriptor(&self) -> AlgorithmDescriptor {
        Self::descriptor(self)
    }

    #[inline]
    fn applicability(&self) -> Applicability {
        Self::applicability(self)
    }
}

#[inline]
fn is_vowel(b: u8) -> bool {
    matches!(b, b'A' | b'E' | b'I' | b'O' | b'U' | b'Y')
}

/// Returns the byte at `src[i]`, or `0` if out of bounds. `0` is convenient
/// because it never matches any ASCII uppercase letter.
#[inline]
fn at(src: &[u8], i: usize) -> u8 {
    src.get(i).copied().unwrap_or(0)
}

/// Returns true if `src[i..]` starts with `pat`.
#[inline]
fn matches_at(src: &[u8], i: usize, pat: &[u8]) -> bool {
    src.len() >= i + pat.len() && &src[i..i + pat.len()] == pat
}

/// Appends `c` to `out` if the output has room; returns `true` if the output
/// is now at capacity.
#[inline]
fn push_if_room(out: &mut String, c: char) -> bool {
    if out.len() < MAX_KEY_LEN {
        out.push(c);
    }
    out.len() >= MAX_KEY_LEN
}

/// Normalizes `input` to uppercase ASCII letters. Both passes start from
/// this same normalized form.
#[inline]
fn normalize(input: &str) -> Vec<u8> {
    input
        .bytes()
        .filter(u8::is_ascii_alphabetic)
        .map(|b| b.to_ascii_uppercase())
        .collect()
}

/// The kernel: encode `input` to a primary-only Double Metaphone key.
///
/// This function is unchanged from the initial delivery and is called
/// directly by [`DoubleMetaphoneVariant::PrimaryOnly`]. It is also the
/// authoritative source of the primary key for
/// [`DoubleMetaphoneVariant::Full`] — the full variant does not touch
/// this function, ensuring that `full().encode(x).primary ==
/// primary_only().encode(x).primary` for every `x`.
#[allow(
    clippy::too_many_lines,
    reason = "The algorithm's rules are best expressed as one flat match; \
              breaking it up would obscure the letter-by-letter structure."
)]
fn double_metaphone_encode(input: &str) -> DoubleMetaphoneKey {
    let src = normalize(input);

    if src.is_empty() {
        return DoubleMetaphoneKey::primary_only(String::new());
    }

    let mut out = String::with_capacity(MAX_KEY_LEN);

    // Silent-start prefixes: skip the first letter.
    let mut i = 0usize;
    if src.len() >= 2 && matches!(&src[0..2], b"GN" | b"KN" | b"PN" | b"WR" | b"PS") {
        i = 1;
    }

    // Initial X → S (as in "Xavier"). Handled before the main loop because
    // subsequent X's have different rules.
    if src[0] == b'X' {
        out.push('S');
        i = 1;
    }

    while i < src.len() && out.len() < MAX_KEY_LEN {
        let c = src[i];
        match c {
            b'A' | b'E' | b'I' | b'O' | b'U' | b'Y' => {
                // Vowels contribute only when they are the first character
                // committed to the key; internal vowels are silent.
                if out.is_empty() {
                    out.push('A');
                }
                i += 1;
            }
            b'B' => {
                if push_if_room(&mut out, 'P') {
                    break;
                }
                i += if at(&src, i + 1) == b'B' { 2 } else { 1 };
            }
            b'C' => {
                if matches_at(&src, i, b"CH") {
                    if push_if_room(&mut out, 'X') {
                        break;
                    }
                    i += 2;
                } else if matches_at(&src, i, b"CK") {
                    if push_if_room(&mut out, 'K') {
                        break;
                    }
                    i += 2;
                } else if matches_at(&src, i, b"CIA") {
                    if push_if_room(&mut out, 'X') {
                        break;
                    }
                    i += 3;
                } else if matches!(at(&src, i + 1), b'E' | b'I' | b'Y') {
                    // Soft C
                    if push_if_room(&mut out, 'S') {
                        break;
                    }
                    i += 1;
                } else {
                    if push_if_room(&mut out, 'K') {
                        break;
                    }
                    i += if at(&src, i + 1) == b'C' { 2 } else { 1 };
                }
            }
            b'D' => {
                if at(&src, i + 1) == b'G' && matches!(at(&src, i + 2), b'E' | b'I' | b'Y') {
                    if push_if_room(&mut out, 'J') {
                        break;
                    }
                    i += 3;
                } else {
                    if push_if_room(&mut out, 'T') {
                        break;
                    }
                    i += if matches!(at(&src, i + 1), b'D' | b'T') {
                        2
                    } else {
                        1
                    };
                }
            }
            b'F' => {
                if push_if_room(&mut out, 'F') {
                    break;
                }
                i += if at(&src, i + 1) == b'F' { 2 } else { 1 };
            }
            b'G' => {
                if at(&src, i + 1) == b'H' {
                    // GH silent after a vowel (as in "bright", "though")
                    // otherwise emits K (as in "ghost" — start of word)
                    if i > 0 && is_vowel(src[i - 1]) {
                        i += 2;
                    } else {
                        if push_if_room(&mut out, 'K') {
                            break;
                        }
                        i += 2;
                    }
                } else if matches!(at(&src, i + 1), b'E' | b'I' | b'Y') {
                    // Soft G
                    if push_if_room(&mut out, 'J') {
                        break;
                    }
                    i += 1;
                } else {
                    if push_if_room(&mut out, 'K') {
                        break;
                    }
                    i += if at(&src, i + 1) == b'G' { 2 } else { 1 };
                }
            }
            b'H' => {
                // H emits only at start of key or between two vowels.
                let prev_vowel = i > 0 && is_vowel(src[i - 1]);
                let next_vowel = is_vowel(at(&src, i + 1));
                let should_emit = out.is_empty() || (prev_vowel && next_vowel);
                if should_emit && push_if_room(&mut out, 'H') {
                    break;
                }
                i += 1;
            }
            b'J' => {
                if push_if_room(&mut out, 'J') {
                    break;
                }
                i += 1;
            }
            b'K' => {
                if push_if_room(&mut out, 'K') {
                    break;
                }
                i += if at(&src, i + 1) == b'K' { 2 } else { 1 };
            }
            b'L' => {
                if push_if_room(&mut out, 'L') {
                    break;
                }
                i += if at(&src, i + 1) == b'L' { 2 } else { 1 };
            }
            b'M' => {
                if push_if_room(&mut out, 'M') {
                    break;
                }
                i += if at(&src, i + 1) == b'M' { 2 } else { 1 };
            }
            b'N' => {
                if push_if_room(&mut out, 'N') {
                    break;
                }
                i += if at(&src, i + 1) == b'N' { 2 } else { 1 };
            }
            b'P' => {
                if at(&src, i + 1) == b'H' {
                    if push_if_room(&mut out, 'F') {
                        break;
                    }
                    i += 2;
                } else {
                    if push_if_room(&mut out, 'P') {
                        break;
                    }
                    i += if at(&src, i + 1) == b'P' { 2 } else { 1 };
                }
            }
            b'Q' => {
                if push_if_room(&mut out, 'K') {
                    break;
                }
                i += 1;
            }
            b'R' => {
                if push_if_room(&mut out, 'R') {
                    break;
                }
                i += if at(&src, i + 1) == b'R' { 2 } else { 1 };
            }
            b'S' => {
                if matches_at(&src, i, b"SCH") {
                    if push_if_room(&mut out, 'X') {
                        break;
                    }
                    i += 3;
                } else if at(&src, i + 1) == b'H' {
                    if push_if_room(&mut out, 'X') {
                        break;
                    }
                    i += 2;
                } else {
                    if push_if_room(&mut out, 'S') {
                        break;
                    }
                    i += if at(&src, i + 1) == b'S' { 2 } else { 1 };
                }
            }
            b'T' => {
                if at(&src, i + 1) == b'H' {
                    // Thomas / Thames family: TH + OM/AM → T (not theta).
                    if matches!(&src[i + 2..src.len().min(i + 4)], b"OM" | b"AM") {
                        if push_if_room(&mut out, 'T') {
                            break;
                        }
                    } else if push_if_room(&mut out, '0') {
                        break;
                    }
                    i += 2;
                } else {
                    if push_if_room(&mut out, 'T') {
                        break;
                    }
                    i += if matches!(at(&src, i + 1), b'T' | b'D') {
                        2
                    } else {
                        1
                    };
                }
            }
            b'V' => {
                if push_if_room(&mut out, 'F') {
                    break;
                }
                i += if at(&src, i + 1) == b'V' { 2 } else { 1 };
            }
            b'W' => {
                // Primary-only variant treats W as silent; the alternate key
                // (Full variant) may emit F for Germanic origins — see
                // `double_metaphone_alternate_encode`.
                i += 1;
            }
            b'X' => {
                // Silent at end of word after AU/OU (French endings like
                // "PEUX", "FAUX"); KS elsewhere.
                let at_end = i == src.len() - 1;
                let after_ou_au = i >= 2 && matches!(&src[i - 2..i], b"OU" | b"AU");
                if at_end && after_ou_au {
                    // silent
                } else {
                    if push_if_room(&mut out, 'K') {
                        break;
                    }
                    if push_if_room(&mut out, 'S') {
                        break;
                    }
                }
                i += 1;
            }
            b'Z' => {
                if push_if_room(&mut out, 'S') {
                    break;
                }
                i += if at(&src, i + 1) == b'Z' { 2 } else { 1 };
            }
            _ => {
                i += 1;
            }
        }
    }

    DoubleMetaphoneKey::primary_only(out)
}

/// The Full variant: runs the primary-only pass unchanged, then a separate
/// alternate pass, and reports the alternate only when it differs.
///
/// The primary key is authoritatively [`double_metaphone_encode`]; this
/// function *never* mutates the primary. That is the crate's central
/// backwards-compatibility guarantee.
fn double_metaphone_encode_full(input: &str) -> DoubleMetaphoneKey {
    let primary = double_metaphone_encode(input).primary;
    let alternate_str = double_metaphone_alternate_encode(input);
    let alternate = if alternate_str.is_empty() || alternate_str == primary {
        None
    } else {
        Some(alternate_str)
    };
    DoubleMetaphoneKey { primary, alternate }
}

/// The kernel of the alternate pass. Mirrors [`double_metaphone_encode`]
/// at every rule *except* the divergence points documented on the module.
///
/// The output is a raw string; the caller decides whether it is meaningful
/// (differs from the primary) or should be reported as `None`.
#[allow(
    clippy::too_many_lines,
    reason = "The alternate pass mirrors the primary pass letter-for-letter; \
              splitting it up would obscure the divergence points."
)]
fn double_metaphone_alternate_encode(input: &str) -> String {
    let src = normalize(input);

    if src.is_empty() {
        return String::new();
    }

    let mut out = String::with_capacity(MAX_KEY_LEN);
    let mut i = 0usize;

    // Silent-start prefixes: skip the first letter. Same as primary.
    if src.len() >= 2 && matches!(&src[0..2], b"GN" | b"KN" | b"PN" | b"WR" | b"PS") {
        i = 1;
    }

    // Initial X → S. Same as primary.
    if src[0] == b'X' {
        out.push('S');
        i = 1;
    }

    // Initial W before a vowel: Germanic V-sound reading. Emit F where the
    // primary leaves W silent. (Wagner → "FKNR" alternate vs "AKNR" primary.)
    if src[0] == b'W' && src.len() >= 2 && is_vowel(src[1]) {
        out.push('F');
        i = 1;
    }

    while i < src.len() && out.len() < MAX_KEY_LEN {
        let c = src[i];
        match c {
            b'A' | b'E' | b'I' | b'O' | b'U' | b'Y' => {
                if out.is_empty() {
                    out.push('A');
                }
                i += 1;
            }
            b'B' => {
                if push_if_room(&mut out, 'P') {
                    break;
                }
                i += if at(&src, i + 1) == b'B' { 2 } else { 1 };
            }
            b'C' => {
                if matches_at(&src, i, b"CH") {
                    // Divergence: at start-of-word CH may emit K (Greek /
                    // Germanic) or J (French) instead of the primary's X.
                    let alt_ch = alternate_ch_at_start(&src, i);
                    if push_if_room(&mut out, alt_ch) {
                        break;
                    }
                    i += 2;
                } else if matches_at(&src, i, b"CK") {
                    if push_if_room(&mut out, 'K') {
                        break;
                    }
                    i += 2;
                } else if matches_at(&src, i, b"CIA") {
                    if push_if_room(&mut out, 'X') {
                        break;
                    }
                    i += 3;
                } else if matches!(at(&src, i + 1), b'E' | b'I' | b'Y') {
                    if push_if_room(&mut out, 'S') {
                        break;
                    }
                    i += 1;
                } else {
                    if push_if_room(&mut out, 'K') {
                        break;
                    }
                    i += if at(&src, i + 1) == b'C' { 2 } else { 1 };
                }
            }
            b'D' => {
                if at(&src, i + 1) == b'G' && matches!(at(&src, i + 2), b'E' | b'I' | b'Y') {
                    if push_if_room(&mut out, 'J') {
                        break;
                    }
                    i += 3;
                } else {
                    if push_if_room(&mut out, 'T') {
                        break;
                    }
                    i += if matches!(at(&src, i + 1), b'D' | b'T') {
                        2
                    } else {
                        1
                    };
                }
            }
            b'F' => {
                if push_if_room(&mut out, 'F') {
                    break;
                }
                i += if at(&src, i + 1) == b'F' { 2 } else { 1 };
            }
            b'G' => {
                if at(&src, i + 1) == b'H' {
                    if i > 0 && is_vowel(src[i - 1]) {
                        i += 2;
                    } else {
                        if push_if_room(&mut out, 'K') {
                            break;
                        }
                        i += 2;
                    }
                } else if matches!(at(&src, i + 1), b'E' | b'I' | b'Y') {
                    if push_if_room(&mut out, 'J') {
                        break;
                    }
                    i += 1;
                } else {
                    if push_if_room(&mut out, 'K') {
                        break;
                    }
                    i += if at(&src, i + 1) == b'G' { 2 } else { 1 };
                }
            }
            b'H' => {
                let prev_vowel = i > 0 && is_vowel(src[i - 1]);
                let next_vowel = is_vowel(at(&src, i + 1));
                let should_emit = out.is_empty() || (prev_vowel && next_vowel);
                if should_emit && push_if_room(&mut out, 'H') {
                    break;
                }
                i += 1;
            }
            b'J' => {
                if push_if_room(&mut out, 'J') {
                    break;
                }
                i += 1;
            }
            b'K' => {
                if push_if_room(&mut out, 'K') {
                    break;
                }
                i += if at(&src, i + 1) == b'K' { 2 } else { 1 };
            }
            b'L' => {
                if push_if_room(&mut out, 'L') {
                    break;
                }
                i += if at(&src, i + 1) == b'L' { 2 } else { 1 };
            }
            b'M' => {
                if push_if_room(&mut out, 'M') {
                    break;
                }
                i += if at(&src, i + 1) == b'M' { 2 } else { 1 };
            }
            b'N' => {
                if push_if_room(&mut out, 'N') {
                    break;
                }
                i += if at(&src, i + 1) == b'N' { 2 } else { 1 };
            }
            b'P' => {
                if at(&src, i + 1) == b'H' {
                    if push_if_room(&mut out, 'F') {
                        break;
                    }
                    i += 2;
                } else {
                    if push_if_room(&mut out, 'P') {
                        break;
                    }
                    i += if at(&src, i + 1) == b'P' { 2 } else { 1 };
                }
            }
            b'Q' => {
                if push_if_room(&mut out, 'K') {
                    break;
                }
                i += 1;
            }
            b'R' => {
                // Divergence: final R after IE (French -IER endings, but
                // not -MER / -MAR) is silent in the alternate branch.
                let at_end = i == src.len() - 1;
                let silent_r = at_end && is_ier_ending(&src, i);
                if !silent_r && push_if_room(&mut out, 'R') {
                    break;
                }
                i += if at(&src, i + 1) == b'R' { 2 } else { 1 };
            }
            b'S' => {
                if matches_at(&src, i, b"SCH") {
                    // Divergence: SCH → S in the alternate (Anglicized
                    // reading) vs X in the primary.
                    if push_if_room(&mut out, 'S') {
                        break;
                    }
                    i += 3;
                } else if at(&src, i + 1) == b'H' {
                    if push_if_room(&mut out, 'X') {
                        break;
                    }
                    i += 2;
                } else {
                    if push_if_room(&mut out, 'S') {
                        break;
                    }
                    i += if at(&src, i + 1) == b'S' { 2 } else { 1 };
                }
            }
            b'T' => {
                if at(&src, i + 1) == b'H' {
                    // TH + OM/AM: Thomas / Thames exception applies to both
                    // branches. Otherwise TH → T in the alternate (vs '0'
                    // theta in the primary) — the classical Double Metaphone
                    // primary/alternate split for the theta phoneme.
                    if matches!(&src[i + 2..src.len().min(i + 4)], b"OM" | b"AM") {
                        if push_if_room(&mut out, 'T') {
                            break;
                        }
                    } else if push_if_room(&mut out, 'T') {
                        break;
                    }
                    i += 2;
                } else {
                    if push_if_room(&mut out, 'T') {
                        break;
                    }
                    i += if matches!(at(&src, i + 1), b'T' | b'D') {
                        2
                    } else {
                        1
                    };
                }
            }
            b'V' => {
                if push_if_room(&mut out, 'F') {
                    break;
                }
                i += if at(&src, i + 1) == b'V' { 2 } else { 1 };
            }
            b'W' => {
                // Divergence family for internal W:
                //
                // * -WIG Germanic ending (n-3..n): alternate emits F and
                //   consumes the IG (primary keeps W silent and emits K for
                //   the G). Barwig → "PRF" alternate vs "PRK" primary.
                // * -WICZ Slavic ending (n-4..n): alternate emits F for the
                //   W and X for the CZ.
                // * Otherwise, W is silent (same as primary) unless the
                //   next character is a vowel — in that case the primary is
                //   still silent (its rule) but the alternate has already
                //   handled the start-of-word Germanic reading above, so
                //   for internal W-before-vowel we mirror the primary
                //   (silent) to avoid spurious alternates.
                if matches_at(&src, i, b"WIG") && i + 3 == src.len() {
                    if push_if_room(&mut out, 'F') {
                        break;
                    }
                    i += 3;
                    continue;
                }
                if matches_at(&src, i, b"WICZ") && i + 4 == src.len() {
                    if push_if_room(&mut out, 'F') {
                        break;
                    }
                    if push_if_room(&mut out, 'X') {
                        break;
                    }
                    i += 4;
                    continue;
                }
                // Otherwise silent, as in the primary.
                i += 1;
            }
            b'X' => {
                let at_end = i == src.len() - 1;
                let after_ou_au = i >= 2 && matches!(&src[i - 2..i], b"OU" | b"AU");
                if at_end && after_ou_au {
                    // silent
                } else {
                    if push_if_room(&mut out, 'K') {
                        break;
                    }
                    if push_if_room(&mut out, 'S') {
                        break;
                    }
                }
                i += 1;
            }
            b'Z' => {
                if push_if_room(&mut out, 'S') {
                    break;
                }
                i += if at(&src, i + 1) == b'Z' { 2 } else { 1 };
            }
            _ => {
                i += 1;
            }
        }
    }

    out
}

/// Alternate-branch selector for CH at start of word.
///
/// * CH at position 0 (with no silent-start offset) followed by `IE`:
///   French reading, alternate `J` (e.g. *Chien*).
/// * CH at position 0 followed by `O`, `A`, or `Y`: Greek / Germanic
///   reading, alternate `K` (e.g. *Chorus*, *Chaos*, *Chymist*).
/// * Otherwise: mirror the primary's `X`.
fn alternate_ch_at_start(src: &[u8], i: usize) -> char {
    if i != 0 {
        return 'X';
    }
    // Look at src[i+2] — the character after CH.
    let next = at(src, i + 2);
    let after_next = at(src, i + 3);
    if next == b'I' && after_next == b'E' {
        return 'J';
    }
    if matches!(next, b'O' | b'A' | b'Y') {
        return 'K';
    }
    'X'
}

/// True if `src[i]` is the final `R` of a `...IER` ending that is not
/// `-MER` or `-MAR` (which Apache Commons excludes; the endings are so
/// common in English that treating the R as silent would inflate false
/// positives).
fn is_ier_ending(src: &[u8], i: usize) -> bool {
    if i < 2 {
        return false;
    }
    if &src[i - 2..i] != b"IE" {
        return false;
    }
    if i >= 4 && matches!(&src[i - 4..i - 2], b"ME" | b"MA") {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_core::{AlgorithmFamily, VariantId};

    /// Convenience wrapper for the primary-only encoder's primary key.
    fn primary(name: &str) -> String {
        DoubleMetaphone::primary_only().encode(name).primary
    }

    /// Convenience wrapper for the full encoder.
    fn full_key(name: &str) -> DoubleMetaphoneKey {
        DoubleMetaphone::full().encode(name)
    }

    #[test]
    fn primary_only_descriptor_matches_family_and_variant() {
        let d = DoubleMetaphone::primary_only().descriptor();
        assert_eq!(d.family, AlgorithmFamily::DoubleMetaphone);
        assert_eq!(d.variant, VariantId("philips-1999-primary-only"));
    }

    #[test]
    fn full_descriptor_matches_family_and_variant() {
        let d = DoubleMetaphone::full().descriptor();
        assert_eq!(d.family, AlgorithmFamily::DoubleMetaphone);
        assert_eq!(d.variant, VariantId("philips-1999-full"));
    }

    #[test]
    fn descriptor_const_alias_matches_primary_only() {
        assert_eq!(
            DoubleMetaphone::DESCRIPTOR,
            DoubleMetaphone::PRIMARY_ONLY_DESCRIPTOR
        );
    }

    #[test]
    fn default_constructs_primary_only() {
        let d = DoubleMetaphone::default();
        assert_eq!(d.variant(), DoubleMetaphoneVariant::PrimaryOnly);
    }

    #[test]
    fn applicability_is_english_latin() {
        let a = DoubleMetaphone::primary_only().applicability();
        assert_eq!(a.languages, &[LanguageTag("en")]);
        assert_eq!(a.scripts, &[ScriptTag("Latn")]);
        // Both variants share applicability.
        assert_eq!(DoubleMetaphone::full().applicability(), a);
    }

    #[test]
    fn primary_only_alternate_is_always_none() {
        for name in ["Schmidt", "Xavier", "Wagner", "Thompson", "Smith"] {
            let key = DoubleMetaphone::primary_only().encode(name);
            assert_eq!(
                key.alternate, None,
                "{name:?} produced a non-None alternate under primary-only variant"
            );
        }
    }

    #[test]
    fn xavier_starts_with_s() {
        // Initial X → S; A skipped as internal vowel; V → F; I skipped;
        // E skipped; R → R. → "SFR".
        assert_eq!(primary("Xavier"), "SFR");
    }

    #[test]
    fn wagner_starts_with_a() {
        // W silent; A becomes first-committed-letter → A; G → K; N → N;
        // E skipped; R → R. → "AKNR".
        assert_eq!(primary("Wagner"), "AKNR");
    }

    #[test]
    fn schmidt_uses_sch_x() {
        // SCH → X; M → M; I skipped; D → T. → "XMT".
        assert_eq!(primary("Schmidt"), "XMT");
    }

    #[test]
    fn thomas_uses_th_t_exception() {
        // TH+OM → T (Thomas exception); O skipped; M → M; A skipped; S → S.
        //   → "TMS".
        assert_eq!(primary("Thomas"), "TMS");
    }

    #[test]
    fn thompson_uses_th_t_exception() {
        // TH+OM → T; O skipped; M → M; P → P; S → S (truncated at 4).
        //   → "TMPS".
        assert_eq!(primary("Thompson"), "TMPS");
    }

    #[test]
    fn smith_uses_th_theta() {
        // S → S; M → M; I skipped; TH → 0 (theta, not TH+OM/AM). → "SM0".
        assert_eq!(primary("Smith"), "SM0");
    }

    #[test]
    fn knight_silent_kn_start() {
        // KN silent start → K skipped; N → N; I skipped; GH silent after
        //   vowel (I); T → T. → "NT".
        assert_eq!(primary("Knight"), "NT");
    }

    #[test]
    fn gnome_silent_gn_start() {
        // GN silent start → G skipped; N → N; O skipped; M → M; E skipped.
        //   → "NM".
        assert_eq!(primary("Gnome"), "NM");
    }

    #[test]
    fn phillips_ph_becomes_f() {
        // PH → F; I skipped; L → L; L (double collapsed); I skipped;
        //   P → P; S → S (but truncated at 4). → "FLPS".
        assert_eq!(primary("Phillips"), "FLPS");
    }

    #[test]
    fn empty_and_junk_input_return_empty_primary() {
        assert_eq!(primary(""), "");
        assert_eq!(primary("1234"), "");
        assert_eq!(primary("---"), "");
    }

    #[test]
    fn is_case_insensitive() {
        assert_eq!(primary("smith"), primary("Smith"));
        assert_eq!(primary("SMITH"), primary("Smith"));
    }

    #[test]
    fn primary_length_bounded_by_four() {
        for name in [
            "A",
            "AB",
            "Constantinople",
            "Antidisestablishmentarianism",
            "Bratislavsky",
        ] {
            assert!(
                primary(name).len() <= MAX_KEY_LEN,
                "primary({name:?}) exceeded {MAX_KEY_LEN}"
            );
        }
    }

    #[test]
    fn descriptor_is_const() {
        const D: AlgorithmDescriptor = DoubleMetaphone::PRIMARY_ONLY_DESCRIPTOR;
        const F: AlgorithmDescriptor = DoubleMetaphone::FULL_DESCRIPTOR;
        assert_eq!(D.variant.0, "philips-1999-primary-only");
        assert_eq!(F.variant.0, "philips-1999-full");
    }

    #[test]
    fn trait_and_inherent_encode_agree() {
        let enc = DoubleMetaphone::primary_only();
        for name in ["Schmidt", "Xavier", "Wagner", ""] {
            assert_eq!(
                <DoubleMetaphone as PhoneticEncoder>::encode(&enc, name),
                enc.encode(name)
            );
        }
    }

    // -- Full variant --------------------------------------------------------

    #[test]
    fn full_wagner_germanic_w_becomes_f() {
        let k = full_key("Wagner");
        assert_eq!(k.primary, "AKNR");
        assert_eq!(k.alternate.as_deref(), Some("FKNR"));
    }

    #[test]
    fn full_xavier_ier_silent_r_in_alternate() {
        let k = full_key("Xavier");
        assert_eq!(k.primary, "SFR");
        assert_eq!(k.alternate.as_deref(), Some("SF"));
    }

    #[test]
    fn full_schmidt_sch_alternate_is_s() {
        let k = full_key("Schmidt");
        assert_eq!(k.primary, "XMT");
        assert_eq!(k.alternate.as_deref(), Some("SMT"));
    }

    #[test]
    fn full_chorus_ch_greek_alternate_is_k() {
        let k = full_key("Chorus");
        assert_eq!(k.primary, "XRS");
        assert_eq!(k.alternate.as_deref(), Some("KRS"));
    }

    #[test]
    fn full_chien_ch_french_alternate_is_j() {
        let k = full_key("Chien");
        assert_eq!(k.primary, "XN");
        assert_eq!(k.alternate.as_deref(), Some("JN"));
    }

    #[test]
    fn full_barwig_wig_ending_alternate_is_prf() {
        let k = full_key("Barwig");
        assert_eq!(k.primary, "PRK");
        assert_eq!(k.alternate.as_deref(), Some("PRF"));
    }

    #[test]
    fn full_smith_theta_alternate_is_t() {
        let k = full_key("Smith");
        assert_eq!(k.primary, "SM0");
        assert_eq!(k.alternate.as_deref(), Some("SMT"));
    }

    #[test]
    fn full_thompson_no_alternate_divergence() {
        // TH+OM exception applies in both branches; no other divergence.
        let k = full_key("Thompson");
        assert_eq!(k.primary, "TMPS");
        assert_eq!(k.alternate, None);
    }

    #[test]
    fn full_cachao_ch_mid_word_no_divergence() {
        // Internal CH stays X in both branches.
        let k = full_key("Cachao");
        assert_eq!(k.primary, "KX");
        assert_eq!(k.alternate, None);
    }

    #[test]
    fn full_alternate_never_exceeds_max_key_len() {
        for name in [
            "Xavier",
            "Wagner",
            "Schmidt",
            "Chorus",
            "Chien",
            "Barwig",
            "Antidisestablishmentarianism",
            "Constantinople",
        ] {
            let k = full_key(name);
            if let Some(alt) = k.alternate {
                assert!(
                    alt.len() <= MAX_KEY_LEN,
                    "alternate({name:?}) = {alt:?} exceeded {MAX_KEY_LEN}"
                );
            }
        }
    }

    #[test]
    fn full_primary_equals_primary_only_primary() {
        // Sanity across the golden-case set — the property test covers this
        // exhaustively, but a directed check reads better in a failure log.
        for name in [
            "Xavier", "Wagner", "Schmidt", "Thomas", "Thompson", "Smith", "Knight", "Gnome",
            "Phillips", "Chorus", "Chien", "Barwig", "Cachao", "",
        ] {
            let po = DoubleMetaphone::primary_only().encode(name).primary;
            let f = DoubleMetaphone::full().encode(name).primary;
            assert_eq!(po, f, "primary key differs for {name:?}");
        }
    }
}
