//! Keep the process MTA alive while WinRT's cached agile factories exist.
#![allow(unsafe_code)]

/// Keep WinRT's cached factories alive across temporary worker apartments.
pub fn ensure_mta() -> Result<(), windows::core::HRESULT> {
    static MTA: std::sync::OnceLock<Result<(), windows::core::HRESULT>> =
        std::sync::OnceLock::new();
    *MTA.get_or_init(|| {
        // SAFETY: this process-lifetime usage reference is intentionally not
        // released. Cached WinRT factories also live until process exit; a
        // transient WIC worker must not tear their apartment down when it exits.
        unsafe { windows::Win32::System::Com::CoIncrementMTAUsage() }
            .map(|_| ())
            .map_err(|error| error.code())
    })
}

/// Make this thread a single-threaded apartment, once, for the packages'
/// WinRT calls on the loop's thread (LLP 1133.022 D10 item 5): a WinRT
/// picker and a GATT client run there. A thread winit already initialized
/// for OLE stays as it is (`RPC_E_CHANGED_MODE` or `S_FALSE`, both kept).
pub fn ensure_sta() {
    thread_local!(static DONE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) });
    if DONE.with(|done| done.replace(true)) {
        return;
    }
    // SAFETY: never uninitialized; the loop's thread keeps its apartment
    // until the process ends.
    let _ = unsafe {
        windows::Win32::System::Com::CoInitializeEx(
            None,
            windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
        )
    };
}
