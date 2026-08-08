//! A rule-based lightweight Icelandic stemmer.
//!
//! # Why not Snowball?
//!
//! Unlike its Nordic siblings Swedish / Norwegian / Danish, Icelandic
//! has **no official Snowball stemmer**. Martin Porter's Snowball
//! project distributes reference algorithms for Danish / Dutch /
//! English / Finnish / French / German / Hungarian / Italian /
//! Norwegian / Portuguese / Romanian / Russian / Spanish / Swedish /
//! Turkish, but not for Icelandic — Icelandic morphology is fusional
//! with rich noun/adjective declension (4 cases × singular/plural × 3
//! genders) and strong/weak verb inflection, plus a definite article
//! that agglutinates as a suffix on the noun and interacts with the
//! stem's final vowel. A faithful lemmatizer would need a lexicon.
//!
//! # What this module ships
//!
//! A **rule-based lightweight suffix stripper** that removes the most
//! common inflectional endings, sufficient to collapse the plainest
//! noun / adjective / verb paradigms for IR-style keyword lookup.
//! Not a lemmatizer — the output is a suffix-stripped form, not the
//! dictionary head form. Callers who need lemma-quality reduction
//! (e.g., collapsing u-umlaut alternations like `hafa` / `höfum` to a
//! single form) should reach for a lexicon-backed pack.
//!
//! # Algorithm sketch
//!
//! 1. **Lowercase (Unicode-aware).** Fold input to lowercase so the
//!    suffix table (all lowercase) matches.
//! 2. **Longest-match suffix strip, minimum stem length ≥ 3.** Walk
//!    the (private) `SUFFIXES` table (sorted longest-first). For each
//!    entry, if the word ends with that suffix and the residue is ≥ 3
//!    characters, strip the suffix and restart the loop. Repeat until
//!    no suffix matches or the residue is ≤ 3 characters.
//!
//! # Suffix inventory
//!
//! Grouped by grammatical role. Any given suffix may be reached from
//! more than one paradigm — the table is deduplicated and applied by
//! longest match, so the role labels below are illustrative.
//!
//! **Definite-article suffixes** (Icelandic writes the definite
//! article as a noun suffix; the agglutination pattern depends on the
//! noun's gender and case):
//!
//! * `-inum` (4) — masc dat sg definite (`hestinum` "to the horse")
//! * `-inni` (4) — fem dat sg definite (`vísunni` "to the verse")
//! * `-inn` (3) — masc nom sg definite (`hesturinn` "the horse")
//! * `-nir` (3) — masc nom pl definite (`hestarnir` "the horses")
//! * `-nar` (3) — fem/masc acc pl definite (`konurnar` "the women")
//! * `-num` (3) — dat pl definite (`hestunum` "to the horses")
//! * `-nni` (3) — fem dat sg definite (alt. after weak-noun `-u` stem)
//! * `-ið` (2) — neut nom/acc sg definite (`húsið` "the house")
//! * `-in` (2) — fem nom sg / neut nom pl definite (`bókin` "the book")
//! * `-nu` (2) — fem acc/dat sg definite (weak-noun context)
//!
//! **Noun case endings** (indefinite):
//!
//! * `-ur` (2) — masc nom sg (`hestur` "horse", `strákur` "boy")
//! * `-ar` (2) — masc/fem nom pl, fem gen sg (`hestar`, `bókar`)
//! * `-ir` (2) — fem nom pl, masc weak-decl nom pl
//! * `-um` (2) — dat pl universal (`hestum`, `konum`)
//! * `-s` (1) — masc/neut gen sg (`hests`, `húss`)
//! * `-i` (1) — dat sg universal (`hesti`, `barni`)
//! * `-a` (1) — gen pl universal / weak neut nom/acc (`hesta`, `auga`)
//!
//! **Verb personal endings** (present indicative):
//!
//! * `-um` (2) — 1pl (`komum` "we come")
//! * `-uð` (2) — 2pl (`komuð` — archaic 2pl past)
//! * `-ir` (2) — 2sg (`kemur` for strong verbs; `kastir` for weak)
//! * `-ið` (2) — 2pl (`komið` "you (pl.) come")
//! * `-a` (1) — infinitive / 3pl (`koma`, `hafa`)
//!
//! **Adjective agreement** (strong and weak):
//!
//! * `-ur` (2) — masc nom sg strong (`stór` → `stórir`? no, base is
//!   `stór`; comparative `stærri` is a different lexeme)
//! * `-ir` (2) — masc nom pl strong
//! * `-um` (2) — dat pl strong
//! * `-a` (1) — weak declension universal (`stóra`)
//! * `-t` (1) — neut nom sg strong (`stórt`)
//!
//! # Minimum stem length
//!
//! Every strip is guarded so the residue has **at least three
//! characters**. This protects short words like `hús` "house"
//! (`h`,`ú`,`s` = 3 chars: the `-s` strip would leave `hú` = 2 chars,
//! blocked) and `mín` "my" (`m`,`í`,`n` = 3 chars: no matching
//! suffix, unchanged).
//!
//! # Non-goals
//!
//! * **Lexicon-backed lemmatization.** Reducing `hafa` and `höfum`
//!   to the same head form requires knowing that `hafa` is a lexeme
//!   whose 1pl present is `höfum` — orthographic suffix stripping
//!   alone can't reverse u-umlaut. This stemmer emits `haf` and `höf`
//!   respectively.
//! * **Compound splitting.** Icelandic productively compounds nouns
//!   (`bókasafn = bóka + safn` "library"). Splitting requires a
//!   compound-noun dictionary.
//! * **Consonant-mutation reversal.** Strong-verb ablaut
//!   (`taka`/`tók`, `bera`/`bar`) and the `-a`/`-á` alternations in
//!   noun paradigms are outside the stemmer's reach.
//! * **Article-final-vowel restoration.** After stripping `-inn`
//!   from `hesturinn` the residue is `hestur` — the algorithm doesn't
//!   restore the underlying `hest` in a single pass, but the loop
//!   re-runs and the second pass strips the `-ur` correctly. This is
//!   why the algorithm iterates rather than running a single pass.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use stringcheese_lang::Stemmer;

/// The Icelandic rule-based stemmer.
///
/// A zero-sized unit value; construct as [`IcelandicStemmer`] and
/// reuse the value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm and the
/// suffix inventory.
///
/// # Example
///
/// ```
/// use stringcheese_is::IcelandicStemmer;
/// use stringcheese_lang::Stemmer;
///
/// assert_eq!(IcelandicStemmer.stem("hesturinn"), "hest");
/// assert_eq!(IcelandicStemmer.stem("bókin"), "bók");
/// assert_eq!(IcelandicStemmer.stem("konum"), "kon");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct IcelandicStemmer;

/// The character-count floor for the residue after any single suffix
/// strip. See the [module-level docs](self) for the rationale.
const MIN_STEM_CHARS: usize = 3;

/// Extra character-count floor for the adjective neuter `-t` strip.
///
/// The bare `-t` neuter-strong strip is only safe when the residue is
/// at least four characters. Without this guard, `hest` (the residue
/// after `hestur → hest` via the `-ur` strip) would over-strip to
/// `hes` (still ≥ `MIN_STEM_CHARS` but no longer the intended stem).
/// Bumping the floor to 4 preserves the useful neuter fold
/// (`stórt` (5) → `stór` (4)) while blocking the false-positive on
/// short residues.
const MIN_STEM_CHARS_T: usize = 4;

/// A single suffix entry — a `char` slice plus the minimum residue
/// length required to strip it. Every entry uses the workspace-wide
/// [`MIN_STEM_CHARS`] floor except the neuter adjective `-t`, which
/// requires the stricter [`MIN_STEM_CHARS_T`] to avoid the
/// `hest → hes` over-strip described above.
struct Suffix {
    chars: &'static [char],
    min_stem: usize,
}

/// The suffix inventory used by the stemmer, deduplicated and sorted
/// longest-first. See the [module-level docs](self) for a breakdown
/// by grammatical role.
///
/// Storing entries as `&[char]` (rather than `&str`) lets the match
/// code work in units of Unicode scalars — every Icelandic-specific
/// letter (`ð`, `þ`, `æ`, `ö`, and the vowel-accented forms) is a
/// multi-byte UTF-8 scalar, and byte-index arithmetic would silently
/// corrupt the boundaries.
const SUFFIXES: &[Suffix] = &[
    // 4-char suffixes.
    Suffix {
        chars: &['i', 'n', 'u', 'm'],
        min_stem: MIN_STEM_CHARS,
    }, // -inum
    Suffix {
        chars: &['i', 'n', 'n', 'i'],
        min_stem: MIN_STEM_CHARS,
    }, // -inni
    // 3-char suffixes.
    Suffix {
        chars: &['i', 'n', 'n'],
        min_stem: MIN_STEM_CHARS,
    }, // -inn
    Suffix {
        chars: &['n', 'i', 'r'],
        min_stem: MIN_STEM_CHARS,
    }, // -nir
    Suffix {
        chars: &['n', 'a', 'r'],
        min_stem: MIN_STEM_CHARS,
    }, // -nar
    Suffix {
        chars: &['n', 'u', 'm'],
        min_stem: MIN_STEM_CHARS,
    }, // -num
    Suffix {
        chars: &['n', 'n', 'i'],
        min_stem: MIN_STEM_CHARS,
    }, // -nni
    // 2-char suffixes.
    Suffix {
        chars: &['n', 'u'],
        min_stem: MIN_STEM_CHARS,
    }, // -nu
    Suffix {
        chars: &['i', 'ð'],
        min_stem: MIN_STEM_CHARS,
    }, // -ið
    Suffix {
        chars: &['i', 'n'],
        min_stem: MIN_STEM_CHARS,
    }, // -in
    Suffix {
        chars: &['u', 'r'],
        min_stem: MIN_STEM_CHARS,
    }, // -ur
    Suffix {
        chars: &['a', 'r'],
        min_stem: MIN_STEM_CHARS,
    }, // -ar
    Suffix {
        chars: &['i', 'r'],
        min_stem: MIN_STEM_CHARS,
    }, // -ir
    Suffix {
        chars: &['u', 'm'],
        min_stem: MIN_STEM_CHARS,
    }, // -um
    Suffix {
        chars: &['u', 'ð'],
        min_stem: MIN_STEM_CHARS,
    }, // -uð
    // 1-char suffixes. `-t` carries the stricter MIN_STEM_CHARS_T.
    Suffix {
        chars: &['a'],
        min_stem: MIN_STEM_CHARS,
    }, // -a
    Suffix {
        chars: &['i'],
        min_stem: MIN_STEM_CHARS,
    }, // -i
    Suffix {
        chars: &['s'],
        min_stem: MIN_STEM_CHARS,
    }, // -s
    Suffix {
        chars: &['t'],
        min_stem: MIN_STEM_CHARS_T,
    }, // -t
];

impl IcelandicStemmer {
    /// Stems `word` per the Icelandic rule-based algorithm.
    ///
    /// Returns the stem as a [`Cow`]. If the algorithm makes no
    /// change to a lowercase input, the returned `Cow` borrows the
    /// input.
    #[must_use]
    pub fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        if word.chars().count() <= MIN_STEM_CHARS {
            // No possible strip leaves a residue that meets the
            // MIN_STEM_CHARS floor when the input is already at (or
            // under) that floor.
            return Cow::Borrowed(word);
        }

        // 1. Lowercase (Unicode-aware).
        let mut chars: Vec<char> = word.chars().flat_map(char::to_lowercase).collect();

        // 2. Repeated longest-match strip, guarded by MIN_STEM_CHARS.
        loop {
            let n = chars.len();
            if n <= MIN_STEM_CHARS {
                break;
            }
            let mut matched = false;
            for entry in SUFFIXES {
                let sl = entry.chars.len();
                if sl >= n {
                    // Would leave a residue of 0 or negative length.
                    continue;
                }
                let stem_len = n - sl;
                if stem_len < entry.min_stem {
                    continue;
                }
                if chars[stem_len..] == *entry.chars {
                    chars.truncate(stem_len);
                    matched = true;
                    break;
                }
            }
            if !matched {
                break;
            }
        }

        let out: String = chars.into_iter().collect();
        if out == word {
            Cow::Borrowed(word)
        } else {
            Cow::Owned(out)
        }
    }
}

impl Stemmer for IcelandicStemmer {
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
        IcelandicStemmer::stem(self, word)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: &str) -> String {
        IcelandicStemmer.stem(w).into_owned()
    }

    #[test]
    fn short_words_are_unchanged() {
        assert_eq!(s(""), "");
        assert_eq!(s("a"), "a");
        assert_eq!(s("og"), "og");
        assert_eq!(s("hús"), "hús"); // 3 chars — protected by MIN_STEM_CHARS.
        assert_eq!(s("mín"), "mín");
    }

    #[test]
    fn def_article_inn_strip_masc_nom_sg() {
        // hesturinn (the horse) → -inn → hestur → -ur → hest.
        assert_eq!(s("hesturinn"), "hest");
        // strákurinn (the boy) → -inn → strákur → -ur → strák.
        assert_eq!(s("strákurinn"), "strák");
    }

    #[test]
    fn def_article_in_strip_fem_nom_sg() {
        // bókin (the book) → -in → bók.
        assert_eq!(s("bókin"), "bók");
    }

    #[test]
    fn def_article_id_strip_neut_sg() {
        // húsið (the house) → -ið → hús.
        assert_eq!(s("húsið"), "hús");
    }

    #[test]
    fn def_article_nir_strip_masc_nom_pl() {
        // strákarnir (the boys) → -nir → strákar → -ar → strák.
        assert_eq!(s("strákarnir"), "strák");
    }

    #[test]
    fn def_article_nar_strip_fem_acc_pl() {
        // konurnar (the women) → -nar → konur → -ur → kon.
        assert_eq!(s("konurnar"), "kon");
    }

    #[test]
    fn def_article_num_strip_dat_pl() {
        // hestunum (to the horses) → -num → hestu → -a? no; done.
        //   Actually: -num leaves 'hestu' (5 chars). Then no matching
        //   suffix (u is not a listed suffix). Result 'hestu'.
        //   Note this is a genuine limitation of the rule-based
        //   approach — the underlying stem 'hest' would require the
        //   full analysis of the -unum agglutination.
        assert_eq!(s("hestunum"), "hestu");
    }

    #[test]
    fn def_article_nni_strip_fem_dat_sg() {
        // konunni (to the woman) → -nni → konu (4). Then no match on
        //   final 'u'. Result 'konu'.
        assert_eq!(s("konunni"), "konu");
    }

    #[test]
    fn def_article_inum_strip_masc_dat_sg() {
        // hestinum (to the horse) → -inum → hest (4).
        assert_eq!(s("hestinum"), "hest");
    }

    #[test]
    fn def_article_inni_strip_fem_dat_sg_alt() {
        // vísunni (to the verse) is fem dat sg def; but the classic
        //   fem-dat-sg agglutination that ends -inni surfaces on a-
        //   stems: 'skálinni' (to the bowl) → -inni → skál (4).
        assert_eq!(s("skálinni"), "skál");
    }

    #[test]
    fn noun_case_ur_strip() {
        // hestur (horse) → -ur → hest.
        assert_eq!(s("hestur"), "hest");
    }

    #[test]
    fn noun_case_ar_strip() {
        // hestar (horses) → -ar → hest.
        assert_eq!(s("hestar"), "hest");
    }

    #[test]
    fn noun_case_ir_strip() {
        // gestir (guests, masc nom pl) → -ir → gest.
        assert_eq!(s("gestir"), "gest");
    }

    #[test]
    fn noun_case_um_strip() {
        // konum (to women, dat pl) → -um → kon.
        assert_eq!(s("konum"), "kon");
    }

    #[test]
    fn noun_case_s_strip_gen_sg() {
        // hests (of horse) → -s → hest.
        assert_eq!(s("hests"), "hest");
        // Guarded by MIN_STEM_CHARS: 'húss' (of house) → -s would
        //   leave 'hús' (3 chars), which passes the guard.
        assert_eq!(s("húss"), "hús");
    }

    #[test]
    fn noun_case_i_strip_dat_sg() {
        // hesti (to horse, dat sg) → -i → hest.
        assert_eq!(s("hesti"), "hest");
    }

    #[test]
    fn noun_case_a_strip_gen_pl() {
        // hesta (of horses, gen pl) → -a → hest.
        assert_eq!(s("hesta"), "hest");
        // mála (of paintings, gen pl of mál "paint") → -a → mál.
        assert_eq!(s("mála"), "mál");
    }

    #[test]
    fn verb_um_strip_1pl() {
        // komum (we come) → -um → kom.
        assert_eq!(s("komum"), "kom");
    }

    #[test]
    fn verb_id_strip_2pl() {
        // komið (you (pl.) come) → -ið → kom.
        assert_eq!(s("komið"), "kom");
    }

    #[test]
    fn verb_a_strip_infinitive() {
        // koma (to come) → -a → kom.
        assert_eq!(s("koma"), "kom");
        // hafa (to have) → -a → haf.
        assert_eq!(s("hafa"), "haf");
    }

    #[test]
    fn adjective_t_strip_neut_strong() {
        // stórt (big, neut nom sg strong) → -t → stór.
        assert_eq!(s("stórt"), "stór");
    }

    #[test]
    fn adjective_um_strip_dat_pl() {
        // stórum (big, dat pl) → -um → stór.
        assert_eq!(s("stórum"), "stór");
    }

    #[test]
    fn adjective_ir_strip_masc_nom_pl() {
        // stórir (big, masc nom pl) → -ir → stór.
        assert_eq!(s("stórir"), "stór");
    }

    #[test]
    fn min_stem_guard_protects_short_words() {
        // hús (house, 3 chars) — no strip possible.
        assert_eq!(s("hús"), "hús");
        // ís (ice, 2 chars) — no strip possible.
        assert_eq!(s("ís"), "ís");
        // ár (year, 2 chars) — no strip possible.
        assert_eq!(s("ár"), "ár");
    }

    #[test]
    fn icelandic_specific_letters_preserved_when_no_rule_strips() {
        // þögn (silence) — 4 chars. No matching suffix (last char n
        //   isn't listed alone). Result 'þögn'.
        assert_eq!(s("þögn"), "þögn");
    }

    #[test]
    fn lowercase_fold_is_applied() {
        // Uppercase input folds and stems.
        assert_eq!(s("HESTUR"), "hest");
        assert_eq!(s("Bókin"), "bók");
    }

    #[test]
    fn iterative_strip_converges() {
        // hesturinn stems to hest in two passes; run again — result
        //   stable.
        assert_eq!(s("hest"), "hest");
    }
}
