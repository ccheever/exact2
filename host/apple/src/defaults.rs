//! The app's `UserDefaults` domain, for persisted state (LLP 1116 D5):
//! CoreFoundation's preferences under the app's id, through their C
//! interface on the main thread, never through Swift. Each persisted state
//! is one string value, `exact.state.<name>` → its JSON text, so `defaults
//! read <app id>` shows them. A preference, never the Keychain: a setting
//! is not a secret, and the Keychain outlives the app.
#![allow(unsafe_code)]

use std::ffi::c_void;

type CFTypeRef = *const c_void;
type CFIndex = isize;

#[repr(C)]
struct CFRange {
    location: CFIndex,
    length: CFIndex,
}

const UTF8: u32 = 0x0800_0100;

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    static kCFPreferencesCurrentUser: CFTypeRef;
    static kCFPreferencesAnyHost: CFTypeRef;
    fn CFRelease(value: CFTypeRef);
    fn CFGetTypeID(value: CFTypeRef) -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFStringCreateWithBytes(
        allocator: CFTypeRef,
        bytes: *const u8,
        length: CFIndex,
        encoding: u32,
        external: u8,
    ) -> CFTypeRef;
    fn CFStringGetLength(string: CFTypeRef) -> CFIndex;
    fn CFStringGetBytes(
        string: CFTypeRef,
        range: CFRange,
        encoding: u32,
        loss: u8,
        external: u8,
        buffer: *mut u8,
        max: CFIndex,
        used: *mut CFIndex,
    ) -> CFIndex;
    fn CFArrayGetCount(array: CFTypeRef) -> CFIndex;
    fn CFArrayGetValueAtIndex(array: CFTypeRef, index: CFIndex) -> CFTypeRef;
    fn CFPreferencesCopyAppValue(key: CFTypeRef, app: CFTypeRef) -> CFTypeRef;
    fn CFPreferencesSetAppValue(key: CFTypeRef, value: CFTypeRef, app: CFTypeRef);
    fn CFPreferencesCopyKeyList(app: CFTypeRef, user: CFTypeRef, host: CFTypeRef) -> CFTypeRef;
    fn CFPreferencesAppSynchronize(app: CFTypeRef) -> u8;
}

/// An owned CoreFoundation object, released when dropped.
struct Owned(CFTypeRef);

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: `self.0` is a non-null object this value owns (a Create or
        // Copy result), released once.
        unsafe { CFRelease(self.0) }
    }
}

fn cf_string(text: &str) -> Option<Owned> {
    // SAFETY: the bytes are valid UTF-8 of the given length; a null
    // allocator is the default one.
    let s = unsafe {
        CFStringCreateWithBytes(
            std::ptr::null(),
            text.as_ptr(),
            text.len() as CFIndex,
            UTF8,
            0,
        )
    };
    (!s.is_null()).then_some(Owned(s))
}

/// `value` as Rust text, when it is a CFString.
fn rust_string(value: CFTypeRef) -> Option<String> {
    // SAFETY: `value` is a live CF object the caller holds; its type is
    // checked before it is read as a string, and the buffer is sized by a
    // first measuring call.
    unsafe {
        if value.is_null() || CFGetTypeID(value) != CFStringGetTypeID() {
            return None;
        }
        let range = CFRange {
            location: 0,
            length: CFStringGetLength(value),
        };
        let mut used: CFIndex = 0;
        CFStringGetBytes(value, range, UTF8, 0, 0, std::ptr::null_mut(), 0, &mut used);
        let mut buffer = vec![0u8; used as usize];
        let range = CFRange {
            location: 0,
            length: CFStringGetLength(value),
        };
        CFStringGetBytes(
            value,
            range,
            UTF8,
            0,
            0,
            buffer.as_mut_ptr(),
            used,
            &mut used,
        );
        buffer.truncate(used as usize);
        String::from_utf8(buffer).ok()
    }
}

/// Every `(key, value)` of domain `app` whose key starts with `prefix` and
/// whose value is a string.
pub fn read(app: &str, prefix: &str) -> Vec<(String, String)> {
    let Some(domain) = cf_string(app) else {
        return Vec::new();
    };
    // SAFETY: the domain is a live CFString; the two constants are
    // CoreFoundation's; the key list, when not null, is owned here.
    let keys = unsafe {
        CFPreferencesCopyKeyList(domain.0, kCFPreferencesCurrentUser, kCFPreferencesAnyHost)
    };
    if keys.is_null() {
        return Vec::new();
    }
    let keys = Owned(keys);
    // SAFETY: `keys` is a live CFArray of CFStrings.
    let count = unsafe { CFArrayGetCount(keys.0) };
    let mut out = Vec::new();
    for i in 0..count {
        // SAFETY: `i` is within the array; the element is borrowed from it.
        let key = unsafe { CFArrayGetValueAtIndex(keys.0, i) };
        let Some(name) = rust_string(key).filter(|k| k.starts_with(prefix)) else {
            continue;
        };
        // SAFETY: both are live CFStrings; the result, when not null, is owned.
        let value = unsafe { CFPreferencesCopyAppValue(key, domain.0) };
        if value.is_null() {
            continue;
        }
        let value = Owned(value);
        if let Some(text) = rust_string(value.0) {
            out.push((name, text));
        }
    }
    out
}

/// Set `key` in domain `app` to `value`, or remove it, and hand the
/// domain to the preferences daemon, which writes it to disk.
pub fn write(app: &str, key: &str, value: Option<&str>) -> Result<(), String> {
    let (Some(domain), Some(name)) = (cf_string(app), cf_string(key)) else {
        return Err(format!("{key}: not a preference name"));
    };
    let value = match value {
        Some(text) => Some(cf_string(text).ok_or_else(|| format!("{key}: not text"))?),
        None => None,
    };
    let raw = value.as_ref().map_or(std::ptr::null(), |v| v.0);
    // SAFETY: the key and domain are live CFStrings; a null value removes the key.
    let synced = unsafe {
        CFPreferencesSetAppValue(name.0, raw, domain.0);
        CFPreferencesAppSynchronize(domain.0)
    };
    if synced == 0 {
        return Err(format!(
            "{key}: the preferences of {app} could not be saved"
        ));
    }
    Ok(())
}
