//! LLP 1069.002: `input type="file"` is a `Control` with `accept` and
//! `multiple`; `showPicker` is a host command; `change` carries a
//! `list<Picked>` and `cancel` nothing; `accept` is bounded at bake.

use exact_kernel::{Kernel, NodeType, PropId};
use exact_runner::{ControlValue, DataError, DataSource, Event, Picked, Runner, Value};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _args: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.to_string()))
    }
}

const COMPOSE: &str = r#"component App
  state summary = ""
  state cancels = 0
  action choose
    showPicker("attach")
  action attach(files: list<Picked>) writes summary
    summary = join(map(files, (f, i) => `${f.name}:${f.type}:${f.size}:${match f.width { case some(w) => w, case none => 0 }}`), ",")
  action dismissed writes cancels
    cancels = cancels + 1
  view
    column
      input type="file" accept="image/*, Video/MP4" multiple id="attach" testId="attach" display="none" change=attach cancel=dismissed
      input type="file" accept="image/png" id="one" testId="one" display="none"
      button press=choose testId="choose"
        text "Add"
"#;

fn boot(src: &str) -> Runner<NoData> {
    Runner::boot(
        contract::compile(src).unwrap_or_else(|e| panic!("{e}")),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn view_of(r: &Runner<NoData>, test_id: &str) -> u32 {
    let key = r.kernel().find_by_test_id(test_id)[0];
    r.kernel().node_by_key(key).unwrap().id
}

fn slot(r: &Runner<NoData>, name: &str) -> String {
    let state = exact_runner::agent::state(r);
    let v: serde_json::Value = serde_json::from_str(&state).unwrap();
    v["slots"][name].to_string()
}

#[test]
fn a_file_input_is_a_control_with_accept_and_multiple() {
    let r = boot(COMPOSE);
    let attach = r.kernel().node(view_of(&r, "attach")).unwrap();
    assert_eq!(attach.node_type, NodeType::Control);
    assert_eq!(attach.props.str(PropId::Type), Some("file"));
    assert_eq!(attach.props.bool(PropId::Multiple), Some(true));
    let request = r.picker("attach").unwrap();
    assert_eq!(request.accept, ["image/*", "video/mp4"]);
    assert!(request.multiple);
    assert!(!r.picker("one").unwrap().multiple);
    assert!(
        r.picker("choose").is_none(),
        "only a file input is a picker"
    );
}

#[test]
fn change_carries_the_picked_records_and_cancel_nothing() {
    let mut r = boot(COMPOSE);
    let attach = view_of(&r, "attach");
    let files = Picked::payload(
        "app:/tmp/picked/1-1.jpg\tIMG_0412.HEIC\timage/jpeg\t2048\t4032\t3024\t\napp:/tmp/picked/1-2.mp4\tclip.mp4\tvideo/mp4\t9\t\t\t2.5",
    )
    .unwrap();
    r.dispatch(attach, Event::Change(ControlValue::Files(files)))
        .unwrap();
    assert_eq!(
        slot(&r, "summary"),
        "\"IMG_0412.HEIC:image/jpeg:2048:4032,clip.mp4:video/mp4:9:0\""
    );
    r.dispatch(attach, Event::Cancel).unwrap();
    assert_eq!(slot(&r, "cancels"), "1");
    // A node without `cancel=` takes it as nothing; text is refused.
    let one = view_of(&r, "one");
    r.dispatch(one, Event::Cancel).unwrap();
    assert!(r
        .dispatch(attach, Event::Change(ControlValue::Text("x".into())))
        .is_err());
}

#[test]
fn a_hold_is_answered_only_by_files_the_input_accepts() {
    let mut r = boot(COMPOSE);
    let one = r.hold_picker("one").unwrap();
    assert!(r.hold_picker("one").is_err(), "one picker at a time");
    let answer = |v: &str| exact_runner::HoldAnswer::Value(v.into());
    let e = r.answer_hold(one, &answer("/a/b.jpg")).unwrap_err();
    assert!(e.contains("not among accept"), "{e}");
    let e = r
        .answer_hold(one, &answer("/a/b.png\n/a/c.png"))
        .unwrap_err();
    assert!(e.contains("takes one file"), "{e}");
    r.answer_hold(one, &answer("/a/b.png")).unwrap();
    // Safari's rule: a HEIC photo under `image/jpeg`-only is a JPEG.
    let heic = boot(&COMPOSE.replace("accept=\"image/png\"", "accept=\"image/jpeg\""));
    let mut heic = heic;
    let t = heic.hold_picker("one").unwrap();
    heic.answer_hold(t, &answer("/a/IMG_0412.HEIC")).unwrap();
}

#[test]
fn the_bake_bounds_accept_and_refuses_capture() {
    let refused = |src: &str| contract::compile(src).unwrap_err().id;
    let view = |attrs: &str| format!("component App\n  view\n    input type=\"file\" {attrs}\n");
    assert_eq!(
        refused(&view("accept=\"application/json\"")),
        "bake-picker-accept"
    );
    assert_eq!(
        refused(&view("accept=\"image/*,*/*\"")),
        "bake-picker-accept"
    );
    assert_eq!(refused(&view("")), "lower-picker-accept");
    let bound = "component App\n  state kinds = \"image/*\"\n  view\n    input type=\"file\" accept=kinds\n";
    assert_eq!(refused(bound), "lower-picker-accept");
    assert_eq!(
        refused(&view("accept=\"image/*\" capture=\"user\"")),
        "lower-picker-capture"
    );
    assert_eq!(
        refused("component App\n  view\n    input accept=\"image/*\"\n"),
        "lower-attr-tag"
    );
    contract::compile(&view("accept=\"image/heic,video/*\"")).unwrap();
}
