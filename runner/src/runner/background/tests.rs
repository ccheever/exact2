//! The background ticket, the runner half (LLP 1097 D5): a scripted module
//! whose background work is a count of rounds, no engine.
use super::super::*;
use crate::{Outcome, Request, Response, Store};
use exact_kernel::Kernel;

/// `save(v)` answers `v` now and leaves `rounds` operations to the
/// background; `bad` refuses. Each round lands one operation.
#[derive(Default)]
struct Saving {
    /// Background operations still to land.
    left: u64,
    /// A round is out.
    out: bool,
    /// The next round fails in the executor.
    fail: bool,
    logs: Vec<String>,
    landed: u64,
}

impl DataSource for Saving {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        unreachable!("the fixture implements answer")
    }

    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        match (source, args.first()) {
            ("save", Some(Value::Number(n))) => {
                self.left += *n as u64;
                self.logs.push(format!("console: saving {n}"));
                Ok(Answer::Now(Value::Record(vec![Value::Number(*n)].into())))
            }
            _ => Err(DataError::Unavailable("refused".into())),
        }
    }

    fn background(&mut self, _: &Store) -> Option<Request> {
        if self.out || self.left == 0 {
            return None;
        }
        self.out = true;
        Some(Request::continuation(BACKGROUND))
    }

    fn background_landed(
        &mut self,
        _: &Store,
        outcome: Outcome,
    ) -> Result<Option<Request>, DataError> {
        self.out = false;
        if std::mem::take(&mut self.fail) {
            return Err(DataError::Unavailable("the store went away".into()));
        }
        assert!(matches!(outcome, Outcome::Response(_)));
        self.left -= 1;
        self.landed += 1;
        Ok(self.background(&Store::new("", Vec::<(String, String)>::new())))
    }

    fn background_state(&self) -> Option<BackgroundState> {
        Some(BackgroundState {
            queued: self.left.saturating_sub(1),
            in_flight: u64::from(self.left > 0),
            done: self.landed,
            ..Default::default()
        })
    }

    fn take_logs(&mut self) -> Vec<String> {
        std::mem::take(&mut self.logs)
    }
}

fn runner() -> Runner<Saving> {
    let plan = contract::compile(
        r#"
shape Saved
  n: number
component App
  mutation saved as shape Saved
  action save(n: number)
    send saved = save(n)
  action bad
    send saved = bad()
  view
    column
      text "saving"
"#,
    )
    .unwrap();
    Runner::boot(
        plan,
        Saving::default(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn done() -> Outcome {
    Outcome::Response(Response {
        status: 204,
        headers: vec![],
        body: vec![],
    })
}

/// The one round a host holds: its ticket.
fn round(r: &mut Runner<Saving>) -> Option<u64> {
    let mut out = r.take_requests();
    assert!(out.len() <= 1, "one round at a time: {out:?}");
    let req = out.pop()?;
    assert_eq!(req.target, "background");
    assert_eq!(req.request.continuation, Some(BACKGROUND));
    Some(req.ticket)
}

#[test]
fn a_round_is_in_flight_held_and_pending_and_its_rounds_never_forget() {
    let mut r = runner();
    assert!(round(&mut r).is_none(), "an idle module hands out none");
    r.act("save", vec![Value::Number(3.)]).unwrap();
    let first = round(&mut r).expect("the background's round");
    assert!(r.holds(first) && r.has_pending());
    assert_eq!(r.in_flight(), [("background".to_string(), first)]);
    let mut tickets = vec![first];
    let mut ticket = first;
    for _ in 0..3 {
        assert_eq!(r.fulfill(ticket, done()).unwrap(), None, "no commit");
        match round(&mut r) {
            Some(next) => {
                assert!(!r.holds(ticket), "a fresh ticket each round");
                tickets.push(next);
                ticket = next;
            }
            None => break,
        }
    }
    assert_eq!(tickets.len(), 3);
    assert!(!r.has_pending());
    let journal: Vec<&str> = r.journal().collect();
    assert!(
        journal.iter().all(|l| !l.contains("forgot")),
        "{journal:#?}"
    );
    assert!(journal
        .iter()
        .any(|l| l.ends_with("background: storage (2 waiting)")));
    assert!(journal
        .iter()
        .any(|l| l.ends_with("background: done (3 operations)")));
    assert!(journal.iter().any(|l| l.ends_with("console: saving 3")));
    assert_eq!(r.data().landed, 3);
}

#[test]
fn a_refused_commit_keeps_the_live_round_and_a_failure_is_journaled_alone() {
    let mut r = runner();
    r.act("save", vec![Value::Number(2.)]).unwrap();
    let ticket = round(&mut r).unwrap();
    assert!(r.act("bad", vec![]).is_err(), "the commit is refused");
    assert!(r.holds(ticket), "the restore kept the live round");
    let batch = r.batch;
    r.data().fail = true;
    assert_eq!(r.fulfill(ticket, done()).unwrap(), None);
    assert_eq!(r.batch, batch, "no commit, no update");
    assert!(r
        .journal()
        .any(|l| l.ends_with("background failed: the store went away")));
}

#[test]
fn poison_drops_the_round_with_its_line() {
    let mut r = runner();
    r.act("save", vec![Value::Number(4.)]).unwrap();
    let ticket = round(&mut r).unwrap();
    r.poison();
    assert!(!r.holds(ticket) && !r.has_pending());
    assert!(r
        .journal()
        .any(|l| l.ends_with("background: dropped (poisoned), 3 operations waiting")));
    assert!(round(&mut r).is_none(), "a poisoned runner hands out none");
}
