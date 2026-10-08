//! title-custom-snooze: the compiled plan's root actions, run as the window runs
//! them. Thread title › Snooze › Custom… (`ui:custom-snooze`, shell.ts) reaches
//! the sidebar's Custom snooze command for the item's own thread, with the
//! window's wall time and "title" as its origin, as the row's Custom does with
//! none. The data module is a stand-in that records each `command` request and
//! leaves it pending, so the actions' own busy guard is what is exercised.

use super::PLAN;
use exact_apple::measure::{CMetrics, CRequest, CallbackMeasurer};
use exact_runner::{Answer, DataError, DataSource, Request, Store, Value};

#[derive(Default)]
struct Commands {
    calls: Vec<Vec<Value>>,
}

impl DataSource for Commands {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.to_string()))
    }

    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        if source == "command" {
            self.calls.push(args.to_vec());
        }
        Ok(Answer::Later(Request::get("https://fixture.invalid/")))
    }
}

extern "C" fn measure(_: *mut std::ffi::c_void, _: *const CRequest) -> CMetrics {
    CMetrics::default()
}

const EPOCH: f64 = 1_791_446_400_000.0;

fn host() -> exact_apple::Host<Commands> {
    let measurer = CallbackMeasurer::new(measure, std::ptr::null_mut(), None);
    let (mut host, _) =
        exact_apple::Host::boot(PLAN, Commands::default(), Box::new(measurer), 1280.0, 840.0)
            .unwrap();
    let runner = host.runner_mut();
    runner.set_time(EPOCH, 0.0).unwrap();
    runner.advance(1250.0).unwrap();
    runner.data().calls.clear();
    host
}

fn pick(op: &str, target: &str) -> Vec<Value> {
    vec![
        Value::str(op),
        Value::str(target),
        Value::str(""),
        Value::str(""),
        Value::str(""),
        Value::Bool(false),
    ]
}

fn command(op: &str, target: &str, value: &str) -> Vec<Value> {
    vec![
        Value::str(op),
        Value::str(target),
        Value::str(value),
        Value::Number(EPOCH + 1250.0),
    ]
}

#[test]
fn title_custom_snooze_asks_the_sidebar_command_for_its_own_thread() {
    let mut host = host();
    let runner = host.runner_mut();
    runner
        .act("titleMenuPick", pick("ui:custom-snooze", "thread-b"))
        .unwrap();
    assert_eq!(runner.slot("titleMenuOpen"), Some(&Value::Bool(false)));
    assert_eq!(
        runner.data_ref().calls,
        vec![command("sidebar:snooze:custom", "thread-b", "title")]
    );
    // The request is still out: a second pick waits, as every sidebar command does.
    runner
        .act("titleMenuPick", pick("ui:custom-snooze", "thread-a"))
        .unwrap();
    assert_eq!(runner.data_ref().calls.len(), 1);
}

#[test]
fn the_sidebar_rows_custom_snooze_is_unchanged() {
    let mut host = host();
    let runner = host.runner_mut();
    let args = vec![
        Value::str("snooze:custom"),
        Value::str("thread-b"),
        Value::str(""),
    ];
    runner.act("sidebarRun", args).unwrap();
    assert_eq!(
        runner.data_ref().calls,
        vec![command("sidebar:snooze:custom", "thread-b", "")]
    );
}
