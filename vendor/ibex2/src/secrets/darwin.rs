//! The Keychain behind the `SecretStore` trait.
//!
//! See `src/engine/darwin_keychain.mm` for the Objective-C++ half: generic
//! password items, service = the app's identity, account = the name. On iOS
//! the item is `AfterFirstUnlockThisDeviceOnly`; on macOS it goes to the login
//! keychain, whose ACL trusts the app that created it by its code signature —
//! which is why a consumer that rebuilds is signed with a stable identity
//! (LLP 0069 §3; exact2 LLP 1018 D7).
//!
//! @ref LLP 0069#3-the-backends — the Keychain, and why the login keychain on macOS

use std::ffi::{c_char, c_int, c_uchar, c_void, CStr, CString};

use super::{is_valid_name, SecretStore};
use crate::boundary::HostError;

extern "C" {
    /// 0 found (`out_value`/`out_len` set, freed with `ibex2_darwin_free`),
    /// 1 not found, -1 failed (`out_error` set).
    fn ibex2_darwin_keychain_get(
        service: *const c_char,
        name: *const c_char,
        out_value: *mut *mut c_uchar,
        out_len: *mut usize,
        out_error: *mut *mut c_char,
    ) -> c_int;
    /// 0 kept, -1 failed (`out_error` set).
    fn ibex2_darwin_keychain_set(
        service: *const c_char,
        name: *const c_char,
        value: *const c_uchar,
        len: usize,
        out_error: *mut *mut c_char,
    ) -> c_int;
    /// 0 forgotten (or never kept), -1 failed (`out_error` set).
    fn ibex2_darwin_keychain_forget(
        service: *const c_char,
        name: *const c_char,
        out_error: *mut *mut c_char,
    ) -> c_int;
    /// The main bundle's identifier, or null when there is no bundle; freed
    /// with `ibex2_darwin_free`.
    fn ibex2_darwin_bundle_identifier() -> *mut c_char;
    fn ibex2_darwin_free(value: *mut c_void);
}

/// The main bundle's identifier, when the process has a bundle.
pub fn bundle_identifier() -> Option<String> {
    // SAFETY: the shim returns null or a malloc'd NUL-terminated UTF-8 string
    // that is ours to free.
    unsafe {
        let raw = ibex2_darwin_bundle_identifier();
        if raw.is_null() {
            return None;
        }
        let id = CStr::from_ptr(raw).to_string_lossy().into_owned();
        ibex2_darwin_free(raw as *mut c_void);
        Some(id).filter(|s| !s.is_empty())
    }
}

/// Generic-password items in the Keychain under one service.
#[derive(Debug, Clone)]
pub struct KeychainStore {
    service: CString,
}

impl KeychainStore {
    /// Items under `service` — the app's identity by default (`app_identity`).
    pub fn new(service: impl Into<String>) -> Self {
        let service: String = service.into();
        Self {
            service: CString::new(service).unwrap_or_else(|_| CString::new("ibex2").unwrap()),
        }
    }

    fn account(name: &str) -> Result<CString, HostError> {
        if !is_valid_name(name) {
            return Err(HostError::InvalidArgument(format!(
                "`{name}` is not a secret name ([a-z0-9._-]{{1,64}})"
            )));
        }
        CString::new(name).map_err(|_| HostError::InvalidArgument("a NUL in a secret name".into()))
    }
}

/// Take the shim's error message, freeing it.
fn take_error(raw: *mut c_char, what: &str, name: &str) -> HostError {
    if raw.is_null() {
        return HostError::Failed(format!("keychain: {what} {name} failed"));
    }
    // SAFETY: a malloc'd NUL-terminated string from the shim, ours to free.
    let message = unsafe {
        let m = CStr::from_ptr(raw).to_string_lossy().into_owned();
        ibex2_darwin_free(raw as *mut c_void);
        m
    };
    HostError::Failed(format!("keychain: {what} {name}: {message}"))
}

impl SecretStore for KeychainStore {
    fn get(&self, name: &str) -> Result<Option<String>, HostError> {
        let account = Self::account(name)?;
        let mut value: *mut c_uchar = std::ptr::null_mut();
        let mut len: usize = 0;
        let mut error: *mut c_char = std::ptr::null_mut();
        // SAFETY: every pointer is valid for the call; the shim writes the
        // out-parameters it documents and hands back memory we free.
        let rc = unsafe {
            ibex2_darwin_keychain_get(
                self.service.as_ptr(),
                account.as_ptr(),
                &mut value,
                &mut len,
                &mut error,
            )
        };
        match rc {
            0 => {
                // SAFETY: `value` is `len` bytes the shim malloc'd for us.
                let bytes = unsafe { std::slice::from_raw_parts(value, len).to_vec() };
                unsafe { ibex2_darwin_free(value as *mut c_void) };
                String::from_utf8(bytes)
                    .map(Some)
                    .map_err(|_| HostError::Failed(format!("keychain: {name} is not UTF-8")))
            }
            1 => Ok(None),
            _ => Err(take_error(error, "read", name)),
        }
    }

    fn set(&self, name: &str, value: &str) -> Result<(), HostError> {
        let account = Self::account(name)?;
        let mut error: *mut c_char = std::ptr::null_mut();
        // SAFETY: as above; `value` is read for `len` bytes and not kept.
        let rc = unsafe {
            ibex2_darwin_keychain_set(
                self.service.as_ptr(),
                account.as_ptr(),
                value.as_ptr(),
                value.len(),
                &mut error,
            )
        };
        if rc == 0 {
            Ok(())
        } else {
            Err(take_error(error, "keep", name))
        }
    }

    fn forget(&self, name: &str) -> Result<(), HostError> {
        let account = Self::account(name)?;
        let mut error: *mut c_char = std::ptr::null_mut();
        // SAFETY: as above.
        let rc = unsafe {
            ibex2_darwin_keychain_forget(self.service.as_ptr(), account.as_ptr(), &mut error)
        };
        if rc == 0 {
            Ok(())
        } else {
            Err(take_error(error, "forget", name))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// In this process's own keychain service, under a name no other build
    /// ever used, so no other build's item is touched and nothing prompts.
    #[test]
    fn the_keychain_round_trips() {
        let store = KeychainStore::new("ibex2.tests");
        let name = format!(
            "roundtrip.{}.{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        assert_eq!(store.get(&name).unwrap(), None);
        store.set(&name, "one").unwrap();
        assert_eq!(store.get(&name).unwrap().as_deref(), Some("one"));
        store
            .set(&name, r#"{"token":"t0k","username":"ada"}"#)
            .unwrap();
        assert_eq!(
            store.get(&name).unwrap().as_deref(),
            Some(r#"{"token":"t0k","username":"ada"}"#)
        );
        store.forget(&name).unwrap();
        store.forget(&name).unwrap();
        assert_eq!(store.get(&name).unwrap(), None);
        assert!(matches!(
            store.get("../x"),
            Err(HostError::InvalidArgument(_))
        ));
    }
}
