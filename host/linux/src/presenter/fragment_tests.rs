//! The runner's `scrollToFragment` on Linux (LLP 1153 D5): the element with
//! that id in the visit's own route, though the route beneath holds one too,
//! at the start of its scroller; no such element, the route's top and a line
//! in the journal; a scroll by the person first (a wheel, a key's or a
//! scrollbar's `scroll_to`), no scroll; the next Tab from the target; a link
//! to a fragment of the visit shown, again after the person scrolled; and an
//! author's `scrollIntoView` over the fragment's hold.
use super::*;

/// A list route that holds an `id="moves"` of its own, beneath a detail with
/// a link to its `#moves`, buttons that commit or scroll, and a scroller of
/// ten 50 px rows, the `moves` box, a button, then ten more.
fn boot() -> Presenter<()> {
    let rows = |from: usize| -> String {
        (from..from + 10)
            .map(|i| format!("                box height=50 id=\"row-{i}\"\n"))
            .collect()
    };
    let source = format!(
        r##"routes nav
  home "/"
    detail "/detail"
component App
  state count = 0
  action bump
    count = count + 1
  action aim
    scrollIntoView("row-0", block="start")
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
            button press=bump tabindex=1 testId="bump"
              text "Bump"
            button press=aim testId="aim"
              text "Aim"
            scroll testId="port" height=100
              column
{}                box id="moves" testId="target" height=50
                button tabindex=2 testId="after"
                  text "After"
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
fn open(
    p: &mut Presenter<()>,
    link: &str,
    first: Option<fn(&mut Presenter<()>, ViewId)>,
) -> ViewId {
    p.tap(id(p, link)).unwrap();
    settle(p);
    let port = id(p, "port");
    if let Some(scroll) = first {
        scroll(p, port);
    }
    p.run_commands(|| ());
    settle(p);
    port
}

#[test]
fn a_fragment_scrolls_its_own_routes_element_to_the_start() {
    let mut p = boot();
    let port = open(&mut p, "moves", None);
    assert_eq!(p.scroll_of(port).1, 500., "ten 50 px rows above it");
    // The next Tab starts from the target, not from the list's button: the
    // stop after it in tree order, though a lower tabindex comes first in
    // the order (HTML's sequential navigation search algorithm).
    p.move_focus(false, 1.);
    assert_eq!(p.focus, Some(id(&p, "after")));
    let _ = open(&mut p, "to-moves", None);
    p.move_focus(true, 2.);
    assert_eq!(
        p.focus,
        Some(id(&p, "aim")),
        "Shift-Tab: the stop before it"
    );
}

#[test]
fn a_link_to_the_fragment_shown_scrolls_again_after_the_person_scrolled() {
    let mut p = boot();
    let port = open(&mut p, "moves", None);
    let _ = open(&mut p, "bump", None);
    p.wheel(port, 0., -100.).unwrap();
    settle(&mut p);
    assert_eq!(p.scroll_of(port).1, 400.);
    let _ = open(&mut p, "to-moves", None);
    assert_eq!(p.scroll_of(port).1, 500., "a new activation, armed now");
}

#[test]
fn the_authors_scroll_into_view_ends_the_fragments_hold() {
    let mut p = boot();
    let port = open(&mut p, "moves", None);
    // As while the route's pictures load.
    p.fragment_anchor = Some((id(&p, "target"), p.person_scrolls));
    let _ = open(&mut p, "aim", None);
    assert!(!p.hold_fragment(), "nothing held");
    assert_eq!(p.scroll_of(port).1, 0., "where the author put it");
}

#[test]
fn no_such_element_says_so_and_leaves_the_route_at_its_top() {
    let mut p = boot();
    let port = open(&mut p, "nothing", None);
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
    let port = open(&mut p, "nothing", None);
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
fn a_scroll_by_the_person_before_the_scroll_cancels_it() {
    let wheel: fn(&mut Presenter<()>, ViewId) = |p, port| {
        p.wheel(port, 0., 30.).unwrap();
    };
    let key: fn(&mut Presenter<()>, ViewId) = |p, port| {
        p.scroll_to(port, (0., 30.)).unwrap();
    };
    for first in [wheel, key] {
        let mut p = boot();
        let port = open(&mut p, "moves", Some(first));
        assert_eq!(p.scroll_of(port).1, 30., "where the person left it");
        let journal = crate::agent::handle(&mut p, r#"{"op":"logs"}"#);
        assert!(journal.contains("the person scrolled first"), "{journal}");
    }
}
