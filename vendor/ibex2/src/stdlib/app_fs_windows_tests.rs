use super::*;
use crate::stdlib::fs::run;
use std::{fs, process::Command};

struct Fixture {
    dirs: AppDirectories,
    path: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "ibex café fs-{}",
            crate::stdlib::crypto::random_uuid().unwrap()
        ));
        for part in ["data", "cache", "tmp"] {
            fs::create_dir_all(path.join(part)).unwrap();
        }
        let dirs =
            AppDirectories::new(path.join("data"), path.join("cache"), path.join("tmp")).unwrap();
        Self { dirs, path }
    }
    fn call(
        &self,
        op: FsOp,
        path: &str,
        destination: Option<&str>,
        data: Option<&[u8]>,
    ) -> Result<FsResult, HostError> {
        run(
            &GrantSet::parse("fs.read app:/\nfs.write app:/").unwrap(),
            Some(&self.dirs),
            op,
            path,
            destination,
            data,
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.path).unwrap();
    }
}
fn junction(target: &Path, link: &Path) {
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", "New-Item -ItemType Junction -Path $env:IBEX_TEST_JUNCTION_LINK -Value $env:IBEX_TEST_JUNCTION_TARGET -ErrorAction Stop | Out-Null"])
        .env("IBEX_TEST_JUNCTION_LINK", link).env("IBEX_TEST_JUNCTION_TARGET", target).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn every_operation_round_trips_with_unicode_and_atomic_replacement() {
    let f = Fixture::new();
    f.call(FsOp::Mkdir, "app:/data/deep/雪", None, None)
        .unwrap();
    let path = "app:/data/deep/雪/été";
    f.call(FsOp::WriteFile, path, None, Some(b"one")).unwrap();
    f.call(FsOp::AppendFile, path, None, Some(b"two")).unwrap();
    assert_eq!(
        f.call(FsOp::ReadFile, path, None, None).unwrap(),
        FsResult::Bytes(b"onetwo".to_vec())
    );
    f.call(FsOp::AtomicWriteFile, path, None, Some(b"whole"))
        .unwrap();
    f.call(FsOp::CopyFile, path, Some("app:/cache/copy"), None)
        .unwrap();
    f.call(
        FsOp::Rename,
        "app:/cache/copy",
        Some("app:/tmp/moved"),
        None,
    )
    .unwrap();
    assert_eq!(
        f.call(FsOp::Realpath, "app:/tmp//moved", None, None)
            .unwrap(),
        FsResult::Text("app:/tmp/moved".into())
    );
    assert_eq!(
        f.call(FsOp::ReadDir, "app:/tmp", None, None).unwrap(),
        FsResult::Names(vec!["moved".into()])
    );
    let FsResult::Stat(stat) = f.call(FsOp::Stat, "app:/tmp/moved", None, None).unwrap() else {
        panic!()
    };
    assert!(stat.is_file);
    assert_eq!(stat.size, 5);
    f.call(FsOp::Remove, "app:/data/deep", None, None).unwrap();
    f.call(FsOp::Remove, "app:/data/deep", None, None).unwrap();
    assert_eq!(
        f.call(FsOp::ReadDir, "app:/data", None, None).unwrap(),
        FsResult::Names(vec![])
    );
}

#[test]
fn grants_and_names_are_checked_before_mutating_the_filesystem() {
    let f = Fixture::new();
    let grants = GrantSet::parse("fs.read app:/data\nfs.write app:/cache").unwrap();
    for path in [
        "app:/data/../cache/x",
        "app:/data/./x",
        "app:/data/x:stream",
        "app:/data/NUL",
        "app:/data/x.",
        "app:/data/x ",
        "app:/data/x\\y",
        "app:/data/x\0y",
    ] {
        assert!(
            f.call(FsOp::WriteFile, path, None, Some(b"bad")).is_err(),
            "{path}"
        );
    }
    assert!(run(
        &GrantSet::none(),
        Some(&f.dirs),
        FsOp::Mkdir,
        "app:/data/new",
        None,
        None
    )
    .is_err());
    assert!(!f.path.join("data/new").exists());
    f.call(FsOp::WriteFile, "app:/data/a", None, Some(b"keep"))
        .unwrap();
    assert!(run(
        &grants,
        Some(&f.dirs),
        FsOp::Rename,
        "app:/data/a",
        Some("app:/cache/a"),
        None
    )
    .is_err());
    run(
        &grants,
        Some(&f.dirs),
        FsOp::CopyFile,
        "app:/data/a",
        Some("app:/cache/a"),
        None,
    )
    .unwrap();
    assert!(f
        .call(FsOp::CopyFile, "app:/data/a", Some("app:/data/a"), None)
        .is_err());
    fs::hard_link(f.path.join("data/a"), f.path.join("cache/alias")).unwrap();
    assert!(f
        .call(
            FsOp::CopyFile,
            "app:/data/a",
            Some("app:/cache/alias"),
            None
        )
        .is_err());
    assert_eq!(fs::read(f.path.join("data/a")).unwrap(), b"keep");
    assert!(f
        .call(FsOp::Rename, "app:/data/a", Some("C:/native"), None)
        .is_err());
    assert!(AppDirectories::new("relative", "relative", "relative").is_err());
}

#[test]
fn directory_copy_refusal_never_creates_or_truncates_its_destination() {
    let f = Fixture::new();
    fs::write(f.path.join("cache/existing"), b"keep").unwrap();
    for target in ["absent", "existing"] {
        assert!(f
            .call(
                FsOp::CopyFile,
                "app:/data",
                Some(&format!("app:/cache/{target}")),
                None
            )
            .is_err());
    }
    assert!(!f.path.join("cache/absent").exists());
    assert_eq!(fs::read(f.path.join("cache/existing")).unwrap(), b"keep");
}

#[test]
fn reparse_substitution_never_supplies_or_receives_app_bytes() {
    let f = Fixture::new();
    fs::write(f.path.join("cache/secret"), b"private").unwrap();
    junction(&f.path.join("cache"), &f.path.join("data/link"));
    assert!(AppDirectories::new(
        f.path.join("data/link"),
        f.path.join("cache"),
        f.path.join("tmp")
    )
    .is_err());
    for op in [
        FsOp::ReadFile,
        FsOp::WriteFile,
        FsOp::AppendFile,
        FsOp::AtomicWriteFile,
        FsOp::Stat,
        FsOp::Remove,
        FsOp::ReadDir,
        FsOp::Mkdir,
        FsOp::Realpath,
    ] {
        assert!(
            f.call(op, "app:/data/link/secret", None, Some(b"bad"))
                .is_err(),
            "{op:?}"
        );
        assert!(
            f.call(op, "app:/data/link", None, Some(b"bad")).is_err(),
            "{op:?}"
        );
    }
    assert_eq!(fs::read(f.path.join("cache/secret")).unwrap(), b"private");
    fs::rename(f.path.join("data"), f.path.join("old-data")).unwrap();
    junction(&f.path.join("cache"), &f.path.join("data"));
    f.call(FsOp::WriteFile, "app:/data/new", None, Some(b"pinned"))
        .unwrap();
    assert_eq!(fs::read(f.path.join("old-data/new")).unwrap(), b"pinned");
    assert!(!f.path.join("cache/new").exists());
    assert!(f.dirs.sqlite_path("app:/data/new.db").is_err());
}

#[test]
fn final_replacement_during_atomic_commit_is_refused() {
    let f = Fixture::new();
    f.call(FsOp::WriteFile, "app:/data/a", None, Some(b"old"))
        .unwrap();
    let (parent, leaf) = f.dirs.parent("app:/data/a").unwrap();
    let result = parent.write_checked(&leaf, b"new", &token().unwrap(), || {
        fs::remove_file(f.path.join("data/a"))?;
        junction(&f.path.join("cache"), &f.path.join("data/a"));
        Ok(())
    });
    assert!(result.is_err());
    assert!(fs::read_dir(f.path.join("cache")).unwrap().next().is_none());
}

#[test]
fn sqlite_admission_refuses_reparse_databases_and_every_sidecar() {
    let f = Fixture::new();
    let grants = GrantSet::parse("sqlite.open app:/data").unwrap();
    assert!(resolve_sqlite(&GrantSet::none(), Some(&f.dirs), "app:/data/db").is_err());
    assert!(resolve_sqlite(&grants, Some(&f.dirs), "app:/cache/db").is_err());
    assert!(resolve_sqlite(&grants, Some(&f.dirs), "app:/data/db").is_ok());
    for (index, suffix) in ["", "-journal", "-wal", "-shm"].iter().enumerate() {
        let leaf = format!("db{index}{suffix}");
        junction(&f.path.join("cache"), &f.path.join("data").join(leaf));
        assert!(resolve_sqlite(&grants, Some(&f.dirs), &format!("app:/data/db{index}")).is_err());
    }
}
