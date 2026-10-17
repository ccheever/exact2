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
            (view, rect)
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
