//! The Linux host's input and paint against the web's rules: the events
//! beyond press and change reach their handlers, a transparent parent keeps
//! its children's hit boxes, a password paints bullets, and the store's write
//! log does not outlive its commit.
use super::*;
use exact_runner::{Answer, DataError, Store, Value};

#[derive(Default)]
struct Keeps;
impl DataSource for Keeps {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
    fn grants(&self) -> &'static str {
        "secret.keep token\n"
    }
    fn answer(&mut self, store: &mut Store, name: &str, _: &[Value]) -> Result<Answer, DataError> {
        assert_eq!(name, "keep");
        store.set("token", "a secret").unwrap();
        Ok(Answer::Now(Value::Bool(true)))
    }
}

const APP: &str = r#"component App
  state text = ""
  state commits = 0
  state saved = ""
  state focuses = 0
  state blurs = 0
  state submits = 0
  state lastKey = ""
  state presses = 0
  state outerKeys = ""
  mutation kept as shape bool
  action edit(value)
    text = value
  action committed(value)
    commits = commits + 1
    saved = value
  action focused
    focuses = focuses + 1
  action blurred
    blurs = blurs + 1
  action sent
    submits = submits + 1
  action keyed(value)
    lastKey = value
    if value == "Escape"
      stopPropagation()
  action outer(value: string)
    outerKeys = `${outerKeys}${value}`
  action pressed
    presses = presses + 1
  action keep
    send kept = keep()
  action goField
    focus("entry")
  view
    column width=400 height=400 key=outer
      text `${commits}:${saved}` testId="commits" height=20
      text outerKeys testId="outer" height=20
      input value=text input=edit change=committed submit=sent focus=focused blur=blurred key=keyed testId="field" id="entry" height=32
      button "Focus" press=goField testId="go-field" height=32
      button "Other" press=pressed testId="other" height=32
      box opacity=0 width=200 height=40
        button "Ghost" press=pressed testId="ghost" width=200 height=40
      button "Keep" press=keep testId="keep" height=32
      input value="abc" type="password" testId="secret" width=200 height=32
      input value="•••" testId="shown" width=200 height=32
      text `${focuses} ${blurs} ${submits} ${lastKey} ${presses}` testId="log" height=20
"#;

fn boot() -> Presenter<Keeps> {
    let (p, error) = Presenter::boot_with(
        &contract::compile(APP).unwrap().encode(),
        Keeps,
        (400., 400.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p
}
fn id<D: DataSource>(p: &Presenter<D>, test_id: &str) -> ViewId {
    let k = p.host().kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}
fn log<D: DataSource>(p: &Presenter<D>) -> String {
    let k = p.host().kernel();
    let node = k.node_by_key(k.find_by_test_id("log")[0]).unwrap();
    node.props.str(PropId::Text).unwrap().to_string()
}

#[test]
fn cursor_resolves_inheritance_hud_override_stationary_changes_and_unmount() {
    let source = r#"component CursorTest
  state armed = false
  state shown = true
  action arm
    armed = !armed
  action hide
    shown = false
  view
    column width=200 height=200
      button "Arm" testId="arm" press=arm height=30
      button "Hide" testId="hide" press=hide height=30
      when shown
        column cursor=(armed ? "crosshair" : "default") width=200 height=100
          button "HUD" testId="hud" cursor="auto" press=arm width=50 height=30
          box testId="zone" width=200 height=70
"#;
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(source).unwrap().encode(),
        Keeps,
        (200., 200.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    let zone = id(&p, "zone");
    let (x, y, w, h) = p.rect_of(zone).unwrap();
    let point = (x + w / 2., y + h / 2.);
    p.pointer_move(point.0, point.1, 0.).unwrap();
    assert_eq!(p.cursor_at(point.0, point.1).name(), "default");
    p.tap(id(&p, "arm")).unwrap();
    assert_eq!(p.cursor_at(point.0, point.1).name(), "crosshair");
    let (x, y, w, h) = p.rect_of(id(&p, "hud")).unwrap();
    assert_eq!(p.cursor_at(x + w / 2., y + h / 2.).name(), "auto");
    p.tap(id(&p, "arm")).unwrap();
    assert_eq!(p.cursor_at(point.0, point.1).name(), "default");
    p.tap(id(&p, "arm")).unwrap();
    p.tap(id(&p, "hide")).unwrap();
    assert_eq!(p.cursor_at(point.0, point.1).name(), "auto");
}

#[test]
fn keys_submit_focus_and_blur_reach_their_handlers() {
    let mut p = boot();
    let field = id(&p, "field");
    p.type_key(field, "KeyA", "a", true, false).unwrap();
    assert_eq!(
        log(&p),
        "1 0 0 a 0",
        "focus, then the key, then the typed character"
    );
    p.type_key(field, "Enter", "Enter", true, false).unwrap();
    assert_eq!(
        log(&p),
        "1 0 1 Enter 0",
        "Enter at a single-line input submits it"
    );
    let k = p.host().kernel();
    let value = k
        .node(field)
        .unwrap()
        .props
        .str(PropId::Value)
        .unwrap()
        .to_string();
    assert_eq!(value, "a", "Enter is not text in a single-line input");
    p.tap(id(&p, "other")).unwrap();
    assert_eq!(
        log(&p),
        "1 1 1 Enter 1",
        "a press elsewhere blurs the field"
    );
}

/// `stopPropagation()` (files diary F8): the field's handler is the last
/// to hear the key; an ancestor's hears every other.
#[test]
fn a_stopped_key_reaches_no_ancestor() {
    let mut p = boot();
    let field = id(&p, "field");
    p.type_key(field, "KeyA", "a", true, false).unwrap();
    p.type_key(field, "Escape", "Escape", true, false).unwrap();
    p.type_key(field, "KeyB", "b", true, false).unwrap();
    let k = p.host().kernel();
    let outer = k.node_by_key(k.find_by_test_id("outer")[0]).unwrap();
    assert_eq!(outer.props.str(PropId::Text), Some("ab"));
    assert!(
        log(&p).contains(" b "),
        "the field heard them all: {}",
        log(&p)
    );
}

#[test]
fn focus_from_an_action_moves_the_focus() {
    let mut p = boot();
    p.tap(id(&p, "go-field")).unwrap();
    p.run_commands(|| Keeps);
    assert_eq!(
        p.focus(),
        Some(id(&p, "field")),
        "focus(\"entry\") is the field's"
    );
    assert!(
        log(&p).starts_with("1 0 "),
        "its focus handler ran: {}",
        log(&p)
    );
}

#[test]
fn a_transparent_parent_keeps_its_childs_hit_box() {
    let mut p = boot();
    p.tap(id(&p, "ghost")).unwrap();
    assert!(
        log(&p).ends_with(" 1"),
        "opacity is paint only: {}",
        log(&p)
    );
}

#[test]
fn a_password_paints_one_bullet_a_character() {
    let mut p = boot();
    let pixmap = p.frame();
    let rect = |p: &mut Presenter<Keeps>, test_id: &str| p.rect_of(id(p, test_id)).unwrap();
    let (secret, shown) = (rect(&mut p, "secret"), rect(&mut p, "shown"));
    assert_eq!((secret.0, secret.2, secret.3), (shown.0, shown.2, shown.3));
    let row = |y: f32, dy: usize| {
        let y = y as usize + dy;
        (secret.0 as usize..(secret.0 + secret.2) as usize)
            .map(|x| pixmap.pixel(x as u32, y as u32).unwrap().red())
            .collect::<Vec<_>>()
    };
    let inked = (0..secret.3 as usize)
        .filter(|&dy| row(secret.1, dy).iter().any(|&r| r < 200))
        .count();
    assert!(inked > 0, "the password paints something");
    for dy in 0..secret.3 as usize {
        assert_eq!(
            row(secret.1, dy),
            row(shown.1, dy),
            "row {dy}: \"abc\" masked is \"•••\""
        );
    }
}

#[test]
fn store_writes_are_dropped_with_their_commit() {
    let mut p = boot();
    p.tap(id(&p, "keep")).unwrap();
    let kept = |p: &Presenter<Keeps>| {
        p.host()
            .runner()
            .store_names()
            .contains(&"token".to_string())
    };
    for _ in 0..100 {
        if kept(&p) {
            break;
        }
        p.pump(p.host().now());
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(kept(&p), "the memory store keeps the value");
    assert!(
        p.host.take_store_writes_for_test().is_empty(),
        "the write log went with its commit"
    );
}

fn press_fixture() -> Presenter<Keeps> {
    let source = APP.replace(
        "height=32\n      box opacity",
        "width=100 height=32 scale=1.5 -exact-press-scale=0.5 transform-origin=\"0 0\"\n      box opacity",
    );
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(&source).unwrap().encode(),
        Keeps,
        (400., 400.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    assert!(p
        .set_preferences(exact_runner::Preferences {
            reduced_motion: true,
            ..Default::default()
        })
        .is_none());
    p
}

#[test]
fn a_held_press_multiplies_scale_about_the_origin_even_under_reduced_motion() {
    let mut p = press_fixture();
    let button = id(&p, "other");
    let before = p.rect_of(button).unwrap();
    let (x, y) = (before.0 + 20., before.1 + 10.);
    assert!(p.pointer_down(x, y, 0.).unwrap());
    p.clock(120.);
    let held = p.rect_of(button).unwrap();
    assert_eq!(held, (before.0, before.1, before.2 * 0.5, before.3 * 0.5));
    // Moving off and back inside the original edge keeps the contact alive.
    p.pointer_move(before.0 + before.2 + 5., y, 120.).unwrap();
    p.clock(240.);
    assert_eq!(p.rect_of(button).unwrap(), before);
    p.pointer_move(before.0 + before.2 - 2., y, 240.).unwrap();
    p.clock(360.);
    assert_eq!(p.rect_of(button).unwrap(), held);
    p.pointer_up(before.0 + before.2 - 2., y, 360.).unwrap();
    p.clock(480.);
    assert_eq!(p.rect_of(button).unwrap(), before);
    assert!(
        log(&p).ends_with(" 1"),
        "the release still activates the original button"
    );
}

#[test]
fn a_repress_during_release_keeps_the_original_hit_box_and_cancel_never_activates() {
    let mut p = press_fixture();
    let button = id(&p, "other");
    let before = p.rect_of(button).unwrap();
    let (x, y) = (before.0 + 20., before.1 + 10.);
    p.pointer_down(x, y, 0.).unwrap();
    p.clock(120.);
    p.pointer_cancel(120.).unwrap();
    p.clock(150.);
    let releasing = p.rect_of(button).unwrap();
    assert!(releasing.2 > before.2 * 0.5 && releasing.2 < before.2);
    p.pointer_down(x, y, 150.).unwrap();
    assert_eq!(p.rect_of(button).unwrap(), releasing, "no jump on re-press");
    p.pointer_move(before.0 + before.2 - 2., y, 150.).unwrap();
    p.clock(270.);
    assert_eq!(p.rect_of(button).unwrap().2, before.2 * 0.5);
    p.pointer_cancel(270.).unwrap();
    p.clock(390.);
    assert_eq!(p.rect_of(button).unwrap(), before);
    assert!(!p.host.motion(), "the feedback needs no more frames");
    assert!(log(&p).ends_with(" 0"));
}

/// LLP 1005 §3, LLP 1056 §3 stage 3: the pointer's down, moves and up reach
/// the node hearing them with its record, from its content box; a free
/// pointer's moves too, with no button down.
#[test]
fn the_pointer_events_carry_their_record_from_the_content_box() {
    const PAD: &str = r#"component App
  state seen = ""
  action down(e: PointerEvent)
    seen = `${seen} d${e.offsetX},${e.offsetY}/${e.buttons}`
  action move(e: PointerEvent)
    seen = `${seen} m${e.offsetX},${e.offsetY}/${e.buttons}`
  action up(e: PointerEvent)
    seen = `${seen} u${e.offsetX},${e.offsetY}/${e.pressure}`
  view
    column width=400 height=400
      box height=40
      box pointerdown=down pointermove=move pointerup=up testId="pad" width=200 height=100 padding=10
        box testId="inner" width=50 height=50
      text seen testId="log" height=20
"#;
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(PAD).unwrap().encode(),
        Keeps,
        (400., 400.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.boxes();
    p.pointer_move(15., 55., 1.).unwrap();
    p.pointer_down(20., 60., 2.).unwrap();
    p.pointer_move(30., 70., 3.).unwrap();
    p.pointer_move(300., 300., 4.).unwrap();
    p.pointer_up(310., 290., 5.).unwrap();
    assert_eq!(
        log(&p),
        " m5,5/0 d10,10/1 m20,20/1 m290,250/1 u300,240/0",
        "a held pointer is the pad's wherever it goes"
    );
    // Any button holds the pointer, as in a browser (review b5-b 1): the
    // secondary's down, moves and up with `buttons` 2, a middle chord 6.
    p.pointer_move(20., 60., 6.).unwrap();
    p.pointer_aux(2, true, 20., 60., 7.);
    p.pointer_move(25., 65., 8.).unwrap();
    p.pointer_aux(4, true, 25., 65., 9.);
    p.pointer_move(26., 66., 10.).unwrap();
    p.pointer_aux(2, false, 26., 66., 11.);
    p.pointer_aux(4, false, 26., 66., 12.);
    assert!(
        log(&p).ends_with(" m10,10/0 d10,10/2 m15,15/2 m16,16/6 u16,16/0"),
        "{}",
        log(&p)
    );
}

/// A `wheel` is heard by a disabled box (it means nothing on a `<div>`, as
/// in Chrome) and a disabled link (an `<a>`), and not by a disabled button
/// (review b5-b 4, b5-delta).
#[test]
fn a_disabled_box_hears_the_wheel_and_a_disabled_button_does_not() {
    const WHEELS: &str = r#"component App
  state seen = ""
  action box(e: WheelEvent)
    seen = `${seen} box${e.deltaY}`
  action button(e: WheelEvent)
    seen = `${seen} button${e.deltaY}`
  action link(e: WheelEvent)
    seen = `${seen} link${e.deltaY}`
  view
    column width=400 height=400
      box wheel=box disabled=true testId="pad" width=200 height=100
      button "Off" wheel=button disabled=true testId="off" height=40
      link href="/docs" wheel=link disabled=true testId="docs" height=40
        text "Docs"
      text seen testId="log" height=20
"#;
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(WHEELS).unwrap().encode(),
        Keeps,
        (400., 400.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.boxes();
    let (x, y, _, _) = p.rect_of(id(&p, "pad")).unwrap();
    p.wheel_at(x + 10., y + 10., 0., 30.);
    let (x, y, _, _) = p.rect_of(id(&p, "off")).unwrap();
    p.wheel_at(x + 10., y + 10., 0., 40.);
    // A disabled link is the web's `<a>`: `disabled` means nothing there
    // either (review b5-delta).
    let (x, y, _, _) = p.rect_of(id(&p, "docs")).unwrap();
    p.wheel_at(x + 10., y + 10., 0., 50.);
    assert_eq!(log(&p), " box30 link50");
}

/// LLP 1051.000 D1 (changed 2026-10-04; the kanban diary's F4): `frame()`
/// reads the box where the viewer sees it, the scroller's offset applied.
#[test]
fn frame_reads_a_box_with_the_scroll_above_it_applied() {
    const PANE: &str = r#"component App
  state y = -1
  action read
    y = frame("card").y
  view
    column width=400 height=400
      button "Read" press=read testId="read" height=40
      scroll testId="pane" height=200
        box height=300
        box id="card" height=50
      text `${y}` testId="log" height=20
"#;
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(PANE).unwrap().encode(),
        Keeps,
        (400., 400.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.boxes();
    p.tap(id(&p, "read")).unwrap();
    assert_eq!(log(&p), "340");
    p.wheel(id(&p, "pane"), 0., 120.).unwrap();
    p.boxes();
    p.tap(id(&p, "read")).unwrap();
    assert_eq!(log(&p), "220", "the pane's 120 px of scrolling");
}

/// HTML's `tabindex` and Tab (LLP 1088 D7.3): an explicit value makes a plain
/// box focusable and, ≥ 0, a Tab stop, positive values first; a negative
/// one takes a tap and `autofocus` but Tab skips it; absent is never `0`;
/// a disabled button, inert and hidden boxes are skipped, and a disabled
/// box is not (`disabled` means nothing on a div, as in Chrome); a bound value moves a box
/// in and out; Tab and Shift-Tab walk and wrap, from no focus to the first
/// or the last; an ancestor's `key` hears a key the focused box bubbles.
#[test]
fn tabindex_makes_tab_stops_and_tab_walks_them() {
    const TABS: &str = r#"component App
  state open = false
  state keys = ""
  action reveal
    open = true
  action heard(key: string)
    keys = `${keys}${key};`
  view
    column width=400 height=400 key=heard
      text keys testId="keys" height=20
      box tabindex=0 testId="zero" width=40 height=20
      box testId="plain" width=40 height=20
      box tabindex=-1 autofocus=true testId="minus" width=40 height=20
      button "Reveal" press=reveal testId="reveal" height=20
      box tabindex=2 testId="two" width=40 height=20
      box tabindex=1 testId="one" width=40 height=20
      box tabindex=(open ? 0 : -1) testId="bound" width=40 height=20
      box tabindex=0 disabled=true testId="disabled" width=40 height=20
      button "Off" disabled=true tabindex=0 testId="off" height=20
      box inert=true
        box tabindex=0 testId="inert" width=40 height=20
      box tabindex=0 display="none" testId="hidden" width=40 height=20
"#;
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(TABS).unwrap().encode(),
        Keeps,
        (400., 400.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.advance(16.);
    assert_eq!(p.focus(), Some(id(&p, "minus")), "autofocus takes -1");
    let tab = |p: &mut Presenter<Keeps>, shift: bool| {
        if shift {
            p.hold_modifier("ShiftLeft", true);
        }
        p.hardware_key("Tab", "Tab", true, false);
        p.hardware_key("Tab", "Tab", false, false);
        if shift {
            p.hold_modifier("ShiftLeft", false);
        }
        let k = p.host().kernel();
        p.focus()
            .and_then(|f| k.node(f))
            .and_then(|n| n.props.str(PropId::TestId))
            .unwrap_or("")
            .to_string()
    };
    let walked: Vec<String> = (0..7).map(|_| tab(&mut p, false)).collect();
    assert_eq!(
        walked,
        ["one", "two", "", "zero", "reveal", "disabled", "one"],
        "positive first, then tree order (the column hears keys, so the web makes it a stop); \
         -1, plain, the disabled button, inert, hidden skipped; the disabled box is a stop; wraps"
    );
    assert_eq!(
        tab(&mut p, true),
        "disabled",
        "Shift-Tab walks back, wrapping"
    );
    let keys = |p: &Presenter<Keeps>| {
        let k = p.host().kernel();
        let n = k.node_by_key(k.find_by_test_id("keys")[0]).unwrap();
        n.props.str(PropId::Text).unwrap_or("").to_string()
    };
    assert!(
        keys(&p).contains("Tab;"),
        "the column heard Tab bubble: {}",
        keys(&p)
    );
    p.tap(id(&p, "reveal")).unwrap();
    assert_eq!(
        tab(&mut p, false),
        "bound",
        "a bound tabindex joins the order"
    );
    p.tap(id(&p, "minus")).unwrap();
    assert_eq!(p.focus(), Some(id(&p, "minus")), "a tap focuses -1");
    p.focus = None;
    assert_eq!(
        tab(&mut p, false),
        "one",
        "from no focus, Tab takes the first"
    );
    p.focus = None;
    assert_eq!(tab(&mut p, true), "disabled", "and Shift-Tab the last");
}

#[test]
fn agent_typing_commits_only_on_enter_or_blur() {
    let mut p = boot();
    let field = id(&p, "field");
    let committed = |p: &Presenter<Keeps>| {
        p.host()
            .kernel()
            .node(id(p, "commits"))
            .unwrap()
            .props
            .str(PropId::Text)
            .unwrap()
            .to_owned()
    };
    p.type_text(field, "first").unwrap();
    p.type_text(field, "second").unwrap();
    assert_eq!(committed(&p), "0:");
    assert_eq!(
        p.host()
            .kernel()
            .node(field)
            .unwrap()
            .props
            .str(PropId::Value),
        Some("second")
    );
    p.type_key(field, "Enter", "Enter", true, false).unwrap();
    assert_eq!(committed(&p), "1:second");
    p.type_key(field, "Enter", "Enter", false, false).unwrap();
    p.tap(id(&p, "other")).unwrap();
    assert_eq!(
        committed(&p),
        "1:second",
        "blur after Enter does not commit twice"
    );
    p.type_text(field, "third").unwrap();
    assert_eq!(committed(&p), "1:second");
    p.tap(id(&p, "other")).unwrap();
    assert_eq!(committed(&p), "2:third");
}

#[test]
fn an_unbound_field_holds_its_typed_text_until_its_bound_value_changes() {
    // LLP 1069.001 D4: an unbound control holds its own state; the web
    // build writes a field's `value` only when the bound value changes.
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(
            r#"component App
  state saved = ""
  state kept = "kept"
  state loud = ""
  action save(next: string)
    saved = next
  action refuse(next: string)
    saved = saved
  action shout(next: string)
    loud = next + "!"
  action swap()
    kept = kept == "kept" ? "other" : "kept"
  view
    column
      input change=save testId="free"
      input value="x" change=save testId="literal"
      button "swap" press=swap testId="swap"
      input value=kept input=refuse testId="refused"
      input value=loud input=shout testId="loud"
"#,
        )
        .unwrap()
        .encode(),
        Keeps,
        (400., 400.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    let saved = |p: &Presenter<Keeps>| p.host().runner().slot("saved").cloned();
    let free = id(&p, "free");
    let reply = p.type_text(free, "ab").unwrap();
    assert!(reply.contains("\"value\":\"ab\""), "{reply}");
    // Hardware keys edit the typed text, not the empty prop, and show it
    // with no handler to hear them.
    p.dirty = false;
    p.key(Some('c'), false, 0.);
    assert!(p.dirty);
    assert_eq!(p.field_text(free), "abc");
    p.key(None, true, 0.);
    assert_eq!(p.field_text(free), "ab");
    let tree = crate::agent::handle(&mut p, r#"{"op":"tree","target":"free","shallow":true}"#);
    assert!(
        tree.contains(r#""value":"ab""#),
        "the agent's tree shows it: {tree}"
    );
    p.type_key(free, "Enter", "Enter", true, false).unwrap();
    assert_eq!(
        saved(&p),
        Some(Value::str("ab")),
        "change commits the typed text"
    );
    p.type_key(free, "Enter", "Enter", false, false).unwrap();
    // A bound value its action did not write keeps the typed text, as the
    // web's element does; one it wrote is shown.
    let refused = id(&p, "refused");
    p.type_text(refused, "mine").unwrap();
    assert_eq!(p.field_text(refused), "mine");
    // Its bound value changing replaces it, and coming back does not bring it back.
    p.tap(id(&p, "swap")).unwrap();
    assert_eq!(p.field_text(refused), "other");
    p.tap(id(&p, "swap")).unwrap();
    assert_eq!(p.field_text(refused), "kept");
    // Two commits before the presenter looks: the one between still counts.
    p.type_text(refused, "again").unwrap();
    let swap = id(&p, "swap");
    p.host_mut()
        .dispatch_at(swap, exact_runner::Event::Press, 0.);
    p.host_mut()
        .dispatch_at(swap, exact_runner::Event::Press, 0.);
    p.after_commit();
    assert_eq!(p.field_text(refused), "kept");
    // A key after a commit that replaced the typed text edits what shows.
    p.type_text(refused, "mine").unwrap();
    p.host_mut()
        .dispatch_at(swap, exact_runner::Event::Press, 0.);
    p.key(Some('q'), false, 0.);
    assert_eq!(p.field_text(refused), "otherq");
    let literal = id(&p, "literal");
    p.type_text(literal, "y").unwrap();
    assert_eq!(p.field_text(literal), "y", "a literal value is a default");
    p.type_key(literal, "Enter", "Enter", true, false).unwrap();
    assert_eq!(saved(&p), Some(Value::str("y")));
    let loud = id(&p, "loud");
    p.type_text(loud, "hi").unwrap();
    assert_eq!(p.field_text(loud), "hi!");
}

#[test]
fn a_reload_forgets_unbound_controls_own_state() {
    // A restart replaces the tree and its runner reuses view ids: an unbound
    // checkbox's own state, or a field's typed text, is not the new tree's
    // (LLP 1069.001 D4; `Presenter::replaced`).
    let src = r#"component App
  view
    column
      input type="checkbox" testId="box"
      input testId="free"
"#;
    let plan = contract::compile(src).unwrap().encode();
    let (mut p, error) = Presenter::boot_with(
        &plan,
        Keeps,
        (400., 400.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    let (box_id, free) = (id(&p, "box"), id(&p, "free"));
    p.tap(box_id).unwrap();
    assert_eq!(p.controls.get(&box_id), Some(&true));
    p.type_text(free, "draft").unwrap();
    assert_eq!(p.field_text(free), "draft");
    assert_eq!(p.edited, Some(free));
    p.reload(&plan, Keeps).unwrap();
    assert_eq!(
        p.edited, None,
        "the old field's edit is not the new one's to commit"
    );
    assert!(p.fields.is_empty());
    let (box_id, free) = (id(&p, "box"), id(&p, "free"));
    assert_eq!(
        p.controls.get(&box_id),
        None,
        "the checkbox starts unchecked again"
    );
    assert_eq!(p.field_text(free), "");
}

/// `bad` and the background round ask with a deadline only HTTP takes, so
/// the executor refuses both at admission; `good` asks for work it runs.
#[derive(Default)]
struct Refused {
    armed: bool,
    landed: Arc<std::sync::atomic::AtomicUsize>,
    good: Arc<std::sync::atomic::AtomicUsize>,
}
fn with_deadline(mut request: exact_runner::Request) -> exact_runner::Request {
    request.timeout_ms = Some(5);
    request
}
impl DataSource for Refused {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
    fn answer(&mut self, _: &mut Store, name: &str, _: &[Value]) -> Result<Answer, DataError> {
        Ok(Answer::Later(match name {
            "bad" => with_deadline(exact_runner::Request::continuation(1)),
            _ => exact_runner::Request::continuation(2),
        }))
    }
    fn continuation(
        &mut self,
        _: u64,
    ) -> Option<Box<dyn FnOnce() -> exact_runner::Outcome + Send>> {
        Some(Box::new(|| exact_runner::Outcome::Storage(vec![])))
    }
    fn parse(
        &mut self,
        _: &mut Store,
        name: &str,
        _: &[Value],
        _: exact_runner::Outcome,
    ) -> Result<Answer, DataError> {
        if name == "good" {
            self.good.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
        Ok(Answer::Now(Value::Number(1.)))
    }
    fn background(&mut self, _: &Store) -> Option<exact_runner::Request> {
        (!std::mem::replace(&mut self.armed, true)).then(|| {
            with_deadline(exact_runner::Request::continuation(
                exact_runner::BACKGROUND,
            ))
        })
    }
    fn background_landed(
        &mut self,
        _: &Store,
        _: exact_runner::Outcome,
    ) -> Result<Option<exact_runner::Request>, DataError> {
        self.landed
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(None)
    }
}

/// A refused background round behind a refused ordered request, and a send
/// held behind both: driven only by the executor's wake, as the display loop
/// is, the send still runs. The runner kept no refusal for a background
/// ticket, so its refusal never settled and nothing lifted the fence over the
/// send (Astra, round 2).
#[test]
#[cfg(unix)]
#[allow(unsafe_code)] // poll(2) on the executor's wake, as the display loop waits
fn a_send_held_behind_a_refused_background_round_runs_on_the_wake() {
    let source = Refused::default();
    let (landed, good) = (source.landed.clone(), source.good.clone());
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(
            "component App\n  resource bad = bad() as shape number\n  mutation kept as shape number\n  action ask\n    send kept = good()\n  view\n    column width=200 height=200\n      button \"Ask\" press=ask testId=\"ask\" height=30\n",
        )
        .unwrap()
        .encode(),
        source,
        (200., 200.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    // Before any pump: `bad`'s refusal is still retained, so the send is held.
    p.tap(id(&p, "ask")).unwrap();
    let mut fds = libc::pollfd {
        fd: p.executor_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    while good.load(std::sync::atomic::Ordering::SeqCst) == 0 {
        // SAFETY: one valid pollfd for the duration of the call.
        let ready = unsafe { libc::poll(&mut fds, 1, 2000) };
        assert!(ready > 0, "no wake with the send still held");
        p.pump(p.host().now());
    }
    assert_eq!(
        landed.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "the refused round settled"
    );
}

/// Every answer waits on shared work: its continuation is a re-ask
/// (`Dispatch::Again`), asked once, then the answer settles.
#[derive(Default)]
struct Waiting {
    asked: std::collections::HashSet<String>,
    settled: Arc<std::sync::atomic::AtomicUsize>,
}
impl DataSource for Waiting {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
    fn answer(&mut self, _: &mut Store, _: &str, args: &[Value]) -> Result<Answer, DataError> {
        let i = match args[0] {
            Value::Number(n) => n as u64,
            _ => 0,
        };
        Ok(Answer::Later(exact_runner::Request::continuation(i + 1)))
    }
    fn dispatch(&mut self, _: u64, _: &Store) -> exact_runner::Dispatch {
        exact_runner::Dispatch::Again
    }
    fn parse(
        &mut self,
        _: &mut Store,
        _: &str,
        args: &[Value],
        _: exact_runner::Outcome,
    ) -> Result<Answer, DataError> {
        let i = match args[0] {
            Value::Number(n) => n as u64,
            _ => 0,
        };
        // Asked again once more, as a waiter re-parks before its work lands.
        if self.asked.insert(i.to_string()) {
            return Ok(Answer::Later(exact_runner::Request::continuation(1000 + i)));
        }
        self.settled
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(Answer::Now(Value::Number(i as f64)))
    }
}

/// Two hundred answers re-asked twice each, more than the re-ask markers'
/// window of 128: driven only by the executor's wake, as the display loop
/// is, every one settles and none is refused (LLP 1041 §8.4, amended
/// 2026-10-09). Before, each re-ask was opaque work against the sixteen.
#[test]
#[cfg(unix)]
#[allow(unsafe_code)] // poll(2) on the executor's wake, as the display loop waits
fn re_asks_past_their_window_settle_on_the_wake_alone() {
    const N: usize = 200;
    let source = Waiting::default();
    let settled = source.settled.clone();
    let mut src = String::from("component App\n");
    for i in 0..N {
        src += &format!("  resource w{i} = wait({i}) as shape number\n");
    }
    src += "  view\n    column width=200 height=200\n      text \"waiting\"\n";
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(&src).unwrap().encode(),
        source,
        (200., 200.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    let mut fds = libc::pollfd {
        fd: p.executor_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    while settled.load(std::sync::atomic::Ordering::SeqCst) < N {
        // SAFETY: one valid pollfd for the duration of the call.
        let ready = unsafe { libc::poll(&mut fds, 1, 2000) };
        assert!(
            ready > 0,
            "no wake with {} of {N} settled",
            settled.load(std::sync::atomic::Ordering::SeqCst)
        );
        p.pump(p.host().now());
    }
    assert!(p.host().runner().failed_resources().is_empty());
    assert!(!p.host().runner().has_pending());
}
