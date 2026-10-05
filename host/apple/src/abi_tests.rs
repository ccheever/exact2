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
    let bytes = contract::compile("component App\n  state count = 0\n  action increment\n    count = count + 1\n  view\n    button press=increment testId=\"increment\"\n      text `${count}`\n").unwrap().encode();
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
    // The platform's secret store as launches share it, stood in for on this
    // thread (`store::PLATFORM`): hermetic, whatever the environment's
    // `EXACT_AGENT`/`EXACT_STORE` say and whether its keychain is unlocked.
    let platform = Arc::new(ibex2::secrets::MemoryStore::new());
    crate::store::PLATFORM.with(|p| *p.borrow_mut() = Some(platform));
    struct Restore;
    impl Drop for Restore {
        fn drop(&mut self) {
            crate::store::PLATFORM.with(|p| *p.borrow_mut() = None);
        }
    }
    let _restore = Restore;
    let name = "exact.prepare.returning";
    let grants = "secret.keep exact.prepare.returning\n";
    let bindings = crate::store::endow(grants).unwrap();
    assert_eq!(bindings.secrets.get(name).unwrap(), None);
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
    let embedded: &'static [u8] = plan(None).leak();
    for (selected, expected) in [
        (
            Some(b"verified bytes that are not a plan".to_vec()),
            vec!["started", "refused"],
        ),
        (Some(embedded.to_vec()), vec!["started"]),
        (None, vec![]),
    ] {
        SELECTED.with(|value| {
            *value.borrow_mut() = selected.map(|bytes| ("verified-selection".into(), bytes))
        });
        BOOTS.with(|events| events.borrow_mut().clear());
        let mut bridge = Bridge::new();
        bridge.set_delivery(Some(&DELIVERY));
        let count = bridge.boot_selected(
            embedded,
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
    let plan = contract::compile("shape Reply\n  value: string\ncomponent App\n  mutation reply as shape Reply\n  action start\n    send reply = write()\n  view\n    button press=start testId=\"start\"\n      text \"send\"\n").unwrap().encode();
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
fn holds_do_not_cross_bridge_incarnations_or_runtime_boundaries() {
    fn boot() -> Bridge<StorageModule> {
        let mut bridge = Bridge::new();
        let n = bridge.boot(
            &plan(None),
            StorageModule::default(),
            Hooks::none(),
            400.,
            600.,
        );
        assert!(std::str::from_utf8(bridge.output_bytes(n as usize))
            .unwrap()
            .contains("\"error\":null"));
        bridge
    }
    let mut bridge = boot();
    let view = bridge.host.as_ref().unwrap().runner().roots()[0];
    let len = bridge.hold_begin(view, 0, 0.);
    let json = std::str::from_utf8(bridge.output_bytes(len as usize)).unwrap();
    let token: u64 = json
        .split("\"token\":\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!(bridge.has_hold(token));
    assert!(!boot().has_hold(token));
    bridge.input_write(&plan(None));
    bridge.prepare_plan(
        plan(None).len(),
        StorageModule::default(),
        Hooks::none(),
        400.,
        600.,
    );
    bridge.commit_plan();
    assert!(!bridge.has_hold(token));
    let n = bridge.hold_update(token, f64::NAN, f64::NAN, f64::NAN);
    let out = std::str::from_utf8(bridge.output_bytes(n as usize)).unwrap();
    assert!(out.contains("\"error\":null"), "{out}");
}

#[test]
fn invalid_hold_property_preserves_status_and_other_presentations() {
    let bytes = contract::compile(
        r#"component App
  state count = 0
  action tick
    count = count + 1
  task clock mount
    every(100, tick)
  view
    box transition="opacity 180ms linear"
"#,
    )
    .unwrap()
    .encode();
    let mut bridge = Bridge::new();
    bridge.boot(&bytes, StorageModule::default(), Hooks::none(), 400., 600.);
    let host = bridge.host.as_mut().unwrap();
    let row = host.runner().roots()[0];
    let serial = |s: String| -> u64 {
        s.split("\"token\":\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .parse()
            .unwrap()
    };
    let held = serial(host.hold_begin(row, exact_motion::Property::Translate, 0.));
    host.hold_update(held, exact_motion::Value::new(91., 0.), 0.);
    let other = serial(host.hold_begin(row, exact_motion::Property::Opacity, 0.));
    host.hold_update(other, exact_motion::Value::scalar(0.2), 0.);
    host.hold_end(other, exact_motion::HoldEnd::Cancel, 0.);
    let len = bridge.hold_begin(row, 99, f64::NAN);
    let out = std::str::from_utf8(bridge.output_bytes(len as usize)).unwrap();
    assert!(out.contains("unknown motion property"), "{out}");
    assert!(out.contains("\"motion\":true"), "{out}");
    assert!(out.contains("\"timers\":true"), "{out}");
    assert!(out.contains("\"ops\":[]"), "{out}");
    assert!(bridge.has_hold(held));
    let len = bridge.hold_begin(row, 0, 0.);
    let out = std::str::from_utf8(bridge.output_bytes(len as usize)).unwrap();
    assert!(out.contains("\"x\":91"), "{out}");
}

#[test]
fn height_release_abi_separates_synthesis_from_generation_checked_pointer_completion() {
    let bytes = contract::compile(
        r#"component App
  state height = 180
  action release(value: number, velocity: number)
    height = value
  view
    box id="sheet" height=height box-sizing="border-box" transition="height 200ms linear"
      box testId="header" heightDragFor="sheet" heightrelease=release
"#,
    )
    .unwrap()
    .encode();
    let mut bridge = Bridge::new();
    bridge.boot(&bytes, StorageModule::default(), Hooks::none(), 400., 800.);
    let host = bridge.host.as_ref().unwrap();
    let kernel = host.runner().kernel();
    let header = kernel.find_by_test_id("header")[0];
    let target = kernel.height_drag_target(header).unwrap();
    let view = kernel.node_by_key(header).unwrap().id;
    let pack = exact_kernel::motion::motion_node;
    let n = bridge.height_drag_begin(pack(header), pack(target), 0.);
    let out = std::str::from_utf8(bridge.output_bytes(n as usize)).unwrap();
    let token: u64 = out
        .split("\"token\":\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    bridge.height_drag_update(token, 300., 0.);
    bridge.height_drag_release(token, 320., 0.);
    assert!(bridge.has_hold(token));
    bridge.hold_end(token, 0, 200., 0., 0.);
    assert!(!bridge.has_hold(token));
    let n = bridge.height_drag_release(token, f64::NAN, f64::NAN);
    let out = std::str::from_utf8(bridge.output_bytes(n as usize)).unwrap();
    assert!(out.contains("\"error\":null"), "{out}");
    assert_eq!(bridge.host.as_ref().unwrap().engine().now(), 0.);
    // Synthesis uses the ordinary typed parser, with no physical token authority.
    let len = bridge.input_write(b"640,0");
    bridge.dispatch(view, 15, len, 0.);
    assert_eq!(
        bridge
            .host
            .as_ref()
            .unwrap()
            .engine()
            .target(pack(target), exact_motion::Property::Height),
        Some(exact_motion::Value::scalar(640.))
    );
    for bad in ["-1,0", "200,NaN", "200,0,1", "NaN,0"] {
        let len = bridge.input_write(bad.as_bytes());
        let n = bridge.dispatch(view, 15, len, 100.);
        let out = std::str::from_utf8(bridge.output_bytes(n as usize)).unwrap();
        assert!(out.contains("invalid height release coordinates"), "{out}");
        assert!(out.contains("\"motion\":true"), "{out}");
        assert_eq!(bridge.host.as_ref().unwrap().engine().now(), 0.);
    }
}

// The catalog callback is synchronous and precedes every candidate's first
// measure. The Swift owner restores its checkpoint after a failed fresh boot.
#[derive(Default)]
struct MetricCatalogProbe {
    catalog: std::cell::Cell<u32>,
    calls: std::cell::RefCell<Vec<u32>>,
}
#[allow(unsafe_code)] // Borrow the test-owned callback context synchronously.
extern "C" fn identified_install(ctx: *mut c_void, _: *const crate::measure::CFontCatalog) {
    let p = unsafe { &*ctx.cast::<MetricCatalogProbe>() };
    p.catalog.set(p.catalog.get() + 1);
}
#[allow(unsafe_code)] // Borrow the test-owned callback context synchronously.
extern "C" fn identified_catalog_measure(
    ctx: *mut c_void,
    _: *const crate::measure::CRequest,
) -> crate::measure::CMetrics {
    let p = unsafe { &*ctx.cast::<MetricCatalogProbe>() };
    let catalog = p.catalog.get();
    p.calls.borrow_mut().push(catalog);
    crate::measure::CMetrics {
        width: 80.0,
        height: 10.0 * catalog as f32,
        baseline: 5.0,
    }
}
fn catalog_hooks(p: &MetricCatalogProbe) -> Hooks {
    Hooks {
        measure: Some(identified_catalog_measure),
        ctx: std::ptr::from_ref(p).cast_mut().cast(),
        ..Hooks::none()
    }
}
fn identified_plan() -> Vec<u8> {
    contract::compile("component App\n  view\n    column\n      text \"catalog text\"\n")
        .unwrap()
        .encode()
}

#[test]
fn identified_candidate_catalog_commit_and_discard_keep_separate_measurers() {
    let bytes = identified_plan();
    let live = MetricCatalogProbe::default();
    let candidate = MetricCatalogProbe::default();
    let mut bridge = Bridge::new();
    bridge.set_fonts(Some(identified_install), catalog_hooks(&live).ctx);
    bridge.boot(
        &bytes,
        StorageModule::default(),
        catalog_hooks(&live),
        390.0,
        844.0,
    );
    assert!(!live.calls.borrow().is_empty());
    assert!(live.calls.borrow().iter().all(|c| *c == 1));
    let calls = live.calls.borrow().len();
    bridge.set_fonts(Some(identified_install), catalog_hooks(&candidate).ctx);
    bridge.input_write(&bytes);
    bridge.prepare_plan(
        bytes.len(),
        StorageModule::default(),
        catalog_hooks(&candidate),
        390.0,
        844.0,
    );
    assert!(bridge.prepared.is_some());
    assert!(!candidate.calls.borrow().is_empty());
    assert_eq!(live.calls.borrow().len(), calls);
    bridge.discard_plan();
    assert!(bridge.host.is_some());
    bridge.resize(390.0, 845.0);
    bridge.resize(390.0, 844.0);
    let candidate_calls = candidate.calls.borrow().len();
    bridge.input_write(&bytes);
    bridge.prepare_plan(
        bytes.len(),
        StorageModule::default(),
        catalog_hooks(&candidate),
        390.0,
        844.0,
    );
    assert!(bridge.prepared.is_some());
    assert!(candidate.calls.borrow().len() > candidate_calls);
    assert_eq!(candidate.catalog.get(), 2);
    assert_eq!(*candidate.calls.borrow().last().unwrap(), 2);
    bridge.commit_plan();
    let previous_live_calls = live.calls.borrow().len();
    bridge.resize(400.0, 844.0);
    assert_eq!(live.calls.borrow().len(), previous_live_calls);
    assert_eq!(*candidate.calls.borrow().last().unwrap(), 2);
}

#[test]
fn identified_failed_fresh_boot_preserves_catalog_and_live_measurer() {
    let bytes = identified_plan();
    let context = MetricCatalogProbe::default();
    let hooks = catalog_hooks(&context);
    let mut bridge = Bridge::new();
    bridge.set_fonts(Some(identified_install), hooks.ctx);
    bridge.boot(&bytes, StorageModule::default(), hooks, 390.0, 844.0);
    let old_keys = bridge.host.as_ref().unwrap().runner().roots();
    let checkpoint = context.catalog.get();
    let len = bridge.boot(&bytes, StorageModule::default(), hooks, f32::NAN, 844.0);
    assert!(String::from_utf8_lossy(bridge.output_bytes(len as usize)).contains("error"));
    assert_eq!(
        context.catalog.get(),
        checkpoint,
        "invalid viewport refuses before font installation"
    );
    // ExactSession.boot's error branch does text.restore(cp); real Swift
    // catalog/paragraph restoration remains covered by TextGeometryTests.
    context.catalog.set(checkpoint);
    assert_eq!(bridge.host.as_ref().unwrap().runner().roots(), old_keys);
    context.calls.borrow_mut().clear();
    bridge.resize(391.0, 844.0);
    assert!(context
        .calls
        .borrow()
        .iter()
        .all(|catalog| *catalog == checkpoint));
}

#[test]
fn pan_dispatch_twenty_commits_deltas_without_using_reorder_eighteen() {
    let bytes = contract::compile(
        r#"component App
  state x = 0
  action move(dx: number, dy: number)
    x = x + dx + dy
  view
    box
      box testId="pan" pan=move position="relative" left=x
"#,
    )
    .unwrap()
    .encode();
    let mut bridge = Bridge::new();
    bridge.boot(&bytes, StorageModule::default(), Hooks::none(), 400., 800.);
    let host = bridge.host.as_ref().unwrap();
    let key = host.runner().kernel().find_by_test_id("pan")[0];
    let view = host.runner().kernel().node_by_key(key).unwrap().id;
    let len = bridge.input_write(b"120,-40");
    let n = bridge.dispatch(view, 20, len, 0.);
    let out = std::str::from_utf8(bridge.output_bytes(n as usize)).unwrap();
    assert!(out.contains("\"error\":null"), "{out}");
    assert_eq!(
        bridge.host.as_ref().unwrap().carry().slots,
        vec![("x".into(), Value::Number(80.))]
    );
    assert_eq!(
        bridge
            .host
            .as_ref()
            .unwrap()
            .runner()
            .kernel()
            .node(view)
            .unwrap()
            .frame
            .x,
        80.
    );
    let len = bridge.input_write(b"NaN,0");
    let n = bridge.dispatch(view, 20, len, 0.);
    assert!(std::str::from_utf8(bridge.output_bytes(n as usize))
        .unwrap()
        .contains("invalid pan deltas"));
}

#[test]
fn pan_release_twenty_eight_reaches_the_action_and_refuses_bad_velocity() {
    let bytes = contract::compile(
        r#"component App
  state vx = 0
  state vy = 0
  state released = 0
  action release(x: number, y: number)
    vx = x
    vy = y
    released = released + 1
  state x = 0
  action move(dx: number, dy: number)
    x = x + dx + dy
  view
    box testId="pan" pan=move panrelease=release
"#,
    )
    .unwrap()
    .encode();
    let mut bridge = Bridge::new();
    bridge.boot(&bytes, StorageModule::default(), Hooks::none(), 400., 800.);
    let host = bridge.host.as_ref().unwrap();
    let key = host.runner().kernel().find_by_test_id("pan")[0];
    let view = host.runner().kernel().node_by_key(key).unwrap().id;
    let slots = |b: &Bridge<StorageModule>| b.host.as_ref().unwrap().carry().slots;
    let len = bridge.input_write(b"900,-12.5");
    let n = bridge.dispatch(view, 28, len, 0.);
    let out = std::str::from_utf8(bridge.output_bytes(n as usize)).unwrap();
    assert!(out.contains("\"error\":null"), "{out}");
    let released = vec![
        ("vx".into(), Value::Number(900.)),
        ("vy".into(), Value::Number(-12.5)),
        ("released".into(), Value::Number(1.)),
        ("x".into(), Value::Number(0.)),
    ];
    assert_eq!(slots(&bridge), released);
    for bad in [&b"NaN,0"[..], b"1,inf", b"12", b""] {
        let len = bridge.input_write(bad);
        let n = bridge.dispatch(view, 28, len, 0.);
        let out = std::str::from_utf8(bridge.output_bytes(n as usize)).unwrap();
        assert!(out.contains("invalid pan release velocity"), "{out}");
        assert_eq!(
            slots(&bridge),
            released,
            "a refused payload changes nothing"
        );
    }
}

#[test]
fn dispatch_names_every_kind_and_refuses_unknown_ones() {
    let bytes = contract::compile(
        r#"component App
  state text = "hello"
  action edit(value: string)
    text = value
  view
    box
      input testId="field" value=text change=edit
"#,
    )
    .unwrap()
    .encode();
    let mut bridge = Bridge::new();
    bridge.boot(&bytes, StorageModule::default(), Hooks::none(), 400., 800.);
    let host = bridge.host.as_ref().unwrap();
    let key = host.runner().kernel().find_by_test_id("field")[0];
    let view = host.runner().kernel().node_by_key(key).unwrap().id;
    let slots = |bridge: &Bridge<StorageModule>| bridge.host.as_ref().unwrap().carry().slots;
    let hello = vec![("text".into(), Value::str("hello"))];
    // A collection move (kind 18, include/exact.h) is never the text of a change.
    let packet = Event::ReorderDrop {
        item: "row".into(),
        before: None,
    }
    .reorder_drop_bytes()
    .unwrap();
    let len = bridge.input_write(&packet);
    bridge.dispatch(view, 18, len, 0.);
    assert_eq!(slots(&bridge), hello);
    let len = bridge.input_write(b"\x07\0\0\0");
    let n = bridge.dispatch(view, 18, len, 0.);
    let out = std::str::from_utf8(bridge.output_bytes(n as usize)).unwrap();
    assert!(out.contains("invalid reorder event"), "{out}");
    for kind in [29, 99, u32::MAX] {
        let len = bridge.input_write(b"typed");
        let n = bridge.dispatch(view, kind, len, 0.);
        let out = std::str::from_utf8(bridge.output_bytes(n as usize)).unwrap();
        assert!(out.contains(&format!("unknown event kind {kind}")), "{out}");
        assert_eq!(slots(&bridge), hello);
    }
    let len = bridge.input_write(b"typed");
    bridge.dispatch(view, 1, len, 0.);
    assert_eq!(slots(&bridge), vec![("text".into(), Value::str("typed"))]);
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
    for bytes in [
        b"world\0{\"beacons\":9,\"extra\":\"\xff\"}".as_slice(),
        b"wor\xffld",
    ] {
        let n = bridge.input_write(bytes);
        let n = bridge.surface_record(n);
        assert!(String::from_utf8_lossy(bridge.output_bytes(n as usize)).contains("UTF-8"));
        assert_eq!(
            bridge.host.as_ref().unwrap().runner().resource("hud"),
            Some(&Value::record(vec![Value::Number(2.)]))
        );
    }
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

/// A source whose module loads after first pixel and whose answer reads the
/// store: the runner keeps that answer for the next launch (LLP 1027 D4).
#[derive(Clone, Default)]
struct Keeping(Arc<std::sync::atomic::AtomicBool>);

impl DataSource for Keeping {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
    fn ready(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::SeqCst)
    }
    fn activate(&mut self) -> Result<(), DataError> {
        self.0.store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
    fn answer(&mut self, store: &mut Store, _: &str, _: &[Value]) -> Result<Answer, DataError> {
        let _ = store.get("crew.host");
        Ok(Answer::Now(Value::str("fresh")))
    }
}

#[test]
fn kept_answers_reach_the_platform_store_without_an_app_grant() {
    // A memory store, isolated from the parent's process-global environment.
    const CHILD: &str = "EXACT_KEPT_ANSWER_FIXTURE";
    if std::env::var_os(CHILD).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "abi::tests::kept_answers_reach_the_platform_store_without_an_app_grant",
            ])
            .env(CHILD, "1")
            .env("EXACT_STORE", "memory")
            .env_remove("EXACT_AGENT")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let mut builder = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    let string = builder.primitive(TypeKind::String);
    let crew = builder.resource("crew", "crew", &[], string, Some(&Value::str("")));
    builder.set_resource_reader(crew, true);
    builder.node(
        exact_kernel::NodeType::View as u8,
        None,
        None,
        0,
        &[],
        &[],
        None,
    );
    let bytes = builder.finish().unwrap().encode();
    let mut bridge = Bridge::new();
    bridge.boot(&bytes, Keeping::default(), Hooks::none(), 390.0, 844.0);
    bridge.data_ready();
    let runner = bridge.host.as_ref().unwrap().runner();
    let kept: Vec<_> = runner
        .journal()
        .filter(|line| line.contains("store exact.kept.crew"))
        .collect();
    assert!(
        !kept.is_empty() && kept.iter().all(|l| !l.contains("failed")),
        "{kept:?}"
    );
}

#[test]
fn candidate_profiles_are_isolated_until_commit_and_after_discard() {
    let source = |asset: &str| {
        format!("color-profile --candidate-test src=\"{asset}\" rendering-intent=\"perceptual\"\ncomponent App\n  state on = false\n  action toggle\n    on = !on\n  view\n    column\n      button testId=\"toggle\" press=toggle\n        text \"Toggle\"\n      box testId=\"color\" width=20 height=20 background-color=(on ? \"color(--candidate-test 0 1 0)\" : \"color(--candidate-test 1 0 0)\")\n")
    };
    let a = contract::compile(&source("assets/live.icc"))
        .unwrap()
        .encode();
    let b = contract::compile(&source("assets/candidate.icc"))
        .unwrap()
        .encode();
    let mut bridge = Bridge::new();
    bridge.boot(&a, StorageModule::default(), Hooks::none(), 100.0, 100.0);
    let check = |bridge: &mut Bridge<StorageModule>, expected: &str| {
        let host = bridge.host.as_mut().unwrap();
        let kernel = host.runner().kernel();
        let button = kernel
            .node_by_key(kernel.find_by_test_id("toggle")[0])
            .unwrap()
            .id;
        let batch = host.dispatch_at(button, Event::Press, 1.0);
        assert!(batch.contains(expected), "{batch}");
    };
    bridge.input_write(&b);
    bridge.prepare_plan(
        b.len(),
        StorageModule::default(),
        Hooks::none(),
        100.0,
        100.0,
    );
    assert!(bridge.prepared.is_some());
    check(&mut bridge, "icc:assets/live.icc");
    bridge.discard_plan();
    check(&mut bridge, "icc:assets/live.icc");
    bridge.input_write(&b);
    bridge.prepare_plan(
        b.len(),
        StorageModule::default(),
        Hooks::none(),
        100.0,
        100.0,
    );
    assert!(bridge.prepared.is_some());
    bridge.commit_plan();
    check(&mut bridge, "icc:assets/candidate.icc");
}
