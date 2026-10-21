//! A TV remote's D-pad (LLP 1115.001.003 D3, D4): on a device with no
//! pointer, the arrows move the focus by Android's `FocusFinder` rules and
//! Select presses it; a phone's arrows do not.
use super::*;

/// Buttons placed by hand: `x` is right of `a` in its row, `y` is nearer but
/// below the row, `minus` (a negative `tabindex`) and `hidden` sit between
/// `a` and `z`, below it.
const PLACED: &str = r#"component App
  state log = ""
  state on = false
  action pressA
    log = `${log}a;`
  action flip(checked: bool)
    on = checked
  action goBack
    log = `${log}back;`
  action heard(key: string)
    log = `${log}${key};`
    if key == "ArrowUp"
      preventDefault()
  view
    box position="relative" width=600 height=400
      button "A" press=pressA key=heard testId="a" position="absolute" left=0 top=0 width=100 height=40
      button "X" testId="x" position="absolute" left=300 top=0 width=100 height=40
      button "Y" testId="y" position="absolute" left=160 top=60 width=100 height=40
      box tabindex=-1 testId="minus" position="absolute" left=0 top=60 width=100 height=40
      box tabindex=0 display="none" testId="hidden" position="absolute" left=0 top=120 width=100 height=40
      button "Z" key=heard testId="z" position="absolute" left=0 top=180 width=100 height=40
      input type="checkbox" checked=on change=flip aria-label="On" testId="check" position="absolute" left=300 top=180
      text log testId="log" position="absolute" left=0 top=360
      button "Back" press=goBack aria-keyshortcuts="Escape" testId="back" position="absolute" left=500 top=340 width=80 height=40
"#;

fn boot(tv: bool) -> Presenter<()> {
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(PLACED),
        (),
        (600., 400.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    let device = Device {
        night: false,
        font_scale: 1.0,
        locale: "en-US".into(),
        rtl: false,
        hour24: false,
        reduced_motion: false,
        bold_text: false,
        high_contrast: false,
        touch: !tv,
        tv,
        touch_slop: 8.0,
        dates: String::new(),
    };
    assert!(p.set_device(&device).is_none());
    p.advance(16.);
    p
}

fn id(p: &Presenter<()>, test_id: &str) -> ViewId {
    let k = p.host().kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

fn focused(p: &Presenter<()>) -> String {
    let k = p.host().kernel();
    p.focus()
        .and_then(|f| k.node(f))
        .and_then(|n| n.props.str(PropId::TestId).map(str::to_owned))
        .unwrap_or_default()
}

fn text(p: &Presenter<()>, test_id: &str) -> String {
    let k = p.host().kernel();
    let node = k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap();
    node.props.str(PropId::Text).unwrap_or_default().to_owned()
}

/// A remote's key, down and up, as the reader sends it.
fn press(p: &mut Presenter<()>, code: &str, key: &str) {
    p.hardware_key(code, key, true, false);
    p.hardware_key(code, key, false, false);
    p.advance(16.);
}

fn arrow(p: &mut Presenter<()>, key: &str) -> String {
    press(p, key, key);
    focused(p)
}

#[test]
fn a_tv_reports_no_pointer_and_no_hover() {
    let p = boot(true);
    let viewport = p.host().runner().viewport().preferences;
    assert_eq!(viewport.pointer, exact_runner::Pointer::None);
    assert_eq!(viewport.hover, exact_runner::Hover::None);
}

#[test]
fn from_no_focus_an_arrow_takes_the_first_stop() {
    let mut p = boot(true);
    assert_eq!(p.focus(), None);
    assert_eq!(arrow(&mut p, "ArrowDown"), "a");
}

#[test]
fn across_the_row_its_own_node_beats_a_nearer_one_below_it() {
    let mut p = boot(true);
    p.set_focus(Some(id(&p, "a")), 0.);
    assert_eq!(arrow(&mut p, "ArrowRight"), "x", "the beam wins across");
    assert_eq!(arrow(&mut p, "ArrowLeft"), "a");
}

#[test]
fn down_skips_a_negative_tabindex_and_a_hidden_node() {
    let mut p = boot(true);
    p.set_focus(Some(id(&p, "a")), 0.);
    // `minus` is straight below `a` and as near as `y`: as a stop it would
    // win. Skipped, the near diagonal `y` beats `z`, straight down but far
    // (up and down, FocusFinder weighs the distance).
    assert_eq!(arrow(&mut p, "ArrowDown"), "y");
}

#[test]
fn at_an_edge_the_focus_stays() {
    let mut p = boot(true);
    p.set_focus(Some(id(&p, "a")), 0.);
    assert_eq!(arrow(&mut p, "ArrowLeft"), "a", "no wrap");
}

#[test]
fn a_handler_that_prevents_an_arrow_keeps_the_focus() {
    let mut p = boot(true);
    p.set_focus(Some(id(&p, "z")), 0.);
    assert_eq!(arrow(&mut p, "ArrowUp"), "z");
    assert!(text(&p, "log").contains("ArrowUp;"), "{}", text(&p, "log"));
}

#[test]
fn select_presses_the_focus_and_toggles_a_checkbox() {
    let mut p = boot(true);
    p.set_focus(Some(id(&p, "a")), 0.);
    press(&mut p, "Select", "Enter");
    assert!(text(&p, "log").ends_with("Enter;a;"), "{}", text(&p, "log"));
    p.set_focus(Some(id(&p, "check")), 0.);
    press(&mut p, "Select", "Enter");
    let k = p.host().kernel();
    let check = k.node_by_key(k.find_by_test_id("check")[0]).unwrap();
    assert_eq!(check.props.bool(PropId::Checked), Some(true));
}

#[test]
fn a_phones_arrows_move_no_focus() {
    let mut p = boot(false);
    p.set_focus(Some(id(&p, "a")), 0.);
    assert_eq!(arrow(&mut p, "ArrowRight"), "a");
}

#[test]
fn a_tvs_back_presses_the_button_that_answers_escape() {
    let mut p = boot(true);
    assert!(
        p.back_goes(),
        "the reader takes Back while that button shows"
    );
    assert!(p.system_back(0.));
    p.advance(16.);
    assert!(text(&p, "log").ends_with("back;"), "{}", text(&p, "log"));
}

#[test]
fn a_phones_back_leaves_that_button_alone() {
    let mut p = boot(false);
    assert!(!p.back_goes());
    assert!(!p.system_back(0.));
    assert!(!text(&p, "log").contains("back;"));
}

/// Two radios of one group in a row, then a button.
const RADIOS: &str = r#"component App
  state pick = "a"
  action choose(value: string)
    pick = value
  view
    row width=600 height=100 gap=20
      input type="radio" name="g" value="a" checked=(pick == "a") change=choose testId="ra" width=40 height=40
      input type="radio" name="g" value="b" checked=(pick == "b") change=choose testId="rb" width=40 height=40
      button "Go" testId="go" width=100 height=40
"#;

#[test]
fn the_remote_reaches_every_radio_of_a_group_and_select_checks_one() {
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(RADIOS),
        (),
        (600., 400.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    let mut tv = boot(true).device().unwrap().clone();
    tv.tv = true;
    assert!(p.set_device(&tv).is_none());
    p.advance(16.);
    p.set_focus(Some(id(&p, "ra")), 0.);
    press(&mut p, "Select", "Enter");
    assert_eq!(arrow(&mut p, "ArrowRight"), "rb", "the unchecked option");
    press(&mut p, "Select", "Enter");
    let k = p.host().kernel();
    let rb = k.node_by_key(k.find_by_test_id("rb")[0]).unwrap();
    assert_eq!(rb.props.bool(PropId::Checked), Some(true));
    assert_eq!(arrow(&mut p, "ArrowRight"), "go");
}

#[test]
fn a_held_select_presses_once_and_toggles_once() {
    let mut p = boot(true);
    let held = |p: &mut Presenter<()>| {
        p.hardware_key("Select", "Enter", true, false);
        p.hardware_key("Select", "Enter", true, true);
        p.hardware_key("Select", "Enter", true, true);
        p.hardware_key("Select", "Enter", false, false);
        p.advance(16.);
    };
    p.set_focus(Some(id(&p, "a")), 0.);
    held(&mut p);
    assert_eq!(
        text(&p, "log").matches("a;").count(),
        1,
        "{}",
        text(&p, "log")
    );
    p.set_focus(Some(id(&p, "check")), 0.);
    held(&mut p);
    let k = p.host().kernel();
    let check = k.node_by_key(k.find_by_test_id("check")[0]).unwrap();
    assert_eq!(
        check.props.bool(PropId::Checked),
        Some(true),
        "toggled once"
    );
}

#[test]
fn a_held_arrow_keeps_moving_the_focus() {
    let mut p = boot(true);
    p.set_focus(Some(id(&p, "a")), 0.);
    p.hardware_key("ArrowRight", "ArrowRight", true, false);
    assert_eq!(focused(&p), "x");
    p.hardware_key("ArrowLeft", "ArrowLeft", true, true);
    assert_eq!(focused(&p), "a", "a repeat moves too");
}
