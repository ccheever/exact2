//! LLP 1069.011 on Linux: a native button is a painted button that presses
//! (never toggles), sized as its title in its look's font plus the look's
//! padding, its face never painted as children; `type` refuses it.

use exact_linux::{presenter::PainterChoice, Presenter};
use exact_runner::{DataError, DataSource, Value};

#[derive(Default)]
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
  action go
    n = n + 1
  view
    column align-items="flex-start" press=go testId="row"
      text `pressed ${n}` testId="count"
      button appearance="auto" buttonStyle="filled" press=go testId="filled"
        text "Send"
      button appearance="auto" buttonStyle="filled" press=go testId="wider"
        text "Send this much longer title"
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
    let mut engine = exact_linux::text::TextEngine::new();
    let text = engine.paragraph(
        &exact_linux::paint::text_spec(&exact_kernel::StyleProps::default(), "Send"),
        None,
    );
    assert_eq!(
        (frame.width, frame.height),
        (text.width + 2.0 * 9.0, text.height + 2.0 * 7.0),
        "medium control: 16px host font, 9px horizontal and 7px vertical padding"
    );
    // Measured, not the kernel's 64 × 34 before a host reports a size.
    let wider = p.host().kernel().node(id(&p, "wider")).unwrap().frame;
    assert!(
        wider.width > frame.width + 80.,
        "{wider:?} against {frame:?}: each is its own title's width"
    );
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

/// LLP 1069.011.000 D1: a native button is a button under a tab's role —
/// Enter presses it, as it presses a custom tab.
#[test]
fn a_native_tab_presses_on_enter_as_a_button_does() {
    let plan = contract::compile(
        r#"component App
  state n = 0
  action go
    n = n + 1
  view
    column align-items="flex-start"
      text `pressed ${n}` testId="count"
      row role="tablist" text-transform="uppercase"
        button appearance="auto" role="tab" aria-selected=true press=go testId="tab"
          text "Inbox"
"#,
    )
    .unwrap();
    let dir = std::env::temp_dir().join(format!("exact-native-tabs-{}", std::process::id()));
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
    let tab = id(&p, "tab");
    p.type_key(tab, "Enter", "Enter", true, false).unwrap();
    // A held key's repeats press it once, as a native `role="button"`'s do.
    p.type_key(tab, "Enter", "Enter", true, true).unwrap();
    p.type_key(tab, "Enter", "Enter", false, false).unwrap();
    // Its face's title is as painted (grok's code review); this host
    // exposes no accessibility tree to name it in (LLP 1080.002 D2).
    let face = p.host().kernel().press_face(tab).and_then(|f| f.title);
    assert_eq!(face.as_deref(), Some("INBOX"));
    let ax = exact_linux::agent::handle(&mut p, r#"{"op":"tree","ax":true}"#);
    assert!(ax.contains(r#""unavailable":true"#), "{ax}");
    let k = p.host().kernel();
    let count = k.node(id(&p, "count")).unwrap();
    let text: String = count
        .text_runs()
        .iter()
        .map(|r| r.text.to_string())
        .collect();
    assert_eq!(text, "pressed 1");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn empty_invoker_targets_still_run_the_press_handler() {
    for attrs in [
        "popovertarget=\"\"",
        "commandfor=\"\" command=\"show-modal\"",
    ] {
        let source = format!(
            r#"component App
  state n = 0
  action go
    n = n + 1
  view
    column
      button "Press" appearance="auto" {attrs} press=go testId="button"
      text `pressed ${{n}}` testId="count"
"#
        );
        let plan = contract::compile(&source).unwrap();
        let (mut p, err) = Presenter::boot_with(
            &plan.encode(),
            NoData,
            (400., 300.),
            1.,
            std::path::PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(err.is_none(), "{err:?}");
        let k = p.host().kernel();
        let button = k.node_by_key(k.find_by_test_id("button")[0]).unwrap().id;
        p.tap(button).unwrap();
        let k = p.host().kernel();
        let count = k.node_by_key(k.find_by_test_id("count")[0]).unwrap();
        assert_eq!(
            count
                .text_runs()
                .iter()
                .map(|r| r.text.to_string())
                .collect::<String>(),
            "pressed 1",
            "{attrs}"
        );
    }
}

#[test]
fn disabled_buttons_keep_authored_accent_fill() {
    let plan = contract::compile(r##"component App
  view
    column width=400 height=300 background-color="white" align-items="flex-start"
      button "Authored" appearance="auto" buttonStyle="filled" accent-color="#ff0000" disabled=true width=120 height=40 testId="authored"
      button "Default" appearance="auto" buttonStyle="filled" disabled=true width=120 height=40 testId="default"
"##).unwrap();
    let (mut p, err) = Presenter::boot_with(
        &plan.encode(),
        NoData,
        (400., 300.),
        1.,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(err.is_none(), "{err:?}");
    let sample = |p: &mut Presenter<NoData>, name: &str| {
        let k = p.host().kernel();
        let f = k.node_by_key(k.find_by_test_id(name)[0]).unwrap().frame;
        let shot = p.frame();
        let c = shot.pixel(f.x as u32 + 60, f.y as u32 + 4).unwrap();
        [c.red(), c.green(), c.blue(), c.alpha()]
    };
    assert_eq!(sample(&mut p, "authored"), [255, 0, 0, 255]);
    assert_ne!(sample(&mut p, "default"), [0, 117, 255, 255]);
}

#[test]
fn native_and_bare_buttons_ring_on_focus_without_a_handler() {
    let plan = contract::compile(r##"component App
  action focusNative
    focus("native")
  action focusBare
    focus("bare")
  action focusOff
    focus("off")
  view
    column width=400 height=300 background-color="white" align-items="flex-start"
      button "Native" id="native" appearance="auto" width=120 height=40 accent-color="#ff0000" testId="native"
      button id="bare" appearance="none" width=120 height=40 accent-color="#ff0000" testId="bare"
        box width="100%" height="100%" background-color="blue"
          text "Bare"
      button "Off" id="off" appearance="none" disabled=true width=120 height=40 testId="off"
      button "Focus native" press=focusNative testId="focusNative"
      button "Focus bare" press=focusBare testId="focusBare"
      button "Focus disabled" press=focusOff testId="focusOff"
"##).unwrap();
    let (mut p, err) = Presenter::boot_with(
        &plan.encode(),
        NoData,
        (400., 300.),
        1.,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(err.is_none(), "{err:?}");
    let sample = |p: &mut Presenter<NoData>, name: &str| {
        let k = p.host().kernel();
        let f = k.node_by_key(k.find_by_test_id(name)[0]).unwrap().frame;
        let shot = p.frame();
        let c = shot.pixel(f.x as u32 + 60, f.y as u32).unwrap();
        [c.red(), c.green(), c.blue(), c.alpha()]
    };
    let focus = |p: &mut Presenter<NoData>, action: &str| {
        let k = p.host().kernel();
        let id = k.node_by_key(k.find_by_test_id(action)[0]).unwrap().id;
        p.tap(id).unwrap();
        p.run_commands(NoData::default);
    };
    for dark in [false, true] {
        p.set_system_scheme(dark);
        focus(&mut p, "focusNative");
        assert_eq!(sample(&mut p, "native"), [255, 0, 0, 255]);
        focus(&mut p, "focusBare");
        assert_eq!(sample(&mut p, "bare"), [255, 0, 0, 255]);
        assert_ne!(
            sample(&mut p, "native"),
            [255, 0, 0, 255],
            "old ring cleared"
        );
        focus(&mut p, "focusOff");
        assert_eq!(sample(&mut p, "off"), [255; 4], "disabled never rings");
    }
}
