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
    assert!(persist_store_writes(app, &[write("platformer.best", Some("22050"))]).is_empty());
    assert!(persist_store_writes(app, &[write("exact.kept.score", Some("9"))]).is_empty());
    let again = store_snapshot(app, false);
    assert!(
        again.contains(&("platformer.best".into(), "22050".into())),
        "{again:?}"
    );
    assert!(
        again.contains(&("exact.kept.score".into(), "9".into())),
        "{again:?}"
    );
    let file = crate::picker::secret_root(app)
        .unwrap()
        .join("secrets")
        .join("platformer.best");
    assert!(file.is_file(), "{}", file.display());
    // A carried reload does not wipe, even while a fresh flag is set.
    std::env::set_var("EXACT_AGENT_STORAGE_FRESH", "1");
    assert!(store_snapshot(app, true).is_empty());
    assert!(file.is_file(), "a carried reload leaves the scratch files");
    assert!(
        store_snapshot(app, false).is_empty(),
        "a fresh launch reads nothing"
    );
    assert!(!file.exists());
    // A commit after that boot outlives the activation's storage
    // configuration and a second boot here: the tree is emptied once.
    assert!(persist_store_writes(app, &[write("platformer.best", Some("7"))]).is_empty());
    crate::picker::empty_fresh_tree(app).unwrap();
    assert!(store_snapshot(app, false).contains(&("platformer.best".into(), "7".into())));
    std::env::remove_var("EXACT_AGENT_STORAGE_FRESH");
    assert!(persist_store_writes(app, &[write("platformer.best", Some("22050"))]).is_empty());
    std::env::remove_var("EXACT_AGENT_STORAGE");
    assert!(persist_store_writes(app, &[write("platformer.best", Some("x"))]).is_empty());
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
    assert!(persist_store_writes(app, &[write]).is_empty());
    let secrets = crate::picker::secret_root(app).unwrap().join("secrets");
    std::fs::set_permissions(&secrets, std::fs::Permissions::from_mode(0o555)).unwrap();
    std::env::set_var("EXACT_AGENT_STORAGE_FRESH", "1");
    assert!(
        crate::picker::empty_fresh_tree(app).is_err(),
        "the wipe fails"
    );
    assert!(
        store_snapshot(app, false).is_empty(),
        "nothing the last drive left"
    );
    std::fs::set_permissions(&secrets, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// LLP 1027.007 D13: outside the agent, a secret is kept across a relaunch in
/// the app's own data root, `0600` in a `0700` directory. A child, so the
/// environment stays off this process.
#[cfg(unix)]
#[test]
fn an_app_keeps_a_secret_across_a_relaunch_outside_the_agent() {
    use std::os::unix::fs::PermissionsExt;
    const CHILD: &str = "EXACT_LINUX_SECRET_APP_TEST";
    if std::env::var_os(CHILD).is_none() {
        let home =
            std::env::temp_dir().join(format!("exact-linux-app-secrets-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "host::tests::an_app_keeps_a_secret_across_a_relaunch_outside_the_agent",
            ])
            .env(CHILD, "1")
            .env("HOME", &home)
            .env("XDG_DATA_HOME", home.join("data-home"))
            .env("XDG_CACHE_HOME", home.join("cache-home"))
            .env_remove("EXACT_AGENT")
            .env_remove("EXACT_AGENT_STORAGE")
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
    let app = "test.exact.session";
    let write = |name: &str, value: Option<&str>| exact_runner::StoreWrite {
        name: name.into(),
        value: value.map(str::to_string),
    };
    assert!(persist_store_writes(app, &[write("supabase.session", Some("{\"t\":1}"))]).is_empty());
    assert!(persist_store_writes(app, &[write("exact.kept.items", Some("[]"))]).is_empty());
    let again = store_snapshot(app, false);
    assert!(
        again.contains(&("supabase.session".into(), "{\"t\":1}".into())),
        "{again:?}"
    );
    assert!(
        again.contains(&("exact.kept.items".into(), "[]".into())),
        "{again:?}"
    );
    let root = crate::picker::secret_root(app).unwrap();
    let data_home = std::path::PathBuf::from(std::env::var_os("XDG_DATA_HOME").unwrap());
    assert_eq!(
        root,
        data_home.join("exact").join(app),
        "the app's own data root"
    );
    let file = root.join("secrets").join("supabase.session");
    let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "the file is the user's alone");
    let dir = std::fs::metadata(root.join("secrets"))
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(dir, 0o700, "so is the directory");
    assert!(persist_store_writes(app, &[write("supabase.session", None)]).is_empty());
    assert!(!store_snapshot(app, false)
        .iter()
        .any(|(name, _)| name == "supabase.session"));
}
