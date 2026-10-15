//! What a button invokes in the top layer, as HTML resolves it
//! (`exact_kernel::Invocation`): after its own `press`, by the rule a
//! `commandfor` governs, and by the mode a surface was opened in.
use super::*;
use exact_runner::Value;

/// A menu with a row that opens a submenu by command alone; an alertdialog
/// `dialog` that is also a popover, its rows closing it (lowering requires a
/// dialog's rows to close it, so it presents only modally); and the buttons
/// that invoke them.
const APP: &str = r#"routes nav
  home "/"
component App
  state said = "nothing"
  state off = false
  state armed = ""
  state subName = "sub"
  action arm
    armed = "menu"
  action retarget
    subName = "sub2"
  action keep
    said = "kept"
  action rename
    said = "renamed"
  action disable
    off = true
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" width="100%" height="100%"
      for e in stack(nav) key=e.id
        column navigationKey=`${e.id}` position="absolute" inset=0 align-items="flex-start"
          button commandfor="missing" command="show-popover" popovertarget="menu" testId="dangling" height=30
            text "Dangling"
          button popovertarget="menu" popovertargetaction="HIDE" testId="hide-upper" height=30
            text "Hide"
          button press=disable disabled=off popovertarget="menu" testId="self-disable" height=30
            text "Disables itself"
          button commandfor="dpm" command="show-modal" testId="open-dpm" height=30
            text "Ask modally"
          button id="back" commandfor="dpm" command="show-modal" testId="back" height=30
            text "Back asks first"
          button press=arm popovertarget=armed testId="arm" height=30
            text "Armed by its handler"
          button commandfor="menu" command="show-popover" testId="native-cmd" appearance="auto" height=30
            text "Native"
          column press=keep testId="wrapper" width=200 height=30
            button commandfor="menu" command="show-popover" testId="wrapped" width=200 height=30
              text "Wrapped"
      text said testId="said"
      column id="menu" popover="auto" role="menu"
        button press=rename popovertarget="menu" popovertargetaction="hide" testId="menu-rename"
          text "Rename"
        button commandfor="sub" command="show-popover" testId="to-sub"
          text "More"
        button press=retarget commandfor=subName command="show-popover" testId="to-retargeted"
          text "Elsewhere"
        button press=keep commandfor="dpm" command="show-modal" testId="to-confirm"
          text "Ask first"
      column id="sub" popover="auto" role="menu"
        button press=keep popovertarget="sub" popovertargetaction="hide" testId="sub-keep"
          text "Keep"
      column id="sub2" popover="auto" role="menu"
        button press=keep popovertarget="sub2" popovertargetaction="hide" testId="sub2-keep"
          text "Keep there"
      dialog id="dpm" popover="auto" role="alertdialog" aria-label="Keep it?"
        text "Modally."
        button press=keep commandfor="dpm" command="close" testId="dpm-keep"
          text "Keep"
        button commandfor="dpm" command="close" testId="dpm-cancel"
          text "Cancel"
"#;

struct Nothing;
impl SurfacePainter for Nothing {
    fn record(&mut self, _: (f32, f32), _: f32, _: &mut dyn FnMut(&mut dyn Backend)) -> Vec<u32> {
        Vec::new()
    }
}

fn windowed() -> Presenter<()> {
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(APP),
        (),
        (400., 800.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.serve_requests();
    p.serve_windows(Box::new(Nothing));
    p
}

fn id(p: &Presenter<()>, name: &str) -> ViewId {
    let kernel = p.host().kernel();
    kernel
        .node_by_key(kernel.find_by_test_id(name)[0])
        .unwrap()
        .id
}

/// The `id` of the open surface's node, if one is open.
fn open(p: &Presenter<()>) -> Option<String> {
    let shown = p.surface()?;
    let node = p.host().kernel().node(shown.node)?;
    node.props.str(PropId::Id).map(str::to_string)
}

/// Press the button `name` as an invoker is pressed, and run what its action issued.
fn invoke(p: &mut Presenter<()>, name: &str) {
    let view = id(p, name);
    p.invoke(view, 0.0, true);
    p.run_commands(|| ());
}

#[test]
fn a_commandfor_that_names_nothing_falls_back_and_hide_is_any_case() {
    let mut p = windowed();
    invoke(&mut p, "dangling");
    assert_eq!(open(&p).as_deref(), Some("menu"), "the popovertarget acts");
    invoke(&mut p, "hide-upper");
    assert_eq!(open(&p), None, "HIDE hides");
}

#[test]
fn a_button_its_handler_disabled_invokes_nothing() {
    let mut p = windowed();
    invoke(&mut p, "self-disable");
    assert_eq!(p.host().runner().slot("off"), Some(&Value::Bool(true)));
    assert_eq!(open(&p), None);
}

#[test]
fn popover_commands_leave_a_dialog_open_modally_alone() {
    let mut p = windowed();
    invoke(&mut p, "open-dpm");
    assert_eq!(open(&p).as_deref(), Some("dpm"));
    p.popover_command("hidePopover", &[Value::str("dpm")]);
    p.popover_command("togglePopover", &[Value::str("dpm")]);
    assert_eq!(
        open(&p).as_deref(),
        Some("dpm"),
        "popover commands leave a modal dialog open"
    );
    p.close_modal(&[Value::str("dpm")]);
    assert_eq!(open(&p), None, "close(id) closes it");
    // Opened as a popover, its close rows do not close it: it is no alert.
    p.popover_command("showPopover", &[Value::str("dpm")]);
    assert_eq!(open(&p), None);
}

/// Touch the open surface's row `name` and lift there.
fn choose(p: &mut Presenter<()>, name: &str) {
    let row = id(p, name);
    let shown = p.surface().expect("a surface");
    let serial = shown.serial;
    let (x, y, w, h) = (shown.targets.iter().find(|t| t.node == row))
        .expect("a row for it")
        .rect;
    for action in [0, 1] {
        p.surface_event(serial, action, x + w / 2.0, y + h / 2.0, 0.0);
    }
    p.run_commands(|| ());
}

#[test]
fn a_menu_row_opens_a_submenu_it_names_by_command_alone() {
    let mut p = windowed();
    p.popover_command("showPopover", &[Value::str("menu")]);
    choose(&mut p, "to-sub");
    assert_eq!(open(&p).as_deref(), Some("sub"));
}

#[test]
fn a_menu_rows_submenu_is_the_one_it_opens_after_its_press() {
    let mut p = windowed();
    p.popover_command("showPopover", &[Value::str("menu")]);
    choose(&mut p, "to-retargeted");
    assert_eq!(
        open(&p).as_deref(),
        Some("sub2"),
        "its handler retargeted it"
    );
}

#[test]
fn a_button_its_handler_gives_a_target_invokes_it() {
    let mut p = windowed();
    let button = id(&p, "arm");
    p.activate(button, 0.0);
    p.run_commands(|| ());
    assert_eq!(open(&p).as_deref(), Some("menu"));
}

#[test]
fn enter_on_a_command_only_invoker_invokes_it() {
    let mut p = windowed();
    let button = id(&p, "native-cmd");
    assert_eq!(p.set_focus(Some(button), 0.0), None);
    p.key_down("Enter", 1.0);
    p.run_commands(|| ());
    assert_eq!(open(&p).as_deref(), Some("menu"));
}

#[test]
fn a_tap_naming_a_wrapper_does_not_press_the_invoker_inside_it() {
    let mut p = windowed();
    let wrapper = id(&p, "wrapper");
    let result = p.tap(wrapper);
    assert!(
        result.as_ref().is_err_and(|e| e.contains("would press")),
        "{result:?}"
    );
    assert_eq!(open(&p), None);
}

#[test]
fn a_menu_row_opens_a_confirmation_by_show_modal() {
    let mut p = windowed();
    p.popover_command("showPopover", &[Value::str("menu")]);
    choose(&mut p, "to-confirm");
    assert_eq!(
        p.host().runner().slot("said"),
        Some(&Value::str("kept")),
        "its press ran"
    );
    assert_eq!(
        open(&p).as_deref(),
        Some("dpm"),
        "then the confirmation, modally"
    );
    p.popover_command("hidePopover", &[Value::str("dpm")]);
    assert_eq!(open(&p).as_deref(), Some("dpm"));
}

#[test]
fn the_system_back_activates_an_authored_back_control_as_a_press_does() {
    let mut p = windowed();
    assert!(p.system_back(0.0));
    p.run_commands(|| ());
    assert_eq!(open(&p).as_deref(), Some("dpm"));
}
