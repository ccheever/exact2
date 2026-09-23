//! Documents in Contract (LLP 1048.003 part A): the `head` (D1), a node that
//! takes no space, whose fields the innermost active head sets field by field;
//! `scroll document` (D4).

use exact_kernel::{Kernel, NodeType, Offer, PropId};
use exact_plan::{ActivatePolicy, RenderPolicy};
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
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../corpus/document.contract"),
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
            status: None,
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
            status: None,
        }
    );
    // The not-found view declares its status; its robots wins over the site's.
    let missing = boot("/nowhere").head();
    assert_eq!(missing.status, Some(404));
    assert_eq!(missing.title, some("Not found"));
    assert_eq!(missing.robots, some("noindex"));
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
            "status": null,
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
            "`head` takes only title, description, image, canonical, robots, status; `testId` is not one",
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
            "component A\n  view\n    column\n      head status=200\n",
            "lower-attr-value",
            "`status` is 404 or 410",
        ),
        (
            "component A\n  state gone = true\n  view\n    column\n      head status=(gone ? 410 : 404)\n",
            "lower-attr-value",
            "as a literal",
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

#[test]
fn scroll_document_marks_the_page_scroller_and_nothing_else() {
    let r = boot("/");
    let k = r.kernel();
    let page = k.node_by_key(k.find_by_test_id("page")[0]).unwrap();
    assert_eq!(page.node_type, NodeType::ScrollView);
    assert_eq!(
        page.props.get(PropId::ScrollDocument),
        Some(&exact_kernel::PropValue::Bool(true))
    );
    let inner = k.node_by_key(k.find_by_test_id("inner")[0]).unwrap();
    assert_eq!(inner.props.get(PropId::ScrollDocument), None);
    // The word is read by its spelling, whatever is in scope.
    let shadowed = "component A\n  state document = 1\n  view\n    scroll document height=10\n      text \"a\"\n";
    let plan = contract::compile(shadowed).unwrap();
    assert!(plan
        .bindings
        .iter()
        .any(|b| b.id == PropId::ScrollDocument as u16));
    for (src, id, says) in [
        (
            "component A\n  view\n    scroll \"document\" height=10\n      text \"a\"\n",
            "lower-positional",
            "`scroll` takes one word, `document`",
        ),
        // Natively the page's scroller is still bounded by its layout.
        (
            "component A\n  view\n    scroll document\n      text \"a\"\n",
            "lower-scroll-unbounded",
            "never scroll",
        ),
        (
            "component A\n  view\n    column document\n      text \"a\"\n",
            "type-unknown-name",
            "document",
        ),
    ] {
        let error = contract::compile(src).unwrap_err();
        assert_eq!(error.id, id, "{src}: {error}");
        assert!(error.message.contains(says), "{src}: {}", error.message);
    }
}

#[test]
fn routes_declare_their_render_and_activation_policies() {
    let r = boot("/");
    let plan = r.plan();
    let policy = |name: &str| {
        let row = plan
            .routes
            .iter()
            .find(|row| plan.str(row.name) == name)
            .unwrap();
        (row.render, row.activate)
    };
    assert_eq!(
        policy("home"),
        (RenderPolicy::Build, ActivatePolicy::Inferred)
    );
    assert_eq!(
        policy("post"),
        (RenderPolicy::Request, ActivatePolicy::Idle)
    );
    assert_eq!(
        policy("notfound"),
        (RenderPolicy::Build, ActivatePolicy::Inferred)
    );
    // Undeclared is `client`: nothing renders a route that didn't ask.
    let plain =
        contract::compile("routes nav\n  tab home \"/\"\ncomponent A\n  view\n    text \"a\"\n")
            .unwrap();
    assert_eq!(plain.routes[0].render, RenderPolicy::Client);
    // The formatter keeps a field's spelling.
    let src = "routes nav\n  tab home \"/\" render=build activate=never\ncomponent A\n  view\n    text \"a\"\n";
    assert_eq!(contract_syntax::fmt::format(src).unwrap(), src);
    for (src, says) in [
        (
            "routes nav\n  tab home \"/\" cache=build\n",
            "a route has no field `cache`",
        ),
        (
            "routes nav\n  tab home \"/\" render=static\n",
            "`render` is a word: client, build, cached or request (route `home`)",
        ),
        (
            "routes nav\n  tab home \"/\" render=\"build\"\n",
            "`render` is a word",
        ),
        (
            "routes nav\n  tab home \"/\" activate=eager\n",
            "`activate` is a word: idle or never",
        ),
        (
            "routes nav\n  tab home \"/\"\n    post \"/post/:post\" render=build\n",
            "route `post` has parameters",
        ),
    ] {
        let src = format!("{src}component A\n  view\n    text \"a\"\n");
        let error = contract::compile(&src).unwrap_err();
        assert_eq!(error.id, "lower-route-field", "{src}: {error}");
        assert!(error.message.contains(says), "{src}: {}", error.message);
    }
}
