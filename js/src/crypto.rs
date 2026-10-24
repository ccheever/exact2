//! `crypto.subtle` on the Hermes executor (LLP 1069.005): the byte work
//! behind the prelude's `SubtleCrypto`, through `__exact_bytes`. The prelude
//! owns the WebCrypto shapes (algorithm names, errors, promises); this owns
//! the arithmetic.
//!
//! @ref LLP 1069.005 D1 — digests are pure: no read, no mark, any time.

use std::ffi::{c_char, c_void, CStr};

use exact_data::crypto::Sha;

use crate::{c_string, HostState};

/// A SHA-2 digest of `data`, lowercase hex: the name is the prelude's,
/// already normalized (`SHA-256`, `SHA-384`, `SHA-512`). The Rust
/// sources' own (D5), so both languages hash with one implementation.
fn digest(name: &str, data: &[u8]) -> Result<String, String> {
    let sha = match name {
        "SHA-256" => Sha::Sha256,
        "SHA-384" => Sha::Sha384,
        "SHA-512" => Sha::Sha512,
        other => return Err(format!("NotSupportedError: {other}")),
    };
    Ok(hex(&exact_data::crypto::digest(sha, data)))
}

/// Host op 12: `authCallback()` (LLP 1069.006 D2), this native build's
/// callback, a device fact, so bake compiles no answer that asks.
/// (`placement` is the web worker realm's question alone.)
pub(crate) fn auth_callback(state: &mut HostState, what: &str) -> Option<String> {
    let store = state.store.filter(|_| what == "callback")?;
    // SAFETY: the store the seam handed this call outlives the call.
    unsafe { (*store).observe_external_read() };
    state.auth_callback.clone()
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

/// ECDSA P-256 behind the prelude's `CryptoKey` (LLP 1069.005 D1b), by
/// handle into `state.keys`: 2 generate(extractable "1"/"0") →
/// `{"private":h,"public":h}`; 3 sign(handle, data) → `r‖s` as hex; 4
/// import(`{"jwk":…,"extractable":…}`) → a handle; 5 export(handle) → the
/// JWK; 6 keep(`{"name","private","public"}`) writes the pair's JWK under the
/// secret; 7 kept(name) → a pair of handles, or "" when nothing is kept.
/// Generating and signing mark the answer's read (the store's), as op 8
/// does; with no store (an in-process query) nothing is marked.
fn ec(state: &mut HostState, op: u32, a: &str, data: &[u8]) -> Result<String, String> {
    use exact_data::crypto::{self as c, EcKey, EcKeyPair, Jwk};
    use exact_runner::{DataError, Store};
    let message = |e: DataError| match e {
        DataError::BadArguments(m)
        | DataError::Unavailable(m)
        | DataError::UnknownSource(m)
        | DataError::Interface(m)
        | DataError::DeferredAtBake(m)
        | DataError::Failed(_, m, _) => m,
    };
    let scratch = Store::new("", []);
    let store_ptr = state.store;
    // SAFETY: the store the seam handed this call outlives the call.
    let store: &Store = match store_ptr {
        Some(s) => unsafe { &*s },
        None => &scratch,
    };
    let key = |state: &HostState, text: &str| -> Result<EcKey, String> {
        text.parse::<usize>()
            .ok()
            .and_then(|i| state.keys.get(i).cloned())
            .ok_or_else(|| "InvalidAccessError: not a live CryptoKey".to_string())
    };
    let push = |state: &mut HostState, k: EcKey| {
        state.keys.push(k);
        state.keys.len() - 1
    };
    let pair = |state: &mut HostState, p: EcKeyPair| {
        let private = push(state, p.private);
        let public = push(state, p.public);
        format!("{{\"private\":{private},\"public\":{public}}}")
    };
    match op {
        2 => {
            let p = c::generate_p256(store, a == "1").map_err(message)?;
            Ok(pair(state, p))
        }
        3 => Ok(hex(
            &c::sign_es256(store, &key(state, a)?, data).map_err(message)?
        )),
        4 => {
            let v: serde_json::Value = serde_json::from_str(a).map_err(|e| e.to_string())?;
            let jwk = Jwk::from_json(&v["jwk"].to_string()).map_err(message)?;
            let k = EcKey::from_jwk(&jwk, v["extractable"].as_bool().unwrap_or(false))
                .map_err(message)?;
            Ok(push(state, k).to_string())
        }
        5 => Ok(key(state, a)?.to_jwk().map_err(message)?.to_json()),
        6 => {
            let v: serde_json::Value = serde_json::from_str(a).map_err(|e| e.to_string())?;
            let handle = |k: &str| v[k].as_u64().map(|n| n.to_string()).unwrap_or_default();
            let p = EcKeyPair {
                private: key(state, &handle("private"))?,
                public: key(state, &handle("public"))?,
            };
            let name = v["name"].as_str().unwrap_or_default();
            match store_ptr {
                // SAFETY: as above; the one writer during this call.
                Some(s) => c::keep_key(unsafe { &mut *s }, name, &p).map_err(message)?,
                None => return Err("store.keepKey: no store at bake".into()),
            }
            Ok(String::new())
        }
        7 => match c::kept_key(store, a).map_err(message)? {
            Some(p) => Ok(pair(state, p)),
            None => Ok(String::new()),
        },
        _ => Err(format!("__exact_bytes: no op {op}")),
    }
}

/// The bytes door (`__exact_bytes(op, a, arrayBuffer)`). Ops: 1 digest(name);
/// 2–7 ECDSA P-256 and kept keys ([`ec`]); 8 LLP 1133.003 §9's stub decode,
/// under the agent only, which copies the bytes at `a`'s offset and length
/// and drops them; 9 the
/// binary message being mapped, written into the buffer, which the prelude
/// made its size.
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
    let state = &mut *(ctx as *mut HostState);
    let a = CStr::from_ptr(a).to_string_lossy();
    // Op 9 writes: no shared slice of the buffer may live while it does.
    // SAFETY: the shim hands over JSI's `ArrayBuffer::data`, the prelude's
    // own buffer, writable and `len` long for this call.
    if op == 9 {
        let (status, text) = if len == state.binary.len() {
            if len > 0 {
                std::ptr::copy_nonoverlapping(state.binary.as_ptr(), bytes as *mut u8, len);
            }
            (0, String::new())
        } else {
            (
                1,
                "message: a buffer that is not the message's size".to_string(),
            )
        };
        *out = c_string(&text);
        return status;
    }
    let data: &[u8] = if len == 0 {
        &[]
    } else {
        std::slice::from_raw_parts(bytes, len)
    };
    let reply = match op {
        1 => digest(&a, data),
        2..=7 => ec(state, op, &a, data),
        8 if state.agent.is_none() => Err("decode: the gate's stub decode is the agent's".into()),
        8 => {
            let mut range = a.split(' ').map(str::parse::<usize>);
            match (range.next(), range.next()) {
                (Some(Ok(at)), Some(Ok(n)))
                    if at.checked_add(n).is_some_and(|end| end <= data.len()) =>
                {
                    let chunk = data[at..at + n].to_vec();
                    exact_runner::stream_timing::decoded(chunk.len());
                    drop(chunk);
                    Ok(String::new())
                }
                _ => Err("decode: a range outside the chunk's buffer".into()),
            }
        }
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

    /// `(status, text)` of one bytes-door call on `state` with `buffer`.
    fn call(state: &mut HostState, op: u32, a: &str, buffer: &mut [u8]) -> (i32, String) {
        let a = std::ffi::CString::new(a).unwrap();
        let mut out = std::ptr::null_mut();
        // SAFETY: `state` and `buffer` outlive the call; `out` is the
        // malloc'd string the door returns, freed below.
        unsafe {
            let ctx = state as *mut HostState as *mut c_void;
            let status = bytes_door(
                ctx,
                op,
                a.as_ptr(),
                buffer.as_mut_ptr(),
                buffer.len(),
                &mut out,
            );
            let text = CStr::from_ptr(out).to_string_lossy().into_owned();
            libc_free(out as *mut c_void);
            (status, text)
        }
    }
    extern "C" {
        #[link_name = "free"]
        fn libc_free(p: *mut c_void);
    }

    #[test]
    fn a_message_fills_a_buffer_of_its_size_and_refuses_any_other() {
        let mut state = HostState {
            binary: vec![1, 2, 3],
            ..HostState::default()
        };
        let mut wrong = [0u8; 2];
        let (status, text) = call(&mut state, 9, "", &mut wrong);
        assert_eq!(
            (status, text.as_str(), wrong),
            (
                1,
                "message: a buffer that is not the message's size",
                [0, 0]
            )
        );
        let mut right = [0u8; 3];
        assert_eq!(call(&mut state, 9, "", &mut right).0, 0);
        assert_eq!(right, [1, 2, 3]);
    }

    #[test]
    fn the_stub_decode_is_the_agents_alone() {
        let mut state = HostState::default();
        let mut chunk = [0u8; 4];
        let (status, text) = call(&mut state, 8, "0 4", &mut chunk);
        assert_eq!(
            (status, text.as_str()),
            (1, "decode: the gate's stub decode is the agent's")
        );
        state.agent = Some(exact_data::crypto::AgentStream::new(1, "typescript"));
        assert_eq!(call(&mut state, 8, "0 4", &mut chunk).0, 0);
        assert_eq!(
            call(&mut state, 8, "2 4", &mut chunk).0,
            1,
            "a range past the chunk"
        );
    }
}
