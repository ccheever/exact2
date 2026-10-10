//! A re-ask (`Dispatch::Again`) on the web host settles at once, on the
//! immediate-outcome path the no-op it replaces took (LLP 1041 §8.4,
//! amended 2026-10-09): the browser's admission is the browser's.
use exact_runner::{Answer, DataError, DataSource, Dispatch, Outcome, Request, Value};
use exact_web::Host;

#[derive(Default)]
struct Waiting {
    asked: usize,
}
impl DataSource for Waiting {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        unreachable!()
    }
    fn answer(
        &mut self,
        _: &mut exact_runner::Store,
        _: &str,
        _: &[Value],
    ) -> Result<Answer, DataError> {
        Ok(Answer::Later(Request::continuation(1)))
    }
    fn dispatch(&mut self, _: u64, _: &exact_runner::Store) -> Dispatch {
        Dispatch::Again
    }
    fn parse(
        &mut self,
        _: &mut exact_runner::Store,
        _: &str,
        _: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        assert_eq!(outcome, Dispatch::again_outcome());
        self.asked += 1;
        // Waits once more, then settles.
        Ok(if self.asked < 2 {
            Answer::Later(Request::continuation(2))
        } else {
            Answer::Now(Value::Number(7.))
        })
    }
}

#[test]
fn a_re_ask_settles_at_once_on_the_web() {
    let plan = contract::compile(
        "component App\n  resource r = wait() as shape number\n  view\n    text \"x\"\n",
    )
    .unwrap()
    .encode();
    let (mut host, batch) =
        Host::boot_with(&plan, Waiting::default(), None, Default::default(), "/").unwrap();
    assert!(!batch.contains("\"op\":\"continue\""), "{batch}");
    assert_eq!(host.runner().resource("r"), Some(&Value::Number(7.)));
    assert!(!host.runner().has_pending());
    assert_eq!(host.runner_mut().data().asked, 2);
}
