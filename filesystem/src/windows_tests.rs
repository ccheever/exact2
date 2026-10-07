// @ref LLP 1030.002 D4 — these run without symlink privilege or Developer Mode.
use super::tests::{Fixture, TOKEN};
use super::*;
use std::fs;
use std::path::Path;
use std::process::Command;

fn junction(target: &Path, link: &Path) {
    // Paths travel through environment values, never parsed PowerShell source.
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command",
            "New-Item -ItemType Junction -Path $env:EXACT_TEST_JUNCTION_LINK -Value $env:EXACT_TEST_JUNCTION_TARGET -ErrorAction Stop | Out-Null"])
        .env("EXACT_TEST_JUNCTION_TARGET", target)
        .env("EXACT_TEST_JUNCTION_LINK", link)
        .output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn unicode_tree_copy_and_immutable_bytes() {
    let f = Fixture::new();
    let root = f.dir("café assets");
    let (dir, leaf) = root.parent("textures 雪/été.bin", true).unwrap();
    dir.write(&leaf, b"\0\x01\xffpixels", true, TOKEN).unwrap();
    assert_eq!(
        dir.write(&leaf, b"\0\x01\xffpixels", true, TOKEN).unwrap(),
        "present"
    );
    assert!(dir.write(&leaf, b"other", true, TOKEN).is_err());
    let tree = operate(&root, &json!({"op":"tree"}), None).unwrap();
    assert_eq!(
        tree["textures 雪/été.bin"],
        STANDARD.encode(b"\0\x01\xffpixels")
    );
    operate(
        &root,
        &json!({"op":"copy", "target":f.path("copied café"), "token":TOKEN}),
        None,
    )
    .unwrap();
    assert_eq!(
        fs::read(f.path("copied café/textures 雪/été.bin")).unwrap(),
        b"\0\x01\xffpixels"
    );
    assert_eq!(dir.names().unwrap(), vec!["été.bin"]);
    assert_eq!(dir.names().unwrap(), vec!["été.bin"]);
}

#[test]
fn directory_enumeration_crosses_buffers_and_restarts() {
    let f = Fixture::new();
    let root = f.dir("many files");
    let expected: Vec<_> = (0..600)
        .map(|index| format!("{index:04}-{}.bin", "雪".repeat(60)))
        .collect();
    for name in &expected {
        fs::write(f.path("many files").join(name), b"payload").unwrap();
    }
    // More than two 64 KiB native enumeration buffers, with variable UTF-16 names.
    assert_eq!(root.names().unwrap(), expected);
    assert_eq!(root.names().unwrap(), expected);
}

#[test]
fn invalid_windows_names_fail_before_io() {
    let f = Fixture::new();
    let root = f.dir("root");
    for name in [
        "", ".", "..", "a/b", "a\\b", "a:stream", "name.", "name ", "NUL", "con.txt", "COM1",
        "lpt²", "a\0b",
    ] {
        assert!(root.read(name).is_err(), "read {name:?}");
        assert!(
            root.write(name, b"wrong", false, TOKEN).is_err(),
            "write {name:?}"
        );
    }
    for path in ["../escape", "/root", "a//b", "a/NUL", "a/./b", "a\0b"] {
        assert!(root.parent(path, true).is_err(), "parent {path:?}");
    }
    assert!(root.names().unwrap().is_empty());
    for path in [
        "C:relative",
        "\\\\server\\share",
        "\\\\?\\C:\\",
        "\\\\.\\C:\\",
    ] {
        assert!(Directory::root(path, false).is_err());
    }
}

#[test]
fn junctions_are_refused_at_root_intermediate_and_leaf() {
    let f = Fixture::new();
    let root = f.dir("assets");
    let outside = f.dir("outside");
    outside.write("secret", b"private", false, TOKEN).unwrap();
    junction(&f.path("outside"), &f.path("assets/linked"));
    assert!(Directory::root(f.path("assets/linked").to_str().unwrap(), false).is_err());
    assert!(root.parent("linked/secret", false).is_err());
    assert!(root.read("linked").is_err());
    assert!(root.write("linked", b"wrong", false, TOKEN).is_err());
    assert!(operate(&root, &json!({"op":"tree"}), None).is_err());
    assert_eq!(outside.read("secret").unwrap(), b"private");
}

#[test]
fn held_parent_survives_rename_and_junction_replacement() {
    let f = Fixture::new();
    let root = f.dir("root");
    let outside = f.dir("outside");
    outside.write("secret", b"private", false, TOKEN).unwrap();
    let (parent, leaf) = root.parent("nested/value", true).unwrap();
    fs::rename(f.path("root/nested"), f.path("root/held")).unwrap();
    junction(&f.path("outside"), &f.path("root/nested"));
    parent.write(&leaf, b"public", false, TOKEN).unwrap();
    assert_eq!(parent.names().unwrap(), vec!["value"]);
    assert_eq!(fs::read(f.path("root/held/value")).unwrap(), b"public");
    assert!(root.parent("nested/secret", false).is_err());
    assert!(!f.path("outside/value").exists());
}

#[test]
fn final_file_replaced_by_junction_refuses_read_and_commit() {
    let f = Fixture::new();
    let root = f.dir("root");
    let outside = f.dir("outside");
    outside.write("secret", b"private", false, TOKEN).unwrap();
    root.write("value", b"original", false, TOKEN).unwrap();
    let result = root.write_checked("value", b"candidate", false, TOKEN, || {
        fs::remove_file(f.path("root/value"))?;
        junction(&f.path("outside"), &f.path("root/value"));
        assert!(root.read("value").is_err());
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(outside.read("secret").unwrap(), b"private");
    assert_eq!(root.names().unwrap(), vec!["value"]);
}

#[test]
fn locks_exclude_replaced_owners_and_durable_heads_refuse() {
    let f = Fixture::new();
    let root = f.dir("root");
    let lock = Lock::acquire(&root, ".retained/.lock", TOKEN).unwrap();
    assert!(lock.verify(&root).is_ok());
    assert!(Lock::acquire(&root, ".retained/.lock", TOKEN).is_err());
    fs::rename(f.path("root/.retained/.lock"), f.path("root/old-lock")).unwrap();
    let successor = Lock::acquire(&root, ".retained/.lock", "successor").unwrap();
    assert!(lock.verify(&root).is_err());
    drop(lock);
    assert!(successor.verify(&root).is_ok());
    let error = operate(&root, &json!({"op":"head"}), None).unwrap_err();
    assert!(error.to_string().contains("not qualified on Windows"));
    drop(successor);
    let lock = Lock::acquire(&root, ".retained/.lock", TOKEN).unwrap();
    match fs::rename(f.path("root/.retained"), f.path("root/old-dir")) {
        Ok(()) => {
            let _other = Lock::acquire(&root, ".retained/.lock", TOKEN).unwrap();
            assert!(lock.verify(&root).is_err());
        }
        // NTFS can refuse renaming a directory with an open locked child.
        Err(error) => {
            assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
            assert!(lock.verify(&root).is_ok());
        }
    }
}

#[test]
fn resident_reads_reopen_roots_and_disallow_mutations() {
    let f = Fixture::new();
    f.dir("root")
        .write("value", b"first", false, TOKEN)
        .unwrap();
    let request = json!({"op":"get", "root":f.path("root"), "path":"value"});
    assert_eq!(read_request(&request).unwrap(), STANDARD.encode("first"));
    fs::rename(f.path("root"), f.path("old")).unwrap();
    f.dir("root")
        .write("value", b"second", false, TOKEN)
        .unwrap();
    assert_eq!(read_request(&request).unwrap(), STANDARD.encode("second"));
    assert!(read_request(&json!({"op":"put"})).is_err());
}
