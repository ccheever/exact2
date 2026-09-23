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
        Ok(Answer::Now(match (source, outcome) {
            ("post", Outcome::Response(r)) if r.status == 200 => post("7", "Hello"),
            // A failure is data the source shapes (LLP 1016 D4).
            ("post", _) => post("7", "Unavailable"),
            ("comments", _) => Value::list(vec![Value::str("first")]),
            (other, _) => return Err(DataError::UnknownSource(other.into())),
        }))
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
fn without_a_placeholder_a_record_that_answers_later_still_refuses_and_says_how() {
    let src = corpus().replace(" else emptyPost()", "");
    let plan = contract::bake(contract::compile(&src).unwrap(), Blog::default()).unwrap();
    let Err(RunnerError::Data {
        resource,
        error: DataError::Unavailable(message),
    }) = boot(&plan, Blog::default(), "/post/7")
    else {
        panic!("a record with nothing to show refuses");
    };
    assert_eq!(resource, "post");
    assert!(message.contains("declare a placeholder"), "{message}");

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
