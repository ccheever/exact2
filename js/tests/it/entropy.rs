//! LLP 1069.005 D2/D3 through the Hermes bytecode executor, runner and
//! bake: secure randomness is a counted device read, refused during module
//! initialization, and never compiled into a plan. `js/web/tests/browser.rs`
//! holds the browser realms to the same answers.
#![cfg(exact_js_engine)]

use exact_js::Module;
use exact_kernel::Kernel;
use exact_plan::{Plan, Value};
use exact_runner::{Answer, DataSource, Outcome, Response, Runner, Store};

const HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/entropy.hbc"));
const APP: &str = "test.entropy";
/// `crypto`'s shape, the same string the browser realms answer.
const GLOBALS: &str = "object/function/undefined/[object Crypto]/getRandomValues,randomUUID,subtle/[object SubtleCrypto]";
const GRANTS: &str = "net.fetch https://fixture.exact.test\n";

const SRC: &str = r#"
component App
  state form = "uuid"
  resource id = uuid() as shape string
  resource plain = plain() as shape string
  resource abc = abcDigest() as shape string
  mutation result as shape string
  action atInit writes result
    send result = atInit(form)
  action bytes writes result
    send result = bytes(4)
  action later writes result
    send result = uuidLater()
  action refusals writes result
    send result = refusals()
  action describe writes result
    send result = globals()
  action digests writes result
    send result = digests()
  action digestRefusals writes result
    send result = digestRefusals()
  action digestLater writes result
    send result = digestLater()
  view
    column
      text id testId="id"
      text plain testId="plain"
      text abc testId="abc"
"#;

fn plan() -> Plan {
    contract::compile(SRC).expect("entropy fixture compiles")
}

fn module() -> Module {
    let mut module = Module::loaded(HBC.to_vec(), APP, GRANTS).expect("entropy fixture loads");
    module.set_budget_ms(f64::INFINITY);
    module.bind(&plan());
    module
}

fn text(answer: Answer) -> String {
    match answer {
        Answer::Now(value) => value.as_str().expect("a string").to_string(),
        Answer::Later(_) => panic!("expected an answer now"),
    }
}

fn is_v4(id: &str) -> bool {
    let bytes = id.as_bytes();
    id.len() == 36
        && [8, 13, 18, 23].iter().all(|&i| bytes[i] == b'-')
        && bytes[14] == b'4'
        && matches!(bytes[19], b'8' | b'9' | b'a' | b'b')
        && id
            .chars()
            .all(|c| c == '-' || c.is_ascii_digit() || ('a'..='f').contains(&c))
}

#[test]
fn a_draw_is_a_counted_read_and_refuses_during_initialization() {
    let mut module = module();
    for form in ["uuid", "bytes"] {
        let message = module.query("atInit", &[Value::str(form)]).unwrap();
        assert!(
            message
                .as_str()
                .unwrap()
                .contains("unavailable during module initialization; call it inside an answer"),
            "{form}: {message:?}"
        );
    }
    let mut store = Store::new(GRANTS, vec![]);
    let first = text(module.answer(&mut store, "uuid", &[]).unwrap());
    assert!(is_v4(&first), "{first}");
    assert_eq!((store.reads(), store.entropy_draws()), (1, 1));
    let second = text(module.answer(&mut store, "uuid", &[]).unwrap());
    assert!(is_v4(&second) && second != first);
    let bytes = text(
        module
            .answer(&mut store, "bytes", &[Value::Number(4.0)])
            .unwrap(),
    );
    assert_eq!(bytes.split(',').count(), 4, "{bytes}");
    assert_eq!(store.entropy_draws(), 3);

    // Refusals and shape: the same strings the browser realms answer.
    let before = store.reads();
    for (source, expected) in [
        ("plain", "no randomness"),
        ("refusals", "QuotaExceededError/TypeMismatchError/TypeError"),
        ("globals", GLOBALS),
    ] {
        assert_eq!(
            text(module.answer(&mut store, source, &[]).unwrap()),
            expected
        );
    }
    assert_eq!(store.reads(), before, "no draw, no read");

    // A draw after a fetch belongs to the answer the fetch resumes.
    assert!(matches!(
        module.answer(&mut store, "uuidLater", &[]),
        Ok(Answer::Later(_))
    ));
    let reply = Outcome::Response(Response {
        status: 200,
        headers: vec![],
        body: vec![],
    });
    let later = text(module.parse(&mut store, "uuidLater", &[], reply).unwrap());
    assert!(is_v4(&later));
    assert_eq!(store.entropy_draws(), 4);
}

#[test]
fn bake_compiles_no_random_value_and_the_device_asks() {
    let a = contract::bake(plan(), module()).unwrap();
    let b = contract::bake(plan(), module()).unwrap();
    assert_eq!(a.encode(), b.encode(), "no draw reaches the plan's bytes");
    let row = |name: &str| {
        a.resources
            .iter()
            .find(|r| a.str(r.name) == name)
            .unwrap()
            .clone()
    };
    assert_eq!(row("id").initial.len, 0, "a draw is not compiled");
    assert!(row("id").reader, "and it is the device's to answer");
    assert!(row("plain").initial.len > 0 && !row("plain").reader);
    // D1: a digest is pure, so bake compiles its value like any other.
    assert!(row("abc").initial.len > 0 && !row("abc").reader);

    let runner = Runner::boot(
        a,
        module(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let id = runner.resource("id").unwrap().as_str().unwrap().to_string();
    assert!(is_v4(&id), "{id}");
    assert!(runner.resource_reads_store("id"));
    assert!(runner.resource_draws_entropy("id") && !runner.resource_draws_entropy("plain"));
}

/// What the fixture's `digests` answers: each SHA-2 size over the empty
/// string, `abc` and 1 MiB of `i % 251`, then SHA-256 of `abc` twice more
/// (an algorithm object with a lowercase name over a view, and a buffer).
fn expected_digests() -> String {
    use sha2::Digest;
    let large: Vec<u8> = (0..1usize << 20).map(|i| (i % 251) as u8).collect();
    let inputs: [&[u8]; 3] = [b"", b"abc", &large];
    let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let mut lines = Vec::new();
    for input in inputs {
        lines.push(hex(&sha2::Sha256::digest(input)));
    }
    for input in inputs {
        lines.push(hex(&sha2::Sha384::digest(input)));
    }
    for input in inputs {
        lines.push(hex(&sha2::Sha512::digest(input)));
    }
    lines.push(hex(&sha2::Sha256::digest(b"abc")));
    lines.push(hex(&sha2::Sha256::digest(b"abc")));
    lines.join("\n")
}

#[test]
fn a_digest_is_pure_on_every_input_and_refuses_the_rest_by_name() {
    let mut module = module();
    assert_eq!(
        module.query("atInit", &[Value::str("digest")]).unwrap(),
        Value::str("ran: function"),
        "a digest is allowed during module initialization"
    );
    let mut store = Store::new(GRANTS, vec![]);
    let mut answer = |source: &str| text(module.answer(&mut store, source, &[]).unwrap());
    assert_eq!(answer("digests"), expected_digests());
    assert_eq!(
        answer("digestRefusals"),
        "NotSupportedError/NotSupportedError/TypeError/TypeError/NotSupportedError/NotSupportedError"
    );
    assert_eq!(
        (store.reads(), store.entropy_draws()),
        (0, 0),
        "a digest is no read"
    );
    // After a fetch, in the answer the fetch resumes.
    assert!(matches!(
        module.answer(&mut store, "digestLater", &[]),
        Ok(Answer::Later(_))
    ));
    let reply = Outcome::Response(Response {
        status: 200,
        headers: vec![],
        body: vec![],
    });
    assert_eq!(
        text(module.parse(&mut store, "digestLater", &[], reply).unwrap()),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}
