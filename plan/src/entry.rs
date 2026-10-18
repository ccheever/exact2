//! The plan header and envelope use the same portable entry identifier.

/// An entry kind that is safe as a bundle filename on every host.
pub fn entry_identifier(value: &str) -> bool {
    value
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphabetic)
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
}
