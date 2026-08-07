//! PHONEX-Vietnamese reference input/output pairs.
//!
//! A curated set of Vietnamese surnames, common words, and place
//! names that exercise every preprocessing rule (`ng → N`,
//! `nh → N`, `ph → F`, `kh → K`, `tr → T`, `ch → X`, `gi → Y`,
//! `qu → K`, silent `H`) and every diacritic-fold edge case.
//!
//! The expected values are computed against the module-level
//! algorithm documented in [`stringcheese_vi::phonetic`] — see there
//! for the classification table.

extern crate alloc;

use stringcheese_vi::VietnamesePhonex;

/// Reference pairs (input, expected 4-char PHONEX-Vietnamese key).
const PAIRS: &[(&str, &str)] = &[
    // Nguyễn: all-diacritic strip → "NGUYEN", then digraph ng→N →
    //   "NUYEN". N seed last=5. U reset. Y reset (class 0). E reset.
    //   N code=5 push (last was 0) → "N5" pad → "N500".
    ("Nguyễn", "N500"),
    // Trần: "TRAN" → digraph tr→T → "TAN". T seed last=3. A reset.
    //   N code=5 push → "T5" → "T500".
    ("Trần", "T500"),
    // Lê: "LE". L seed last=4. E reset → "L" pad → "L000".
    ("Lê", "L000"),
    // Hà: "HA" → silent H drops → "A". A seed last=0 → "A" pad →
    //   "A000".
    ("Hà", "A000"),
    // Phạm: "PHAM" → digraph ph→F → "FAM". F seed last=1. A reset.
    //   M code=5 push → "F5" → "F500".
    ("Phạm", "F500"),
    // Hoàng: "HOANG" → H silent → "OANG" → digraph ng→N → "OAN".
    //   O seed last=0. A reset. N code=5 push → "O5" → "O500".
    ("Hoàng", "O500"),
    // Vũ: "VU". V seed last=1. U reset → "V" pad → "V000".
    ("Vũ", "V000"),
    // Đặng: "DANG" → digraph ng→N → "DAN". D seed last=3. A reset.
    //   N code=5 push → "D5" → "D500".
    ("Đặng", "D500"),
    // Bùi: "BUI". B seed last=1. U reset. I reset → "B" pad →
    //   "B000".
    ("Bùi", "B000"),
    // Đỗ: "DO". D seed last=3. O reset → "D" pad → "D000".
    ("Đỗ", "D000"),
    // Hồ: "HO" → silent H → "O". O seed last=0 → "O" pad → "O000".
    ("Hồ", "O000"),
    // Ngô: "NGO" → digraph ng→N → "N". N seed last=5 → "N" pad →
    //   "N000".
    ("Ngô", "N000"),
    // Dương: "DUONG" → digraph ng→N → "DUON". D seed last=3. U reset.
    //   O reset. N code=5 push → "D5" → "D500".
    ("Dương", "D500"),
    // Lý: "LY". L seed last=4. Y reset (vowel/glide) → "L" pad →
    //   "L000".
    ("Lý", "L000"),
    // Phở: "PHO" → digraph ph→F → "FO". F seed last=1. O reset → "F"
    //   pad → "F000".
    ("Phở", "F000"),
    // Không: "KHONG" → digraph kh→K → "KONG" → digraph ng→N → "KON".
    //   K seed last=2. O reset. N code=5 push → "K5" → "K500".
    ("Không", "K500"),
    // Khách: "KHACH" → digraph kh→K → "KACH" → digraph ch→X → "KAX".
    //   K seed last=2. A reset. X code=2 push (not dup — reset)
    //   → "K2" pad → "K200".
    ("Khách", "K200"),
    // Chào: "CHAO" → digraph ch→X → "XAO". X seed last=2. A reset.
    //   O reset → "X" pad → "X000".
    ("Chào", "X000"),
    // Quả: "QUA" → digraph qu→K → "KA". K seed last=2. A reset → "K"
    //   pad → "K000".
    ("Quả", "K000"),
    // Giáo: "GIAO" → digraph gi→Y → "YAO". Y seed last=0. A reset.
    //   O reset → "Y" pad → "Y000".
    ("Giáo", "Y000"),
    // Nhà: "NHA" → digraph nh→N → "NA". N seed last=5. A reset → "N"
    //   pad → "N000".
    ("Nhà", "N000"),
    // Trong: "TRONG" → tr→T → "TONG" → ng→N → "TON". T seed last=3.
    //   O reset. N code=5 push → "T5" → "T500".
    ("Trong", "T500"),
];

#[test]
fn phonex_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = VietnamesePhonex.encode(input).unwrap_or_default();
        if got != expected {
            failures.push(alloc::format!(
                "  PHONEX-VI({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} PHONEX-Vietnamese reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // The task spec asks for at least 15 pairs.
    assert!(
        PAIRS.len() >= 15,
        "reference pair count {} is below the 15-pair floor",
        PAIRS.len()
    );
}

#[test]
fn tone_variants_collapse() {
    // All six tone variants of `ban` collapse to the same key.
    let baseline = VietnamesePhonex.encode("ban").unwrap();
    for w in ["bàn", "bán", "bản", "bãn", "bạn"] {
        let k = VietnamesePhonex.encode(w).unwrap();
        assert_eq!(
            k, baseline,
            "PHONEX-VI({w:?}) = {k:?} != PHONEX-VI(\"ban\") = {baseline:?}"
        );
    }
}

#[test]
fn ng_and_nh_digraph_equivalences() {
    // ng → N and nh → N: `nga`, `nha`, `na` all collapse.
    assert_eq!(
        VietnamesePhonex.encode("nga"),
        VietnamesePhonex.encode("na"),
        "NG-N merger failed"
    );
    assert_eq!(
        VietnamesePhonex.encode("nha"),
        VietnamesePhonex.encode("na"),
        "NH-N merger failed"
    );
}

#[test]
fn ph_digraph_equivalence() {
    // `pha` and `fa` both encode identically (ph → F).
    assert_eq!(
        VietnamesePhonex.encode("pha"),
        VietnamesePhonex.encode("fa"),
        "PH-F merger failed"
    );
}

#[test]
fn kh_digraph_equivalence() {
    // `kha` and `ka` both encode identically (kh → K).
    assert_eq!(
        VietnamesePhonex.encode("kha"),
        VietnamesePhonex.encode("ka"),
        "KH-K merger failed"
    );
}

#[test]
fn tr_digraph_equivalence() {
    // `tra` and `ta` both encode identically (tr → T).
    assert_eq!(
        VietnamesePhonex.encode("tra"),
        VietnamesePhonex.encode("ta"),
        "TR-T merger failed"
    );
}

#[test]
fn letter_modifier_folds() {
    // Every letter modifier folds to its ASCII base for phonex.
    // `ăn` and `an` share the vocalic skeleton → same key.
    assert_eq!(VietnamesePhonex.encode("ăn"), VietnamesePhonex.encode("an"));
    // `đâu` and `dau` share.
    assert_eq!(
        VietnamesePhonex.encode("đâu"),
        VietnamesePhonex.encode("dau")
    );
    // `ơn` and `on` share.
    assert_eq!(VietnamesePhonex.encode("ơn"), VietnamesePhonex.encode("on"));
    // `ư` folds to `u`; `ưa` and `ua` share.
    assert_eq!(VietnamesePhonex.encode("ưa"), VietnamesePhonex.encode("ua"));
}
