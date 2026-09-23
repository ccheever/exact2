//! The document's `head` (LLP 1048.003 D1): a node that takes no space, whose
//! fields the innermost active head sets, field by field.

use exact_kernel::{Kernel, NodeType, Offer, PropId};
use exact_runner::{agent, DataError, DataSource, Head, Runner, Value};
use std::path::Path;

#[derive(Default)]
struct Posts;

impl DataSource for Posts {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match source {
            "loadPost" => {
                let id = match args.first() {
                    Some(Value::List(ids)) => ids.first().and_then(Value::as_str).unwrap_or(""),
                    _ => "",
                };
                Ok(Value::record(vec![
                    Value::str(&format!("Post {id}")),
                    Value::str(&format!("What post {id} says")),
                ]))
            }
            other => Err(DataError::UnknownSource(other.into())),
        }
    }
}

fn boot(launch: &str) -> Runner<Posts> {
    let src = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../corpus/head.contract"),
    )
    .unwrap();
    Runner::boot(
        contract::compile(&src).unwrap(),
        Posts,
        Kernel::with_monospace(),
        Default::default(),
        launch,
    )
    .unwrap()
}

fn some(s: &str) -> Option<String> {
    Some(s.to_owned())
}

#[test]
fn the_innermost_active_head_wins_field_by_field() {
    assert_eq!(
        boot("/").head(),
        Head {
            title: some("Home"),
            description: some("Every post"),
            image: some("/assets/site.png"),
            canonical: some("/"),
            robots: some("index"),
        }
    );
    // The home route is still in the stack, covered: its head is inactive.
    assert_eq!(
        boot("/post/7").head(),
        Head {
            title: some("Post 7"),
            description: some("What post 7 says"),
            image: some("/assets/site.png"),
            canonical: some("/post/7"),
            robots: some("index"),
        }
    );
}

#[test]
fn heads_take_no_space_and_the_agent_reports_the_active_one() {
    let mut r = boot("/post/7");
    let heads: Vec<_> = r
        .kernel()
        .node_by_key(r.kernel().find_by_test_id("navigation")[0])
        .unwrap()
        .children()
        .into_iter()
        .filter(|id| r.kernel().node(*id).unwrap().node_type == NodeType::Head)
        .collect();
    assert_eq!(
        heads.len(),
        1,
        "the site's head is the navigation root's child"
    );
    let root = r.roots()[0];
    r.kernel_mut()
        .compute_layout(root, Offer::definite(390.0, 844.0))
        .unwrap();
    let site = r.kernel().node(heads[0]).unwrap();
    assert_eq!((site.frame.width, site.frame.height), (0.0, 0.0));
    assert_eq!(site.props.str(PropId::HeadTitle), Some("Site"));
    let state: serde_json::Value = serde_json::from_str(&agent::state(&r)).unwrap();
    assert_eq!(
        state["head"],
        serde_json::json!({
            "title": "Post 7",
            "description": "What post 7 says",
            "image": "/assets/site.png",
            "canonical": "/post/7",
            "robots": "index",
        })
    );
}

#[test]
fn a_page_with_no_head_has_none() {
    let r = Runner::boot(
        contract::compile("component A\n  view\n    text \"a\"\n").unwrap(),
        Posts,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(r.head(), Head::default());
    let state: serde_json::Value = serde_json::from_str(&agent::state(&r)).unwrap();
    assert!(state["head"]["title"].is_null());
}

#[test]
fn head_fields_belong_to_head_and_head_takes_only_them() {
    for (src, id, says) in [
        (
            "component A\n  view\n    column title=\"x\"\n      text \"a\"\n",
            "lower-attr-tag",
            "`title` belongs to `head`, not `column`",
        ),
        (
            "component A\n  view\n    column\n      head title=\"x\" testId=\"h\"\n",
            "lower-attr-tag",
            "`head` takes only title, description, image, canonical, robots; `testId` is not one",
        ),
        (
            "component A\n  view\n    column\n      head title=\"x\" width=10\n",
            "lower-attr-tag",
            "`width` is not one",
        ),
        (
            "component A\n  view\n    column\n      head title=\"x\"\n        text \"a\"\n",
            "lower-leaf-children",
            "`head` cannot hold children",
        ),
        (
            "component A\n  view\n    column\n      head \"x\"\n",
            "lower-positional",
            "`head` takes no positional argument",
        ),
        (
            "component A\n  view\n    title \"x\"\n",
            "lower-unknown-tag",
            "`head title=",
        ),
    ] {
        let error = contract::compile(src).unwrap_err();
        assert_eq!(error.id, id, "{src}: {error}");
        assert!(error.message.contains(says), "{src}: {}", error.message);
    }
}
