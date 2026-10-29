//! A store reader that failed is asked again when a reply's turn writes the
//! store, as a browser's continuation writes it outside every commit (LLP
//! 1146 §2.3). A write an action makes in its commit leaves the failure
//! standing, on every host.
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{Answer, DataError, DataSource, Outcome, Request, Response, Runner, Store};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

const SOURCE: &str = r##"shape Row
  n: number
shape Done
  ok: bool
component App
  state pokes = 0
  resource reader = reader() as shape Row
  mutation forgot as shape Done
  mutation wrote as shape Done
  mutation fetched as shape Done
  mutation posted as shape Done
  action forget
    send forgot = forget()
  action write
    send wrote = write()
  action fetch
    send fetched = fetch()
  action post
    send posted = post()
  action poke
    pokes = pokes + 1
  view
    text `${reader.n} ${pokes}`
"##;

/// `reader` asks with a GET and answers what the store keeps under `k` when
/// the reply lands, failing without it; `forget` and `write` change `k` in
/// their commit, and `fetch` and `post` in their reply's turn, `post`'s reply
/// answering outside its shape so that its commit is refused.
struct Source {
    asks: Arc<AtomicUsize>,
}

impl DataSource for Source {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }

    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        _: &[Value],
    ) -> Result<Answer, DataError> {
        let unavailable = |e| DataError::Unavailable(format!("{e:?}"));
        let done = || Value::record(vec![Value::Bool(true)]);
        match source {
            "reader" => {
                self.asks.fetch_add(1, Ordering::SeqCst);
                Ok(Answer::Later(Request::get("https://example.test/reader")))
            }
            "forget" => store
                .forget("k")
                .map(|_| Answer::Now(done()))
                .map_err(unavailable),
            "write" => store
                .set("k", "2")
                .map(|_| Answer::Now(done()))
                .map_err(unavailable),
            "fetch" => Ok(Answer::Later(Request::get("https://example.test/fetch"))),
            "post" => Ok(Answer::Later(Request::post_json(
                "https://example.test/post",
                "{}",
            ))),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }

    fn parse(
        &mut self,
        store: &mut Store,
        source: &str,
        _: &[Value],
        _: Outcome,
    ) -> Result<Answer, DataError> {
        let unavailable = |e| DataError::Unavailable(format!("{e:?}"));
        if source == "fetch" {
            store.set("k", "3").map_err(unavailable)?;
            return Ok(Answer::Now(Value::record(vec![Value::Bool(true)])));
        }
        if source == "post" {
            store.set("k", "4").map_err(unavailable)?;
            return Ok(Answer::Now(Value::str("outside its shape")));
        }
        let n = (store.get("k")).ok_or_else(|| DataError::Unavailable("nothing kept".into()))?;
        Ok(Answer::Now(Value::record(vec![Value::Number(
            n.parse().unwrap(),
        )])))
    }

    fn grants(&self) -> &str {
        "secret.keep k"
    }
}

/// Answers each request out, and says how many there were.
fn reply(r: &mut Runner<Source>) -> usize {
    let tickets: Vec<u64> = r.take_requests().iter().map(|o| o.ticket).collect();
    for &ticket in &tickets {
        let body = b"{}".to_vec();
        let reply = Outcome::Response(Response {
            status: 200,
            headers: Vec::new(),
            body,
        });
        let _ = r.fulfill(ticket, reply);
    }
    tickets.len()
}

#[test]
fn a_failed_store_reader_is_asked_again_when_a_reply_writes_the_store() {
    let asks = Arc::new(AtomicUsize::new(0));
    let source = Source { asks: asks.clone() };
    let plan = contract::compile(SOURCE).unwrap();
    let snapshot = vec![("k".to_string(), b"1".to_vec())];
    let kernel = Kernel::with_monospace();
    let mut r =
        Runner::boot_stored(plan, source, kernel, snapshot, Default::default(), "/").unwrap();
    reply(&mut r);
    assert_eq!(r.resource("reader"), Some(&row(1.0)));
    r.act("forget", vec![]).unwrap();
    assert_eq!(reply(&mut r), 1, "a reader that stood is asked again");
    assert_eq!(r.failed_resources().len(), 1, "it fails without `k`");
    let failed = asks.load(Ordering::SeqCst);
    r.act("poke", vec![]).unwrap();
    r.act("write", vec![]).unwrap();
    assert_eq!(
        reply(&mut r),
        0,
        "an action's write leaves the failure standing"
    );
    assert_eq!(asks.load(Ordering::SeqCst), failed);
    r.act("fetch", vec![]).unwrap();
    assert_eq!(reply(&mut r), 1, "`fetch`'s GET");
    assert_eq!(reply(&mut r), 1, "its reply's write asks the reader again");
    assert!(
        r.failed_resources().is_empty(),
        "{:?}",
        r.failed_resources()
    );
    assert_eq!(r.resource("reader"), Some(&row(3.0)));
}

fn row(n: f64) -> Value {
    Value::record(vec![Value::Number(n)])
}

/// A reply with effects whose commit is refused keeps what its turn wrote
/// (LLP 1146 D2), and that write asks a failed store reader again too.
#[test]
fn a_refused_reply_s_kept_write_asks_a_failed_store_reader_again() {
    let source = Source {
        asks: Arc::new(AtomicUsize::new(0)),
    };
    let plan = contract::compile(SOURCE).unwrap();
    let snapshot = vec![("k".to_string(), b"1".to_vec())];
    let kernel = Kernel::with_monospace();
    let mut r =
        Runner::boot_stored(plan, source, kernel, snapshot, Default::default(), "/").unwrap();
    reply(&mut r);
    r.act("forget", vec![]).unwrap();
    reply(&mut r);
    assert_eq!(r.failed_resources().len(), 1, "it fails without `k`");
    r.act("post", vec![]).unwrap();
    assert_eq!(reply(&mut r), 1, "`post`'s POST");
    assert_eq!(reply(&mut r), 1, "its kept write asks the reader again");
    assert_eq!(r.resource("reader"), Some(&row(4.0)));
}

/// `picky` answers its first ask without reading the store; asked again when
/// `ended`'s write ends, a reconciling ask, it reads `k` and fails without it.
/// `fetched`'s reply writes `k` in its turn.
const RECONCILING: &str = r##"shape Row
  n: number
shape Done
  ok: bool
component App
  resource picky = picky() as shape Row
  mutation ended as shape Done refreshes picky
  mutation fetched as shape Done
  action end
    send ended = end()
  action fetch
    send fetched = fetch()
  view
    text `${picky.n}`
"##;

struct Picky {
    asks: usize,
}

impl DataSource for Picky {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }

    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        _: &[Value],
    ) -> Result<Answer, DataError> {
        match source {
            "picky" => {
                self.asks += 1;
                if self.asks == 1 {
                    return Ok(Answer::Now(row(0.0)));
                }
                let n = (store.get("k"))
                    .ok_or_else(|| DataError::Unavailable("nothing kept".into()))?;
                Ok(Answer::Now(row(n.parse().unwrap())))
            }
            "end" => Ok(Answer::Later(Request::get("https://example.test/end"))),
            "fetch" => Ok(Answer::Later(Request::get("https://example.test/fetch"))),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }

    fn parse(
        &mut self,
        store: &mut Store,
        source: &str,
        _: &[Value],
        _: Outcome,
    ) -> Result<Answer, DataError> {
        if source == "fetch" {
            store
                .set("k", "5")
                .map_err(|e| DataError::Unavailable(format!("{e:?}")))?;
        }
        Ok(Answer::Now(Value::record(vec![Value::Bool(true)])))
    }

    fn grants(&self) -> &str {
        "secret.keep k"
    }
}

/// A reconciling ask that reads the store and fails at once leaves a failed
/// store reader, which a reply's write asks again (LLP 1146 §2.3).
#[test]
fn a_reconciling_ask_that_read_the_store_and_failed_is_asked_again() {
    let plan = contract::compile(RECONCILING).unwrap();
    let mut r = Runner::boot(
        plan,
        Picky { asks: 0 },
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(r.resource("picky"), Some(&row(0.0)));
    r.act("end", vec![]).unwrap();
    assert_eq!(reply_picky(&mut r), 1, "`end`'s request");
    assert_eq!(r.failed_resources().len(), 1, "its reconciling ask failed");
    r.act("fetch", vec![]).unwrap();
    assert_eq!(reply_picky(&mut r), 1, "`fetch`'s request");
    assert_eq!(r.resource("picky"), Some(&row(5.0)));
}

/// Answers each request out, and says how many there were.
fn reply_picky(r: &mut Runner<Picky>) -> usize {
    let tickets: Vec<u64> = r.take_requests().iter().map(|o| o.ticket).collect();
    for &ticket in &tickets {
        let reply = Outcome::Response(Response {
            status: 200,
            headers: Vec::new(),
            body: b"{}".to_vec(),
        });
        let _ = r.fulfill(ticket, reply);
    }
    tickets.len()
}
