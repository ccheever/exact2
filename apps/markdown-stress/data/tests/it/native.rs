//! Native continuation behavior through the actual data and Runner seams.
use exact_kernel::{Kernel, Offer};
use exact_plan::Value;
use exact_runner::{Answer, DataError, DataSource, Outcome, Request, Runner, Store};
use markdown_stress_data::{MarkdownStress, NativeMarkdownStress};

fn args(profile: &str, size: usize, revision: usize, page: usize, eager: bool) -> Vec<Value> {
    vec![
        Value::str(profile),
        Value::Number(size as f64),
        Value::Number(revision as f64),
        Value::Number(page as f64),
        Value::Bool(eager),
        Value::Bool(false),
    ]
}

fn request(source: &mut NativeMarkdownStress, args: &[Value]) -> Request {
    let answer = source
        .answer(&mut Store::default(), "document", args)
        .unwrap();
    let Answer::Later(request) = answer else {
        panic!("cold native document must yield before source generation/parse");
    };
    assert!(request.is_ordered());
    assert!(request.continuation.is_some());
    request
}

fn work(source: &mut NativeMarkdownStress, request: &Request) -> Outcome {
    let token = request.continuation.unwrap();
    let work = source.continuation(token).expect("current unclaimed token");
    assert!(source.continuation(token).is_none(), "take exactly once");
    std::thread::spawn(work).join().unwrap()
}

fn accept(source: &mut NativeMarkdownStress, args: &[Value], outcome: Outcome) -> Value {
    match source
        .parse(&mut Store::default(), "document", args, outcome)
        .unwrap()
    {
        Answer::Now(value) => value,
        Answer::Later(_) => panic!("completed current parse must settle"),
    }
}

fn field(value: &Value, index: usize) -> &Value {
    let Value::Record(fields) = value else {
        panic!("document record")
    };
    &fields[index]
}

#[test]
fn native_cold_document_yields_to_ordered_work_and_returns_only_serial_ack() {
    let mut source = NativeMarkdownStress::default();
    let args = args("mixed", 16_384, 7, 0, false);
    let request = request(&mut source, &args);
    let outcome = work(&mut source, &request);
    let Outcome::Response(response) = &outcome else {
        panic!("completion ACK")
    };
    assert_eq!(response.status, 200);
    assert!(response.headers.is_empty());
    assert_eq!(
        response.body.as_slice(),
        request.continuation.unwrap().to_le_bytes()
    );
    let value = accept(&mut source, &args, outcome);
    assert_eq!(
        value,
        MarkdownStress::default().query("document", &args).unwrap()
    );
    assert_eq!(field(&value, 6), &Value::Number(7.0));
}

#[test]
fn same_key_page_replacement_settles_after_old_producer_ticket_is_discarded() {
    let mut source = NativeMarkdownStress::default();
    let first = args("blocks", 16_384, 0, 0, false);
    let second = args("blocks", 16_384, 0, 1, false);
    let a = request(&mut source, &first);
    let a_work = source.continuation(a.continuation.unwrap()).unwrap();
    let b = request(&mut source, &second);
    assert_ne!(a.continuation, b.continuation);
    // Run in the existing ordered-lane order. Runner is allowed to drop A's ACK.
    drop(std::thread::spawn(a_work).join().unwrap());
    let outcome = work(&mut source, &b);
    let value = accept(&mut source, &second, outcome);
    assert_eq!(field(&value, 4), &Value::Number(40.0));
    assert_eq!(
        value,
        MarkdownStress::default()
            .query("document", &second)
            .unwrap()
    );
    let eager = args("blocks", 16_384, 0, 0, true);
    assert_eq!(
        source
            .answer(&mut Store::default(), "document", &eager)
            .unwrap(),
        Answer::Now(MarkdownStress::default().query("document", &eager).unwrap())
    );
}

#[test]
fn old_same_arguments_ack_cannot_take_a_newer_ticket_result() {
    let mut source = NativeMarkdownStress::default();
    let args = args("code", 16_384, 3, 0, false);
    let a = request(&mut source, &args);
    let old = work(&mut source, &a);
    let b = request(&mut source, &args);
    assert!(source
        .parse(&mut Store::default(), "document", &args, old)
        .is_err());
    let current = work(&mut source, &b);
    let duplicate = current.clone();
    let value = accept(&mut source, &args, current);
    assert_eq!(
        value,
        MarkdownStress::default().query("document", &args).unwrap()
    );
    assert!(source
        .parse(&mut Store::default(), "document", &args, duplicate)
        .is_err());
}

#[test]
fn malformed_ack_and_each_mismatched_argument_preserve_current_result() {
    let mut source = NativeMarkdownStress::default();
    let args = args("blocks", 16_384, 3, 1, false);
    let r = request(&mut source, &args);
    let current = work(&mut source, &r);
    for (index, value) in [
        Value::str("code"),
        Value::Number(262_144.0),
        Value::Number(4.0),
        Value::Number(0.0),
        Value::Bool(true),
        Value::Bool(true),
    ]
    .into_iter()
    .enumerate()
    {
        let mut wrong = args.clone();
        wrong[index] = value;
        assert!(source
            .parse(&mut Store::default(), "document", &wrong, current.clone())
            .is_err());
    }
    for bytes in [vec![], vec![0; 7], vec![0; 8], vec![0; 9]] {
        let Outcome::Response(mut response) = current.clone() else {
            unreachable!()
        };
        response.body = bytes;
        assert!(source
            .parse(
                &mut Store::default(),
                "document",
                &args,
                Outcome::Response(response)
            )
            .is_err());
    }
    assert_eq!(
        accept(&mut source, &args, current),
        MarkdownStress::default().query("document", &args).unwrap()
    );
}

#[test]
fn invalid_replacement_does_not_revoke_pending_work() {
    let mut source = NativeMarkdownStress::default();
    let good = args("paragraph", 16_384, 1, 0, false);
    let r = request(&mut source, &good);
    for (index, bad) in [
        (0, Value::str("unknown")),
        (1, Value::Number(1.0)),
        (2, Value::Number(1001.0)),
        (3, Value::Number(-1.0)),
        (4, Value::Number(0.0)),
    ] {
        let mut values = good.clone();
        values[index] = bad;
        assert!(source
            .answer(&mut Store::default(), "document", &values)
            .is_err());
    }
    let outcome = work(&mut source, &r);
    assert_eq!(
        accept(&mut source, &good, outcome),
        MarkdownStress::default().query("document", &good).unwrap()
    );
}

#[test]
fn synchronous_query_cannot_start_a_second_parse_while_native_work_is_pending() {
    let mut source = NativeMarkdownStress::default();
    let current = args("code", 16_384, 0, 0, false);
    let r = request(&mut source, &current);
    let other = args("mixed", 16_384, 0, 0, false);
    assert!(source.query("document", &other).is_err());
    let outcome = work(&mut source, &r);
    accept(&mut source, &current, outcome);
    assert_eq!(
        source.query("document", &other).unwrap(),
        MarkdownStress::default().query("document", &other).unwrap()
    );
}

#[derive(Default)]
struct Counted {
    source: NativeMarkdownStress,
    queries: usize,
    answers: usize,
    parses: usize,
}

impl DataSource for Counted {
    fn app_id(&self) -> &str {
        self.source.app_id()
    }
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        self.queries += 1;
        self.source.query(source, args)
    }
    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        self.answers += 1;
        self.source.answer(store, source, args)
    }
    fn continuation(&mut self, token: u64) -> Option<Box<dyn FnOnce() -> Outcome + Send>> {
        self.source.continuation(token)
    }
    fn parse(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        self.parses += 1;
        self.source.parse(store, source, args, outcome)
    }
}

fn boot() -> Runner<Counted> {
    let plan = contract::compile(include_str!("../../../app.contract")).unwrap();
    let baked = contract::bake(plan, MarkdownStress::default()).unwrap();
    Runner::boot(
        baked,
        Counted::default(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

#[test]
fn baked_and_previous_document_survive_pending_and_runner_skips_obsolete_parse() {
    let mut runner = boot();
    let baked = runner.resource("doc").unwrap().clone();
    assert!(!runner.has_pending());
    assert_eq!(runner.data_ref().answers, 0);
    runner
        .act("chooseProfile", vec![Value::str("code")])
        .unwrap();
    assert_eq!(runner.resource("doc"), Some(&baked));
    let a = runner.take_requests().remove(0);
    let a_work = runner
        .data()
        .continuation(a.request.continuation.unwrap())
        .unwrap();
    let old_outcome = std::thread::spawn(a_work).join().unwrap();
    runner
        .act("chooseProfile", vec![Value::str("paragraph")])
        .unwrap();
    let b = runner.take_requests().remove(0);
    assert!(runner.fulfill(a.ticket, old_outcome).unwrap().is_none());
    assert_eq!(
        runner.data_ref().parses,
        0,
        "obsolete ticket never enters source parse"
    );
    assert_eq!(runner.resource("doc"), Some(&baked));
    let b_work = runner
        .data()
        .continuation(b.request.continuation.unwrap())
        .unwrap();
    runner
        .fulfill(b.ticket, std::thread::spawn(b_work).join().unwrap())
        .unwrap();
    let accepted = runner.resource("doc").unwrap().clone();
    assert_ne!(accepted, baked);
    runner.act("reparse", vec![]).unwrap();
    assert_eq!(runner.resource("doc"), Some(&accepted));
    assert!(runner.has_pending());
}

#[test]
fn actual_width_and_typing_do_zero_query_work_while_pending_and_after_settlement() {
    let mut runner = boot();
    runner
        .act("chooseProfile", vec![Value::str("blocks")])
        .unwrap();
    let pending = runner.take_requests().remove(0);
    for settled in [false, true] {
        if settled {
            let work = runner
                .data()
                .continuation(pending.request.continuation.unwrap())
                .unwrap();
            runner
                .fulfill(pending.ticket, std::thread::spawn(work).join().unwrap())
                .unwrap();
        }
        let before = (
            runner.data_ref().queries,
            runner.data_ref().answers,
            runner.data_ref().parses,
        );
        for (wide, width) in [(true, 920.0), (false, 480.0), (true, 1000.0)] {
            runner.act("chooseWidth", vec![Value::Bool(wide)]).unwrap();
            runner
                .act(
                    "editDraft",
                    vec![Value::str("Native parse remains pending 🦀")],
                )
                .unwrap();
            let root = runner.roots()[0];
            runner
                .kernel_mut()
                .compute_layout(root, Offer::definite(width, 800.0))
                .unwrap();
        }
        assert_eq!(
            before,
            (
                runner.data_ref().queries,
                runner.data_ref().answers,
                runner.data_ref().parses
            )
        );
        assert!(runner.take_requests().is_empty());
        assert_eq!(runner.has_pending(), !settled);
    }
}

#[test]
fn original_source_remains_synchronous_and_native_idle_query_matches_it() {
    let args = args("table", 16_384, 0, 0, false);
    let expected = MarkdownStress::default().query("document", &args).unwrap();
    assert_eq!(
        MarkdownStress::default()
            .answer(&mut Store::default(), "document", &args)
            .unwrap(),
        Answer::Now(expected.clone())
    );
    assert_eq!(
        NativeMarkdownStress::default()
            .query("document", &args)
            .unwrap(),
        expected
    );
    assert_eq!(
        NativeMarkdownStress::default().query("theme", &[]).unwrap(),
        MarkdownStress::default().query("theme", &[]).unwrap()
    );
}

fn giant_exact(size: usize) {
    for profile in ["paragraph", "code"] {
        let args = args(profile, size, 13, 0, true);
        let mut source = NativeMarkdownStress::default();
        let r = request(&mut source, &args);
        let outcome = work(&mut source, &r);
        let actual = accept(&mut source, &args, outcome);
        let expected = MarkdownStress::default().query("document", &args).unwrap();
        assert_eq!(actual, expected, "full Value content, not a summary/hash");
        assert_eq!(field(&actual, 2), &Value::Number(2.0));
        assert!(field(&actual, 7).as_number().unwrap() > size as f64 * 0.95);
    }
}

#[test]
#[ignore = "explicit 1 MiB direct data equality; no layout/shaping"]
fn native_one_mib_final_document_is_exact_and_unsplit() {
    giant_exact(1 << 20);
}

#[test]
#[ignore = "explicit 4 MiB direct data equality; no layout/shaping"]
fn native_four_mib_final_document_is_exact_and_unsplit() {
    giant_exact(4 << 20);
}
