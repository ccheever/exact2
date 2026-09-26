//! @ref LLP 1027.000.000 — the date is a host fact: unknown (0) at bake and
//! boot, then one commit when the host says it; refused when implausible.
use exact_kernel::Kernel;
use exact_runner::{DataError, DataSource, Runner, RunnerError, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("host fact reached data: {source}")
    }
}

const APP: &str = "shape Time\n  epochAtZero: number\n  utcOffset: number\ncomponent App\n  resource time = exactTime() as shape Time\n  derive date = time.epochAtZero + now()\n  view\n    text `${date}/${time.utcOffset}` testId=\"date\"\n";

fn text(r: &Runner<NoData>) -> String {
    let key = r.kernel().find_by_test_id("date")[0];
    let node = r.kernel().node_by_key(key).unwrap();
    node.props
        .str(exact_kernel::PropId::Text)
        .unwrap_or("")
        .to_string()
}

#[test]
fn the_date_arrives_as_one_commit_and_bad_facts_are_refused() {
    let plan = contract::bake(contract::compile(APP).unwrap(), NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(text(&r), "0/0");
    assert!(r.set_time(1_790_000_000_000.0, 120.0).unwrap().is_some());
    assert_eq!(text(&r), "1790000000000/120");
    // Less than a second of drift is the same fact: no commit.
    assert!(r.set_time(1_790_000_000_400.0, 120.0).unwrap().is_none());
    assert!(matches!(
        r.set_time(f64::NAN, 0.0),
        Err(RunnerError::InvalidTime)
    ));
    assert!(matches!(
        r.set_time(1.0, 24.0 * 60.0),
        Err(RunnerError::InvalidTime)
    ));
    assert_eq!(text(&r), "1790000000000/120");
}
