//! Store, readiness, rollback, and reload behavior across component scopes.

use exact_kernel::{Kernel, PropValue};
use exact_plan::Value;
use exact_runner::{
    Answer, DataError, DataSource, Event, Outcome, Request, Response, Runner, Store,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

fn text<D: DataSource>(runner: &Runner<D>, test_id: &str) -> String {
    let key = runner.kernel().find_by_test_id(test_id)[0];
    runner
        .kernel()
        .node_by_key(key)
        .unwrap()
        .props
        .iter()
        .find_map(|(prop, value)| match value {
            PropValue::Str(value) if prop.name() == "text" => Some(value.clone()),
            _ => None,
        })
        .unwrap()
}

fn press<D: DataSource>(runner: &mut Runner<D>, test_id: &str) {
    let key = runner.kernel().find_by_test_id(test_id)[0];
    let view = runner.kernel().node_by_key(key).unwrap().id;
    runner.dispatch(view, Event::Press).unwrap();
}

struct FixpointSource;

impl DataSource for FixpointSource {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }

    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        match source {
            "read" => Ok(Answer::Now(Value::str(&format!(
                "{}:{}",
                args[0].as_str().unwrap(),
                store.get("token").unwrap_or("missing")
            )))),
            "write" => {
                store.set("token", "new")?;
                Ok(Answer::Now(Value::str("written")))
            }
            other => Err(DataError::UnknownSource(other.into())),
        }
    }

    fn grants(&self) -> &'static str {
        "secret.keep token\n"
    }
}

#[test]
fn later_child_store_writer_reanswers_root_and_earlier_scoped_reader_same_commit() {
    let source = r#"
component App
  resource rootSeen = read("root") as shape string
  view
    column
      text rootSeen testId="root-seen"
      Reader()
      Writer()
component Reader
  resource childSeen = read("child") as shape string
  view
    text childSeen testId="child-seen"
component Writer
  resource written = write() as shape string
  view
    text written testId="writer"
"#;
    let plan = contract::compile(source).unwrap();
    let mut runner = Runner::boot_stored(
        plan,
        FixpointSource,
        Kernel::with_monospace(),
        vec![("token".into(), "old".into())],
    )
    .unwrap();

    assert_eq!(text(&runner, "root-seen"), "root:new");
    assert_eq!(text(&runner, "child-seen"), "child:new");
    assert_eq!(text(&runner, "writer"), "written");
    let writes = runner.take_store_writes();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].value.as_deref(), Some("new"));
}

#[derive(Clone)]
struct ReadinessSource {
    ready: Rc<Cell<bool>>,
    calls: Rc<RefCell<Vec<String>>>,
}

impl DataSource for ReadinessSource {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }

    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        if source != "profile" {
            return Err(DataError::UnknownSource(source.into()));
        }
        let id = args[0].as_str().unwrap();
        let account = store.get("account").unwrap_or("none");
        self.calls.borrow_mut().push(format!("{id}:{account}"));
        if self.ready.get() {
            Ok(Answer::Now(Value::str(&format!("live-{id}-{account}"))))
        } else {
            Ok(Answer::Later(Request::get(&format!(
                "https://example.test/profile/{id}"
            ))))
        }
    }

    fn grants(&self) -> &'static str {
        "secret.keep account\nnet.fetch https://example.test/\n"
    }

    fn ready(&self) -> bool {
        self.ready.get()
    }
}

#[test]
fn baked_owned_placeholder_waits_for_ready_then_requeries_live_child_scope() {
    let source = r#"
component App
  view
    column
      Loader()
component Loader
  state id = "a"
  action choose writes id
    id = "b"
  resource profile = profile(id) as shape string else `loading-${id}`
  view
    column
      text profile testId="profile"
      button "choose" press=choose testId="choose"
"#;
    let plan = contract::compile(source).unwrap();
    let bake_source = ReadinessSource {
        ready: Rc::new(Cell::new(false)),
        calls: Rc::new(RefCell::new(Vec::new())),
    };
    let baked = contract::bake(plan, bake_source).unwrap();

    let ready = Rc::new(Cell::new(false));
    let calls = Rc::new(RefCell::new(Vec::new()));
    let runtime_source = ReadinessSource {
        ready: ready.clone(),
        calls: calls.clone(),
    };
    let mut runner = Runner::boot_stored(
        baked,
        runtime_source,
        Kernel::with_monospace(),
        vec![("account".into(), "ada".into())],
    )
    .unwrap();

    assert_eq!(text(&runner, "profile"), "loading-a");
    assert!(runner.take_requests().is_empty());
    assert!(calls.borrow().is_empty());
    press(&mut runner, "choose");
    assert_eq!(text(&runner, "profile"), "loading-b");
    assert!(runner.take_requests().is_empty());
    assert!(calls.borrow().is_empty());
    assert!(runner.data_ready().unwrap().is_none());

    ready.set(true);
    assert!(runner.data_ready().unwrap().is_some());
    assert_eq!(text(&runner, "profile"), "live-b-ada");
    assert_eq!(calls.borrow().as_slice(), ["b:ada"]);
}

#[derive(Clone)]
struct UnloadedSource {
    ready: Rc<Cell<bool>>,
    calls: Rc<Cell<usize>>,
}

impl DataSource for UnloadedSource {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }

    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        assert!(
            self.ready.get(),
            "an unloaded data module must not be queried"
        );
        if source != "lateProfile" {
            return Err(DataError::UnknownSource(source.into()));
        }
        self.calls.set(self.calls.get() + 1);
        Ok(Answer::Later(Request::get(&format!(
            "https://example.test/late/{}",
            args[0].as_str().unwrap()
        ))))
    }

    fn parse(
        &mut self,
        _: &mut Store,
        source: &str,
        _: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        if source != "lateProfile" {
            return Err(DataError::UnknownSource(source.into()));
        }
        match outcome {
            Outcome::Response(response) => Ok(Answer::Now(Value::str(
                &String::from_utf8(response.body).unwrap(),
            ))),
            _ => Err(DataError::Unavailable("late request failed".into())),
        }
    }

    fn grants(&self) -> &'static str {
        "net.fetch https://example.test/\n"
    }

    fn ready(&self) -> bool {
        self.ready.get()
    }
}

#[test]
fn child_absent_at_bake_can_mount_during_unready_activation_window() {
    let source = r#"
component App
  state shown = false
  action show writes shown
    shown = true
  view
    column
      button "show" press=show testId="show"
      when shown
        LateProfile(id="late")
component LateProfile
  props
    id: string
  resource profile = lateProfile(id) as shape string else `loading-${id}`
  view
    column
      text profile testId="late-profile"
      text `${pending(profile)}` testId="late-pending"
"#;
    let ready = Rc::new(Cell::new(false));
    let calls = Rc::new(Cell::new(0));
    let data = UnloadedSource {
        ready: ready.clone(),
        calls: calls.clone(),
    };
    let baked = contract::bake(contract::compile(source).unwrap(), data.clone()).unwrap();
    assert_eq!(calls.get(), 0, "the child was absent during bake");

    let mut runner = Runner::boot(baked, data, Kernel::with_monospace()).unwrap();
    press(&mut runner, "show");
    assert_eq!(text(&runner, "late-profile"), "loading-late");
    assert_eq!(text(&runner, "late-pending"), "true");
    assert_eq!(calls.get(), 0);
    assert!(runner.take_requests().is_empty());

    ready.set(true);
    assert!(runner.data_ready().unwrap().is_some());
    assert_eq!(calls.get(), 1);
    let requests = runner.take_requests();
    assert_eq!(requests.len(), 1);
    runner
        .fulfill(
            requests[0].ticket,
            Outcome::Response(Response {
                status: 200,
                headers: vec![],
                body: b"loaded-late".to_vec(),
            }),
        )
        .unwrap();
    assert_eq!(text(&runner, "late-profile"), "loaded-late");
    assert_eq!(text(&runner, "late-pending"), "false");
}

struct RollbackSource;

impl DataSource for RollbackSource {
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
            "gate" if store.get("mode") == Some("bad") => {
                Err(DataError::Unavailable("root settlement refused".into()))
            }
            "gate" => Ok(Answer::Now(Value::str("open"))),
            "save" => {
                store.set("mode", "bad")?;
                Ok(Answer::Later(Request::post_json(
                    "https://example.test/save",
                    "{}",
                )))
            }
            other => Err(DataError::UnknownSource(other.into())),
        }
    }

    fn grants(&self) -> &'static str {
        "secret.keep mode\nnet.fetch https://example.test/\n"
    }
}

#[test]
fn prewalk_root_refusal_rolls_back_owned_slot_mutation_store_and_request() {
    let source = r#"
component App
  resource gate = gate() as shape string
  view
    column
      text gate testId="gate"
      Editor()
component Editor
  state edits = 0
  mutation result as shape string
  action submit writes edits, result
    edits = edits + 1
    send result = save()
  view
    column
      button "submit" press=submit testId="submit"
      text `${edits}` testId="edits"
      text `${pending(result)}` testId="saving"
      match result
        case some(value)
          text value testId="result"
        case none
          text "none" testId="result"
"#;
    let plan = contract::compile(source).unwrap();
    let mut runner = Runner::boot_stored(
        plan,
        RollbackSource,
        Kernel::with_monospace(),
        vec![("mode".into(), "ok".into())],
    )
    .unwrap();
    let key = runner.kernel().find_by_test_id("submit")[0];
    let view = runner.kernel().node_by_key(key).unwrap().id;

    assert!(runner.dispatch(view, Event::Press).is_err());
    assert!(!runner.is_poisoned());
    assert_eq!(text(&runner, "gate"), "open");
    assert_eq!(text(&runner, "edits"), "0");
    assert_eq!(text(&runner, "saving"), "false");
    assert_eq!(text(&runner, "result"), "none");
    assert_eq!(runner.store().get("mode"), Some("ok"));
    assert!(runner.take_store_writes().is_empty());
    assert!(runner.take_requests().is_empty());
    assert!(runner.pending().is_empty());
}

#[derive(Clone, Default)]
struct ReloadSource {
    calls: Rc<Cell<usize>>,
}

impl DataSource for ReloadSource {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        if source != "read" {
            return Err(DataError::UnknownSource(source.into()));
        }
        let next = self.calls.get() + 1;
        self.calls.set(next);
        Ok(Value::str(&format!("answer-{next}")))
    }
}

const RELOAD_SOURCE: &str = r#"
component App
  state rootCount = 0
  action bumpRoot writes rootCount
    rootCount = rootCount + 1
  view
    column
      button "root" press=bumpRoot testId="bump-root"
      text `${rootCount}` testId="root-count"
      Child()
component Child
  state childCount = 0
  resource value = read() as shape string
  action bumpChild writes childCount
    childCount = childCount + 1
  view
    column
      button "child" press=bumpChild testId="bump-child"
      text `${childCount}` testId="child-count"
      text value testId="child-resource"
"#;

#[test]
fn whole_plan_reload_retains_root_state_and_restarts_scoped_state_and_resource() {
    let source = ReloadSource::default();
    let calls = source.calls.clone();
    let mut runner = Runner::boot(
        contract::compile(RELOAD_SOURCE).unwrap(),
        source.clone(),
        Kernel::with_monospace(),
    )
    .unwrap();
    press(&mut runner, "bump-root");
    press(&mut runner, "bump-child");
    assert_eq!(text(&runner, "root-count"), "1");
    assert_eq!(text(&runner, "child-count"), "1");
    assert_eq!(text(&runner, "child-resource"), "answer-1");

    let carried = runner.carry();
    let reloaded = Runner::boot_carrying(
        contract::compile(RELOAD_SOURCE).unwrap(),
        source,
        Kernel::with_monospace(),
        &carried,
    )
    .unwrap();

    assert_eq!(text(&reloaded, "root-count"), "1");
    assert_eq!(text(&reloaded, "child-count"), "0");
    assert_eq!(text(&reloaded, "child-resource"), "answer-2");
    assert_eq!(calls.get(), 2);
}
