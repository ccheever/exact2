//! `TextEncoder` / `TextDecoder` — pure, ungated (LLP 0059.000 §3.3).
//!
//! **UTF-8 only in v1.** The measured uses are all UTF-8; `latin1`, UTF-16, and
//! the legacy label set are out. `fatal` and `ignoreBOM` are honored.

use crate::boundary::HostError;

/// `TextEncoder.encode`. Always UTF-8, so this is the string's own bytes.
pub fn encode(input: &str) -> Vec<u8> {
    input.as_bytes().to_vec()
}

/// `TextEncoder.encodeInto`: write into a caller-supplied buffer.
///
/// Returns `(read, written)` as the spec does — code units read from the source
/// and bytes written to the destination. This writes whole characters only, so
/// a truncated buffer never receives a partial code point.
///
/// This is the operation LLP 0059.000 §3.3 names as the reason `ArrayBuffer`
/// handles are in the boundary contract at all: the buffer is the caller's, and
/// Rust writes into it rather than allocating and copying back.
pub fn encode_into(input: &str, destination: &mut [u8]) -> (usize, usize) {
    // Take the longest prefix that fits, backing up to a character boundary so
    // a partial code point is never written, then copy it in one go.
    //
    // The obvious implementation — walk chars() and encode_utf8 each one — is
    // correct but roughly 3x slower than a memcpy on a large input, which made
    // encodeInto slower than the encode it exists to be faster than. Measured
    // before believing it.
    let mut end = input.len().min(destination.len());
    while end > 0 && !input.is_char_boundary(end) {
        end -= 1;
    }
    let prefix = &input[..end];
    destination[..end].copy_from_slice(prefix.as_bytes());

    // `read` counts UTF-16 units, because that is what a JavaScript caller
    // counts. ASCII is one unit per byte, and checking for it is vectorized —
    // so the per-character walk only happens when the text is not ASCII.
    let read = if prefix.is_ascii() {
        end
    } else {
        prefix.chars().map(char::len_utf16).sum()
    };
    (read, end)
}

/// How a decoder handles bytes that are not valid UTF-8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnInvalid {
    /// `fatal: false` — the default. Invalid sequences become U+FFFD.
    Replace,
    /// `fatal: true` — invalid sequences throw.
    Throw,
}

/// `TextDecoder.decode`.
pub fn decode(bytes: &[u8], on_invalid: OnInvalid, ignore_bom: bool) -> Result<String, HostError> {
    // The BOM is stripped unless ignoreBOM was requested. Note the web's naming
    // is backwards from the intuition: ignoreBOM:true means "treat it as
    // ordinary data and keep it", not "skip it".
    let bytes = if !ignore_bom && bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &bytes[3..]
    } else {
        bytes
    };

    match on_invalid {
        OnInvalid::Replace => Ok(String::from_utf8_lossy(bytes).into_owned()),
        OnInvalid::Throw => String::from_utf8(bytes.to_vec()).map_err(|_| {
            HostError::Failed("TypeError: the encoded data was not valid UTF-8".into())
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_utf8() {
        assert_eq!(encode("hi"), b"hi");
        assert_eq!(encode("€"), vec![0xE2, 0x82, 0xAC]);
    }

    #[test]
    fn decodes_utf8() {
        assert_eq!(decode(b"hi", OnInvalid::Replace, false).unwrap(), "hi");
        assert_eq!(
            decode(&[0xE2, 0x82, 0xAC], OnInvalid::Replace, false).unwrap(),
            "€"
        );
    }

    #[test]
    fn invalid_bytes_replace_by_default_and_throw_when_fatal() {
        let invalid = [0x66, 0xFF, 0x6F];
        assert_eq!(
            decode(&invalid, OnInvalid::Replace, false).unwrap(),
            "f\u{FFFD}o"
        );
        assert!(decode(&invalid, OnInvalid::Throw, false).is_err());
    }

    #[test]
    fn the_bom_is_stripped_unless_ignore_bom_is_set() {
        let with_bom = [0xEF, 0xBB, 0xBF, b'h', b'i'];
        assert_eq!(decode(&with_bom, OnInvalid::Replace, false).unwrap(), "hi");
        assert_eq!(
            decode(&with_bom, OnInvalid::Replace, true).unwrap(),
            "\u{FEFF}hi"
        );
    }

    #[test]
    fn encode_into_writes_whole_characters_only() {
        // Four bytes of room, but '€' needs three and the next char would
        // overflow — so we stop rather than writing half a code point.
        let mut buffer = [0u8; 4];
        let (read, written) = encode_into("€€", &mut buffer);
        assert_eq!(written, 3);
        assert_eq!(read, 1);
        assert_eq!(&buffer[..3], &[0xE2, 0x82, 0xAC]);
        assert_eq!(buffer[3], 0, "no partial code point was written");
    }

    #[test]
    fn encode_into_reports_utf16_units_read() {
        // An astral character is one char in Rust but two UTF-16 units, which
        // is what a JavaScript caller counts.
        let mut buffer = [0u8; 8];
        let (read, written) = encode_into("😀", &mut buffer);
        assert_eq!(written, 4);
        assert_eq!(read, 2);
    }

    #[test]
    fn encode_into_a_zero_length_buffer_writes_nothing() {
        let mut buffer = [0u8; 0];
        assert_eq!(encode_into("hi", &mut buffer), (0, 0));
    }
}
