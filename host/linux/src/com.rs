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
