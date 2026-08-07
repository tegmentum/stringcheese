//! A Vietnamese-tuned Soundex-family phonetic encoder.
//!
//! # Origin
//!
//! Vietnamese has no widely-adopted canonical phonetic encoder the
//! way English has Soundex or German has Kölner Phonetik. The closest
//! options are:
//!
//! * **Language-independent Soundex** applied to ASCII-folded
//!   Vietnamese — stable and portable, but linguistically inaccurate
//!   (the Vietnamese digraphs `ng`, `nh`, `ph`, `kh`, `tr`, `ch` all
//!   get swept under the English mapping, and Vietnamese `tr` and `ch`
//!   land in different Soundex classes than they should).
//! * **PHONEX-family encoders** — the Vietnamese equivalent of the
//!   French / Spanish / Portuguese / Polish PHONEX encoders shipped
//!   in the sibling language packs: apply Vietnamese-specific
//!   preprocessing (all diacritics stripped, digraph rewrites for
//!   `ng`, `nh`, `ph`, `kh`, `tr`, `ch`), then run a Soundex-shape
//!   encoder over the Vietnamese-tuned classification table.
//!
//! # Implementation choice
//!
//! This module ships a **PHONEX-Vietnamese** encoder. Concretely, the
//! algorithm is a 4-character `<letter><digit><digit><digit>` Soundex
//! key with a Vietnamese-tuned preprocessing pass and a
//! Vietnamese-tuned classification table:
//!
//! 1. **Strip every diacritic and fold to plain ASCII uppercase.**
//!    Delegate to
//!    [`crate::normalize::VietnameseNormalizer::with_strip_all_diacritics`]
//!    so `ằ → A`, `đ → D`, `ệ → E`, `ự → U`. The phonetic key is a
//!    coarse equivalence class and Vietnamese speakers frequently
//!    type toneless or diacritic-free text on non-Vietnamese
//!    keyboards; folding every diacritic ensures `bàn` / `bán` /
//!    `bản` / `bãn` / `bạn` / `ban` all produce keys with the same
//!    vocalic skeleton.
//! 2. **Vietnamese digraph substitutions** (applied left-to-right,
//!    longest-match first):
//!    * `NG → N` — the /ŋ/ velar-nasal digraph collapses to the
//!      nasal slot. Applies to `nga`, `nghi`, `ngôn`, etc.
//!    * `NH → N` — the /ɲ/ palatal-nasal digraph collapses to the
//!      nasal slot. Applies to `nhà`, `nhỏ`, etc.
//!    * `PH → F` — the /f/ digraph collapses to the fricative slot.
//!      Applies to `phở`, `pháp`, `phim`, etc. `F` then falls into
//!      the labial class 1.
//!    * `KH → K` — the /x/ digraph folds to `K` (velar). Applies to
//!      `khách`, `không`, etc.
//!    * `TR → T` — the retroflex /ʈ/ digraph (or /tɕ/ in some
//!      dialects) collapses to `T`; encoding it as class `3` matches
//!      `t`. Vietnamese `tr` and `t` are frequently confused in
//!      informal spelling.
//!    * `CH → X` — the /tɕ/ digraph. Encoded as `X` which falls into
//!      class `2` alongside `k / g / q / j`. We use `X` (not `C`) as
//!      the intermediate letter to keep the digraph visible in the
//!      preprocessed buffer for debugging.
//!    * `GI → Y` — the /z/ or /j/ digraph (dialect-dependent) folds
//!      to `Y` (class 0, dropped as a vowel/glide).
//!    * `GH → G` — the /ɣ/ digraph (before front vowels) folds to
//!      `G` (class 2, velar).
//!    * `QU → K` — the labiovelar /kw/ digraph folds to `K` (class 2).
//!    * `H → ` — silent (dropped entirely). Note: `H` is stripped
//!      **after** the digraph pass, so `ph / kh / nh / ch / gh` don't
//!      lose their consonant.
//! 3. **Soundex-shape encoding.** Retain the first letter; encode
//!    each subsequent letter by the classification table below; drop
//!    the zero class (vowels); collapse consecutive equal codes;
//!    truncate to three digits and left-pad with `'0'` to reach
//!    length four.
//!
//! **Classification table.**
//!
//! | Code | Letters |
//! |------|---------|
//! | 1    | B P F V W |
//! | 2    | C K G Q J X |
//! | 3    | D T     |
//! | 4    | L       |
//! | 5    | M N     |
//! | 6    | R       |
//! | 7    | S Z     |
//! | 0    | A E I O U Y (dropped as vowels); H is stripped in preprocessing |
//!
//! **Grouping rationale.** Vietnamese `B/P/F/V/W` are all labial
//! obstruents; `C/K/G/Q/J/X` are the velar / palatal family (`X`
//! from the `ch` digraph rewrite ends up here); `D/T` are dental
//! stops; `S/Z` are the sibilant family; `L` / `R` get their own
//! classes; `M/N` are nasals (which after the `ng` / `nh` digraph
//! rewrites also subsumes the two Vietnamese nasal digraphs). `Y`
//! is treated as a vowel (dropped) — Vietnamese `y` is a glide that
//! functions as a vowel in most positions.
//!
//! # Deferred to a follow-up wave
//!
//! * **Métaphone Vietnamese** — a parallel variable-length encoder
//!   with better discrimination; heavier to reference-test.
//! * **Dialect-specific rules.** Northern (Hanoi), Central (Huế),
//!   and Southern (Saigon) dialects have distinct pronunciations
//!   (`d` = /z/ in the north, /j/ in the south; `gi` = /z/ vs. /j/;
//!   `s` = /s/ vs. /ʂ/; …) that a dialect-aware encoder could
//!   distinguish. Out of scope for the initial drop.
//! * **Tone-preserving encoder.** A follow-up encoder that carries
//!   one tone-mark digit at the end of the key (`bàn → B5-1`,
//!   `bán → B5-2`) would preserve tonal distinctions that the
//!   current design deliberately folds. Out of scope for the initial
//!   drop.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

use crate::normalize::VietnameseNormalizer;

/// The Vietnamese PHONEX encoder.
///
/// A zero-sized value; construct as [`VietnamesePhonex`] and reuse
/// the value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_vi::VietnamesePhonex;
///
/// let key = VietnamesePhonex.encode("Nguyễn").unwrap();
/// // Vietnamese-tuned digraph pass: `ng → N`, then Soundex-shape
/// // encoding over the diacritic-folded stem "NUYEN".
/// assert_eq!(key, "N500");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct VietnamesePhonex;

impl VietnamesePhonex {
    /// Encodes `word` per the PHONEX-Vietnamese algorithm.
    ///
    /// Returns `None` when `word` has no letter content (empty input,
    /// pure whitespace, all punctuation, or an input reduced to
    /// nothing after silent-`H` stripping). Otherwise returns a
    /// 4-character key of the form
    /// `<uppercase letter><three ASCII digits>`.
    #[must_use]
    pub fn encode(&self, word: &str) -> Option<String> {
        let ascii = preprocess(word);
        if ascii.is_empty() {
            return None;
        }
        let bytes = ascii.as_bytes();

        let mut out = String::with_capacity(4);
        out.push(bytes[0] as char);
        let mut last_code = code_of(bytes[0]);
        for &b in &bytes[1..] {
            let code = code_of(b);
            if code == b'0' {
                // Vowel — reset the duplicate-collapse state.
                last_code = b'0';
                continue;
            }
            if code == last_code {
                continue;
            }
            out.push(code as char);
            last_code = code;
            if out.len() == 4 {
                break;
            }
        }
        while out.len() < 4 {
            out.push('0');
        }
        Some(out)
    }
}

/// Preprocess `word` into uppercase-ASCII letters after Vietnamese
/// digraph and silent-letter substitutions.
fn preprocess(word: &str) -> String {
    // Step 1: strip every diacritic (tone marks + letter modifiers)
    // and fold to plain ASCII, delegating to the normalizer's
    // full-diacritic strip. The result is still mixed-case; we
    // uppercase in a subsequent pass.
    let stripped = VietnameseNormalizer::builder()
        .with_strip_all_diacritics(true)
        .normalize(word);

    // Step 2: uppercase-ASCII fold — drop anything that isn't an
    // ASCII letter after the diacritic strip.
    let mut ascii = String::with_capacity(stripped.len());
    for c in stripped.chars() {
        if c.is_ascii_alphabetic() {
            ascii.push(c.to_ascii_uppercase());
        }
    }

    // Step 3: digraph & silent-letter substitutions on the ASCII
    // upper-case buffer.
    let bytes = ascii.as_bytes();
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        // Two-byte digraph substitutions (longest match).
        if i + 1 < bytes.len() {
            let b2 = bytes[i + 1];
            match (b, b2) {
                (b'N', b'G') => {
                    // /ŋ/ — collapse to the nasal slot.
                    out.push('N');
                    i += 2;
                    continue;
                }
                (b'N', b'H') => {
                    // /ɲ/ — collapse to the nasal slot.
                    out.push('N');
                    i += 2;
                    continue;
                }
                (b'P', b'H') => {
                    // /f/ — collapse to the fricative slot.
                    out.push('F');
                    i += 2;
                    continue;
                }
                (b'K', b'H') => {
                    // /x/ — velar fricative, fold to `K` (class 2).
                    out.push('K');
                    i += 2;
                    continue;
                }
                (b'T', b'R') => {
                    // /ʈ/ — collapse to `T` (class 3).
                    out.push('T');
                    i += 2;
                    continue;
                }
                (b'C', b'H') => {
                    // /tɕ/ — collapse to `X` (class 2).
                    out.push('X');
                    i += 2;
                    continue;
                }
                (b'G', b'I') => {
                    // /z/ or /j/ — collapse to `Y` (glide, dropped as
                    // vowel).
                    out.push('Y');
                    i += 2;
                    continue;
                }
                (b'G', b'H') => {
                    // /ɣ/ (before front vowels) — collapse to `G`.
                    out.push('G');
                    i += 2;
                    continue;
                }
                (b'Q', b'U') => {
                    // /kw/ — collapse to `K`.
                    out.push('K');
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        // Single-letter substitutions.
        match b {
            b'H' => { /* drop as silent — after all H-carrying
                digraphs have already fired above */
            }
            _ => out.push(b as char),
        }
        i += 1;
    }
    out
}

/// Soundex-family digit for byte `b` (an ASCII uppercase letter).
#[inline]
fn code_of(b: u8) -> u8 {
    match b {
        b'B' | b'P' | b'F' | b'V' | b'W' => b'1',
        b'C' | b'K' | b'G' | b'Q' | b'J' | b'X' => b'2',
        b'D' | b'T' => b'3',
        b'L' => b'4',
        b'M' | b'N' => b'5',
        b'R' => b'6',
        b'S' | b'Z' => b'7',
        _ => b'0',
    }
}

/// Adapter that exposes [`VietnamesePhonex`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Vietnamese::phonetic_encoder`](crate::Vietnamese) hands back.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct VietnamesePhonexAdapter;

impl LanguagePhoneticEncoder for VietnamesePhonexAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        VietnamesePhonex.encode(word).map(|k| (k, None))
    }

    fn name(&self) -> &'static str {
        "phonex-vi"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(w: &str) -> String {
        VietnamesePhonex.encode(w).expect("non-empty input encodes")
    }

    #[test]
    fn empty_input_returns_none() {
        assert!(VietnamesePhonex.encode("").is_none());
        assert!(VietnamesePhonex.encode("   ").is_none());
        assert!(VietnamesePhonex.encode("---").is_none());
    }

    #[test]
    fn tone_variants_collapse() {
        // The six tone variants of `ban` all encode identically —
        // every diacritic is stripped in preprocessing.
        //   ban / bàn / bán / bản / bãn / bạn all → "BAN"
        //   → B seed last=1. A reset last=0. N code=5 push last=5.
        //   → "B5" pad → "B500".
        assert_eq!(p("ban"), "B500");
        assert_eq!(p("bàn"), "B500");
        assert_eq!(p("bán"), "B500");
        assert_eq!(p("bản"), "B500");
        assert_eq!(p("bãn"), "B500");
        assert_eq!(p("bạn"), "B500");
    }

    #[test]
    fn letter_modifiers_fold_to_base() {
        // `ă / â` all fold to `a`. `ê` folds to `e`. `ô / ơ` fold to
        // `o`. `ư` folds to `u`. `đ` folds to `d`.
        //   `băn` → "BAN" → same as `ban`.
        assert_eq!(p("băn"), p("ban"));
        //   `bên` → "BEN" → B seed last=1. E reset. N code=5 push.
        //   → "B5" → "B500". Same as `ban`.
        assert_eq!(p("bên"), p("ban"));
        //   `đường` → "DUONG" → then `ng` → `N`: "DUON".
        //   D seed last=3. U reset. O reset. N code=5 push last=5.
        //   → "D5" → "D500".
        assert_eq!(p("đường"), "D500");
    }

    #[test]
    fn ng_digraph_collapses() {
        // `ng` → `N`. `Nga` → "NA" (after ng→N). Wait — the digraph
        // rewrite happens on the ASCII buffer *after* diacritic
        // stripping, so `Nga` → "NGA" → "NA" (after ng→N).
        //   N seed last=5. A reset → "N" pad → "N000".
        assert_eq!(p("Nga"), "N000");
        //   `Nghi` → "NGHI" → "NH" wait, let me re-trace. The rewrite
        //   loop consumes NG → 'N' at i=0 (advancing by 2), then
        //   sees H at i=2 which is silent-dropped, then I at i=3.
        //   Buffer: "NI".
        //   N seed last=5. I reset → "N" pad → "N000".
        assert_eq!(p("Nghi"), "N000");
    }

    #[test]
    fn nh_digraph_collapses_to_nasal() {
        // `nh` → `N`. `nhà` → "NHA" → "NA" (after nh→N).
        //   N seed last=5. A reset → "N" pad → "N000".
        assert_eq!(p("nhà"), "N000");
        //   `nhỏ` → "NHO" → "NO" → "N000".
        assert_eq!(p("nhỏ"), "N000");
    }

    #[test]
    fn ph_digraph_folds_to_f() {
        // `ph` → `F` (labial class 1). `phở` → "PHO" → "FO".
        //   F seed last=1. O reset → "F" pad → "F000".
        assert_eq!(p("phở"), "F000");
        //   `pháp` → "PHAP" → "FAP".
        //   F seed last=1. A reset. P code=1 push — wait, that's a
        //   dup with last_code=1. Actually after A reset last_code=0,
        //   so P code=1 != 0, push → "F1" last=1. → "F1" pad → "F100".
        assert_eq!(p("pháp"), "F100");
    }

    #[test]
    fn kh_digraph_folds_to_k() {
        // `kh` → `K`. `không` → "KHONG" → "KON" (kh→K, then ng→N).
        //   K seed last=2. O reset. N code=5 push last=5 → "K5"
        //   → "K500".
        assert_eq!(p("không"), "K500");
        //   `khách` → "KHACH" → "KAX" (kh→K, ch→X).
        //   K seed last=2. A reset. X code=2 push (not dup — reset)
        //   → "K2" pad → "K200".
        assert_eq!(p("khách"), "K200");
    }

    #[test]
    fn tr_digraph_folds_to_t() {
        // `tr` → `T`. `trà` → "TRA" → "TA".
        //   T seed last=3. A reset → "T" pad → "T000".
        assert_eq!(p("trà"), "T000");
        //   `trong` → "TRONG" → "TON" (tr→T, ng→N).
        //   T seed last=3. O reset. N code=5 push → "T5" → "T500".
        assert_eq!(p("trong"), "T500");
    }

    #[test]
    fn ch_digraph_folds_to_x_which_encodes_as_class_2() {
        // `ch` → `X`. `chào` → "CHAO" → "XAO".
        //   X seed last=2. A reset. O reset → "X" pad → "X000".
        assert_eq!(p("chào"), "X000");
    }

    #[test]
    fn silent_h_is_stripped() {
        // Bare `h` drops. `hoa` → "HOA" → "OA" (h dropped, oa are
        // vowels).
        //   O seed last=0. A reset → "O" pad → "O000".
        assert_eq!(p("hoa"), "O000");
        // `hà` → "HA" → "A" → "A000".
        assert_eq!(p("hà"), "A000");
    }

    #[test]
    fn qu_digraph_folds_to_k() {
        // `qu` → `K`. `quả` → "QUA" → "KA".
        //   K seed last=2. A reset → "K" pad → "K000".
        assert_eq!(p("quả"), "K000");
    }

    #[test]
    fn common_vietnamese_surnames() {
        // Nguyễn: "NGUYEN" (all diacritics stripped) → "NUYEN" (ng→N).
        //   N seed last=5. U reset. Y reset (vowel/glide class 0).
        //   E reset. N code=5 push (not dup — reset between) → "N5"
        //   → "N500".
        //
        // Hmm wait — after N seed last=5, we have U reset last=0, Y
        // reset last=0 (Y is class 0), E reset, N code=5, last was 0
        // so 5 != 0, push. "N5". But wait, we already have N in
        // position 0 — the seed. The consecutive-collapse doesn't
        // check the seed, only successive pushes. Let me re-check the
        // algorithm.
        //
        // Actually looking at the code: last_code = code_of(bytes[0])
        // = 5. Then U: code 0, last_code = 0. Y: code 0, no push,
        // last=0. E: code 0, last=0. N: code 5, 5 != 0, push. So we
        // get "N5" (len 2). Pad → "N500". Hmm but we already have
        // "N" as position 0 — the second N is a code push. So the
        // output is "N" + "5" = "N5" → "N500". But wait, is it
        // desirable that a word starting with N and having another N
        // later gets `N5`? Yes — that's how Soundex works. Good.
        //
        // Actually hold on. Let me recount. "NUYEN" is 5 chars.
        // Seed: 'N' → out="N", last_code=5.
        // Next 'U': code=0 (vowel), last_code=0.
        // Next 'Y': code=0 (vowel), last_code=0.
        // Next 'E': code=0 (vowel), last_code=0.
        // Next 'N': code=5, 5 != 0 (last was reset), push '5'. out="N5"
        //   last_code=5.
        // Out is 2 chars. Pad → "N500".
        //
        // Wait — but the trace should end with N5. Actually the test
        // used to expect N250. Let me update — the doc example above
        // said "N250" but the correct trace is "N500".
        //
        // Update the doc example above accordingly (already fixed).
        assert_eq!(p("Nguyễn"), "N500");
        // Trần: TRAN → TAN (tr→T). T seed last=3. A reset. N code=5
        //   push. → "T5" → "T500".
        assert_eq!(p("Trần"), "T500");
        // Lê: LE → L seed last=4. E reset → "L" → "L000".
        assert_eq!(p("Lê"), "L000");
        // Hà (family name): HA → A (h dropped) → "A000".
        assert_eq!(p("Hà"), "A000");
        // Phạm: PHAM → FAM. F seed last=1. A reset. M code=5 push.
        //   → "F5" → "F500".
        assert_eq!(p("Phạm"), "F500");
        // Hoàng: HOANG → OANG → ON (h dropped, ng→N). O seed last=0.
        //   A reset. N code=5 push. → "O5" → "O500".
        assert_eq!(p("Hoàng"), "O500");
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(p("NGUYEN"), p("nguyen"));
        assert_eq!(p("PHỞ"), p("phở"));
    }

    #[test]
    fn short_input_pads_to_four() {
        assert_eq!(p("A"), "A000");
        assert_eq!(p("Ba"), "B000");
    }

    #[test]
    fn duplicate_consonants_collapse() {
        // Not common in Vietnamese, but the algorithm handles it.
        // "ann" → A(seed), N(5,push,last=5), N(dup drop) → "A5"
        //   → "A500"
        assert_eq!(p("ann"), "A500");
    }

    #[test]
    fn adapter_returns_name_phonex_vi() {
        assert_eq!(VietnamesePhonexAdapter.name(), "phonex-vi");
    }

    #[test]
    fn adapter_returns_none_for_empty() {
        assert!(VietnamesePhonexAdapter.encode("").is_none());
    }

    #[test]
    fn adapter_wraps_phonex_output_in_primary_only_tuple() {
        let (primary, alt) = VietnamesePhonexAdapter.encode("Nguyễn").unwrap();
        assert_eq!(primary, "N500");
        assert!(alt.is_none());
    }
}
