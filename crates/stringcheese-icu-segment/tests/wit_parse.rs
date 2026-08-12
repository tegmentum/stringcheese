//! `wit-parser` gate — asserts the shipped WIT file parses cleanly
//! under the same tooling that consumes it downstream. Mirrors the
//! `stringcheese-icu-datetime` / `-plural` / `-number` crates'
//! sibling test.

#![cfg(feature = "std")]

use std::path::Path;

const WIT_SOURCE: &str =
    include_str!("../../../component/wit/segment/stringcheese-icu-segment.wit");

#[test]
fn wit_file_parses_under_wit_parser() {
    let mut resolve = wit_parser::Resolve::new();
    let pkg = resolve
        .push_str(Path::new("stringcheese-icu-segment.wit"), WIT_SOURCE)
        .expect("component/wit/segment/stringcheese-icu-segment.wit must parse under wit-parser");
    let pkg_name = &resolve.packages[pkg].name;
    assert_eq!(pkg_name.namespace, "tegmentum");
    assert_eq!(pkg_name.name, "i18n-segment");
    assert_eq!(
        pkg_name
            .version
            .as_ref()
            .expect("package must carry a version")
            .to_string(),
        "0.1.0"
    );
}

#[test]
fn wit_file_declares_segment_world() {
    let mut resolve = wit_parser::Resolve::new();
    let _ = resolve
        .push_str(Path::new("stringcheese-icu-segment.wit"), WIT_SOURCE)
        .expect("WIT parses");
    assert!(
        resolve
            .worlds
            .iter()
            .any(|(_, w)| w.name == "segment-world"),
        "WIT must export the `segment-world` world",
    );
}

#[test]
fn wit_file_declares_expected_interfaces() {
    let mut resolve = wit_parser::Resolve::new();
    let _ = resolve
        .push_str(Path::new("stringcheese-icu-segment.wit"), WIT_SOURCE)
        .expect("WIT parses");
    let names: Vec<_> = resolve
        .interfaces
        .iter()
        .filter_map(|(_, i)| i.name.clone())
        .collect();
    for expected in &["types", "segment", "capabilities"] {
        assert!(
            names.iter().any(|n| n == expected),
            "WIT must export the `{expected}` interface; got {names:?}",
        );
    }
}
