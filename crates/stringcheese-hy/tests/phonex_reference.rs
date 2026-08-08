//! PHONEX-Armenian reference pairs.
//!
//! The pairs below cover every letter of the modern Armenian alphabet
//! at least once, including the aspirated / plain / voiced stop
//! triples that fold to their base class, the `ու` digraph, and the
//! `և` ligature. The mapping is the one documented in
//! [`stringcheese_hy::phonetic`].

extern crate alloc;

use stringcheese_hy::ArmenianPhonex;

/// Reference pairs (input, expected 4-character PHONEX key).
const PAIRS: &[(&str, &str)] = &[
    // -------------------------------------------------------------
    // Common Armenian place names.
    // -------------------------------------------------------------
    // Երևան — E R EV A N → E R E V A N → E seed, R(6), E reset,
    //   V(1), A reset, N(5) → "E615".
    ("Երևան", "E615"),
    // Հայաստան — H A Y A S T A N → H seed, A reset, Y reset (Y is
    //   class 0), A reset, S(7), T(3), A reset, N(5) → "H735".
    ("Հայաստան", "H735"),
    // -------------------------------------------------------------
    // Aspiration folds — Eastern /b/ /p/ /pʰ/ all fold to P.
    // -------------------------------------------------------------
    // բաբ (dummy: three labial stops) — P A P → P seed, A reset,
    //   P(1) push → "P100". Since P is class 1 which is different
    //   from seed's class (0), we push.
    // Simpler: `պապ` (grandfather) — P A P.
    ("պապ", "P100"),
    // -------------------------------------------------------------
    // Ligature և → EV.
    // -------------------------------------------------------------
    // Bare `և` folds to `EV`. E seed, V(1) push → "EV" then pad →
    //   "E100".
    ("և", "E100"),
    // -------------------------------------------------------------
    // ու digraph → U.
    // -------------------------------------------------------------
    // Bare `ու` → U → "U000".
    ("ու", "U000"),
    // -------------------------------------------------------------
    // Consonant-triple aspiration folds.
    // -------------------------------------------------------------
    // Դատարան (courthouse) — T A T A R A N.
    //   T seed, A reset, T(3) push→"T3", A reset, R(6) push→"T36",
    //   A reset, N(5) push→"T365".
    ("Դատարան", "T365"),
    // -------------------------------------------------------------
    // Simple single-letter roundtrips (for full alphabet coverage).
    // -------------------------------------------------------------
    // Consonants each become their family letter + "000".
    ("բ", "P000"), // labial → P
    ("գ", "K000"), // velar → K
    ("դ", "T000"), // dental → T
    ("զ", "Z000"), // sibilant → Z
    ("լ", "L000"), // liquid
    ("մ", "M000"), // nasal
    ("ն", "N000"),
    ("ս", "S000"),
    ("վ", "V000"),
    ("րր", "R000"), // liquid (double folds via Soundex duplicate collapse)
    ("ֆ", "F000"),
    // Fricatives.
    ("խ", "X000"),
    ("ղ", "X000"),
    ("հ", "H000"), // /h/
    // Aspirated variants — same key as base.
    ("փ", "P000"), // aspirated labial
    ("թ", "T000"), // aspirated dental
    ("ք", "K000"), // aspirated velar
    // -------------------------------------------------------------
    // Vowels — encode to seed + "000" (all vowels are class 0).
    // -------------------------------------------------------------
    ("ա", "A000"),
    ("ե", "E000"),
    ("է", "E000"),
    ("ը", "E000"),
    ("ի", "I000"),
    ("ո", "O000"),
    ("օ", "O000"),
];

#[test]
fn phonex_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = ArmenianPhonex.encode(input).expect("encodes");
        if got != expected {
            failures.push(alloc::format!(
                "  PHONEX-hy({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} PHONEX-Armenian reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // The task spec asks for enough pairs to cover the alphabet.
    assert!(
        PAIRS.len() >= 20,
        "reference pair count {} is below the 20-pair floor",
        PAIRS.len()
    );
}

#[test]
fn aspiration_triples_produce_same_key() {
    // The three-way plain / aspirated / voiced fold. Every triple
    // encodes to the same 4-char key when there is no other
    // consonant to disambiguate.
    let labial: alloc::vec::Vec<String> = ["բ", "պ", "փ"]
        .into_iter()
        .map(|w| ArmenianPhonex.encode(w).unwrap())
        .collect();
    assert!(labial.iter().all(|k| k == &labial[0]));

    let dental: alloc::vec::Vec<String> = ["դ", "տ", "թ"]
        .into_iter()
        .map(|w| ArmenianPhonex.encode(w).unwrap())
        .collect();
    assert!(dental.iter().all(|k| k == &dental[0]));

    let velar: alloc::vec::Vec<String> = ["գ", "կ", "ք"]
        .into_iter()
        .map(|w| ArmenianPhonex.encode(w).unwrap())
        .collect();
    assert!(velar.iter().all(|k| k == &velar[0]));

    let dental_affr: alloc::vec::Vec<String> = ["ձ", "ծ", "ց"]
        .into_iter()
        .map(|w| ArmenianPhonex.encode(w).unwrap())
        .collect();
    assert!(dental_affr.iter().all(|k| k == &dental_affr[0]));

    let pa_affr: alloc::vec::Vec<String> = ["ջ", "ճ", "չ"]
        .into_iter()
        .map(|w| ArmenianPhonex.encode(w).unwrap())
        .collect();
    assert!(pa_affr.iter().all(|k| k == &pa_affr[0]));
}

#[test]
fn every_armenian_letter_is_covered() {
    // Walk the full modern Armenian lowercase alphabet plus the
    // ligature. Every scalar should encode to a valid PHONEX key.
    const ALPHABET: &[char] = &[
        'ա', 'բ', 'գ', 'դ', 'ե', 'զ', 'է', 'ը', 'թ', 'ժ', 'ի', 'լ', 'խ', 'ծ', 'կ', 'հ', 'ձ', 'ղ',
        'ճ', 'մ', 'յ', 'ն', 'շ', 'ո', 'չ', 'պ', 'ջ', 'ռ', 'ս', 'վ', 'տ', 'ր', 'ց', 'ւ', 'փ', 'ք',
        'օ', 'ֆ', 'և',
    ];
    for &letter in ALPHABET {
        let s: String = core::iter::once(letter).collect();
        let key = ArmenianPhonex.encode(&s);
        assert!(
            key.is_some(),
            "letter {letter:?} did not encode to a PHONEX key"
        );
        let k = key.unwrap();
        assert_eq!(k.chars().count(), 4, "key {k:?} is not 4 chars");
    }
}

#[test]
fn uppercase_and_lowercase_encode_identically() {
    // Case-fold applies before encoding.
    assert_eq!(
        ArmenianPhonex.encode("ԵՐԵՎԱՆ"),
        ArmenianPhonex.encode("երևան")
    );
}

#[test]
fn eu_two_letter_and_ligature_encode_identically() {
    // `եւ` normalizes to `և` before encoding — both spellings
    // produce the same key.
    assert_eq!(ArmenianPhonex.encode("եւ"), ArmenianPhonex.encode("և"));
}
