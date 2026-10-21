//! Windows: the operating system's ICU behind Ibex's Intl shims.
//!
//! The shims link the ICU C API they share with Hermes against the frozen
//! `icuuc`/`icuin` import libraries the Hermes VM already imports at load
//! time. The few entry points only `icu.dll` exports (the `unumf_*` number
//! formatter, Windows 10 2004 and later) are not linked at all: the shims call
//! them through a table of function pointers that [`available`] binds once,
//! from System32's `icu.dll` loaded by its full path and verified by
//! `GetModuleFileNameW`. The probe *is* the binding. If the module or any
//! pointer is missing, `INTL` is refused with a clear error, the Intl host
//! operations refuse to run, and nothing ever calls an unbound pointer.
//! Because no shim symbol is linked against `icu.dll`, a shim that calls a new
//! `icu.dll`-only function without adding it to the table fails to link.
//!
//! The ICU version, its CLDR data, and its tzdata belong to Windows and change
//! with Windows Update. [`os_icu`] reports what this process observes; it is a
//! diagnostic, never a pinned identity, and nothing here claims a digest.
//!
//! @ref LLP 0057.000#511-windows-intl-uses-the-os-icu — OS ICU bound by full System32 path, probe-gated INTL, observed versions

use std::ffi::{c_char, c_int, c_void, CStr};
use std::sync::OnceLock;
use windows_sys::Win32::Foundation::{FreeLibrary, ERROR_MOD_NOT_FOUND, HMODULE};
use windows_sys::Win32::System::LibraryLoader::{
    GetModuleFileNameW, GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32,
};
use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;

/// The OS ICU DLL, by base name. It is only ever loaded from System32 by its
/// full path.
pub(crate) const DLL: &str = "icu.dll";

/// What this process observes of the operating system's ICU. Every field is
/// an unpinned fact about the machine running the process, not about the
/// build; the same binary reports different values after a Windows update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsIcu {
    /// The DLL the shims' pointers are bound from (`icu.dll`).
    pub dll: &'static str,
    /// Where that module was loaded from, as `GetModuleFileNameW` reports it:
    /// always the System32 path, for example `C:\WINDOWS\system32\icu.dll`.
    pub path: String,
    /// `u_getVersion`, for example `72.1.0.4`.
    pub icu: String,
    /// `u_getUnicodeVersion`, for example `15.1`.
    pub unicode: String,
    /// `ulocdata_getCLDRVersion`, for example `42.0`. Microsoft modifies the
    /// CLDR data it ships, so this names the base release only.
    pub cldr: String,
    /// `ucal_getTZDataVersion`, for example `2022g`.
    pub tzdata: String,
}

extern "C" {
    /// `intl_icu_windows.cc`: resolve every entry in the shims' pointer
    /// table from `module` and publish the table. Returns null on success, or
    /// the name of the first entry point `module` does not export (and then
    /// publishes nothing).
    fn ibex2_intl_os_icu_bind(module: *mut c_void) -> *const c_char;
}

/// An `HMODULE` that is never freed once bound: the shims' pointers point
/// into it for the life of the process.
#[derive(Clone, Copy)]
struct Module {
    handle: usize,
}

struct Bound {
    module: Module,
    path: String,
}

fn bound() -> Result<&'static Bound, &'static str> {
    static BOUND: OnceLock<Result<Bound, String>> = OnceLock::new();
    BOUND.get_or_init(bind).as_ref().map_err(String::as_str)
}

/// `GetSystemDirectoryW()` + `\icu.dll`, without a terminating NUL.
fn system32_icu() -> Result<Vec<u16>, String> {
    let mut buffer = vec![0u16; 260];
    loop {
        // SAFETY: the buffer is writable for `buffer.len()` UTF-16 units.
        let length =
            unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) } as usize;
        if length == 0 {
            return Err(format!(
                "GetSystemDirectoryW failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        if length < buffer.len() {
            buffer.truncate(length);
            break;
        }
        // Too small: `length` is the size needed, including the NUL.
        buffer.resize(length, 0);
    }
    buffer.extend(format!("\\{DLL}").encode_utf16());
    Ok(buffer)
}

/// `GetModuleFileNameW` for a loaded module, without a terminating NUL.
fn module_file_name(module: HMODULE) -> Result<Vec<u16>, String> {
    let mut buffer = vec![0u16; 260];
    loop {
        // SAFETY: `module` is a loaded module and the buffer is writable for
        // `buffer.len()` UTF-16 units.
        let length = unsafe { GetModuleFileNameW(module, buffer.as_mut_ptr(), buffer.len() as u32) }
            as usize;
        if length == 0 {
            return Err(format!(
                "GetModuleFileNameW failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        if length < buffer.len() {
            buffer.truncate(length);
            return Ok(buffer);
        }
        // Truncated: retry with room to spare (long paths reach 32,767).
        if buffer.len() >= 32_768 {
            return Err("GetModuleFileNameW: path too long".to_owned());
        }
        buffer.resize(buffer.len() * 2, 0);
    }
}

/// NTFS compares paths case-insensitively; System32 is reported as
/// `C:\WINDOWS\system32` by one API and `C:\Windows\System32` by another.
fn same_path(a: &[u16], b: &[u16]) -> bool {
    String::from_utf16_lossy(a).to_lowercase() == String::from_utf16_lossy(b).to_lowercase()
}

fn bind() -> Result<Bound, String> {
    let expected = system32_icu()?;
    let expected_display = String::from_utf16_lossy(&expected);
    let path: Vec<u16> = expected.iter().copied().chain(std::iter::once(0)).collect();
    // SAFETY: `path` is a NUL-terminated absolute path that outlives the call.
    // An absolute path loads that file even when another module named
    // icu.dll (an application-directory copy, say) is already mapped, and the
    // System32-only flag keeps icu.dll's own dependencies in System32.
    let module = unsafe {
        LoadLibraryExW(
            path.as_ptr(),
            std::ptr::null_mut(),
            LOAD_LIBRARY_SEARCH_SYSTEM32,
        )
    };
    if module.is_null() {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(ERROR_MOD_NOT_FOUND as i32) {
            return Err(format!(
                "{expected_display} is not present (Windows 10 version 1903 or later provides it)"
            ));
        }
        return Err(format!("cannot load {expected_display}: {error}"));
    }
    let loaded = match module_file_name(module) {
        Ok(loaded) => loaded,
        Err(error) => {
            // SAFETY: releases the reference taken by LoadLibraryExW above.
            unsafe { FreeLibrary(module) };
            return Err(error);
        }
    };
    if !same_path(&loaded, &expected) {
        // SAFETY: as above; nothing was bound from this module.
        unsafe { FreeLibrary(module) };
        return Err(format!(
            "loading {expected_display} produced {}, not the System32 module",
            String::from_utf16_lossy(&loaded)
        ));
    }
    // SAFETY: `module` is loaded and, from here on, never freed.
    let missing = unsafe { ibex2_intl_os_icu_bind(module) };
    if !missing.is_null() {
        // SAFETY: the binder returns one of its own static entry-point names.
        let name = unsafe { CStr::from_ptr(missing) }.to_string_lossy();
        // SAFETY: as above; the binder published nothing.
        unsafe { FreeLibrary(module) };
        return Err(format!(
            "{DLL} does not export {name} (Windows 10 version 2004 or later is required)"
        ));
    }
    Ok(Bound {
        module: Module {
            handle: module as usize,
        },
        path: String::from_utf16_lossy(&loaded),
    })
}

fn symbol(module: Module, name: &CStr) -> Option<unsafe extern "system" fn() -> isize> {
    // SAFETY: the module handle is live for the process (never freed) and
    // `name` is NUL-terminated.
    unsafe { GetProcAddress(module.handle as HMODULE, name.as_ptr().cast()) }
}

/// Whether `INTL` can run on this Windows: the first call loads System32's
/// `icu.dll` and binds the shims' pointer table; later calls return the
/// cached outcome.
pub(crate) fn available() -> Result<(), &'static str> {
    bound().map(|_| ())
}

/// What this process observes of the operating system's ICU, or why
/// `INTL` is unavailable on this Windows.
pub fn os_icu() -> Result<&'static OsIcu, &'static str> {
    static OBSERVED: OnceLock<Result<OsIcu, String>> = OnceLock::new();
    OBSERVED
        .get_or_init(|| observe(bound().map_err(str::to_owned)?))
        .as_ref()
        .map_err(String::as_str)
}

type GetVersion = unsafe extern "C" fn(*mut u8);
type GetCldrVersion = unsafe extern "C" fn(*mut u8, *mut c_int);
type GetTzDataVersion = unsafe extern "C" fn(*mut c_int) -> *const c_char;

fn observe(bound: &Bound) -> Result<OsIcu, String> {
    let resolve = |name: &CStr| {
        symbol(bound.module, name)
            .ok_or_else(|| format!("{DLL} does not export {}", name.to_string_lossy()))
    };
    // SAFETY: each pointer was resolved by name from the verified System32
    // icu.dll, and each type matches the ICU C declaration in the Windows
    // SDK's <icu.h> (UVersionInfo is uint8_t[4]; UErrorCode is a C enum,
    // i.e. int).
    unsafe {
        let get_version: GetVersion = std::mem::transmute(resolve(c"u_getVersion")?);
        let get_unicode: GetVersion = std::mem::transmute(resolve(c"u_getUnicodeVersion")?);
        let get_cldr: GetCldrVersion = std::mem::transmute(resolve(c"ulocdata_getCLDRVersion")?);
        let get_tzdata: GetTzDataVersion = std::mem::transmute(resolve(c"ucal_getTZDataVersion")?);

        let mut icu = [0u8; 4];
        get_version(icu.as_mut_ptr());
        let mut unicode = [0u8; 4];
        get_unicode(unicode.as_mut_ptr());
        let mut cldr = [0u8; 4];
        let mut status: c_int = 0;
        get_cldr(cldr.as_mut_ptr(), &mut status);
        if status > 0 {
            return Err(format!(
                "ulocdata_getCLDRVersion failed with UErrorCode {status}"
            ));
        }
        let mut status: c_int = 0;
        let tzdata = get_tzdata(&mut status);
        if status > 0 || tzdata.is_null() {
            return Err(format!(
                "ucal_getTZDataVersion failed with UErrorCode {status}"
            ));
        }
        Ok(OsIcu {
            dll: DLL,
            path: bound.path.clone(),
            icu: version_string(icu),
            unicode: version_string(unicode),
            cldr: version_string(cldr),
            tzdata: CStr::from_ptr(tzdata).to_string_lossy().into_owned(),
        })
    }
}

/// `u_versionToString`'s format: dotted fields, trailing zero fields omitted,
/// but never fewer than two.
fn version_string(version: [u8; 4]) -> String {
    let mut fields = 4;
    while fields > 2 && version[fields - 1] == 0 {
        fields -= 1;
    }
    version[..fields]
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(".")
}

/// The C++ half of the INTL check: `validate_groups` refuses the group
/// exactly when [`crate::bindings::Groups::validate`] does, and every call
/// through the shims' pointer table first confirms the table is bound.
#[no_mangle]
pub extern "C" fn ibex2_intl_os_icu_available() -> c_int {
    c_int::from(available().is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_windows_binds_the_system32_os_icu() {
        available().expect("this Windows exports every entry point the shims bind");
        let observed = os_icu().expect("observed OS ICU versions");
        assert_eq!(observed.dll, "icu.dll");
        let expected = String::from_utf16_lossy(&system32_icu().unwrap());
        assert!(
            observed.path.eq_ignore_ascii_case(&expected),
            "{} is not {expected}",
            observed.path
        );
        assert!(!observed.icu.is_empty() && !observed.cldr.is_empty());
    }

    #[test]
    fn versions_format_like_icu() {
        assert_eq!(version_string([72, 1, 0, 4]), "72.1.0.4");
        assert_eq!(version_string([42, 0, 0, 0]), "42.0");
        assert_eq!(version_string([15, 1, 0, 0]), "15.1");
        assert_eq!(version_string([74, 2, 1, 0]), "74.2.1");
    }
}
