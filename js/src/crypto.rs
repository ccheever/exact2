//! `crypto.subtle` on the Hermes executor (LLP 1069.005): the byte work
//! behind the prelude's `SubtleCrypto`, through `__exact_bytes`. The prelude
//! owns the WebCrypto shapes (algorithm names, errors, promises); this owns
//! the arithmetic.
//!
//! @ref LLP 1069.005 D1 — digests are pure: no read, no mark, any time.

use std::ffi::{c_char, c_void, CStr};

use sha2::Digest;

use crate::{c_string, HostState};

/// A SHA-2 digest of `data`, lowercase hex: the name is the prelude's,
/// already normalized (`SHA-256`, `SHA-384`, `SHA-512`).
fn digest(name: &str, data: &[u8]) -> Result<String, String> {
    let bytes = match name {
        "SHA-256" => sha2::Sha256::digest(data).to_vec(),
        "SHA-384" => sha2::Sha384::digest(data).to_vec(),
        "SHA-512" => sha2::Sha512::digest(data).to_vec(),
        other => return Err(format!("NotSupportedError: {other}")),
    };
    Ok(hex(&bytes))
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(HEX[(byte >> 4) as usize] as char);
        text.push(HEX[(byte & 0xf) as usize] as char);
    }
    text
}

/// The bytes door (`__exact_bytes(op, a, arrayBuffer)`). Ops: 1 digest(name).
///
/// # Safety
/// Called by the shim on the engine's thread with `ctx` the `HostState` the
/// engine was created with, `a` NUL-terminated, and `bytes` valid for `len`.
pub(crate) unsafe extern "C" fn bytes_door(
    ctx: *mut c_void,
    op: u32,
    a: *const c_char,
    bytes: *const u8,
    len: usize,
    out: *mut *mut c_char,
) -> i32 {
    let _state = &mut *(ctx as *mut HostState);
    let a = CStr::from_ptr(a).to_string_lossy();
    let data: &[u8] = if len == 0 {
        &[]
    } else {
        std::slice::from_raw_parts(bytes, len)
    };
    let reply = match op {
        1 => digest(&a, data),
        other => Err(format!("__exact_bytes: no op {other}")),
    };
    let (status, text) = match reply {
        Ok(text) => (0, text),
        Err(text) => (1, text),
    };
    *out = c_string(&text);
    status
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digests_are_the_sha2_vectors() {
        assert_eq!(
            digest("SHA-256", b"abc").unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(digest("SHA-1", b"")
            .unwrap_err()
            .starts_with("NotSupportedError"));
    }
}
