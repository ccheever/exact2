//! LLP 1027.000 through the actual Hermes bytecode executor and runner.
#![cfg(exact_js_engine)]

use exact_js::Module;
use exact_kernel::Kernel;
use exact_plan::{Plan, Value};
use exact_runner::{Answer, DataError, DataSource, Outcome, Response, Runner, Store};

const HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/inputs.hbc"));
const INIT_HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ambient-init.hbc"));
const APP: &str = "test.explicit-inputs";
const GRANTS: &str = "net.fetch https://fixture.exact.test\n";
const FORMS: &[(&str, &str)] = &[
    ("now", "Date.now()"),
    ("new", "new Date()"),
    ("call", "Date()"),
    ("call-with-arg", "Date()"),
    ("random", "Math.random()"),
    ("alias-now", "Date.now()"),
    ("alias-random", "Math.random()"),
    ("alias-date", "new Date()"),
    ("prototype-constructor", "new Date()"),
    ("computed-now", "Date.now()"),
    ("computed-random", "Math.random()"),
    ("bound-now", "Date.now()"),
    ("bound-new", "new Date()"),
    ("reflect", "new Date()"),
    ("intl-format", "Intl.DateTimeFormat.format()"),
    ("intl-format-undefined", "Intl.DateTimeFormat.format()"),
    ("intl-parts", "Intl.DateTimeFormat.formatToParts()"),
    (
        "intl-parts-undefined",
        "Intl.DateTimeFormat.formatToParts()",
    ),
    ("intl-format-alias", "Intl.DateTimeFormat.format()"),
    (
        "intl-format-alias-undefined",
        "Intl.DateTimeFormat.format()",
    ),
    ("intl-parts-alias", "Intl.DateTimeFormat.formatToParts()"),
    (
        "intl-parts-alias-undefined",
        "Intl.DateTimeFormat.formatToParts()",
    ),
    ("intl-format-getter", "Intl.DateTimeFormat.format()"),
    ("intl-format-computed", "Intl.DateTimeFormat.format()"),
    (
        "intl-parts-prototype",
        "Intl.DateTimeFormat.formatToParts()",
    ),
];

const SRC: &str = r#"
component App
  state elapsedMs = 0
  state seed = 7
  state form = "now"
  resource value = explicit(elapsedMs, seed) as shape string
  mutation result as shape string
  action refreshValue writes elapsedMs
    refresh value
  action tick writes elapsedMs
    elapsedMs = now()
  action reseed(v: number) writes seed
    seed = v
  action ambient writes result
    send result = ambient(form)
  action ambientLater writes result
    send result = ambientLater(form)
  action explicitLater writes result
    send result = explicitLater(elapsedMs, seed)
  action atInit writes result
    send result = atInit(form)
  action utc writes result
    send result = utc()
  action intl writes result
    send result = intl(elapsedMs)
  view
    text value testId="value"
"#;

fn plan() -> Plan {
    contract::compile(SRC).expect("input fixture compiles")
}

fn module() -> Module {
    let mut module = Module::loaded(HBC.to_vec(), APP, GRANTS).expect("input fixture loads");
    module.bind(&plan());
    module
}

fn response() -> Outcome {
    Outcome::Response(Response {
        status: 200,
        headers: vec![],
        body: vec![],
    })
}

fn refused(result: Result<Value, DataError>, api: &str) {
    match result {
        Err(DataError::Unavailable(message)) => diagnostic(&message, api),
        other => panic!("{api} must refuse, got {other:?}"),
    }
}

fn diagnostic(message: &str, api: &str) {
    assert!(message.contains(api), "{api}: {message}");
    assert!(message.contains("as an argument"), "{message}");
}

#[test]
fn initialization_and_captured_aliases_cannot_obtain_ambient_inputs() {
    let mut module = module();
    for &(form, api) in FORMS {
        let message = module.query("atInit", &[Value::str(form)]).unwrap();
        diagnostic(message.as_str().unwrap(), api);
        // query is also the no-store bake path.
        refused(module.query("ambient", &[Value::str(form)]), api);
    }
    let error = Module::loaded(INIT_HBC.to_vec(), APP, GRANTS).unwrap_err();
    assert!(error.contains("module did not load"), "{error}");
    diagnostic(&error, "Date.now()");
}

#[test]
fn ambient_reads_still_refuse_after_the_host_fulfills_a_fetch() {
    let mut module = module();
    let mut store = Store::new(GRANTS, vec![]);
    for &(form, api) in FORMS {
        let args = [Value::str(form)];
        assert!(matches!(
            module.answer(&mut store, "ambientLater", &args),
            Ok(Answer::Later(_))
        ));
        let result = module
            .parse(&mut store, "ambientLater", &args, response())
            .map(|answer| match answer {
                Answer::Now(value) => value,
                Answer::Later(_) => panic!("second request"),
            });
        refused(result, api);
    }
    assert_eq!(module.in_flight(), 0);
}

#[test]
fn explicit_dates_and_seeds_repeat_across_calls_modules_and_async_interleaving() {
    let mut module = module();
    let args = [Value::Number(0.0), Value::Number(7.0)];
    let expected = Value::str("1970-01-01T00:00:00.000Z/1025555898");
    for _ in 0..3 {
        assert_eq!(module.query("explicit", &args).unwrap(), expected);
    }
    assert_eq!(
        module.query("utc", &[]).unwrap(),
        Value::str("2024-02-29T12:34:56.789Z/12/1709210096789/true")
    );
    for (epoch_ms, year) in [(0.0, "1970"), (1709210096789.0, "2024"), (-1.0, "1969")] {
        assert_eq!(
            module.query("intl", &[Value::Number(epoch_ms)]).unwrap(),
            Value::str(&[year; 4].join("/"))
        );
    }
    module.unload();
    module.load().unwrap();
    assert_eq!(module.query("explicit", &args).unwrap(), expected);

    let mut store = Store::new(GRANTS, vec![]);
    let newer = [Value::Number(1000.0), Value::Number(8.0)];
    for input in [&args, &newer] {
        assert!(matches!(
            module.answer(&mut store, "explicitLater", input),
            Ok(Answer::Later(_))
        ));
    }
    // Settle in reverse order: the continuation keeps its own invocation
    // inputs instead of reading the most recent call's mutable context.
    for input in [&newer, &args] {
        let expected = module.query("explicit", input).unwrap();
        let Answer::Now(actual) = module
            .parse(&mut store, "explicitLater", input, response())
            .unwrap()
        else {
            panic!("expected settled explicit answer")
        };
        assert_eq!(actual, expected);
    }
}

#[test]
fn bake_refuses_ambient_reads_and_keeps_explicit_inputs_as_compiled_data() {
    for &(form, api) in FORMS {
        let source = SRC.replace("explicit(elapsedMs, seed)", &format!("ambient(\"{form}\")"));
        let error = contract::bake(contract::compile(&source).unwrap(), module()).unwrap_err();
        diagnostic(&error.to_string(), api);
    }
    let a = contract::bake(plan(), module()).unwrap();
    let b = contract::bake(plan(), module()).unwrap();
    assert_eq!(a.encode(), b.encode());
    assert!(
        a.resources[0].initial.len > 0,
        "explicit boot inputs can be baked"
    );
}

#[test]
fn runner_cache_refresh_clock_and_reload_observe_only_the_declared_inputs() {
    let baked = contract::bake(plan(), module()).unwrap();
    let mut runner = Runner::boot(baked.clone(), module(), Kernel::with_monospace()).unwrap();
    let initial = runner.resource("value").cloned().unwrap();
    assert!(
        runner.data().take_logs().is_empty(),
        "boot uses compiled data"
    );

    runner.advance(1000.0).unwrap();
    assert_eq!(runner.resource("value"), Some(&initial));
    assert!(
        runner.data().take_logs().is_empty(),
        "clock alone isn't a subscription"
    );
    runner.act("refreshValue", vec![]).unwrap();
    assert_eq!(runner.resource("value"), Some(&initial));
    assert_eq!(runner.data().take_logs(), ["explicit 0 7"]);

    runner.act("tick", vec![]).unwrap();
    let at_1000 = Value::str("1970-01-01T00:00:01.000Z/1025555898");
    assert_eq!(runner.resource("value"), Some(&at_1000));
    assert_eq!(runner.data().take_logs(), ["explicit 1000 7"]);
    runner.act("tick", vec![]).unwrap();
    assert!(
        runner.data().take_logs().is_empty(),
        "unchanged arguments reuse the answer"
    );

    let carried = runner.carry();
    let mut runner =
        Runner::boot_carrying(baked, module(), Kernel::with_monospace(), &carried).unwrap();
    assert_eq!(runner.now_ms(), 1000.0);
    assert_eq!(runner.resource("value"), Some(&at_1000));
    assert!(
        runner.data().take_logs().is_empty(),
        "matching resources carry without a call"
    );
    runner.act("reseed", vec![Value::Number(8.0)]).unwrap();
    assert_eq!(
        runner.resource("value"),
        Some(&Value::str("1970-01-01T00:00:01.000Z/1027220423"))
    );
    assert_eq!(runner.data().take_logs(), ["explicit 1000 8"]);
}

#[test]
fn newer_explicit_arguments_and_reload_do_not_accept_stale_async_results() {
    let baked = contract::bake(plan(), module()).unwrap();
    let mut runner = Runner::boot(baked.clone(), module(), Kernel::with_monospace()).unwrap();
    runner.act("explicitLater", vec![]).unwrap();
    let old = runner.take_requests()[0].ticket;
    runner.advance(1000.0).unwrap();
    runner.act("tick", vec![]).unwrap();
    runner.act("explicitLater", vec![]).unwrap();
    let current = runner.take_requests()[0].ticket;
    assert!(runner.fulfill(old, response()).unwrap().is_none());
    assert_eq!(runner.slot("result"), Some(&Value::NONE));
    runner.fulfill(current, response()).unwrap();
    let expected = Value::some(Value::str("1970-01-01T00:00:01.000Z/1025555898"));
    assert_eq!(runner.slot("result"), Some(&expected));

    runner.act("explicitLater", vec![]).unwrap();
    let before_reload = runner.take_requests()[0].ticket;
    let carried = runner.carry();
    let mut reloaded =
        Runner::boot_carrying(baked, module(), Kernel::with_monospace(), &carried).unwrap();
    assert!(!reloaded.has_pending());
    assert!(reloaded
        .fulfill(before_reload, response())
        .unwrap()
        .is_none());
    assert_eq!(reloaded.slot("result"), Some(&expected));
}
