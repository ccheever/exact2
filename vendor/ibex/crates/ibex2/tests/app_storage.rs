//! A Rust app uses the same mounted paths and SQLite provider with no engine.
struct Project(std::path::PathBuf);
impl Project {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "ibex2-{name}-{}",
            ibex2::stdlib::crypto::random_uuid().unwrap()
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root)
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

use ibex2::{
    grant::GrantSet,
    host::Host,
    stdlib::{app_fs::AppDirectories, sqlite::Value},
};
use std::sync::Arc;

fn directories(project: &Project) -> AppDirectories {
    for name in ["data", "cache", "tmp"] {
        std::fs::create_dir_all(project.0.join(name)).unwrap();
    }
    AppDirectories::new(
        project.0.join("data"),
        project.0.join("cache"),
        project.0.join("tmp"),
    )
    .unwrap()
}
fn host(project: &Project) -> Host {
    Host::new()
        .with_app_directories(directories(project))
        .with_sqlite_provider(Arc::new(ibex2_sqlite::SqliteProvider))
}
#[test]
fn app_mounts_and_sqlite_persist_under_separate_grants_without_an_engine() {
    let first = Project::new("rust-storage-a");
    let second = Project::new("rust-storage-b");
    let grants = || {
        GrantSet::parse("fs.read app:/data/settings\nfs.write app:/data/settings\nsqlite.open app:/data/app.db\n").unwrap()
    };
    let a = host(&first).endow(grants());
    let b = host(&second).endow(grants());
    a.fs.atomic_write_file("app:/data/settings", b"first app")
        .unwrap();
    b.fs.atomic_write_file("app:/data/settings", b"second app")
        .unwrap();
    assert_eq!(a.fs.read_file("app:/data/settings").unwrap(), b"first app");
    assert_eq!(b.fs.read_file("app:/data/settings").unwrap(), b"second app");
    assert!(a.fs.read_file("app:/data/app.db").is_err());
    assert!(a.sqlite.open("app:/cache/app.db").is_err());
    let db = a.sqlite.open("app:/data/app.db").unwrap();
    db.execute("CREATE TABLE notes (body TEXT)", &[]).unwrap();
    let insert = db.prepare("INSERT INTO notes VALUES (?)").unwrap();
    insert
        .execute(&[Value::Text("saved by Rust".into())])
        .unwrap();
    insert.close();
    db.close().unwrap();
    drop(a);
    let reopened = host(&first)
        .endow(grants())
        .sqlite
        .open("app:/data/app.db")
        .unwrap();
    assert_eq!(
        reopened.query("SELECT body FROM notes", &[]).unwrap().rows,
        vec![vec![Value::Text("saved by Rust".into())]]
    );
    reopened.close().unwrap();
}
#[test]
fn configuration_and_provider_are_explicit_and_denials_create_nothing() {
    let project = Project::new("rust-storage-denial");
    let dirs = directories(&project);
    let no_mount = Host::new().endow(GrantSet::parse("fs.write app:/data\n").unwrap());
    assert!(no_mount.fs.write_file("app:/data/file", b"no").is_err());
    let no_provider = Host::new()
        .with_app_directories(dirs.clone())
        .endow(GrantSet::parse("sqlite.open app:/data\n").unwrap());
    assert!(no_provider
        .sqlite
        .open("app:/data/missing.db")
        .err()
        .unwrap()
        .to_string()
        .contains("provider"));
    let denied = Host::new()
        .with_app_directories(dirs)
        .with_sqlite_provider(Arc::new(ibex2_sqlite::SqliteProvider))
        .endow(GrantSet::none());
    assert!(denied
        .sqlite
        .open("app:/data/missing.db")
        .err()
        .unwrap()
        .to_string()
        .contains("denied"));
    assert!(!project.0.join("data/missing.db").exists());
}
