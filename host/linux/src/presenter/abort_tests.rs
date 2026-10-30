//! An app's abort whose answer the runner no longer owes (a newer send
//! replaced it) still cancels its request: the pump that finds no outcome
//! hands the cancel to the executor, and the server sees the connection end.
use super::*;
use exact_runner::{Answer, DataError, Store, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::time::Duration;

struct Held {
    grants: String,
    url: String,
    aborted: Arc<AtomicBool>,
}
impl DataSource for Held {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
    fn grants(&self) -> &str {
        &self.grants
    }
    fn answer(&mut self, _: &mut Store, name: &str, _: &[Value]) -> Result<Answer, DataError> {
        if name != "hold" {
            return Ok(Answer::Now(Value::Bool(true)));
        }
        let mut request = exact_runner::Request::post_json(&self.url, "{}");
        request.fetch = Some(1);
        Ok(Answer::Later(request))
    }
    /// The app aborts its fetch 1, as a module's door reports it, once.
    fn take_aborted(&mut self) -> Vec<u64> {
        match self.aborted.swap(false, Ordering::SeqCst) {
            true => vec![1],
            false => Vec::new(),
        }
    }
}

const APP: &str = r#"component App
  mutation m as shape bool
  mutation s as shape bool
  action hold
    send m = hold()
  action halt
    send s = halt()
  action replace
    send m = now()
  view
    column width=200 height=200
      button press=hold testId="hold" height=20
        text "Hold"
      button press=halt testId="halt" height=20
        text "Abort"
      button press=replace testId="replace" height=20
        text "Replace"
"#;

fn id(p: &Presenter<Held>, test_id: &str) -> ViewId {
    let k = p.host().kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

#[test]
fn an_abort_whose_answer_was_replaced_is_still_cancelled_on_an_empty_pump() {
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
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    break Err("still open after 10 s")
                }
                Err(_) => break Ok(()), // reset: closed too
            }
        };
        let _ = ended.send(end);
    });
    let aborted = Arc::new(AtomicBool::new(false));
    let source = Held {
        grants: format!("net.fetch http://127.0.0.1:{port}\n"),
        url: format!("http://127.0.0.1:{port}/held"),
        aborted: aborted.clone(),
    };
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(APP),
        source,
        (200., 200.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.tap(id(&p, "hold")).unwrap();
    let head = head
        .recv_timeout(Duration::from_secs(10))
        .expect("the write reached the server");
    assert!(head.starts_with("POST /held"), "{head}");
    // The abort is heard in another answer's turn, then a newer send of the
    // same mutation replaces the held one before any pump: no answer is owed.
    aborted.store(true, Ordering::SeqCst);
    p.tap(id(&p, "halt")).unwrap();
    p.tap(id(&p, "replace")).unwrap();
    let cancelled = std::time::Instant::now();
    // Both halves of the pump's alternation, each finding no outcome.
    p.pump(0.);
    p.pump(0.);
    let end = closed
        .recv_timeout(Duration::from_secs(10))
        .expect("the server's read ended");
    assert_eq!(end, Ok(()), "the server saw the connection close");
    assert!(
        cancelled.elapsed() < Duration::from_secs(5),
        "closed promptly"
    );
}
