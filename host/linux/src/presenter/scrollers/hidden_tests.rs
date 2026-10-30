//! A scroller under a hidden route keeps its offset (Excord's Android picture dismiss, theme2 2026-10-28 07:39):
//! a full-screen route over a list hides the list's route, whose layout then has no travel, and the clamp put
//! the list at 0, so the list showed its first rows when the route came back. On Linux and Android alike.
use super::*;

/// A list of 100 rows of 20 px under a full-screen picture route; rows can arrive below or above while it shows.
const APP: &str = r#"routes nav
  home "/"
    photo "/photo"
component App
  state below = 0
  state above = 0
  state second = false
  state kept = 100
  action open
    nav = push(nav, "/photo")
  action close
    nav = back(nav)
  action arrive
    below = 5
  action prepend
    above = 50
  action replace
    second = true
  action trim
    kept = 20
  view
    main navigationKey=`${top(nav).id}` width="100%" height="100%"
      for e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationPresentation=(e.name == "photo" ? "fullscreen" : "push") position="absolute" inset=0
          if e.name == "home"
            button "Open" testId="open" press=open
            button "Arrive" testId="arrive" press=arrive
            button "Prepend" testId="prepend" press=prepend
            button "Replace" testId="replace" press=replace
            button "Trim" testId="trim" press=trim
            for k in [second ? "b" : "a"] key=k
              scroll testId="port" width=400 height=300
                for i in (above > 0 ? [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49] : []) key=i
                  box testId=`old${i}` width=400 height=20
                ROWS
                for i in (below > 0 ? [0, 1, 2, 3, 4] : []) key=i
                  box testId=`new${i}` width=400 height=20
          if e.name == "photo"
            button "Close" testId="close" press=close
"#;

fn boot() -> Presenter<()> {
    let rows: String = (0..100)
        .map(|r| {
            format!(
                "                if kept > {r}\n                  box testId=\"row{r}\" width=400 height=20\n"
            )
        })
        .collect();
    let source = APP.replace("                ROWS\n", &rows);
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(&source),
        (),
        (400., 600.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert_eq!(error, None);
    p.frame();
    p
}

fn named(p: &Presenter<()>, name: &str) -> ViewId {
    let kernel = p.host.kernel();
    kernel
        .node_by_key(kernel.find_by_test_id(name)[0])
        .unwrap()
        .id
}

fn press(p: &mut Presenter<()>, name: &str) {
    commit(p, name);
    attach(p);
}

/// A press and its commit, no frame painted yet.
fn commit(p: &mut Presenter<()>, name: &str) {
    let id = named(p, name);
    assert_eq!(p.host.dispatch_at(id, Event::Press, p.host.now()), None);
    assert_eq!(p.after_commit(), None);
}

/// A frame painted and acknowledged, as a host with a display (Android's) does after each commit.
fn attach(p: &mut Presenter<()>) {
    for _ in 0..3 {
        let Some(frame) = p.display_frame() else {
            return;
        };
        assert!(p.display_complete(&frame));
    }
}

fn row_y(p: &mut Presenter<()>, name: &str) -> f32 {
    let row = named(p, name);
    p.boxes().iter().find(|b| b.id == row).unwrap().rect.1
}

/// The list scrolled to row 40 at its top, then the picture opened over it: the list's route is hidden.
fn opened() -> (Presenter<()>, ViewId, f32) {
    let mut p = boot();
    let port = named(&p, "port");
    attach(&mut p);
    assert_eq!(p.scroll_to(port, (0., 800.)), Some((0., 800.)));
    attach(&mut p);
    let shown = row_y(&mut p, "row40");
    press(&mut p, "open");
    assert!(
        p.host.route_visibility(port).0,
        "the list's route is hidden under the picture"
    );
    (p, port, shown)
}

#[test]
fn a_list_under_a_full_screen_route_keeps_its_place() {
    let (mut p, port, shown) = opened();
    assert_eq!(p.scroll_of(port), (0., 800.), "hidden, it keeps its offset");
    press(&mut p, "close");
    assert_eq!(p.scroll_of(port), (0., 800.));
    assert_eq!(row_y(&mut p, "row40"), shown, "the same row where it was");
}

#[test]
fn rows_arriving_below_while_hidden_leave_the_place_alone() {
    let (mut p, port, shown) = opened();
    press(&mut p, "arrive");
    press(&mut p, "close");
    assert_eq!(p.scroll_of(port), (0., 800.));
    assert_eq!(row_y(&mut p, "row40"), shown);
}

#[test]
fn rows_prepended_while_hidden_keep_the_anchor_row_on_show() {
    // The re-clamp on show keeps the anchor row and its offset, never an end.
    let (mut p, port, shown) = opened();
    press(&mut p, "prepend");
    press(&mut p, "close");
    assert_eq!(row_y(&mut p, "row40"), shown, "row40 where it was");
    assert_eq!(
        p.scroll_of(port),
        (0., 1800.),
        "the offset grew by the 50 rows above"
    );
}

#[test]
fn a_hidden_list_rebuilt_under_the_same_route_is_clamped_afresh() {
    // A hidden route whose tree is destroyed and built again keeps no stale offset for the new list.
    // A runner never reuses a view id (`Ids::fresh` counts up; a reload drops every offset; a renewed row's
    // offset is dropped by `renew`), so a hidden tree built again is fresh ids: the old list's 800 must go with
    // its node and the new one start at its top.
    let (mut p, old, _) = opened();
    assert_eq!(p.scroll_of(old), (0., 800.));
    press(&mut p, "replace");
    let port = named(&p, "port");
    assert_ne!(port, old, "built again, the list is another node");
    assert!(
        !p.scroll.contains_key(&old),
        "no offset is kept for the destroyed list"
    );
    press(&mut p, "close");
    assert_eq!(p.scroll_of(port), (0., 0.), "a new list starts at its top");
}

#[test]
fn a_list_scrolled_and_covered_before_its_frame_keeps_its_place() {
    // The scroll's frame never painted before the picture covered it: its offset is still the
    // list's.
    let mut p = boot();
    attach(&mut p);
    let port = named(&p, "port");
    assert_eq!(p.scroll_to(port, (0., 800.)), Some((0., 800.)));
    press(&mut p, "open");
    assert_eq!(p.scroll_of(port), (0., 800.));
    press(&mut p, "close");
    assert_eq!(p.scroll_of(port), (0., 800.));
}

#[test]
fn a_list_wheeled_and_covered_before_its_frame_keeps_its_place() {
    // A wheel writes the offset itself (`scroll_container_by`), with no scroll handler and no frame before the
    // picture covers the list: the acknowledged picture painted this list at its id, so the offset is still
    // its own.
    let mut p = boot();
    attach(&mut p);
    let port = named(&p, "port");
    p.wheel(port, 0., 800.).unwrap();
    assert_eq!(p.scroll_of(port), (0., 800.));
    press(&mut p, "open");
    assert_eq!(p.scroll_of(port), (0., 800.), "kept under the picture");
    press(&mut p, "close");
    assert_eq!(p.scroll_of(port), (0., 800.));
}

#[test]
fn content_that_shrank_while_hidden_shows_in_range_from_the_first_frame() {
    // 800 held, then only 20 rows (400 px under a 300 px port) remain: the first frame that shows the list again
    // paints it in range, at its end, staged as the list's own clamp; its input offset stays 800 until that frame
    // is acknowledged, then 100. A list's own clamp keeps the reader's revision (ack_tests).
    let (mut p, port, _) = opened();
    press(&mut p, "trim");
    assert_eq!(p.scroll_of(port), (0., 800.), "hidden, nothing to clamp to");
    commit(&mut p, "close");
    assert_eq!(
        p.scroll_of(port),
        (0., 800.),
        "the input offset waits for the frame"
    );
    assert_eq!(
        p.collection_paint_scroll()
            .and_then(|m| m.get(&port).copied()),
        Some((0., 100.)),
        "the first frame paints in range"
    );
    let frame = p.display_frame().unwrap();
    assert_eq!(
        p.scroll_of(port),
        (0., 800.),
        "still, until its acknowledgement"
    );
    assert!(p.display_complete(&frame));
    assert_eq!(p.scroll_of(port), (0., 100.));
}

#[test]
fn a_hidden_id_reused_by_another_node_keeps_no_stale_offset() {
    // A wire id is unique only among live nodes: under a hidden route, an id whose node is not the one the
    // clamp last saw shown (its slot reused by another allocation) is clamped, not held.
    let (mut p, port, _) = opened();
    let seen = p.host.kernel().node(port).unwrap().key;
    let other = exact_kernel::NodeKey {
        index: seen.index,
        generation: seen.generation + 1,
    };
    p.collection.shown_keys.insert(port, other);
    p.clamp_scroll();
    assert_eq!(
        p.scroll_of(port),
        (0., 0.),
        "no travel in the picture: clamped to its top"
    );
}

/// `rows(first, count)`: `count` consecutive numbers from `first`.
struct Rows;
impl DataSource for Rows {
    fn query(
        &mut self,
        _: &str,
        args: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, exact_runner::DataError> {
        use exact_runner::Value;
        let [Value::Number(first), Value::Number(count)] = args else {
            return Ok(Value::list(Vec::new()));
        };
        Ok(Value::list(
            (0..*count as u32)
                .map(|n| Value::Number(first + f64::from(n)))
                .collect(),
        ))
    }
}

#[test]
fn a_virtualized_list_keeps_its_row_when_rows_arrive_while_hidden() {
    // Excord's transcript: a virtualized list (collection feedback and corrections) under the picture, rows
    // arriving below (the gateway) or a page above (older history) while it is hidden; on dismiss the same row
    // shows at the same place, from the first frame.
    let source = r#"routes nav
  home "/"
    photo "/photo"
component App
  state first = 1000
  state count = 200
  action open
    nav = push(nav, "/photo")
  action close
    nav = back(nav)
  action arrive
    count = count + 5
  action page
    first = 950
    count = count + 50
  resource rows = rows(first, count) as shape number[]
  view
    main navigationKey=`${top(nav).id}` width="100%" height="100%"
      for e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationPresentation=(e.name == "photo" ? "fullscreen" : "push") position="absolute" inset=0
          if e.name == "home"
            button "Open" testId="open" press=open
            button "Arrive" testId="arrive" press=arrive
            button "Page" testId="page" press=page
            list virtualized=true estimated-item-height=20 width=400 height=300 testId="port"
              for x in rows key=x
                box testId=`r${x}` width=400 height=24
          if e.name == "photo"
            button "Close" testId="close" press=close
"#;
    for change in ["arrive", "page"] {
        let (mut p, error) = Presenter::boot_with(
            &crate::test_plan(source),
            Rows,
            (400., 600.),
            1.,
            PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert_eq!(error, None);
        let settle = |p: &mut Presenter<Rows>| {
            for _ in 0..16 {
                assert!(p.pump(p.host.now()).is_none());
                if let Some(frame) = p.display_frame() {
                    assert!(p.display_complete(&frame));
                }
            }
        };
        settle(&mut p);
        let id = |p: &Presenter<Rows>, name: &str| {
            let kernel = p.host.kernel();
            kernel
                .node_by_key(kernel.find_by_test_id(name)[0])
                .unwrap()
                .id
        };
        let press = |p: &mut Presenter<Rows>, name: &str| {
            let target = id(p, name);
            assert_eq!(p.host.dispatch_at(target, Event::Press, p.host.now()), None);
            assert_eq!(p.after_commit(), None);
        };
        let port = id(&p, "port");
        p.scroll_to(port, (0., 2400.)).unwrap();
        settle(&mut p);
        let y = |p: &mut Presenter<Rows>, name: &str| {
            let row = id(p, name);
            p.boxes().iter().find(|b| b.id == row).unwrap().rect.1
        };
        let shown = y(&mut p, "r1110");
        press(&mut p, "open");
        settle(&mut p);
        press(&mut p, change);
        settle(&mut p);
        press(&mut p, "close");
        assert!(p.pump(p.host.now()).is_none());
        let frame = p.display_frame().unwrap();
        assert!(p.display_complete(&frame));
        assert_eq!(
            y(&mut p, "r1110"),
            shown,
            "{change}: the same row at the same place on the first frame"
        );
        settle(&mut p);
        assert_eq!(y(&mut p, "r1110"), shown, "{change}: and after");
    }
}

#[test]
fn a_hidden_route_destroyed_and_built_again_keeps_no_stale_offset() {
    // The route under the picture is popped and a new one pushed in its place (its tree built again, fresh
    // ids), the picture over it again: the old list's 800 leaves with its node, and the new list shows at its
    // top, bounded, with no tracking edited by hand.
    let rows: String = (0..100)
        .map(|r| format!("              box testId=\"row{r}\" width=400 height=20\n"))
        .collect();
    let source = r#"routes nav
  home "/"
    chat "/chat"
      photo "/chat/photo"
component App
  action enter
    nav = push(nav, "/chat")
  action open
    nav = push(nav, "/chat/photo")
  action rebuild
    nav = push(push(back(back(nav)), "/chat"), "/chat/photo")
  action close
    nav = back(nav)
  view
    main navigationKey=`${top(nav).id}` width="100%" height="100%"
      for e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationPresentation=(e.name == "photo" ? "fullscreen" : "push") position="absolute" inset=0
          if e.name == "home"
            button "Enter" testId="enter" press=enter
          if e.name == "chat"
            button "Open" testId="open" press=open
            scroll testId="port" width=400 height=300
ROWS
          if e.name == "photo"
            button "Rebuild" testId="rebuild" press=rebuild
            button "Close" testId="close" press=close
"#
    .replace("ROWS\n", &rows);
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(&source),
        (),
        (400., 600.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert_eq!(error, None);
    p.frame();
    attach(&mut p);
    press(&mut p, "enter");
    let old = named(&p, "port");
    assert_eq!(p.scroll_to(old, (0., 800.)), Some((0., 800.)));
    attach(&mut p);
    press(&mut p, "open");
    assert_eq!(p.scroll_of(old), (0., 800.), "kept under the picture");
    press(&mut p, "rebuild");
    let port = named(&p, "port");
    assert_ne!(port, old, "the route's tree was built again");
    assert!(p.host.route_visibility(port).0, "under the picture again");
    assert!(
        !p.scroll.contains_key(&old),
        "the destroyed list's offset left with it"
    );
    press(&mut p, "close");
    assert_eq!(p.scroll_of(port), (0., 0.), "the new list at its top");
}

/// 100 keyed rows of 20 px in a 300 px port under the picture route, any of them removable while hidden.
fn kept_source() -> String {
    let rows: String = (0..100)
        .map(|r| format!("              if lo <= {r} && {r} < hi\n                box testId=\"row{r}\" width=400 height=20\n"))
        .collect();
    r#"routes nav
  home "/"
    photo "/photo"
component App
  state lo = 0
  state hi = 100
  action open
    nav = push(nav, "/photo")
  action close
    nav = back(nav)
  action astra
    lo = 40
  action grok
    lo = 15
    hi = 60
  view
    main navigationKey=`${top(nav).id}` width="100%" height="100%"
      for e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationPresentation=(e.name == "photo" ? "fullscreen" : "push") position="absolute" inset=0
          if e.name == "home"
            button "Open" testId="open" press=open
            button "Astra" testId="astra" press=astra
            button "Grok" testId="grok" press=grok
            scroll testId="port" width=400 height=300
ROWS
          if e.name == "photo"
            button "Close" testId="close" press=close
"#
    .replace("ROWS\n", &rows)
}

fn boot_kept() -> Presenter<()> {
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(&kept_source()),
        (),
        (400., 600.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert_eq!(error, None);
    p.frame();
    attach(&mut p);
    p
}

#[test]
fn rows_removed_above_while_hidden_keep_the_anchor_row_over_a_stale_offset() {
    // At 1600 (row80 on top), the first 40 rows removed while
    // the picture covers the list; the anchor staged while hidden (800, row80 on top) wins over the stale input
    // offset 1600, whose clamp would be 900: the first frame shows row80 on top, not the end (theme2 10:42).
    let mut p = boot_kept();
    let port = named(&p, "port");
    assert_eq!(p.scroll_to(port, (0., 1600.)), Some((0., 1600.)));
    attach(&mut p);
    let top = p.boxes().iter().find(|b| b.id == port).unwrap().rect.1;
    press(&mut p, "open");
    press(&mut p, "astra");
    commit(&mut p, "close");
    assert_eq!(
        p.collection_paint_scroll()
            .and_then(|m| m.get(&port).copied()),
        Some((0., 800.)),
        "the first frame's offset"
    );
    let frame = p.display_frame().unwrap();
    assert!(p.display_complete(&frame));
    assert_eq!(p.scroll_of(port), (0., 800.));
    assert_eq!(row_y(&mut p, "row80"), top, "row80 on top");
}

#[test]
fn an_anchor_past_the_new_travel_is_clamped_not_the_stale_offset() {
    // At 800, 300 px removed above (anchor 500) and enough below that the travel is
    // 600: the anchor's 500 is in range and wins; the stale 800 clamped would be 600.
    let mut p = boot_kept();
    let port = named(&p, "port");
    assert_eq!(p.scroll_to(port, (0., 800.)), Some((0., 800.)));
    attach(&mut p);
    press(&mut p, "open");
    press(&mut p, "grok");
    commit(&mut p, "close");
    assert_eq!(
        p.collection_paint_scroll()
            .and_then(|m| m.get(&port).copied()),
        Some((0., 500.))
    );
    attach(&mut p);
    assert_eq!(p.scroll_of(port), (0., 500.));
}

#[test]
fn a_horizontal_scroller_shrunk_while_hidden_paints_in_range_from_the_first_frame() {
    // An ordinary horizontal scroller with no scrollLeft binding, at x 800, its content shrunk
    // to 400 px under a 300 px port while hidden: the staged clamp (x 100) survives the scheduling of ordinary
    // scrollers, and the first frame paints at x 100, not 800.
    let source = r#"routes nav
  home "/"
    photo "/photo"
component App
  state wide = 2000
  action open
    nav = push(nav, "/photo")
  action close
    nav = back(nav)
  action shrink
    wide = 400
  view
    main navigationKey=`${top(nav).id}` width="100%" height="100%"
      for e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationPresentation=(e.name == "photo" ? "fullscreen" : "push") position="absolute" inset=0
          if e.name == "home"
            button "Open" testId="open" press=open
            button "Shrink" testId="shrink" press=shrink
            scroll testId="port" width=300 height=100 overflow-x="scroll" overflow-y="hidden"
              box width=wide height=80
          if e.name == "photo"
            button "Close" testId="close" press=close
"#;
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (400., 600.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert_eq!(error, None);
    p.frame();
    attach(&mut p);
    let port = named(&p, "port");
    assert_eq!(p.scroll_to(port, (800., 0.)), Some((800., 0.)));
    attach(&mut p);
    press(&mut p, "open");
    press(&mut p, "shrink");
    commit(&mut p, "close");
    assert!(
        p.pump(p.host.now()).is_none(),
        "ordinary scrollers scheduled"
    );
    assert_eq!(
        p.collection_paint_scroll()
            .and_then(|m| m.get(&port).copied()),
        Some((100., 0.)),
        "the first frame's offset"
    );
    attach(&mut p);
    assert_eq!(p.scroll_of(port), (100., 0.));
}

/// An ordinary horizontal scroller (no scroll bindings) under a route the picture hides; `next` builds a fresh one.
const STAGED: &str = r#"routes nav
  home "/"
    photo "/photo"
component App
  state wide = 2000
  state shown = true
  state n = 0
  action open
    nav = push(nav, "/photo")
  action close
    nav = back(nav)
  action shrink
    wide = 400
  action remove
    shown = false
  action next
    n = n + 1
    wide = 2000
  view
    main navigationKey=`${top(nav).id}` width="100%" height="100%"
      for e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationPresentation=(e.name == "photo" ? "fullscreen" : "push") position="absolute" inset=0
          if e.name == "home"
            button "Open" testId="open" press=open
            button "Shrink" testId="shrink" press=shrink
            button "Remove" testId="remove" press=remove
            button "Next" testId="next" press=next
            if shown
              for k in [n] key=k
                scroll testId="port" width=300 height=100 overflow-x="scroll" overflow-y="hidden"
                  box width=wide height=80
          if e.name == "photo"
            button "Close" testId="close" press=close
"#;

fn boot_staged() -> Presenter<()> {
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(STAGED),
        (),
        (400., 600.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert_eq!(error, None);
    p.frame();
    attach(&mut p);
    p
}

/// The scroller at x 800, shrunk to 400 px under the picture, and shown again: the clamp (x 100) is staged.
fn stage_clamp(p: &mut Presenter<()>) -> ViewId {
    let port = named(p, "port");
    assert_eq!(p.scroll_to(port, (800., 0.)), Some((800., 0.)));
    attach(p);
    press(p, "open");
    press(p, "shrink");
    commit(p, "close");
    assert!(p.pump(p.host.now()).is_none());
    assert_eq!(p.cursors_left(port), (true, 1), "the clamp staged");
    port
}

#[test]
fn a_staged_clamp_leaves_no_cursor_after_its_acknowledgement() {
    // The cursor made only to carry the clamp must not outlive its acknowledgement or its node: ordinary
    // scrollers are kept, and no binding pass sweeps them.
    let mut p = boot_staged();
    let port = stage_clamp(&mut p);
    attach(&mut p);
    assert_eq!(p.scroll_of(port), (100., 0.));
    assert_eq!(
        p.cursors_left(port),
        (false, 0),
        "retired by its acknowledgement"
    );
    press(&mut p, "remove");
    assert_eq!(
        p.cursors_left(port),
        (false, 0),
        "nothing left of the scroller"
    );
}

#[test]
fn fresh_scrollers_shown_and_hidden_leave_no_staging_cursors() {
    // Each scroller is destroyed for a fresh one, either after its clamp is painted or before.
    let mut p = boot_staged();
    let mut ports = Vec::new();
    for round in 0..6 {
        let port = stage_clamp(&mut p);
        if round % 2 == 0 {
            attach(&mut p);
        }
        press(&mut p, "next");
        assert_ne!(named(&p, "port"), port, "a fresh scroller");
        ports.push(port);
    }
    for port in ports {
        assert!(
            !p.cursors_left(port).0,
            "{port:?}'s cursor dropped with its node"
        );
    }
    assert_eq!(
        p.cursors_left(named(&p, "port")).1,
        0,
        "no staging cursors left"
    );
}
