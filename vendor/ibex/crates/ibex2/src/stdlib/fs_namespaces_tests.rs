//! `home:/` and `bundle:/` (Exact patch 24): admitted as spelt, `bundle:`
//! read only, run below the host's directories with every component opened
//! `O_NOFOLLOW`, so a link anywhere below a root is refused, not followed.
use super::*;

/// The roots are the process's: one test names them at a time.
pub(super) static ROOTED: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// A home with `.t3` granted and a bundle with `Resources`, beside a
/// directory outside both; their roots named for this process.
fn roots(name: &str) -> (PathBuf, PathBuf, GrantSet) {
    let base = std::env::temp_dir().join(format!("ibex2-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (home, bundle) = (base.join("home"), base.join("bundle"));
    std::fs::create_dir_all(home.join(".t3")).unwrap();
    std::fs::create_dir_all(bundle.join("Resources")).unwrap();
    std::fs::create_dir_all(base.join("outside")).unwrap();
    std::fs::write(bundle.join("Resources/runtime.tar"), b"tar").unwrap();
    std::fs::write(base.join("outside/secret"), b"no").unwrap();
    set_roots(Roots {
        home: Some(home.clone()),
        bundle: Some(bundle),
    });
    let grants =
        GrantSet::parse("fs.write home:/.t3\nfs.read home:/.t3\nfs.read bundle:/Resources")
            .unwrap();
    (base, home, grants)
}

fn call(
    grants: &GrantSet,
    op: FsOp,
    path: &str,
    dest: Option<&str>,
    data: Option<&[u8]>,
) -> Result<FsResult, HostError> {
    run(grants, None, op, path, dest, data)
}

#[test]
fn home_and_bundle_paths_run_on_the_hosts_directories() {
    let _rooted = ROOTED.lock().unwrap_or_else(|e| e.into_inner());
    let (base, home, grants) = roots("namespaces");
    let call = |op, path: &str, dest: Option<&str>, data: Option<&[u8]>| {
        call(&grants, op, path, dest, data)
    };
    call(
        FsOp::WriteFile,
        "home:/.t3/settings.json",
        None,
        Some(b"{}"),
    )
    .unwrap();
    assert_eq!(
        std::fs::read(home.join(".t3/settings.json")).unwrap(),
        b"{}"
    );
    assert_eq!(
        call(FsOp::ReadFile, "home:/.t3/settings.json", None, None).unwrap(),
        FsResult::Bytes(b"{}".to_vec())
    );
    // Copied out of the bundle into the home directory, as an install does.
    call(
        FsOp::CopyFile,
        "bundle:/Resources/runtime.tar",
        Some("home:/.t3/runtime.tar"),
        None,
    )
    .unwrap();
    assert_eq!(std::fs::read(home.join(".t3/runtime.tar")).unwrap(), b"tar");
    let denied = |r: Result<FsResult, HostError>| format!("{r:?}");
    assert!(denied(call(
        FsOp::WriteFile,
        "bundle:/Resources/x",
        None,
        Some(b"x")
    ))
    .contains("denied"));
    assert!(denied(call(FsOp::ReadFile, "home:/.ssh/config", None, None)).contains("fs.read"));
    assert!(denied(call(FsOp::ReadFile, "home:/.t3/../.ssh", None, None)).contains(". or .."));
    // A link itself is the link's to read: an lstat or a readlink names it,
    // and nothing follows it.
    std::os::unix::fs::symlink(base.join("outside"), home.join(".t3/escape")).unwrap();
    let FsResult::Stat(link) = call(FsOp::Lstat, "home:/.t3/escape", None, None).unwrap() else {
        panic!()
    };
    assert!(link.is_symlink);
    assert_eq!(
        call(FsOp::Readlink, "home:/.t3/escape", None, None).unwrap(),
        FsResult::Text(base.join("outside").display().to_string())
    );
    assert!(call(FsOp::ReadFile, "home:/.t3/escape/secret", None, None).is_err());
    assert!(call(FsOp::Stat, "home:/.t3/escape", None, None).is_err());
    let realpath = call(FsOp::Realpath, "home:/.t3/settings.json", None, None).unwrap();
    assert_eq!(
        realpath,
        FsResult::Text(
            std::fs::canonicalize(home.join(".t3/settings.json"))
                .unwrap()
                .display()
                .to_string()
        )
    );
}

#[test]
fn a_link_the_grants_holder_plants_reaches_nothing_outside_the_grant() {
    let _rooted = ROOTED.lock().unwrap_or_else(|e| e.into_inner());
    let (base, home, grants) = roots("planted");
    let call = |op, path: &str, data: Option<&[u8]>| call(&grants, op, path, None, data);
    let outside = base.join("outside");
    // The granted directory itself replaced by a link that leads outside: its
    // file is not read through it.
    std::fs::remove_dir(home.join(".t3")).unwrap();
    let target = outside.display().to_string();
    call(FsOp::Symlink, "home:/.t3", Some(target.as_bytes())).unwrap();
    assert!(home.join(".t3").is_symlink());
    let read = call(FsOp::ReadFile, "home:/.t3/secret", None);
    assert!(read.is_err(), "{read:?}");
    assert!(call(FsOp::Lstat, "home:/.t3/secret", None).is_err());
    std::fs::remove_file(home.join(".t3")).unwrap();
    std::fs::create_dir(home.join(".t3")).unwrap();
    // A dangling link inside it: a write does not make its target, and a
    // chmod, a digest or an access does not reach through it.
    let created = outside.join("created").display().to_string();
    call(FsOp::Symlink, "home:/.t3/p", Some(created.as_bytes())).unwrap();
    let write = call(FsOp::WriteFile, "home:/.t3/p", Some(b"x"));
    assert!(write.is_err(), "{write:?}");
    assert!(!outside.join("created").exists());
    std::os::unix::fs::symlink(outside.join("secret"), home.join(".t3/secret")).unwrap();
    for op in [FsOp::ReadFile, FsOp::Digest, FsOp::Access, FsOp::Stat] {
        let data: &[u8] = if op == FsOp::Digest { b"SHA-256" } else { b"4" };
        let r = call(op, "home:/.t3/secret", Some(data));
        assert!(r.is_err(), "{op:?}: {r:?}");
    }
    let mode = |p: &Path| {
        std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(p).unwrap().permissions())
            & 0o777
    };
    let before = mode(&outside.join("secret"));
    assert!(call(FsOp::Chmod, "home:/.t3/secret", Some(b"511")).is_err());
    assert_eq!(
        mode(&outside.join("secret")),
        before,
        "chmod followed the link"
    );
    // The link itself still answers for what it is.
    assert!(
        matches!(call(FsOp::Lstat, "home:/.t3/p", None), Ok(FsResult::Stat(s)) if s.is_symlink)
    );
}
