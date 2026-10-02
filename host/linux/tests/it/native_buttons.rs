//! LLP 1069.011 on Linux: a native button is a painted button that presses
//! (never toggles), sized as its title in its look's font plus the look's
//! padding, its face never painted as children; `type` refuses it.

use exact_linux::{presenter::PainterChoice, Presenter};
use exact_runner::{DataError, DataSource, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

#[test]
fn a_native_button_presses_is_sized_and_takes_no_value() {
    let plan = contract::compile(
        r#"component App
  state n = 0
  action go writes n
    n = n + 1
  view
    column align-items="flex-start" press=go testId="row"
      text `pressed ${n}` testId="count"
      button appearance="auto" buttonStyle="filled" press=go testId="filled"
        text "Send"
      button appearance="auto" testId="ancestor"
        text "Up"
      button appearance="auto" buttonStyle="filled" press=go disabled=true testId="off"
        text "Off"
"#,
    )
    .unwrap();
    let dir = std::env::temp_dir().join(format!("exact-native-buttons-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (mut p, error) = Presenter::boot_with(
        &plan.encode(),
        NoData,
        (400., 300.),
        1.,
        dir.clone(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    let id = |p: &Presenter<NoData>, t: &str| {
        let k = p.host().kernel();
        k.node_by_key(k.find_by_test_id(t)[0]).unwrap().id
    };
    let count = |p: &Presenter<NoData>| {
        let k = p.host().kernel();
        let node = k.node(id(p, "count")).unwrap();
        node.text_runs()
            .iter()
            .map(|r| r.text.to_string())
            .collect::<String>()
    };
    let filled = id(&p, "filled");
    let frame = p.host().kernel().node(filled).unwrap().frame;
    assert!(
        frame.width > 40. && frame.width < 80.,
        "{frame:?}: Send in 13.33 px plus 2 × 12"
    );
    assert!(frame.height > 25. && frame.height < 35., "{frame:?}");
    p.tap(filled).unwrap();
    assert_eq!(count(&p), "pressed 1");
    let ancestor = id(&p, "ancestor");
    p.tap(ancestor).unwrap();
    assert_eq!(
        count(&p),
        "pressed 2",
        "no handler of its own: the ancestor's"
    );
    let off = id(&p, "off");
    let _ = p.tap(off);
    assert_eq!(count(&p), "pressed 2", "disabled");
    assert!(p
        .type_text(filled, "x")
        .unwrap_err()
        .contains("takes a press"));
    std::fs::remove_dir_all(dir).unwrap();
}
