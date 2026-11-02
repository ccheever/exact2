//! An app's abort, whatever the request's method: a queued one dropped
//! unsent, a running one's transport aborted and its lane released.
use super::*;

#[test]
fn an_app_aborted_running_write_is_cancelled_and_the_next_ordered_request_runs() {
    // `forget` lets a running write run on (above); an app's explicit
    // abort cancels it whatever its method, as a browser's does.
    let (core, fixture, woke) = setup();
    core.run(
        job(1, Request::post_json("https://example.test/hold", "{}")),
        None,
    )
    .unwrap();
    fixture.wait_held(1);
    core.run(job(2, Request::get("https://example.test/read")), None)
        .unwrap();
    core.cancel(&[1]);
    // The held write ended by its abort, never released: the next request
    // runs and its outcome is the only one drained.
    assert_eq!(
        collect(&core, &woke, 1)
            .into_iter()
            .map(|v| v.0)
            .collect::<Vec<_>>(),
        [2]
    );
    until_settled(&core);
    assert!(core.ordered_idle());
    assert!(
        core.drain().is_empty(),
        "the aborted write's outcome is dropped"
    );
    assert!(
        !fixture.state.lock().unwrap().1,
        "the hold was never released"
    );
}

#[test]
fn an_app_abort_reaches_a_running_write_an_earlier_forget_let_run_on() {
    // The runner let the write go first (its answer was replaced): it runs
    // on, as a write does. The app's abort then cancels it all the same.
    let (core, fixture, woke) = setup();
    core.run(
        job(1, Request::post_json("https://example.test/hold", "{}")),
        None,
    )
    .unwrap();
    fixture.wait_held(1);
    core.run(job(2, Request::get("https://example.test/read")), None)
        .unwrap();
    core.forget(|ticket| ticket != 1);
    std::thread::sleep(Duration::from_millis(100));
    assert!(
        core.drain().is_empty(),
        "the forgotten write still holds the lane"
    );
    core.cancel(&[1]);
    assert_eq!(
        collect(&core, &woke, 1)
            .into_iter()
            .map(|v| v.0)
            .collect::<Vec<_>>(),
        [2]
    );
    until_settled(&core);
    assert!(
        !fixture.state.lock().unwrap().1,
        "the hold was never released"
    );
}

#[test]
fn an_app_aborted_queued_write_is_dropped_unsent() {
    let (core, fixture, woke) = setup();
    core.run(job(1, Request::get("https://example.test/hold")), None)
        .unwrap();
    fixture.wait_held(1);
    core.run(
        job(2, Request::post_json("https://example.test/write", "{}")),
        None,
    )
    .unwrap();
    core.run(job(3, Request::get("https://example.test/read")), None)
        .unwrap();
    core.cancel(&[2]);
    fixture.release();
    assert_eq!(
        collect(&core, &woke, 2)
            .into_iter()
            .map(|v| v.0)
            .collect::<Vec<_>>(),
        [1, 3]
    );
    until_settled(&core);
    assert_eq!(
        fixture.state.lock().unwrap().2,
        ["https://example.test/hold", "https://example.test/read"],
        "the aborted write never reached the transport"
    );
}

/// An app's abort closes a held write's connection on a real transport: the
/// server sees it end, never answered. `host` makes each owner's transport.
pub(crate) fn an_aborted_held_write_closes_its_connection(host: impl Fn() -> ibex2::host::Host) {
    use std::io::Read;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (heard, head) = channel();
    let (ended, closed) = channel();
    std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut bytes = [0u8; 4096];
        let n = socket.read(&mut bytes).unwrap_or(0);
        heard
            .send(String::from_utf8_lossy(&bytes[..n]).to_string())
            .unwrap();
        // Never answered: the request ends only when the client closes it.
        let end = loop {
            match socket.read(&mut bytes) {
                Ok(0) => break Ok(()),
                Ok(_) => continue,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    break Err("still open after 10 s".to_string())
                }
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                    break Err("still open after 10 s".to_string())
                }
                Err(_) => break Ok(()), // reset: closed too
            }
        };
        let _ = ended.send(end);
    });
    let grants = format!("net.fetch http://127.0.0.1:{port}");
    let owners = (0..WORKERS)
        .map(|_| Some(host().endow(ibex2::grant::GrantSet::parse(&grants).unwrap())))
        .collect();
    let core = Core::with_owners(owners, &grants, Box::new(|| {}));
    let url = format!("http://127.0.0.1:{port}/held");
    core.run(job(1, Request::post_json(&url, "{}")), None)
        .unwrap();
    let head = head
        .recv_timeout(Duration::from_secs(10))
        .expect("the write reached the server");
    assert!(head.starts_with("POST /held"), "{head}");
    let cancelled = std::time::Instant::now();
    core.cancel(&[1]);
    let end = closed
        .recv_timeout(Duration::from_secs(10))
        .expect("the server's read ended");
    assert_eq!(end, Ok(()), "the server saw the connection close");
    assert!(
        cancelled.elapsed() < Duration::from_secs(5),
        "closed promptly"
    );
    until_settled(&core);
    assert!(core.drain().is_empty(), "nothing was answered");
}

#[test]
fn an_app_aborted_held_write_closes_its_connection_on_the_platform_transport() {
    an_aborted_held_write_closes_its_connection(ibex2::host::Host::new);
}
