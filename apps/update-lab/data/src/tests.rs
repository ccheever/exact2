use super::*;
use exact_kernel::Kernel;
use exact_runner::Runner;
use serde_json::{json, Value as Json};

#[derive(Clone)]
struct Source {
    value: u8,
    revision: String,
    ready: bool,
}
impl Source {
    fn new(value: u8, ready: bool) -> Self {
        Self {
            value,
            revision: value.to_string(),
            ready,
        }
    }
}
impl DataSource for Source {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        assert!(self.ready);
        if source == "rustExecutor" {
            return Ok(Value::record(vec![Value::str("Fixture")]));
        }
        Ok(Value::record(vec![
            Value::str(&self.revision),
            args.first().cloned().unwrap_or(Value::Number(0.0)),
            Value::Number(f64::from(self.value)),
        ]))
    }
    fn app_id(&self) -> &str {
        "com.exact.updatelab"
    }
    fn revision(&self) -> Option<&str> {
        Some(&self.revision)
    }
    fn ready(&self) -> bool {
        self.ready
    }
    fn activate(&mut self) -> Result<(), DataError> {
        self.ready = true;
        Ok(())
    }
    fn replacement(&self, _: &[u8], _: &str, module: Vec<u8>) -> Result<Self, DataError> {
        Ok(Self::new(module[0], false))
    }
}
fn paired_receipt() -> Json {
    json!({"version":1,"kind":"mixed","javascriptBytes":1,"javascript":{"kind":"javascript"},"rust":{"kind":"rust"}})
}
fn lab() -> Lab<Source, Source> {
    compose(Source::new(1, true), Source::new(1, true), true, |source| {
        Ok(source.clone())
    })
}

#[test]
fn fixed_probe_survives_a_javascript_only_generation() {
    let mut current = compose(Source::new(1, true), Probe, false, |_| Ok(Probe));
    let args = [Value::Number(7.0)];
    let before = current.query("rustProbe", &args).unwrap();
    let mut next = current
        .replacement(&[1], r#"{"kind":"javascript"}"#, vec![2])
        .unwrap();
    next.activate().unwrap();
    assert_eq!(next.query("rustProbe", &args).unwrap(), before);
}

#[test]
fn replacement_carries_the_real_labs_counter_note_and_clock() {
    let plan = contract::compile(include_str!("../../app.contract")).unwrap();
    let mut runner = Runner::boot(plan.clone(), lab(), Kernel::with_monospace()).unwrap();
    runner.act("increment", vec![]).unwrap();
    runner
        .act("editNote", vec![Value::str("kept through both updates")])
        .unwrap();
    runner.advance(12_345.0).unwrap();
    let receipt = paired_receipt().to_string();
    for module in [vec![2, 1], vec![2, 3]] {
        let carried = runner.carry();
        let mut candidate = runner
            .data_ref()
            .replacement(&[1], &receipt, module)
            .unwrap();
        candidate.activate_for_validation().unwrap();
        runner = Runner::boot_carrying(plan.clone(), candidate, Kernel::with_monospace(), &carried)
            .unwrap();
        assert_eq!(runner.slot("counter"), Some(&Value::Number(2.0)));
        assert_eq!(
            runner.slot("note"),
            Some(&Value::str("kept through both updates"))
        );
        assert_eq!(runner.carry().now_ms, 12_345.0);
        runner.act("runTypescript", vec![]).unwrap();
        runner.act("runRust", vec![]).unwrap();
    }
}
