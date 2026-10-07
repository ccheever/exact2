use super::*;
use crate::{
    grant::{Grant, PathPrefix},
    stdlib::fs::run,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

struct Fixture {
    root: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "ibex native café {}",
            crate::stdlib::crypto::random_uuid().unwrap()
        ));
        fs::create_dir_all(root.join("data")).unwrap();
        fs::create_dir_all(root.join("outside")).unwrap();
        Self {
            root: root.canonicalize().unwrap(),
        }
    }
    fn path(&self, name: &str) -> String {
        self.root.join(name).to_str().unwrap().to_owned()
    }
    fn grants(&self, read: &str, write: &str) -> GrantSet {
        GrantSet::none()
            .with(Grant::FsRead(PathPrefix::new(&self.path(read)).unwrap()))
            .with(Grant::FsWrite(PathPrefix::new(&self.path(write)).unwrap()))
    }
    fn call(
        &self,
        op: FsOp,
        name: &str,
        to: Option<&str>,
        data: Option<&[u8]>,
    ) -> Result<FsResult, HostError> {
        run(
            &self.grants("data", "data"),
            None,
            op,
            &self.path(name),
            to.map(|s| self.path(s)).as_deref(),
            data,
        )
    }
    fn pin(&self, name: &str) -> Target {
        let path = NativePath::parse(&self.path(name), true).unwrap();
        Target::pin(
            &Directory::native_drive(path.drive).unwrap(),
            &path.parts.iter().map(String::as_str).collect::<Vec<_>>(),
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        assert!(self
            .root
            .starts_with(std::env::temp_dir().canonicalize().unwrap()));
        fs::remove_dir_all(&self.root).unwrap();
    }
}
fn junction(target: &Path, link: &Path) {
    let output=Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command", "New-Item -ItemType Junction -Path $env:IBEX_TEST_JUNCTION_LINK -Value $env:IBEX_TEST_JUNCTION_TARGET -ErrorAction Stop | Out-Null"])
        .env("IBEX_TEST_JUNCTION_LINK",link).env("IBEX_TEST_JUNCTION_TARGET",target).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn drive_names_normalize_without_changing_authority_namespace() {
    assert_eq!(
        NativePath::parse(r"c:\Data\.\sub\..\file", true)
            .unwrap()
            .spelling(),
        "C:/Data/file"
    );
    let grant = PathPrefix::new(r"C:\Data\one").unwrap();
    for path in [
        "C:/Data/one",
        r"c:\Data\one",
        r"\\?\C:\Data\one",
        r"C:\Data\one\child",
    ] {
        assert!(grant.covers(path), "{path}");
    }
    for path in [
        "D:/Data/one",
        "C:/data/one",
        "C:/Data/ONE",
        "C:/Data/one-more",
        "/Data/one",
        "app:/Data/one",
    ] {
        assert!(!grant.covers(path), "{path}");
    }
    for path in [
        r"C:relative",
        "C:",
        r"\Data",
        "/Data",
        r"\\server\share\x",
        r"\\?\UNC\server\share\x",
        r"\\.\C:\Data",
        r"\??\C:\Data",
        r"\\?\GLOBALROOT\Device\x",
        r"\\?\Volume{abc}\x",
        r"C:\..\x",
        r"C:\Data\NUL.txt",
        r"C:\Data\x:stream",
        r"C:\Data\x.",
        r"C:\Data\x ",
        "C:/Data/x\0y",
        r"C:\Data\*",
        r"C:\Data\LPT¹.txt",
        r#"C:\Data\a"b"#,
        "C:/Data/tab\tname",
        "C:/Data/del\u{7f}name",
        "C:/Data/control\u{85}name",
    ] {
        assert!(NativePath::parse(path, true).is_err(), "{path:?}");
    }
    assert!(PathPrefix::new(r"C:\Data\..\Other").is_none());
    assert!(NativePath::parse(&format!("C:/{}", "x".repeat(256)), true).is_err());
    assert!(!PathPrefix::new("app:/").unwrap().covers(r"C:\data"));
    assert!(!PathPrefix::new("/").unwrap().covers(r"C:\data"));
    let posix = GrantSet::parse("fs.read /\nfs.write /\n").unwrap();
    assert_eq!(crate::grant::realized_fs(&posix), posix);
    assert!(!crate::grant::realized_fs(&posix).permits(&Operation::FsRead { path: "C:/".into() }));
}

#[test]
fn every_native_operation_round_trips_with_spaces_unicode_and_canonical_paths() {
    let f = Fixture::new();
    f.call(FsOp::Mkdir, "data/雪/deep", None, None).unwrap();
    let name = "data/雪/deep/été.txt";
    f.call(FsOp::WriteFile, name, None, Some(b"one")).unwrap();
    f.call(FsOp::AppendFile, name, None, Some(b"two")).unwrap();
    assert_eq!(
        f.call(FsOp::ReadFile, name, None, None).unwrap(),
        FsResult::Bytes(b"onetwo".to_vec())
    );
    f.call(FsOp::AtomicWriteFile, name, None, Some(b"whole"))
        .unwrap();
    f.call(FsOp::CopyFile, name, Some("data/copy"), None)
        .unwrap();
    f.call(FsOp::Rename, "data/copy", Some("data/moved"), None)
        .unwrap();
    let FsResult::Stat(stat) = f.call(FsOp::Stat, "data/moved", None, None).unwrap() else {
        panic!()
    };
    assert!(stat.is_file);
    assert_eq!(stat.size, 5);
    assert_eq!(
        f.call(FsOp::ReadDir, "data", None, None).unwrap(),
        FsResult::Names(vec!["moved".into(), "雪".into()])
    );
    let FsResult::Text(real) = f.call(FsOp::Realpath, "data/moved", None, None).unwrap() else {
        panic!()
    };
    assert_eq!(Path::new(&real), f.root.join("data/moved"));
    f.call(FsOp::Remove, "data/雪", None, None).unwrap();
    f.call(FsOp::Remove, "data/雪/deep/missing", None, None)
        .unwrap();
}

#[test]
fn least_authority_checks_both_operands_and_never_creates_ungranted_ancestors() {
    let f = Fixture::new();
    fs::write(f.root.join("data/source"), b"secret").unwrap();
    let grants = f.grants("data/source", "data/allowed");
    assert!(run(
        &grants,
        None,
        FsOp::ReadFile,
        &f.path("outside/missing"),
        None,
        None
    )
    .is_err());
    assert!(run(
        &grants,
        None,
        FsOp::WriteFile,
        &f.path("data/allowed-more"),
        None,
        Some(b"bad")
    )
    .is_err());
    run(
        &grants,
        None,
        FsOp::CopyFile,
        &f.path("data/source"),
        Some(&f.path("data/allowed")),
        None,
    )
    .unwrap();
    assert!(run(
        &grants,
        None,
        FsOp::ReadFile,
        &f.path("data/allowed"),
        None,
        None
    )
    .is_err());
    assert!(run(
        &grants,
        None,
        FsOp::Rename,
        &f.path("data/source"),
        Some(&f.path("data/allowed")),
        None
    )
    .is_err());
    assert!(f
        .call(FsOp::ReadFile, "data/../outside/missing", None, None)
        .is_err());
    let narrow = f.grants("data/source", "missing/leaf");
    assert!(run(
        &narrow,
        None,
        FsOp::Mkdir,
        &f.path("missing/leaf/descendant"),
        None,
        None
    )
    .is_err());
    assert!(!f.root.join("missing").exists());
    let narrow = f.grants("data/source", "data/new");
    run(
        &narrow,
        None,
        FsOp::Mkdir,
        &f.path("data/new/deep"),
        None,
        None,
    )
    .unwrap();
    assert!(f.root.join("data/new/deep").is_dir());
    assert!(run(
        &GrantSet::none(),
        None,
        FsOp::Mkdir,
        "Z:/unavailable/no-authority",
        None,
        None
    )
    .unwrap_err()
    .to_string()
    .contains("denied"));
}

#[test]
fn drive_root_can_be_read_but_not_removed_or_renamed() {
    let f = Fixture::new();
    let p = NativePath::parse(&f.path("data"), true).unwrap();
    let root = format!("{}:/", p.drive as char);
    let grants = GrantSet::parse(&format!("fs.read {root}\nfs.write {root}")).unwrap();
    let FsResult::Stat(stat) = run(&grants, None, FsOp::Stat, &root, None, None).unwrap() else {
        panic!()
    };
    assert!(stat.is_directory);
    assert!(run(&grants, None, FsOp::Remove, &root, None, None).is_err());
    assert!(run(
        &grants,
        None,
        FsOp::Rename,
        &root,
        Some(&f.path("data/new")),
        None
    )
    .is_err());
}

#[test]
fn native_sqlite_grant_syntax_does_not_enable_native_sqlite_execution() {
    let f = Fixture::new();
    let path = NativePath::parse(&f.path("data/db.sqlite"), false)
        .unwrap()
        .spelling();
    let grants = GrantSet::none().with(Grant::SqliteOpen(PathPrefix::new(&path).unwrap()));
    let error = crate::stdlib::app_fs::resolve_sqlite(&grants, None, &path).unwrap_err();
    assert!(error
        .to_string()
        .contains("native Windows SQLite grants are not supported"));
    assert!(!f.root.join("data/db.sqlite").exists());
    // The shared path constructor admits the syntax even for sqlite.open;
    // granting it still cannot reach an unqualified native executor.
    assert!(GrantSet::parse("sqlite.open C:/data/db.sqlite").is_ok());
}

#[test]
fn copy_refuses_directory_self_and_hardlink_without_target_damage() {
    let f = Fixture::new();
    assert!(f
        .call(FsOp::CopyFile, "data", Some("data/absent"), None)
        .is_err());
    assert!(!f.root.join("data/absent").exists());
    fs::write(f.root.join("data/file"), b"keep").unwrap();
    assert!(f
        .call(FsOp::CopyFile, "data", Some("data/file"), None)
        .is_err());
    assert!(f
        .call(FsOp::CopyFile, "data/file", Some("data/file"), None)
        .is_err());
    fs::hard_link(f.root.join("data/file"), f.root.join("data/alias")).unwrap();
    assert!(f
        .call(FsOp::CopyFile, "data/file", Some("data/alias"), None)
        .is_err());
    assert_eq!(fs::read(f.root.join("data/file")).unwrap(), b"keep");
}

#[test]
fn ordinary_case_lookup_does_not_widen_exact_lexical_grants() {
    let f = Fixture::new();
    fs::write(f.root.join("data/Exact.txt"), b"one").unwrap();
    let grants = f.grants("data/exact.txt", "data/exact.txt");
    assert_eq!(
        run(
            &grants,
            None,
            FsOp::ReadFile,
            &f.path("data/exact.txt"),
            None,
            None
        )
        .unwrap(),
        FsResult::Bytes(b"one".to_vec())
    );
    assert!(run(
        &grants,
        None,
        FsOp::ReadFile,
        &f.path("data/Exact.txt"),
        None,
        None
    )
    .is_err());
}

#[test]
fn an_available_short_name_needs_its_own_lexical_grant() {
    use windows_sys::Win32::Storage::FileSystem::GetShortPathNameW;
    let f = Fixture::new();
    fs::write(f.root.join("data/file.txt"), b"same object").unwrap();
    let long = f.path("data/file.txt");
    let input: Vec<_> = long.encode_utf16().chain(Some(0)).collect();
    let mut output = vec![0; input.len() + 1];
    // SAFETY: input is terminated and output is writable for its declared size.
    let length =
        unsafe { GetShortPathNameW(input.as_ptr(), output.as_mut_ptr(), output.len() as u32) }
            as usize;
    assert!(
        length > 0 && length < output.len(),
        "{}",
        io::Error::last_os_error()
    );
    let short = String::from_utf16(&output[..length]).unwrap();
    if short == long {
        eprintln!("8.3 alias unavailable on this volume; no alias qualification claimed");
        return;
    }
    let grants = GrantSet::none().with(Grant::FsRead(PathPrefix::new(&short).unwrap()));
    assert_eq!(
        run(&grants, None, FsOp::ReadFile, &short, None, None).unwrap(),
        FsResult::Bytes(b"same object".to_vec())
    );
    assert!(!grants.permits(&Operation::FsRead { path: long }));
}

#[test]
fn distinct_case_files_remain_separate_on_a_sensitive_directory() {
    let f = Fixture::new();
    let output = Command::new("fsutil.exe")
        .args(["file", "setCaseSensitiveInfo"])
        .arg(f.root.join("data"))
        .arg("enable")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "case-sensitive fixture prerequisite: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    fs::write(f.root.join("data/Exact.txt"), b"one").unwrap();
    fs::write(f.root.join("data/exact.txt"), b"two").unwrap();
    let grants = f.grants("data/exact.txt", "data/exact.txt");
    assert_eq!(
        run(
            &grants,
            None,
            FsOp::ReadFile,
            &f.path("data/exact.txt"),
            None,
            None
        )
        .unwrap(),
        FsResult::Bytes(b"two".to_vec())
    );
    assert!(run(
        &grants,
        None,
        FsOp::ReadFile,
        &f.path("data/Exact.txt"),
        None,
        None
    )
    .is_err());
    run(
        &grants,
        None,
        FsOp::WriteFile,
        &f.path("data/exact.txt"),
        None,
        Some(b"changed"),
    )
    .unwrap();
    assert_eq!(fs::read(f.root.join("data/Exact.txt")).unwrap(), b"one");
    run(
        &grants,
        None,
        FsOp::AtomicWriteFile,
        &f.path("data/exact.txt"),
        None,
        Some(b"atomic"),
    )
    .unwrap();
    assert_eq!(fs::read(f.root.join("data/exact.txt")).unwrap(), b"atomic");
    assert_eq!(fs::read(f.root.join("data/Exact.txt")).unwrap(), b"one");
    fs::write(f.root.join("data/source"), b"renamed").unwrap();
    let rename_grants = grants
        .with(Grant::FsRead(
            PathPrefix::new(&f.path("data/source")).unwrap(),
        ))
        .with(Grant::FsWrite(
            PathPrefix::new(&f.path("data/source")).unwrap(),
        ));
    run(
        &rename_grants,
        None,
        FsOp::Rename,
        &f.path("data/source"),
        Some(&f.path("data/exact.txt")),
        None,
    )
    .unwrap();
    assert_eq!(fs::read(f.root.join("data/exact.txt")).unwrap(), b"renamed");
    assert_eq!(fs::read(f.root.join("data/Exact.txt")).unwrap(), b"one");
    assert_eq!(
        f.call(FsOp::ReadFile, "data/Exact.txt", None, None)
            .unwrap(),
        FsResult::Bytes(b"one".to_vec())
    );
}

#[test]
fn reparse_roots_intermediates_and_final_names_are_refused() {
    let f = Fixture::new();
    fs::write(f.root.join("outside/secret"), b"outside").unwrap();
    let link = f.root.join("data/link");
    junction(&f.root.join("outside"), &link);
    assert!(f
        .call(FsOp::ReadFile, "data/link/secret", None, None)
        .is_err());
    assert!(f
        .call(FsOp::WriteFile, "data/link/new", None, Some(b"bad"))
        .is_err());
    assert!(f.call(FsOp::Stat, "data/link", None, None).is_err());
    let root_grant = f.grants("data/link", "data/link");
    assert!(run(
        &root_grant,
        None,
        FsOp::ReadFile,
        &f.path("data/link/secret"),
        None,
        None
    )
    .is_err());
    assert!(!f.root.join("outside/new").exists());
    fs::remove_dir(&link).unwrap();
}

#[test]
fn pinned_parent_and_both_operands_survive_path_replacement() {
    let f = Fixture::new();
    fs::write(f.root.join("data/source"), b"inside").unwrap();
    let source = f.pin("data/source");
    let target = f.pin("data/copy");
    fs::rename(f.root.join("data"), f.root.join("moved")).unwrap();
    junction(&f.root.join("outside"), &f.root.join("data"));
    assert_eq!(
        perform(FsOp::ReadFile, &source, None, None).unwrap(),
        FsResult::Bytes(b"inside".to_vec())
    );
    perform(FsOp::CopyFile, &source, Some(&target), None).unwrap();
    assert_eq!(fs::read(f.root.join("moved/copy")).unwrap(), b"inside");
    assert!(!f.root.join("outside/copy").exists());
    assert!(f.call(FsOp::ReadFile, "data/source", None, None).is_err());
    fs::remove_dir(f.root.join("data")).unwrap();
}

#[test]
fn a_final_name_replaced_by_a_junction_cannot_redirect_reads_or_writes() {
    let f = Fixture::new();
    fs::write(f.root.join("data/file"), b"inside").unwrap();
    let target = f.pin("data/file");
    fs::remove_file(f.root.join("data/file")).unwrap();
    junction(&f.root.join("outside"), &f.root.join("data/file"));
    assert!(perform(FsOp::ReadFile, &target, None, None).is_err());
    assert!(perform(FsOp::WriteFile, &target, None, Some(b"bad")).is_err());
    assert!(perform(FsOp::AtomicWriteFile, &target, None, Some(b"bad")).is_err());
    fs::remove_dir(f.root.join("data/file")).unwrap();
}

#[test]
fn rename_keeps_both_distinct_parents_after_destination_replacement() {
    let f = Fixture::new();
    fs::create_dir(f.root.join("data/from")).unwrap();
    fs::create_dir(f.root.join("data/to")).unwrap();
    fs::write(f.root.join("data/from/file"), b"inside").unwrap();
    let source = f.pin("data/from/file");
    let target = f.pin("data/to/file");
    fs::rename(f.root.join("data/to"), f.root.join("destination-moved")).unwrap();
    junction(&f.root.join("outside"), &f.root.join("data/to"));
    perform(FsOp::Rename, &source, Some(&target), None).unwrap();
    assert_eq!(
        fs::read(f.root.join("destination-moved/file")).unwrap(),
        b"inside"
    );
    assert!(!f.root.join("data/from/file").exists());
    assert!(!f.root.join("outside/file").exists());
    fs::remove_dir(f.root.join("data/to")).unwrap();
}
