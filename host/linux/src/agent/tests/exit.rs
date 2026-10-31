//! An orderly exit's wait for the work the app must finish: background
//! storage, and a request with effects in flight, within its bound.
use super::*;

/// LLP 1097 D10: an orderly exit pumps the module's storage to its end
/// first, within its bound.
#[test]
fn an_orderly_exit_finishes_background_storage() {
    let plan = contract::compile(
        "component App\n  resource item = save() as shape number else save()\n  view\n    text item.toString() testId=\"item\" height=20\n",
    )
    .unwrap();
    let (mut p, _) = Presenter::boot_with(
        &plan.encode(),
        Saving::default(),
        (300.0, 300.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(p.host().runner().background_operations() > 0);
    crate::teardown::finish(&mut p, crate::teardown::EXIT_BOUND);
    assert_eq!(p.host().runner().background_operations(), 0);
    assert!(!p.host().runner().has_pending());
}

/// At its bound an orderly exit stops waiting and names what it drops: a
/// request with effects in flight, or the module's storage still landing.
#[test]
fn an_orderly_exit_at_its_bound_names_what_it_drops() {
    // A POST nobody answers: the listener takes the connection and never replies
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let plan = contract::compile(
        "component App\n  resource item = post() as shape number\n  view\n    text \"item\" height=20\n",
    )
    .unwrap();
    let source = Posting {
        url: format!("{origin}/item"),
        grants: format!("net.fetch {origin}"),
    };
    let (mut p, _) = Presenter::boot_with(
        &plan.encode(),
        source,
        (300.0, 300.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(p.host().runner().effects_in_flight());
    assert_eq!(
        crate::teardown::finish(&mut p, std::time::Duration::ZERO).as_deref(),
        Some("a request with effects in flight")
    );
    let plan = contract::compile(
        "component App\n  resource item = save() as shape number else save()\n  view\n    text item.toString() testId=\"item\" height=20\n",
    )
    .unwrap();
    let (mut p, _) = Presenter::boot_with(
        &plan.encode(),
        Saving::default(),
        (300.0, 300.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    let storage = p.host().runner().background_operations();
    assert!(storage > 0);
    assert_eq!(
        crate::teardown::finish(&mut p, std::time::Duration::ZERO),
        Some(format!("{storage} storage operations waiting"))
    );
    drop(listener);
}

/// `item` asks with a POST to a local server, which this host's grant admits.
#[derive(Default)]
struct Posting {
    url: String,
    grants: String,
}
impl DataSource for Posting {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
    fn answer(
        &mut self,
        _: &mut exact_runner::Store,
        _: &str,
        _: &[Value],
    ) -> Result<exact_runner::Answer, DataError> {
        Ok(exact_runner::Answer::Later(
            exact_runner::Request::post_json(&self.url, "{}"),
        ))
    }
    fn parse(
        &mut self,
        _: &mut exact_runner::Store,
        _: &str,
        _: &[Value],
        outcome: exact_runner::Outcome,
    ) -> Result<exact_runner::Answer, DataError> {
        let landed = matches!(outcome, exact_runner::Outcome::Response(_));
        Ok(exact_runner::Answer::Now(Value::Number(if landed {
            1.0
        } else {
            -1.0
        })))
    }
    fn grants(&self) -> &str {
        &self.grants
    }
}

thread_local!(static NAMED: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) });

/// Answers each call with its name, and records what it is asked: made,
/// each call, destroyed.
struct Named;
impl crate::package::Package for Named {
    fn new(_: crate::package::Context) -> Self {
        NAMED.with(|n| n.borrow_mut().push("made".into()));
        Named
    }
    fn later(
        &mut self,
        name: &str,
        _: &crate::package::Json,
        _: Option<&crate::package::Json>,
        reply: crate::package::Reply,
    ) {
        NAMED.with(|n| n.borrow_mut().push(name.into()));
        reply.send(serde_json::json!({ "name": name }));
    }
    fn destroy(&mut self) {
        NAMED.with(|n| n.borrow_mut().push("destroyed".into()));
    }
}

/// LLP 1146 D2: an orderly exit waits for a request with effects in flight,
/// so its reply lands, and its turn runs, before the process ends, within an
/// agent's quit's bound; the module calls made meanwhile are served, as the
/// loop serves them, before the packages end.
#[test]
fn an_orderly_exit_waits_for_a_request_with_effects() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let (release, released) = std::sync::mpsc::channel::<()>();
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut seen = Vec::new();
        let mut buf = [0u8; 1024];
        while !seen.ends_with(b"{}") {
            let n = socket.read(&mut buf).unwrap();
            assert!(n > 0, "the POST was cut off");
            seen.extend_from_slice(&buf[..n]);
        }
        // Answered once the exit is waiting: the module call that waits as it begins has been served
        released
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("the exit served the waiting module call");
        let reply = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}";
        socket.write_all(reply.as_bytes()).unwrap();
    });
    let plan = contract::compile(
        "component App\n  resource item = post() as shape number\n  view\n    text \"item\" height=20\n",
    )
    .unwrap();
    let source = Posting {
        url: format!("{origin}/item"),
        grants: format!("net.fetch {origin}"),
    };
    let (mut p, _) = Presenter::boot_with(
        &plan.encode(),
        source,
        (300.0, 300.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(p.host().runner().effects_in_flight());
    p.serve_packages(vec![crate::package::Entry::of::<Named>("named")]);
    let (called, answered) = std::sync::mpsc::channel();
    let handler = p
        .host()
        .runner()
        .native_slot()
        .handler()
        .expect("a handler");
    // `served`: told once the call is answered
    let call = |name: &str, served: Option<std::sync::mpsc::Sender<()>>| {
        let called = called.clone();
        handler(
            format!(r#"{{"extension":"named","send":"{name}","args":{{}}}}"#).into_bytes(),
            exact_runner::Reply::new(move |outcome| {
                if let exact_runner::Outcome::Response(r) = outcome {
                    let _ = called.send(String::from_utf8_lossy(&r.body).into_owned());
                }
                if let Some(served) = served {
                    let _ = served.send(());
                }
            }),
        );
    };
    // The package is made by a call the loop serves, then a second waits in the inbox as the exit begins; the server
    // answers once it has been served
    call("first", None);
    p.package_turn();
    call("during", Some(release));
    crate::teardown::finish(&mut p, crate::teardown::AGENT_EXIT_BOUND);
    assert!(!p.host().runner().must_finish());
    let replies: Vec<String> = answered.try_iter().collect();
    assert_eq!(replies, [r#"{"name":"first"}"#, r#"{"name":"during"}"#]);
    assert_eq!(
        NAMED.with(|n| n.borrow().clone()),
        ["made", "first", "during", "destroyed"],
        "the module call was served by the package before it ended"
    );
    let state: serde_json::Value =
        serde_json::from_str(&handle(&mut p, r#"{"op":"state"}"#)).unwrap();
    assert_eq!(state["resources"]["item"], 1, "the reply landed: {state}");
    server.join().unwrap();
}
