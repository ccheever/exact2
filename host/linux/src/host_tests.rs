
use super::*;

/// A named agent drive keeps `secret.keep` in its scratch tree, and a
/// fresh launch reads nothing back (platformer R10). A child, so the
/// environment stays off this process.
#[test]
fn named_agent_storage_keeps_a_secret_across_a_relaunch() {
    const CHILD: &str = "EXACT_LINUX_SECRET_KEEP_TEST";
    if std::env::var_os(CHILD).is_none() {
        let home = std::env::temp_dir().join(format!("exact-linux-secrets-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "host::tests::named_agent_storage_keeps_a_secret_across_a_relaunch",
            ])
            .env(CHILD, "1")
            .env("HOME", &home)
            .env("XDG_CACHE_HOME", &home)
            .env("EXACT_AGENT", "1")
            .env("EXACT_AGENT_STORAGE", "s1")
            .env_remove("EXACT_AGENT_STORAGE_FRESH")
            .env_remove("EXACT_STORE")
            .output()
            .unwrap();
        let _ = std::fs::remove_dir_all(&home);
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let app = "test.exact.keep";
    let write = |name: &str, value: Option<&str>| exact_runner::StoreWrite {
        name: name.into(),
        value: value.map(str::to_string),
    };
    assert!(persist_agent_writes(app, &[write("platformer.best", Some("22050"))]).is_empty());
    assert!(persist_agent_writes(app, &[write("exact.kept.score", Some("9"))]).is_empty());
    let again = agent_store_snapshot(app, false);
    assert!(
        again.contains(&("platformer.best".into(), "22050".into())),
        "{again:?}"
    );
    assert!(
        again.contains(&("exact.kept.score".into(), "9".into())),
        "{again:?}"
    );
    let file = crate::picker::agent_secret_root(app)
        .unwrap()
        .join("secrets")
        .join("platformer.best");
    assert!(file.is_file(), "{}", file.display());
    // A carried reload does not wipe, even while a fresh flag is set.
    std::env::set_var("EXACT_AGENT_STORAGE_FRESH", "1");
    assert!(agent_store_snapshot(app, true).is_empty());
    assert!(file.is_file(), "a carried reload leaves the scratch files");
    assert!(
        agent_store_snapshot(app, false).is_empty(),
        "a fresh launch reads nothing"
    );
    assert!(!file.exists());
    // A commit after that boot outlives the activation's storage
    // configuration and a second boot here: the tree is emptied once.
    assert!(persist_agent_writes(app, &[write("platformer.best", Some("7"))]).is_empty());
    crate::picker::empty_fresh_tree(app).unwrap();
    assert!(agent_store_snapshot(app, false).contains(&("platformer.best".into(), "7".into())));
    std::env::remove_var("EXACT_AGENT_STORAGE_FRESH");
    assert!(persist_agent_writes(app, &[write("platformer.best", Some("22050"))]).is_empty());
    std::env::remove_var("EXACT_AGENT_STORAGE");
    assert!(persist_agent_writes(app, &[write("platformer.best", Some("x"))]).is_empty());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "22050");
}

/// b6 review C1: a fresh drive whose scratch tree cannot be emptied boots
/// with nothing, not with the secret the last drive left. A child, as
/// above.
#[test]
fn a_fresh_tree_that_cannot_be_emptied_is_not_read() {
    const CHILD: &str = "EXACT_LINUX_FRESH_WIPE_TEST";
    if std::env::var_os(CHILD).is_none() {
        let home = std::env::temp_dir().join(format!("exact-linux-wipe-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "host::tests::a_fresh_tree_that_cannot_be_emptied_is_not_read",
            ])
            .env(CHILD, "1")
            .env("HOME", &home)
            .env("XDG_CACHE_HOME", &home)
            .env("EXACT_AGENT", "1")
            .env("EXACT_AGENT_STORAGE", "s2")
            .env_remove("EXACT_AGENT_STORAGE_FRESH")
            .env_remove("EXACT_STORE")
            .output()
            .unwrap();
        let _ = std::process::Command::new("chmod")
            .args(["-R", "u+w"])
            .arg(&home)
            .status();
        let _ = std::fs::remove_dir_all(&home);
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    use std::os::unix::fs::PermissionsExt;
    let app = "test.exact.wipe";
    let write = exact_runner::StoreWrite {
        name: "platformer.best".into(),
        value: Some("22050".into()),
    };
    assert!(persist_agent_writes(app, &[write]).is_empty());
    let secrets = crate::picker::agent_secret_root(app)
        .unwrap()
        .join("secrets");
    std::fs::set_permissions(&secrets, std::fs::Permissions::from_mode(0o555)).unwrap();
    std::env::set_var("EXACT_AGENT_STORAGE_FRESH", "1");
    assert!(
        crate::picker::empty_fresh_tree(app).is_err(),
        "the wipe fails"
    );
    assert!(
        agent_store_snapshot(app, false).is_empty(),
        "nothing the last drive left"
    );
    std::fs::set_permissions(&secrets, std::fs::Permissions::from_mode(0o755)).unwrap();
}
