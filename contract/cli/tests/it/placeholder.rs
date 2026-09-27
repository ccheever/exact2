//! LLP 1048.003 D6 end to end: a resource whose source answers later, with
//! nothing kept for its arguments, shows its placeholder — declared, or a
//! list's empty value — with `pending(x)` true, where boot used to refuse.

use exact_kernel::{Kernel, PropId};
use exact_runner::{
    Answer, DataError, DataSource, FailureKind, Outcome, Request, Response, Runner, RunnerError,
    Store, Value,
};
use std::path::Path;

fn corpus() -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../corpus/placeholder.contract"),
    )
    .unwrap()
}

/// A blog whose posts and comments are fetched: the home page's (no post
/// parameter) answer now, as the build asks them; any other answers later.
#[derive(Default)]
struct Blog {
    later_at_build: bool,
    placeholder_later: bool,
    /// A module its host hasn't loaded yet: every answer refuses.
    not_loaded: std::rc::Rc<std::cell::Cell<bool>>,
    /// A failure isn't data here: parse refuses it (a worker-placed module).
    refuse_failures: bool,
}

fn post(id: &str, title: &str) -> Value {
    Value::record(vec![Value::str(id), Value::str(title)])
}

fn home(args: &[Value]) -> bool {
    matches!(args.first(), Some(Value::List(items)) if items.is_empty())
}

impl DataSource for Blog {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match source {
            "emptyPost" => Ok(post("", "")),
            "post" if home(args) => Ok(post("", "Home")),
            "comments" if home(args) => Ok(Value::list(Vec::new())),
            other => Err(DataError::Unavailable(format!("{other} answers later"))),
        }
    }

    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        if self.not_loaded.get() {
            return Err(DataError::Unavailable("the engine is not loaded".into()));
        }
        let later = match source {
            "emptyPost" => self.placeholder_later,
            _ => self.later_at_build || !home(args),
        };
        if later {
            return Ok(Answer::Later(Request::get(&format!(
                "https://blog.test/{source}"
            ))));
        }
        self.query(source, args).map(Answer::Now)
    }

    fn parse(
        &mut self,
        _: &mut Store,
        source: &str,
        _: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        if self.refuse_failures && matches!(outcome, Outcome::Failed { .. }) {
            return Err(DataError::Unavailable(
                "the module refused a failure".into(),
            ));
        }
        Ok(Answer::Now(match (source, outcome) {
            ("post", Outcome::Response(r)) if r.status == 200 => post("7", "Hello"),
            // A failure is data the source shapes (LLP 1016 D4).
            ("post", _) => post("7", "Unavailable"),
            ("comments", _) => Value::list(vec![Value::str("first")]),
            (other, _) => return Err(DataError::UnknownSource(other.into())),
        }))
    }

    fn ready(&self) -> bool {
        !self.not_loaded.get()
    }
}

fn ok() -> Outcome {
    Outcome::Response(Response {
        status: 200,
        headers: vec![],
        body: b"{}".to_vec(),
    })
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

fn boot(plan: &exact_plan::Plan, data: Blog, launch: &str) -> Result<Runner<Blog>, RunnerError> {
    Runner::boot(
        plan.clone(),
        data,
        Kernel::with_monospace(),
        Default::default(),
        launch,
    )
}

#[test]
fn a_deep_link_shows_placeholders_until_its_answers_arrive() {
    let plan = contract::bake(contract::compile(&corpus()).unwrap(), Blog::default()).unwrap();
    // The build answered the home page's post, its comments and the
    // placeholder, whose arguments never change.
    let named = |name: &str| {
        plan.resources
            .iter()
            .find(|r| plan.str(r.name) == name)
            .unwrap()
    };
    assert!(named("post").initial.len > 0);
    assert!(named("post#else").initial.len > 0);
    assert_eq!(named("post").placeholder.map(|p| p.0), Some(2));

    let mut r = boot(&plan, Blog::default(), "/post/7").unwrap();
    assert_eq!(text_of(&r, "title"), "");
    assert_eq!(text_of(&r, "state"), "loading");
    assert_eq!(text_of(&r, "comments"), "0 comments");
    assert!(
        !r.journal().any(|l| l.contains("query post#else")),
        "the compiled placeholder is not asked again"
    );
    let requests = r.take_requests();
    let targets: Vec<&str> = requests.iter().map(|q| q.target.as_str()).collect();
    assert_eq!(targets, ["post", "comments"]);

    r.fulfill(requests[0].ticket, ok()).unwrap();
    assert_eq!(text_of(&r, "title"), "Hello");
    assert_eq!(text_of(&r, "state"), "ready");
    r.fulfill(requests[1].ticket, ok()).unwrap();
    assert_eq!(text_of(&r, "comments"), "1 comments");

    // A failed request is the source's to shape: its answer replaces the
    // placeholder like any other.
    let mut failed = boot(&plan, Blog::default(), "/post/7").unwrap();
    let ticket = failed.take_requests()[0].ticket;
    failed
        .fulfill(
            ticket,
            Outcome::Failed {
                kind: FailureKind::Network,
                message: "offline".into(),
            },
        )
        .unwrap();
    assert_eq!(text_of(&failed, "title"), "Unavailable");
    assert_eq!(text_of(&failed, "state"), "ready");
}

#[test]
fn a_source_that_answers_later_at_build_is_asked_at_launch() {
    let data = Blog {
        later_at_build: true,
        ..Blog::default()
    };
    let plan = contract::bake(contract::compile(&corpus()).unwrap(), data).unwrap();
    for (name, compiled) in [("post", false), ("comments", false), ("post#else", true)] {
        let row = plan
            .resources
            .iter()
            .find(|r| plan.str(r.name) == name)
            .unwrap();
        assert_eq!(row.initial.len > 0, compiled, "{name}");
    }
    let data = Blog {
        later_at_build: true,
        ..Blog::default()
    };
    let mut r = boot(&plan, data, "/").unwrap();
    assert_eq!(text_of(&r, "state"), "loading");
    assert_eq!(r.take_requests().len(), 2);
}

#[test]
fn without_a_placeholder_a_record_that_answers_later_shows_its_zero_pending() {
    // @ref LLP 1054.000.002 D1 — where boot used to refuse.
    let src = corpus().replace(" else emptyPost()", "");
    let plan = contract::bake(contract::compile(&src).unwrap(), Blog::default()).unwrap();
    let mut r = boot(&plan, Blog::default(), "/post/7").unwrap();
    assert_eq!(text_of(&r, "title"), "");
    assert_eq!(text_of(&r, "state"), "loading");
    let ticket = r
        .take_requests()
        .into_iter()
        .find(|q| q.target == "post")
        .unwrap()
        .ticket;
    r.fulfill(ticket, ok()).unwrap();
    assert_eq!(text_of(&r, "title"), "Hello");
    assert_eq!(text_of(&r, "state"), "ready");

    // A placeholder answers now: one that answers later names its resource.
    let data = Blog {
        placeholder_later: true,
        ..Blog::default()
    };
    let Err(RunnerError::Data {
        resource,
        error: DataError::Unavailable(message),
    }) = boot(&contract::compile(&corpus()).unwrap(), data, "/post/7")
    else {
        panic!("a placeholder that answers later refuses");
    };
    assert_eq!(resource, "post");
    assert!(message.contains("a placeholder answers now"), "{message}");
}

#[test]
fn a_placeholder_is_a_source_call_over_values() {
    let reads = corpus().replace("else emptyPost()", "else emptyPost(params(nav, \"post\"))");
    let e = contract::compile(&reads).unwrap_err();
    assert!(format!("{e}").contains("type-placeholder-reads"), "{e}");
    assert!(
        format!("{e}").contains("`post`'s placeholder reads `nav`"),
        "{e}"
    );
    // A source keeps one signature wherever it is named.
    let clash = corpus().replace(
        "resource comments = comments(params(nav, \"post\"))",
        "resource comments = emptyPost(params(nav, \"post\"))",
    );
    let e = contract::compile(&clash).unwrap_err();
    assert!(format!("{e}").contains("type-source-signature"), "{e}");
    // Values are fine, and the placeholder's source is in the seam's table.
    let values = corpus().replace("else emptyPost()", "else emptyPost(\"draft\", 2)");
    let plan = contract::compile(&values).unwrap();
    assert!(plan
        .sources
        .iter()
        .any(|s| plan.str(s.name) == "emptyPost" && s.params.len == 2));
}

#[test]
fn a_module_not_loaded_at_boot_shows_placeholders_until_data_ready() {
    // The build couldn't answer the post or its comments, so nothing is
    // compiled for them; the placeholder's source answered.
    let built = Blog {
        later_at_build: true,
        ..Blog::default()
    };
    let plan = contract::bake(contract::compile(&corpus()).unwrap(), built).unwrap();
    let data = Blog::default();
    data.not_loaded.set(true);
    let loaded = data.not_loaded.clone();
    let mut r = boot(&plan, data, "/post/7").unwrap();
    assert_eq!(text_of(&r, "title"), "");
    assert_eq!(text_of(&r, "state"), "loading");
    assert_eq!(text_of(&r, "comments"), "0 comments");
    assert!(
        r.take_requests().is_empty(),
        "nothing asks a module that isn't loaded"
    );
    loaded.set(false);
    r.data_ready().unwrap();
    let requests = r.take_requests();
    let targets: Vec<&str> = requests.iter().map(|q| q.target.as_str()).collect();
    assert_eq!(targets, ["post", "comments"]);
    // The bake answered the placeholder's own arguments from no store: that
    // answer stands, as it does when the source is ready at boot (Seth's
    // Crew port asked each `#else` again, a worker turn each).
    assert!(
        !r.journal().any(|l| l.contains("query post#else")),
        "{:?}",
        r.journal().collect::<Vec<_>>()
    );
    assert_eq!(text_of(&r, "state"), "loading");
    r.fulfill(requests[0].ticket, ok()).unwrap();
    assert_eq!(text_of(&r, "title"), "Hello");
    assert_eq!(text_of(&r, "state"), "ready");
}

#[test]
fn asks_refused_admission_are_asked_again_once_the_last_refusal_settles() {
    let plan = contract::bake(contract::compile(&corpus()).unwrap(), Blog::default()).unwrap();
    let data = Blog {
        refuse_failures: true,
        ..Blog::default()
    };
    let mut r = boot(&plan, data, "/post/7").unwrap();
    let asked = r.take_requests();
    assert_eq!(asked.len(), 2);
    for q in &asked {
        r.refuse_request(q.ticket, "native executor admission limit reached", true);
    }
    // The source can't shape the refusal: the ticket isn't kept pending
    // forever. Asked again now, it would be refused behind the other one.
    let (first, outcome) = r.take_request_refusal(true).unwrap();
    assert_eq!(r.fulfill(first, outcome).unwrap(), None);
    assert!(!r.holds(first));
    assert!(r.take_requests().is_empty());
    let (second, outcome) = r.take_request_refusal(true).unwrap();
    assert!(r.fulfill(second, outcome).unwrap().is_some());
    assert!(!r.holds(second));
    let again = r.take_requests();
    let targets: Vec<&str> = again.iter().map(|q| q.target.as_str()).collect();
    assert_eq!(targets, ["post", "comments"]);
    assert!(again.iter().all(|q| r.holds(q.ticket)));
    assert_eq!(text_of(&r, "state"), "loading");
    assert_eq!(
        r.journal()
            .filter(|l| l.contains("was refused admission: asked again"))
            .count(),
        2
    );
}

#[test]
fn empty_gives_the_zero_with_named_fields_replaced() {
    // @ref LLP 1054.000.002 D2/D3 — no source, and nothing at the bake.
    let src = corpus().replace("else emptyPost()", "else empty(title=\"Untitled\")");
    let plan = contract::bake(contract::compile(&src).unwrap(), Blog::default()).unwrap();
    assert!(!plan.sources.iter().any(|s| plan.str(s.name) == "empty"));
    assert!(!plan
        .resources
        .iter()
        .any(|r| plan.str(r.name) == "post#else"));
    let r = boot(&plan, Blog::default(), "/post/7").unwrap();
    assert_eq!(text_of(&r, "title"), "Untitled");
    assert_eq!(text_of(&r, "state"), "loading");
    // Every mistake is named, together.
    for (placeholder, id) in [
        ("empty(titel=\"x\")", "type-placeholder-field"),
        (
            "empty(title=\"x\", title=\"y\")",
            "type-placeholder-duplicate",
        ),
        ("empty(\"x\")", "type-placeholder-fields"),
        ("empty(title=3)", "type-placeholder-type"),
        (
            "empty(title=params(nav, \"post\"))",
            "type-placeholder-value",
        ),
    ] {
        let e = contract::compile(&corpus().replace("emptyPost()", placeholder)).unwrap_err();
        assert!(format!("{e}").contains(id), "{placeholder}: {e}");
    }
}

#[test]
fn a_placeholder_shown_before_the_source_is_ready_is_never_compiled_as_its_answer() {
    // @ref LLP 1054.000.002 D4 — no ticket, and still not an answer.
    let not_loaded = std::rc::Rc::new(std::cell::Cell::new(true));
    let data = Blog {
        not_loaded: not_loaded.clone(),
        ..Blog::default()
    };
    let src = corpus().replace(" else emptyPost()", "");
    let plan = contract::bake(contract::compile(&src).unwrap(), data).unwrap();
    let row = plan
        .resources
        .iter()
        .find(|r| plan.str(r.name) == "post")
        .unwrap();
    assert_eq!(
        row.initial.len, 0,
        "the zero shown at the bake is not compiled"
    );
}

#[test]
fn empty_nests_and_every_other_field_is_its_zero() {
    let src = "shape Author\n  name: string\n  found: bool\n  image: option<string>\nshape Article\n  title: string\n  author: Author\n  count: number\n  tags: list<string>\ncomponent App\n  resource post = article() as shape Article else empty(title=\"…\", count=-1, author=empty(found=true, image=some(\"/a.svg\")))\n  view\n    column\n      text post.author.name\n";
    let plan = contract::compile(src).unwrap();
    let row = plan
        .resources
        .iter()
        .find(|r| plan.str(r.name) == "post")
        .unwrap();
    let value = Value::from_bytes(plan.bytes(row.placeholder_value)).unwrap();
    let author = Value::record(vec![
        Value::str(""),
        Value::Bool(true),
        Value::some(Value::str("/a.svg")),
    ]);
    assert_eq!(
        value,
        Value::record(vec![
            Value::str("…"),
            author,
            Value::Number(-1.0),
            Value::list(vec![])
        ])
    );
}

#[derive(Default)]
struct FailedSearch {
    asks: usize,
    stored: bool,
    succeed: bool,
}

impl DataSource for FailedSearch {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        unreachable!()
    }
    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        if source == "touch" {
            store.set("revision", args[0].as_str().unwrap())?;
            return Ok(Answer::Now(Value::Bool(true)));
        }
        if source == "report" {
            return Ok(Answer::Now(args[0].clone()));
        }
        if source == "rows" {
            return Ok(Answer::Now(Value::list(vec![
                Value::Number(1.0),
                Value::Number(2.0),
            ])));
        }
        self.asks += 1;
        if args[0].as_str() == Some("broken") {
            return Err(DataError::Unavailable("query refused".into()));
        }
        if self.stored {
            store.get("revision");
        }
        if args[0].as_str() == Some("Menl") {
            Ok(Answer::Now(Value::str("standing")))
        } else {
            Ok(Answer::Later(Request::get("https://search.test/")))
        }
    }
    fn parse(
        &mut self,
        _: &mut Store,
        _: &str,
        _: &[Value],
        _: Outcome,
    ) -> Result<Answer, DataError> {
        if self.succeed {
            Ok(Answer::Now(Value::str("new answer")))
        } else {
            Err(DataError::Unavailable("deterministic failure".into()))
        }
    }
    fn grants(&self) -> &'static str {
        "secret.keep revision\n"
    }
}

fn failed_search(stored: bool) -> Runner<FailedSearch> {
    let src = "component App\n  state query = \"Menl\"\n  resource results = search(query) as shape string\n  mutation changed as shape bool\n  action search(q: string) writes query\n    query = q\n  action touch(v: string) writes changed\n    send changed = touch(v)\n  action retry\n    refresh results\n  view\n    text `${results}/${pending(results)}`\n";
    Runner::boot(
        contract::compile(src).unwrap(),
        FailedSearch {
            stored,
            ..Default::default()
        },
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

#[test]
fn a_failed_request_keeps_the_standing_answers_arguments() {
    let mut r = failed_search(false);
    r.act("search", vec![Value::str("Menlo Park")]).unwrap();
    let ticket = r.take_requests()[0].ticket;
    r.fulfill(ticket, ok()).unwrap();
    assert!(r.pending().is_empty());
    assert!(r.take_requests().is_empty());
    assert_eq!(r.resource("results"), Some(&Value::str("standing")));
    let checkpoint = r.document_checkpoint("/");
    assert_eq!(checkpoint.answers[0].2, [Value::str("Menl")]);
}

#[test]
fn a_failed_store_reader_waits_for_changed_arguments_or_refresh() {
    let mut r = failed_search(true);
    r.act("search", vec![Value::str("Menlo Park")]).unwrap();
    let ticket = r.take_requests()[0].ticket;
    r.fulfill(ticket, ok()).unwrap();
    // A refused argument change must restore the failure marker too.
    assert!(r.act("search", vec![Value::str("broken")]).is_err());
    assert_eq!(r.slot("query"), Some(&Value::str("Menlo Park")));
    let asks = r.data().asks;
    for revision in ["one", "two"] {
        r.set_full_evaluation(revision == "two");
        r.act("touch", vec![Value::str(revision)]).unwrap();
        assert_eq!(
            r.data().asks,
            asks,
            "store changes must not retry a failed query"
        );
        assert!(r.take_requests().is_empty());
    }
    r.act("search", vec![Value::str("Menlo")]).unwrap();
    let ticket = r.take_requests()[0].ticket;
    r.fulfill(ticket, ok()).unwrap();
    r.act("search", vec![Value::str("Menlo Park")]).unwrap();
    let ticket = r.take_requests()[0].ticket;
    r.fulfill(ticket, ok()).unwrap();
    r.act("retry", vec![]).unwrap();
    let ticket = r.take_requests()[0].ticket;
    r.data().succeed = true;
    r.fulfill(ticket, ok()).unwrap();
    assert_eq!(r.resource("results"), Some(&Value::str("new answer")));
    assert_eq!(
        r.document_checkpoint("/").answers[0].2,
        [Value::str("Menlo Park")]
    );
    r.act("touch", vec![Value::str("three")]).unwrap();
    assert_eq!(
        r.take_requests().len(),
        1,
        "success clears the failure marker"
    );
}

#[test]
fn a_failed_placeholder_stays_a_placeholder_without_becoming_an_answer() {
    for (placeholder, title) in [
        ("else emptyPost()", ""),
        ("", ""),
        ("else empty(title=\"Waiting\")", "Waiting"),
    ] {
        let src = corpus()
            .replace("else emptyPost()", placeholder)
            .replace("  view", "  action retry\n    refresh post\n  view");
        let plan = contract::compile(&src).unwrap();
        let mut r = boot(
            &plan,
            Blog {
                refuse_failures: true,
                ..Default::default()
            },
            "/post/7",
        )
        .unwrap();
        let ticket = r
            .take_requests()
            .iter()
            .find(|q| q.target == "post")
            .unwrap()
            .ticket;
        r.fulfill(
            ticket,
            Outcome::Failed {
                kind: FailureKind::Network,
                message: "offline".into(),
            },
        )
        .unwrap();
        assert_eq!(text_of(&r, "title"), title);
        assert_eq!(text_of(&r, "state"), "failed");
        assert!(!r.pending().iter().any(|(name, _)| name == "post"));
        assert!(r.take_requests().is_empty());
        assert!(r.resource_is_placeholder("post"));
        assert!(!r.carry().resources.iter().any(|(name, ..)| name == "post"));
        assert!(!r
            .document_checkpoint("/post/7")
            .answers
            .iter()
            .any(|(name, ..)| name == "post"));
        r.act("retry", vec![]).unwrap();
        assert_eq!(text_of(&r, "state"), "loading");
        assert!(r.resource_is_placeholder("post"));
        let ticket = r.take_requests()[0].ticket;
        r.fulfill(ticket, ok()).unwrap();
        assert_eq!(text_of(&r, "state"), "ready");
        assert_eq!(text_of(&r, "title"), "Hello");
        assert!(!r.resource_is_placeholder("post"));
    }
}

#[test]
fn failed_search_is_readable_and_clears_on_changed_arguments_refresh_and_success() {
    let src = r#"component App
  state query = "Menlo Park"
  resource results = search(query) as shape string
  derive unavailable = failed(results)
  resource status = report(failed(results)) as shape bool
  resource rows = rows() as shape list<number>
  mutation changed as shape bool
  action search(q: string) writes query
    query = q
  action touch(v: string) writes changed
    send changed = touch(v)
  action retry
    refresh results
  view
    column
      text `${results}/${pending(results)}/${failed(results)}` testId="state"
      text `${unavailable}/${status}` testId="derived"
      each n in rows key=n
        text `${failed(results)}` testId=`row-${n}`
      when failed(results)
        text "Couldn't load" testId="failure"
"#;
    for full in [false, true] {
        let mut r = Runner::boot(
            contract::compile(src).unwrap(),
            FailedSearch {
                stored: true,
                ..Default::default()
            },
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        r.set_full_evaluation(full);
        let check = |r: &Runner<FailedSearch>, value: &str, pending: bool, failed: bool| {
            assert_eq!(text_of(r, "state"), format!("{value}/{pending}/{failed}"));
            assert_eq!(text_of(r, "derived"), format!("{failed}/{failed}"));
            for row in ["row-1", "row-2"] {
                assert_eq!(text_of(r, row), failed.to_string());
            }
            assert_eq!(!r.kernel().find_by_test_id("failure").is_empty(), failed);
        };
        check(&r, "", true, false);
        let ticket = r.take_requests()[0].ticket;
        r.fulfill(ticket, ok()).unwrap();
        check(&r, "", false, true);
        assert!(r.resource_is_placeholder("results"));
        assert!(!r
            .document_checkpoint("/")
            .answers
            .iter()
            .any(|(name, ..)| name == "results"));
        assert!(r.act("search", vec![Value::str("broken")]).is_err());
        check(&r, "", false, true);
        let asks = r.data().asks;
        r.act("touch", vec![Value::str("one")]).unwrap();
        check(&r, "", false, true);
        assert_eq!(r.data().asks, asks);
        assert!(r.take_requests().is_empty());

        r.act("search", vec![Value::str("Menlo")]).unwrap();
        check(&r, "", true, false);
        let ticket = r.take_requests()[0].ticket;
        r.fulfill(ticket, ok()).unwrap();
        check(&r, "", false, true);
        r.act("retry", vec![]).unwrap();
        check(&r, "", true, false);
        let ticket = r.take_requests()[0].ticket;
        r.data().succeed = true;
        r.fulfill(ticket, ok()).unwrap();
        check(&r, "new answer", false, false);
        assert!(!r.resource_is_placeholder("results"));
        assert_eq!(
            r.document_checkpoint("/").answers[0].2,
            [Value::str("Menlo")]
        );

        r.data().succeed = false;
        r.act("search", vec![Value::str("Menlo Park")]).unwrap();
        check(&r, "new answer", true, false);
        let ticket = r.take_requests()[0].ticket;
        r.fulfill(ticket, ok()).unwrap();
        check(&r, "new answer", false, true);
        assert!(!r.resource_is_placeholder("results"));
        assert_eq!(
            r.document_checkpoint("/").answers[0].2,
            [Value::str("Menlo")]
        );
        r.act("search", vec![Value::str("Menl")]).unwrap();
        check(&r, "standing", false, false);
    }
}

#[test]
fn failed_names_one_resource() {
    let src = "component App\n  state query = \"Menl\"\n  resource results = search(query) as shape string\n  mutation changed as shape bool\n  view\n    text `${failed(results)}`\n";
    for expression in [
        "failed()",
        "failed(results, results)",
        "failed(query)",
        "failed(changed)",
        "failed(1)",
        "failed(missing)",
    ] {
        let error = contract::compile(&src.replace("failed(results)", expression)).unwrap_err();
        assert_eq!(error.id, "type-failed-argument", "{expression}: {error}");
    }
    let error = contract::compile(&src.replace("failed(results)", "faild(results)")).unwrap_err();
    assert!(error.message.contains("did you mean `failed`?"), "{error}");
}
