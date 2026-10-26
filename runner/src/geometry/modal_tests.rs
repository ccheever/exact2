use super::*;
use crate::{DataError, DataSource, Runner};
use exact_kernel::Offer;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn fixture(parent: &str, dialog: &str) -> Runner<NoData> {
    let source = format!(
        r#"component App
  state point = "unset"
  state points = []
  state matches = []
  action read(x: number, y: number)
    point = match elementFromPoint(x, y) {{ case some(id) => id, case none => "none" }}
    points = elementsFromPoint(x, y)
    matches = querySelectorAll("[data-paint~=\"accent\"]")
  view
    column id="page" width=400 height=800 data-paint="accent"
      column {parent}
        dialog id="lower" position="absolute" left=0 top=0 width=200 height=200 data-paint="accent"
          column id="lower-child" width=50 height=50 data-paint="accent"
        dialog id="upper" position="absolute" left=100 top=100 width=200 height=200 data-paint="accent" {dialog}
          column id="upper-child" width=50 height=50 data-paint="accent"
          column id="pop" popover="manual" position="absolute" left=0 top=0 width=50 height=50 data-paint="accent"
          column inert=true
            column id="inert-child" data-paint="accent"
          column display="none"
            column id="hidden-child" data-paint="accent"
"#
    );
    let mut r = Runner::boot(
        contract::compile(&source).unwrap(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let root = r.kernel().roots()[0];
    r.kernel_mut()
        .compute_layout(root, Offer::definite(400.0, 800.0))
        .unwrap();
    r
}

#[test]
fn structural_reads_mark_only_the_first_candidate_even_when_all_are_refused() {
    use std::cell::RefCell;
    thread_local! { static FIRST: RefCell<Vec<bool>> = const { RefCell::new(Vec::new()) }; }
    let r = fixture("", "");
    for value in ["missing", "accent", "accent"] {
        let hits = data_matches(
            r.kernel(),
            &Scrolled::default(),
            |_, _, _, first| {
                FIRST.with_borrow_mut(|calls| calls.push(first));
                false
            },
            "paint",
            value,
        );
        assert!(hits.is_empty());
        let calls = FIRST.take();
        if value == "missing" {
            assert!(calls.is_empty());
        } else {
            assert!(calls.len() > 1);
            assert!(calls[0]);
            assert!(calls[1..].iter().all(|first| !first));
        }
    }
}

fn open(r: &mut Runner<NoData>, names: &[&str]) {
    let frames: Vec<_> = names
        .iter()
        .map(|name| {
            let view = r
                .kernel()
                .arena()
                .local_id(r.kernel().find_by_id(name)[0].index);
            let rect = match *name {
                "lower" => [0.0, 0.0, 200.0, 200.0],
                "upper" => [100.0, 100.0, 200.0, 200.0],
                "pop" => [0.0, 0.0, 50.0, 50.0],
                _ => unreachable!(),
            };
            (view, rect, None)
        })
        .collect();
    r.top_layer_frames(&frames, None);
}

fn read(r: &mut Runner<NoData>, x: f64, y: f64, point: &str, points: &[&str], matches: &[&str]) {
    r.act("read", vec![Value::Number(x), Value::Number(y)])
        .unwrap();
    assert_eq!(
        r.slot("point"),
        Some(&Value::str(point)),
        "elementFromPoint"
    );
    for (slot, expected) in [("matches", matches), ("points", points)] {
        assert_eq!(
            r.slot(slot),
            Some(&Value::list(
                expected.iter().map(|id| Value::str(id)).collect()
            )),
            "{slot}"
        );
    }
}

#[test]
fn a_modal_dialog_blocks_the_uncovered_page() {
    let mut r = fixture("", "");
    open(&mut r, &["lower"]);
    read(&mut r, 350.0, 350.0, "none", &[], &["lower", "lower-child"]);
    open(&mut r, &[]);
    read(&mut r, 350.0, 350.0, "page", &["page"], &["page"]);
}

#[test]
fn only_the_front_modal_dialog_is_reachable() {
    let mut r = fixture("", "");
    open(&mut r, &["lower", "upper"]);
    let matches = ["upper", "upper-child"];
    read(
        &mut r,
        120.0,
        120.0,
        "upper-child",
        &["upper-child", "upper"],
        &matches,
    );
    read(&mut r, 20.0, 20.0, "none", &[], &matches);
    open(&mut r, &["upper", "lower"]);
    read(
        &mut r,
        120.0,
        120.0,
        "lower",
        &["lower"],
        &["lower", "lower-child"],
    );
}

#[test]
fn a_popover_above_a_modal_keeps_the_modal_boundary() {
    let mut r = fixture("", "");
    open(&mut r, &["lower", "upper", "pop"]);
    let matches = ["upper", "upper-child", "pop"];
    read(&mut r, 20.0, 20.0, "pop", &["pop"], &matches);
    read(
        &mut r,
        120.0,
        120.0,
        "upper-child",
        &["upper-child", "upper"],
        &matches,
    );
    read(&mut r, 350.0, 350.0, "none", &[], &matches);
}

#[test]
fn selectors_reach_modal_descendants_under_an_inert_ancestor() {
    let mut r = fixture("inert=true", "");
    open(&mut r, &["upper", "pop"]);
    read(
        &mut r,
        120.0,
        120.0,
        "upper-child",
        &["upper-child", "upper"],
        &["upper", "upper-child", "pop"],
    );
    let mut r = fixture("inert=true", "inert=true");
    open(&mut r, &["upper", "pop"]);
    read(&mut r, 120.0, 120.0, "none", &[], &[]);
    let mut r = fixture("inert=true", "popover=\"manual\"");
    open(&mut r, &["upper", "pop"]);
    read(&mut r, 120.0, 120.0, "page", &["page"], &["page"]);
}

#[test]
fn a_top_layer_clip_bounds_reads_without_rescaling_descendants() {
    fn present(r: &mut Runner<NoData>, frames: &[TopLayerFrame], modal: Option<usize>) -> Scrolled {
        r.top_layer_frames(frames, modal);
        let mut scrolled = Scrolled::default();
        scrolled.top_layer_frames(r.kernel(), frames, modal);
        scrolled
    }
    for (dialog, boundary, modal) in [
        ("", None, true),
        ("overflow-y=\"auto\"", None, true),
        ("popover=\"manual\"", None, false),
        ("popover=\"manual\"", Some(0), true),
    ] {
        let mut r = fixture("", dialog);
        let upper = view_of(r.kernel(), "upper").unwrap();
        let child = view_of(r.kernel(), "upper-child").unwrap();
        let frame = [100.0, 100.0, 200.0, 200.0];
        let scrolled = present(
            &mut r,
            &[(upper, frame, Some([110.0, 110.0, 30.0, 20.0]))],
            boundary,
        );
        let matches: &[&str] = if modal {
            &["upper", "upper-child"]
        } else {
            &["page", "upper", "upper-child"]
        };
        let inside: &[&str] = if modal {
            &["upper-child", "upper"]
        } else {
            &["upper-child", "upper", "page"]
        };
        let outside: &[&str] = if modal { &[] } else { &["page"] };
        for (x, y) in [
            (105.0, 120.0),
            (120.0, 105.0),
            (140.0, 120.0),
            (120.0, 130.0),
        ] {
            read(
                &mut r,
                x,
                y,
                if modal { "none" } else { "page" },
                outside,
                matches,
            );
        }
        read(&mut r, 120.0, 120.0, "upper-child", inside, matches);
        let child_frame = kernel_frame(r.kernel(), &scrolled, child);
        assert_eq!(
            (
                child_frame.x,
                child_frame.y,
                child_frame.width,
                child_frame.height
            ),
            (100.0, 100.0, 50.0, 50.0)
        );
        let upper_frame = kernel_frame(r.kernel(), &scrolled, upper);
        assert_eq!(
            (
                upper_frame.x,
                upper_frame.y,
                upper_frame.width,
                upper_frame.height
            ),
            (100.0, 100.0, 200.0, 200.0)
        );
        let (_, _, visible, ratio) = intersection(r.kernel(), &scrolled, child);
        assert!(visible);
        assert_eq!(ratio, 30.0 * 20.0 / (50.0 * 50.0));

        let scrolled = present(
            &mut r,
            &[(upper, frame, Some([180.0, 180.0, 20.0, 20.0]))],
            boundary,
        );
        read(
            &mut r,
            120.0,
            120.0,
            if modal { "none" } else { "page" },
            outside,
            if modal {
                &["upper"]
            } else {
                &["page", "upper"]
            },
        );
        let (_, _, visible, ratio) = intersection(r.kernel(), &scrolled, child);
        // A scrolling root measures against itself, as under a pane's block clip.
        assert_eq!(visible, dialog.contains("overflow"));
        assert_eq!(ratio, 0.0);
        assert_eq!(kernel_frame(r.kernel(), &scrolled, child), child_frame);

        let scrolled = present(
            &mut r,
            &[(upper, frame, Some([100.0, 100.0, 0.0, 0.0]))],
            boundary,
        );
        read(
            &mut r,
            120.0,
            120.0,
            if modal { "none" } else { "page" },
            outside,
            if modal { &[] } else { &["page"] },
        );

        assert_eq!(kernel_frame(r.kernel(), &scrolled, child), child_frame);

        let scrolled = present(&mut r, &[(upper, frame, None)], boundary);
        read(&mut r, 105.0, 105.0, "upper-child", inside, matches);
        assert_eq!(kernel_frame(r.kernel(), &scrolled, child), child_frame);
        let (_, _, visible, ratio) = intersection(r.kernel(), &scrolled, child);
        assert!(visible);
        assert_eq!(ratio, 1.0);
    }
}

#[test]
fn a_lifted_popover_above_a_sheet_escapes_its_clip() {
    let mut r = fixture("", "");
    let upper = view_of(r.kernel(), "upper").unwrap();
    let pop = view_of(r.kernel(), "pop").unwrap();
    r.top_layer_frames(
        &[
            (
                upper,
                [100.0, 100.0, 200.0, 200.0],
                Some([180.0, 180.0, 20.0, 20.0]),
            ),
            (pop, [0.0, 0.0, 50.0, 50.0], None),
        ],
        Some(0),
    );
    read(&mut r, 20.0, 20.0, "pop", &["pop"], &["upper", "pop"]);
    read(&mut r, 120.0, 120.0, "none", &[], &["upper", "pop"]);
}

#[test]
fn a_native_content_viewport_scrolls_the_root_without_moving_its_clip() {
    let source = r#"component App
  view
    column id="page" width=400 height=800
      column id="route" position="absolute" left=10 top=15 width=200 height=400
        column id="sticky" position="sticky" top=0 width=50 height=40
        column id="field" position="absolute" left=0 top=180 width=50 height=50
"#;
    let mut r = Runner::boot(
        contract::compile(source).unwrap(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let page = view_of(r.kernel(), "page").unwrap();
    let route = view_of(r.kernel(), "route").unwrap();
    let field = view_of(r.kernel(), "field").unwrap();
    let sticky = view_of(r.kernel(), "sticky").unwrap();
    r.kernel_mut()
        .set_host_block(
            route,
            Some(exact_kernel::HostBlock {
                width: exact_kernel::BlockAxis::Is(200.0),
                height: exact_kernel::BlockAxis::Is(200.0),
            }),
        )
        .unwrap();
    let root = r.kernel().roots()[0];
    r.kernel_mut()
        .compute_layout(root, Offer::definite(400.0, 800.0))
        .unwrap();
    for (offset, ratio) in [(-20.0, 0.0), (0.0, 0.1), (100.0, 1.0), (250.0, 0.0)] {
        let mut scrolled = Scrolled::default();
        scrolled.place(
            r.kernel(),
            route,
            Some(([20.0, 30.0, 200.0, 200.0], page, Some([0.0, offset]))),
        );
        assert_eq!(kernel_frame(r.kernel(), &scrolled, route).y, 45.0 - offset);
        assert_eq!(kernel_frame(r.kernel(), &scrolled, field).y, 225.0 - offset);
        assert_eq!(intersection(r.kernel(), &scrolled, field).3, ratio);
        assert_eq!(
            intersection(r.kernel(), &scrolled, sticky).3,
            1.0,
            "sticky at offset {offset}"
        );
        assert_eq!(kernel_point(r.kernel(), &scrolled, 35.0, 29.0), Some(page));
        assert_eq!(kernel_point(r.kernel(), &scrolled, 35.0, 230.0), Some(page));
        if offset == 100.0 {
            assert_eq!(
                kernel_point(r.kernel(), &scrolled, 35.0, 135.0),
                Some(field)
            );
        }
    }
    for (scale, offset, y, ratio) in [
        (0.5, 0.0, 127.5, 0.1),
        (0.5, 100.0, 77.5, 1.0),
        (2.0, 100.0, 220.0, 1.0),
    ] {
        let mut scrolled = Scrolled::default();
        scrolled.place(
            r.kernel(),
            route,
            Some((
                [20.0, 30.0, 200.0 * scale, 200.0 * scale],
                page,
                Some([0.0, offset]),
            )),
        );
        assert_eq!(kernel_frame(r.kernel(), &scrolled, field).y, y);
        assert_eq!(intersection(r.kernel(), &scrolled, field).3, ratio);
        assert_eq!(
            kernel_point(r.kernel(), &scrolled, 45.0, 30.0 + 200.0 * scale),
            Some(page)
        );
    }
    for offset in [100.0, 250.0] {
        let mut scrolled = Scrolled::default();
        scrolled.place(
            r.kernel(),
            route,
            Some(([20.0, 30.0, 200.0, 200.0], page, Some([0.0, offset]))),
        );
        scrolled.top_layer_frames(
            r.kernel(),
            &[(route, [20.0, 30.0, 200.0, 200.0], None)],
            None,
        );
        assert_eq!(kernel_frame(r.kernel(), &scrolled, field).y, 225.0);
        assert_eq!(
            (
                intersection(r.kernel(), &scrolled, field).3,
                kernel_point(r.kernel(), &scrolled, 35.0, 240.0),
            ),
            (0.1, Some(page)),
            "top-layer geometry replaces the ordinary placement and its scroll offset"
        );
        assert_eq!(intersection(r.kernel(), &scrolled, sticky).3, 1.0);
    }
}
