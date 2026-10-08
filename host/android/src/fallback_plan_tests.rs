use super::Bridge;
use crate::Hooks;
use exact_kernel::NodeType;
use exact_plan::{Plan, Value};
use exact_runner::{DataError, DataSource};
use std::{
    borrow::Cow,
    sync::{Arc, Mutex, OnceLock},
};

#[derive(Clone, Default)]
struct Probe {
    // Observe the plan that actually reaches the runner, not the admission copy.
    bindings: Arc<Mutex<Vec<(bool, usize, usize)>>>,
}
impl DataSource for Probe {
    fn bind(&mut self, plan: &Plan) {
        self.bindings.lock().unwrap().push((
            matches!(plan.data, Cow::Borrowed(_)),
            plan.data.as_ptr() as usize,
            plan.data.len(),
        ));
    }
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        android_core_data::Core.query(source, args)
    }
}

fn fixture(general: bool) -> &'static [u8] {
    static CORE: OnceLock<Vec<u8>> = OnceLock::new();
    static GENERAL: OnceLock<Vec<u8>> = OnceLock::new();
    (if general { &GENERAL } else { &CORE }).get_or_init(|| {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../apps/android-core/app.contract");
        let mut plan = contract::bake(
            contract::compile_path(&path).unwrap(),
            android_core_data::Core,
        )
        .unwrap();
        if general {
            plan.nodes[0].node_type = NodeType::Canvas as u8;
        }
        assert_eq!(crate::core_eligible(&plan, &Probe::default()), !general);
        plan.encode()
    })
}

fn state(bridge: &mut Bridge<Probe>) -> Vec<u8> {
    let len = bridge.input_write(br#"{"op":"state"}"#);
    let len = bridge.agent(len);
    bridge.output_bytes(len as usize).to_vec()
}
fn apple_state(bridge: &mut exact_apple::abi::Bridge<Probe>) -> Vec<u8> {
    let len = bridge.input_write(br#"{"op":"state"}"#);
    let len = bridge.agent(len);
    bridge.output_bytes(len as usize).to_vec()
}

#[test]
fn linked_general_plan_retains_static_pool_and_matches_apple_boot() {
    let bytes = fixture(true);
    let source = Probe::default();
    let observations = source.bindings.clone();
    let mut current = Bridge::<Probe>::new();
    let a = current.boot_selected(bytes, || source, Hooks::none(), 390., 844.);
    assert!(!current.binary_output());
    let bindings = observations.lock().unwrap();
    assert_eq!(
        bindings.len(),
        1,
        "one runner bind despite fallback admission"
    );
    let (borrowed, pointer, len) = bindings[0];
    assert!(
        borrowed,
        "general fallback must retain the linked immutable pool"
    );
    assert!(len > 0 && pointer >= bytes.as_ptr() as usize);
    assert!(pointer + len <= bytes.as_ptr() as usize + bytes.len());
    drop(bindings);
    let mut reference = exact_apple::abi::Bridge::new();
    let b = reference.boot_selected(bytes, Probe::default, Hooks::none(), 390., 844.);
    assert_eq!(
        current.output_bytes(a as usize),
        reference.output_bytes(b as usize)
    );
    assert_eq!(state(&mut current), apple_state(&mut reference));
    let a = current.resize(420., 860.);
    let b = reference.resize(420., 860.);
    assert_eq!(
        current.output_bytes(a as usize),
        reference.output_bytes(b as usize)
    );
}

#[test]
fn dynamic_general_pool_survives_input_overwrite_and_drop() {
    let mut bytes = fixture(true).to_vec();
    let source = Probe::default();
    let observations = source.bindings.clone();
    let mut current = Bridge::<Probe>::new();
    let a = current.boot(&bytes, source, Hooks::none(), 390., 844.);
    let mut reference = exact_apple::abi::Bridge::new();
    let b = reference.boot(&bytes, Probe::default(), Hooks::none(), 390., 844.);
    assert_eq!(
        current.output_bytes(a as usize),
        reference.output_bytes(b as usize)
    );
    let bindings = observations.lock().unwrap();
    assert_eq!(bindings.len(), 1);
    let (borrowed, pointer, len) = bindings[0];
    assert!(!borrowed, "temporary/update bytes must remain owned");
    assert!(
        len > 0
            && (pointer < bytes.as_ptr() as usize
                || pointer >= bytes.as_ptr() as usize + bytes.len())
    );
    drop(bindings);
    bytes.fill(0);
    drop(bytes);
    assert_eq!(state(&mut current), apple_state(&mut reference));
    let a = current.resize(420., 860.);
    let b = reference.resize(420., 860.);
    assert_eq!(
        current.output_bytes(a as usize),
        reference.output_bytes(b as usize)
    );
}

#[test]
fn core_selection_still_distinguishes_static_and_dynamic_pool_lifetimes() {
    let bytes = fixture(false);
    for linked in [true, false] {
        let source = Probe::default();
        let observations = source.bindings.clone();
        let mut current = Bridge::<Probe>::new();
        let n = if linked {
            current.boot_selected(bytes, || source, Hooks::none(), 390., 844.)
        } else {
            current.boot(bytes, source, Hooks::none(), 390., 844.)
        };
        assert!(current.binary_output() && n > 0);
        let bindings = observations.lock().unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].0, linked);
    }
}

#[test]
fn refused_plan_never_binds_and_preserves_previous_general_state() {
    let mut current = Bridge::<Probe>::new();
    current.boot_selected(fixture(true), Probe::default, Hooks::none(), 390., 844.);
    let previous = state(&mut current);
    for linked in [true, false] {
        let source = Probe::default();
        let observations = source.bindings.clone();
        let invalid = &fixture(true)[..12];
        let n = if linked {
            current.boot_selected(invalid, || source, Hooks::none(), 390., 844.)
        } else {
            current.boot(invalid, source, Hooks::none(), 390., 844.)
        };
        assert!(String::from_utf8_lossy(current.output_bytes(n as usize)).contains("plan:"));
        assert!(observations.lock().unwrap().is_empty());
        assert_eq!(state(&mut current), previous);
    }
}
