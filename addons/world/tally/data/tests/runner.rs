use exact_kernel::Kernel;
use exact_runner::{Answer, DataError, DataSource, Runner, Store, Value};
use std::sync::atomic::Ordering::Relaxed;
use tally_data::{metrics, TallySource};

#[test]
fn runner_cadence_cost() {
    let text = include_str!("../../app.contract");
    for interval in [10, 16, 20, 100] {
        let plan =
            contract::compile(&text.replace("every(10,", &format!("every({interval},"))).unwrap();
        let mut r = Runner::boot(
            plan,
            TallySource::default(),
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        r.advance(1000.).unwrap();
        let before = (metrics::ALLOCS.load(Relaxed), metrics::BYTES.load(Relaxed));
        let start = std::time::Instant::now();
        let receipts = r.advance(2000.).unwrap();
        let elapsed = start.elapsed().as_micros();
        let after = (metrics::ALLOCS.load(Relaxed), metrics::BYTES.load(Relaxed));
        println!("COST {{\"intervalMs\":{interval},\"commits\":{},\"allocations\":{},\"bytes\":{},\"cpuUs\":{elapsed}}}",receipts.len(),after.0-before.0,after.1-before.1);
        assert_eq!(receipts.len(), (2000 / interval - 1000 / interval) as usize);
    }
}

#[test]
fn negative_control_source_mutation_is_not_rolled_back() {
    use std::{cell::Cell, rc::Rc};
    struct Refuse {
        source: TallySource,
        tick: Rc<Cell<u64>>,
    }
    impl DataSource for Refuse {
        fn query(&mut self, s: &str, a: &[Value]) -> Result<Value, DataError> {
            self.source.query(s, a)
        }
        fn answer(&mut self, store: &mut Store, s: &str, a: &[Value]) -> Result<Answer, DataError> {
            let out = self.source.answer(store, s, a)?;
            self.tick.set(self.source.0.sim.world().tick());
            if s == "world" && self.tick.get() > 1 {
                return Ok(Answer::Now(Value::Number(123.)));
            }
            Ok(out)
        }
    }
    let plan = contract::compile(include_str!("../../app.contract")).unwrap();
    let tick = Rc::new(Cell::new(0));
    let mut r = Runner::boot(
        plan,
        Refuse {
            source: TallySource::default(),
            tick: tick.clone(),
        },
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let before = r.resource("world").unwrap().clone();
    assert!(r.advance(100.).is_err());
    assert_eq!(*r.resource("world").unwrap(), before);
    assert_eq!(
        tick.get(),
        2,
        "private source advanced despite refused transaction"
    );
}
