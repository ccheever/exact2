//! Windows app roots; lookup is read-only and never invents a HOME fallback.
#![allow(unsafe_code)]

use exact_runner::DataError;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::UI::Shell::{FOLDERID_LocalAppData, SHGetKnownFolderPath};

struct Folder(*mut u16);
impl Drop for Folder {
    fn drop(&mut self) {
        // SAFETY: SHGetKnownFolderPath owns this allocation on either outcome;
        // its contract requires CoTaskMemFree, which also accepts null.
        unsafe { CoTaskMemFree(self.0.cast()) }
    }
}

pub(super) fn local_app_data() -> Result<PathBuf, DataError> {
    let mut path = Folder(std::ptr::null_mut());
    // SAFETY: valid GUID and output pointer; null token selects current user.
    let status = unsafe {
        SHGetKnownFolderPath(&FOLDERID_LocalAppData, 0, std::ptr::null_mut(), &mut path.0)
    };
    if status < 0 || path.0.is_null() {
        return Err(DataError::Unavailable(format!(
            "app storage LocalAppData lookup failed (HRESULT {status:#x})"
        )));
    }
    // SAFETY: successful API output is a live, NUL-terminated UTF-16 string
    // until the Folder guard frees it after this owned copy.
    let value = unsafe {
        let mut length = 0;
        while *path.0.add(length) != 0 {
            length += 1;
        }
        OsString::from_wide(std::slice::from_raw_parts(path.0, length))
    };
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(DataError::Unavailable(
            "app storage LocalAppData is not absolute".into(),
        ));
    }
    Ok(path)
}

pub(super) fn bases(local: &Path, app: &str) -> Result<(PathBuf, PathBuf), DataError> {
    if !local.is_absolute() || !super::identity(app) {
        return Err(DataError::Unavailable(
            "unsafe Windows app storage root or identity".into(),
        ));
    }
    let base = local.join("exact").join(app);
    Ok((base.join("data"), base))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_folder_is_absolute_and_does_not_need_home() {
        const CHILD: &str = "EXACT_WINDOWS_KNOWN_FOLDER_TEST";
        if std::env::var_os(CHILD).is_none() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "picker::windows::tests::known_folder_is_absolute_and_does_not_need_home",
                ])
                .env(CHILD, "1")
                .env_remove("HOME")
                .env_remove("LOCALAPPDATA")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        assert!(local_app_data().unwrap().is_absolute());
    }

    #[test]
    fn app_and_scratch_identities_are_native_leaves() {
        let base = Path::new(r"C:\owned café\Local App Data");
        let (data, cache) = bases(base, "com.exact.app").unwrap();
        assert_eq!(data, base.join("exact/com.exact.app/data"));
        assert_eq!(cache, base.join("exact/com.exact.app"));
        assert_ne!(data, bases(base, "com.exact.other").unwrap().0);
        assert!(bases(Path::new("relative"), "com.exact.app").is_err());
        for name in [
            "", ".", "..", "CON", "con.txt", "LPT1", "app.", "app ", "C:app", r"a\b", "a/b",
        ] {
            assert!(!super::super::identity(name), "{name:?}");
            assert!(bases(base, name).is_err(), "{name:?}");
        }
    }

    #[test]
    fn all_logical_roots_refuse_windows_aliases_before_root_selection() {
        let base = Path::new(r"C:\owned café\Local App Data\exact\test.app");
        for (logical, selected) in [("data", "data"), ("cache", "cache"), ("tmp", "temporary")] {
            for part in [
                r"..\outside",
                r"C:\outside",
                "C:relative",
                r"\rooted",
                r"\\server\share",
                r"\\?\C:\outside",
                "a:stream",
                "NUL",
                "con.txt",
                "LPT³.log",
                "trailing.",
                "trailing ",
                "a\0b",
                "a\u{7f}b",
                "a?b",
                "a//b",
                "../cache",
                "nested/../data",
            ] {
                let path = format!("app:/{logical}/{part}");
                assert!(
                    super::super::resolve_from(&path, || panic!("invalid input selected roots"))
                        .is_none(),
                    "{path:?}"
                );
            }
            let path = format!("app:/{logical}/folder café/图像.png");
            let actual = super::super::resolve_from(&path, || {
                [
                    base.join("data"),
                    base.join("cache"),
                    base.join("temporary"),
                ]
            })
            .unwrap();
            assert_eq!(actual, base.join(selected).join("folder café/图像.png"));
        }
    }

    #[test]
    fn agent_without_storage_selects_no_known_folder() {
        const CHILD: &str = "EXACT_WINDOWS_EMPTY_STORAGE_TEST";
        if std::env::var_os(CHILD).is_none() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "picker::windows::tests::agent_without_storage_selects_no_known_folder",
                ])
                .env(CHILD, "1")
                .env("EXACT_AGENT", "1")
                .env_remove("EXACT_AGENT_STORAGE")
                .env_remove("HOME")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        assert!(super::super::app_dirs("com.exact.no-storage")
            .unwrap()
            .is_none());
    }
}
