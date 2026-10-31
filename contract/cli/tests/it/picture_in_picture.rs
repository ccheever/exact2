//! An image's picture in picture (LLP 1042 §9): `requestPictureInPicture(id)`
//! and `exitPictureInPicture()` reach the host, and the image hears
//! `enterpictureinpicture`, `leavepictureinpicture` and, with its reason,
//! `pictureinpictureerror` as the host reports them.
use exact_runner::{DataError, DataSource, Event, Runner, Value};

const APP: &str = r#"component App
  state floating = false
  state entries = 0
  state refusal = ""
  action float
    requestPictureInPicture("page")
  action unfloat
    exitPictureInPicture()
  action refused(why: string)
    refusal = why
  action entered
    floating = true
    entries = entries + 1
  action left
    floating = false
  view
    column
      button press=float testId="float"
        text "Float"
      image "app:/tmp/frame.jpg" id="page" testId="page" width=1 height=1 enterpictureinpicture=entered leavepictureinpicture=left pictureinpictureerror=refused
"#;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.to_string()))
    }
}

fn boot() -> Runner<NoData> {
    Runner::boot(
        contract::compile(APP).unwrap(),
        NoData,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

#[test]
fn the_request_is_a_command_naming_the_image() {
    let mut r = boot();
    r.take_commands();
    r.act("float", vec![]).unwrap();
    r.act("unfloat", vec![]).unwrap();
    let ran: Vec<_> = r
        .take_commands()
        .into_iter()
        .map(|c| (c.name, c.args))
        .collect();
    assert_eq!(
        ran,
        [
            (
                "requestPictureInPicture".to_string(),
                vec![Value::str("page")]
            ),
            ("exitPictureInPicture".to_string(), vec![])
        ]
    );
}

#[test]
fn the_image_hears_it_enter_and_leave() {
    let mut r = boot();
    let key = r.kernel().find_first_by_test_id("page").unwrap();
    let id = r.kernel().node_by_key(key).unwrap().id;
    for (wire, floating) in [
        ("enterpictureinpicture\n", true),
        ("leavepictureinpicture\n", false),
        ("enterpictureinpicture\n", true),
    ] {
        let event = Event::media_payload(wire).unwrap_or_else(|| panic!("{wire:?} refused"));
        r.dispatch(id, event).unwrap();
        assert_eq!(r.slot("floating"), Some(&Value::Bool(floating)), "{wire:?}");
    }
    assert_eq!(r.slot("entries"), Some(&Value::Number(2.0)));
    let refused = "picture in picture is not possible";
    let event = Event::media_payload(&format!("pictureinpictureerror\n{refused}")).unwrap();
    r.dispatch(id, event).unwrap();
    assert_eq!(r.slot("refusal"), Some(&Value::str(refused)));
}

#[test]
fn the_events_carry_nothing() {
    for wire in ["enterpictureinpicture\ntrue", "leavepictureinpicture\n1"] {
        assert!(Event::media_payload(wire).is_none(), "{wire:?}");
    }
}

#[test]
fn the_request_names_one_element() {
    for statement in [
        r#"requestPictureInPicture()"#,
        r#"requestPictureInPicture(1)"#,
        r#"requestPictureInPicture("page", "other")"#,
        r#"exitPictureInPicture("page")"#,
    ] {
        let source = APP.replace(r#"requestPictureInPicture("page")"#, statement);
        let error = contract::compile(&source).unwrap_err().to_string();
        assert!(error.contains("type-media-command"), "{statement}: {error}");
    }
}
