use super::*;
use exact_plan::builder::PlanBuilder;
use exact_plan::{TypeKind, Value};
use exact_runner::{Answer, DataError, Store};
use std::sync::{Arc, Mutex};

fn plan(resource: Option<&str>) -> Vec<u8> {
    let mut builder = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    if let Some(name) = resource {
        let string = builder.primitive(TypeKind::String);
        builder.resource(name, "remember", &[], string, None);
    }
    builder.node(
        exact_kernel::NodeType::View as u8,
        None,
        None,
        0,
        &[],
        &[],
        None,
    );
    builder.finish().unwrap().encode()
}

#[derive(Clone, Default)]
struct StorageModule {
    configured: bool,
    loaded: bool,
    calls: Arc<Mutex<Vec<&'static str>>>,
    preparing: Arc<std::sync::atomic::AtomicBool>,
}

impl DataSource for StorageModule {
    fn preload(&self) -> Result<bool, DataError> {
        Ok(!self.preparing.load(std::sync::atomic::Ordering::SeqCst))
    }
    fn app_id(&self) -> &str {
        "test.exact.storage.reload"
    }
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
    fn ready(&self) -> bool {
        self.loaded
    }
    fn configure_storage(
        &mut self,
        data: std::path::PathBuf,
        cache: std::path::PathBuf,
        temporary: std::path::PathBuf,
    ) -> Result<(), DataError> {
        assert!(!self.loaded, "cannot configure an already-loaded module");
        assert!(!self.configured, "configure only once per candidate");
        for path in [data, cache, temporary] {
            assert!(path.is_absolute());
            assert!(path.components().any(|p| p.as_os_str() == self.app_id()));
        }
        self.configured = true;
        self.calls.lock().unwrap().push("configure");
        Ok(())
    }
    fn activate(&mut self) -> Result<(), DataError> {
        assert!(!self.loaded, "candidate must not activate twice");
        assert_eq!(self.configured, std::env::var_os("EXACT_AGENT").is_none());
        self.loaded = true;
        self.calls.lock().unwrap().push("activate");
        Ok(())
    }
    fn activate_for_validation(&mut self) -> Result<(), DataError> {
        assert!(!self.configured && !self.loaded);
        self.loaded = true;
        self.calls.lock().unwrap().push("validate");
        Ok(())
    }
    fn replacement(&self, _: &[u8], _: &str, _: Vec<u8>) -> Result<Self, DataError> {
        Ok(Self {
            calls: self.calls.clone(),
            preparing: self.preparing.clone(),
            ..Self::default()
        })
    }
}

#[test]
fn pending_native_preparation_keeps_the_running_app_and_carries_later_input() {
    use std::sync::atomic::Ordering;
    let bytes = contract::compile("component App\n  state count = 0\n  action increment writes count\n    count = count + 1\n  view\n    button press=increment testId=\"increment\"\n      text `${count}`\n").unwrap().encode();
    let source = StorageModule::default();
    let mut bridge = Bridge::new();
    bridge.boot(&bytes, source.clone(), Hooks::none(), 390.0, 844.0);
    bridge.data_ready();
    source.calls.lock().unwrap().clear();
    source.preparing.store(true, Ordering::SeqCst);
    for _ in 0..2 {
        bridge.input_write(&bytes);
        let count = bridge.prepare_module(
            [bytes.len(), 0, 0],
            source.clone(),
            Hooks::none(),
            390.0,
            844.0,
        );
        let batch = String::from_utf8_lossy(bridge.output_bytes(count as usize));
        assert!(batch.contains("\"pending\":true"), "{batch}");
        assert!(bridge.prepared.is_none());
        assert!(
            source.calls.lock().unwrap().is_empty(),
            "no app code runs before the image is ready"
        );
        let kernel = bridge.host.as_ref().unwrap().runner().kernel();
        let button = kernel
            .node_by_key(kernel.find_by_test_id("increment")[0])
            .unwrap()
            .id;
        bridge.dispatch(button, 0, 0, 0.0);
    }
    let carried = bridge.host.as_ref().unwrap().carry();
    assert_eq!(carried.slots, vec![("count".into(), Value::Number(2.0))]);
    source.preparing.store(false, Ordering::SeqCst);
    bridge.input_write(&bytes);
    bridge.prepare_module(
        [bytes.len(), 0, 0],
        source.clone(),
        Hooks::none(),
        390.0,
        844.0,
    );
    assert!(bridge.prepared.is_some());
    bridge.commit_plan();
    assert_eq!(bridge.host.as_ref().unwrap().carry().slots, carried.slots);
    assert_eq!(*source.calls.lock().unwrap(), ["validate"]);
}

#[test]
fn module_replacement_configures_before_activation_once_and_only_after_pixel() {
    const CHILD: &str = "EXACT_MODULE_STORAGE_ORDER_TEST";
    if std::env::var_os(CHILD).is_none() {
        for agent in [false, true] {
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command.args(["--exact", "abi::tests::module_replacement_configures_before_activation_once_and_only_after_pixel"])
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
    let expected = if std::env::var_os("EXACT_AGENT").is_some() {
        vec!["activate"]
    } else {
        vec!["configure", "activate"]
    };
    for painted in [false, true] {
        let bytes = plan(None);
        let source = StorageModule::default();
        let mut bridge = Bridge::new();
        bridge.boot(&bytes, source.clone(), Hooks::none(), 390.0, 844.0);
        assert!(bridge.host.is_some());
        assert!(source.calls.lock().unwrap().is_empty());
        if painted {
            bridge.data_ready();
            assert_eq!(*source.calls.lock().unwrap(), expected);
            source.calls.lock().unwrap().clear();
        }
        bridge.input_write(&bytes);
        bridge.prepare_module(
            [bytes.len(), 0, 0],
            source.clone(),
            Hooks::none(),
            390.0,
            844.0,
        );
        assert!(bridge.prepared.is_some());
        let prepared = if painted { vec!["validate"] } else { vec![] };
        assert_eq!(*source.calls.lock().unwrap(), prepared);
        bridge.commit_plan();
        assert_eq!(*source.calls.lock().unwrap(), prepared);
        source.calls.lock().unwrap().clear();
        bridge.data_ready();
        bridge.data_ready();
        assert_eq!(*source.calls.lock().unwrap(), expected);
    }
}

#[derive(Clone)]
struct Returning {
    name: &'static str,
    grants: &'static str,
    seen: Arc<Mutex<Vec<String>>>,
}

impl DataSource for Returning {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }

    fn grants(&self) -> &'static str {
        self.grants
    }

    fn answer(&mut self, store: &mut Store, _: &str, _: &[Value]) -> Result<Answer, DataError> {
        let value = store.get(self.name).unwrap_or("missing").to_string();
        self.seen.lock().unwrap().push(value.clone());
        store.set(self.name, &format!("{value}-saved"))?;
        Ok(Answer::Now(Value::str(&value)))
    }
}

#[test]
fn fresh_preparation_reads_platform_secrets_and_defers_effects_until_commit() {
    // Use ordinary platform bindings, isolated from an agent-mode parent
    // and its process-global environment. Never touch an app's own names.
    const CHILD: &str = "EXACT_PREPARE_SECRET_FIXTURE";
    if std::env::var(CHILD).as_deref() != Ok("1") {
        // Security.framework discovers executable identity by walking its directory.
        // Cargo's enormous deps directory is not an app bundle; isolate the same binary.
        let directory =
            std::env::temp_dir().join(format!("exact-keychain-fixture-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let executable = directory.join("keychain-fixture");
        std::fs::copy(std::env::current_exe().unwrap(), &executable).unwrap();
        let output = std::process::Command::new(executable)
            .args(["--exact", "abi::tests::fresh_preparation_reads_platform_secrets_and_defers_effects_until_commit"])
            .env(CHILD, "1")
            .env_remove("EXACT_AGENT")
            .env_remove("EXACT_STORE")
            .output()
            .unwrap();
        std::fs::remove_dir_all(directory).unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let name = Box::leak(format!("exact.prepare.{}.{nonce}", std::process::id()).into_boxed_str());
    let grants = Box::leak(format!("secret.keep {name}\n").into_boxed_str());
    let bindings = endow(grants).unwrap();
    struct Cleanup(ibex2::host::Secrets, &'static str);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            self.0.forget(self.1).unwrap();
        }
    }
    assert_eq!(bindings.secrets.get(name).unwrap(), None);
    let _cleanup = Cleanup(bindings.secrets.clone(), name);
    bindings.secrets.set(name, "returning").unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let data = Returning {
        name,
        grants,
        seen: seen.clone(),
    };
    let initial = plan(Some("initial"));
    let mut bridge = Bridge::new();
    bridge.input_write(&initial);
    let count = bridge.prepare_plan(initial.len(), data.clone(), Hooks::none(), 390.0, 844.0);
    assert!(
        bridge.prepared.is_some(),
        "{}",
        String::from_utf8_lossy(bridge.output_bytes(count as usize))
    );
    assert_eq!(*seen.lock().unwrap(), ["returning"]);
    assert!(bridge.host.is_none() && bridge.executor.is_none());
    assert_eq!(
        bindings.secrets.get(name).unwrap().as_deref(),
        Some("returning")
    );
    bridge.discard_plan();
    assert!(bridge.prepared.is_none() && bridge.host.is_none() && bridge.executor.is_none());
    assert_eq!(
        bindings.secrets.get(name).unwrap().as_deref(),
        Some("returning")
    );

    bridge.prepare_plan(initial.len(), data.clone(), Hooks::none(), 390.0, 844.0);
    assert!(bridge.executor.is_none());
    bridge.commit_plan();
    assert!(bridge.executor.is_some());
    assert_eq!(
        bridge.host.as_ref().unwrap().runner().store().get(name),
        Some("returning-saved")
    );
    assert_eq!(
        bindings.secrets.get(name).unwrap().as_deref(),
        Some("returning-saved")
    );

    // A peer's later platform write must not replace a running session's
    // own snapshot on reload; a new resource forces an initial query.
    bindings.secrets.set(name, "peer-change").unwrap();
    let reload = plan(Some("reloaded"));
    bridge.input_write(&reload);
    bridge.prepare_plan(reload.len(), data, Hooks::none(), 390.0, 844.0);
    assert_eq!(
        seen.lock().unwrap().last().map(String::as_str),
        Some("returning-saved")
    );
    assert_eq!(
        bindings.secrets.get(name).unwrap().as_deref(),
        Some("peer-change")
    );
    bridge.discard_plan();
    assert_eq!(
        bridge.host.as_ref().unwrap().runner().store().get(name),
        Some("returning-saved")
    );
    assert_eq!(
        bindings.secrets.get(name).unwrap().as_deref(),
        Some("peer-change")
    );
}

thread_local! {
    static SELECTED: RefCell<Option<(String, Vec<u8>)>> = const { RefCell::new(None) };
    static BOOTS: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
}
static DELIVERY: crate::delivery::Hooks = crate::delivery::Hooks {
    selected_plan: || SELECTED.with(|value| value.borrow().clone()),
    selected_module: || Ok(None),
    candidate_delivery: |_, _| None,
    boot_started: || BOOTS.with(|events| events.borrow_mut().push("started")),
    entry_refused: |entry, reason| {
        assert_eq!(entry, "verified-selection");
        assert!(!reason.is_empty());
        BOOTS.with(|events| events.borrow_mut().push("refused"));
    },
    status_into: |_| {},
    take_note: || None,
    last_line: || None,
};

#[test]
fn a_verified_selected_boot_is_counted_before_plan_refusal() {
    let embedded = plan(None);
    for (selected, expected) in [
        (
            Some(b"verified bytes that are not a plan".to_vec()),
            vec!["started", "refused"],
        ),
        (Some(embedded.clone()), vec!["started"]),
        (None, vec![]),
    ] {
        SELECTED.with(|value| {
            *value.borrow_mut() = selected.map(|bytes| ("verified-selection".into(), bytes))
        });
        BOOTS.with(|events| events.borrow_mut().clear());
        let mut bridge = Bridge::new();
        bridge.set_delivery(Some(&DELIVERY));
        let count = bridge.boot_selected(
            &embedded,
            || Returning {
                name: "unused",
                grants: "",
                seen: Arc::default(),
            },
            Hooks::none(),
            390.0,
            844.0,
        );
        assert!(
            bridge.host.is_some(),
            "{}",
            String::from_utf8_lossy(bridge.output_bytes(count as usize))
        );
        BOOTS.with(|events| assert_eq!(*events.borrow(), expected));
    }
}

#[test]
fn candidate_delivery_precedes_initial_and_carried_resource_queries() {
    #[derive(Clone)]
    struct Facts(Arc<Mutex<Vec<f64>>>);
    impl DataSource for Facts {
        fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(source.into()))
        }
        fn answer(&mut self, _: &mut Store, _: &str, args: &[Value]) -> Result<Answer, DataError> {
            let Value::Number(seq) = args[0] else {
                panic!("expected the candidate sequence")
            };
            self.0.lock().unwrap().push(seq);
            Ok(Answer::Now(Value::record(vec![Value::Number(seq)])))
        }
    }
    let source = "shape Delivery\n  seq: number\nshape Reply\n  value: number\ncomponent App\n  resource delivery = exactDelivery() as shape Delivery\n  resource reply = read(delivery.seq) as shape Reply\n  view\n    text `${reply.value}`\n";
    let seen = Arc::new(Mutex::new(Vec::new()));
    let data = Facts(seen.clone());
    let bytes = contract::bake(contract::compile(source).unwrap(), data.clone())
        .unwrap()
        .encode();
    seen.lock().unwrap().clear();
    let mut bridge = Bridge::new();
    bridge.input_write(&bytes);
    for seq in [41, 42] {
        let facts = exact_runner::Delivery {
            seq,
            ..Default::default()
        };
        let count = bridge.prepare_plan_with_delivery(
            bytes.len(),
            data.clone(),
            Hooks::none(),
            390.0,
            844.0,
            Some(facts),
        );
        assert!(
            bridge.prepared.is_some(),
            "{}",
            String::from_utf8_lossy(bridge.output_bytes(count as usize))
        );
        if seq == 41 {
            bridge.commit_plan();
        } else {
            bridge.discard_plan();
        }
    }
    assert_eq!(
        *seen.lock().unwrap(),
        [41.0, 42.0],
        "neither baked nor carried delivery may leak into candidate queries"
    );
}

#[test]
fn module_replacement_preserves_pending_requests_at_prepare_and_commit() {
    #[derive(Clone)]
    struct Held(Arc<Mutex<usize>>);
    impl DataSource for Held {
        fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(name.into()))
        }
        fn answer(&mut self, _: &mut Store, _: &str, _: &[Value]) -> Result<Answer, DataError> {
            Ok(Answer::Later(exact_runner::Request::post_json(
                "https://pending.invalid",
                "{}",
            )))
        }
        fn replacement(&self, _: &[u8], _: &str, _: Vec<u8>) -> Result<Self, DataError> {
            *self.0.lock().unwrap() += 1;
            Ok(self.clone())
        }
    }
    let plan = contract::compile("shape Reply\n  value: string\ncomponent App\n  mutation reply as shape Reply\n  action start writes reply\n    send reply = write()\n  view\n    button press=start testId=\"start\"\n      text \"send\"\n").unwrap().encode();
    let calls = Arc::new(Mutex::new(0));
    let source = Held(calls.clone());
    let mut bridge = Bridge::new();
    bridge.boot(&plan, source.clone(), Hooks::none(), 390.0, 844.0);
    bridge.data_ready();
    bridge.input_write(&plan);
    bridge.prepare_module(
        [plan.len(), 0, 0],
        source.clone(),
        Hooks::none(),
        390.0,
        844.0,
    );
    assert!(bridge.prepared.is_some());
    let prepared_calls = *calls.lock().unwrap();
    let kernel = bridge.host.as_ref().unwrap().runner().kernel();
    let button = kernel
        .node_by_key(kernel.find_by_test_id("start")[0])
        .unwrap()
        .id;
    bridge.dispatch(button, 0, 0, 0.0);
    let pending = bridge.host.as_ref().unwrap().runner().pending();
    assert!(!pending.is_empty());
    let count = bridge.commit_plan();
    assert!(String::from_utf8_lossy(bridge.output_bytes(count as usize))
        .contains("retry after they settle"));
    assert_eq!(bridge.host.as_ref().unwrap().runner().pending(), pending);
    bridge.input_write(&plan);
    let count = bridge.prepare_module([plan.len(), 0, 0], source, Hooks::none(), 390.0, 844.0);
    assert!(
        String::from_utf8_lossy(bridge.output_bytes(count as usize)).contains("pending requests")
    );
    assert_eq!(
        *calls.lock().unwrap(),
        prepared_calls,
        "refusal precedes loading candidate code"
    );
    assert_eq!(bridge.host.as_ref().unwrap().runner().pending(), pending);
}

#[test]
fn surface_record_abi_distinguishes_an_invalid_empty_record_from_disposal() {
    let plan = contract::compile("shape Hud\n  beacons: number\ncomponent App\n  resource hud = exactSurface(\"world\") as shape Hud\n  view\n    text `${hud.beacons}`\n").unwrap();
    let mut bridge = Bridge::new();
    bridge.boot(
        &plan.encode(),
        StorageModule::default(),
        Hooks::none(),
        390.,
        844.,
    );
    let n = bridge.input_write(b"world\0{\"beacons\":2}");
    bridge.surface_record(n);
    assert_eq!(
        bridge.host.as_ref().unwrap().runner().resource("hud"),
        Some(&Value::record(vec![Value::Number(2.)]))
    );
    let n = bridge.input_write(b"world\0");
    let n = bridge.surface_record(n);
    let error = String::from_utf8_lossy(bridge.output_bytes(n as usize));
    assert!(
        error.contains("hud") && error.contains("expected a value"),
        "{error}"
    );
    let n = bridge.input_write(b"world");
    bridge.surface_record(n);
    assert_eq!(
        bridge.host.as_ref().unwrap().runner().resource("hud"),
        Some(&Value::record(vec![Value::Number(0.)]))
    );
}
