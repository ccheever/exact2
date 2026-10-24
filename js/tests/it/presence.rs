//! A presence secret through the Hermes executor (LLP 1133.010 D5): data
//! code reads a name's occupancy, never a locked value with `get`; a locked
//! write is refused at once where no locked secret can be kept; and
//! `unlock` leaves as a request of its own, whose reply alone carries the
//! value back to its promise.
#![cfg(exact_js_engine)]
use exact_js::Module;
use exact_plan::{Plan, Value};
use exact_runner::{Answer, DataSource, Outcome, Response, Store};

const HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/presence.hbc"));
const GRANTS: &str =
    "secret.keep token user-presence optional\nsecret.keep key user-presence\ndevice.local-auth purpose.lock\n";

fn plan() -> Plan {
    contract::compile(
        "shape Result\n  text: string\ncomponent App\n  mutation output as shape Result\n  action invoke(op: string)\n    send output = work(op)\n  view\n    column\n",
    )
    .unwrap()
}

fn module() -> Module {
    let mut m = Module::loaded(HBC.to_vec(), "dev.exact.presence-test", GRANTS).expect("loads");
    m.set_budget_ms(f64::INFINITY);
    m.bind(&plan());
    m
}

fn text(answer: Answer) -> String {
    match answer {
        Answer::Now(Value::Record(fields)) => fields[0].as_str().unwrap().into(),
        other => panic!("{other:?}"),
    }
}

fn store(occupancy: &str, capable: bool) -> Store {
    let mut snapshot = vec![
        ("token".to_owned(), b"secret-1".to_vec()),
        (
            format!("{}token", Store::PRESENCE),
            occupancy.as_bytes().to_vec(),
        ),
    ];
    if capable {
        snapshot.push((Store::PRESENCE.to_owned(), b"1".to_vec()));
    }
    Store::new(GRANTS, snapshot)
}

#[test]
fn data_code_reads_occupancy_and_never_a_locked_value() {
    let mut m = module();
    let mut locked = store("locked", false);
    let read = text(
        m.answer(&mut locked, "work", &[Value::str("read")])
            .unwrap(),
    );
    assert!(
        read.starts_with("InvalidAccessError InvalidAccessError:"),
        "{read}"
    );
    assert!(read.contains("store.unlock"), "{read}");
    let occupancy = text(
        m.answer(&mut locked, "work", &[Value::str("occupancy")])
            .unwrap(),
    );
    assert_eq!(occupancy, "true true false false");
    // A mandatory name's write where no locked secret can be kept: refused at once.
    let set = text(
        m.answer(&mut locked, "work", &[Value::str("set-key")])
            .unwrap(),
    );
    assert!(set.starts_with("NotSupportedError"), "{set}");
    assert!(locked.take_writes().is_empty());
    let protect = text(finish(&mut m, &mut store("plain", false), "protect", None));
    assert!(protect.starts_with("NotSupportedError"), "{protect}");
    // A plain name answers `unlock` at once, with no request.
    let mut plain = store("plain", true);
    assert_eq!(
        text(m.answer(&mut plain, "work", &[Value::str("read")]).unwrap()),
        "secret-1"
    );
    assert_eq!(
        text(finish(&mut m, &mut plain, "unlock", None)),
        "unlocked secret-1"
    );
}

#[test]
fn unlock_is_a_request_of_its_own_whose_reply_alone_carries_the_value() {
    let mut m = module();
    let mut s = store("locked", true);
    let reply = r#"{"value":"secret-1"}"#;
    assert_eq!(
        text(finish(&mut m, &mut s, "unlock", Some(reply))),
        "unlocked secret-1"
    );
    assert!(!s
        .snapshot()
        .iter()
        .any(|(_, v)| v.windows(6).any(|w| w == b"secret")));
    let cancelled = r#"{"error":"USER_CANCEL","message":"the person cancelled"}"#;
    let text = text(finish(&mut m, &mut s, "unlock", Some(cancelled)));
    assert_eq!(text, "USER_CANCEL USER_CANCEL: the person cancelled");
}

/// One call to its end: its request, when it makes one, answered by `reply`.
fn finish(m: &mut Module, s: &mut Store, op: &str, reply: Option<&str>) -> Answer {
    let args = [Value::str(op)];
    let mut answer = m.answer(s, "work", &args).unwrap();
    while let Answer::Later(request) = answer {
        let outcome = if request.is_native() {
            let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
            assert_eq!(body["presence"], op, "{body}");
            assert_eq!(body["name"], "token");
            let reply = reply.expect("a request answered");
            Outcome::Response(Response {
                status: 200,
                headers: vec![],
                body: reply.as_bytes().to_vec(),
            })
        } else {
            let token = request.continuation.expect("a continuation");
            match super::runnable(m.dispatch(token, s)) {
                exact_runner::Dispatch::Run(exact_runner::Work::Now(work)) => work(),
                _ => panic!("unexpected dispatch"),
            }
        };
        answer = m.parse(s, "work", &args, outcome).unwrap();
    }
    answer
}
