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
    Host::boot(&plan.encode(), data, Default::default(), launch)
        .unwrap()
        .0
}

fn document(src: &str) -> String {
    host(src, Says(""), "/").document().unwrap().root
}

fn caltrain() -> (Host<caltrain_data::Caltrain>, String) {
    let plan = caltrain::build().unwrap();
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
  action bump writes n
    n = n + 1
  view
    column inert=true
      input value="hi" disabled=true readonly=true testId="field" width=10
      input value="" disabled=false testId="enabled" width=10
      textarea value="\nfirst" testId="area" width=10
      view focus=bump testId="focusable" width=10 height=10
      button focus=bump testId="button" width=10 height=10
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
    let doc = document(src);
    assert!(
        doc.contains("<span style=\"font-weight:700;\">b</span>"),
        "{doc}"
    );
    assert!(doc.contains("<a href=\"https://e.dev/\">safe</a>"), "{doc}");
    assert!(doc.contains("<span>unsafe</span>"), "{doc}");
    assert!(!doc.contains("javascript"), "{doc}");
    assert!(doc.contains("<br>"), "{doc}");
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
  action bump writes n
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

#[test]
fn routes_other_than_the_selected_one_are_hidden_and_inert() {
    let src = r#"
routes nav
  tab home "/"
    post "/post/:post"
component App
  action back writes nav
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
