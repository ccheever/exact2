//! `atob` / `btoa` — pure, ungated (LLP 0059.000 §3.10).
//!
//! Base64 in Rust, throwing on invalid input per spec. The subtlety worth
//! getting right: these are the *binary string* functions, not general base64.
//! `btoa` accepts only code points U+0000..U+00FF and must reject anything
//! else, which is what makes `btoa('€')` a `InvalidCharacterError` in a browser
//! rather than silently encoding UTF-8.

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;

use crate::boundary::HostError;

/// `btoa`: a binary string in, base64 out.
pub fn btoa(input: &str) -> Result<String, HostError> {
    let mut bytes = Vec::with_capacity(input.len());
    for ch in input.chars() {
        let code = ch as u32;
        if code > 0xFF {
            return Err(HostError::Failed(
                "InvalidCharacterError: btoa argument contains characters outside Latin-1".into(),
            ));
        }
        bytes.push(code as u8);
    }
    Ok(STANDARD.encode(bytes))
}

/// `atob`: base64 in, a binary string out.
///
/// Per the WHATWG forgiving-base64 rules, ASCII whitespace is stripped before
/// decoding and everything else must be valid.
pub fn atob(input: &str) -> Result<String, HostError> {
    let stripped: String = input
        .chars()
        .filter(|c| !matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0c'))
        .collect();
    let bytes = STANDARD.decode(stripped.as_bytes()).map_err(|_| {
        HostError::Failed("InvalidCharacterError: atob argument is not valid base64".into())
    })?;
    // Each byte becomes one code point, which is what makes this a binary
    // string rather than a UTF-8 decode.
    Ok(bytes.iter().map(|b| *b as char).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_ascii() {
        assert_eq!(btoa("hello").unwrap(), "aGVsbG8=");
        assert_eq!(atob("aGVsbG8=").unwrap(), "hello");
    }

    #[test]
    fn btoa_rejects_characters_above_latin1() {
        // The canonical browser behaviour: btoa('€') throws.
        assert!(btoa("€").is_err());
        assert!(btoa("😀").is_err());
        // ...but Latin-1 is fine, and encodes as one byte, not two.
        assert_eq!(btoa("ÿ").unwrap(), "/w==");
    }

    #[test]
    fn atob_round_trips_high_bytes_as_single_code_points() {
        let encoded = btoa("ÿ").unwrap();
        assert_eq!(atob(&encoded).unwrap(), "ÿ");
        assert_eq!(atob(&encoded).unwrap().chars().count(), 1);
    }

    #[test]
    fn atob_rejects_invalid_input() {
        assert!(atob("not!base64").is_err());
        assert!(atob("a").is_err());
    }

    #[test]
    fn atob_ignores_ascii_whitespace() {
        assert_eq!(atob("aGVs bG8=").unwrap(), "hello");
        assert_eq!(atob("aGVsbG8=\n").unwrap(), "hello");
    }

    #[test]
    fn empty_input_is_not_an_error() {
        assert_eq!(btoa("").unwrap(), "");
        assert_eq!(atob("").unwrap(), "");
    }
}
