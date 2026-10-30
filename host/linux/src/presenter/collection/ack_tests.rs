//! A list's own position acknowledged under a reader's fling (Excord's Android fling, theme2 2026-10-28 07:39):
//! the acknowledgement never moves the reader's revision on, so the reader's fling goes on and its moves stand;
//! an app's position still lands where asked and does move it on.
use super::*;

/// A scroll port of 50 rows of 20 px under a header the app can fold, 50 more rows it can put above (the
/// 50-row page an older-history load prepends), and an app position (`scrollTop`) it can set.
fn source() -> String {
    let mut s = String::from(
        "component App\n  state header = 48\n  state above = 0\n  state top = 0\n  action fold\n    header = 0\n  action prepend\n    above = 50\n  action jump\n    top = 100\n  view\n    column\n      button \"Fold\" press=fold testId=\"fold\"\n      button \"Prepend\" press=prepend testId=\"prepend\"\n      button \"Jump\" press=jump testId=\"jump\"\n      scroll testId=\"port\" width=400 height=400 scrollTop=top\n        box height=header\n",
    );
    for row in 0..50 {
        s.push_str(&format!(
            "        if above > 0\n          box testId=\"old{row}\" width=400 height=20\n"
        ));
    }
    for row in 0..50 {
        s.push_str(&format!(
            "        box testId=\"row{row}\" width=400 height=20\n"
        ));
    }
    s
}

fn boot() -> Presenter<()> {
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(&source()),
        (),
        (400., 460.),
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
    let id = named(p, name);
    assert_eq!(p.host.dispatch_at(id, Event::Press, p.host.now()), None);
    assert_eq!(p.after_commit(), None);
}

fn attach(p: &mut Presenter<()>) {
    let frame = p.display_frame().unwrap();
    assert!(p.display_complete(&frame));
}

/// The reader's revision of `port`, as its statement says it.
fn revision(p: &mut Presenter<()>, port: ViewId) -> u32 {
    let words = p.reader_scrollers(&[], 1.).unwrap();
    let at = words
        .chunks(9)
        .position(|w| w[0] as ViewId == port)
        .unwrap();
    words[at * 9 + 2] as u32
}

/// A port scrolled to 600, painted, with the reader holding it, as a fling's start does.
fn flinging() -> (Presenter<()>, ViewId, u32) {
    let mut p = boot();
    let port = named(&p, "port");
    p.scroll_to(port, (0., 600.)).unwrap();
    attach(&mut p);
    let rev = p.reader_under(200., 300., 1.)[5] as u32;
    assert_eq!(revision(&mut p, port), rev);
    (p, port, rev)
}

/// Plans a list's own position for `port` at `top`, as a measurement correction does: from where the reader
/// is now, on the current input sequence.
fn plan_own(p: &mut Presenter<()>, port: ViewId, top: f32) {
    plan(p, port, top, Origin::Anchor);
}

/// Plans a position of `origin` for `port` at `top` on a list that is not anchor-scrolled (a virtualized
/// list's cursor), from where the reader is now.
fn plan(p: &mut Presenter<()>, port: ViewId, top: f32, origin: Origin) {
    let key = p.host.kernel().node(port).unwrap().key;
    let mut offset = p.scroll_of(port);
    let cursor = p.collection.cursors.entry(port).or_default();
    cursor.bind(key, cursor.sequence);
    cursor.anchored = false;
    cursor.model_offset(key, (0., top), true, origin, &mut offset);
}

#[test]
fn a_lists_own_position_acknowledged_during_a_fling_keeps_the_revision_and_the_fling() {
    // The trace's case: the reader has moved, then a list's own position (a measurement's 3 px) is planned
    // from there, painted and acknowledged while the reader still flings. Before, the acknowledgement moved
    // the offset 3 px but rebased the reader by the anchor's share alone (0), so the revision moved on and the
    // Android reader ended its fling, about 30 ms after release in every trace.
    let (mut p, port, rev) = flinging();
    for _ in 0..3 {
        assert_eq!(p.reader_scrolled(port, (0., -40.), rev), Some((0., -40.)));
    }
    assert_eq!(p.scroll_of(port), (0., 480.));
    plan_own(&mut p, port, 477.);
    attach(&mut p);
    assert_eq!(p.scroll_of(port), (0., 477.), "the list's own 3 px land");
    assert_eq!(
        revision(&mut p, port),
        rev,
        "a list's own position is no app scroll"
    );
    assert_eq!(
        p.reader_scrolled(port, (0., -40.), rev),
        Some((0., -40.)),
        "the fling's next move is taken"
    );
    assert_eq!(
        p.scroll_of(port),
        (0., 437.),
        "and the reader's moves stand"
    );
}

#[test]
fn a_readers_move_before_the_acknowledgement_carries_the_lists_own_anchor_by_its_change() {
    // An anchor's 3 px planned from 600, painted, the reader moving 40 before its acknowledgement: the move
    // stands and the anchor's change goes on with it, painted; the revision stands.
    let (mut p, port, rev) = flinging();
    plan_own(&mut p, port, 597.);
    let painted = p.display_frame().unwrap();
    assert_eq!(p.reader_scrolled(port, (0., -40.), rev), Some((0., -40.)));
    assert!(
        p.display_complete(&painted),
        "the old picture's acknowledgement"
    );
    assert_eq!(p.scroll_of(port), (0., 560.), "the reader's move stands");
    attach(&mut p);
    assert_eq!(
        p.scroll_of(port),
        (0., 557.),
        "the carried anchor painted and acknowledged"
    );
    assert_eq!(revision(&mut p, port), rev);
}

#[test]
fn a_clamp_carried_across_a_readers_move_clamps_where_the_reader_is() {
    // An absolute clamp (offset 600 to 550) and a reader's sample to 540: the clamp re-clamps 540, which is in
    // range, not 540 - 50.
    let (mut p, port, rev) = flinging();
    plan(&mut p, port, 550., Origin::Clamp);
    let painted = p.display_frame().unwrap();
    assert_eq!(p.reader_scrolled(port, (0., -60.), rev), Some((0., -60.)));
    assert!(p.display_complete(&painted));
    attach(&mut p);
    assert_eq!(p.scroll_of(port), (0., 540.));
    assert_eq!(revision(&mut p, port), rev);
    // And one the reader stays outside of is still clamped there.
    plan(&mut p, port, 500., Origin::Clamp);
    let painted = p.display_frame().unwrap();
    assert_eq!(p.reader_scrolled(port, (0., 10.), rev), Some((0., 10.)));
    assert!(p.display_complete(&painted));
    attach(&mut p);
    assert_eq!(p.scroll_of(port), (0., 500.), "550 clamped to 500 again");
    assert_eq!(revision(&mut p, port), rev);
}

#[test]
fn an_acknowledgement_that_applies_part_of_a_newer_position_moves_its_base() {
    // Base 600, painted 620, a newer 630 planned from the same base; the 620 is acknowledged, then the reader
    // moves 10: the newer position has 10 left to apply, not 30, so 640, not 660.
    let (mut p, port, rev) = flinging();
    plan_own(&mut p, port, 620.);
    let painted = p.display_frame().unwrap();
    plan_own(&mut p, port, 630.);
    assert!(p.display_complete(&painted));
    assert_eq!(p.scroll_of(port), (0., 620.));
    assert_eq!(p.reader_scrolled(port, (0., 10.), rev), Some((0., 10.)));
    attach(&mut p);
    assert_eq!(p.scroll_of(port), (0., 640.));
    assert_eq!(revision(&mut p, port), rev);
}

#[test]
fn fifty_rows_prepended_during_a_fling_shift_it_by_their_height_without_ending_it() {
    // iOS's behaviour (CollectionIOS `shift`): the content stays where it shows and the glide goes on.
    let (mut p, port, rev) = flinging();
    let row = named(&p, "row30");
    let y = |p: &mut Presenter<()>| p.boxes().iter().find(|b| b.id == row).unwrap().rect.1;
    let shown = y(&mut p);
    assert_eq!(p.reader_scrolled(port, (0., -40.), rev), Some((0., -40.)));
    press(&mut p, "prepend");
    let painted = p.display_frame().unwrap();
    assert_eq!(p.reader_scrolled(port, (0., -40.), rev), Some((0., -40.)));
    assert!(p.display_complete(&painted));
    attach(&mut p);
    // 600 - 80 moved by the reader, + 1,000 for the 50 rows above.
    assert_eq!(p.scroll_of(port), (0., 1520.));
    assert_eq!(
        y(&mut p),
        shown + 80.,
        "row30 moved only as the reader moved it"
    );
    assert_eq!(
        revision(&mut p, port),
        rev,
        "rows prepended are no app scroll"
    );
    assert_eq!(p.reader_scrolled(port, (0., -40.), rev), Some((0., -40.)));
}

#[test]
fn an_apps_position_during_a_fling_lands_where_asked_and_ends_it() {
    // As UIKit's setContentOffset stops a glide: an app's position is the app's, not the list's.
    let (mut p, port, rev) = flinging();
    assert_eq!(p.reader_scrolled(port, (0., -40.), rev), Some((0., -40.)));
    press(&mut p, "jump");
    attach(&mut p);
    assert_eq!(p.scroll_of(port), (0., 100.));
    assert_ne!(
        revision(&mut p, port),
        rev,
        "an app's position moves the revision on"
    );
    assert_eq!(
        p.reader_scrolled(port, (0., -40.), rev),
        None,
        "a move under the old revision is dropped"
    );
}

#[test]
fn a_fractional_own_position_keeps_the_revision() {
    // In f32, 480 + 477.1 - 480 is 477.09998 against an offset of 477.1: the reader's tracked offset must be
    // exactly the offset, or the revision's exact compare takes it for an app scroll.
    let (mut p, port, rev) = flinging();
    for _ in 0..3 {
        assert_eq!(p.reader_scrolled(port, (0., -40.), rev), Some((0., -40.)));
    }
    plan_own(&mut p, port, 477.1);
    attach(&mut p);
    assert_eq!(p.scroll_of(port), (0., 477.1));
    assert_eq!(revision(&mut p, port), rev);
    assert!(p.reader_scrolled(port, (0., -40.), rev).is_some());
}

#[test]
fn a_lists_own_refinement_of_an_apps_position_stays_the_apps() {
    // An app's position refined by a measurement correction before it is painted still lands where the app
    // asked and still ends the fling.
    let (mut p, port, rev) = flinging();
    assert_eq!(p.reader_scrolled(port, (0., -40.), rev), Some((0., -40.)));
    let key = p.host.kernel().node(port).unwrap().key;
    let mut offset = p.scroll_of(port);
    let cursor = p.collection.cursors.entry(port).or_default();
    cursor.bind(key, cursor.sequence);
    cursor.model_offset(key, (0., 100.), true, Origin::App, &mut offset);
    cursor.model_offset(key, (0., 94.), true, Origin::Anchor, &mut offset);
    attach(&mut p);
    assert_eq!(p.scroll_of(port), (0., 94.));
    assert_ne!(revision(&mut p, port), rev, "still the app's position");
}
