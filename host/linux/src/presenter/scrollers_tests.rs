//! The boundary with a shell that scrolls by itself (LLP 1115.001 D7.3):
//! which scrollers are under a point, what the shell is told of each, a
//! touch a scroll won pressing nothing, and the app's own scroll moving a
//! scroller's revision on so the shell's fling ends.
use super::*;
use exact_runner::Value;

fn boot(source: &str) -> Presenter<()> {
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (400., 400.),
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
    for _ in 0..8 {
        assert!(p.pump(p.host.now()).is_none());
        if p.dirty() {
            let _ = p.frame();
        }
    }
}

fn id(p: &Presenter<()>, name: &str) -> ViewId {
    let k = p.host().kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}

fn log(p: &Presenter<()>) -> String {
    let k = p.host().kernel();
    let node = k.node_by_key(k.find_by_test_id("log")[0]).unwrap();
    node.props.str(PropId::Text).unwrap_or_default().to_string()
}

/// A page that scrolls, 1,400 tall in a 400 viewport: a pressable row, a
/// strip that scrolls sideways, a scroller of its own nested in the page,
/// and a button that scrolls the page to 300.
const PAGE_OF_SCROLLERS: &str = r#"component App
  state pressed = 0
  state top = 0
  action press
    pressed = pressed + 1
  action jump
    top = top + 300
  view
    column width=400 height=400
      scroll testId="page" width=400 height=380 scrollTop=top
        button press=press appearance="none" testId="row" width=400 height=100
          text "A row"
        scroll testId="strip" width=400 height=80 overflow-x="scroll" overflow-y="hidden"
          box width=1000 height=80
        scroll testId="inner" width=400 height=120
          box width=400 height=420
        button press=jump appearance="none" testId="jump" width=400 height=100
          text "Jump"
        box width=400 height=1000
      text `${pressed}` testId="log" height=20
"#;

/// What a statement says of scroller `id`: its flags, its revision, where
/// it is, where its rows were painted, and its farthest offset.
fn told(words: &[i32], id: ViewId) -> (i32, i32, [f32; 6]) {
    let s = words
        .chunks(9)
        .find(|s| s[0] == id as i32)
        .unwrap_or_else(|| panic!("scroller {id} is not in {words:?}"));
    (
        s[1],
        s[2],
        std::array::from_fn(|i| f32::from_bits(s[3 + i] as u32)),
    )
}

#[test]
fn the_scrollers_under_a_point_are_told_innermost_first() {
    let mut p = boot(PAGE_OF_SCROLLERS);
    let (page, strip, inner) = (id(&p, "page"), id(&p, "strip"), id(&p, "inner"));
    assert_ne!(page, PAGE, "no node is the page's id");
    // Over the row: the page's scroller alone, up and down, 1,400 in 380.
    let under = p.scrollers_at(200., 50.);
    assert_eq!(under.claims, (false, false));
    assert_eq!(
        under.chain,
        vec![Scroller {
            id: page,
            axes: (false, true),
            max: (0., 1020.),
            offset: (0., 0.),
            port: [0., 0., 400., 380.],
            node: false,
        }]
    );
    // Over the strip: it first, sideways, then the page.
    let under = p.scrollers_at(200., 140.);
    let ids: Vec<_> = under.chain.iter().map(|s| (s.id, s.axes)).collect();
    assert_eq!(ids, [(strip, (true, false)), (page, (false, true))]);
    assert_eq!(under.chain[0].max, (600., 0.));
    assert_eq!(under.chain[0].port, [0., 100., 400., 80.]);
    // Over the nested scroller: it, then the page it hands off to.
    let under = p.scrollers_at(200., 240.);
    let ids: Vec<_> = under.chain.iter().map(|s| (s.id, s.axes)).collect();
    assert_eq!(ids, [(inner, (false, true)), (page, (false, true))]);
    assert_eq!(under.chain[0].max, (0., 300.));
    assert_eq!(under.chain[0].port, [0., 180., 400., 120.]);
    // Outside every scroller, and outside the viewport: none.
    assert!(p.scrollers_at(200., 390.).chain.is_empty());
    assert!(p.scrollers_at(200., 900.).chain.is_empty());
    // A scroller's port follows the page: where it shows now, clipped.
    assert_eq!(p.scroll_to(page, (0., 150.)), Some((0., 150.)));
    let under = p.scrollers_at(200., 60.);
    assert_eq!(under.chain[0].id, inner);
    assert_eq!(under.chain[0].port, [0., 30., 400., 120.]);
    assert_eq!(under.chain[1].offset, (0., 150.));
}

#[test]
fn a_contained_scroller_ends_the_chain_on_its_axis() {
    let source = PAGE_OF_SCROLLERS.replace(
        r#"testId="inner" width=400"#,
        r#"testId="inner" overscroll-behavior="contain" width=400"#,
    );
    let mut p = boot(&source);
    let under = p.scrollers_at(200., 240.);
    let ids: Vec<_> = under.chain.iter().map(|s| s.id).collect();
    assert_eq!(ids, [id(&p, "inner")], "nothing past it takes the drag");
}

#[test]
fn a_page_taller_than_the_viewport_is_the_last_scroller() {
    let mut p = boot(
        r#"component App
  view
    column width=400
      box width=400 height=900 testId="tall"
"#,
    );
    let under = p.scrollers_at(200., 200.);
    assert_eq!(under.chain.len(), 1);
    let page = under.chain[0];
    assert_eq!(
        (page.id, page.axes, page.max),
        (PAGE, (false, true), (0., 500.))
    );
    assert_eq!(p.scroll_to(PAGE, (0., 9000.)), Some((0., 500.)));
    assert_eq!(p.scrollers_at(200., 200.).chain[0].offset, (0., 500.));
}

#[test]
fn the_reader_moves_a_scroller_and_is_told_where_it_is() {
    let mut p = boot(PAGE_OF_SCROLLERS);
    let (page, inner) = (id(&p, "page"), id(&p, "inner"));
    // () is told before a touch asks.
    assert_eq!(p.reader_scrollers(&[], 2.), None);
    assert_eq!(p.reader_scrolled(page, (0., 10.), 0), None, "never told");
    // The touch's answer, in the reader's pixels (two a point).
    let words = p.reader_under(400., 480., 2.);
    assert_eq!(
        &words[..3],
        &[0, 2, 0],
        "no claims, two scrollers, no hosted field"
    );
    let first = &words[3..14];
    assert_eq!(
        &first[..3],
        &[inner as i32, 2, 0],
        "up and down, revision 0"
    );
    let px: Vec<f32> = first[3..]
        .iter()
        .map(|w| f32::from_bits(*w as u32))
        .collect();
    assert_eq!(px, [0., 0., 0., 600., 0., 360., 800., 240.]);
    assert_eq!(words[14], page as i32);
    // The first statement is whole: both scrollers, where they are.
    let all = p.reader_scrollers(&[(page, (0., 0.))], 2.).unwrap();
    assert_eq!(told(&all, page), (2 | 4, 0, [0., 0., 0., 0., 0., 2040.]));
    assert_eq!(told(&all, inner), (2, 0, [0., 0., 0., 0., 0., 600.]));
    assert_eq!(
        p.reader_scrollers(&[(page, (0., 0.))], 2.),
        None,
        "nothing new"
    );
    // The reader moves the nested one: clamped at its end, the rest is the page's.
    assert_eq!(p.reader_scrolled(inner, (0., 250.), 0), Some((0., 250.)));
    assert_eq!(p.reader_scrolled(inner, (0., 250.), 0), Some((0., 50.)));
    assert_eq!(p.reader_scrolled(page, (0., 200.), 0), Some((0., 200.)));
    assert_eq!(
        p.reader_scrolled(page, (30., 0.), 0),
        Some((0., 0.)),
        "not its axis"
    );
    // Told back with the same revisions: these were the reader's own moves.
    // The page's rows were painted at 0 and are 400 px from there now.
    let all = p.reader_scrollers(&[(page, (0., 0.))], 2.).unwrap();
    assert_eq!(told(&all, inner), (2, 0, [0., 600., 0., 600., 0., 600.]));
    assert_eq!(told(&all, page), (2 | 4, 0, [0., 400., 0., 0., 0., 2040.]));
}

#[test]
fn a_touch_a_scroll_won_presses_nothing_and_a_tap_after_it_still_does() {
    let mut p = boot(PAGE_OF_SCROLLERS);
    p.set_pointer_kind(PointerKind::Touch);
    let page = id(&p, "page");
    // A finger lands on the row, and the reader asks what is under it.
    p.pointer_down(200., 50., 1.).unwrap();
    assert!(p.contact_position().is_some());
    p.reader_under(200., 50., 1.);
    p.pointer_move(200., 47., 2.).unwrap();
    // The drag passes the system's slop in the reader, on an axis the page
    // scrolls along: the scroll wins the touch, which is cancelled here.
    p.pointer_cancel(3.).unwrap();
    assert!(p.contact_position().is_none());
    assert_eq!(p.reader_scrolled(page, (0., 40.), 0), Some((0., 40.)));
    settle(&mut p);
    assert_eq!(log(&p), "0", "no press for a drag that became a scroll");
    // The lift is the reader's too: nothing reaches the presenter for it.
    // A tap on the row where it now shows presses it.
    p.pointer_down(200., 30., 10.).unwrap();
    p.pointer_up(200., 30., 11.).unwrap();
    settle(&mut p);
    assert_eq!(log(&p), "1");
}

#[test]
fn a_scroll_the_app_makes_moves_the_revision_on_and_drops_a_stale_move() {
    let mut p = boot(PAGE_OF_SCROLLERS);
    let page = id(&p, "page");
    p.reader_under(200., 50., 1.);
    assert_eq!(p.reader_scrolled(page, (0., 120.), 0), Some((0., 120.)));
    let all = p.reader_scrollers(&[], 1.).unwrap();
    assert_eq!(told(&all, page).1, 0);
    // The app scrolls the page while the reader's fling runs.
    p.tap(id(&p, "jump")).unwrap();
    settle(&mut p);
    assert_eq!(p.scroll_of(page), (0., 300.));
    // A move the reader sent before it knew is dropped: the app's offset stands.
    assert_eq!(p.reader_scrolled(page, (0., 60.), 0), None);
    assert_eq!(p.scroll_of(page), (0., 300.));
    // The reader is told the offset under a new revision, and goes on from it.
    let all = p.reader_scrollers(&[], 1.).unwrap();
    let (_, rev, at) = told(&all, page);
    assert_eq!((rev, at[1]), (1, 300.));
    assert_eq!(p.reader_scrolled(page, (0., 60.), 1), Some((0., 60.)));
    assert_eq!(told(&p.reader_scrollers(&[], 1.).unwrap(), page).1, 1);
    // A list's own correction of its offset is no such move: the content
    // stays where it shows, and the revision with it.
    p.scroll.insert(page, (0., 400.));
    p.reader.rebased(page, (0., 360.), (0., 400.));
    let (_, rev, at) = told(&p.reader_scrollers(&[], 1.).unwrap(), page);
    assert_eq!((rev, at[1]), (1, 400.));
    // A driver from outside takes a scroller over: nothing more is told of it.
    // The reader is told so once, by a statement the scroller is not in.
    p.reader_forgets(page);
    assert_eq!(p.reader_scrolled(page, (0., 10.), 1), None);
    assert_eq!(p.reader_scrollers(&[], 1.), Some(Vec::new()));
    assert_eq!(p.reader_scrollers(&[], 1.), None);
}

#[test]
fn the_presenters_own_gestures_claim_their_axes() {
    let claims = |row: &str, x: f32, y: f32| {
        let source = format!(
            r#"component App
  state n = 0
  action hear
    n = n + 1
  action drag(dx: number, dy: number)
    n = n + 1
  view
    scroll width=400 height=400
      {row}
      box width=400 height=1000
"#
        );
        boot(&source).scrollers_at(x, y).claims
    };
    // A swipe row takes a sideways drag; up and down is the scroller's.
    let swipe = r#"box swiperight=hear touch-action="pan-y" width=400 height=100"#;
    assert_eq!(claims(swipe, 200., 50.), (true, false));
    assert_eq!(claims(swipe, 200., 150.), (false, false), "below the row");
    // A `pan` hears what its `touch-action` keeps from scrolling.
    let pan =
        |action: &str| format!(r#"box pan=drag touch-action="{action}" width=400 height=100"#);
    assert_eq!(claims(&pan("none"), 200., 50.), (true, true));
    assert_eq!(claims(&pan("pan-y"), 200., 50.), (true, false));
    assert_eq!(claims(&pan("pan-x"), 200., 50.), (false, true));
    assert_eq!(claims(&pan("auto"), 200., 50.), (false, false));
}

/// The fixture a phone is driven with (`apps/unsaid`, its page to scroll):
/// what the reader is told there is what the page was written to show.
#[test]
fn the_fixtures_list_its_strip_and_its_nested_scroller_are_told() {
    let mut p = boot(include_str!("../../../../apps/unsaid/app.contract"));
    p.tap(id(&p, "open-rows")).unwrap();
    settle(&mut p);
    let (rows, strip, nested) = (id(&p, "rows"), id(&p, "strip"), id(&p, "nested"));
    let middle = |p: &mut Presenter<()>, id: ViewId| {
        let b = p.box_of(id).unwrap();
        (b.rect.0 + b.rect.2 / 2., b.rect.1 + b.rect.3 / 2.)
    };
    // The strip scrolls sideways and the list it is a row of up and down.
    let (x, y) = middle(&mut p, strip);
    let under = p.scrollers_at(x, y);
    let chain: Vec<_> = under.chain.iter().map(|s| (s.id, s.axes)).collect();
    assert_eq!(chain, [(strip, (true, false)), (rows, (false, true))]);
    // 20 cards of 140 and 19 gaps of 8, in a list 368 wide.
    assert_eq!(under.chain[0].max.0, 20. * 140. + 19. * 8. - 368.);
    // The nested scroller hands off to the list.
    let (x, y) = middle(&mut p, nested);
    let under = p.scrollers_at(x, y);
    let chain: Vec<_> = under.chain.iter().map(|s| (s.id, s.axes)).collect();
    assert_eq!(chain, [(nested, (false, true)), (rows, (false, true))]);
    // 240 rows: the strip, the nested scroller, and 238 of 56.
    let list = under.chain[1];
    let port = p.box_of(rows).unwrap().rect.3;
    assert_eq!(list.max.1, 120. + 200. + 238. * 56. - port);
    // The button scrolls the list to row 150, and a row pressed there says so.
    p.reader_under(x, y, 1.);
    p.tap(id(&p, "jump")).unwrap();
    settle(&mut p);
    assert_eq!(p.scroll_of(rows), (0., 320. + 148. * 56.));
    let all = p.reader_scrollers(&[], 1.).unwrap();
    assert_eq!(told(&all, rows).1, 1, "the app moved it");
    p.tap(id(&p, "row-150")).unwrap();
    settle(&mut p);
    let k = p.host().kernel();
    let picked = k.node_by_key(k.find_by_test_id("picked")[0]).unwrap();
    assert_eq!(picked.props.str(PropId::Text), Some("Pressed row 150"));
}

#[test]
fn native_intersect_mount_and_scroll_without_a_scroll_handler() {
    let mut p = boot(include_str!(
        "../../../../scripts/fixtures/intersect.contract"
    ));
    let facts = |p: &Presenter<()>| {
        p.host
            .runner()
            .slot("count")
            .and_then(Value::as_number)
            .unwrap()
    };
    let root = id(&p, "port");
    assert_eq!(facts(&p), 1.0, "initial offscreen entry");
    assert_eq!(
        p.host.runner().slot("stickyCount"),
        Some(&Value::Number(1.0))
    );
    assert_eq!(
        p.host.runner().slot("stickyInside"),
        Some(&Value::Bool(true))
    );
    assert_eq!(
        p.host.runner().slot("stickyRatio"),
        Some(&Value::Number(1.0))
    );
    let sticky = id(&p, "sticky-port");
    p.scroll_to(sticky, (0.0, 201.0));
    settle(&mut p);
    assert_eq!(
        p.host.runner().slot("stickyCount"),
        Some(&Value::Number(2.0))
    );
    assert_eq!(
        p.host.runner().slot("stickyInside"),
        Some(&Value::Bool(false))
    );
    p.scroll_to(sticky, (0.0, 60.0));
    settle(&mut p);
    assert_eq!(
        p.host.runner().slot("stickyCount"),
        Some(&Value::Number(3.0))
    );
    assert_eq!(
        p.host.runner().slot("stickyInside"),
        Some(&Value::Bool(true))
    );
    assert_eq!(p.host.runner().slot("inside"), Some(&Value::Bool(false)));
    assert_eq!(p.scroll_to(root, (0.0, 60.0)), Some((0.0, 60.0)));
    settle(&mut p);
    assert_eq!(facts(&p), 2.0);
    assert_eq!(p.host.runner().slot("inside"), Some(&Value::Bool(true)));
    assert_eq!(
        p.host.runner().slot("ratio").and_then(Value::as_number),
        Some(0.6)
    );
    p.scroll_to(root, (0.0, 100.0));
    settle(&mut p);
    assert_eq!(facts(&p), 2.0, "ratio-only changes are silent");
    p.scroll_to(root, (0.0, 211.0));
    settle(&mut p);
    assert_eq!(facts(&p), 3.0);
    assert_eq!(p.host.runner().slot("inside"), Some(&Value::Bool(false)));
}

#[test]
fn native_intersect_samples_after_resize_handlers() {
    let p = boot(
        r#"component App
  state gap = 80
  state count = 0
  state ratio = 0
  action resized(width: number, height: number)
    gap = 0
  action seen(e: IntersectionObserverEntry)
    count = count + 1
    ratio = e.intersectionRatio
  view
    column width=200 height=100 overflow="hidden"
      box height=gap flex-shrink=0 resize=resized
      box width=100 height=50 flex-shrink=0 intersect=seen
"#,
    );
    assert_eq!(p.host.runner().slot("count"), Some(&Value::Number(1.0)));
    assert_eq!(p.host.runner().slot("ratio"), Some(&Value::Number(1.0)));
}
