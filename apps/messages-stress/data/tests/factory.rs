#[path = "../../factory.rs"]
mod factory;

use factory::Source;
use std::ffi::OsStr;

#[test]
fn absent_and_explicit_reuse_select_the_same_factory() {
    assert_eq!(Source::parse(None), Ok(Source::Reuse));
    assert_eq!(Source::parse(Some(OsStr::new("reuse"))), Ok(Source::Reuse));
}

#[test]
fn explicit_stateless_control_is_preserved() {
    assert_eq!(
        Source::parse(Some(OsStr::new("stateless"))),
        Ok(Source::Stateless)
    );
    assert_eq!(
        Source::Stateless.entry("off").unwrap(),
        "type AppData = messages_stress_data::MessagesStress;\n#[allow(dead_code)]\nfn app_data() -> AppData { messages_stress_data::MessagesStress }\n"
    );
}

#[test]
fn reuse_generates_the_real_type_and_constructor() {
    assert_eq!(
        Source::Reuse.entry("off").unwrap(),
        "type AppData = messages_stress_data::ReusableMessagesStress;\n#[allow(dead_code)]\nfn app_data() -> AppData { messages_stress_data::ReusableMessagesStress::default() }\n"
    );
}

#[test]
fn invalid_and_empty_values_are_not_defaulted_or_normalized() {
    for value in ["", "Reuse", "STATELESS", " reuse", "reuse ", "reuse\n", "0"] {
        assert!(Source::parse(Some(OsStr::new(value))).is_err(), "{value:?}");
    }
}

#[cfg(unix)]
#[test]
fn non_unicode_is_refused_instead_of_becoming_unset() {
    use std::os::unix::ffi::OsStrExt;
    assert!(Source::parse(Some(OsStr::from_bytes(b"\xff"))).is_err());
}

#[test]
fn build_environment_uses_the_same_strict_parser() {
    let value = std::env::var_os("EXACT_MESSAGES_SOURCE");
    assert_eq!(Source::selected(), Source::parse(value.as_deref()).unwrap());
}
