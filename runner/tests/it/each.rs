//! A resource over a list (LLP 1017.000 P4c, amended 2026-11-04): one
//! request for each element, keyed as a `for` row is. An element that joins
//! starts its request; one that leaves lets only its own request go; the
//! others go on. The resource reads its answers in the list's order, pending
//! or failed when any element's request is.
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{
    Answer, DataError, DataSource, InFlight, Outcome, Request, Response, Runner, Store,
};
use std::cell::RefCell;
use std::rc::Rc;

const SOURCE: &str = r##"shape Got
  phase: string
component App
  state urls: string[] = ["a", "b", "c"]
  resource got = receive(u, "callback") for u in urls as shape Got
  action add(u: string)
    urls = urls.concat([u])
  action drop(u: string)
    urls = urls.filter(x => x != u)
  derive phases = got.map(g => g.phase).join(",")
  derive waiting = pending(got)
  derive broken = failed(got)
  view
    text phases
"##;

/// `receive(u, what)` waits for a GET of `https://example.test/<u>`, and its
/// reply's body is the phase; a 500 fails it. What the runner let go is
/// heard as the requests still in flight.
#[derive(Default)]
struct Receiver {
    asked: Rc<RefCell<Vec<String>>>,
    in_flight: Rc<RefCell<Vec<String>>>,
}

impl DataSource for Receiver {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }

    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        assert_eq!(source, "receive");
        assert_eq!(
            args[1],
            Value::str("callback"),
            "the other arguments as written"
        );
        let u = args[0].as_str().unwrap().to_string();
        self.asked.borrow_mut().push(u.clone());
        Ok(Answer::Later(Request::get(&format!(
            "https://example.test/{u}"
        ))))
    }

    fn parse(
        &mut self,
        _: &mut Store,
        _: &str,
        _: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        let Outcome::Response(r) = outcome else {
            return Err(DataError::Unavailable("no reply".into()));
        };
        if r.status != 200 {
            return Err(DataError::Unavailable("the callback failed".into()));
        }
        let phase = String::from_utf8(r.body).unwrap();
        Ok(Answer::Now(Value::record(vec![Value::str(&phase)])))
    }

    fn forgotten(&mut self, _: &Store, in_flight: &[InFlight<'_>]) {
        *self.in_flight.borrow_mut() = in_flight
            .iter()
            .map(|f| f.args[0].as_str().unwrap().to_string())
            .collect();
    }
}

fn reply(status: u16, body: &str) -> Outcome {
    Outcome::Response(Response {
        status,
        headers: Vec::new(),
        body: body.as_bytes().to_vec(),
    })
}

/// The tickets the host was handed, by element.
fn asked(r: &mut Runner<Receiver>) -> Vec<(String, u64)> {
    (r.take_requests().iter())
        .map(|o| {
            (
                o.request.url.rsplit('/').next().unwrap().to_string(),
                o.ticket,
            )
        })
        .collect()
}

fn text(r: &Runner<Receiver>, derive: &str) -> String {
    match r.derive(derive) {
        Some(Value::Bool(b)) => b.to_string(),
        Some(v) => v.as_str().unwrap().to_string(),
        None => "?".into(),
    }
}

#[test]
fn each_element_has_its_own_request_and_only_a_leaving_one_lets_go() {
    let source = Receiver::default();
    let (asks, in_flight) = (source.asked.clone(), source.in_flight.clone());
    let plan = contract::compile(SOURCE).unwrap();
    let mut r = Runner::boot(
        plan,
        source,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let mut tickets = asked(&mut r);
    assert_eq!(
        tickets.iter().map(|t| t.0.as_str()).collect::<Vec<_>>(),
        ["a", "b", "c"]
    );
    assert_eq!(
        text(&r, "phases"),
        ",,",
        "each element shows its shape's zero"
    );
    assert_eq!(text(&r, "waiting"), "true");
    assert_eq!(r.pending().len(), 3, "three wait at once");

    r.act("add", vec![Value::str("d")]).unwrap();
    tickets.extend(asked(&mut r));
    assert_eq!(
        *asks.borrow(),
        ["a", "b", "c", "d"],
        "a joining element alone is asked"
    );
    assert_eq!(r.pending().len(), 4, "four wait at once");

    let ticket = |name: &str| tickets.iter().find(|t| t.0 == name).unwrap().1;
    r.fulfill(ticket("b"), reply(200, "received")).unwrap();
    assert_eq!(text(&r, "phases"), ",received,,");
    assert_eq!(text(&r, "waiting"), "true", "the others still wait");

    r.act("drop", vec![Value::str("a")]).unwrap();
    assert!(asked(&mut r).is_empty(), "nobody is asked again");
    assert_eq!(
        *in_flight.borrow(),
        ["c", "d"],
        "only `a`'s request is let go"
    );
    assert_eq!(text(&r, "phases"), "received,,");
    assert_eq!(
        r.fulfill(ticket("a"), reply(200, "late")).unwrap(),
        None,
        "a let-go reply lands nowhere"
    );

    r.fulfill(ticket("c"), reply(500, "")).unwrap();
    assert_eq!(
        text(&r, "broken"),
        "true",
        "an element's failure fails the list"
    );
    r.fulfill(ticket("d"), reply(200, "received")).unwrap();
    assert_eq!(text(&r, "phases"), "received,,received");
    assert_eq!(text(&r, "waiting"), "false");

    r.act("drop", vec![Value::str("c")]).unwrap();
    assert_eq!(text(&r, "broken"), "false", "the failed element left");
    r.act("add", vec![Value::str("a")]).unwrap();
    assert_eq!(
        asked(&mut r)
            .iter()
            .map(|t| t.0.as_str())
            .collect::<Vec<_>>(),
        ["a"],
        "a returning element asks again"
    );
    assert_eq!(text(&r, "phases"), "received,received,");
}
