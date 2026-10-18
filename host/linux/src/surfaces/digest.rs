//! A GPU module's identity as the bake records it (`bake/src/receipt.rs`,
//! `code_digest`): its SHA-256, except that a signed 64-bit Mach-O's code
//! signature is left out, with the three sizes that follow it (the signature
//! command's `datasize`, `__LINKEDIT`'s `vmsize` and `filesize`) read as zero.
//! On macOS the linker signs every dylib, so a release module's whole-file hash
//! is never the bake's. Anything else is hashed whole.
//! GpuModule.swift's `codeDigest` and host/apple/assets.mjs's `codeDigest` are
//! the same rule.
use sha2::{Digest, Sha256};

pub(crate) fn code_digest(bytes: &[u8]) -> String {
    let word = |at: usize| {
        bytes
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize)
    };
    let macho = || -> Option<Vec<u8>> {
        if word(0)? != 0xfeed_facf {
            return None;
        }
        let (mut at, mut signature, mut zero) = (32, None, Vec::new());
        for _ in 0..word(16)? {
            let (cmd, size) = (word(at)?, word(at + 4)?);
            if cmd == 0x1d {
                signature = Some(word(at + 8)?);
                zero.push((at + 12, 4));
            } else if cmd == 0x19 && bytes.get(at + 8..at + 24)? == b"__LINKEDIT\0\0\0\0\0\0" {
                zero.extend([(at + 32, 8), (at + 48, 8)]);
            }
            at += size.max(8);
        }
        let mut code = bytes.get(..signature?)?.to_vec();
        for (at, len) in zero {
            code.get_mut(at..at + len)?.fill(0);
        }
        Some(code)
    };
    match macho() {
        Some(code) => format!("{:x}", Sha256::digest(&code)),
        None => format!("{:x}", Sha256::digest(bytes)),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_signature_is_left_out_and_anything_else_is_hashed_whole() {
        use sha2::{Digest, Sha256};
        assert_eq!(
            super::code_digest(b"ELF"),
            format!("{:x}", Sha256::digest(b"ELF"))
        );
        // A Mach-O whose only load command is a 16-byte LC_CODE_SIGNATURE at 48.
        let mut image = vec![0u8; 64];
        image[..4].copy_from_slice(&0xfeed_facfu32.to_le_bytes());
        image[16..20].copy_from_slice(&1u32.to_le_bytes());
        image[32..36].copy_from_slice(&0x1du32.to_le_bytes());
        image[36..40].copy_from_slice(&16u32.to_le_bytes());
        image[40..44].copy_from_slice(&48u32.to_le_bytes());
        image[44..48].copy_from_slice(&16u32.to_le_bytes());
        let mut resigned = image.clone();
        resigned[48..].fill(7);
        resigned[44..48].copy_from_slice(&8u32.to_le_bytes());
        assert_eq!(super::code_digest(&image), super::code_digest(&resigned));
        assert_ne!(
            super::code_digest(&image),
            format!("{:x}", Sha256::digest(&image))
        );
    }
}
