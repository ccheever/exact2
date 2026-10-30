//! The runner's `scrollToFragment` on Linux (LLP 1153 D5): the element with
//! that id in the visit's own route, though the route beneath holds one too,
//! at the start of its scroller; no such element, the route's top and a line
//! in the journal; a wheel the person turned first, no scroll; and a link to
//! a fragment of the visit shown.
use super::*;

/// A list route that holds an `id="moves"` of its own, beneath a detail with
/// a link to its `#moves` and a scroller of ten 50 px rows, the `moves` box,
/// then ten more.
fn boot() -> Presenter<()> {
    let rows = |from: usize| -> String {
        (from..from + 10)
            .map(|i| format!("                box height=50 testId=\"row-{i}\"\n"))
            .collect()
    };
    let source = format!(
        r##"routes nav
  home "/"
    detail "/detail"
component App
  action open(url: string)
    nav = push(nav, url)
  action follow(url: string)
    nav = go(nav, url)
  action back
    nav = back(nav)
  view
    main navigationKey=`${{top(nav).id}}` navigationBack="back" navigate=follow width="100%" height="100%"
      for e in stack(nav) key=e.id
        column navigationKey=`${{e.id}}` position="absolute" inset=0
          if e.name == "home"
            button press=open("/detail#moves") testId="moves"
              text "Moves"
            button press=open("/detail#nothing") testId="nothing"
              text "Nothing"
            box id="moves" height=50
          if e.name == "detail"
            link href="#moves" testId="to-moves"
              text "Moves"
            scroll testId="port" height=100
              column
{}                box id="moves" height=50
{}"##,
        rows(0),
        rows(10)
    );
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(&source),
        (),
        (400., 800.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    settle(&mut p);
    p
}

fn settle(p: &mut Presenter<()>) {
    for _ in 0..4 {
        assert!(p.pump(p.host.now()).is_none());
        if p.dirty() {
            let _ = p.frame();
        }
    }
}

fn id(p: &Presenter<()>, name: &str) -> ViewId {
    let k = p.host().kernel();
    let key = k.find_by_test_id(name).first().copied();
    let node = key.and_then(|key| k.node_by_key(key));
    node.unwrap_or_else(|| panic!("no node {name}")).id
}

/// Press `link` on the list, then run what the commits asked of the host.
fn open(p: &mut Presenter<()>, link: &str, wheel_first: bool) -> ViewId {
    p.tap(id(p, link)).unwrap();
    settle(p);
    let port = id(p, "port");
    if wheel_first {
        p.wheel(port, 0., 30.).unwrap();
    }
    p.run_commands(|| ());
    settle(p);
    port
}

#[test]
fn a_fragment_scrolls_its_own_routes_element_to_the_start() {
    let mut p = boot();
    let port = open(&mut p, "moves", false);
    assert_eq!(p.scroll_of(port).1, 500., "ten 50 px rows above it");
}

#[test]
fn no_such_element_says_so_and_leaves_the_route_at_its_top() {
    let mut p = boot();
    let port = open(&mut p, "nothing", false);
    assert_eq!(p.scroll_of(port).1, 0.);
    let journal = crate::agent::handle(&mut p, r#"{"op":"logs"}"#);
    assert!(
        journal.contains("fragment #nothing: no element"),
        "{journal}"
    );
}

#[test]
fn a_link_to_a_fragment_is_the_visit_shown_at_that_fragment() {
    let mut p = boot();
    let port = open(&mut p, "nothing", false);
    let visit = p.host().router_op().unwrap().top;
    p.tap(id(&p, "to-moves")).unwrap();
    settle(&mut p);
    let op = p.host().router_op().unwrap();
    assert_eq!(
        (op.top, op.fragment.as_str()),
        (visit, "moves"),
        "the same visit"
    );
    p.run_commands(|| ());
    settle(&mut p);
    assert_eq!(p.scroll_of(port).1, 500.);
}

#[test]
fn a_wheel_before_the_scroll_cancels_it() {
    let mut p = boot();
    let port = open(&mut p, "moves", true);
    assert_eq!(p.scroll_of(port).1, 30., "where the wheel left it");
    let journal = crate::agent::handle(&mut p, r#"{"op":"logs"}"#);
    assert!(journal.contains("the person scrolled first"), "{journal}");
}
