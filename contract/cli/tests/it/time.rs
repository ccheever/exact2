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

const PLACED: &str = "shape Time\n  epochAtZero: number\n  locale: string\n  timeZone: string\ncomponent App\n  resource time = exactTime() as shape Time\n  view\n    text `${time.locale}|${time.timeZone}|${time.epochAtZero}` testId=\"date\"\n";

#[test]
fn the_locale_and_zone_arrive_beside_the_date_and_bad_ones_are_refused() {
    let plan = contract::bake(contract::compile(PLACED).unwrap(), NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    // Usable by Intl even before the host reports its place.
    assert_eq!(text(&r), "en-US|UTC|0");
    assert!(r
        .set_place("en-GB", "Europe/London", None)
        .unwrap()
        .is_some());
    assert_eq!(text(&r), "en-GB|Europe/London|0");
    assert!(r
        .set_place("en-GB", "Europe/London", None)
        .unwrap()
        .is_none());
    assert!(r.set_time(1_790_000_000_000.0, 60.0).unwrap().is_some());
    assert_eq!(text(&r), "en-GB|Europe/London|1790000000000");
    for (locale, zone) in [
        ("", "UTC"),
        ("en GB", "UTC"),
        ("en", ""),
        ("en", "Europe/London; rm"),
    ] {
        assert!(matches!(
            r.set_place(locale, zone, None),
            Err(RunnerError::InvalidPlace)
        ));
    }
    assert!(r
        .set_place("pt-BR", "America/Sao_Paulo", None)
        .unwrap()
        .is_some());
    assert_eq!(text(&r), "pt-BR|America/Sao_Paulo|1790000000000");
}

#[test]
fn the_launch_seed_arrives_as_a_host_fact_and_nothing_else_is_one() {
    const SEEDED: &str = "shape Time\n  seed: number\n  locale: string\ncomponent App\n  resource time = exactTime() as shape Time\n  view\n    text `${time.seed}|${time.locale}` testId=\"date\"\n";
    let plan = contract::bake(contract::compile(SEEDED).unwrap(), NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(text(&r), "0|en-US", "zero seed until the host says");
    let epoch = r.kernel().epoch();
    let receipt = r
        .set_place("fr-FR", "Europe/Paris", Some(123_456_789.0))
        .unwrap()
        .unwrap();
    assert_eq!(receipt.epoch, epoch + 1, "place and seed commit together");
    assert_eq!(text(&r), "123456789|fr-FR");
    assert!(r
        .set_place("en-GB", "Europe/London", None)
        .unwrap()
        .is_some());
    assert_eq!(text(&r), "123456789|en-GB", "a place keeps the seed");
    assert!(r
        .set_place("en-GB", "Europe/London", Some(123_456_789.0))
        .unwrap()
        .is_none());
    for bad in [-1.0, 0.5, f64::NAN, 9_007_199_254_740_992.0] {
        assert!(matches!(
            r.set_place("fr-FR", "Europe/Paris", Some(bad)),
            Err(RunnerError::InvalidPlace)
        ));
        assert_eq!(
            text(&r),
            "123456789|en-GB",
            "bad seed refuses the whole place"
        );
    }
}
