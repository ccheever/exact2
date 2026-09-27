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

fn text_of(r: &Runner<Blog>, test_id: &str) -> String {
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
