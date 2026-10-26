//! The Material look's controls (LLP 1115.001 D9, S2): a checkbox, a radio
//! and a filled button take Material's size and colours, and what the
//! author wrote wins; each takes a touch over 48 dp, apart from its layout
//! (LLP 1115.001.001 §5 Q5). The
//! look is the process's (`app::show_material_look`), so these run in a
//! child process of their own.
use super::*;

const APP: &str = r##"component App
  view
    column padding=10 align-items="flex-start"
      input type="checkbox" checked=true testId="on"
      input type="checkbox" testId="off"
      input type="checkbox" checked=true disabled=true testId="faded"
      button buttonStyle="filled" testId="save"
        text "Save"
      button buttonStyle="filled" -exact-control-size="large" testId="large"
        text "Save"
      button buttonStyle="filled" disabled=true accent-color="#ff0000" testId="red"
        text "Save"
      button buttonStyle="filled" -exact-corner-style="small" testId="small"
        text "Save"
      button buttonStyle="filled" disabled=true testId="quiet"
        text "Save"
        text "Later"
"##;

/// Whether this is the child process that shows the Material look; the
/// parent runs the test there and returns false.
fn material_process() -> bool {
    const CHILD: &str = "EXACT_MATERIAL_LOOK_TEST";
    if std::env::var_os(CHILD).is_some() {
        crate::app::show_material_look();
        return true;
    }
    let name = std::thread::current().name().unwrap().to_owned();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &name, "--nocapture"])
        .env(CHILD, "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    false
}

fn boot() -> Presenter<()> {
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(APP),
        (),
        (400., 600.),
        2.4375,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.frame();
    p
}

fn id(p: &Presenter<()>, name: &str) -> ViewId {
    let k = p.host().kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}

/// The colour at a point of the frame, in points.
fn pixel(p: &mut Presenter<()>, x: f32, y: f32) -> [u8; 4] {
    let frame = p.frame();
    let at = |v: f32| (v * 2.4375) as u32;
    let c = frame.pixel(at(x), at(y)).unwrap().demultiply();
    [c.red(), c.green(), c.blue(), c.alpha()]
}

#[test]
fn materials_checkbox_and_filled_button_take_its_size_and_colours() {
    if !material_process() {
        return;
    }
    let mut p = boot();
    let on = p.rect_of(id(&p, "on")).unwrap();
    assert_eq!(
        (on.2, on.3),
        (24.0, 24.0),
        "the mark-size row's Material 24: the 20 drawn and 2 round it"
    );
    // Checked: the accent's fill above the check, and the page in the 2
    // round it; unchecked: the page shows inside its outline.
    let accent = crate::paint::control::accents(None, false).0;
    let page = pixel(&mut p, 2.0, 2.0);
    assert_eq!(pixel(&mut p, on.0 + 12.0, on.1 + 5.0), accent);
    assert_eq!(pixel(&mut p, on.0 + 1.0, on.1 + 12.0), page, "its padding");
    let off = p.rect_of(id(&p, "off")).unwrap();
    assert_eq!(pixel(&mut p, off.0 + 12.0, off.1 + 12.0), page);
    assert_ne!(
        pixel(&mut p, off.0 + 2.5, off.1 + 12.0),
        page,
        "its outline"
    );
    // Disabled: no longer the accent.
    let faded = p.rect_of(id(&p, "faded")).unwrap();
    assert_ne!(pixel(&mut p, faded.0 + 12.0, faded.1 + 5.0), accent);
    // A size reported before (a switch that became a checkbox) is forgotten.
    let view = id(&p, "off");
    assert!(p.host.set_intrinsic(view, Some((38.0, 22.0))).is_none());
    p.size_controls();
    p.frame();
    let off = p.rect_of(view).unwrap();
    assert_eq!((off.2, off.3), (24.0, 24.0));
    // The filled button: Material's 40 tall, at least 58 wide, a pill in
    // the accent.
    let save = p.rect_of(id(&p, "save")).unwrap();
    assert_eq!(save.3, 40.0);
    assert!(save.2 >= 58.0, "{save:?}");
    assert_eq!(pixel(&mut p, save.0 + 12.0, save.1 + 20.0), accent);
    assert_eq!(
        pixel(&mut p, save.0 + 1.0, save.1 + 1.0),
        page,
        "a pill's corner"
    );
    // A larger control size is a larger button.
    let large = p.rect_of(id(&p, "large")).unwrap();
    assert!(large.3 > save.3 && large.2 > save.2, "{large:?} {save:?}");
    // An authored accent stays when disabled; an authored corner style wins.
    let red = p.rect_of(id(&p, "red")).unwrap();
    assert_eq!(pixel(&mut p, red.0 + 12.0, red.1 + 20.0), [255, 0, 0, 255]);
    let small = p.rect_of(id(&p, "small")).unwrap();
    assert_ne!(
        pixel(&mut p, small.0 + 1.5, small.1 + 1.5),
        page,
        "small corners"
    );
    // Disabled, the subtitle is fainter than the title, not stronger.
    let quiet = p.rect_of(id(&p, "quiet")).unwrap();
    let frame = (*p.frame()).clone();
    let ink = |from: f32, to: f32| {
        let at = |v: f32| (v * 2.4375) as u32;
        let mut darkest = 255u8;
        for y in at(from)..at(to) {
            for x in at(quiet.0 + 20.0)..at(quiet.0 + quiet.2 - 20.0) {
                darkest = darkest.min(frame.pixel(x, y).unwrap().demultiply().red());
            }
        }
        darkest
    };
    let middle = quiet.1 + quiet.3 / 2.0;
    assert!(ink(middle, quiet.1 + quiet.3 - 6.0) >= ink(quiet.1 + 6.0, middle));
}

const MARKS: &str = r##"component App
  state on = false
  state picked = false
  state ghost = false
  action flip(value)
    on = value
  action haunt(value)
    ghost = value
  action pick
    picked = true
  view
    column padding=16 align-items="flex-start"
      input type="radio" name="r" checked=true testId="chosen"
      input type="radio" name="r" testId="other"
      row align-items="flex-start"
        input type="checkbox" checked=on change=flip testId="near"
        button press=pick testId="beside"
          text "Beside"
      text `${on} ${picked} ${ghost}` testId="said"
      box height=40
      input type="checkbox" checked=ghost change=haunt visibility="hidden" testId="hidden"
      box height=40
      input type="checkbox" width=40 height=40 checked=true testId="large"
      box height=40
      column position="relative"
        input type="checkbox" checked=ghost change=haunt testId="covered"
        box position="absolute" inset=-12 background-color="#808080" testId="cover"
      box height=40
      column disabled=true width=24 height=24
        input type="checkbox" checked=ghost change=haunt testId="quelled"
      box height=40
      column position="relative"
        input type="checkbox" checked=ghost change=haunt testId="halved"
        box position="absolute" top=-12 left=-12 width=20 height=48 background-color="#808080" testId="half"
      box height=40
      row width=60 testId="wrapper"
        input type="checkbox" checked=ghost change=haunt testId="wrapped"
      box height=40
      column rotate="45deg" overflow="hidden"
        input type="checkbox" checked=ghost change=haunt testId="turned"
"##;

fn said(p: &Presenter<()>) -> String {
    let k = p.host().kernel();
    let node = k.node_by_key(k.find_by_test_id("said")[0]).unwrap();
    node.props.str(PropId::Text).unwrap_or_default().to_owned()
}

/// A finger down and up at (`x`, `y`), then a frame.
fn press(p: &mut Presenter<()>, x: f32, y: f32, at: f64) {
    p.pointer_down(x, y, at).unwrap();
    p.pointer_up(x, y, at + 50.0).unwrap();
    p.frame();
}

#[test]
fn materials_radio_and_touch_target_reach_past_the_drawn_control() {
    if !material_process() {
        return;
    }
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(MARKS),
        (),
        (400., 700.),
        2.4375,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    // A finger, as the reader's device says.
    let phone = Device {
        night: false,
        font_scale: 1.0,
        locale: "en-US".into(),
        rtl: false,
        hour24: false,
        reduced_motion: false,
        bold_text: false,
        high_contrast: false,
        touch: true,
        tv: false,
        touch_slop: 8.0,
        dates: String::new(),
    };
    assert!(p.set_device(&phone).is_none());
    p.set_pointer_kind(crate::presenter::PointerKind::Touch);
    p.frame();
    // A radio is Material's: 24 laid out, its dot the accent, the page
    // inside an unselected ring.
    let accent = crate::paint::control::accents(None, false).0;
    let page = pixel(&mut p, 2.0, 2.0);
    let chosen = p.rect_of(id(&p, "chosen")).unwrap();
    assert_eq!((chosen.2, chosen.3), (24.0, 24.0));
    assert_eq!(pixel(&mut p, chosen.0 + 12.0, chosen.1 + 12.0), accent);
    let other = p.rect_of(id(&p, "other")).unwrap();
    assert_eq!(pixel(&mut p, other.0 + 12.0, other.1 + 12.0), page);
    // A press 10 below a checkbox's box, in nothing that takes a press,
    // checks it; what is hit there is still the page's, for hover and
    // scrolling.
    let near = p.rect_of(id(&p, "near")).unwrap();
    let below = (near.0 + 12.0, near.1 + near.3 + 10.0);
    assert_ne!(p.hit(below.0, below.1), Some(id(&p, "near")));
    press(&mut p, below.0, below.1, 1000.0);
    assert_eq!(said(&p), "true false false", "the press below checked it");
    // A press in the button beside it, inside its touch box, is the
    // button's.
    let beside = p.rect_of(id(&p, "beside")).unwrap();
    assert!(
        beside.0 < near.0 + near.2 + 12.0,
        "in the checkbox's touch box: {beside:?}"
    );
    press(&mut p, beside.0 + 1.0, beside.1 + 12.0, 2000.0);
    assert_eq!(
        said(&p),
        "true true false",
        "the button's, not the checkbox's"
    );
    // A hidden checkbox takes no press round it.
    let hidden = p.rect_of(id(&p, "hidden")).unwrap();
    press(&mut p, hidden.0 + 12.0, hidden.1 - 6.0, 3000.0);
    assert_eq!(
        said(&p),
        "true true false",
        "nothing reached the hidden one"
    );
    // A checkbox under a cover, one half covered pressed on its cover, one
    // in a disabled column, one turned, and one an agent taps beside by
    // naming its wrapper take nothing.
    let cover = p.rect_of(id(&p, "cover")).unwrap();
    press(&mut p, cover.0 + 2.0, cover.1 + 2.0, 4000.0);
    let quelled = p.rect_of(id(&p, "quelled")).unwrap();
    press(
        &mut p,
        quelled.0 + 12.0,
        quelled.1 + quelled.3 + 6.0,
        5000.0,
    );
    let half = p.rect_of(id(&p, "half")).unwrap();
    press(&mut p, half.0 + 16.0, half.1 + 24.0, 5500.0);
    // Turned, outside its clip but inside the square round it.
    let turned = p.rect_of(id(&p, "turned")).unwrap();
    let (cx, cy) = (turned.0 + turned.2 / 2.0, turned.1 + turned.3 / 2.0);
    press(&mut p, cx + 15.0, cy + 15.0, 5700.0);
    let wrapper = id(&p, "wrapper");
    let _ = p.tap(wrapper);
    p.frame();
    assert_eq!(
        said(&p),
        "true true false",
        "a covered, a disabled, a turned or an unnamed control took a press"
    );
    // An authored size draws the mark across its box.
    let large = p.rect_of(id(&p, "large")).unwrap();
    assert_eq!((large.2, large.3), (40.0, 40.0));
    assert_eq!(pixel(&mut p, large.0 + 20.0, large.1 + 4.0), accent);
}
