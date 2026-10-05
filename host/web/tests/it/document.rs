//! The document projection (LLP 1048.000 D1) over the Caltrain app and small
//! plans: the live host's DOM as HTML, before browser layout.

use exact_runner::{DataError, DataSource, Value};
use exact_web::document::DocumentError;
use exact_web::Host;

/// A source that answers every call with one string, so a view can bind
/// text no Contract literal can spell.
#[derive(Clone)]
struct Says(&'static str);

impl DataSource for Says {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        Ok(Value::str(self.0))
    }
}

fn host<D: DataSource + Clone>(src: &str, data: D, launch: &str) -> Host<D> {
    let plan = contract::bake(contract::compile(src).unwrap(), data.clone()).unwrap();
    exact_web::link(exact_web_capabilities::ALL);
    Host::boot(&plan.encode(), data, Default::default(), launch)
        .unwrap()
        .0
}

fn document(src: &str) -> String {
    host(src, Says(""), "/").document().unwrap().root
}

#[test]
fn raw_symbols_prerender_as_nonfetching_decorative_images() {
    let doc = document(
        r#"component App
  view
    row font-size=28
      image "symbol:sf/airpodsmax" testId="raw"
      image "symbol:sf/" width=40 testId="empty"
"#,
    );
    assert!(
        doc.contains("data-symbol-source=\"symbol:sf/airpodsmax\""),
        "{doc}"
    );
    assert!(doc.contains("data-symbol-source=\"symbol:sf/\""), "{doc}");
    assert_eq!(doc.matches("src=\"data:image/svg+xml,").count(), 2, "{doc}");
    assert!(!doc.contains("src=\"symbol:"), "{doc}");
    assert_eq!(doc.matches("width='28' height='28'").count(), 2, "{doc}");
    assert_eq!(doc.matches("data-symbol-path=\"\"").count(), 2, "{doc}");
    assert_eq!(doc.matches("alt=\"\"").count(), 2, "{doc}");
}

fn caltrain() -> (Host<caltrain_data::Caltrain>, String) {
    let plan = caltrain::build().unwrap();
    exact_web::link(exact_web_capabilities::ALL);
    Host::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        Default::default(),
        "/",
    )
    .unwrap()
}

/// The opening tag of the element carrying `data-view="id"`.
fn opening(doc: &str, id: u64) -> &str {
    let at = doc
        .find(&format!(" data-view=\"{id}\""))
        .unwrap_or_else(|| panic!("no element for view {id}"));
    let start = doc[..at].rfind('<').unwrap();
    let end = at + doc[at..].find('>').unwrap() + 1;
    &doc[start..end]
}

#[test]
fn caltrain_document_is_the_first_batch_as_html() {
    let (host, first) = caltrain();
    let doc = host.document().unwrap().root;
    // Every element the first batch creates is in the document once, under
    // the tag the glue creates (a canvas is its div).
    let mut created = 0;
    for op in first.split("{\"op\":\"create\",\"id\":").skip(1) {
        let id: u64 = op[..op.find(',').unwrap()].parse().unwrap();
        let tag = op.split("\"tag\":\"").nth(1).unwrap();
        let tag = &tag[..tag.find('"').unwrap()];
        let element = if tag == "canvas" { "div" } else { tag };
        let open = opening(&doc, id);
        assert!(open.starts_with(&format!("<{element} ")), "{id}: {open}");
        assert_eq!(doc.matches(&format!(" data-view=\"{id}\"")).count(), 1);
        created += 1;
    }
    assert!(created > 50, "{created}");
    assert_eq!(doc.matches(" data-view=\"").count(), created);
    // A parser keeps whitespace between elements as text; there is none.
    assert!(!doc.contains("> <") && !doc.contains(">\n<"));
    // The canvas's surface element comes first, as the glue creates it.
    let sky = doc.find("data-testid=\"sky\"").unwrap();
    assert!(doc[sky..].split_once('>').unwrap().1.starts_with(
        "<canvas data-surface=\"\" style=\"position:absolute;inset:0;width:100%;height:100%;display:block;z-index:-1\"></canvas>"
    ));
}

#[test]
fn repeated_renders_in_one_process_are_equal() {
    // Runtime incarnation ids (the reorder runtime counter) differ between
    // hosts in one process; the document never carries them.
    let a = caltrain().0.document().unwrap();
    let b = caltrain().0.document().unwrap();
    assert_eq!(a, b);
}

#[test]
fn hostile_text_and_attributes_are_escaped() {
    let src = r#"
component App
  resource said = say() as shape string
  view
    column testId=said
      text said
"#;
    let doc = host(src, Says("</div><script>alert(\"x\")</script>&amp;\r"), "/")
        .document()
        .unwrap()
        .root;
    assert!(doc.contains(
        "data-testid=\"&lt;/div&gt;&lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt;&amp;amp;&#13;\""
    ));
    assert!(
        doc.contains(">&lt;/div&gt;&lt;script&gt;alert(\"x\")&lt;/script&gt;&amp;amp;&#13;</div>")
    );
    assert!(!doc.contains("<script"));
}

#[test]
fn a_nul_is_refused_with_its_view() {
    let src = r#"
component App
  resource said = say() as shape string
  view
    column
      text said testId="said"
"#;
    let host = host(src, Says("a\0b"), "/");
    let DocumentError { view, reason } = host.document().unwrap_err();
    assert!(reason.contains("NUL"), "{reason}");
    let node = host.runner().kernel().node(view).unwrap();
    assert_eq!(
        node.props.str(exact_kernel::PropId::TestId),
        Some("said"),
        "the refusal names the view"
    );
}

#[test]
fn links_that_would_run_script_lose_their_href() {
    let src = r#"
component App
  resource said = say() as shape string
  view
    column
      link href="https://e.dev/a?b=1" testId="good" width=10 height=10
      link href="javascript:alert(1)" testId="bad" width=10 height=10
      link href=said testId="bound" width=10 height=10
      text "run "
        text "here" href=said testId="run"
      iframe src="javascript:alert(1)" testId="frame"
"#;
    let doc = host(src, Says(" java\tscript:alert(2)"), "/")
        .document()
        .unwrap()
        .root;
    let tag = |test_id: &str| {
        let at = doc.find(&format!("data-testid=\"{test_id}\"")).unwrap();
        let start = doc[..at].rfind('<').unwrap();
        doc[start..at + doc[at..].find('>').unwrap()].to_owned()
    };
    assert!(tag("good").contains(" href=\"https://e.dev/a?b=1\""));
    for refused in ["bad", "bound", "run"] {
        let open = tag(refused);
        assert!(open.starts_with("<a "), "{open}");
        assert!(!open.contains("href"), "{open}");
    }
    assert!(tag("frame").contains(" src=\"about:blank\""));
}

#[test]
fn properties_become_the_attributes_and_text_the_glue_gives_them() {
    let src = r#"
component App
  state n = 0
  action bump
    n = n + 1
  view
    column inert=true
      input value="hi" disabled=true readonly=true testId="field" width=10
      input value="" disabled=false testId="enabled" width=10
      textarea value="\nfirst" testId="area" width=10
      view focus=bump testId="focusable" width=10 height=10
      button focus=bump testId="button" width=10 height=10
      box tabindex=0 testId="plain-stop" width=10 height=10
      view focus=bump tabindex=-1 testId="out-of-order" width=10 height=10
      box testId="no-stop" width=10 height=10
      scroll scrollTop=40 testId="scroller" height=10
      canvas width=10 height=10 testId="canvas"
"#;
    let doc = document(src);
    let tag = |test_id: &str| {
        let at = doc.find(&format!("data-testid=\"{test_id}\"")).unwrap();
        let start = doc[..at].rfind('<').unwrap();
        doc[start..at + doc[at..].find('>').unwrap() + 1].to_owned()
    };
    assert!(doc.starts_with("<div inert "), "{doc}");
    let field = tag("field");
    for want in [" disabled ", " readonly ", " value=\"hi\""] {
        assert!(field.contains(want), "{want}: {field}");
    }
    let enabled = tag("enabled");
    assert!(!enabled.contains("disabled"), "{enabled}");
    // The parser drops a newline right after `<textarea>`: one more keeps it.
    let area = doc.find("data-testid=\"area\"").unwrap();
    assert!(doc[area..].contains(">\n\nfirst</textarea>"));
    assert!(tag("focusable").contains(" tabindex=\"0\""));
    assert!(!tag("button").contains("tabindex"));
    // LLP 1088 D7.3: an authored `tabindex` is the attribute itself, it wins
    // over the synthesized one, and absent is never `0`.
    assert!(
        tag("plain-stop").contains(" tabindex=\"0\""),
        "{}",
        tag("plain-stop")
    );
    let out = tag("out-of-order");
    assert!(
        out.contains(" tabindex=\"-1\"") && !out.contains("tabindex=\"0\""),
        "{out}"
    );
    assert!(!tag("no-stop").contains("tabindex"), "{}", tag("no-stop"));
    // Scroll offsets are the browser's, never attributes.
    assert!(!tag("scroller").to_lowercase().contains("scrolltop"));
    let canvas = tag("canvas");
    assert!(canvas.starts_with("<div "), "{canvas}");
    assert!(canvas.contains("position:relative;isolation:isolate;"));
}

#[test]
fn markdown_is_spans_and_navigable_links() {
    let src = r#"
component App
  view
    column
      text "**b** [safe](https://e.dev/) [unsafe](javascript:alert(1))\nnext" markup="markdown" testId="md"
"#;
    // Markdown is linked by use (LLP 1047 D3): this binary links it, as an
    // app's generated entry does when its plan uses it.
    exact_web::link(exact_web_capabilities::ALL);
    let doc = document(src);
    assert!(
        doc.contains("<span style=\"font-weight:700;\">b</span>"),
        "{doc}"
    );
    assert!(doc.contains("<a href=\"https://e.dev/\">safe</a>"), "{doc}");
    assert!(doc.contains("<span>unsafe</span>"), "{doc}");
    assert!(!doc.contains("javascript"), "{doc}");
    assert!(doc.contains("<br>"), "{doc}");
    // A list item is a block padded by its indent, its marker hung in the
    // gutter, as `renderMarkup` builds it (LLP 1045 D4).
    let list = document(
        "component App\n  view\n    text \"Top\\n\\n- one\\n  - two\" markup=\"markdown\"\n",
    );
    let marker =
        "display:inline-flex;justify-content:flex-end;width:40px;white-space:pre;text-indent:0;";
    assert!(
        list.contains(&format!("<span style=\"display:block;padding-left:40px;text-indent:-40px;\"><span style=\"opacity:0.62;{marker}\">• </span><span>one</span><span><br></span></span>")),
        "{list}"
    );
    assert!(
        list.contains(&format!("<span style=\"display:block;padding-left:80px;text-indent:-40px;\"><span style=\"opacity:0.62;{marker}\">◦ </span><span>two</span></span>")),
        "{list}"
    );
}

#[test]
fn nesting_the_parser_would_undo_is_refused() {
    let link_in_link = r#"
component App
  view
    link href="https://a.dev/" testId="outer"
      text "see "
        text "inner" href="https://b.dev/" testId="inner"
"#;
    let err = host(link_in_link, Says(""), "/").document().unwrap_err();
    assert_eq!(err.reason, "a link inside a link");
    let button_in_button = r#"
component App
  state n = 0
  action bump
    n = n + 1
  view
    button press=bump testId="outer"
      button press=bump testId="inner"
        text "inner"
"#;
    let host = host(button_in_button, Says(""), "/");
    let err = host.document().unwrap_err();
    assert_eq!(err.reason, "a button inside a button");
    let inner = host.runner().kernel().node(err.view).unwrap();
    assert_eq!(inner.props.str(exact_kernel::PropId::TestId), Some("inner"));
}

/// A `button` is a real `<button>` and holds only phrasing
/// content (LLP 1007 §1): its containers, paragraphs and headings are
/// `<span>`s with the same style — a block unless a row says otherwise — in
/// the live host's batch and in the document alike.
#[test]
fn a_buttons_containers_are_spans() {
    let src = r#"
component App
  state n = 0
  action bump
    n = n + 1
  view
    column
      button press=bump testId="card"
        column testId="stack"
          box width=10 height=10 testId="plain"
          text "Title" aria-level=2 testId="title"
          text "Body" testId="body"
        image "symbol:close" width=10 height=10 testId="icon"
      column testId="outside"
        text "Title" aria-level=2 testId="heading"
"#;
    let data = Says("");
    let plan = contract::bake(contract::compile(src).unwrap(), data.clone()).unwrap();
    let (host, first) = Host::boot(&plan.encode(), data, Default::default(), "/").unwrap();
    let doc = host.document().unwrap().root;
    let opening = |test_id: &str| {
        let at = doc.find(&format!("data-testid=\"{test_id}\"")).unwrap();
        let start = doc[..at].rfind('<').unwrap();
        doc[start..at + doc[at..].find('>').unwrap() + 1].to_owned()
    };
    let card = opening("card");
    assert!(card.starts_with("<button "), "{card}");
    // Its native role isn't restated (ARIA in HTML).
    assert!(!card.contains(" role="), "{card}");
    // Chrome's button (LLP 1001 §1): the reset's block, with the UA sheet's
    // centred text; the browser centres its content in its height.
    for want in [" type=\"button\"", "text-align:center;"] {
        assert!(card.contains(want), "{want}: {card}");
    }
    assert!(!card.contains("display:"), "{card}");
    let stack = opening("stack");
    assert!(stack.starts_with("<span "), "{stack}");
    assert!(
        !stack.contains("display:block"),
        "a row's display wins: {stack}"
    );
    assert!(opening("plain").contains("style=\"width:10px;height:10px;display:block;\""));
    assert!(
        opening("title").starts_with("<span "),
        "a heading in a button is a span"
    );
    assert!(opening("body").starts_with("<span data-exact-text"));
    assert!(opening("icon").starts_with("<img "));
    assert!(opening("outside").starts_with("<div "));
    assert!(opening("heading").starts_with("<h2 "));
    // The live host creates the same elements.
    let tag = |test_id: &str| {
        let at = first
            .find(&format!("\"data-testid\":\"{test_id}\""))
            .unwrap();
        let op = &first[first[..at].rfind("{\"op\":\"create\"").unwrap()..at];
        op.split("\"tag\":\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .to_owned()
    };
    for (test_id, want) in [
        ("card", "button"),
        ("stack", "span"),
        ("plain", "span"),
        ("title", "span"),
        ("body", "span"),
        ("icon", "img"),
        ("outside", "div"),
        ("heading", "h2"),
    ] {
        assert_eq!(tag(test_id), want, "{test_id}");
    }
}

#[test]
fn routes_other_than_the_selected_one_are_hidden_and_inert() {
    let src = r#"
routes nav
  tab home "/"
    post "/post/:post"
component App
  action back
    nav = back(nav)
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" width="100%" height="100%"
      each e in stack(nav) key=e.id
        column navigationKey=`${e.id}` testId=`route-${e.name}`
          text e.name
"#;
    let doc = host(src, Says(""), "/post/5").document().unwrap().root;
    let tag = |test_id: &str| {
        let at = doc.find(&format!("data-testid=\"{test_id}\"")).unwrap();
        let start = doc[..at].rfind('<').unwrap();
        doc[start..at + doc[at..].find('>').unwrap() + 1].to_owned()
    };
    let home = tag("route-home");
    assert!(home.contains(" inert "), "{home}");
    assert!(home.contains("visibility:hidden;"), "{home}");
    let post = tag("route-post");
    assert!(
        !post.contains("inert") && !post.contains("visibility"),
        "{post}"
    );
}

// @ref LLP 1075.003 §3.3, §3.7 — with tabs, each tab's panel holds its own
// stack: the unselected panel is hidden and inert, its stack keeps its top
// route; an app's data-* words are attributes of their own. Both are linked
// by use, and a host without them refuses the plan by name.
#[test]
fn tabs_show_their_own_stacks_and_data_words_are_attributes() {
    let src = r#"
routes nav
  tab home "/"
    post "/post/:post"
  tab second "/second"
component App
  action back
    nav = back(nav)
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" width="100%" height="100%"
      each t in nav.tabs key=t.name
        column role="tabpanel" id=`panel-${t.name}` testId=`panel-${t.name}`
          each e in t.stack key=e.id
            column navigationKey=`${e.id}` data-screen=e.name testId=`route-${e.name}`
              text e.name
      row role="tablist"
        button role="tab" aria-controls="panel-home" testId="tab-home"
          text "Home"
        button role="tab" aria-controls="panel-second" testId="tab-second"
          text "Second"
"#;
    let doc = host(src, Says(""), "/post/5").document().unwrap().root;
    let tag = |test_id: &str| {
        let at = doc.find(&format!("data-testid=\"{test_id}\"")).unwrap();
        let start = doc[..at].rfind('<').unwrap();
        doc[start..at + doc[at..].find('>').unwrap() + 1].to_owned()
    };
    let hidden = |t: &str| t.contains(" inert ") && t.contains("visibility:hidden;");
    let shown = |t: &str| !t.contains("inert") && !t.contains("visibility");
    let second = tag("panel-second");
    assert!(hidden(&second), "{second}");
    let home = tag("panel-home");
    assert!(shown(&home), "{home}");
    let root = tag("route-home");
    assert!(hidden(&root), "{root}");
    let post = tag("route-post");
    assert!(shown(&post), "{post}");
    assert!(post.contains("data-screen=\"post\""), "{post}");
    // The other tab's stack shows its top route inside its hidden panel.
    let other = tag("route-second");
    assert!(shown(&other), "{other}");

    let plan = contract::bake(contract::compile(src).unwrap(), Says("")).unwrap();
    exact_web::link(exact_web::Linked::CORE);
    let refused = Host::boot(&plan.encode(), Says(""), Default::default(), "/").map(|_| ());
    exact_web::link(exact_web_capabilities::ALL);
    let refused = format!("{:?}", refused.unwrap_err());
    assert!(
        refused.contains("dataset") && refused.contains("tabs"),
        "{refused}"
    );
}

#[test]
fn a_head_is_the_pages_head_never_an_element() {
    let src = r#"
component App
  state n = 0
  action bump
    n = n + 1
  view
    column testId="page"
      head title=`Count ${n}` description="Counting"
      button press=bump testId="bump" width=10 height=10
      when n > 0
        head title="Counted"
"#;
    let plan = contract::bake(contract::compile(src).unwrap(), Says("")).unwrap();
    let (mut host, first) = Host::boot(&plan.encode(), Says(""), Default::default(), "/").unwrap();
    assert!(
        first.contains(r#"{"op":"head","title":"Count 0","description":"Counting","image":null,"canonical":null,"robots":null,"status":null}"#),
        "{first}"
    );
    let kernel = host.runner().kernel();
    let page = kernel
        .node_by_key(kernel.find_by_test_id("page")[0])
        .unwrap();
    let head = page.children()[0];
    assert_eq!(
        kernel.node(head).unwrap().node_type,
        exact_kernel::NodeType::Head
    );
    // No element, and no place among its parent's children.
    assert!(
        !first.contains(&format!("\"op\":\"create\",\"id\":{head},")),
        "{first}"
    );
    let bump = page.children()[1];
    assert!(
        first.contains(&format!(
            "\"op\":\"children\",\"id\":{},\"ids\":[{bump}]",
            page.id
        )),
        "{first}"
    );
    let doc = host.document().unwrap();
    assert!(!doc.root.contains(&format!("data-view=\"{head}\"")));
    assert_eq!(doc.head, host.runner().head());
    assert_eq!(doc.head.title.as_deref(), Some("Count 0"));
    // A deeper head arrives: the page's head follows, field by field.
    let changed = host.dispatch(bump, exact_runner::Event::Press);
    assert!(
        changed.contains(r#"{"op":"head","title":"Counted","description":"Counting","image":null,"canonical":null,"robots":null,"status":null}"#),
        "{changed}"
    );
    // A commit that moves no head sends none.
    let plain = host.dispatch(bump, exact_runner::Event::Press);
    assert!(!plain.contains("\"op\":\"head\""), "{plain}");
    // A page without a head never sends one.
    let (_, none) = Host::boot(
        &contract::bake(
            contract::compile("component A\n  view\n    text \"a\"\n").unwrap(),
            Says(""),
        )
        .unwrap()
        .encode(),
        Says(""),
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(!none.contains("\"op\":\"head\""), "{none}");
}

#[test]
fn the_page_scroller_is_marked_in_the_document() {
    let doc = document(
        "component A\n  view\n    column height=\"100%\"\n      scroll document flex=1 min-height=0 testId=\"page\"\n        text \"a\"\n",
    );
    let at = doc.find("data-testid=\"page\"").unwrap();
    let open = &doc[doc[..at].rfind('<').unwrap()..at + doc[at..].find('>').unwrap()];
    // The document says so, for `<html data-scrolldocument>`, which the
    // shell's rule reads to make the page scroll.
    assert!(open.contains(" data-scrolldocument=\"true\""), "{open}");
    assert!(open.contains(" data-scroll=\"true\""), "{open}");
    // A bound marker that doesn't hold says so, and the rule doesn't match it.
    let doc = document(
        "component A\n  state signedOut = false\n  view\n    column height=\"100%\"\n      scroll document=signedOut flex=1 min-height=0 testId=\"page\"\n        text \"a\"\n",
    );
    assert!(doc.contains(" data-scrolldocument=\"false\""), "{doc}");
    let marked = |src: &str| host(src, Says(""), "/").document().unwrap().scroll_document;
    assert!(marked(
        "component A\n  view\n    scroll document height=100\n      text \"a\"\n"
    ));
    assert!(!marked(
        "component A\n  state on = false\n  view\n    scroll document=on height=100\n      text \"a\"\n"
    ));
}

/// LLP 1053.000.000.000 D3: `glassGroup="auto"` is written as `auto`, not its
/// reserved `-1`; a number as the number. Nothing draws it.
#[test]
fn an_auto_glass_group_is_written_as_auto() {
    let doc = document(
        "component App\n  view\n    column\n      row glassGroup=\"auto\" gap=8\n        box width=4 height=4\n      row glassGroup=12\n        box width=4 height=4\n",
    );
    assert!(doc.contains("glassGroup=\"auto\""), "{doc}");
    assert!(doc.contains("glassGroup=\"12\""), "{doc}");
    assert!(!doc.contains("glassGroup=\"-1\""), "{doc}");
}
