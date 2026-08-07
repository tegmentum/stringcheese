//! Buckwalter transliteration reference input/output pairs.
//!
//! 32 well-known Arabic names, most from the "common Arabic surnames"
//! canon (Muhammad, Ahmad, Ali, Hassan, Husayn, Umar, Yusuf, Ibrahim,
//! Khalid, Fatima, Aisha, Maryam), plus a handful of function words
//! and a couple of vocalized forms to exercise the diacritic mappings.
//! Every expected value is derived character-by-character from the
//! Buckwalter mapping table documented at
//! <https://en.wikipedia.org/wiki/Buckwalter_transliteration>.

extern crate alloc;

use stringcheese_ar::Buckwalter;

/// 32 reference pairs.
const PAIRS: &[(&str, &str)] = &[
    // -----------------------------------------------------------------
    // Common male given names.
    // -----------------------------------------------------------------
    ("محمد", "mHmd"),       // Muhammad
    ("أحمد", ">Hmd"),       // Ahmad (hamza above alef)
    ("علي", "Ely"),         // Ali
    ("حسن", "Hsn"),         // Hassan
    ("حسين", "Hsyn"),       // Husayn
    ("عمر", "Emr"),         // Umar
    ("يوسف", "ywsf"),       // Yusuf
    ("إبراهيم", "<brAhym"), // Ibrahim (hamza below alef)
    ("خالد", "xAld"),       // Khalid
    ("سعيد", "sEyd"),       // Said
    ("طارق", "TArq"),       // Tariq
    ("زياد", "zyAd"),       // Ziyad
    ("جمال", "jmAl"),       // Jamal
    ("عبد", "Ebd"),         // Abd (as in Abd-Allah)
    ("رشيد", "r$yd"),       // Rashid — sheen encodes to `$`
    // -----------------------------------------------------------------
    // Common female given names.
    // -----------------------------------------------------------------
    ("فاطمة", "fATmp"), // Fatima (teh marbuta → 'p')
    ("عائشة", "EA}$p"), // Aisha (yeh with hamza above → '}')
    ("مريم", "mrym"),   // Maryam
    ("زينب", "zynb"),   // Zaynab
    ("سارة", "sArp"),   // Sara
    ("ليلى", "lylY"),   // Layla (alef maqsura → 'Y')
    ("خديجة", "xdyjp"), // Khadija
    ("نور", "nwr"),     // Nur
    // -----------------------------------------------------------------
    // Function words (common in Arabic corpora).
    // -----------------------------------------------------------------
    ("في", "fy"),   // in
    ("من", "mn"),   // from
    ("على", "ElY"), // on (with terminal alef maqsura)
    ("إلى", "<lY"), // to (hamza below alef + alef maqsura)
    ("هذا", "h*A"), // this (thal encodes to '*')
    // -----------------------------------------------------------------
    // Diacritics — fully vocalized names.
    // -----------------------------------------------------------------
    ("مُحَمَّد", "muHama~d"), // Muhammad, fully voweled. Note the fatha/shadda
    // ordering: Unicode canonical combining class puts fatha (ccc=30) before
    // shadda (ccc=33), so the NFC-normalized source has fatha first.
    ("عَلِيّ", "Ealiy~"),      // Ali, fully voweled with terminal shadda
    ("قُرآن", "qur|n"),      // Qur'an (madda alef → '|')
    ("مَدْرَسَة", "madorasap"), // school, fully voweled with sukun and teh marbuta
];

#[test]
fn buckwalter_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = Buckwalter.encode(input);
        if got != expected {
            failures.push(alloc::format!(
                "  Buckwalter({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} Buckwalter reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // The task spec asks for "30+ pairs". Verify we're above that.
    assert!(
        PAIRS.len() >= 30,
        "reference pair count {} is below the 30-pair floor",
        PAIRS.len()
    );
}

/// Round-trip test: `Buckwalter.inverse(Buckwalter.encode(x)) == x`
/// for every reference pair.
#[test]
fn every_reference_pair_round_trips() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, _expected) in PAIRS {
        let encoded = Buckwalter.encode(input);
        let back = Buckwalter.inverse(&encoded);
        if back != input {
            failures.push(alloc::format!(
                "  inverse(encode({input:?})) = {back:?} (expected {input:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} reference pair(s) failed round-trip:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}
