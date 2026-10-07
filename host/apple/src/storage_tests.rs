//! Where a host keeps an app's files, and where an agent's drive does.

use super::*;
use exact_kernel::MonospaceMeasurer;
use exact_plan::{builder::PlanBuilder, Value};
use exact_runner::DataError;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Seen {
    paths: Option<[PathBuf; 3]>,
    activations: usize,
}

struct Source(&'static str, Arc<Mutex<Seen>>);
impl DataSource for Source {
    fn app_id(&self) -> &str {
        self.0
    }
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
    fn configure_storage(
        &mut self,
        data: PathBuf,
        cache: PathBuf,
        temporary: PathBuf,
    ) -> Result<(), DataError> {
        let mut seen = self.1.lock().unwrap();
        assert_eq!(seen.activations, 0, "configuration precedes app activation");
        seen.paths = Some([data, cache, temporary]);
        Ok(())
    }
    fn activate(&mut self) -> Result<(), DataError> {
        self.1.lock().unwrap().activations += 1;
        Ok(())
    }
}

#[test]
fn storage_configuration_is_post_pixel_app_scoped_and_absent_in_agent_mode() {
    const CHILD: &str = "EXACT_STORAGE_CONFIGURATION_TEST";
    if std::env::var_os(CHILD).is_none() {
        for agent in [false, true] {
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command.args(["--exact", "host::storage_tests::storage_configuration_is_post_pixel_app_scoped_and_absent_in_agent_mode"])
                .env(CHILD, "1").env_remove("EXACT_AGENT");
            if agent {
                command.env("EXACT_AGENT", "1");
            }
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        return;
    }
    let mut builder = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    builder.node(NodeType::View as u8, None, None, 0, &[], &[], None);
    let plan = builder.finish().unwrap().encode();
    let mut paths = Vec::new();
    for app_id in ["test.exact.storage.a", "test.exact.storage.b"] {
        let seen = Arc::new(Mutex::new(Seen::default()));
        let (mut host, _) = Host::boot(
            &plan,
            Source(app_id, seen.clone()),
            Box::new(MonospaceMeasurer::default()),
            10.0,
            10.0,
        )
        .unwrap();
        assert_eq!(seen.lock().unwrap().activations, 0);
        assert!(
            seen.lock().unwrap().paths.is_none(),
            "boot cannot configure storage"
        );
        // The roots an `app:/` image resolves against are known at boot,
        // before storage is and with nothing picked (LLP 1069.002 D7; recipes F18).
        let at_boot = host.answer_hold("{\"op\":\"appRoots\"}").unwrap();
        host.activate_data();
        host.activate_data();
        let seen = seen.lock().unwrap();
        assert_eq!(seen.activations, 1);
        if std::env::var_os("EXACT_AGENT").is_some() {
            assert!(seen.paths.is_none());
        } else {
            let app_paths = seen.paths.as_ref().unwrap();
            assert!(app_paths.iter().all(|p| p.is_absolute()));
            for path in app_paths {
                assert!(at_boot.contains(&*path.to_string_lossy()), "{at_boot}");
            }
            for (index, path) in app_paths.iter().enumerate() {
                assert!(path.components().any(|part| part.as_os_str() == app_id));
                assert!(app_paths
                    .iter()
                    .enumerate()
                    .all(|(other, p)| other == index || !p.starts_with(path)));
            }
            paths.push(app_paths.clone());
        }
    }
    if paths.len() == 2 {
        assert!(paths[0].iter().zip(&paths[1]).all(|(a, b)| a != b));
    }
}

/// A fresh agent drive (`EXACT_AGENT_STORAGE_FRESH`) empties its scratch
/// tree once, at the boot that reads it: a secret written after that read —
/// before the post-pixel activation configures storage — is still on disk
/// after it, so a relaunch reads what memory held (tooling5 review: the
/// activation emptied the tree a second time, under a live store).
#[test]
fn a_fresh_drive_empties_its_tree_before_the_boot_reads_and_never_after() {
    const CHILD: &str = "EXACT_FRESH_STORAGE_ORDER_TEST";
    if std::env::var_os(CHILD).is_none() {
        let home = std::env::temp_dir().join(format!("exact-apple-fresh-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "host::storage_tests::a_fresh_drive_empties_its_tree_before_the_boot_reads_and_never_after"])
            .env(CHILD, "1")
            .env("HOME", &home)
            .env("EXACT_AGENT", "1")
            .env("EXACT_AGENT_STORAGE", "fresh")
            .env("EXACT_AGENT_STORAGE_FRESH", "1")
            .env_remove("EXACT_STORE")
            .output()
            .unwrap();
        let _ = std::fs::remove_dir_all(&home);
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    use crate::store::{endow_bound, endow_for, snapshot_of, Platform};
    let app = "test.exact.fresh";
    let grants = "secret.keep fresh.best";
    let root = crate::picker::agent_secret_root(app).unwrap();
    // The last drive's leftovers, which this launch must not read.
    std::fs::create_dir_all(root.join("secrets")).unwrap();
    std::fs::write(root.join("secrets").join("fresh.best"), "stale").unwrap();
    std::fs::create_dir_all(root.join("data")).unwrap();
    std::fs::write(root.join("data").join("left.txt"), "stale").unwrap();
    let (bindings, unbound) = endow_bound(grants, app, true);
    assert!(unbound.is_none(), "{unbound:?}");
    assert!(
        snapshot_of(bindings.as_ref()).is_empty(),
        "the boot reads an empty store"
    );
    assert!(
        !root.join("data").join("left.txt").exists(),
        "the whole tree is emptied at boot"
    );
    // A commit between the boot and the activation keeps a secret.
    Platform::of(bindings.as_ref().unwrap())
        .write(&exact_runner::StoreWrite {
            name: "fresh.best".into(),
            value: Some("22050".into()),
        })
        .unwrap();
    let mut builder = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    builder.node(NodeType::View as u8, None, None, 0, &[], &[], None);
    let plan = builder.finish().unwrap().encode();
    let seen = Arc::new(Mutex::new(Seen::default()));
    let (mut host, _) = Host::boot(
        &plan,
        Source(app, seen.clone()),
        Box::new(MonospaceMeasurer::default()),
        10.0,
        10.0,
    )
    .unwrap();
    host.activate_data();
    assert_eq!(seen.lock().unwrap().activations, 1);
    assert!(
        seen.lock().unwrap().paths.is_some(),
        "a named drive has storage"
    );
    let again = snapshot_of(Some(&endow_for(grants, app).unwrap()));
    assert!(
        again.contains(&("fresh.best".into(), "22050".into())),
        "{again:?}"
    );
    // A second boot in this process (the dev menu's fresh-state reload) reads it too.
    let (bindings, _) = endow_bound(grants, app, true);
    assert!(snapshot_of(bindings.as_ref()).contains(&("fresh.best".into(), "22050".into())));
}
