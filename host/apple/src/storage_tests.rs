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
        host.activate_data();
        host.activate_data();
        let seen = seen.lock().unwrap();
        assert_eq!(seen.activations, 1);
        if std::env::var_os("EXACT_AGENT").is_some() {
            assert!(seen.paths.is_none());
        } else {
            let app_paths = seen.paths.as_ref().unwrap();
            assert!(app_paths.iter().all(|p| p.is_absolute()));
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
