/// A GPU product's digest: its exact bytes, except that a signed 64-bit
/// Mach-O's code signature is left out, with the three sizes that follow it
/// (the signature command's `datasize`, `__LINKEDIT`'s `vmsize` and
/// `filesize`) read as zero. A re-signer (AppDrop signs every .ipa again
/// for each recipient) keeps the digest; any change to the code it signs
/// does not. Anything else is hashed whole. GpuModule.swift's
/// `codeDigest` and host/apple/assets.mjs's `codeDigest` are this one.
pub fn code_digest(bytes: &[u8]) -> String {
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
    crate::sha256_hex(&macho().unwrap_or_else(|| bytes.to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_gpu_digest_survives_re_signing_but_not_a_code_change() {
        // A 64-bit Mach-O: __LINKEDIT, then the signature command, the code,
        // and the signature at offset 160.
        let signed = |signature: &[u8], code: u8| {
            let mut b = vec![0u8; 160];
            b[..4].copy_from_slice(&0xfeed_facfu32.to_le_bytes());
            b[16..20].copy_from_slice(&2u32.to_le_bytes());
            b[32..36].copy_from_slice(&0x19u32.to_le_bytes());
            b[36..40].copy_from_slice(&72u32.to_le_bytes());
            b[40..50].copy_from_slice(b"__LINKEDIT");
            b[64..72].copy_from_slice(&(signature.len() as u64).to_le_bytes());
            b[80..88].copy_from_slice(&(signature.len() as u64 + 8).to_le_bytes());
            b[104..108].copy_from_slice(&0x1du32.to_le_bytes());
            b[108..112].copy_from_slice(&16u32.to_le_bytes());
            b[112..116].copy_from_slice(&160u32.to_le_bytes());
            b[116..120].copy_from_slice(&(signature.len() as u32).to_le_bytes());
            b[150] = code;
            b.extend_from_slice(signature);
            b
        };
        let ours = code_digest(&signed(b"ad-hoc", 1));
        assert_eq!(
            ours,
            code_digest(&signed(b"a recipient's longer signature", 1))
        );
        assert_ne!(ours, code_digest(&signed(b"ad-hoc", 2)));
        assert_ne!(ours, crate::sha256_hex(&signed(b"ad-hoc", 1)));
        let original = signed(b"ad-hoc", 1);
        let resigned = signed(b"a recipient's longer signature", 1);
        assert_ne!(crate::sha256_hex(&original), crate::sha256_hex(&resigned));
        for bytes in [b"not Mach-O".as_slice(), &original[..100]] {
            assert_eq!(code_digest(bytes), crate::sha256_hex(bytes));
        }
    }
}
