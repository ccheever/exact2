//! A long poll on work that is no HTTP request, through the renderer (LLP 1016.000 D8): refused before its
//! work runs, a refusal of what was asked (its placeholder kept), not a busy render.
use super::SITE;
use exact_plan::Value;
use exact_render::{render, Settled};
use exact_runner::{Answer, DataError, DataSource, Outcome, Request, Store};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static RAN: AtomicUsize = AtomicUsize::new(0);

struct PollsOtherWork;
impl DataSource for PollsOtherWork {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
    fn answer(&mut self, _: &mut Store, _: &str, _: &[Value]) -> Result<Answer, DataError> {
        Ok(Answer::Later(
            Request::continuation(1).long_poll("since", "seq"),
        ))
    }
    fn continuation(&mut self, _: u64) -> Option<Box<dyn FnOnce() -> Outcome + Send>> {
        Some(Box::new(|| {
            RAN.fetch_add(1, Ordering::SeqCst);
            Outcome::Storage(vec![])
        }))
    }
}

#[test]
fn a_long_poll_on_other_work_is_refused_before_its_work_runs() {
    let plan = contract::compile(
        "component App\n  resource polled = polled() as shape number\n  view\n    text \"x\"\n",
    )
    .unwrap();
    let rendered = render(
        &plan,
        || PollsOtherWork,
        Default::default(),
        "/",
        &SITE,
        Duration::from_secs(2),
    )
    .unwrap();
    assert_eq!(RAN.load(Ordering::SeqCst), 0, "its work never ran");
    assert_ne!(
        rendered.settled,
        Settled::Busy,
        "a refusal, not a busy render"
    );
}
