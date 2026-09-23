//! LLP 1048.000 D6: a rendered document's checkpoint answers are an input of
//! their own. The runtime takes them as answers it already has — not asked
//! again at boot or at `data_ready`, only when their arguments or inputs
//! change — and the device's store and clock apply as ordinary updates.

use exact_kernel::{Kernel, PropId};
use exact_runner::{
    Answer, Checkpoint, DataError, DataSource, Delivery, Event, Outcome, Request, Runner,
    RunnerError, Store, Value,
};
use std::{cell::Cell, cell::RefCell, rc::Rc};

const SRC: &str = r#"
shape Post
  id: string
  title: string

component Blog
  state id = "7"
  resource post = post(id) as shape Post else emptyPost()
  resource viewer = viewer() as shape string
  action next writes id
    id = "8"
  view
    column
      text post.title testId="title"
      text viewer testId="viewer"
      button press=next testId="next"
        text "Next"
"#;

/// Posts answer now (or later), the viewer reads the store; every ask is
/// recorded, and readiness and the answers' wording are the test's.
#[derive(Clone, Default)]
struct Source {
    asks: Rc<RefCell<Vec<String>>>,
    not_ready: Rc<Cell<bool>>,
    later: bool,
    wording: &'static str,
    revision: Option<&'static str>,
}

fn post(id: &str, title: &str) -> Value {
    Value::record(vec![Value::str(id), Value::str(title)])
}

impl DataSource for Source {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }

    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        self.asks.borrow_mut().push(source.to_string());
        let id = args.first().and_then(Value::as_str).unwrap_or("");
        Ok(match source {
            "post" if self.later => Answer::Later(Request::get("https://blog.test/post")),
            "post" => Answer::Now(post(id, &format!("{} {id}", self.wording))),
            "emptyPost" => Answer::Now(post("", "")),
            "viewer" => Answer::Now(Value::str(store.get("token").unwrap_or("anonymous"))),
            other => return Err(DataError::UnknownSource(other.into())),
        })
    }

    fn parse(
        &mut self,
        _: &mut Store,
        _: &str,
        args: &[Value],
        _: Outcome,
    ) -> Result<Answer, DataError> {
        let id = args.first().and_then(Value::as_str).unwrap_or("");
        Ok(Answer::Now(post(id, &format!("Fresh {id}"))))
    }

    fn grants(&self) -> &str {
        "secret.keep token\n"
    }

    fn ready(&self) -> bool {
        !self.not_ready.get()
    }

    fn revision(&self) -> Option<&str> {
        self.revision
    }
}

fn text_of<D: DataSource>(r: &Runner<D>, test_id: &str) -> String {
    let k = r.kernel();
    let key = k.find_by_test_id(test_id).into_iter().next().unwrap();
    k.node_by_key(key)
        .unwrap()
        .props
        .str(PropId::Text)
        .unwrap()
        .to_string()
}

fn view_of<D: DataSource>(r: &Runner<D>, test_id: &str) -> u32 {
    let k = r.kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

/// The checkpoint a render of `plan` at `/` writes, answered in `wording`.
fn rendered(plan: &exact_plan::Plan, wording: &'static str) -> Checkpoint {
    let source = Source {
        wording,
        ..Source::default()
    };
    let r = Runner::boot(
        plan.clone(),
        source,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.document_checkpoint("/")
}

fn boot(
    plan: &exact_plan::Plan,
    source: Source,
    checkpoint: &Checkpoint,
    store: &[(&str, &str)],
) -> Result<Runner<Source>, RunnerError> {
    Runner::boot_checkpoint(
        plan.clone(),
        source,
        Kernel::with_monospace(),
        checkpoint,
        store
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        Delivery::default(),
        Default::default(),
        "/",
    )
}

#[test]
fn the_render_records_what_the_document_read() {
    let plan = contract::compile(SRC).unwrap();
    let checkpoint = rendered(&plan, "Rendered");
    assert_eq!(checkpoint.location, "/");
    let names: Vec<&str> = checkpoint
        .answers
        .iter()
        .map(|(name, ..)| name.as_str())
        .collect();
    assert_eq!(names, ["post", "viewer", "post#else"]);
    assert_eq!(checkpoint.answers[0].2, vec![Value::str("7")]);
    assert!(checkpoint.pending.is_empty());

    // A source that hadn't answered is pending, not an answer.
    let later = Runner::boot(
        plan.clone(),
        Source {
            later: true,
            ..Source::default()
        },
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
    .document_checkpoint("/");
    assert_eq!(later.pending, ["post"]);
    assert!(later.answers.iter().all(|(name, ..)| name != "post"));
}

#[test]
fn a_checkpoint_answer_is_not_asked_again_at_boot_or_at_data_ready() {
    let plan = contract::compile(SRC).unwrap();
    let checkpoint = rendered(&plan, "Rendered");
    let source = Source {
        wording: "Asked",
        ..Source::default()
    };
    source.not_ready.set(true);
    let mut r = boot(&plan, source.clone(), &checkpoint, &[]).unwrap();
    assert!(
        source.asks.borrow().is_empty(),
        "{:?}",
        source.asks.borrow()
    );
    assert_eq!(text_of(&r, "title"), "Rendered 7");
    assert!(r
        .journal()
        .any(|l| l.contains("checkpoint: 3 of 3 answers taken")));
    assert!(r.take_requests().is_empty());
    source.not_ready.set(false);
    assert_eq!(r.data_ready().unwrap(), None, "nothing was waiting");
    assert!(source.asks.borrow().is_empty());

    // Its arguments change: asked, as any answer is.
    r.dispatch(view_of(&r, "next"), Event::Press).unwrap();
    assert_eq!(*source.asks.borrow(), ["post"]);
    assert_eq!(text_of(&r, "title"), "Asked 8");
}

#[test]
fn other_logic_or_an_answer_that_no_longer_fits_is_asked_again() {
    let plan = contract::compile(SRC).unwrap();
    let mut checkpoint = rendered(&plan, "Rendered");
    let source = Source {
        wording: "Asked",
        revision: Some("module-b"),
        ..Source::default()
    };
    checkpoint.logic = Some("module-a".into());
    let r = boot(&plan, source.clone(), &checkpoint, &[]).unwrap();
    assert_eq!(text_of(&r, "title"), "Asked 7");
    assert!(source.asks.borrow().contains(&"post".to_string()));
    assert!(r.journal().any(|l| l.contains("other logic answered it")));

    let mut checkpoint = rendered(&plan, "Rendered");
    checkpoint.answers[0].3 = Value::str("not a post");
    let source = Source {
        wording: "Asked",
        ..Source::default()
    };
    let r = boot(&plan, source.clone(), &checkpoint, &[]).unwrap();
    assert_eq!(text_of(&r, "title"), "Asked 7");
    assert_eq!(*source.asks.borrow(), ["post"]);
}

#[test]
fn the_devices_store_applies_to_what_reads_it() {
    // The bake marks `viewer` as reading the store.
    let plan = contract::bake(
        contract::compile(SRC).unwrap(),
        Source {
            wording: "Built",
            ..Source::default()
        },
    )
    .unwrap();
    let checkpoint = rendered(&plan, "Rendered");
    // An empty store is the render's own input: nothing is asked.
    let source = Source::default();
    let r = boot(&plan, source.clone(), &checkpoint, &[]).unwrap();
    assert_eq!(text_of(&r, "viewer"), "anonymous");
    assert!(source.asks.borrow().is_empty());
    // A signed-in device asks the reader again, at boot when it can.
    let source = Source::default();
    let r = boot(&plan, source.clone(), &checkpoint, &[("token", "ada")]).unwrap();
    assert_eq!(text_of(&r, "viewer"), "ada");
    assert_eq!(*source.asks.borrow(), ["viewer"]);
    // Or once the module loads, showing the rendered answer until then.
    let source = Source::default();
    source.not_ready.set(true);
    let mut r = boot(&plan, source.clone(), &checkpoint, &[("token", "ada")]).unwrap();
    assert_eq!(text_of(&r, "viewer"), "anonymous");
    assert_eq!(text_of(&r, "title"), "Built 7");
    source.not_ready.set(false);
    r.data_ready().unwrap();
    assert_eq!(text_of(&r, "viewer"), "ada");
    assert_eq!(*source.asks.borrow(), ["viewer"]);
}

#[test]
fn the_clock_starts_at_the_render_time() {
    let plan = contract::compile(SRC).unwrap();
    let mut checkpoint = rendered(&plan, "Rendered");
    checkpoint.now_ms = 5_000.0;
    let r = boot(&plan, Source::default(), &checkpoint, &[]).unwrap();
    assert_eq!(r.now_ms(), 5_000.0);
    checkpoint.now_ms = f64::NAN;
    assert!(matches!(
        boot(&plan, Source::default(), &checkpoint, &[]),
        Err(RunnerError::NonFiniteClock)
    ));
}
