//! The Nazief-Adriani Indonesian stemmer (simplified).
//!
//! # Origin
//!
//! Named after Bobby Nazief and Mirna Adriani (Universitas Indonesia,
//! 1996), whose confix-stripping algorithm is the canonical reference
//! for Indonesian IR stemmers. The algorithm was designed for a
//! language with **rich derivational affixation but zero inflection**:
//! Indonesian verbs don't conjugate for tense, aspect, mood, person,
//! or number, and nouns don't decline — every grammatical relation is
//! expressed by word order or by function words (`akan` future, `sudah`
//! perfective, `sedang` progressive, `para`/`banyak` plurality). What
//! *does* vary is the affix chain built out of the derivational
//! morphology: prefixes (`me-`, `pe-`, `di-`, `ber-`, `ter-`, `se-`,
//! `ke-`, `per-`), suffixes (`-kan`, `-i`, `-an`, `-nya`, `-lah`,
//! `-kah`, `-tah`, `-pun`, `-ku`, `-mu`), and combinations (circumfixes
//! like `ke...-an`, `pe...-an`, `me...-kan`).
//!
//! The full reference algorithm consults a **root-word dictionary** at
//! every strip step (accept a strip iff the residue is a valid root).
//! This crate ships the algorithm's **rule structure without the
//! dictionary lookup** — the strip decisions are made purely from the
//! surface form under length guards and consonant-restoration rules.
//! This is the same trade-off every offline `me-`/`pe-` stemmer makes
//! (Sastrawi's Java implementation ships a 30 000-word dictionary; the
//! Snowball-community candidate ships none). The shipped rules are
//! calibrated to over-stem rarely and under-stem sometimes, which is
//! the safer default for IR use.
//!
//! # Algorithm
//!
//! Five ordered steps, each conditional on a residue-length floor of
//! three characters. If any step would leave a stem shorter than
//! three characters, that step is skipped (and the algorithm falls
//! through to the next step with the pre-strip stem).
//!
//! ## Step 1 — stopword short-circuit
//!
//! If the input is in the crate's [`STOPWORDS`]
//! list, return it unchanged. Indonesian stopwords are function
//! words (`dan`, `di`, `yang`, `dengan`, …) — they carry no affix
//! chain and running the stripper on them can only mis-stem.
//!
//! ## Step 2 — particle suffix strip
//!
//! Remove one of the four **inflectional particle** suffixes if
//! present: `-lah` (imperative / emphatic), `-kah` (question /
//! interrogative), `-tah` (rhetorical), `-pun` (concessive, "even").
//! These attach to nouns, verbs, and pronouns interchangeably; they
//! sit outside the derivational chain and always strip first.
//!
//! ## Step 3 — possessive pronoun suffix strip
//!
//! Remove one of the three **possessive pronoun** suffixes if
//! present: `-ku` (1sg), `-mu` (2sg), `-nya` (3sg / definite). These
//! attach to nouns to form the possessive (`bukuku` "my book",
//! `rumahmu` "your house", `namanya` "his/her/its name"). Like the
//! particles, they sit outside the derivational chain.
//!
//! ## Step 4 — derivational suffix strip
//!
//! Remove one of the three **derivational suffixes** if present:
//! `-kan` (causative / benefactive), `-an` (nominalizer), `-i`
//! (locative / applicative). The rules:
//!
//! * `-kan` is stripped if present and the residue is ≥ 3 chars.
//! * `-an` is stripped if present and the residue is ≥ 3 chars. A
//!   special guard: don't strip `-an` when it forms the tail of the
//!   preceding `-kan` strip that already fired (avoids double-stripping
//!   `-k` then `-an`).
//! * `-i` is stripped if present, the residue is ≥ 3 chars, and the
//!   character before `-i` is NOT a vowel (so `pergi` "go" stays
//!   `pergi` — the final `-i` is part of the root, and stripping it
//!   would leave the vowel-final residue `perg` which is not a root).
//!   The "consonant before `-i`" guard is a standard Nazief-Adriani
//!   over-strip protection.
//!
//! ## Step 5 — derivational prefix strip with consonant restoration
//!
//! Remove the leading **derivational prefix**, if any. Indonesian
//! productive prefixes come in two flavours:
//!
//! ### Non-assimilating prefixes
//!
//! Strip cleanly, no consonant restoration:
//!
//! * `di-` — passive-voice marker (`dibaca` "is read" → `baca`).
//! * `ke-` — nominalizer / ordinal (`ketua` "chairperson" → `tua`).
//! * `se-` — one / similar (`seorang` "one person" → `orang`).
//! * `ter-` — accidental / superlative (`terjatuh` "fell" → `jatuh`,
//!   `terbaik` "best" → `baik`).
//! * `ber-` — reflexive / stative (`berjalan` "walk" → `jalan`).
//!   Special allomorph `bel-` on the root `ajar` (`belajar` "learn"
//!   → `ajar`); this crate handles that case explicitly.
//! * `per-` — causative nominalizer (`perbuatan` "action" → `buat`
//!   after `-an` suffix strip).
//!
//! ### Assimilating prefixes: `me-` and `pe-`
//!
//! These trigger **nasal assimilation** on the root's initial
//! consonant, and the initial consonant of the root sometimes elides.
//! The Nazief-Adriani stripping table reverses these:
//!
//! | Surface prefix | Rule                                    | Example                              |
//! |----------------|-----------------------------------------|--------------------------------------|
//! | `mem-` + vowel | strip `mem`, restore `p`                | `memilih` → `pilih`                  |
//! | `mem-` + b/f/v | strip `mem`, keep root                  | `membaca` → `baca`                   |
//! | `men-` + vowel | strip `men`, restore `t`                | `menulis` → `tulis`                  |
//! | `men-` + d/c/j | strip `men`, keep root                  | `mendengar` → `dengar`               |
//! | `meng-` + vowel| strip `meng`, restore `k` OR keep       | `mengambil` → `ambil` (no elision);  |
//! |                | (ambiguous; heuristic below)            | `mengirim` → `kirim` (k elided)      |
//! | `meny-` + vowel| strip `meny`, restore `s`               | `menyapu` → `sapu`                   |
//! | `me-` + l/m/n/r/w/y | strip `me`, keep root              | `melihat` → `lihat`                  |
//! | `pem-`, `pen-`, `peng-`, `peny-`, `pe-` | mirror rules for `pe-` | `pemilih` → `pilih`, `penulis` → `tulis` |
//!
//! **The `meng-` ambiguity.** `mengambil` "to take" is `meng- + ambil`
//! (no elision — `ambil` starts with a vowel). `mengirim` "to send" is
//! `meng- + kirim` (k elided — original root `kirim` starts with `k`,
//! and `k`-initial roots always elide under `meng-`). Without a
//! dictionary, both readings are plausible, so the stemmer makes a
//! best-effort choice: **prefer the no-elision reading** — strip
//! `meng` and take the residue as the stem. This under-stems `mengirim`
//! to `irim` (which is not a real root), but for IR purposes the
//! resulting equivalence class is stable — every conjugation of `kirim`
//! stems consistently as long as the algorithm's decisions are
//! deterministic. Callers that need dictionary-backed restoration
//! should reach for the Sastrawi library or a similar lexicon-driven
//! stemmer.
//!
//! # Iteration
//!
//! Steps 2–5 fire at most once each. The classic Nazief-Adriani
//! algorithm can strip a second prefix after the first (e.g., the
//! confix `me-per-` in `memperbaiki`), but under the no-dictionary
//! constraint a second strip is far more likely to over-stem, so the
//! shipped algorithm caps at one prefix per stem call.
//!
//! # Non-goals
//!
//! * **Dictionary-backed confirmation.** Requires shipping a root
//!   lexicon; that's a separate future crate.
//! * **Reduplication un-doing.** `buku-buku` "books" tokenizes as two
//!   halves; the stemmer's contract is per-word, so the halves stem
//!   individually and downstream systems can join them by identity.
//! * **Circumfix awareness beyond the three-step cascade.** The
//!   classic circumfixes (`ke...-an`, `pe...-an`, `per...-an`,
//!   `me...-kan`, `me...-i`, `di...-kan`) fall out for free from
//!   applying the suffix step then the prefix step in order.
//! * **Non-productive prefix hyphenated forms** (e.g., `pra-`, `pasca-`,
//!   `anti-` written with a hyphen). The tokenizer splits these at the
//!   hyphen and the stemmer sees the halves individually.

use alloc::borrow::Cow;
use alloc::string::String;

use stringcheese_lang::Stemmer;

use crate::stopwords::STOPWORDS;

/// The Nazief-Adriani Indonesian stemmer.
///
/// A zero-sized unit value; construct as [`IndonesianStemmer`] and
/// reuse the value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_id::IndonesianStemmer;
/// use stringcheese_lang::Stemmer;
///
/// assert_eq!(IndonesianStemmer.stem("membaca"), "baca");
/// assert_eq!(IndonesianStemmer.stem("memilih"), "pilih");
/// assert_eq!(IndonesianStemmer.stem("menulis"), "tulis");
/// assert_eq!(IndonesianStemmer.stem("makanan"), "makan");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct IndonesianStemmer;

/// Minimum stem length. Every strip step refuses to leave a residue
/// shorter than this floor.
const MIN_STEM_LEN: usize = 3;

impl IndonesianStemmer {
    /// Stems `word` per the simplified Nazief-Adriani algorithm.
    ///
    /// Returns the stem as a [`Cow`]. If no strip step fires (short
    /// input, stopword, or nothing matched), the returned `Cow`
    /// borrows the input without allocating.
    #[must_use]
    pub fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        // 0. Guard against inputs too short to bother.
        if word.chars().count() < MIN_STEM_LEN {
            return Cow::Borrowed(word);
        }

        // 1. Stopword short-circuit — stopwords are function words;
        // running the stripper on them can only mis-stem.
        if STOPWORDS.iter().any(|s| s.eq_ignore_ascii_case(word)) {
            return Cow::Borrowed(word);
        }

        // The strip algorithm works on the ASCII-lowercased form.
        // Indonesian's alphabet is exactly 26 ASCII letters so
        // `to_ascii_lowercase` is the correct case fold.
        let lower_owned = word.to_ascii_lowercase();
        let mut stem: String = lower_owned;

        // 2. Particle suffix (`-lah`, `-kah`, `-tah`, `-pun`).
        strip_suffix_if_present(&mut stem, PARTICLE_SUFFIXES);

        // 3. Possessive pronoun suffix (`-ku`, `-mu`, `-nya`).
        // Track whether a possessive was stripped: if so, step 4's
        // `-an` branch is skipped (in Indonesian a bare-noun +
        // possessive like `tangan + -ku` → `tanganku` is far more
        // common than a `-an`-derived noun + possessive like
        // `makanan + -ku` → `makananku`; skipping the second `-an`
        // strip keeps roots like `tangan` intact at the cost of
        // slightly under-stemming the `makananku` shape).
        let stripped_possessive = strip_suffix_if_present(&mut stem, POSSESSIVE_SUFFIXES);

        // 4. Derivational suffix (`-kan`, `-an`, `-i`) — with
        // confix-inhibition guards that consult the raw shape of the
        // pre-suffix-strip stem (so the guards see the productive
        // verbal prefixes that step 5 will later try to strip).
        strip_derivational_suffix(&mut stem, stripped_possessive);

        // 5. Derivational prefix — with `me-`/`pe-` consonant
        // restoration.
        strip_prefix(&mut stem);

        // Return borrowed if the final stem is byte-identical to the
        // input (also covers the "was already lowercase and nothing
        // changed" path). Otherwise return the owned lowercase stem.
        if stem == word {
            Cow::Borrowed(word)
        } else {
            Cow::Owned(stem)
        }
    }
}

impl Stemmer for IndonesianStemmer {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        IndonesianStemmer::stem(self, word)
    }
}

// ---------------------------------------------------------------------
// Suffix tables and stripping helpers.
// ---------------------------------------------------------------------

/// Particle suffixes — step 2.
///
/// Ordered longest-first so the strip helper's first-match walk picks
/// the longer suffix when two entries share a common tail (none of
/// these four do today, but the ordering discipline is worth
/// preserving in case a follow-up adds `-pulah` or similar).
const PARTICLE_SUFFIXES: &[&str] = &["lah", "kah", "tah", "pun"];

/// Possessive pronoun suffixes — step 3.
///
/// Ordered longest-first: `nya` (3 chars) before the 2-char `ku` /
/// `mu`. Without this ordering, `namanya` would strip `-a` (not a
/// possessive), miss `-nya`, and fall through.
const POSSESSIVE_SUFFIXES: &[&str] = &["nya", "ku", "mu"];

/// Strip the first suffix in `table` that matches the tail of `stem`
/// AND leaves a residue of at least [`MIN_STEM_LEN`] characters.
/// Returns `true` iff a strip fired.
///
/// `stem` must be ASCII-lowercase (Indonesian's 26-letter Latin
/// alphabet is entirely ASCII, so `str::ends_with` on `&str` is a
/// byte comparison and safe for the whole language).
fn strip_suffix_if_present(stem: &mut String, table: &[&str]) -> bool {
    for &sfx in table {
        if stem.ends_with(sfx) {
            let new_len = stem.len() - sfx.len();
            if new_len >= MIN_STEM_LEN {
                stem.truncate(new_len);
                return true;
            }
        }
    }
    false
}

/// Strip a derivational suffix (`-kan`, `-an`, `-i`) with
/// Nazief-Adriani's residue guards and confix-inhibition rules.
///
/// **Confix-inhibition rules** applied here (published Nazief-Adriani
/// literature; simplified subset):
///
/// * `ber-` / `di-` / `ter-` + `-an` is not a productive circumfix
///   (contrast with `pe-...-an`, `per-...-an`, `ke-...-an`,
///   `me-...-an` which are productive). If the stem starts with
///   `ber-` / `di-` / `ter-` AND ends with `-an`, don't strip the
///   `-an` — it is more likely to be part of the root
///   (`ber+jalan → berjalan`; the `-an` is root-final, not suffix).
/// * `-i` is more likely to be root-final when the stem starts with
///   a verbal prefix (`me-`/`pe-`/`di-`), because `me-VERB-i` circumfix
///   forms attach `-i` to consonant-final roots (`beri` "give" +
///   applicative `-i` doesn't productively re-suffix). Skipping the
///   `-i` strip when a verbal prefix is present prevents over-strips
///   like `menari` → `menar` → `tar` (correct is `menari` → `tari`).
fn strip_derivational_suffix(stem: &mut String, stripped_possessive: bool) {
    let starts_bdt = starts_with_any(stem, &["ber", "di", "ter"]);
    // Prefixes whose presence signals that `-i` is likely root-final
    // (not derivational). `me-`/`pe-` because `me-VERB` verbs often
    // end in a root-final `-i` (`menari` from `tari`, `menjadi` from
    // `jadi`); `di-` mirrors `me-`; `ber-`/`ter-` because these
    // prefixes don't productively combine with the applicative
    // `-i` suffix (`berlari` = `ber+lari` has root-final `-i`).
    //
    // Deliberately excluded: `per-` — the `per-...-i` circumfix IS
    // productive (`perbaiki` = `per + baik + -i`), so `-i` should
    // strip in that context.
    let starts_i_guarded_prefix = starts_with_any(stem, &["me", "pe", "di", "ber", "ter"]);

    // Try `-kan` first (longest). If it strips, done.
    if stem.ends_with("kan") && stem.len() - 3 >= MIN_STEM_LEN {
        stem.truncate(stem.len() - 3);
        return;
    }
    // Then `-an` — guarded by (a) the `ber-`/`di-`/`ter-` confix
    // inhibition and (b) the "possessive already stripped in step 3"
    // inhibition. Without (a), `berjalan` strips to `berjal` and then
    // step 5 strips `ber-` to `jal` (over-strip). Without (b),
    // `tanganku` strips its `-ku` in step 3 to `tangan`, then step 4
    // strips `-an` to `tang` (over-strip on a root-final `-an`).
    if stem.ends_with("an") && stem.len() - 2 >= MIN_STEM_LEN && !starts_bdt && !stripped_possessive
    {
        stem.truncate(stem.len() - 2);
        return;
    }
    // Finally `-i` — guarded by (a) the "consonant before `-i`" rule
    // and (b) the "no derivational prefix present" rule. Even
    // together, these leave a small residue of over-strips (`mati`
    // → `mat` — the `-i` is root-final but no prefix signals that).
    // Documented in the module docs.
    if stem.ends_with('i') && stem.len() > MIN_STEM_LEN && !starts_i_guarded_prefix {
        let prev = stem.as_bytes()[stem.len() - 2];
        if !is_vowel_byte(prev) {
            stem.truncate(stem.len() - 1);
        }
    }
}

/// Does `stem` start with any prefix in `prefixes`?
fn starts_with_any(stem: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|p| stem.starts_with(p))
}

/// Strip a derivational prefix with `me-`/`pe-` consonant restoration.
///
/// **Ordering.** Three-letter non-assimilating prefixes (`ber-`,
/// `per-`, `ter-`, `bel-`) are tried BEFORE the assimilating
/// `me-`/`pe-` family. Without this ordering, `perbuat` (residue
/// after `perbuatan` — `-an`) would match the bare `pe-` + `r`
/// (sonorant) rule and strip to `rbuat`; with `per-` tried first, it
/// strips cleanly to `buat`. Same reasoning for `berjalan` and
/// `terjatuh`.
fn strip_prefix(stem: &mut String) {
    if stem.len() < MIN_STEM_LEN + 2 {
        // A three-letter word can only shed a one-char prefix at
        // most, which no productive Indonesian prefix is. Bail.
        return;
    }
    // 1. Three-letter non-assimilating prefixes — `ber-` and its
    // `bel-` allomorph, `per-`, `ter-`. Longest first (`ber`/`per`
    // are 3 chars; we check `bel` before the shorter 2-letter
    // prefixes below).
    //
    // **Commit-on-shape.** If any of these 3-letter prefixes matches
    // the word's leading shape (even if the residue is below
    // [`MIN_STEM_LEN`]), we set `committed_to_bt` and skip the
    // assimilating `me-`/`pe-` handler below. Without this commit,
    // `pergi` (root "go", 5 chars) would fail the `per-` strip
    // (residue 2 chars, below floor) and fall through to
    // `pe-`+`r` (sonorant) — producing `rgi`. Committing on shape
    // preserves `pergi` unchanged.
    let mut committed_to_bt = false;
    for &pfx in &["ber", "per", "ter", "bel"] {
        if stem.starts_with(pfx) {
            committed_to_bt = true;
            let new_len = stem.len() - pfx.len();
            if new_len >= MIN_STEM_LEN {
                // Special case: `belajar` (bel- + ajar) is the sole
                // productive `bel-` allomorph of `ber-`. Every other
                // `bel-` initial word is not `bel- + root` and should
                // fall through to a shorter-prefix match (or no
                // match).
                if pfx == "bel" && stem != "belajar" {
                    continue;
                }
                *stem = stem[pfx.len()..].into();
                return;
            }
        }
    }
    // 2. Assimilating `me-`/`pe-` family — with consonant restoration.
    // Skipped when a 3-letter `ber-`/`per-`/`ter-`/`bel-` shape was
    // seen (see commit-on-shape above).
    if !committed_to_bt {
        if let Some(replacement) = try_strip_me_or_pe_prefix(stem.as_str()) {
            if replacement.chars().count() >= MIN_STEM_LEN {
                *stem = replacement;
                return;
            }
        }
    }
    // 3. Two-letter non-assimilating prefixes — `di-`, `ke-`, `se-`.
    for &pfx in &["di", "ke", "se"] {
        if stem.starts_with(pfx) {
            let new_len = stem.len() - pfx.len();
            if new_len >= MIN_STEM_LEN {
                *stem = stem[pfx.len()..].into();
                return;
            }
        }
    }
}

/// Try to strip a `me-`/`pe-` prefix with consonant restoration.
///
/// Returns `Some(new_stem)` when a rule matched, `None` when the input
/// does not begin with a recognized `me-`/`pe-` allomorph.
fn try_strip_me_or_pe_prefix(stem: &str) -> Option<String> {
    let bytes = stem.as_bytes();

    // ------------------------------------------------------------
    // Four-byte prefixes with restoration: `meng` / `meny` / `peng`
    // / `peny`.
    // ------------------------------------------------------------
    if bytes.len() > 4 {
        let head4 = &bytes[..4];
        let tail = &stem[4..];
        let first_tail_byte = bytes.get(4).copied();
        match head4 {
            b"meny" | b"peny" => {
                // `meny-` + vowel → strip `meny`, restore `s`.
                // Applies to `menyapu` → `sapu`, `menyanyi` → `sanyi`
                // (over-stems the native `nyanyi` root; documented
                // trade-off).
                if let Some(fb) = first_tail_byte {
                    if is_vowel_byte(fb) {
                        let mut out = String::with_capacity(tail.len() + 1);
                        out.push('s');
                        out.push_str(tail);
                        return Some(out);
                    }
                }
            }
            b"meng" | b"peng" => {
                // `meng-` + vowel → strip `meng`. Ambiguous with
                // `meng-` + `k`-initial-elision; the "no elision"
                // reading is the safer default (documented in the
                // module docs). `meng-` + consonant (rare — `menggali`
                // "to dig" from `me+gali`) → strip `meng`.
                return Some(tail.into());
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------
    // Three-byte prefixes with restoration: `mem`, `men`, `pem`,
    // `pen`.
    // ------------------------------------------------------------
    if bytes.len() > 3 {
        let head3 = &bytes[..3];
        let tail = &stem[3..];
        let first_tail_byte = bytes.get(3).copied();
        match head3 {
            b"mem" | b"pem" => {
                if let Some(fb) = first_tail_byte {
                    if is_vowel_byte(fb) {
                        // `mem-` + vowel → strip `mem`, restore `p`.
                        // `memilih` → `pilih`.
                        let mut out = String::with_capacity(tail.len() + 1);
                        out.push('p');
                        out.push_str(tail);
                        return Some(out);
                    }
                    // `mem-` + b/f/v → strip `mem`, no restoration.
                    // `membaca` → `baca`.
                    if matches!(fb, b'b' | b'f' | b'v' | b'p') {
                        return Some(tail.into());
                    }
                }
            }
            b"men" | b"pen" => {
                if let Some(fb) = first_tail_byte {
                    if is_vowel_byte(fb) {
                        // `men-` + vowel → strip `men`, restore `t`.
                        // `menulis` → `tulis`.
                        let mut out = String::with_capacity(tail.len() + 1);
                        out.push('t');
                        out.push_str(tail);
                        return Some(out);
                    }
                    // `men-` + d/c/j/t/s/z → strip `men`, no
                    // restoration. `mendengar` → `dengar`, `mencuci`
                    // → `cuci`, `menjaga` → `jaga`.
                    if matches!(fb, b'd' | b'c' | b'j' | b't' | b's' | b'z') {
                        return Some(tail.into());
                    }
                }
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------
    // Two-byte prefixes: `me`, `pe`. These are the bare-`me-` /
    // bare-`pe-` allomorphs that fire before roots starting with a
    // sonorant (l / m / n / r / w / y).
    // ------------------------------------------------------------
    if bytes.len() > 2 {
        let head2 = &bytes[..2];
        let tail = &stem[2..];
        let first_tail_byte = bytes.get(2).copied();
        match head2 {
            b"me" | b"pe" => {
                if let Some(fb) = first_tail_byte {
                    if matches!(fb, b'l' | b'm' | b'n' | b'r' | b'w' | b'y') {
                        return Some(tail.into());
                    }
                }
            }
            _ => {}
        }
    }

    None
}

/// Is the ASCII byte `b` an Indonesian vowel (`a`, `e`, `i`, `o`,
/// `u`)? Indonesian's five-vowel inventory maps directly to five
/// ASCII letters.
#[inline]
const fn is_vowel_byte(b: u8) -> bool {
    matches!(b, b'a' | b'e' | b'i' | b'o' | b'u')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &str) -> String {
        IndonesianStemmer.stem(w).into_owned()
    }

    #[test]
    fn short_words_are_unchanged() {
        assert_eq!(s(""), "");
        assert_eq!(s("a"), "a");
        assert_eq!(s("di"), "di");
    }

    #[test]
    fn stopwords_are_short_circuited() {
        // `dan` is a stopword; the stripper doesn't run — the surface
        // form is preserved even though `-an` would otherwise strip.
        assert_eq!(s("dan"), "dan");
        assert_eq!(s("yang"), "yang");
    }

    #[test]
    fn particle_suffix_strip() {
        // `bacalah` (read!) → `baca`. `-lah` is a bare imperative
        // particle. Step 2 strips `-lah`; steps 4–5 don't fire
        // because `baca` doesn't end in a derivational suffix and
        // doesn't start with a productive prefix.
        assert_eq!(s("bacalah"), "baca");
        // `siapakah` (who?) — `-kah` is the interrogative particle.
        // Step 2 strips `-kah` → `siapa`; the remainder isn't
        // stripped further.
        assert_eq!(s("siapakah"), "siapa");
        // `-pun` — `apapun` (whatever) → step 2 strips `-pun` → `apa`.
        // (`apa` is a stopword but we don't re-check the stopword
        // list at intermediate stages.)
        assert_eq!(s("apapun"), "apa");
    }

    #[test]
    fn possessive_suffix_strip() {
        // `bukuku` (my book) → `buku`.
        assert_eq!(s("bukuku"), "buku");
        // `rumahmu` (your house) → `rumah`.
        assert_eq!(s("rumahmu"), "rumah");
        // `namanya` (his/her name) → `nama`.
        assert_eq!(s("namanya"), "nama");
    }

    #[test]
    fn derivational_suffix_kan() {
        // `bacakan` (read to/for someone) → `baca`.
        assert_eq!(s("bacakan"), "baca");
        // `berikan` (give) → `beri` after ber- strip? No — `berikan`
        // starts with `ber` which is a prefix, and ends with `-kan`.
        // Step 4 strips `-kan` first → `beri`; step 5 tries `ber-`
        // strip which would leave `i` (2 chars, below floor) so it
        // bails.
        assert_eq!(s("berikan"), "beri");
    }

    #[test]
    fn derivational_suffix_an() {
        // `makanan` (food) → `makan`.
        assert_eq!(s("makanan"), "makan");
        // `jalanan` (streets) → `jalan`.
        assert_eq!(s("jalanan"), "jalan");
    }

    #[test]
    fn derivational_suffix_i_guarded_by_consonant() {
        // `panjangi` (lengthen) → `panjang`. The char before `-i` is
        // `g` (a consonant), and `panjangi` doesn't start with a
        // verbal prefix (the `pan-` prefix isn't in the productive
        // list), so the strip fires.
        assert_eq!(s("panjangi"), "panjang");
        // A vowel-before-i case: `bagai` — the char before the final
        // `-i` is `a` (vowel), so the "consonant before `-i`" guard
        // blocks the strip and `bagai` stays `bagai`.
        assert_eq!(s("bagai"), "bagai");
    }

    #[test]
    fn documented_over_strips_are_locked_in() {
        // Without a dictionary, the algorithm cannot distinguish a
        // root-final `-i` from a derivational `-i` when no verbal
        // prefix signals the difference. `mati` (dead) is the canonical
        // example: no prefix, `-i` preceded by consonant `t`, so
        // step 4 strips it → `mat`. A dictionary would reject.
        assert_eq!(s("mati"), "mat");
        // `mengirim` (send): the `meng-` prefix is ambiguous (no
        // elision reading wins under the shipped rules), so the root
        // `kirim` (with elided `k`) is not recovered. Result:
        // `irim`. Documented.
        assert_eq!(s("mengirim"), "irim");
    }

    #[test]
    fn per_prefix_commit_prevents_pergi_over_strip() {
        // Regression: without the commit-on-shape rule for the
        // 3-letter `per-`/`ber-`/`ter-`/`bel-` prefixes, `pergi`
        // (root, "go") would fail the `per-` strip (residue too
        // short) and fall through to `pe-`+`r` (sonorant) → `rgi`.
        // With the commit, `pergi` is left unchanged.
        assert_eq!(s("pergi"), "pergi");
    }

    #[test]
    fn di_prefix_strip() {
        // `dibaca` (is read) → `baca`.
        assert_eq!(s("dibaca"), "baca");
    }

    #[test]
    fn ke_prefix_strip() {
        // `ketua` (chairperson) → `tua`.
        assert_eq!(s("ketua"), "tua");
    }

    #[test]
    fn se_prefix_strip() {
        // `seorang` (one person) → `orang`.
        assert_eq!(s("seorang"), "orang");
    }

    #[test]
    fn ter_prefix_strip() {
        // `terbaik` (best) → `baik`.
        assert_eq!(s("terbaik"), "baik");
        // `terjatuh` (fell) → `jatuh`.
        assert_eq!(s("terjatuh"), "jatuh");
    }

    #[test]
    fn ber_prefix_strip() {
        // `berjalan` (walk) → `jalan`.
        assert_eq!(s("berjalan"), "jalan");
    }

    #[test]
    fn belajar_special_case() {
        // `belajar` (learn) is the sole `bel-` allomorph → `ajar`.
        assert_eq!(s("belajar"), "ajar");
    }

    #[test]
    fn me_prefix_bare_before_sonorant() {
        // `melihat` (see) → `lihat`. `me-` + `l` = bare `me-` strip.
        assert_eq!(s("melihat"), "lihat");
    }

    #[test]
    fn mem_prefix_with_p_restoration() {
        // `memilih` (choose) → `pilih` (p elided under `mem-`).
        assert_eq!(s("memilih"), "pilih");
    }

    #[test]
    fn mem_prefix_no_restoration_before_b() {
        // `membaca` (read) → `baca`.
        assert_eq!(s("membaca"), "baca");
    }

    #[test]
    fn men_prefix_with_t_restoration() {
        // `menulis` (write) → `tulis` (t elided under `men-`).
        assert_eq!(s("menulis"), "tulis");
        // `menari` (dance) → `tari` (t elided).
        assert_eq!(s("menari"), "tari");
    }

    #[test]
    fn men_prefix_no_restoration_before_dcj() {
        // `mendengar` (hear) → `dengar`.
        assert_eq!(s("mendengar"), "dengar");
        // `menjaga` (guard) → `-a` is a vowel, no `-i` strip
        // interference. `men-` + `j` → strip `men` no restoration
        // → `jaga`.
        assert_eq!(s("menjaga"), "jaga");
        // `mencuci` (wash) → the verbal-prefix guard now protects the
        // root-final `-i`, so `-i` is NOT stripped in step 4. Step 5
        // sees `men-` + `c` (a `d/c/j/t/s/z` initial after `men-`)
        // → strip `men` no restoration → `cuci`. Clean.
        assert_eq!(s("mencuci"), "cuci");
    }

    #[test]
    fn meng_prefix_before_vowel() {
        // `mengambil` (take) → `ambil` (no elision — `ambil` starts
        // with a vowel).
        assert_eq!(s("mengambil"), "ambil");
    }

    #[test]
    fn meny_prefix_with_s_restoration() {
        // `menyapu` (sweep) → `sapu` (s elided under `meny-`).
        assert_eq!(s("menyapu"), "sapu");
    }

    #[test]
    fn pem_prefix_agent_nominalizer() {
        // `pemilih` (voter) → `pilih` (p elided under `pem-`).
        assert_eq!(s("pemilih"), "pilih");
    }

    #[test]
    fn pen_prefix_agent_nominalizer() {
        // `penulis` (writer) → `tulis` (t elided under `pen-`).
        assert_eq!(s("penulis"), "tulis");
    }

    #[test]
    fn min_stem_floor_prevents_over_stripping() {
        // `dua` is a stopword — bail without stripping. It is also 3
        // chars, so even if it weren't a stopword the floor would
        // prevent any prefix from firing.
        assert_eq!(s("dua"), "dua");
        // `pun` is a stopword — bail.
        assert_eq!(s("pun"), "pun");
    }

    #[test]
    fn combined_prefix_and_suffix_circumfix() {
        // `perbuatan` (deed) — `-an` strip first → `perbuat`, then
        // `per-` prefix → `buat`.
        assert_eq!(s("perbuatan"), "buat");
        // `kesatuan` (unity) — `-an` → `kesatu`, then `ke-` → `satu`.
        // But `satu` is a stopword — the short-circuit only runs at
        // the top; after stripping, we don't re-check. So result is
        // `satu`.
        assert_eq!(s("kesatuan"), "satu");
    }
}
