//! A native absolute path (Exact patch 24: exact2 LLP 1133.007 D6a): admitted
//! as spelt and as realized, then run below the realized directory of the
//! deepest grant line that admits it, every component opened `O_NOFOLLOW`,
//! so a link below a granted directory, one the grant's holder made
//! included, is refused rather than followed.
use super::*;
use std::os::unix::fs::PermissionsExt;

/// A granted directory beside one outside it.
fn granted(name: &str) -> (PathBuf, PathBuf, GrantSet) {
    let base = std::env::temp_dir().join(format!("ibex2-native-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (inside, outside) = (base.join("granted"), base.join("outside"));
    std::fs::create_dir_all(&inside).unwrap();
    std::fs::create_dir_all(outside.join("dir")).unwrap();
    std::fs::write(outside.join("secret"), b"outside").unwrap();
    std::fs::set_permissions(
        outside.join("secret"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    std::fs::write(outside.join("dir/inner"), b"inner").unwrap();
    let at = inside.display();
    let grants = GrantSet::parse(&format!("fs.write {at}\nfs.read {at}")).unwrap();
    (inside, outside, grants)
}

fn call(
    grants: &GrantSet,
    op: FsOp,
    path: &Path,
    data: Option<&[u8]>,
) -> Result<FsResult, HostError> {
    run(grants, None, op, &path.display().to_string(), None, data)
}

fn call2(grants: &GrantSet, op: FsOp, path: &Path, to: &Path) -> Result<FsResult, HostError> {
    let (path, to) = (path.display().to_string(), to.display().to_string());
    run(grants, None, op, &path, Some(&to), None)
}

#[test]
fn every_operation_runs_below_a_native_grant() {
    let (inside, _, grants) = granted("runs");
    let at = |name: &str| inside.join(name);
    call(&grants, FsOp::WriteFile, &at("a"), Some(b"one")).unwrap();
    call(&grants, FsOp::AppendFile, &at("a"), Some(b"two")).unwrap();
    call(&grants, FsOp::AtomicWriteFile, &at("b"), Some(b"atomic")).unwrap();
    call(&grants, FsOp::Mkdir, &at("d/e"), None).unwrap();
    assert_eq!(
        call(&grants, FsOp::ReadFile, &at("a"), None).unwrap(),
        FsResult::Bytes(b"onetwo".to_vec())
    );
    let names = call(&grants, FsOp::ReadDir, &inside, None).unwrap();
    assert_eq!(
        names,
        FsResult::Names(vec!["a".into(), "b".into(), "d".into()])
    );
    call(&grants, FsOp::Chmod, &at("a"), Some(b"448")).unwrap();
    let mode = std::fs::metadata(at("a")).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o700);
    assert!(
        matches!(call(&grants, FsOp::Stat, &at("a"), None), Ok(FsResult::Stat(s)) if s.size == 6)
    );
    assert!(call(&grants, FsOp::Access, &at("a"), Some(b"4")).is_ok());
    assert!(matches!(
        call(&grants, FsOp::Digest, &at("b"), Some(b"SHA-256")),
        Ok(FsResult::Text(_))
    ));
    assert!(matches!(
        call(&grants, FsOp::Flock, &at("lock"), Some(b"0")),
        Ok(FsResult::Text(_))
    ));
    // A link inside the grant, made and read as itself.
    call(&grants, FsOp::Symlink, &at("l"), Some(b"a")).unwrap();
    assert!(
        matches!(call(&grants, FsOp::Lstat, &at("l"), None), Ok(FsResult::Stat(s)) if s.is_symlink)
    );
    assert_eq!(
        call(&grants, FsOp::Readlink, &at("l"), None).unwrap(),
        FsResult::Text("a".into())
    );
    call2(&grants, FsOp::CopyFile, &at("a"), &at("c")).unwrap();
    call2(&grants, FsOp::Rename, &at("c"), &at("d/e/c")).unwrap();
    assert_eq!(std::fs::read(at("d/e/c")).unwrap(), b"onetwo");
    let real = std::fs::canonicalize(&inside).unwrap().join("d/e/c");
    let answered = call(&grants, FsOp::Realpath, &at("d/e/c"), None).unwrap();
    assert_eq!(answered, FsResult::Text(real.display().to_string()));
    call(&grants, FsOp::Remove, &at("d"), None).unwrap();
    assert!(!at("d").exists());
}

#[test]
fn a_dangling_link_below_a_native_grant_makes_no_file_outside_it() {
    // The review's first reproduction: a link to a missing outside path,
    // made by the grant's holder, then a write through it.
    let (inside, outside, grants) = granted("dangling");
    let link = inside.join("l");
    let missing = outside.join("made");
    let target = missing.display().to_string();
    call(&grants, FsOp::Symlink, &link, Some(target.as_bytes())).unwrap();
    for (op, data) in [
        (FsOp::WriteFile, Some(&b"x"[..])),
        (FsOp::AppendFile, Some(&b"x"[..])),
        (FsOp::Flock, Some(&b"0"[..])),
    ] {
        assert!(call(&grants, op, &link, data).is_err(), "{op:?}");
        assert!(!missing.exists(), "{op:?} made the link's target");
    }
    std::fs::write(inside.join("src"), b"bytes").unwrap();
    assert!(call2(&grants, FsOp::CopyFile, &inside.join("src"), &link).is_err());
    assert!(!missing.exists(), "copyFile made the link's target");
    // An atomic write replaces the link itself, inside the grant.
    call(&grants, FsOp::AtomicWriteFile, &link, Some(b"y")).unwrap();
    assert!(!missing.exists());
    assert!(!std::fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn a_link_below_a_native_grant_to_an_outside_file_is_not_followed() {
    let (inside, outside, grants) = granted("outside");
    let secret = outside.join("secret");
    std::os::unix::fs::symlink(&secret, inside.join("l")).unwrap();
    std::os::unix::fs::symlink(outside.join("dir"), inside.join("d")).unwrap();
    let link = inside.join("l");
    for (op, data) in [
        (FsOp::ReadFile, None),
        (FsOp::WriteFile, Some(&b"x"[..])),
        (FsOp::AppendFile, Some(&b"x"[..])),
        (FsOp::Chmod, Some(&b"511"[..])),
        (FsOp::Digest, Some(&b"SHA-256"[..])),
        (FsOp::Access, Some(&b"4"[..])),
        (FsOp::Stat, None),
        (FsOp::Flock, Some(&b"0"[..])),
        (FsOp::Realpath, None),
    ] {
        assert!(
            call(&grants, op, &link, data).is_err(),
            "{op:?} followed the link"
        );
    }
    let copied = inside.join("copied");
    assert!(call2(&grants, FsOp::CopyFile, &link, &copied).is_err());
    // Through a linked directory, any operation is refused.
    let through = inside.join("d/inner");
    assert!(call(&grants, FsOp::ReadFile, &through, None).is_err());
    assert!(call(&grants, FsOp::WriteFile, &through, Some(b"x")).is_err());
    assert!(call(&grants, FsOp::Mkdir, &inside.join("d/m"), None).is_err());
    assert!(call(&grants, FsOp::ReadDir, &inside.join("d"), None).is_err());
    assert_eq!(std::fs::read(&secret).unwrap(), b"outside");
    assert_eq!(std::fs::read(outside.join("dir/inner")).unwrap(), b"inner");
    let mode = std::fs::metadata(&secret).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    assert!(!outside.join("dir/m").exists());
    // `lstat`, `readlink` and `rm` act on the link itself.
    assert!(
        matches!(call(&grants, FsOp::Lstat, &link, None), Ok(FsResult::Stat(s)) if s.is_symlink)
    );
    let named = call(&grants, FsOp::Readlink, &link, None).unwrap();
    assert_eq!(named, FsResult::Text(secret.display().to_string()));
    call(&grants, FsOp::Remove, &link, None).unwrap();
    assert!(secret.exists() && !link.exists());
}

#[test]
fn a_path_outside_every_native_grant_is_denied_before_any_walk() {
    let (inside, outside, grants) = granted("denied");
    let denied = call(&grants, FsOp::ReadFile, &outside.join("secret"), None);
    assert!(format!("{denied:?}").contains("fs.read"), "{denied:?}");
    // The granted directory itself answers what a directory can.
    assert!(matches!(
        call(&grants, FsOp::ReadDir, &inside, None),
        Ok(FsResult::Names(_))
    ));
    assert!(call(&grants, FsOp::WriteFile, &inside, Some(b"x")).is_err());
}
