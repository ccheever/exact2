use crate::{bridge::Bridge, core_eligible, CoreOnly, Hooks};
use exact_kernel::{NodeType, StyleId};
use exact_plan::{BindingKind, Plan, Value};
use exact_runner::{DataError, DataSource};

fn fixture_plan() -> &'static [u8] {
    static PLAN: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    PLAN.get_or_init(|| {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../apps/android-core/app.contract");
        let plan = contract::compile_path(&file).unwrap();
        contract::bake(plan, android_core_data::Core)
            .unwrap()
            .encode()
    })
}

#[test]
fn retained_carrier_matches_existing_core_publications_and_state() {
    let plan = Plan::decode(fixture_plan()).unwrap();
    assert!(core_eligible(&plan, &()));
    let mut selected = Bridge::<android_core_data::Core, CoreOnly<android_core_data::Core>>::new();
    let mut previous = Bridge::<android_core_data::Core>::new();
    let a = selected.boot(
        fixture_plan(),
        android_core_data::Core,
        Hooks::none(),
        390.,
        844.,
    );
    let b = previous.boot(
        fixture_plan(),
        android_core_data::Core,
        Hooks::none(),
        390.,
        844.,
    );
    assert_eq!(
        selected.output_bytes(a as usize),
        previous.output_bytes(b as usize)
    );
    for operation in ["state", "tree", "layout"] {
        let request = format!("{{\"op\":\"{operation}\"}}");
        let a = selected.input_write(request.as_bytes());
        let b = previous.input_write(request.as_bytes());
        let a = selected.agent(a);
        let b = previous.agent(b);
        assert_eq!(
            selected.output_bytes(a as usize),
            previous.output_bytes(b as usize)
        );
    }
}

#[test]
fn general_plan_keeps_the_exact_existing_general_owner() {
    let mut plan = Plan::decode(fixture_plan()).unwrap();
    plan.nodes[0].node_type = NodeType::Canvas as u8;
    assert!(!core_eligible(&plan, &()));
    let bytes = plan.encode();
    let mut current = Bridge::<()>::new();
    let mut reference = exact_apple::abi::Bridge::<()>::new();
    let a = current.boot(&bytes, (), Hooks::none(), 390., 844.);
    let b = reference.boot(&bytes, (), Hooks::none(), 390., 844.);
    assert!(!current.binary_output());
    assert_eq!(
        current.output_bytes(a as usize),
        reference.output_bytes(b as usize)
    );
    let a = current.resize(420., 860.);
    let b = reference.resize(420., 860.);
    assert_eq!(
        current.output_bytes(a as usize),
        reference.output_bytes(b as usize)
    );
}

#[test]
fn selected_core_rejects_later_general_plan_before_replacing_live_state() {
    let mut bridge = Bridge::<android_core_data::Core, CoreOnly<android_core_data::Core>>::new();
    bridge.boot(
        fixture_plan(),
        android_core_data::Core,
        Hooks::none(),
        390.,
        844.,
    );
    let mut plan = Plan::decode(fixture_plan()).unwrap();
    plan.nodes[0].node_type = NodeType::Canvas as u8;
    let n = bridge.boot(
        &plan.encode(),
        android_core_data::Core,
        Hooks::none(),
        390.,
        844.,
    );
    let refusal = String::from_utf8_lossy(bridge.output_bytes(n as usize));
    assert!(refusal.contains("baked core carrier refuses"));
    let n = bridge.input_write(br#"{"op":"state"}"#);
    let n = bridge.agent(n);
    assert!(String::from_utf8_lossy(bridge.output_bytes(n as usize)).contains("\"count\":0"));
}

#[test]
fn all_template_shapes_and_literal_or_dynamic_binding_footprints_are_checked() {
    let mut plan = Plan::decode(fixture_plan()).unwrap();
    let last = plan.nodes.len() - 1;
    plan.nodes[last].node_type = NodeType::List as u8;
    assert!(
        !core_eligible(&plan, &()),
        "even initially hidden arms must be admitted"
    );
    for row in [
        StyleId::Transition,
        StyleId::LayoutTransition,
        StyleId::Animation,
        StyleId::ExitAnimation,
        StyleId::DragTimeline,
        StyleId::AnimationTimeline,
        StyleId::AnimationRange,
        StyleId::TimelineScope,
        StyleId::ShapeOutside,
    ] {
        let mut plan = Plan::decode(fixture_plan()).unwrap();
        let binding = plan
            .bindings
            .iter_mut()
            .find(|b| b.kind == BindingKind::Style)
            .unwrap();
        binding.id = row as u16;
        assert!(
            !core_eligible(&plan, &()),
            "binding footprint is checked regardless of current value: {row:?}"
        );
    }
    let mut plan = Plan::decode(fixture_plan()).unwrap();
    let resource = plan
        .resources
        .iter()
        .position(|r| r.initial.len > 0)
        .unwrap();
    plan.resources[resource].reader = true;
    assert!(!core_eligible(&plan, &()));
    plan.resources[resource].reader = false;
    plan.resources[resource].initial.len = 0;
    assert!(!core_eligible(&plan, &()));
}

struct Deferred;
impl DataSource for Deferred {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
    fn ready(&self) -> bool {
        false
    }
}

#[test]
fn unproven_runtime_data_keeps_the_full_owner_and_its_behavior() {
    assert!(!core_eligible(
        &Plan::decode(fixture_plan()).unwrap(),
        &Deferred
    ));
    let mut current = Bridge::<Deferred>::new();
    let mut reference = exact_apple::abi::Bridge::<Deferred>::new();
    let a = current.boot(fixture_plan(), Deferred, Hooks::none(), 390., 844.);
    let b = reference.boot(fixture_plan(), Deferred, Hooks::none(), 390., 844.);
    assert!(!current.binary_output());
    assert_eq!(
        current.output_bytes(a as usize),
        reference.output_bytes(b as usize)
    );
}

#[test]
fn literal_and_dynamic_binding_code_cannot_bypass_motion_row_selection() {
    let original = Plan::decode(fixture_plan()).unwrap();
    for dynamic in [false, true] {
        let at = original
            .bindings
            .iter()
            .position(|b| {
                b.kind == BindingKind::Style
                    && exact_runner::vm::instructions(original.code(b.expr))
                        .any(|i| i.is_ok_and(|i| i.op == exact_plan::Opcode::LoadSlot))
                        == dynamic
            })
            .expect("the real baked fixture has literal and state-driven styles");
        let mut plan = Plan::decode(fixture_plan()).unwrap();
        plan.bindings[at].id = StyleId::Animation as u16;
        // The predicate examines the authored row footprint before evaluating
        // either expression; dynamic current values cannot hide the capability.
        assert!(!core_eligible(&plan, &()));
    }
}

#[test]
fn selector_requires_explicit_core_plan_contract_and_admits_general_by_default() {
    use crate::{baked_core_eligible, DataContract};
    let plan = Plan::decode(fixture_plan()).unwrap();
    assert!(!baked_core_eligible(
        &plan,
        &android_core_data::Core,
        DataContract::default()
    ));
    assert!(baked_core_eligible(
        &plan,
        &android_core_data::Core,
        DataContract::CorePlan
    ));
    assert!(!baked_core_eligible(
        &plan,
        &Deferred,
        DataContract::CorePlan
    ));
    let mut general = Plan::decode(fixture_plan()).unwrap();
    general.nodes[0].node_type = NodeType::Canvas as u8;
    assert!(!baked_core_eligible(
        &general,
        &android_core_data::Core,
        DataContract::CorePlan
    ));
    struct Stateful(std::cell::Cell<usize>);
    impl DataSource for Stateful {
        fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(source.into()))
        }
        fn bind(&mut self, _: &Plan) {
            self.0.set(self.0.get() + 1);
        }
    }
    let source = Stateful(std::cell::Cell::new(0));
    assert!(!baked_core_eligible(&plan, &source, DataContract::Runtime));
    struct Named;
    impl DataSource for Named {
        fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(source.into()))
        }
        fn app_id(&self) -> &str {
            "com.example.named"
        }
    }
    assert!(!baked_core_eligible(&plan, &Named, DataContract::CorePlan));
}

#[test]
fn retained_core_provider_preserves_default_and_bind_side_effects() {
    thread_local! { static EVENTS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) }; }
    struct Source {
        _private: (),
    }
    impl Default for Source {
        fn default() -> Self {
            EVENTS.with(|v| v.set(v.get() + 1));
            Self { _private: () }
        }
    }
    impl DataSource for Source {
        fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(source.into()))
        }
        fn bind(&mut self, _: &Plan) {
            EVENTS.with(|v| v.set(v.get() + 10));
        }
    }
    EVENTS.with(|v| v.set(0));
    let mut bridge = Bridge::<Source, CoreOnly<Source>>::new();
    let n = bridge.boot(fixture_plan(), Source::default(), Hooks::none(), 390., 844.);
    assert!(n > 32);
    assert_eq!(EVENTS.with(std::cell::Cell::get), 11);
}

#[test]
fn retained_provider_identity_gate_is_preserved_at_runtime() {
    struct Named;
    impl DataSource for Named {
        fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(source.into()))
        }
        fn app_id(&self) -> &str {
            "com.example.named"
        }
    }
    let mut plan = Plan::decode(fixture_plan()).unwrap();
    plan.app_id = "com.example.named".into();
    assert!(crate::baked_core_eligible(
        &plan,
        &Named,
        crate::DataContract::CorePlan
    ));
    let mut bridge = Bridge::<Named, CoreOnly<Named>>::new();
    let n = bridge.boot(&plan.encode(), Named, Hooks::none(), 390., 844.);
    assert!(n > 32);
    plan.app_id = "com.example.other".into();
    assert!(!crate::baked_core_eligible(
        &plan,
        &Named,
        crate::DataContract::CorePlan
    ));
    let n = bridge.boot(&plan.encode(), Named, Hooks::none(), 390., 844.);
    assert!(String::from_utf8_lossy(bridge.output_bytes(n as usize)).contains("AppMismatch"));
}
