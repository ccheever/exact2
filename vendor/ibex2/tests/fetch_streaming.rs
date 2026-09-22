//! The authored fetch path settles at headers and shares one cancellable body.
#![cfg(all(feature = "hermes", feature = "loader"))]
mod common;

use ibex2::engine::hermes::{DynamicCode, Hermes};
use ibex2::loader::{ModuleGrants, Root};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

fn runtime(name: &str, source: &str, origin: &str) -> (common::Project, Hermes) {
    let project = common::Project::new(name);
    project.file("index.js", source);
    let mut rt = Hermes::new(DynamicCode::Closed).unwrap();
    assert!(rt.install_stdlib());
    rt.install_bindings().unwrap();
    rt.set_loader(
        Root::Declared(project.0.clone()),
        ModuleGrants::parse(&format!("[*]\nnet.fetch {origin}\n")).unwrap(),
    )
    .unwrap();
    rt.harden().unwrap();
    rt.run_entry("./index.js").unwrap();
    (project, rt)
}

fn head(stream: &mut TcpStream) {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        let mut byte = [0];
        stream.read_exact(&mut byte).unwrap();
        bytes.push(byte[0]);
    }
}

#[test]
fn abort_signal_algorithms_keep_reasons_and_notify_once() {
    let (_, mut rt) = runtime(
        "abort-signals",
        r#"
        const a = new AbortController(), b = new AbortController(), reason = {};
        const combined = AbortSignal.any([a.signal, b.signal]);
        let count = 0;
        a.signal.addEventListener('abort', () => count++, {once:true});
        a.abort(reason); a.abort('second');
        console.log(a.signal.aborted, a.signal.reason === reason, combined.reason === reason, count);
        try { combined.throwIfAborted(); } catch (e) { console.log(e === reason); }
        console.log(AbortSignal.abort().reason.name);
        const parent = new AbortController(), child = AbortSignal.any([parent.signal]), order = [];
        parent.signal.onabort = () => order.push('handler:' + child.aborted);
        parent.signal.addEventListener('abort', () => order.push('parent'));
        child.onabort = () => order.push('child');
        parent.abort(); console.log(order.join(','));
        AbortSignal.timeout(0).addEventListener('abort', function () { console.log(this.reason.name); });
        fetch('http://127.0.0.1:1', {signal:combined}).catch(e => console.log(e === reason));
        console.log(typeof __ibex2_fetch_control, typeof __ibex2_response_read, typeof __ibex2_abort);
        "#,
        "http://127.0.0.1:1",
    );
    rt.run_to_quiescence(Duration::from_secs(5));
    let out: Vec<_> = rt.drain_console().into_iter().map(|r| r.message).collect();
    assert_eq!(
        out,
        [
            "true true true 1",
            "true",
            "AbortError",
            "handler:true,parent,child",
            "undefined undefined undefined",
            "true",
            "TimeoutError"
        ]
    );
}

#[test]
fn response_arrives_before_body_finishes_and_readers_share_consumption() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let (release, wait) = mpsc::channel();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        head(&mut stream);
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: 5\r\nConnection: close\r\n\r\nh")
            .unwrap();
        wait.recv_timeout(Duration::from_secs(15)).unwrap();
        stream.write_all(b"ello").unwrap();
    });
    let source = format!(
        r#"
        fetch('{origin}').then(async response => {{
          console.log('headers', response.status, response.bodyUsed, response.body.locked);
          const first = response.body.getReader();
          console.log('locked', response.bodyUsed, response.body.locked);
          first.releaseLock();
          console.log(await response.text(), response.bodyUsed, response.status);
          try {{ await response.arrayBuffer(); }} catch (e) {{ console.log(e instanceof TypeError); }}
        }}).catch(e => console.log('ERROR', e.message));
    "#
    );
    let (_project, mut rt) = runtime("fetch-stream-headers", &source, &origin);
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    let mut out = Vec::new();
    while out.is_empty() && std::time::Instant::now() < deadline {
        rt.run_to_quiescence(Duration::from_millis(20));
        out.extend(rt.drain_console().into_iter().map(|r| r.message));
    }
    assert_eq!(out, ["headers 200 false false", "locked false true"]);
    release.send(()).unwrap();
    rt.run_to_quiescence(Duration::from_secs(5));
    out.extend(rt.drain_console().into_iter().map(|r| r.message));
    assert_eq!(&out[2..], ["hello true 200", "true"]);
    server.join().unwrap();
}

#[test]
fn abort_after_headers_rejects_pending_and_future_reads_with_the_same_reason() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        head(&mut stream);
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nTransfer-Encoding: chunked\r\n\r\n1\r\nx\r\n")
            .unwrap();
        let mut byte = [0];
        match stream.read(&mut byte) {
            Ok(0) => (),
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => (),
            result => panic!("aborted request did not close: {result:?}"),
        }
    });
    let source = format!(
        r#"
        const controller = new AbortController(), reason = {{ tag: 'cancelled' }};
        controller.signal.addEventListener('abort', () => {{
          throw {{ toString() {{ throw new Error('hostile reporting'); }} }};
        }});
        const dependent = AbortSignal.any([controller.signal]);
        fetch('{origin}', {{signal:dependent}}).then(async response => {{
          const reader = response.body.getReader();
          await reader.read();
          const pending = reader.read();
          await Promise.resolve();
          controller.abort(reason);
          try {{ await pending; }} catch (e) {{ console.log(e === reason); }}
          try {{ await reader.read(); }} catch (e) {{ console.log(e === reason); }}
          try {{ await reader.closed; }} catch (e) {{ console.log(e === reason); }}
        }}).catch(e => console.log('ERROR', e.message));
    "#
    );
    let (_project, mut rt) = runtime("fetch-stream-abort", &source, &origin);
    rt.run_to_quiescence(Duration::from_secs(15));
    let out: Vec<_> = rt.drain_console().into_iter().map(|r| r.message).collect();
    assert_eq!(out, ["true", "true", "true"]);
    server.join().unwrap();
}

#[test]
fn reader_release_preserves_an_inflight_chunk_for_the_next_reader() {
    let mut rt = Hermes::new(DynamicCode::Closed).unwrap();
    assert!(rt.install_stdlib());
    // A controllable native read completion exercises the JavaScript lock race
    // deterministically; the preceding tests exercise the real transport.
    rt.eval(include_str!("../src/bindings/headers.js")).unwrap();
    rt.eval(include_str!("../src/bindings/domexception.js"))
        .unwrap();
    rt.eval(include_str!("../src/bindings/abort.js")).unwrap();
    rt.eval(
        r#"
        var resolveRead, reads = 0, releases = 0;
        globalThis.__ibex2_response_read = function () {
          reads++;
          if (reads > 1) return Promise.resolve(null);
          return new Promise(resolve => { resolveRead = resolve; });
        };
        globalThis.__ibex2_response_field = function (handle, field) {
          if (field === 0) return 200;
          if (field === 1) return true;
          if (field === 2) return 'http://example.test/';
          if (field === 5) return false;
          if (field === 7) return '[]';
          if (field === 8) releases++;
        };
        globalThis.__ibex2_fetch_control = function () { return 1; };
        globalThis.__ibex2_response_own = function (handle, body) { return () => body; };
    "#,
    )
    .unwrap();
    rt.eval(&format!(
        "var makeFetch = {}; var fetch = makeFetch(() => Promise.resolve(1));",
        include_str!("../src/bindings/fetch.js")
    ))
    .unwrap();
    rt.eval(
        r#"
        fetch('http://example.test').then(async response => {
          const first = response.body.getReader();
          const old = first.read().catch(e => e instanceof TypeError);
          await Promise.resolve();
          first.releaseLock();
          const second = response.body.getReader();
          const next = second.read();
          resolveRead(new Uint8Array([104, 105]).buffer);
          console.log(await old, Array.from((await next).value).join(','));
          console.log((await second.read()).done, reads, releases);
          second.releaseLock();
          try { await second.closed; } catch (e) { console.log(e instanceof TypeError); }
        }).catch(e => console.log('ERROR', e.message));
    "#,
    )
    .unwrap();
    rt.drain_microtasks().unwrap();
    let out: Vec<_> = rt.drain_console().into_iter().map(|r| r.message).collect();
    assert_eq!(out, ["true 104,105", "true 2 1", "true"]);
}

#[test]
fn collecting_a_discarded_response_cancels_even_with_a_live_signal() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let (closed, wait) = mpsc::channel();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        head(&mut stream);
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nTransfer-Encoding: chunked\r\n\r\n1\r\nx\r\n").unwrap();
        let mut byte = [0];
        match stream.read(&mut byte) {
            Ok(0) => (),
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => (),
            result => panic!("discarded body did not close: {result:?}"),
        }
        closed.send(()).unwrap();
    });
    let source = format!(
        r#"
        globalThis.keepController = new AbortController();
        fetch('{origin}', {{signal:keepController.signal}}).then(response => console.log(response.status));
    "#
    );
    let (_project, mut rt) = runtime("fetch-stream-gc", &source, &origin);
    rt.run_to_quiescence(Duration::from_secs(15));
    let out: Vec<_> = rt.drain_console().into_iter().map(|r| r.message).collect();
    assert_eq!(out, ["200"]);
    assert!(rt.collect_garbage());
    wait.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(rt.eval("keepController.signal.aborted").unwrap(), "false");
    server.join().unwrap();
}
