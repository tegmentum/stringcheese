//! WIT smoke test — the on-disk `stringcheese-icu-number.wit`
//! file must parse cleanly under `wit-parser` and declare the
//! interfaces Phase 3 of the WIT-i18n design commits to.

const WIT_SOURCE: &str = include_str!("../../../component/wit/number/stringcheese-icu-number.wit");

#[test]
fn wit_file_parses_under_wit_parser() {
    let mut resolve = wit_parser::Resolve::new();
    let pkg = resolve
        .push_str(
            std::path::Path::new("stringcheese-icu-number.wit"),
            WIT_SOURCE,
        )
        .expect("component/wit/number/stringcheese-icu-number.wit must parse under wit-parser");
    let pkg_name = &resolve.packages[pkg].name;
    assert_eq!(pkg_name.namespace, "tegmentum");
    assert_eq!(pkg_name.name, "i18n-number");
    assert_eq!(
        pkg_name
            .version
            .as_ref()
            .expect("package must carry a version")
            .to_string(),
        "0.1.0",
    );
}

#[test]
fn wit_file_declares_number_world() {
    let mut resolve = wit_parser::Resolve::new();
    let _ = resolve
        .push_str(
            std::path::Path::new("stringcheese-icu-number.wit"),
            WIT_SOURCE,
        )
        .expect("WIT parses");
    assert!(
        resolve
            .worlds
            .iter()
            .any(|(_, world)| world.name == "number-world"),
        "WIT must export the `number-world` world"
    );
}

#[test]
fn wit_file_declares_expected_interfaces() {
    let mut resolve = wit_parser::Resolve::new();
    let _ = resolve
        .push_str(
            std::path::Path::new("stringcheese-icu-number.wit"),
            WIT_SOURCE,
        )
        .expect("WIT parses");
    let iface_names: Vec<&str> = resolve
        .interfaces
        .iter()
        .filter_map(|(_, iface)| iface.name.as_deref())
        .collect();
    for expected in ["types", "number", "capabilities"] {
        assert!(
            iface_names.contains(&expected),
            "WIT must declare interface `{expected}`; got {iface_names:?}",
        );
    }
}
