//! A resource a turn over its budget refused waits to be asked again
//! (exact-runner's `overrun.rs`): the frozen clock's `clock settle` goes to
//! each re-ask, rather than stopping with the resource pending.
use super::*;

/// Answers `number(n)` with `n`, its first two turns for 2 over its budget.
#[derive(Default)]
struct Busy(usize);
impl DataSource for Busy {
    fn query(&mut self, _: &str, args: &[Value]) -> Result<Value, DataError> {
        if args[0] == Value::Number(2.) && self.0 > 0 {
            self.0 -= 1;
            return Err(DataError::Overrun(
                "took 180.0 ms, over the 100 ms budget for one turn".into(),
            ));
        }
        Ok(args[0].clone())
    }
}

#[test]
fn settle_waits_for_each_reask() {
    let plan = contract::compile(
        "component App\n  state n = 1\n  resource value = number(n) as shape number\n  action next\n    n = n + 1\n  view\n    text value.toString()\n",
    )
    .unwrap();
    let (mut p, boot_error) = Presenter::boot_with(
        &plan.encode(),
        Busy(2),
        (300.0, 300.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(boot_error.is_none(), "{boot_error:?}");
    p.host_mut().runner_mut().act("next", vec![]).unwrap();
    let value = |p: &Presenter<Busy>| p.host().runner().resource("value").cloned();
    assert_eq!(value(&p), Some(Value::Number(1.)), "it keeps what it shows");
    let reply = handle(&mut p, r#"{"op":"clock","settle":true}"#);
    // Asked again at 250 ms, refused again, and answered at 1250.
    assert!(reply.contains("\"clock\":1250"), "{reply}");
    assert_eq!(value(&p), Some(Value::Number(2.)), "{reply}");
}
