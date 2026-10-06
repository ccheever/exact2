//! Exact document adapter: directory type is evidence; access denial is not.
use super::*;
use std::os::windows::ffi::OsStrExt;
use std::process::Command;
use windows_sys::Win32::Security::Authorization::{SetNamedSecurityInfoW, SE_FILE_OBJECT};
use windows_sys::Win32::Security::*;

struct Dacl {
    descriptor: Vec<u32>,
    control: u16,
    acl: Vec<u8>,
}
fn dacl(path: &[u16]) -> Dacl {
    let mut needed = 0;
    // SAFETY: the path is NUL-terminated; this first call only obtains size.
    unsafe {
        GetFileSecurityW(
            path.as_ptr(),
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            0,
            &mut needed,
        )
    };
    assert!(
        needed > 0,
        "DACL capture unavailable: {}",
        std::io::Error::last_os_error()
    );
    let mut descriptor = vec![0u32; (needed as usize).div_ceil(4)];
    let pointer = descriptor.as_mut_ptr().cast();
    // SAFETY: u32 storage is aligned for the self-relative descriptor and is
    // at least the byte size returned above. Only DACL authority is requested.
    assert_ne!(
        unsafe {
            GetFileSecurityW(
                path.as_ptr(),
                DACL_SECURITY_INFORMATION,
                pointer,
                needed,
                &mut needed,
            )
        },
        0
    );
    let (mut control, mut revision) = (0, 0);
    let (mut present, mut defaulted, mut acl) = (0, 0, std::ptr::null_mut());
    // SAFETY: the successful call above produced a live security descriptor.
    assert_ne!(
        unsafe { GetSecurityDescriptorControl(pointer, &mut control, &mut revision) },
        0
    );
    // SAFETY: all outputs are live; the ACL pointer remains owned by descriptor.
    assert_ne!(
        unsafe { GetSecurityDescriptorDacl(pointer, &mut present, &mut acl, &mut defaulted) },
        0
    );
    assert!(
        present != 0 && !acl.is_null(),
        "owned fixture requires an explicit DACL"
    );
    // SAFETY: GetSecurityDescriptorDacl returned this descriptor's valid ACL.
    let acl =
        unsafe { std::slice::from_raw_parts(acl.cast::<u8>(), (*acl).AclSize as usize) }.to_vec();
    Dacl {
        descriptor,
        control,
        acl,
    }
}

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "exact document ACL café {}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn read(path: &Path) -> Result<FsResult, HostError> {
    let path = path.to_owned();
    let documents = move |_: &str| Ok(Document::Real(path.clone()));
    let grants = GrantSet::parse("fs.read doc:/\n").unwrap();
    run_document(
        &grants,
        Some(&documents),
        FsOp::ReadFile,
        "doc:/1/chosen",
        None,
    )
}

#[test]
fn document_read_uses_handle_type_and_keeps_physical_paths_private() {
    let root = Root::new();
    let file = root.0.join("file.txt");
    std::fs::write(&file, b"contents").unwrap();
    assert_eq!(read(&file).unwrap(), FsResult::Bytes(b"contents".to_vec()));
    let error = read(&root.0).unwrap_err().to_string();
    assert_eq!(
        error,
        "fs.readFile doc:/1/chosen: cannot read a directory (filesystem code EISDIR)"
    );
    assert!(!error.contains(root.0.to_str().unwrap()));
    let never = |_: &str| -> Result<Document, String> {
        panic!("a refused grant must not resolve the document")
    };
    assert!(run_document(
        &GrantSet::none(),
        Some(&never),
        FsOp::ReadFile,
        "doc:/1/chosen",
        None
    )
    .is_err());
}

#[test]
fn document_read_keeps_real_acl_access_denial_and_restores_original_dacl() {
    let root = Root::new();
    let file = root.0.join("denied.txt");
    std::fs::write(&file, b"private").unwrap();
    let path: Vec<u16> = file.as_os_str().encode_wide().chain(Some(0)).collect();
    let original = dacl(&path);
    let (mut present, mut defaulted, mut original_acl) = (0, 0, std::ptr::null_mut());
    // SAFETY: capture succeeded above; the ACL remains owned by original for
    // the entire mutation/restoration sequence. Resolve it before mutation.
    assert_ne!(
        unsafe {
            GetSecurityDescriptorDacl(
                original.descriptor.as_ptr().cast_mut().cast(),
                &mut present,
                &mut original_acl,
                &mut defaulted,
            )
        },
        0
    );
    assert!(present != 0 && !original_acl.is_null());
    let acl = |args: &[&std::ffi::OsStr]| {
        Command::new("icacls.exe")
            .current_dir(&root.0)
            .args(args)
            .output()
            .unwrap()
    };
    // Only this owned file is changed. Even a denied/partially applied command
    // proceeds through restoration before the assertion/panic is surfaced.
    let result = std::panic::catch_unwind(|| {
        let denial = acl(&[
            "denied.txt".as_ref(),
            "/deny".as_ref(),
            "*S-1-1-0:(RD)".as_ref(),
            "/q".as_ref(),
        ]);
        assert!(
            denial.status.success(),
            "ACL mutation unqualified: {}",
            String::from_utf8_lossy(&denial.stderr)
        );
        let error = read(&file).unwrap_err().to_string();
        assert!(error.ends_with("(os error 5)"), "{error}");
        assert!(!error.contains("filesystem code EISDIR"), "{error}");
        assert!(
            !error.contains(root.0.to_str().unwrap()),
            "physical path leaked"
        );
    });
    let protection = if original.control & SE_DACL_PROTECTED != 0 {
        PROTECTED_DACL_SECURITY_INFORMATION
    } else {
        UNPROTECTED_DACL_SECURITY_INFORMATION
    };
    // SAFETY: the captured ACL is still live and the owned path is
    // NUL-terminated. The modern named API preserves automatic inheritance;
    // restore only the DACL and its original protection, never owner/group/SACL.
    let restored = unsafe {
        SetNamedSecurityInfoW(
            path.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | protection,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            original_acl,
            std::ptr::null_mut(),
        )
    };
    assert_eq!(
        restored, 0,
        "owned DACL restore failed with Windows status {restored}"
    );
    let after = dacl(&path);
    assert!(
        after.acl == original.acl,
        "restored DACL differs from captured original"
    );
    let inheritance = SE_DACL_PROTECTED | SE_DACL_AUTO_INHERITED | SE_DACL_AUTO_INHERIT_REQ;
    assert_eq!(after.control & inheritance, original.control & inheritance);
    assert_eq!(std::fs::read(file).unwrap(), b"private");
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}
