//! A native absolute path (Exact patch 24: exact2 LLP 1133.007 D6a): admitted
//! as spelt, then walked from the directory its grant line reaches without
//! following a link below a directory the app may write (`reach`), every
//! component below that opened `O_NOFOLLOW`, so a link below a written
//! directory, one the grant's holder made included, is refused rather than
//! followed, whenever it was made.
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
    // Through a linked directory, every operation is refused.
    let through = inside.join("d/inner");
    for (op, data) in [
        (FsOp::ReadFile, None),
        (FsOp::WriteFile, Some(&b"x"[..])),
        (FsOp::AppendFile, Some(&b"x"[..])),
        (FsOp::AtomicWriteFile, Some(&b"x"[..])),
        (FsOp::Remove, None),
        (FsOp::Stat, None),
        (FsOp::Lstat, None),
        (FsOp::Readlink, None),
        (FsOp::Access, Some(&b"4"[..])),
        (FsOp::Chmod, Some(&b"511"[..])),
        (FsOp::Digest, Some(&b"SHA-256"[..])),
        (FsOp::Flock, Some(&b"0"[..])),
        (FsOp::Realpath, None),
    ] {
        assert!(
            call(&grants, op, &through, data).is_err(),
            "{op:?} through a link"
        );
    }
    let made = inside.join("d/made");
    assert!(call(&grants, FsOp::Symlink, &made, Some(b"inner")).is_err());
    assert!(call(&grants, FsOp::Mkdir, &inside.join("d/m"), None).is_err());
    assert!(call(&grants, FsOp::ReadDir, &inside.join("d"), None).is_err());
    std::fs::write(inside.join("src"), b"src").unwrap();
    assert!(call2(&grants, FsOp::CopyFile, &inside.join("src"), &through).is_err());
    assert!(call2(&grants, FsOp::Rename, &inside.join("src"), &through).is_err());
    assert!(call2(&grants, FsOp::Rename, &through, &inside.join("taken")).is_err());
    assert!(!outside.join("made").exists() && !inside.join("taken").exists());
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

#[test]
fn a_granted_directory_the_app_replaced_with_a_link_is_not_followed() {
    // An `fs.read` line below an `fs.write` one: the outer line lets the app
    // move the inner granted directory away and put a link in its place.
    let (inside, outside, _) = granted("nested");
    let inner = inside.join("inner");
    std::fs::create_dir_all(&inner).unwrap();
    let (at, deeper) = (inside.display(), inner.display());
    let grants = GrantSet::parse(&format!("fs.write {at}\nfs.read {deeper}")).unwrap();
    call(&grants, FsOp::WriteFile, &inner.join("own"), Some(b"own")).unwrap();
    call2(&grants, FsOp::Rename, &inner, &inside.join("moved")).unwrap();
    let target = outside.display().to_string();
    call(&grants, FsOp::Symlink, &inner, Some(target.as_bytes())).unwrap();
    let made = inner.join("made");
    assert!(call(&grants, FsOp::WriteFile, &made, Some(b"x")).is_err());
    assert!(!outside.join("made").exists(), "wrote through the link");
    assert!(call(&grants, FsOp::ReadFile, &inner.join("secret"), None).is_err());
    let mode = Some(&b"511"[..]);
    assert!(call(&grants, FsOp::Chmod, &inner.join("secret"), mode).is_err());
    let secret = std::fs::metadata(outside.join("secret")).unwrap();
    assert_eq!(secret.permissions().mode() & 0o777, 0o600);
    // Put back, the directory reads through the outer line's walk.
    call(&grants, FsOp::Remove, &inner, None).unwrap();
    std::fs::rename(inside.join("moved"), &inner).unwrap();
    assert_eq!(
        call(&grants, FsOp::ReadFile, &inner.join("own"), None).unwrap(),
        FsResult::Bytes(b"own".to_vec())
    );
}

#[test]
fn a_native_grant_below_a_home_line_the_app_replaced_is_not_followed() {
    // A `home:` line covers a native one: the app may replace the native
    // line's directory through the home line.
    let _rooted = (super::namespaces_tests::ROOTED.lock()).unwrap_or_else(|e| e.into_inner());
    let (inside, outside, _) = granted("home");
    let inner = inside.join("inner");
    std::fs::create_dir_all(&inner).unwrap();
    let base = inside.parent().unwrap().to_path_buf();
    set_roots(Roots {
        home: Some(base.clone()),
        bundle: None,
    });
    let deeper = inner.display();
    let grants = GrantSet::parse(&format!(
        "fs.write home:/granted\nfs.read home:/granted\nfs.read {deeper}"
    ))
    .unwrap();
    let moved = "home:/granted/moved";
    run(
        &grants,
        None,
        FsOp::Rename,
        "home:/granted/inner",
        Some(moved),
        None,
    )
    .unwrap();
    std::os::unix::fs::symlink(&outside, &inner).unwrap();
    assert!(call(&grants, FsOp::ReadFile, &inner.join("secret"), None).is_err());
    std::fs::remove_file(&inner).unwrap();
    std::fs::rename(base.join("granted/moved"), &inner).unwrap();
    assert!(matches!(
        call(&grants, FsOp::ReadDir, &inner, None),
        Ok(FsResult::Names(_))
    ));
}

/// The read line spelt one way and the write line above it another (macOS's
/// temporary directory is below `/var`, a link to `/private/var`): the lines
/// are compared by identity, so the inner line's walk still stops at the
/// outer directory, above the inner one the app replaced.
#[test]
fn an_outer_write_line_spelt_through_a_link_still_covers_the_inner_line() {
    for (name, realized_outer) in [("spelt-inner", true), ("spelt-outer", false)] {
        let (inside, outside, _) = granted(name);
        let inner = inside.join("inner");
        std::fs::create_dir_all(&inner).unwrap();
        let real_inside = std::fs::canonicalize(&inside).unwrap();
        // Where the temporary directory is not below a link, a link of the
        // test's own spells the lines two ways.
        let (outer, deeper) = if real_inside != inside {
            (inside.clone(), real_inside.join("inner"))
        } else {
            let spelt = inside.parent().unwrap().join("spelt");
            std::os::unix::fs::symlink(&inside, &spelt).unwrap();
            (spelt, inner.clone())
        };
        let (outer, deeper) = match realized_outer {
            true => (outer, deeper),
            false => (deeper.parent().unwrap().to_path_buf(), outer.join("inner")),
        };
        let grants = GrantSet::parse(&format!(
            "fs.write {0}\nfs.read {0}\nfs.read {1}",
            outer.display(),
            deeper.display()
        ))
        .unwrap();
        call2(
            &grants,
            FsOp::Rename,
            &outer.join("inner"),
            &outer.join("moved"),
        )
        .unwrap();
        let target = outside.display().to_string();
        let link = outer.join("inner");
        call(&grants, FsOp::Symlink, &link, Some(target.as_bytes())).unwrap();
        let got = call(&grants, FsOp::ReadFile, &deeper.join("secret"), None);
        assert!(
            got.is_err(),
            "{name}: read outside through the link: {got:?}"
        );
        let listed = call(&grants, FsOp::ReadDir, &deeper, None);
        assert!(listed.is_err(), "{name}: listed outside: {listed:?}");
    }
}

/// A granted directory the app's grants let something replace before the
/// app's first native operation is not followed either.
#[test]
fn a_granted_directory_replaced_before_the_apps_first_operation_is_not_followed() {
    let (inside, outside, _) = granted("first");
    let inner = inside.join("inner");
    std::fs::create_dir_all(&inner).unwrap();
    let real_inner = std::fs::canonicalize(&inner).unwrap();
    let at = inside.display();
    let grants = GrantSet::parse(&format!(
        "fs.write {at}\nfs.read {at}\nfs.read {}",
        real_inner.display()
    ))
    .unwrap();
    let _bindings = crate::host::Host::new().endow(grants.clone());
    // Changed by hand, as through another grant's holder, before any call.
    std::fs::rename(&inner, inside.join("moved")).unwrap();
    std::os::unix::fs::symlink(&outside, &inner).unwrap();
    let got = call(&grants, FsOp::ReadFile, &real_inner.join("secret"), None);
    assert!(got.is_err(), "read outside through the link: {got:?}");
}

/// The app replaces a directory under one set of grants; a set that names
/// it, loaded later in the same process (a second endow, a reload, another
/// session), does not follow the link.
#[test]
fn a_line_granted_after_the_app_replaced_its_directory_is_not_followed() {
    let (inside, outside, first) = granted("later");
    let inner = inside.join("inner");
    std::fs::create_dir_all(&inner).unwrap();
    call2(&first, FsOp::Rename, &inner, &inside.join("moved")).unwrap();
    let target = outside.display().to_string();
    call(&first, FsOp::Symlink, &inner, Some(target.as_bytes())).unwrap();
    let (at, deeper) = (inside.display(), inner.display());
    let later = format!("fs.write {at}\nfs.read {deeper}\nfs.write {deeper}");
    let later = GrantSet::parse(&later).unwrap();
    let _bindings = crate::host::Host::new().endow(later.clone());
    let got = call(&later, FsOp::ReadFile, &inner.join("secret"), None);
    assert!(got.is_err(), "read outside through the link: {got:?}");
    let made = call(&later, FsOp::WriteFile, &inner.join("made"), Some(b"x"));
    assert!(made.is_err(), "wrote through the link: {made:?}");
    assert!(!outside.join("made").exists());
}

/// Where the app's planting launch and its next one meet: the same grants,
/// spelt the same way, in two processes (`launch`), with and without an
/// `fs.write` line on the inner directory.
#[test]
fn a_link_the_app_planted_in_an_earlier_launch_is_not_followed() {
    for write in [false, true] {
        let (inside, outside, _) = granted(if write { "launch-write" } else { "launch" });
        std::fs::create_dir_all(inside.join("inner")).unwrap();
        for phase in ["plant", "use"] {
            let ran = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "stdlib::fs::native_tests::launch"])
                .args(["--ignored", "--nocapture", "--test-threads=1"])
                .env("IBEX2_LAUNCH", phase)
                .env("IBEX2_LAUNCH_DIR", &inside)
                .env("IBEX2_LAUNCH_WRITE", if write { "1" } else { "" })
                .output()
                .unwrap();
            let said = String::from_utf8_lossy(&ran.stdout);
            assert!(ran.status.success(), "{phase} (write {write}): {said}");
            assert!(said.contains("1 passed"), "{phase} did not run: {said}");
        }
        assert!(!outside.join("planted").exists(), "wrote through the link");
    }
}

/// One launch of the test above: `plant` replaces the inner directory with
/// a link to the outside one, `use` reads and writes through it.
#[test]
#[ignore = "a launch of a_link_the_app_planted_in_an_earlier_launch_is_not_followed"]
fn launch() {
    let inside = PathBuf::from(std::env::var_os("IBEX2_LAUNCH_DIR").unwrap());
    let (inner, outside) = (inside.join("inner"), inside.with_file_name("outside"));
    let (at, deeper) = (inside.display(), inner.display());
    let mut lines = format!("fs.write {at}\nfs.read {at}\nfs.read {deeper}");
    if std::env::var("IBEX2_LAUNCH_WRITE").is_ok_and(|w| !w.is_empty()) {
        lines.push_str(&format!("\nfs.write {deeper}"));
    }
    let grants = GrantSet::parse(&lines).unwrap();
    let _bindings = crate::host::Host::new().endow(grants.clone());
    if std::env::var("IBEX2_LAUNCH").unwrap() == "plant" {
        assert!(call(&grants, FsOp::ReadDir, &inner, None).is_ok());
        call2(&grants, FsOp::Rename, &inner, &inside.join("moved")).unwrap();
        let target = outside.display().to_string();
        call(&grants, FsOp::Symlink, &inner, Some(target.as_bytes())).unwrap();
        return;
    }
    let got = call(&grants, FsOp::ReadFile, &inner.join("secret"), None);
    assert!(got.is_err(), "read outside through the link: {got:?}");
    let made = call(&grants, FsOp::WriteFile, &inner.join("planted"), Some(b"x"));
    assert!(made.is_err(), "wrote through the link: {made:?}");
}

/// A write line naming a directory that does not exist yet, a read line
/// below it spelt in another case: once something other than the app makes
/// the directory, on a filesystem that ignores case, the read line's walk
/// stops there, as the directory is compared by identity.
#[test]
fn a_write_line_made_later_and_spelt_in_another_case_still_stops_the_walk() {
    let (inside, outside, _) = granted("case");
    let real = std::fs::canonicalize(&inside).unwrap();
    let (upper, lower) = (real.join("NEWDIR"), real.join("newdir"));
    let grants = GrantSet::parse(&format!(
        "fs.write {}\nfs.read {}",
        upper.display(),
        lower.join("inner").display()
    ))
    .unwrap();
    assert!(call(&grants, FsOp::Stat, &upper, None).is_err());
    std::fs::create_dir(&upper).unwrap();
    if !lower.exists() {
        return; // Case matters here, so the lines name two directories.
    }
    let target = outside.display().to_string();
    let link = upper.join("inner");
    call(&grants, FsOp::Symlink, &link, Some(target.as_bytes())).unwrap();
    let got = call(&grants, FsOp::ReadFile, &lower.join("inner/secret"), None);
    assert!(got.is_err(), "read outside through the link: {got:?}");
}
