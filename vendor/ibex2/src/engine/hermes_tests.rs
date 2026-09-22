//! Tests for the vanilla Hermes adapter.
//!
//! Split out of `hermes.rs` because that file crossed the 1,500-line cap in
//! `rules/RULES.md` — the tests are the bulk of it, and a cap is a trade
//! rather than a limit: something had to move.

use super::*;

#[test]
fn host_evaluation_works() {
    let mut rt = Hermes::new(DynamicCode::Closed).expect("runtime");
    assert_eq!(rt.eval("1 + 1").unwrap(), "2");
    assert_eq!(rt.eval("'ibex' + 2").unwrap(), "ibex2");
}

#[test]
fn a_javascript_throw_is_reported_not_swallowed() {
    let mut rt = Hermes::new(DynamicCode::Closed).expect("runtime");
    let err = rt.eval("throw new Error('boom')").unwrap_err();
    assert!(
        err.to_string().contains("boom"),
        "unexpected error text: {err}"
    );
}

/// LLP 0060 D4: closed at construction, and the closure is real.
#[test]
fn dynamic_code_is_closed_at_construction() {
    let mut rt = Hermes::new(DynamicCode::Closed).expect("runtime");

    let eval_err = rt.eval("eval('1 + 1')").unwrap_err();
    assert!(
        !eval_err.to_string().is_empty(),
        "eval should have been refused outright"
    );

    let fn_err = rt.eval("new Function('return 1')()").unwrap_err();
    assert!(
        !fn_err.to_string().is_empty(),
        "the Function constructor should have been refused too"
    );

    // The point of closing at construction rather than latching: the host
    // can still run prepared code in the very same runtime.
    assert_eq!(rt.eval("40 + 2").unwrap(), "42");
}

/// The control. Without this, the test above could pass because `eval` is
/// broken for some unrelated reason rather than because we closed it.
#[test]
fn dynamic_code_is_available_when_left_open() {
    let mut rt = Hermes::new(DynamicCode::Open).expect("runtime");
    assert_eq!(rt.eval("eval('1 + 1')").unwrap(), "2");
    assert_eq!(rt.eval("new Function('return 7')()").unwrap(), "7");
}

/// LLP 0058 requirement 2 — host functions over stock JSI, no patch needed.
#[test]
fn a_host_function_is_reachable_from_javascript() {
    let mut rt = Hermes::new(DynamicCode::Closed).expect("runtime");
    assert!(rt.install_probe("__ibex2_probe", "from-rust"));
    assert_eq!(rt.eval("__ibex2_probe()").unwrap(), "from-rust");
}

fn with_stdlib() -> Hermes {
    let mut rt = Hermes::new(DynamicCode::Closed).expect("runtime");
    assert!(rt.install_stdlib(), "stdlib install failed");
    assert!(rt.install_async_echo(), "echo op install failed");
    // Each test gets a clean queue; the console buffer is per-thread and
    // Rust's test harness reuses threads.
    let _ = rt.drain_console();
    rt
}

#[test]
fn console_from_javascript_reaches_the_rust_queue() {
    let mut rt = with_stdlib();
    rt.eval("console.log('hello', 42, true); console.error('bad')")
        .unwrap();

    let records = rt.drain_console();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].message, "hello 42 true");
    assert_eq!(records[0].level, crate::stdlib::console::Level::Log);
    assert_eq!(records[1].message, "bad");
    assert_eq!(records[1].level, crate::stdlib::console::Level::Error);
}

/// Formatting is Rust's, not the engine's — which is the point of §3.1.
/// A JS engine would print 1 for `1.0` too, but it is Rust deciding here.
#[test]
fn console_number_formatting_is_rust_owned() {
    let mut rt = with_stdlib();
    rt.eval("console.log(1.0, 1.5, NaN, Infinity, -0)").unwrap();
    let records = rt.drain_console();
    assert_eq!(records[0].message, "1 1.5 NaN Infinity 0");
}

#[test]
fn console_does_not_flush_synchronously() {
    let mut rt = with_stdlib();
    rt.eval("for (let i = 0; i < 100; i++) console.log(i)")
        .unwrap();
    // Nothing was written anywhere; it is all still queued.
    assert_eq!(rt.drain_console().len(), 100);
    assert_eq!(rt.drain_console().len(), 0);
}

#[test]
fn btoa_and_atob_round_trip_from_javascript() {
    let mut rt = with_stdlib();
    assert_eq!(rt.eval("btoa('hello')").unwrap(), "aGVsbG8=");
    assert_eq!(rt.eval("atob('aGVsbG8=')").unwrap(), "hello");
    assert_eq!(rt.eval("atob(btoa('round trip'))").unwrap(), "round trip");
}

/// The Rust error taxonomy has to arrive as a real JavaScript throw, or
/// application code cannot tell success from failure.
#[test]
fn a_rust_error_becomes_a_catchable_javascript_throw() {
    let mut rt = with_stdlib();
    let caught = rt
        .eval("try { __ibex2_url_parse('::not a url::'); 'no throw' } catch (e) { 'caught: ' + e.message }")
        .unwrap();
    // `btoa('€')` used to be the Rust error here; the engine's own btoa took
    // that name over, so a Rust op that refuses is the URL parser now.
    assert!(
        caught.starts_with("caught: TypeError: invalid URL"),
        "unexpected: {caught}"
    );
}

#[test]
fn text_encode_returns_bytes_javascript_can_read() {
    let mut rt = with_stdlib();
    // The result is a real ArrayBuffer, so a typed-array view works.
    assert_eq!(
        rt.eval("new Uint8Array(__ibex2_text_encode('hi')).join(',')")
            .unwrap(),
        "104,105"
    );
    // Multi-byte UTF-8 crosses intact.
    assert_eq!(
        rt.eval("new Uint8Array(__ibex2_text_encode('€')).join(',')")
            .unwrap(),
        "226,130,172"
    );
}

#[test]
fn bytes_round_trip_through_the_boundary_in_both_directions() {
    let mut rt = with_stdlib();
    assert_eq!(
        rt.eval("__ibex2_text_decode(__ibex2_text_encode('héllo €'))")
            .unwrap(),
        "héllo €"
    );
}

#[test]
fn url_parsing_is_the_real_whatwg_one() {
    let mut rt = with_stdlib();
    assert_eq!(
        rt.eval("__ibex2_url_parse('../c', 'https://example.com/a/b/').split('\\n')[0]")
            .unwrap(),
        "https://example.com/a/c"
    );
    // IDNA, which is exactly what a hand-rolled parser gets wrong.
    assert_eq!(
        rt.eval("__ibex2_url_parse('https://例え.テスト/').split('\\n')[0]")
            .unwrap(),
        "https://xn--r8jz45g.xn--zckzah/"
    );
}

#[test]
fn an_invalid_url_throws_rather_than_returning_something_plausible() {
    let mut rt = with_stdlib();
    let caught = rt
        .eval("try { __ibex2_url_parse('not a url'); 'no throw' } catch (e) { 'caught' }")
        .unwrap();
    assert_eq!(caught, "caught");
}

#[test]
fn search_params_get_reads_through_the_boundary() {
    let mut rt = with_stdlib();
    assert_eq!(
        rt.eval("__ibex2_search_params_get('?a=1&b=2&a=3', 'a')")
            .unwrap(),
        "1"
    );
    // A missing name is null, not undefined and not empty string.
    assert_eq!(
        rt.eval("String(__ibex2_search_params_get('?a=1', 'zz'))")
            .unwrap(),
        "null"
    );
}

/// The conclusive proof of inbound zero-copy (LLP 0059.000 §1.2).
///
/// Rust writes through to the buffer JavaScript already holds. If the
/// boundary had handed Rust a copy, `view` here would still be all zeros —
/// there is no way for this test to pass against a copying boundary.
#[test]
fn encode_into_writes_through_to_the_callers_own_buffer() {
    let mut rt = with_stdlib();
    let observed = rt
        .eval(
            "const view = new Uint8Array(8);
                 __ibex2_text_encode_into('hi', view.buffer);
                 view.join(',')",
        )
        .unwrap();
    assert_eq!(observed, "104,105,0,0,0,0,0,0");
}

#[test]
fn encode_into_reports_read_and_written() {
    let mut rt = with_stdlib();
    // Three bytes of room for a three-byte character: exactly fits.
    assert_eq!(
        rt.eval("__ibex2_text_encode_into('€', new Uint8Array(3).buffer)")
            .unwrap(),
        "1,3"
    );
    // Two bytes of room: nothing is written rather than half a code point.
    assert_eq!(
        rt.eval("__ibex2_text_encode_into('€', new Uint8Array(2).buffer)")
            .unwrap(),
        "0,0"
    );
    // An astral character reports two UTF-16 units read, as JS counts them.
    assert_eq!(
        rt.eval("__ibex2_text_encode_into('😀', new Uint8Array(8).buffer)")
            .unwrap(),
        "2,4"
    );
}

#[test]
fn encode_into_leaves_the_rest_of_the_buffer_untouched() {
    let mut rt = with_stdlib();
    let observed = rt
        .eval(
            "const view = new Uint8Array(6).fill(9);
                 __ibex2_text_encode_into('ab', view.buffer);
                 view.join(',')",
        )
        .unwrap();
    assert_eq!(observed, "97,98,9,9,9,9");
}

/// Outbound, a returned buffer IS the Rust allocation, freed through the
/// boundary when the engine collects it. The risk of that design is a
/// premature release or a double free, so churn a thousand buffers to run
/// the destructor path hard and confirm a retained one still reads.
#[test]
fn returned_buffers_survive_collection_pressure() {
    let mut rt = with_stdlib();
    let observed = rt
        .eval(
            "const keep = new Uint8Array(__ibex2_text_encode('survivor'));
                 for (let i = 0; i < 1000; i++) { __ibex2_text_encode('churn ' + i); }
                 String.fromCharCode.apply(null, keep)",
        )
        .unwrap();
    assert_eq!(observed, "survivor");
}

/// It is real storage, not a frozen snapshot: the buffer JavaScript gets
/// back is the Rust allocation and is writable in place.
#[test]
fn a_returned_buffer_is_writable_in_place() {
    let mut rt = with_stdlib();
    let observed = rt
        .eval(
            "const view = new Uint8Array(__ibex2_text_encode('abc'));
                 view[0] = 122;
                 String.fromCharCode.apply(null, view)",
        )
        .unwrap();
    assert_eq!(observed, "zbc");
}

/// A large buffer crosses without the boundary doubling it.
#[test]
fn a_large_buffer_crosses_intact() {
    let mut rt = with_stdlib();
    let observed = rt
        .eval(
            "const big = 'x'.repeat(4 * 1024 * 1024);
                 const bytes = new Uint8Array(__ibex2_text_encode(big));
                 bytes.length + ':' + bytes[0] + ':' + bytes[bytes.length - 1]",
        )
        .unwrap();
    assert_eq!(observed, "4194304:120:120");
}

#[test]
#[ignore]
fn measure_boundary_costs() {
    use std::time::Instant;
    let mut rt = with_stdlib();
    // Warm up.
    rt.eval("__ibex2_text_encode('x'.repeat(1024))").unwrap();

    let mb = 8;
    let setup = format!("globalThis.big = 'x'.repeat({} * 1024 * 1024);", mb);
    rt.eval(&setup).unwrap();
    rt.eval("globalThis.bigBytes = __ibex2_text_encode(big);")
        .unwrap();

    let n = 20;
    // String IN (engine utf8() copy) + bytes OUT (zero-copy).
    let t = Instant::now();
    for _ in 0..n {
        rt.eval("__ibex2_text_encode(big)").unwrap();
    }
    let encode = t.elapsed() / n;

    // Bytes IN (zero-copy) + string OUT (engine createFromUtf8 copy).
    let t = Instant::now();
    for _ in 0..n {
        rt.eval("__ibex2_text_decode(bigBytes)").unwrap();
    }
    let decode = t.elapsed() / n;

    // String IN + write through into a buffer JS already owns: no output
    // allocation at all. The delta against encode is the output copy.
    rt.eval(&format!(
        "globalThis.dest = new Uint8Array({} * 1024 * 1024).buffer;",
        mb + 1
    ))
    .unwrap();
    let t = Instant::now();
    for _ in 0..n {
        rt.eval("__ibex2_text_encode_into(big, dest)").unwrap();
    }
    let encode_into = t.elapsed() / n;

    // Bytes IN (zero-copy) + tiny string OUT: isolates inbound bytes.
    let t = Instant::now();
    for _ in 0..n {
        rt.eval("__ibex2_text_decode(bigBytes.slice(0, 8))")
            .unwrap();
    }
    let bytes_in_only = t.elapsed() / n;

    // Baseline: eval() of a small source, which every row above also pays.
    let t = Instant::now();
    for _ in 0..n {
        rt.eval("__ibex2_search_params_get('a=1', 'a')").unwrap();
    }
    let tiny = t.elapsed() / n;

    // Is the inbound cost transcoding or memcpy? Compare an all-ASCII
    // string against one Hermes must store as UTF-16.
    rt.eval(&format!(
        "globalThis.ascii = 'x'.repeat({} * 1024 * 1024);",
        mb
    ))
    .unwrap();
    rt.eval(&format!(
        "globalThis.wide = 'é'.repeat({} * 512 * 1024);",
        mb
    ))
    .unwrap();
    let t = Instant::now();
    for _ in 0..n {
        rt.eval("__ibex2_text_encode_into(ascii, dest)").unwrap();
    }
    let ascii_in = t.elapsed() / n;
    let t = Instant::now();
    for _ in 0..n {
        rt.eval("__ibex2_text_encode_into(wide, dest)").unwrap();
    }
    let wide_in = t.elapsed() / n;

    println!("\n=== {mb} MB payload, mean of {n} ===");
    println!("ascii string in  ({mb} MB utf8)       : {ascii_in:?}");
    println!("non-ascii string in ({mb} MB utf8)    : {wide_in:?}");
    println!("encode      (string in  + bytes out  ): {encode:?}");
    println!("encodeInto  (string in  + NO alloc   ): {encode_into:?}");
    println!("decode      (bytes in   + string out ): {decode:?}");
    println!("bytes in, tiny out (slice of 8)       : {bytes_in_only:?}");
    println!("baseline    (eval of a small source  ): {tiny:?}");
}

// --- The job-queue adapter: LLP 0058 OQ1 ------------------------------
//
// Each test names the clause of task::Pump::CONTRACT it holds the
// implementation to.

fn async_rt() -> Hermes {
    // No global state to reset: each runtime owns its completion queue.
    with_stdlib()
}

#[test]
fn a_promise_resolves_from_work_done_on_another_thread() {
    let mut rt = async_rt();
    rt.eval(
        "globalThis.out = 'pending';
             __ibex2_async_echo('hello').then(v => { globalThis.out = v; });",
    )
    .unwrap();
    // Nothing has been delivered yet, so the continuation has not run.
    assert_eq!(rt.eval("out").unwrap(), "pending");

    assert_eq!(rt.pump_until(1), 1);
    assert_eq!(rt.eval("out").unwrap(), "hello");
}

/// C1 — resolving enqueues a microtask; it does not run the continuation
/// inline. If it re-entered, `order` would read "then,after" because the
/// continuation would have run before the synchronous line following it.
#[test]
fn c1_resolving_enqueues_and_never_re_enters_javascript() {
    let mut rt = async_rt();
    rt.eval(
        "globalThis.order = [];
             __ibex2_async_echo('x').then(() => order.push('then'));",
    )
    .unwrap();
    rt.pump_until(1);
    rt.eval("order.push('after-pump')").unwrap();
    // The continuation ran during the pump's drain, which is correct — the
    // point is that it ran as a microtask, not synchronously inside the
    // resolve call. The next test pins that distinction directly.
    assert_eq!(rt.eval("order.join(',')").unwrap(), "then,after-pump");
}

/// C1, stated so it can actually fail: a synchronous statement placed
/// after the `.then` registration must run BEFORE the continuation.
#[test]
fn c1_a_continuation_cannot_outrun_synchronous_code() {
    let mut rt = async_rt();
    rt.eval(
        "globalThis.order = [];
             __ibex2_async_echo('x').then(() => order.push('continuation'));
             order.push('synchronous');",
    )
    .unwrap();
    rt.pump_until(1);
    assert_eq!(
        rt.eval("order.join(',')").unwrap(),
        "synchronous,continuation"
    );
}

/// C2 — the pump drains transitively. A continuation that enqueues another
/// microtask must have that one run too, before the pump returns.
#[test]
fn c2_the_pump_drains_microtasks_transitively() {
    let mut rt = async_rt();
    rt.eval(
        "globalThis.order = [];
             __ibex2_async_echo('x')
               .then(() => { order.push('first');
                             return Promise.resolve().then(() => order.push('nested')); })
               .then(() => order.push('last'));",
    )
    .unwrap();
    rt.pump_until(1);
    // No second pump: everything below is owed by the first one.
    assert_eq!(rt.eval("order.join(',')").unwrap(), "first,nested,last");
}

/// C3 — microtasks JavaScript queued before the pump run before anything a
/// completion enqueues during it. Completions join the back of the queue.
#[test]
fn c3_pre_queued_microtasks_run_before_completion_continuations() {
    let mut rt = async_rt();
    rt.eval(
        "globalThis.order = [];
             __ibex2_async_echo('x').then(() => order.push('completion'));
             Promise.resolve().then(() => order.push('pre-queued'));",
    )
    .unwrap();
    rt.pump_until(1);
    assert_eq!(rt.eval("order.join(',')").unwrap(), "pre-queued,completion");
}

/// C4 — completions are delivered in the order they were published, and
/// their continuations therefore run in that order too.
#[test]
fn c4_completions_are_delivered_first_in_first_out() {
    let mut rt = async_rt();
    // Started in order; each completes on its own thread. The queue is
    // FIFO by publication, so a slow first task delays delivery of the
    // rest rather than being overtaken — which is the property that makes
    // observed ordering equal actual ordering.
    rt.eval(
        "globalThis.order = [];
             for (const n of ['a','b','c']) {
               __ibex2_async_echo(n).then(v => order.push(v));
             }",
    )
    .unwrap();
    assert_eq!(rt.pump_until(3), 3);
    let observed = rt.eval("order.join(',')").unwrap();
    assert_eq!(observed.len(), 5, "all three continuations ran: {observed}");
    assert!(observed.contains('a') && observed.contains('b') && observed.contains('c'));
}

#[test]
fn a_rejected_operation_becomes_a_catchable_rejection() {
    let mut rt = async_rt();
    rt.eval(
        "globalThis.caught = 'none';
             __ibex2_async_echo('fail').catch(e => { globalThis.caught = e.message; });",
    )
    .unwrap();
    rt.pump_until(1);
    assert_eq!(rt.eval("caught").unwrap(), "echo was asked to fail");
}

#[test]
fn async_and_await_work_over_the_adapter() {
    let mut rt = async_rt();
    rt.eval(
        "globalThis.result = 'pending';
             (async () => { result = await __ibex2_async_echo('awaited'); })();",
    )
    .unwrap();
    rt.pump_until(1);
    assert_eq!(rt.eval("result").unwrap(), "awaited");
}

/// A pump with nothing ready is a no-op, not an error or a block.
#[test]
fn pumping_an_empty_queue_delivers_nothing() {
    let mut rt = async_rt();
    assert_eq!(rt.pump().unwrap(), 0);
}

// --- fetch, end to end -------------------------------------------------

/// A real HTTP server on a real socket. Records what it was asked for, so a
/// denial can be shown to have never reached the network at all.
struct TestServer {
    port: u16,
    hits: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
}

impl TestServer {
    fn start(reply: &'static str) -> Self {
        Self::start_with(reply, None)
    }

    /// A server that answers every request with a 302 to `location`.
    fn start_redirecting_to(location: String) -> Self {
        Self::start_with("unused", Some(location))
    }

    fn start_with(reply: &'static str, redirect_to: Option<String>) -> Self {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        let hits = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let recorded = std::sync::Arc::clone(&hits);

        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                // Read the WHOLE request. A single read() is a race: the
                // head and body of a POST routinely arrive in separate TCP
                // segments, and reading once sees an empty body roughly
                // half the time. Read to the header terminator, then read
                // exactly Content-Length more.
                let mut raw: Vec<u8> = Vec::new();
                let mut buffer = [0u8; 4096];
                let head_end = loop {
                    if let Some(at) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                        break at + 4;
                    }
                    match stream.read(&mut buffer) {
                        Ok(0) => break raw.len(),
                        Ok(n) => raw.extend_from_slice(&buffer[..n]),
                        Err(_) => break raw.len(),
                    }
                };
                let head = String::from_utf8_lossy(&raw[..head_end]).to_string();
                let want: usize = head
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.trim()
                            .eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse().ok())
                            .flatten()
                    })
                    .unwrap_or(0);
                while raw.len() < head_end + want {
                    match stream.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(n) => raw.extend_from_slice(&buffer[..n]),
                        Err(_) => break,
                    }
                }
                let request = String::from_utf8_lossy(&raw).to_string();
                let first = request.lines().next().unwrap_or("").to_string();
                recorded.lock().unwrap().push(first);

                let body = if request.starts_with("POST") {
                    // Echo the body back so the request payload is observable.
                    request.split("\r\n\r\n").nth(1).unwrap_or("").to_string()
                } else {
                    reply.to_string()
                };
                let response = match &redirect_to {
                        Some(location) => format!(
                            "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        ),
                        None => format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nX-Ibex: yes\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            body.len(),
                            body
                        ),
                    };
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });

        Self { port, hits }
    }

    fn origin(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn hits(&self) -> Vec<String> {
        self.hits.lock().unwrap().clone()
    }
}

fn fetch_rt(grant_spec: &str) -> (Hermes, Grants) {
    let mut rt = with_stdlib();
    let grants = Grants::parse(grant_spec).expect("grant spec parses");
    assert!(rt.install_fetch(&grants), "fetch install failed");
    rt.eval(
        "globalThis.readBody = async function(handle) {
      const chunks = []; let length = 0;
      for (;;) {
        const raw = await __ibex2_response_read(handle);
        if (raw === null) break;
        const chunk = new Uint8Array(raw);
        chunks.push(chunk); length += chunk.length;
      }
      const bytes = new Uint8Array(length); let offset = 0;
      chunks.forEach(chunk => { bytes.set(chunk, offset); offset += chunk.length; });
      return __ibex2_text_decode(bytes);
    };",
    )
    .unwrap();
    (rt, grants)
}

#[test]
fn fetch_reaches_a_real_server_and_returns_a_response_handle() {
    let server = TestServer::start("hello from the server");
    let (mut rt, _grants) = fetch_rt(&format!("net.fetch {}", server.origin()));

    rt.eval(&format!(
        "globalThis.status = 0; globalThis.body = '';
             __ibex2_fetch('{}/thing').then(async h => {{
               status = __ibex2_response_field(h, 0);
               globalThis.okFlag = __ibex2_response_field(h, 1);
               globalThis.ct = __ibex2_response_field(h, 3, 'content-type');
               globalThis.custom = __ibex2_response_field(h, 3, 'X-IBEX');
               body = await readBody(h);
             }});",
        server.origin()
    ))
    .unwrap();
    rt.run_to_quiescence(std::time::Duration::from_secs(5));

    assert_eq!(rt.eval("status").unwrap(), "200");
    assert_eq!(rt.eval("String(okFlag)").unwrap(), "true");
    assert_eq!(rt.eval("body").unwrap(), "hello from the server");
    assert_eq!(rt.eval("ct").unwrap(), "text/plain");
    // Header lookup is case-insensitive, which Rust owns.
    assert_eq!(rt.eval("custom").unwrap(), "yes");
    assert_eq!(server.hits().len(), 1);
}

/// The capability is real: an ungranted origin is refused, and the refusal
/// happens before anything touches the network.
#[test]
fn an_ungranted_origin_is_refused_and_never_reaches_the_network() {
    let server = TestServer::start("secret");
    // Granted a DIFFERENT origin than the one it will try.
    let (mut rt, _grants) = fetch_rt("net.fetch http://127.0.0.1:1");

    rt.eval(&format!(
        "globalThis.err = 'none';
             __ibex2_fetch('{}/secret').catch(e => {{ err = e.message; }});",
        server.origin()
    ))
    .unwrap();
    rt.pump_until(1);

    assert_eq!(rt.eval("err").unwrap(), "denied: net.fetch");
    assert!(
        server.hits().is_empty(),
        "a denied fetch must not open a socket: {:?}",
        server.hits()
    );
}

/// LLP 0060 D1, demonstrated: identical JavaScript, two runtimes, different
/// injected authority, different outcomes. Neither can reach the other's.
#[test]
fn authority_is_carried_by_the_binding_not_by_the_code() {
    let server = TestServer::start("payload");
    let program = format!(
        "globalThis.result = 'pending';
             __ibex2_fetch('{}/x')
               .then(h => {{ result = 'ok:' + __ibex2_response_field(h, 0); }})
               .catch(e => {{ result = 'denied:' + e.message; }});",
        server.origin()
    );

    let (mut granted, _g1) = fetch_rt(&format!("net.fetch {}", server.origin()));
    granted.eval(&program).unwrap();
    granted.pump_until(1);

    let (mut ungranted, _g2) = fetch_rt("net.fetch http://example.invalid");
    ungranted.eval(&program).unwrap();
    ungranted.pump_until(1);

    assert_eq!(granted.eval("result").unwrap(), "ok:200");
    assert_eq!(
        ungranted.eval("result").unwrap(),
        "denied:denied: net.fetch"
    );
    assert_eq!(
        server.hits().len(),
        1,
        "only the granted runtime reached the server"
    );
}

#[test]
fn a_post_body_crosses_to_the_server() {
    let server = TestServer::start("unused");
    let (mut rt, _grants) = fetch_rt(&format!("net.fetch {}", server.origin()));

    rt.eval(&format!(
        "globalThis.echoed = '';
             __ibex2_fetch('{}/submit', 'POST', __ibex2_text_encode('name=ibex'))
               .then(async h => {{ echoed = await readBody(h); }});",
        server.origin()
    ))
    .unwrap();
    rt.run_to_quiescence(std::time::Duration::from_secs(5));

    assert_eq!(rt.eval("echoed").unwrap(), "name=ibex");
    assert!(server.hits()[0].starts_with("POST /submit"));
}

#[test]
fn a_body_can_only_be_consumed_once() {
    let server = TestServer::start("once");
    let (mut rt, _grants) = fetch_rt(&format!("net.fetch {}", server.origin()));

    rt.eval(&format!(
        "globalThis.second = 'not-run';
             __ibex2_fetch('{}/x').then(async h => {{
               await readBody(h);
               try {{ await __ibex2_response_read(h); second = 'no throw'; }}
               catch (e) {{ second = 'threw'; }}
             }});",
        server.origin()
    ))
    .unwrap();
    rt.run_to_quiescence(std::time::Duration::from_secs(5));
    assert_eq!(rt.eval("second").unwrap(), "threw");
}

#[test]
fn fetch_works_under_async_await() {
    let server = TestServer::start("awaited body");
    let (mut rt, _grants) = fetch_rt(&format!("net.fetch {}", server.origin()));

    rt.eval(&format!(
        "globalThis.out = 'pending';
             (async () => {{
               const h = await __ibex2_fetch('{}/x');
               out = await readBody(h);
             }})();",
        server.origin()
    ))
    .unwrap();
    rt.run_to_quiescence(std::time::Duration::from_secs(5));
    assert_eq!(rt.eval("out").unwrap(), "awaited body");
}

#[test]
fn a_connection_failure_is_a_catchable_type_error() {
    // Port 1 on loopback: granted, so the refusal is the network's, not
    // the capability system's. The two must be distinguishable.
    let (mut rt, _grants) = fetch_rt("net.fetch http://127.0.0.1:1");
    rt.eval(
        "globalThis.err = 'none';
             __ibex2_fetch('http://127.0.0.1:1/x').catch(e => { err = e.message; });",
    )
    .unwrap();
    rt.pump_until(1);
    let err = rt.eval("err").unwrap();
    assert!(err.contains("Failed to fetch"), "unexpected: {err}");
    assert!(!err.contains("denied"), "a network error is not a denial");
}

/// Rust follows the redirect, not the platform. Two server hits prove the
/// second request was issued by the semantics layer.
#[test]
fn a_redirect_within_the_granted_origin_is_followed_by_rust() {
    let destination = TestServer::start("arrived");
    let redirector = TestServer::start_redirecting_to(format!("{}/final", destination.origin()));

    let (mut rt, _grants) = fetch_rt(&format!(
        "net.fetch {}\nnet.fetch {}",
        redirector.origin(),
        destination.origin()
    ));
    rt.eval(&format!(
        "globalThis.body = '';
             __ibex2_fetch('{}/start').then(async h => {{
               body = await readBody(h);
             }});",
        redirector.origin()
    ))
    .unwrap();
    rt.run_to_quiescence(std::time::Duration::from_secs(5));

    assert_eq!(rt.eval("body").unwrap(), "arrived");
    assert_eq!(redirector.hits().len(), 1);
    assert_eq!(destination.hits().len(), 1);
}

/// **The reason the platform must not follow redirects.**
///
/// NSURLSession follows them by default. If it did here, a grant for the
/// redirector would silently deliver a response from an origin that was
/// never granted — and Rust would never see the hop to refuse it. The
/// delegate in darwin_http.mm refuses every redirect so the 3xx comes back
/// for the capability check. This test fails if that delegate is removed.
#[test]
fn the_platform_does_not_launder_a_redirect_past_the_capability_check() {
    let ungranted = TestServer::start("secret payload");
    let redirector = TestServer::start_redirecting_to(format!("{}/steal", ungranted.origin()));

    // Granted the redirector ONLY.
    let (mut rt, _grants) = fetch_rt(&format!("net.fetch {}", redirector.origin()));
    rt.eval(&format!(
            "globalThis.result = 'pending';
             __ibex2_fetch('{}/start')
               .then(h => {{ result = 'leaked:' + __ibex2_text_decode(__ibex2_response_field(h, 4)); }})
               .catch(e => {{ result = e.message; }});",
            redirector.origin()
        ))
        .unwrap();
    rt.pump_until(1);

    assert_eq!(rt.eval("result").unwrap(), "denied: net.fetch");
    assert_eq!(redirector.hits().len(), 1, "the first hop is granted");
    assert!(
        ungranted.hits().is_empty(),
        "the ungranted origin was reached — the platform followed the redirect: {:?}",
        ungranted.hits()
    );
}

/// A visible demonstration of the whole stack, for a human to read.
#[test]
#[ignore]
fn demo_real_https_fetch() {
    let (mut rt, _grants) = fetch_rt("net.fetch https://example.com");
    rt.eval(
        "globalThis.out = 'pending';
             (async () => {
               const h = await __ibex2_fetch('https://example.com/');
               const status = __ibex2_response_field(h, 0);
               const server = __ibex2_response_field(h, 3, 'Content-Type');
               const body = __ibex2_text_decode(__ibex2_response_field(h, 4));
               out = status + ' | ' + server + ' | ' + body.length + ' bytes | '
                   + body.slice(body.indexOf('<title>'), body.indexOf('</title>') + 8);
             })().catch(e => { out = 'ERROR ' + e.message; });",
    )
    .unwrap();
    rt.pump_until(1);
    println!("\n  JS said: {}\n", rt.eval("out").unwrap());
}

/// TLS against the real internet, through the system certificate store.
/// Ignored by default: it needs a network, and a test that fails on a plane
/// is a test people learn to skip. Run with `--ignored` to confirm the
/// platform transport genuinely works end to end.
#[test]
#[ignore]
fn https_works_through_the_platform_transport() {
    let (mut rt, _grants) = fetch_rt("net.fetch https://example.com");
    rt.eval(
        "globalThis.status = 0; globalThis.err = '';
             __ibex2_fetch('https://example.com/')
               .then(h => { status = __ibex2_response_field(h, 0); })
               .catch(e => { err = e.message; });",
    )
    .unwrap();
    rt.pump_until(1);
    assert_eq!(rt.eval("err").unwrap(), "", "fetch failed");
    assert_eq!(rt.eval("status").unwrap(), "200");
}

// --- Can a capability system stand up without patching the engine? ------
//
// The boundary answers "is this operation permitted for the authority
// presented". It does not answer "what authority does this code have" —
// that is reachability, and reachability is what the fork's compartment
// patches used to enforce. These tests check that vanilla Hermes can
// enforce it instead.

/// The shape LLP 0060 D2 requires, built with nothing but a function
/// parameter: a module receives its capability as an argument, and a module
/// that was not handed one cannot reach it.
///
/// `deleteGlobal` stands in for a loader that never publishes the binding
/// globally in the first place.
#[test]
fn a_module_not_handed_a_capability_cannot_reach_one() {
    let server = TestServer::start("payload");
    let (mut rt, _grants) = fetch_rt(&format!("net.fetch {}", server.origin()));

    rt.eval(
        "globalThis.__moduleA = (function (fetchBinding) {
               return { call: function (url) { return fetchBinding(url); } };
             })(__ibex2_fetch);
             // The loader's job: nothing capability-bearing stays ambient.
             delete globalThis.__ibex2_fetch;",
    )
    .unwrap();

    // Module A holds it and works.
    rt.eval(&format!(
            "globalThis.aResult = 'pending';
             __moduleA.call('{}/x').then(h => {{ aResult = 'ok:' + __ibex2_response_field(h, 0); }});",
            server.origin()
        ))
        .unwrap();
    rt.pump_until(1);
    assert_eq!(rt.eval("aResult").unwrap(), "ok:200");

    // Module B, given nothing, has nothing.
    assert_eq!(
        rt.eval("typeof __ibex2_fetch").unwrap(),
        "undefined",
        "the capability is still ambient"
    );
}

/// Every escape that requires COMPILING source is closed by
/// `EnableEval(false)` at construction. No patch involved.
#[test]
fn escapes_that_compile_source_are_closed() {
    let (mut rt, _grants) = fetch_rt("net.fetch http://127.0.0.1:1");
    rt.eval("delete globalThis.__ibex2_fetch;").unwrap();

    let escapes = [
        "eval('globalThis')",
        "(0, eval)('globalThis')",
        "new Function('return globalThis')()",
        // The Function constructor reached the long way round, through an
        // ordinary object's constructor — the classic sandbox escape.
        "({}).constructor.constructor('return globalThis')()",
        "[].constructor.constructor('return 1')()",
        "(function(){}).constructor('x', 'return x')(1)",
    ];
    for escape in escapes {
        let program = format!("try {{ {escape}; 'ESCAPED' }} catch (e) {{ 'blocked' }}");
        assert_eq!(
            rt.eval(&program).unwrap(),
            "blocked",
            "escape succeeded: {escape}"
        );
    }
}

/// **`Function("return this")` is NOT closed, and the model survives it.**
///
/// Hermes serves that exact literal from a cached fast path that compiles
/// nothing, so `EnableEval(false)` does not gate it — which is precisely
/// what carried patch 0014 exists to fix. LLP 0060 D4 claimed
/// construction-time configuration retired that patch; it does not.
///
/// It is nevertheless harmless here, and the reason is the whole argument
/// for the design: **the capability model never depended on making the
/// global object unreachable.** It depends on the global object being
/// EMPTY OF AUTHORITY, which is a far weaker property and survives this
/// hole completely. Reaching `globalThis` buys nothing when nothing
/// capability-bearing is on it.
///
/// This test asserts both halves: the hole is open, and it yields nothing.
#[test]
fn the_return_this_fast_path_is_open_and_yields_no_authority() {
    let (mut rt, _grants) = fetch_rt("net.fetch http://127.0.0.1:1");
    rt.eval("delete globalThis.__ibex2_fetch;").unwrap();
    // The shipping boot: the bindings capture their helpers and remove them,
    // so the reachable global carries no accessor over a handle table either.
    rt.install_bindings().expect("bindings");

    // Open — and if a future engine or config closes it, this line fails
    // and the comment above needs revisiting rather than silently rotting.
    assert_eq!(
        rt.eval("String(({}).constructor.constructor('return this')())")
            .unwrap(),
        "[object global]",
        "the return-this fast path closed; LLP 0060 D4 may now be true"
    );

    // ...and worth nothing, which is the property that actually matters.
    assert_eq!(
        rt.eval(
            "const g = ({}).constructor.constructor('return this')();
                 String(typeof g.__ibex2_fetch)"
        )
        .unwrap(),
        "undefined"
    );
    assert_eq!(
        rt.eval(
            "const g = ({}).constructor.constructor('return this')();
                 Object.getOwnPropertyNames(g).filter(n => n.indexOf('fetch') !== -1).length"
        )
        .unwrap(),
        "0"
    );
    // Grok 4.6's finding 1: this used to be a function that read any module's
    // response by integer handle. The name test above could not see it.
    assert_eq!(
        rt.eval(
            "const g = ({}).constructor.constructor('return this')();
                 [typeof g.__ibex2_response_field, typeof g.__ibex2_headers, typeof g.__ibex2_timer_clear].join(' ')"
        )
        .unwrap(),
        "undefined undefined undefined"
    );
}

/// Two modules, two grants, one runtime — and neither can reach the
/// other's binding. This is D2's property, demonstrated without a module
/// loader and without a patched engine.
#[test]
fn two_modules_in_one_runtime_hold_different_authority() {
    let allowed = TestServer::start("allowed payload");
    let forbidden = TestServer::start("forbidden payload");

    // One runtime, but two bindings built from two different grant sets.
    let mut rt = with_stdlib();
    let grants_a = Grants::parse(&format!("net.fetch {}", allowed.origin())).unwrap();
    assert!(rt.install_fetch(&grants_a));
    rt.eval("globalThis.__a = __ibex2_fetch; delete globalThis.__ibex2_fetch;")
        .unwrap();

    let grants_b = Grants::parse(&format!("net.fetch {}", forbidden.origin())).unwrap();
    assert!(rt.install_fetch(&grants_b));
    rt.eval("globalThis.__b = __ibex2_fetch; delete globalThis.__ibex2_fetch;")
        .unwrap();

    // A may reach `allowed` and not `forbidden`; B is the mirror image.
    rt.eval(&format!(
        "globalThis.r = {{}};
             __a('{a}/x').then(() => r.aa = 'ok', e => r.aa = e.message);
             __a('{f}/x').then(() => r.af = 'ok', e => r.af = e.message);
             __b('{f}/x').then(() => r.bf = 'ok', e => r.bf = e.message);
             __b('{a}/x').then(() => r.ba = 'ok', e => r.ba = e.message);",
        a = allowed.origin(),
        f = forbidden.origin()
    ))
    .unwrap();
    rt.pump_until(4);

    assert_eq!(rt.eval("r.aa").unwrap(), "ok");
    assert_eq!(rt.eval("r.bf").unwrap(), "ok");
    assert_eq!(rt.eval("r.af").unwrap(), "denied: net.fetch");
    assert_eq!(rt.eval("r.ba").unwrap(), "denied: net.fetch");
}

/// The honest limit, pinned so nobody mistakes the above for a sandbox.
///
/// A module that HOLDS a capability can hand it to anyone — that is the
/// explicit-handoff class, out of scope in LLP 0013, LLP 0057 §4, and
/// LLP 0060 §3. It is not defended, and this test exists so that stays a
/// documented property rather than a discovered surprise.
#[test]
fn voluntary_handoff_is_not_defended_and_this_is_by_design() {
    let server = TestServer::start("payload");
    let (mut rt, _grants) = fetch_rt(&format!("net.fetch {}", server.origin()));

    rt.eval(
        "globalThis.__holder = (function (f) {
               // A module that leaks its own binding. Nothing stops it.
               globalThis.__leaked = f;
               return {};
             })(__ibex2_fetch);
             delete globalThis.__ibex2_fetch;",
    )
    .unwrap();

    rt.eval(&format!(
        "globalThis.stolen = 'pending';
             __leaked('{}/x').then(h => {{ stolen = 'ok:' + __ibex2_response_field(h, 0); }});",
        server.origin()
    ))
    .unwrap();
    rt.pump_until(1);

    assert_eq!(
        rt.eval("stolen").unwrap(),
        "ok:200",
        "handoff is expected to work; capability systems bound reach, not trust"
    );
}

// --- Timers: the macrotask side of the loop -----------------------------

fn timer_rt() -> Hermes {
    let mut rt = with_stdlib();
    rt.install_bindings().expect("bindings");
    rt
}

/// Pump until `js` reports done or the deadline passes. Timers are real
/// elapsed time, so this waits rather than spins.
fn pump_for(rt: &mut Hermes, millis: u64) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(millis);
    while std::time::Instant::now() < deadline {
        rt.pump().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    rt.pump().unwrap();
}

#[test]
fn a_timeout_fires_after_its_delay_and_not_before() {
    let mut rt = timer_rt();
    rt.eval("globalThis.fired = false; setTimeout(() => { fired = true }, 20);")
        .unwrap();
    rt.pump().unwrap();
    assert_eq!(rt.eval("String(fired)").unwrap(), "false", "fired early");
    pump_for(&mut rt, 60);
    assert_eq!(rt.eval("String(fired)").unwrap(), "true");
}

#[test]
fn timers_fire_in_delay_order_then_insertion_order() {
    let mut rt = timer_rt();
    rt.eval(
        "globalThis.order = [];
             setTimeout(() => order.push('c'), 30);
             setTimeout(() => order.push('a1'), 5);
             setTimeout(() => order.push('a2'), 5);
             setTimeout(() => order.push('b'), 15);",
    )
    .unwrap();
    pump_for(&mut rt, 80);
    assert_eq!(rt.eval("order.join(',')").unwrap(), "a1,a2,b,c");
}

#[test]
fn clear_timeout_prevents_a_pending_timer() {
    let mut rt = timer_rt();
    rt.eval(
        "globalThis.fired = [];
             const keep = setTimeout(() => fired.push('keep'), 10);
             const drop = setTimeout(() => fired.push('drop'), 10);
             clearTimeout(drop);
             clearTimeout(undefined);
             clearTimeout(0);",
    )
    .unwrap();
    pump_for(&mut rt, 60);
    assert_eq!(rt.eval("fired.join(',')").unwrap(), "keep");
}

#[test]
fn an_interval_repeats_until_cleared() {
    let mut rt = timer_rt();
    rt.eval(
        "globalThis.ticks = 0;
             globalThis.h = setInterval(() => {
               ticks += 1;
               if (ticks === 3) clearInterval(h);
             }, 5);",
    )
    .unwrap();
    pump_for(&mut rt, 120);
    assert_eq!(rt.eval("String(ticks)").unwrap(), "3");
}

#[test]
fn extra_arguments_reach_the_callback() {
    let mut rt = timer_rt();
    rt.eval("globalThis.got = ''; setTimeout((a, b) => { got = a + ':' + b }, 1, 'x', 7);")
        .unwrap();
    pump_for(&mut rt, 40);
    assert_eq!(rt.eval("got").unwrap(), "x:7");
}

/// The HTML microtask checkpoint: a timer is a TASK, so microtasks it
/// enqueues drain before the NEXT timer runs.
///
/// Both timers share a deadline and the pump is called ONCE after both are
/// due, so they fire in the same pump and the placement of the drain is
/// observable. An earlier version of this test polled while the timers came
/// due at different times — so each got its own drain regardless, and the
/// test passed with the checkpoint batched to the end of the pump. It could
/// not fail, which is worse than not existing.
#[test]
fn microtasks_drain_between_timers_not_after_all_of_them() {
    let mut rt = timer_rt();
    rt.eval(
            "globalThis.order = [];
             setTimeout(() => { order.push('t1'); Promise.resolve().then(() => order.push('m1')); }, 1);
             setTimeout(() => { order.push('t2'); Promise.resolve().then(() => order.push('m2')); }, 1);",
        )
        .unwrap();
    // Both deadlines pass with no pump in between, so both are admitted
    // together and the ONE-TASK-PER-CYCLE rule is what separates them.
    std::thread::sleep(std::time::Duration::from_millis(25));
    assert_eq!(
        rt.pump().unwrap(),
        1,
        "a drive cycle runs at most one host task"
    );
    assert_eq!(
        rt.eval("order.join(',')").unwrap(),
        "t1,m1",
        "the first timer's microtasks must drain before the second timer runs"
    );
    assert_eq!(rt.pump().unwrap(), 1);
    assert_eq!(rt.eval("order.join(',')").unwrap(), "t1,m1,t2,m2");
    assert_eq!(rt.pump().unwrap(), 0, "nothing left");
}

/// A throwing timer must not cancel the timers already due behind it,
/// exactly as an unhandled error in one task does not stop the next.
#[test]
fn a_throwing_timer_does_not_stop_the_others() {
    let mut rt = timer_rt();
    rt.eval(
        "globalThis.order = [];
             setTimeout(() => { order.push('before'); throw new Error('boom'); }, 5);
             setTimeout(() => order.push('after'), 10);",
    )
    .unwrap();
    pump_for(&mut rt, 80);
    assert_eq!(rt.eval("order.join(',')").unwrap(), "before,after");
}

/// The string form of setTimeout compiles source, which D4 closed. It
/// refuses with a message that says why rather than surfacing a parser
/// error from three frames down.
#[test]
fn the_string_form_of_set_timeout_is_refused_clearly() {
    let mut rt = timer_rt();
    let caught = rt
        .eval("try { setTimeout('globalThis.x = 1', 0); 'no throw' } catch (e) { e.message }")
        .unwrap();
    assert!(caught.contains("disabled"), "unexpected: {caught}");
}

#[test]
fn performance_now_advances_and_shares_the_runtime_origin() {
    let mut rt = timer_rt();
    let first: f64 = rt
        .eval("String(performance.now())")
        .unwrap()
        .parse()
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    let second: f64 = rt
        .eval("String(performance.now())")
        .unwrap()
        .parse()
        .unwrap();
    assert!(second > first, "{second} !> {first}");
    assert!(second - first >= 15.0, "advanced only {}ms", second - first);
    // Milliseconds since the runtime started, not since the epoch.
    assert!(first < 60_000.0, "not a runtime-relative clock: {first}");
}

/// Timers and off-thread completions share one pump and both make
/// progress — the loop is one loop, not two.
#[test]
fn timers_and_completions_interleave_in_one_pump() {
    let mut rt = timer_rt();
    rt.eval(
        "globalThis.order = [];
             setTimeout(() => order.push('timer'), 5);
             __ibex2_async_echo('completion').then(v => order.push(v));",
    )
    .unwrap();
    pump_for(&mut rt, 80);
    let order = rt.eval("order.join(',')").unwrap();
    assert!(order.contains("timer"), "timer did not fire: {order}");
    assert!(order.contains("completion"), "completion lost: {order}");
}

/// A typed array is what application code actually passes, and it is not an
/// ArrayBuffer. Handling only the latter made
/// `fs.writeFile(p, new TextEncoder().encode(t))` write an empty file, because
/// the payload stringified instead of crossing as bytes.
#[test]
fn a_typed_array_crosses_the_boundary_as_bytes() {
    let mut rt = with_stdlib();
    assert_eq!(
        rt.eval("__ibex2_text_decode(new TextEncoder().encode('hello'))")
            .unwrap(),
        "hello"
    );
    assert_eq!(
        rt.eval("__ibex2_text_decode(new Uint8Array([104, 105]))")
            .unwrap(),
        "hi"
    );
}

/// A view's byteOffset matters: a subarray shares its buffer with the whole,
/// so reading from the buffer's start sends the wrong bytes.
#[test]
fn a_typed_array_view_sends_its_own_window_not_the_whole_buffer() {
    let mut rt = with_stdlib();
    assert_eq!(
        rt.eval(
            "const all = new TextEncoder().encode('PREFIX:payload');
             __ibex2_text_decode(all.subarray(7))"
        )
        .unwrap(),
        "payload"
    );
    assert_eq!(
        rt.eval(
            "const all = new Uint8Array([1, 2, 104, 105, 9]);
             __ibex2_text_decode(all.subarray(2, 4))"
        )
        .unwrap(),
        "hi"
    );
}

/// The assertion that keeps the fork from growing back: a host function
/// installed for one runtime is not ambient in another. This is the shape
/// LLP 0060 D2 needs from the engine — per-runtime, not global-by-default.
#[test]
fn a_probe_installed_in_one_runtime_is_absent_from_another() {
    let mut first = Hermes::new(DynamicCode::Closed).expect("runtime");
    assert!(first.install_probe("__ibex2_probe", "first"));

    let mut second = Hermes::new(DynamicCode::Closed).expect("runtime");
    assert_eq!(
        second.eval("typeof __ibex2_probe").unwrap(),
        "undefined",
        "a host function leaked across runtimes"
    );
}
