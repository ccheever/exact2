//! Optional Windows APIs are loaded only when their caller is enabled.
use windows::{
    core::{PCSTR, PCWSTR},
    Win32::{
        Foundation::{FreeLibrary, HMODULE},
        System::LibraryLoader::*,
    },
};

pub(super) struct Library(HMODULE);
impl Library {
    pub(super) fn open(name: PCWSTR) -> windows::core::Result<Self> {
        // Only system DLLs are used; never search the app or current directory.
        unsafe { LoadLibraryExW(name, None, LOAD_LIBRARY_SEARCH_SYSTEM32).map(Self) }
    }

    /// `T` must be the export's function-pointer ABI and must not outlive this library.
    pub(super) unsafe fn get<T: Copy>(&self, name: PCSTR) -> windows::core::Result<T> {
        let address = unsafe { GetProcAddress(self.0, name) }
            .ok_or_else(windows::core::Error::from_thread)?;
        assert_eq!(size_of::<T>(), size_of_val(&address));
        Ok(unsafe { std::mem::transmute_copy(&address) })
    }
}
impl Drop for Library {
    fn drop(&mut self) {
        unsafe {
            let _ = FreeLibrary(self.0);
        }
    }
}
