use super::*;
use exact_plan::bytes::Writer;
use exact_runner::Dispatch;
use serde_json::json;

struct Fixture;
impl DataSource for Fixture {
    fn app_id(&self) -> &str {
        "test.logic"
    }
    fn grants(&self) -> &str {
        "secret.keep token"
    }
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        Ok(Value::Number(1.))
    }
    fn dispatch(&mut self, token: u64, _: &Store) -> Dispatch {
        match token.checked_add(10) {
            Some(registry) => Dispatch::Host(registry),
            None => Dispatch::Missing,
        }
    }
}

crate::configured!(ConfiguredFixture, Fixture, || Swappable::off(Fixture));

#[test]
fn configured_embedded_executor_preserves_browser_continuation_routing() {
    let mut source = ConfiguredFixture::default();
    let store = Store::default();
    assert!(matches!(source.dispatch(7, &store), Dispatch::Host(17)));
    assert!(matches!(
        source.dispatch(u64::MAX, &store),
        Dispatch::Missing
    ));
    assert_eq!(source.placement(), exact_runner::Placement::Main);
}
fn encoded(op: u8, f: impl FnOnce(&mut Writer)) -> Vec<u8> {
    let mut w = Writer::default();
    w.u32(abi::ABI);
    w.u8(op);
    f(&mut w);
    w.into_vec()
}
fn wasm(call: &str) -> Vec<u8> {
    wasm_with_contract(call, false)
}
pub(crate) fn wasm_with_contract(call: &str, stateless: bool) -> Vec<u8> {
    let metadata = encoded(0, |w| {
        w.string("test.logic");
        w.string("secret.keep token")
    });
    let unit = encoded(1, |w| {
        w.u8(0);
        Value::Unit.encode(w)
    });
    let answer = encoded(2, |w| {
        w.u8(1);
        w.u32(1);
        w.string("token");
        w.u8(1);
        w.string("new");
        w.u8(0);
        Value::Number(42.).encode(w)
    });
    let escape = |bytes: &[u8]| {
        bytes
            .iter()
            .map(|b| format!("\\{b:02x}"))
            .collect::<String>()
    };
    wat::parse_str(format!(
        r#"(module
        (memory (export "memory") 1 2048)
        (global $out (mut i32) (i32.const 0))
        (global $len (mut i32) (i32.const 0))
        (data (i32.const 0) "{}")
        (data (i32.const 128) "{}")
        (data (i32.const 256) "{}")
        (func (export "exact_logic_stateless") (result i32) i32.const {stateless})
        (func (export "exact_logic_abi") (result i32) i32.const {abi})
        (func (export "exact_logic_create") (result i32) i32.const 1)
        (func (export "exact_logic_destroy") (param i32))
        (func (export "exact_logic_alloc") (param i32) (result i32) i32.const 4096)
        (func (export "exact_logic_dealloc") (param i32 i32))
        (func (export "exact_logic_output") (param i32) (result i32) global.get $out)
        (func (export "exact_logic_output_len") (param i32) (result i32) global.get $len)
        (func (export "exact_logic_call") (param i32 i32 i32) (result i32)
          local.get 1 i32.const 4 i32.add i32.load8_u
          i32.eqz
          if i32.const 0 global.set $out i32.const {} global.set $len
          else
            local.get 1 i32.const 4 i32.add i32.load8_u i32.const 3 i32.lt_u
            if i32.const 128 global.set $out i32.const {} global.set $len
            else {} i32.const 256 global.set $out i32.const {} global.set $len
            end
          end
          i32.const 0)
    )"#,
        escape(&metadata),
        escape(&unit),
        escape(&answer),
        metadata.len(),
        unit.len(),
        call,
        answer.len(),
        abi = abi::ABI,
        stateless = u8::from(stateless)
    ))
    .unwrap()
}
fn plan() -> Vec<u8> {
    let mut p = exact_plan::builder::PlanBuilder::new(0, 0)
        .finish()
        .unwrap();
    p.app_id = "test.logic".into();
    p.encode()
}
fn receipt(plan: &[u8], bytes: &[u8]) -> String {
    let card = |file: &str, bytes: &[u8]| json!({"file":file,"bytes":bytes.len(),"sha256":format!("{:x}",Sha256::digest(bytes))});
    json!({"version":1,"kind":"rust","abi":abi::ABI,"appId":"test.logic","grants":"secret.keep token","target":"wasm32-unknown-unknown","executor":"wasm","plan":card("app.plan",plan),"module":card("app.module.wasm",bytes)}).to_string()
}

#[test]
fn deferred_replacement_executes_only_after_activation_and_copies_store_effects() {
    let mut embedded = Swappable::wasm(Fixture);
    assert_eq!(embedded.query("x", &[]).unwrap(), Value::Number(1.));
    let plan = plan();
    let module = wasm("");
    let mut candidate = embedded
        .replacement(&plan, &receipt(&plan, &module), module)
        .unwrap();
    assert!(!candidate.ready());
    assert!(candidate.executor.is_none());
    assert!(candidate.query("x", &[]).is_err());
    candidate.activate_for_validation().unwrap();
    assert!(candidate.ready());
    let mut store = Store::new("secret.keep token", []);
    assert_eq!(
        candidate.answer(&mut store, "x", &[]).unwrap(),
        Answer::Now(Value::Number(42.))
    );
    assert_eq!(store.reads(), 1);
    assert_eq!(store.take_writes()[0].value.as_deref(), Some("new"));
    assert_eq!(embedded.query("x", &[]).unwrap(), Value::Number(1.));
}

#[test]
fn refuses_mismatched_pair_identity_grants_target_and_disabled_policy() {
    let plan = plan();
    let module = wasm("");
    let meta = receipt(&plan, &module);
    for (key, value) in [
        ("appId", json!("another")),
        ("grants", json!("secret.keep extra")),
        ("abi", json!(abi::ABI + 1)),
        ("target", json!(TARGET)),
        ("executor", json!("native")),
    ] {
        let mut changed: serde_json::Value = serde_json::from_str(&meta).unwrap();
        changed[key] = value;
        assert!(
            Swappable::wasm(Fixture)
                .replacement(&plan, &changed.to_string(), module.clone())
                .is_err(),
            "{key}"
        );
    }
    let mut corrupt = module.clone();
    corrupt.push(0);
    assert!(Swappable::wasm(Fixture)
        .replacement(&plan, &meta, corrupt)
        .is_err());
    assert!(Swappable::off(Fixture)
        .replacement(&plan, &meta, module)
        .is_err());
}

#[test]
fn wasm_fuel_memory_imports_and_export_bounds_are_enforced() {
    for body in [
        "(loop $forever br $forever)",
        "i32.const 1024 memory.grow drop",
    ] {
        let plan = plan();
        let module = wasm(body);
        let mut candidate = Swappable::wasm(Fixture)
            .replacement(&plan, &receipt(&plan, &module), module)
            .unwrap();
        candidate.activate().unwrap();
        assert!(matches!(
            candidate.query("x", &[]),
            Err(DataError::Interface(_))
        ));
    }
    let imported =
        wat::parse_str("(module (import \"wasi_snapshot_preview1\" \"fd_write\" (func)))").unwrap();
    assert!(crate::wasm::load(&imported)
        .err()
        .unwrap()
        .contains("imports"));
    let starting = wat::parse_str("(module (func $start) (start $start))").unwrap();
    assert!(crate::wasm::load(&starting).is_err());
    let excessive = wasm("i32.const 0 global.set $out i32.const 0 global.set $len");
    let mut executor = crate::wasm::load(&excessive).unwrap();
    assert!(executor.call(&vec![0; abi::MAX_MESSAGE + 1]).is_err());
}
