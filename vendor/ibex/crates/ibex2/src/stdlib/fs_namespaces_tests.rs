//! `home:/` and `bundle:/` (Exact patch 22): admitted as spelt and as
//! realized, `bundle:` read only, run on the host's directories.
use super::*;

#[test]
fn home_and_bundle_paths_run_on_the_hosts_directories() {
    let base = std::env::temp_dir().join(format!("ibex2-namespaces-{}", std::process::id()));
    let (home, bundle) = (base.join("home"), base.join("bundle"));
    std::fs::create_dir_all(home.join(".t3")).unwrap();
    std::fs::create_dir_all(bundle.join("Resources")).unwrap();
    std::fs::create_dir_all(base.join("outside")).unwrap();
    std::fs::write(bundle.join("Resources/runtime.tar"), b"tar").unwrap();
    std::fs::write(base.join("outside/secret"), b"no").unwrap();
    let _ = std::os::unix::fs::symlink(base.join("outside"), home.join(".t3/escape"));
    set_roots(Roots {
        home: Some(home.clone()),
        bundle: Some(bundle.clone()),
    });
    let grants =
        GrantSet::parse("fs.write home:/.t3\nfs.read home:/.t3\nfs.read bundle:/Resources")
            .unwrap();
    let call = |op, path: &str, dest: Option<&str>, data: Option<&[u8]>| {
        run(&grants, None, op, path, dest, data)
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
    assert!(
        denied(call(FsOp::ReadFile, "home:/.t3/escape/secret", None, None))
            .contains("links are followed")
    );
    assert!(denied(call(FsOp::ReadFile, "home:/.t3/../.ssh", None, None)).contains(". or .."));
    // A link itself, its target outside the grant or missing, is the link's
    // to read: only its parent's links are followed.
    let FsResult::Stat(link) = call(FsOp::Lstat, "home:/.t3/escape", None, None).unwrap() else {
        panic!()
    };
    assert!(link.is_symlink);
    assert_eq!(
        call(FsOp::Readlink, "home:/.t3/escape", None, None).unwrap(),
        FsResult::Text(base.join("outside").display().to_string())
    );
    let _ = std::os::unix::fs::symlink(base.join("nowhere"), home.join(".t3/dangling"));
    assert!(matches!(
        call(FsOp::Lstat, "home:/.t3/dangling", None, None),
        Ok(FsResult::Stat(_))
    ));
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
