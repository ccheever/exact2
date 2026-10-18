//! A baked GPU card's digest: the bake's `code_digest` (`bake/src/receipt.rs`),
//! which leaves a signed 64-bit Mach-O's code signature out and reads the
//! three sizes that follow it as zero. An ELF or a PE file is hashed whole.
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
    format!("{:x}", Sha256::digest(macho().as_deref().unwrap_or(bytes)))
}
