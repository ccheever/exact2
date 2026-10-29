use super::*;

#[cfg(unix)]
#[test]
fn failed_store_writes_retry_on_commits_without_writes() {
    const CHILD: &str = "EXACT_LINUX_STORE_RETRY_TEST";
    if std::env::var_os(CHILD).is_none() {
        let root = std::env::temp_dir().join(format!("exact-linux-retry-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "host::tests::failed_store_writes_retry_on_commits_without_writes",
            ])
            .env(CHILD, "1")
            .env("HOME", &root)
            .env("XDG_DATA_HOME", &root)
            .env_remove("EXACT_AGENT")
            .env_remove("EXACT_STORE")
            .output()
            .unwrap();
        std::fs::remove_dir_all(root).unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    struct Data;
    impl DataSource for Data {
        fn app_id(&self) -> &str {
            "test.exact.retry"
        }
        fn query(
            &mut self,
            _: &str,
            _: &[exact_plan::Value],
        ) -> Result<exact_plan::Value, exact_runner::DataError> {
            unreachable!()
        }
        fn answer(
            &mut self,
            store: &mut exact_runner::Store,
            _: &str,
            args: &[exact_plan::Value],
        ) -> Result<exact_runner::Answer, exact_runner::DataError> {
            let value = args[0].as_str().unwrap();
            if value.is_empty() {
                store.forget("token")?;
            } else {
                store.set("token", value)?;
            }
            Ok(exact_runner::Answer::Now(exact_plan::Value::str(value)))
        }
        fn grants(&self) -> &'static str {
            "secret.keep token\n"
        }
    }
    let plan = contract::compile(
        r#"component App
  mutation saved as shape string
  action first
    send saved = keep("first")
  action second
    send saved = keep("second")
  action third
    send saved = keep("third")
  action forget
    send saved = keep("")
  view
    column
      button testId="first" press=first
        text "first"
      button testId="second" press=second
        text "second"
      button testId="third" press=third
        text "third"
      button testId="forget" press=forget
        text "forget"
"#,
    )
    .unwrap();
    let (mut host, error) = Host::boot(
        &plan.encode(),
        Data,
        Box::new(exact_kernel::MonospaceMeasurer::default()),
        400.,
        300.,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    let root = crate::picker::secret_root(Data.app_id()).unwrap().unwrap();
    let file = root.join("secrets/token");
    std::fs::create_dir_all(&file).unwrap();
    let press = |host: &mut Host<Data>, name: &str| {
        let key = host.kernel().find_by_test_id(name)[0];
        let id = host.kernel().node_by_key(key).unwrap().id;
        assert!(host.dispatch_at(id, Event::Press, 0.).is_none());
    };
    // A commit hands its writes to the store's thread (`store_writer.rs`): each step waits for them to be
    // written, and a commit after it logs what failed.
    let step = |host: &mut Host<Data>, at: f64| {
        host.advance(at);
        assert!(drain_store_writes(std::time::Duration::from_secs(10)));
        host.advance(at + 0.25);
        // (That commit's own retry written too, before the test moves the files or the environment.)
        assert!(drain_store_writes(std::time::Duration::from_secs(10)));
    };
    let warnings = |host: &Host<Data>| {
        exact_runner::agent::logs(host.runner(), 0)
            .to_string()
            .matches("store token failed:")
            .count()
    };
    press(&mut host, "first");
    step(&mut host, 1.);
    step(&mut host, 2.);
    assert_eq!(warnings(&host), 1);
    std::fs::remove_dir(&file).unwrap();
    step(&mut host, 3.);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "first");
    std::fs::remove_file(&file).unwrap();
    std::fs::create_dir(&file).unwrap();
    press(&mut host, "second");
    step(&mut host, 4.);
    assert_eq!(warnings(&host), 2);
    press(&mut host, "third");
    step(&mut host, 5.);
    assert_eq!(warnings(&host), 3);
    std::fs::remove_dir(&file).unwrap();
    step(&mut host, 6.);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "third");
    std::fs::remove_file(&file).unwrap();
    std::fs::create_dir(&file).unwrap();
    press(&mut host, "first");
    press(&mut host, "forget");
    step(&mut host, 7.);
    assert_eq!(warnings(&host), 5);
    std::fs::remove_dir(&file).unwrap();
    std::fs::write(&file, "old").unwrap();
    step(&mut host, 8.);
    assert!(!file.exists());
    let home = std::env::var_os("HOME").unwrap();
    std::env::remove_var("HOME");
    press(&mut host, "second");
    step(&mut host, 9.);
    assert_eq!(warnings(&host), 6);
    std::env::set_var("HOME", home);
    step(&mut host, 10.);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "second");
    std::env::set_var("EXACT_AGENT", "1");
    std::env::remove_var("EXACT_AGENT_STORAGE");
    press(&mut host, "third");
    step(&mut host, 11.);
    assert_eq!(warnings(&host), 6);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "second");
}

#[cfg(windows)]
#[test]
fn windows_persists_state_and_kept_answers_outside_the_agent_without_plaintext_secrets() {
    const CHILD: &str = "EXACT_WINDOWS_PERSISTED_STATE_TEST";
    if std::env::var_os(CHILD).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "host::tests::windows_persists_state_and_kept_answers_outside_the_agent_without_plaintext_secrets",
            ])
            .env(CHILD, "1")
            .env_remove("EXACT_AGENT")
            .env_remove("EXACT_AGENT_STORAGE")
            .env_remove("EXACT_AGENT_STORAGE_FRESH")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let app = format!("test.exact.windows-state-{}", std::process::id());
    let roots = crate::picker::app_dirs(&app).unwrap().unwrap().0;
    let root = roots[0].parent().unwrap();
    assert_eq!(root.file_name().unwrap().to_str(), Some(app.as_str()));
    assert!(!root.exists(), "test needs a new app identity");
    let write = |name: &str, value: Option<&str>| exact_runner::StoreWrite {
        name: name.into(),
        value: value.map(|v| v.as_bytes().to_vec()),
    };
    let result = std::panic::catch_unwind(|| {
        assert!(crate::picker::secret_root(&app).unwrap().is_none());
        let mut writes = StoreWrites::default();
        assert!(writes
            .flush(
                &app,
                &[
                    write("exact.state.drafts", Some("[\"channel\\tdraft\"]")),
                    write("exact.kept.messages", Some("[]")),
                    write("account.token", Some("test token must not reach disk")),
                ]
            )
            .is_empty());
        let snapshot = store_snapshot(&app, false);
        assert!(
            snapshot.contains(&(
                "exact.state.drafts".into(),
                b"[\"channel\\tdraft\"]".to_vec()
            )),
            "{snapshot:?}"
        );
        assert!(
            snapshot.contains(&("exact.kept.messages".into(), b"[]".to_vec())),
            "{snapshot:?}"
        );
        assert!(!snapshot.iter().any(|(name, _)| name == "account.token"));
        assert!(!root.join("secrets").exists());
        let secret_files = ibex2::secrets::FileStore::new(root.join("secrets"));
        ibex2::secrets::SecretStore::set(&secret_files, "account.token", "test-only old file")
            .unwrap();
        assert!(
            !store_snapshot(&app, false)
                .iter()
                .any(|(name, _)| name == "account.token"),
            "ordinary state storage must not enable reading plaintext secrets"
        );
        assert!(store_snapshot(&app, true).is_empty());
        assert!(writes
            .flush(
                &app,
                &[
                    write("exact.state.drafts", None),
                    write("exact.kept.messages", None),
                ]
            )
            .is_empty());
        assert!(store_snapshot(&app, false).is_empty());
        std::env::set_var("EXACT_AGENT", "1");
        assert!(writes
            .flush(&app, &[write("exact.state.drafts", Some("[]"))])
            .is_empty());
        std::env::remove_var("EXACT_AGENT");
        assert!(
            store_snapshot(&app, false).is_empty(),
            "a nameless drive has no storage"
        );
    });
    if root.exists() {
        std::fs::remove_dir_all(root).unwrap();
    }
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}

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
        value: value.map(|v| v.as_bytes().to_vec()),
    };
    assert!(StoreWrites::default()
        .flush(app, &[write("platformer.best", Some("22050"))])
        .is_empty());
    assert!(StoreWrites::default()
        .flush(app, &[write("exact.kept.score", Some("9"))])
        .is_empty());
    // A persisted state (LLP 1116 D5) is a preference in its own kv scope.
    let tip = write("exact.state.tipPercent", Some("22"));
    assert!(StoreWrites::default().flush(app, &[tip]).is_empty());
    let again = store_snapshot(app, false);
    assert!(
        again.contains(&("exact.state.tipPercent".into(), "22".into())),
        "{again:?}"
    );
    let secrets = crate::picker::secret_root(app)
        .unwrap()
        .unwrap()
        .join("secrets");
    assert!(
        !secrets.join("exact.state.tipPercent").exists(),
        "never a secret"
    );
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
    assert!(StoreWrites::default()
        .flush(app, &[write("platformer.best", Some("7"))])
        .is_empty());
    crate::picker::empty_fresh_tree(app).unwrap();
    assert!(store_snapshot(app, false).contains(&("platformer.best".into(), "7".into())));
    std::env::remove_var("EXACT_AGENT_STORAGE_FRESH");
    assert!(StoreWrites::default()
        .flush(app, &[write("platformer.best", Some("22050"))])
        .is_empty());
    std::env::remove_var("EXACT_AGENT_STORAGE");
    assert!(StoreWrites::default()
        .flush(app, &[write("platformer.best", Some("x"))])
        .is_empty());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "22050");
}

/// b6 review C1: a fresh drive whose scratch tree cannot be emptied boots
/// with nothing, not with the secret the last drive left. A child, as
/// above. Unix only: it makes the tree unwritable with mode bits.
#[cfg(unix)]
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
    assert!(StoreWrites::default().flush(app, &[write]).is_empty());
    let secrets = crate::picker::secret_root(app)
        .unwrap()
        .unwrap()
        .join("secrets");
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
        value: value.map(|v| v.as_bytes().to_vec()),
    };
    assert!(StoreWrites::default()
        .flush(app, &[write("supabase.session", Some("{\"t\":1}"))])
        .is_empty());
    assert!(StoreWrites::default()
        .flush(app, &[write("exact.kept.items", Some("[]"))])
        .is_empty());
    let again = store_snapshot(app, false);
    assert!(
        again.contains(&("supabase.session".into(), "{\"t\":1}".into())),
        "{again:?}"
    );
    assert!(
        again.contains(&("exact.kept.items".into(), "[]".into())),
        "{again:?}"
    );
    let root = crate::picker::secret_root(app).unwrap().unwrap();
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
    assert!(StoreWrites::default()
        .flush(app, &[write("supabase.session", None)])
        .is_empty());
    assert!(!store_snapshot(app, false)
        .iter()
        .any(|(name, _)| name == "supabase.session"));
}
