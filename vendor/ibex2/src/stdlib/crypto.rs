//! Secure randomness, available without a host or JavaScript engine.
//!
//! @ref LLP 0059.000#39-crypto--ambient-partially-gated — OS entropy, ungated

use crate::boundary::HostError;

/// WebCrypto's per-call byte ceiling (not an element count).
pub const MAX_RANDOM_BYTES: usize = 65_536;

pub(crate) fn check_length(length: usize) -> Result<(), HostError> {
    if length > MAX_RANDOM_BYTES {
        Err(HostError::Failed(
            "QuotaExceededError: getRandomValues accepts at most 65536 bytes".into(),
        ))
    } else {
        Ok(())
    }
}

/// Fill the caller's bytes using the operating system's cryptographic RNG.
///
/// Empty slices succeed. An oversized slice is refused without mutation.
/// Entropy failure clears the destination and returns an error; it never
/// falls back to a seed, clock, or noncryptographic generator.
pub fn get_random_values(destination: &mut [u8]) -> Result<(), HostError> {
    check_length(destination.len())?;
    getrandom::getrandom(destination).map_err(|error| {
        destination.fill(0);
        HostError::Failed(format!(
            "OperationError: OS randomness unavailable: {error}"
        ))
    })
}

/// A lowercase UUID v4 with 122 random bits, as `crypto.randomUUID()` returns.
pub fn random_uuid() -> Result<String, HostError> {
    let mut bytes = [0; 16];
    get_random_values(&mut bytes)?;
    Ok(format_uuid(bytes))
}

fn format_uuid(mut bytes: [u8; 16]) -> String {
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let mut result = String::with_capacity(36);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for (i, byte) in bytes.into_iter().enumerate() {
        if matches!(i, 4 | 6 | 8 | 10) {
            result.push('-');
        }
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 0xf) as usize] as char);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_sets_only_version_and_variant_bits() {
        assert_eq!(format_uuid([0; 16]), "00000000-0000-4000-8000-000000000000");
        assert_eq!(
            format_uuid([255; 16]),
            "ffffffff-ffff-4fff-bfff-ffffffffffff"
        );
        assert_eq!(
            format_uuid(std::array::from_fn(|i| i as u8)),
            "00010203-0405-4607-8809-0a0b0c0d0e0f"
        );
    }
}
