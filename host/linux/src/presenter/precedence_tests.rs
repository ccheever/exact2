//! LLP 1057.001 §1 on Linux: candidates innermost first, then by rank; the
//! first whose axis rule accepts at the slop begins, and nothing after it.
use super::super::*;
use exact_runner::DataError;

struct NoData;
impl DataSource for NoData {
    fn query(
        &mut self,
        name: &str,
        _: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}
/// A swipe row inside a column that pans: the inner swipe is first.
const SOURCE: &str = r#"component App
  state replies = 0
  state panned = 0
  action reply
    replies = replies + 1
  action moved(dx: number, dy: number)
    panned = panned + dy
  view
    column width=400 height=500 pan=moved
      box testId="row" width=400 height=100 swiperight=reply touch-action="pan-y" transition="translate spring(300, 30, 1)"
        text "swipe me"
      text `${replies} ${panned}` testId="out"
"#;
fn boot() -> Presenter<NoData> {
    let (p, error) = Presenter::boot_with(
        &contract::compile(SOURCE).unwrap().encode(),
        NoData,
        (400., 500.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p
}
fn out(p: &Presenter<NoData>) -> String {
    let k = p.host.kernel();
    let id = k.node_by_key(k.find_by_test_id("out")[0]).unwrap().id;
    k.node(id)
        .unwrap()
        .props
        .str(PropId::Text)
        .unwrap()
        .to_string()
}

#[test]
fn the_inner_swipe_takes_a_horizontal_drag_before_the_ancestor_pan() {
    let mut p = boot();
    assert!(p.pointer_down(20., 40., 0.).unwrap());
    assert!(p.pointer_move(30., 40., 10.).unwrap());
    assert!(p.pointer_move(120., 40., 30.).unwrap());
    p.pointer_up(120., 40., 40.).unwrap();
    assert_eq!(out(&p), "1 0", "the swipe replied and the pan never fired");
}

#[test]
fn a_refusing_inner_swipe_falls_to_the_ancestor_pan() {
    let mut p = boot();
    assert!(p.pointer_down(20., 40., 0.).unwrap());
    assert!(p.pointer_move(20., 60., 10.).unwrap());
    p.pointer_up(20., 60., 20.).unwrap();
    assert_eq!(out(&p), "0 20", "a vertical drag is the column's pan");
}

/// A card that pans with a button inside it (kanban F6): a drag that starts on
/// the button is the card's once it passes the slop, and takes the press; a
/// tap on the button still presses it.
#[test]
fn an_ancestor_pan_takes_a_drag_that_starts_on_a_button() {
    let source = r#"component App
  state panned = 0
  state pressed = 0
  action moved(dx: number, dy: number)
    panned = panned + dx
  action push
    pressed = pressed + 1
  view
    column width=400 height=500 pan=moved
      button testId="open" press=push width=200 height=60
        text "open"
      text `${panned} ${pressed}` testId="out"
"#;
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(source).unwrap().encode(),
        NoData,
        (400., 500.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.pointer_down(20., 20., 0.).unwrap();
    p.pointer_move(23., 20., 10.).unwrap();
    p.pointer_up(23., 20., 20.).unwrap();
    assert_eq!(out(&p), "0 1", "within the slop it is the button's tap");
    p.pointer_down(20., 20., 100.).unwrap();
    p.pointer_move(30., 20., 110.).unwrap();
    p.pointer_move(80., 20., 130.).unwrap();
    p.pointer_up(80., 20., 140.).unwrap();
    assert_eq!(
        out(&p),
        "60 1",
        "past the slop the card pans and the press is taken"
    );
}
