//! The top layer (LLP 1115.001 D10): the shape rule, the surface a shell's
//! windows show, what closes it, the page inert beneath it, the focus
//! moving in and back, Back's order, and a host with no shell unchanged.
use super::look::{self, Inputs, Layout};
use super::shape;
use super::*;
use exact_runner::Value;
use std::cell::RefCell;
use std::rc::Rc;

/// A page with a field and the buttons that open each surface; an alert, a
/// three-action alert, an alert whose author wrote `closedby="any"`, a
/// chooser (no text: not an alert) and a menu of
/// four rows, one disabled, with a separator; a row that names the menu as
/// its context menu; and a second route.
const APP: &str = r#"routes nav
  home "/"
    detail "/detail"
component App
  state said = "nothing"
  state toggled = ""
  action toggling(e: ToggleEvent)
    toggled = `${e.oldState}>${e.newState}`
  state aimed = ""
  state name = ""
  action ask
    showModal("confirm")
  action askThree
    showModal("three")
  action askChooser
    showModal("chooser")
  action askLight
    showModal("light")
  action shut
    close("confirm")
  action remove
    said = "removed"
  action keep
    said = "kept"
  action later
    said = "later"
  action rename
    said = "renamed"
  action send
    said = "sent"
  action aim(id: string)
    aimed = id
  action poke
    said = "poked"
  action type(value)
    name = value
  action open
    nav = push(nav, "/detail")
  view
    main navigationKey=`${top(nav).id}` width="100%" height="100%"
      for e in stack(nav) key=e.id
        column navigationKey=`${e.id}` position="absolute" inset=0 align-items="flex-start"
          if e.name == "home"
            input value=name input=type testId="field" width=200 height=30
            button press=ask testId="ask" height=30
              text "Ask"
            button commandfor="confirm" command="show-modal" testId="ask-button" height=30
              text "Ask by a command"
            button press=askThree testId="ask-three" height=30
              text "Three"
            button press=askChooser testId="ask-chooser" height=30
              text "Chooser"
            button press=askLight testId="ask-light" height=30
              text "Light"
            button press=shut testId="shut" height=30
              text "Shut"
            button press=poke testId="poke" height=30
              text "Poke"
            button press=open testId="open" height=30
              text "Open"
            row width=400 height=40
              button press=poke contextmenu=aim("row") contextPopover="menu" testId="held" width=300 height=40
                text "Hold me"
              button popovertarget="menu" aria-label="More" id="trigger" testId="more" width=40 height=40
                image "symbol:more"
          if e.name == "detail"
            text "Detail" testId="detail"
            button press=ask testId="ask-detail" height=30
              text "Ask"
      text said testId="said"
      text aimed testId="aimed"
      dialog id="confirm" role="alertdialog" aria-label="Delete this?"
        text "It cannot be undone."
        button press=remove destructive=true commandfor="confirm" command="close" testId="confirm-delete"
          text "Delete"
        button commandfor="confirm" command="close" testId="confirm-cancel"
          text "Cancel"
      dialog id="three" role="alertdialog"
        text "Save your changes?"
        text "They are lost otherwise."
        button press=keep commandfor="three" command="close" testId="three-save"
          text "Save"
        button press=later commandfor="three" command="close" testId="three-later"
          text "Not now"
        button commandfor="three" command="close" testId="three-cancel"
          text "Cancel"
      dialog id="light" role="alertdialog" closedby="any" aria-label="Archive this?"
        text "You can bring it back."
        button press=keep commandfor="light" command="close" testId="light-archive"
          text "Archive"
        button commandfor="light" command="close" testId="light-cancel"
          text "Cancel"
      dialog id="notice" role="alertdialog"
        text "Couldn't save."
        text "The disk is full."
        button commandfor="notice" command="close"
          text "OK"
      dialog id="chooser" role="alertdialog"
        button press=keep commandfor="chooser" command="close"
          text "One"
        button press=later commandfor="chooser" command="close"
          text "Two"
        button commandfor="chooser" command="close"
          text "Cancel"
      column id="menu" popover="auto" toggle=toggling role="menu" aria-label="Row"
        button press=rename popovertarget="menu" popovertargetaction="hide" testId="menu-rename"
          image "symbol:pencil"
          text "Rename"
        button press=send popovertarget="menu" popovertargetaction="hide" id="second" testId="menu-share"
          text "Share"
        button press=send disabled=true popovertarget="menu" popovertargetaction="hide" testId="menu-off"
          text "Unavailable"
        hr
        button press=remove destructive=true popovertarget="menu" popovertargetaction="hide" testId="menu-delete"
          text "Delete"
"#;

/// The CPU painter standing in for a shell's: it keeps each surface's
/// pixels, and its words are their width and height.
struct Pixels(Rc<RefCell<Vec<Pixmap>>>);
impl SurfacePainter for Pixels {
    fn record(
        &mut self,
        size: (f32, f32),
        scale: f32,
        paint: &mut dyn FnMut(&mut dyn Backend),
    ) -> Vec<u32> {
        let mut raster = crate::raster::Raster::transparent();
        raster.begin(size.0, size.1, scale);
        paint(&mut raster);
        let pixels = raster.finish().unwrap();
        let words = vec![pixels.width(), pixels.height()];
        self.0.borrow_mut().push(pixels);
        words
    }
}

fn boot() -> Presenter<()> {
    let (p, error) = Presenter::boot_with(
        &crate::test_plan(APP),
        (),
        (400., 800.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p
}

#[test]
fn desktop_overlay_accepts_pointer_keys_and_dismisses_without_clicking_through() {
    let mut p = boot();
    p.serve_overlays();
    p.frame();
    tap(&mut p, "more").unwrap();
    p.frame();
    assert!(
        p.brush.top_surface.is_some(),
        "the menu reaches the page painter"
    );
    let open = p.top.open.as_ref().unwrap();
    let (x, y, w, h) = open.layout.targets[0].rect;
    let x = open.frame[0] as f32 / open.scale + x + w / 2.;
    let y = open.frame[1] as f32 / open.scale + y + h / 2.;
    p.pointer_down(x, y, 0.).unwrap();
    p.pointer_up(x, y, 1.).unwrap();
    assert!(p.surface().is_none());
    assert_eq!(text(&p, "said"), "renamed");
    p.frame();
    assert!(p.brush.top_surface.is_none());

    tap(&mut p, "more").unwrap();
    p.pointer_down(2., 2., 2.).unwrap();
    p.pointer_up(2., 2., 3.).unwrap();
    assert!(p.surface().is_none());
    assert_eq!(text(&p, "said"), "renamed");
    tap(&mut p, "more").unwrap();
    assert!(p.surface_key("ArrowDown", 4.));
    assert!(p.surface_key("ArrowDown", 5.));
    assert!(p.surface_key("Enter", 6.));
    assert_eq!(text(&p, "said"), "sent");
    assert!(p.surface().is_none());
}

#[test]
fn desktop_alerts_focus_cancel_or_ok_on_open_and_replacement() {
    let mut p = boot();
    p.serve_overlays();
    let frame = p.display_frame().unwrap();
    assert!(p.display_complete(&frame));
    tap(&mut p, "ask").unwrap();
    let open = p.top.open.as_ref().unwrap();
    assert_eq!(
        open.focused.map(|i| open.layout.targets[i].node),
        Some(id(&p, "confirm-cancel"))
    );
    assert!(p.surface_key("Enter", 1.));
    assert!(p.surface().is_none());
    assert_eq!(text(&p, "said"), "nothing");
    tap(&mut p, "ask").unwrap();
    let notice = named(&p, "notice");
    let key = p.host.kernel().node(notice).unwrap().key;
    assert!(p.present(Some(key), "notice", None, None, true, 3.));
    assert_eq!(p.top.open.as_ref().unwrap().focused, Some(0));
    assert!(p.surface_key("Enter", 4.));
    assert!(p.surface().is_none());
    assert_eq!(text(&p, "said"), "nothing");
    assert_eq!(p.focus(), Some(id(&p, "ask")));
}

/// A presenter whose shell makes windows, and the pictures it recorded.
fn windowed() -> (Presenter<()>, Rc<RefCell<Vec<Pixmap>>>) {
    let mut p = boot();
    let pictures = Rc::new(RefCell::new(Vec::new()));
    p.serve_requests();
    p.serve_windows(Box::new(Pixels(pictures.clone())));
    (p, pictures)
}

fn id(p: &Presenter<()>, name: &str) -> ViewId {
    let kernel = p.host().kernel();
    kernel
        .node_by_key(kernel.find_by_test_id(name)[0])
        .unwrap()
        .id
}

/// The node whose HTML `id` is `name`.
fn named(p: &Presenter<()>, name: &str) -> ViewId {
    let kernel = p.host().kernel();
    kernel.node_by_key(kernel.find_by_id(name)[0]).unwrap().id
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

fn text(p: &Presenter<()>, name: &str) -> String {
    let node = p.host().kernel().node(id(p, name)).unwrap();
    node.text_runs().iter().map(|r| &*r.text).collect()
}

/// Tap `name` as a person does and run the commands its action issued.
fn tap(p: &mut Presenter<()>, name: &str) -> Result<String, String> {
    let view = id(p, name);
    let result = p.tap(view);
    p.run_commands(|| ());
    result
}

/// Touch the open surface's row for `name` and lift there.
fn choose(p: &mut Presenter<()>, name: &str) {
    let node = id(p, name);
    let shown = p.surface().expect("a surface is open");
    let (serial, target) = (shown.serial, shown.targets.iter().find(|t| t.node == node));
    let (x, y, w, h) = target.expect("a row for the node").rect;
    for action in [0, 1] {
        p.surface_event(serial, action, x + w / 2.0, y + h / 2.0, 0.0);
    }
    p.run_commands(|| ());
}

/// The surface statements among the shell's requests: what it is (0 closed,
/// 1 an alert, 2 a menu) by serial, with the frame and the stream's count.
fn told(p: &mut Presenter<()>) -> Vec<(u32, i32, [i32; 4], i32)> {
    p.tell_shell();
    let words = p.take_requests().unwrap_or_default();
    let (mut out, mut at) = (Vec::new(), 0);
    while at < words.len() {
        let (kind, n) = (words[at], words[at + 2] as usize);
        let body = &words[at + 3..at + 3 + n];
        if kind == 7 {
            let frame = [body[3], body[4], body[5], body[6]];
            assert_eq!(
                body.len(),
                10 + body[9] as usize,
                "the stream ends the body"
            );
            out.push((body[0] as u32, body[1], frame, body[9]));
        }
        at += 3 + n;
    }
    out
}

fn journal(p: &Presenter<()>) -> Vec<String> {
    p.host().runner().journal().map(str::to_owned).collect()
}

fn says(p: &Presenter<()>, what: &str) -> usize {
    journal(p).iter().filter(|l| l.contains(what)).count()
}

#[test]
fn the_shape_rule_is_the_one_the_apple_hosts_follow() {
    // Text, one cancel, at most three buttons.
    assert!(shape::is_alert(1, 1, 1));
    assert!(shape::is_alert(2, 2, 1));
    assert!(!shape::is_alert(0, 1, 1), "no text: a chooser");
    assert!(!shape::is_alert(1, 1, 0), "no cancel");
    assert!(!shape::is_alert(1, 3, 1), "four buttons");
    // The label is the title; with none, the first line is.
    let lines = ["One".to_string(), "Two".to_string(), "Three".to_string()];
    assert_eq!(
        shape::alert_text(Some("Label"), &lines),
        (Some("Label".into()), Some("One\nTwo\nThree".into()))
    );
    assert_eq!(
        shape::alert_text(None, &lines),
        (Some("One".into()), Some("Two\nThree".into()))
    );
    assert_eq!(
        shape::alert_text(Some(""), &lines[..1]),
        (Some("One".into()), None)
    );
    assert_eq!(shape::alert_text(None, &[]), (None, None));

    // Read from the tree: the cancel first, then the actions as written.
    let p = boot();
    let plan = |name: &str| p.surface_plan(named(&p, name), true);
    let Some(Plan::Alert {
        title,
        message,
        rows,
        cancellable,
        outside,
    }) = plan("confirm")
    else {
        panic!("confirm is an alert");
    };
    assert_eq!(title.as_deref(), Some("Delete this?"));
    assert_eq!(message.as_deref(), Some("It cannot be undone."));
    let titles: Vec<_> = rows
        .iter()
        .map(|r| (r.title.as_str(), r.cancel, r.destructive))
        .collect();
    assert_eq!(titles, [("Cancel", true, false), ("Delete", false, true)]);
    // LLP 1115.000 D13.4: lowering implies `closerequest` for the shape, so
    // Back and Escape cancel it and it has no outside.
    assert!(cancellable && !outside);
    assert!(
        matches!(
            plan("light"),
            Some(Plan::Alert {
                cancellable: true,
                outside: true,
                ..
            })
        ),
        "an authored `closedby=\"any\"` is read as it stands"
    );
    let Some(Plan::Alert {
        title,
        message,
        rows,
        ..
    }) = plan("three")
    else {
        panic!("three is an alert");
    };
    assert_eq!(
        (title.as_deref(), message.as_deref()),
        (Some("Save your changes?"), Some("They are lost otherwise."))
    );
    let titles: Vec<_> = rows.iter().map(|r| r.title.as_str()).collect();
    assert_eq!(titles, ["Cancel", "Save", "Not now"]);
    // Text and its one closing button: the one-button alert, which tells.
    let Some(Plan::Alert {
        title,
        message,
        rows,
        cancellable,
        outside,
    }) = plan("notice")
    else {
        panic!("notice is an alert");
    };
    assert_eq!(
        (title.as_deref(), message.as_deref()),
        (Some("Couldn't save."), Some("The disk is full."))
    );
    let titles: Vec<_> = rows.iter().map(|r| (r.title.as_str(), r.cancel)).collect();
    assert_eq!(titles, [("OK", true)]);
    assert!(cancellable && !outside);
    assert_eq!(
        plan("chooser"),
        None,
        "no text: a chooser, which is not presented yet"
    );
    // A menu: its items, the separator the author wrote, the symbol, the
    // destructive row and the disabled one.
    let Some(Plan::Menu(entries)) = plan("menu") else {
        panic!("menu is a menu");
    };
    let rows: Vec<_> = entries
        .iter()
        .map(|e| {
            e.as_ref().map(|r| {
                (
                    r.title.as_str(),
                    r.symbol.as_deref(),
                    r.destructive,
                    r.enabled,
                )
            })
        })
        .collect();
    assert_eq!(
        rows,
        [
            Some(("Rename", Some("pencil"), false, true)),
            Some(("Share", None, false, true)),
            Some(("Unavailable", None, false, false)),
            None,
            Some(("Delete", None, true, true)),
        ]
    );
}

#[test]
fn process_relaunch_is_refused_in_the_journal() {
    let src = "component App\n  action restart\n    relaunch()\n  view\n    button press=restart testId=\"restart\"\n      text \"Restart\"\n";
    for agent in [false, true] {
        let plan = contract::compile(src).unwrap();
        let (mut p, error) = Presenter::boot_with(
            &plan.encode(),
            (),
            (400., 400.),
            1.,
            PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(error.is_none(), "{error:?}");
        p.agent = agent;
        tap(&mut p, "restart").unwrap();
        let journal = crate::agent::handle(&mut p, r#"{"op":"logs"}"#);
        let reason = if agent {
            "the agent owns this process"
        } else {
            "unsupported on Linux"
        };
        assert!(journal.contains(reason), "{journal}");
    }
}

#[test]
fn a_host_with_no_shell_has_no_top_layer_and_keeps_what_it_did() {
    let mut p = boot();
    p.serve_requests();
    // A popover's invoker and a dialog's button are refused, by the words
    // they always were; `showModal` and `close` present nothing.
    assert_eq!(
        tap(&mut p, "more"),
        Err(crate::navigation::POPOVER_UNSUPPORTED.into())
    );
    let button = id(&p, "confirm-delete");
    let refusal = p.host_mut().dispatch_at(button, Event::Press, 0.0);
    assert_eq!(
        refusal.as_deref(),
        Some(crate::navigation::DIALOG_UNSUPPORTED)
    );
    tap(&mut p, "ask").unwrap();
    tap(&mut p, "shut").unwrap();
    let journal = crate::agent::handle(&mut p, r#"{"op":"logs"}"#);
    for command in ["showModal", "close"] {
        assert!(
            journal.contains(&format!(
                "{command} refused: dialogs unsupported on the headless/DRM host"
            )),
            "{journal}"
        );
    }
    assert!(p.surface().is_none());
    assert!(
        !p.long_press(100.0, 290.0, 0.0),
        "no long press without a top layer"
    );
    assert_eq!(text(&p, "aimed"), "");
    // The page is as live as it was, and the shell is told no surface.
    tap(&mut p, "poke").unwrap();
    assert_eq!(text(&p, "said"), "poked");
    assert!(told(&mut p).is_empty());
    assert!(!p.back_goes());
}

#[test]
fn an_alert_opens_by_show_modal_and_by_a_command_and_each_button_does_its_own() {
    let (mut p, pictures) = windowed();
    assert!(told(&mut p).is_empty(), "nothing is open at boot");
    // `showModal` from an action: Material's dialog, 280 to 560 wide, 24
    // from the window's sides, and the shell is told it once.
    tap(&mut p, "ask").unwrap();
    let shown = p.surface().expect("the alert is open");
    assert_eq!(
        (shown.kind, shown.node),
        (SurfaceKind::Alert, named(&p, "confirm"))
    );
    assert_eq!(shown.size.0, 352.0);
    let serial = shown.serial;
    let statement = told(&mut p);
    assert_eq!(statement.len(), 1);
    let (s, what, frame, stream) = statement[0];
    assert_eq!((s, what, stream), (serial, 1, 2));
    assert_eq!(frame[2], 352);
    assert!(frame[3] > 100, "{frame:?}");
    assert!(told(&mut p).is_empty(), "told once while nothing changes");
    assert_eq!(pictures.borrow().len(), 1);
    // The destructive action runs its handler and closes it.
    choose(&mut p, "confirm-delete");
    assert!(p.surface().is_none());
    assert_eq!(text(&p, "said"), "removed");
    assert_eq!(
        told(&mut p),
        [(serial, 0, [0; 4], 0)],
        "the shell is told it closed"
    );

    // A button's `command="show-modal"` opens it too; the cancel closes it
    // and runs nothing.
    tap(&mut p, "ask-button").unwrap();
    let again = p.surface().expect("open again").serial;
    assert!(again > serial, "a serial is never reused");
    choose(&mut p, "confirm-cancel");
    assert!(p.surface().is_none());
    assert_eq!(text(&p, "said"), "removed", "the cancel ran nothing");
    // A finger that leaves its row lets go of it, and lifts on nothing.
    tap(&mut p, "ask").unwrap();
    let shown = p.surface().unwrap();
    let (serial, row) = (shown.serial, shown.targets[1]);
    let (x, y) = (row.rect.0 + 4.0, row.rect.1 + 4.0);
    p.surface_event(serial, 0, x, y, 0.0);
    assert_eq!(p.surface().unwrap().pressed, Some(1));
    let lit = told(&mut p);
    assert_eq!(
        lit.last().map(|s| (s.0, s.1)),
        Some((serial, 1)),
        "a press lights the row: told again"
    );
    p.surface_event(serial, 2, x, y - 60.0, 0.0);
    assert_eq!(p.surface().unwrap().pressed, None);
    p.surface_event(serial, 1, x, y - 60.0, 0.0);
    assert!(p.surface().is_some(), "nothing was chosen");
    // `close(id)` from an action closes it as its cancel does.
    let shut = id(&p, "shut");
    assert!(p.host_mut().dispatch_at(shut, Event::Press, 0.0).is_none());
    assert!(p.after_commit().is_none());
    p.run_commands(|| ());
    assert!(p.surface().is_none());

    // Three buttons are an alert too; a chooser is refused, in the journal.
    tap(&mut p, "ask-three").unwrap();
    assert_eq!(p.surface().unwrap().targets.len(), 3);
    choose(&mut p, "three-later");
    assert_eq!(text(&p, "said"), "later");
    tap(&mut p, "ask-chooser").unwrap();
    assert!(p.surface().is_none());
    assert_eq!(
        says(&p, "showModal \"chooser\" refused"),
        1,
        "{:?}",
        journal(&p)
    );
}

/// A shell's field, stood in for: any text is a line 20 tall.
struct Field;
impl crate::fields::FieldMetrics for Field {
    fn measure(&self, _: &crate::fields::FieldFace, _: &str, width: Option<f32>) -> [f32; 3] {
        [width.unwrap_or(100.0), 20.0, 16.0]
    }
}

/// Seen on a Pixel: a hardware key ends touch mode, Android then gives the
/// focus to the page's hosted `EditText`, and the reader says so. Under an
/// open surface that is no focus of the page's: the field is not given it,
/// the shell is told again that no field holds it (which blurs the view),
/// and when the surface closes the focus goes back where it was, not to
/// the field, so no keyboard rises on a field nobody chose.
#[test]
fn a_hosted_field_cannot_take_the_focus_under_an_open_surface() {
    let _hosting = crate::fields::host_here(&Field, 1.0);
    let (mut p, _) = windowed();
    p.reader_fields(1.0);
    let (field, ask) = (id(&p, "field"), id(&p, "ask"));
    // With nothing open the reader's word is taken: this call is live.
    p.reader_field_focused(field, 0, true, 0.0);
    assert_eq!(p.focus(), Some(field), "the field is one the shell hosts");
    assert!(p.set_focus(Some(ask), 0.0).is_none());
    p.tell_shell();
    p.take_requests();
    // An alert opens, from the button that holds the focus.
    assert!(p.host_mut().dispatch_at(ask, Event::Press, 1.0).is_none());
    assert!(p.after_commit().is_none());
    p.run_commands(|| ());
    assert!(p.surface().is_some());
    assert_eq!(p.focus(), None);
    p.tell_shell();
    p.take_requests();
    // The reader says the field took the focus: refused, and the shell is
    // told the keyboard's state anew, hidden, with no field named.
    p.reader_field_focused(field, 0, true, 2.0);
    assert_eq!(p.focus(), None, "the page is inert under the alert");
    p.tell_shell();
    let words = p.take_requests().unwrap_or_default();
    assert!(
        words.windows(5).any(|w| w == [6, w[1], 2, 0, 0]),
        "the shell is told to put the field's focus away: {words:?}"
    );
    assert!(
        !words
            .windows(5)
            .any(|w| w[0] == 6 && w[2] == 2 && w[3] != 0),
        "and never that a field wants the keyboard: {words:?}"
    );
    // Closed, the focus is back at the button; no keyboard is asked for.
    p.key_down("Escape", 3.0);
    assert!(p.surface().is_none());
    assert_eq!(p.focus(), Some(ask));
    p.tell_shell();
    let words = p.take_requests().unwrap_or_default();
    assert!(
        !words
            .windows(5)
            .any(|w| w[0] == 6 && w[2] == 2 && w[3] != 0),
        "no keyboard rises when the alert closes: {words:?}"
    );
}

#[test]
fn the_page_is_inert_under_an_alert_and_the_focus_moves_in_and_back() {
    let (mut p, _) = windowed();
    // A field holds the focus, and the shell shows its keyboard.
    let field = id(&p, "field");
    assert!(p.set_focus(Some(field), 0.0).is_none());
    p.tell_shell();
    p.take_requests();
    let ask = id(&p, "ask");
    assert!(p.host_mut().dispatch_at(ask, Event::Press, 0.0).is_none());
    assert!(p.after_commit().is_none());
    p.run_commands(|| ());
    assert!(p.surface().is_some());
    // The focus moved in: the field lost it, and the keyboard goes.
    assert_eq!(p.focus(), None);
    p.tell_shell();
    let words = p.take_requests().unwrap();
    assert!(
        words.windows(5).any(|w| w == [6, w[1], 2, 0, 0]),
        "the keyboard is put away: {words:?}"
    );
    // () of the page is hit, pressed or typed into.
    let poke = id(&p, "poke");
    let at = p.rect_of(poke).map(|r| (r.0 + 5.0, r.1 + 5.0)).unwrap();
    assert_eq!(p.hit(at.0, at.1), None);
    assert_eq!(p.pointer_down(at.0, at.1, 1.0), Ok(false));
    assert_eq!(p.pointer_up(at.0, at.1, 2.0), Ok(false));
    let refusal = tap(&mut p, "poke").unwrap_err();
    assert!(refusal.contains("under the open alert"), "{refusal}");
    p.key_down("x", 3.0);
    p.key_down("Enter", 3.0);
    assert_eq!(text(&p, "said"), "nothing");
    assert!(
        p.surface().is_some(),
        "a tap outside does nothing to an alert"
    );
    // A keyboard moves the surface's own focus and chooses with Enter:
    // Tab to the cancel, Tab to Delete, Enter.
    p.key_down("Tab", 4.0);
    p.key_down("Tab", 4.0);
    p.key_down("Enter", 4.0);
    assert!(p.surface().is_none());
    assert_eq!(text(&p, "said"), "removed");
    // The focus is back where it was, and the keyboard with it.
    assert_eq!(p.focus(), Some(field));
    p.tell_shell();
    let words = p.take_requests().unwrap();
    assert!(
        words.windows(5).any(|w| w == [6, w[1], 2, 1, 0]),
        "the keyboard is shown again: {words:?}"
    );
    // Escape cancels, and the system's own dismissal does.
    for dismiss in [0, 1] {
        assert!(p.host_mut().dispatch_at(ask, Event::Press, 5.0).is_none());
        assert!(p.after_commit().is_none());
        p.run_commands(|| ());
        let serial = p.surface().expect("open").serial;
        match dismiss {
            0 => p.key_down("Escape", 6.0),
            _ => p.surface_event(serial, event::DISMISSED, 0.0, 0.0, 6.0),
        }
        assert!(p.surface().is_none());
        assert_eq!(p.focus(), Some(field));
    }
    assert_eq!(text(&p, "said"), "removed", "a cancel runs nothing");
}

/// LLP 1115.000 D13.4: an alert has no outside, and its `closedby` is read
/// as it stands, written or implied. The shape's implied `closerequest`
/// tells the shell that Back dismisses it and a tap outside does not (flags
/// 1). An author's `closedby="any"` is Material's dialog left to itself,
/// which a tap outside dismisses: the shell is told so (flags 3), and a
/// touch that reaches the page, or the driver's tap on a node under it,
/// closes it and runs nothing.
#[test]
fn an_alerts_outside_is_its_closedby_written_or_implied() {
    let (mut p, _) = windowed();
    let flags = |p: &mut Presenter<()>| {
        p.tell_shell();
        let words = p.take_requests().unwrap_or_default();
        let (mut found, mut at) = (None, 0);
        while at < words.len() {
            let n = words[at + 2] as usize;
            if words[at] == 7 && words[at + 4] != 0 {
                found = Some(words[at + 5]);
            }
            at += 3 + n;
        }
        found.expect("a surface was told")
    };
    let poke = id(&p, "poke");
    let at = p.rect_of(poke).map(|r| (r.0 + 5.0, r.1 + 5.0)).unwrap();
    // The alert: no outside.
    tap(&mut p, "ask").unwrap();
    assert_eq!(
        flags(&mut p),
        1,
        "Back dismisses it; a tap outside does not"
    );
    assert_eq!(p.pointer_down(at.0, at.1, 1.0), Ok(false));
    assert_eq!(p.pointer_up(at.0, at.1, 2.0), Ok(false));
    assert!(p.surface().is_some(), "still open");
    assert!(tap(&mut p, "poke")
        .unwrap_err()
        .contains("under the open alert"));
    p.key_down("Escape", 3.0);
    assert!(p.surface().is_none(), "Escape cancels it");
    // The same shape with `closedby="any"`: the dismissible form.
    tap(&mut p, "ask-light").unwrap();
    assert_eq!(flags(&mut p), 3, "and a tap outside dismisses it");
    assert_eq!(p.pointer_down(at.0, at.1, 4.0), Ok(false));
    assert!(p.surface().is_none(), "a touch on the page closes it");
    tap(&mut p, "ask-light").unwrap();
    let dismissed = tap(&mut p, "poke").unwrap();
    assert!(dismissed.contains("\"dismissed\""), "{dismissed}");
    assert!(p.surface().is_none());
    assert_eq!(text(&p, "said"), "nothing", "a dismissal runs nothing");
}

#[test]
fn a_menu_hangs_at_its_invoker_and_closes_on_a_tap_outside() {
    let (mut p, pictures) = windowed();
    tap(&mut p, "more").unwrap();
    let shown = p.surface().expect("the menu is open");
    assert_eq!(shown.kind, SurfaceKind::Menu);
    // Four rows of 48 and a separator's 17, with 8 above and under.
    assert_eq!(shown.targets.len(), 4);
    assert_eq!(shown.size.1, 8.0 + 4.0 * 48.0 + 17.0 + 8.0);
    assert!((112.0..=280.0).contains(&shown.size.0), "{:?}", shown.size);
    let (serial, size) = (shown.serial, shown.size);
    // Under its invoker, ending at the invoker's end where it would not
    // fit starting at its start.
    let more = p.rect_of(id(&p, "more")).unwrap();
    let statement = told(&mut p);
    let (_, what, frame, _) = statement[0];
    assert_eq!(what, 2);
    assert_eq!(frame[1], (more.1 + more.3) as i32);
    assert_eq!(frame[0], (more.0 + more.2 - size.0).round() as i32);
    // A disabled row takes no press; a row's tap runs it and closes.
    let off = id(&p, "menu-off");
    assert!(p.tap(off).unwrap_err().contains("disabled"));
    choose(&mut p, "menu-off");
    assert!(p.surface().is_some());
    choose(&mut p, "menu-rename");
    assert!(p.surface().is_none());
    assert_eq!(text(&p, "said"), "renamed");
    assert_eq!(
        p.focus(),
        Some(id(&p, "more")),
        "the focus is back at the invoker"
    );
    // A touch on the page closes it and presses nothing; so does the
    // system's dismissal, and the agent's tap on a node under it.
    tap(&mut p, "more").unwrap();
    let poke = p.rect_of(id(&p, "poke")).unwrap();
    assert_eq!(p.pointer_down(poke.0 + 5.0, poke.1 + 5.0, 1.0), Ok(false));
    assert_eq!(p.pointer_up(poke.0 + 5.0, poke.1 + 5.0, 2.0), Ok(false));
    assert!(p.surface().is_none());
    assert_eq!(text(&p, "said"), "renamed");
    tap(&mut p, "more").unwrap();
    let serial_again = p.surface().unwrap().serial;
    assert!(tap(&mut p, "poke").unwrap().contains("dismissed"));
    assert!(p.surface().is_none());
    assert_eq!(text(&p, "said"), "renamed");
    assert!(serial_again > serial);
    // A row addressed by its node is chosen, as a driver's `tap` does.
    tap(&mut p, "more").unwrap();
    assert!(tap(&mut p, "menu-delete").unwrap().contains("surface"));
    assert_eq!(text(&p, "said"), "removed");
    assert!(!pictures.borrow().is_empty());
}

#[test]
fn a_long_press_opens_the_context_menu_a_node_names_where_the_finger_is() {
    let (mut p, _) = windowed();
    let held = p.rect_of(id(&p, "held")).unwrap();
    let at = (held.0 + 120.0, held.1 + 10.0);
    assert_eq!(p.pointer_down(at.0, at.1, 0.0), Ok(true));
    let before = p.long_presses();
    assert!(p.long_press(at.0, at.1, 500.0));
    assert_eq!(p.long_presses(), before.wrapping_add(1));
    // Its `contextmenu` ran first, then the menu opened at the finger.
    assert_eq!(text(&p, "aimed"), "row");
    assert_eq!(p.surface().map(|s| s.kind), Some(SurfaceKind::Menu));
    let (_, _, frame, _) = told(&mut p)[0];
    assert_eq!((frame[0], frame[1]), (at.0 as i32, at.1 as i32));
    // The finger lifts on nothing: the row's own press does not run.
    let _ = p.pointer_up(at.0, at.1, 600.0);
    assert_eq!(text(&p, "said"), "nothing");
    // The platform's long-press click was asked for.
    assert!(
        !p.long_press(at.0, at.1, 700.0),
        "not while a surface is open"
    );
    let poke = p.rect_of(id(&p, "poke")).unwrap();
    choose(&mut p, "menu-share");
    assert!(
        !p.long_press(poke.0 + 5.0, poke.1 + 5.0, 800.0),
        "a node that names none"
    );
    assert_eq!(
        p.long_presses(),
        before.wrapping_add(1),
        "counted only when taken"
    );
}

#[test]
fn back_closes_the_top_surface_before_any_route_is_asked() {
    let (mut p, _) = windowed();
    assert!(!p.back_goes());
    tap(&mut p, "open").unwrap();
    assert!(p.back_goes());
    p.tell_shell();
    p.take_requests();
    tap(&mut p, "ask-detail").unwrap();
    assert!(p.surface().is_some());
    // Back closes the alert and the route stays; then Back pops the route.
    assert!(p.system_back(1.0));
    assert!(p.surface().is_none());
    assert!(
        p.host().kernel().find_by_test_id("detail").len() == 1,
        "still on the pushed route"
    );
    assert!(p.system_back(2.0));
    assert!(p.host().kernel().find_by_test_id("detail").is_empty());
    assert!(!p.back_goes());
    // At the first screen an open surface gives Back somewhere to go, and
    // the shell is told so with no commit between.
    p.tell_shell();
    p.take_requests();
    tap(&mut p, "more").unwrap();
    assert!(p.back_goes());
    p.tell_shell();
    let words = p.take_requests().unwrap();
    assert!(
        words
            .windows(4)
            .any(|w| w[0] == 5 && w[2] == 1 && w[3] == 1),
        "{words:?}"
    );
    assert!(p.system_back(3.0));
    assert!(p.surface().is_none());
    p.tell_shell();
    let words = p.take_requests().unwrap();
    assert!(
        words
            .windows(4)
            .any(|w| w[0] == 5 && w[2] == 1 && w[3] == 0),
        "{words:?}"
    );
    assert!(!p.system_back(4.0), "then Back is the system's");
}

#[test]
fn a_surface_follows_the_scheme_the_font_scale_and_a_reader_made_again() {
    let (mut p, pictures) = windowed();
    tap(&mut p, "ask").unwrap();
    let serial = p.surface().unwrap().serial;
    let first = told(&mut p)[0];
    let light = pictures.borrow().last().unwrap().pixel(20, 20).unwrap();
    // Night mode with the alert open: the same surface, painted again.
    let phone = Device {
        night: true,
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
    let dark = told(&mut p);
    assert_eq!(dark.len(), 1);
    assert_eq!((dark[0].0, dark[0].1, dark[0].2), (serial, 1, first.2));
    assert_ne!(
        pictures.borrow().last().unwrap().pixel(20, 20).unwrap(),
        light
    );
    // A larger font scale: taller, the same surface still.
    assert!(p
        .set_device(&Device {
            font_scale: 2.0,
            ..phone.clone()
        })
        .is_none());
    let large = told(&mut p);
    assert_eq!((large[0].0, large[0].1), (serial, 1));
    assert!(large[0].2[3] > first.2[3], "{large:?} {first:?}");
    // A reader made again is told the open surface anew, by its serial.
    p.reader_replaced(Box::new(crate::raster::Raster::new()));
    let again = told(&mut p);
    assert_eq!((again.len(), again[0].0, again[0].1), (1, serial, 1));
    // The shell could not make it: it closes, the page is live again.
    p.surface_event(serial, event::REFUSED, 0.0, 0.0, 0.0);
    assert!(p.surface().is_none());
    assert_eq!(says(&p, "was refused"), 1);
    tap(&mut p, "poke").unwrap();
    assert_eq!(text(&p, "said"), "poked");
}

#[test]
fn the_dialog_and_the_menu_keep_materials_metrics() {
    let engine = crate::text::TextEngine::shared();
    let inputs = |width: f32| Inputs {
        contrast: None,
        dark: false,
        font_scale: 1.0,
        window: (width, 800.0),
    };
    let row = |title: &str, cancel: bool| Row {
        node: 1,
        title: title.into(),
        symbol: None,
        destructive: false,
        enabled: true,
        cancel,
        checked: false,
        opens: None,
    };
    let alert = |rows: Vec<Row>| Plan::Alert {
        title: Some("Title".into()),
        message: Some("Message".into()),
        rows,
        cancellable: true,
        outside: false,
    };
    let two = alert(vec![row("Cancel", true), row("Delete", false)]);
    // 280 to 560 wide, 24 from the window's sides.
    for (window, wide) in [
        (300.0, 280.0),
        (400.0, 352.0),
        (1000.0, 560.0),
        (200.0, 200.0),
    ] {
        let layout = Layout::of(&mut engine.borrow_mut(), &two, 1, &inputs(window));
        assert_eq!(layout.size.0, wide, "a {window} wide window");
    }
    // The buttons end 24 from the trailing edge, 8 apart, each at least 40
    // tall, the cancel first.
    let layout = Layout::of(&mut engine.borrow_mut(), &two, 1, &inputs(400.0));
    let (cancel, delete) = (layout.targets[0].rect, layout.targets[1].rect);
    assert!(near(delete.0 + delete.2, 352.0 - 24.0), "{delete:?}");
    assert!(
        near(cancel.0 + cancel.2 + 8.0, delete.0),
        "{cancel:?} {delete:?}"
    );
    assert!(cancel.3 >= 40.0 && delete.3 >= 40.0);
    assert_eq!(layout.size.1, delete.1 + delete.3 + 24.0);
    assert_eq!((layout.radius, layout.elevation), (28.0, 0.0));
    // Three long ones take a second line.
    let long = alert(vec![
        row("Cancel", true),
        row("Save everything and go on", false),
        row("Throw everything away", false),
    ]);
    let layout = Layout::of(&mut engine.borrow_mut(), &long, 1, &inputs(400.0));
    let lines: std::collections::BTreeSet<i32> =
        layout.targets.iter().map(|t| t.rect.1 as i32).collect();
    assert!(lines.len() > 1, "{:?}", layout.targets);
    assert!(layout
        .targets
        .iter()
        .all(|t| t.rect.0 >= 24.0 && t.rect.0 + t.rect.2 <= 352.0 - 24.0 + 0.01));
    // A menu's place: under its anchor from its start; over it where the
    // window ends; ending at its end where the window's side is near.
    let (window, margins, size) = ((400.0, 800.0), (48.0, 48.0), (200.0, 217.0));
    assert_eq!(
        look::place((20.0, 100.0, 40.0, 40.0), size, window, margins),
        (20.0, 140.0)
    );
    assert_eq!(
        look::place((20.0, 700.0, 40.0, 40.0), size, window, margins),
        (20.0, 483.0)
    );
    assert_eq!(
        look::place((340.0, 100.0, 40.0, 40.0), size, window, margins),
        (180.0, 140.0)
    );
    assert_eq!(
        look::place((340.0, 700.0, 40.0, 40.0), size, window, margins),
        (180.0, 483.0)
    );
    assert_eq!(
        look::place((20.0, 10.0, 40.0, 20.0), size, window, margins),
        (20.0, 48.0),
        "never under the status bar"
    );
}

/// How a driver addresses a node inside a window: `state.surface` names the
/// open surface, the authored node behind each row and the row's box in
/// the window's own points, and `tap` on that node chooses the row. A host
/// with no top layer says so, and never leaves the section out.
#[test]
fn a_driver_reads_the_open_surface_from_state_and_taps_a_row_by_its_node() {
    let state = |p: &mut Presenter<()>| -> serde_json::Value {
        serde_json::from_str(&crate::agent::handle(p, r#"{"op":"state"}"#)).unwrap()
    };
    let mut desktop = boot();
    assert_eq!(state(&mut desktop)["surface"]["unavailable"], true);

    let (mut p, _) = windowed();
    assert!(state(&mut p)["surface"].is_null(), "nothing is open");
    tap(&mut p, "ask").unwrap();
    let surface = state(&mut p)["surface"].clone();
    assert_eq!(surface["kind"], "alert");
    assert_eq!(surface["node"], named(&p, "confirm"));
    assert_eq!(surface["title"], "Delete this?");
    let rows = surface["rows"].as_array().unwrap();
    let delete = id(&p, "confirm-delete");
    let row = rows.iter().find(|row| row["id"] == delete).unwrap();
    assert_eq!(
        (&row["title"], &row["destructive"]),
        (&"Delete".into(), &true.into())
    );
    // Its box is in the window: inside the surface's own size.
    let (x, w) = (row["x"].as_f64().unwrap(), row["w"].as_f64().unwrap());
    assert!(x >= 0.0 && x + w <= surface["w"].as_f64().unwrap(), "{row}");
    // The node the state names is the one a tap chooses.
    let tapped = crate::agent::handle(&mut p, &format!(r#"{{"op":"tap","id":{delete}}}"#));
    assert!(tapped.contains("\"surface\""), "{tapped}");
    p.run_commands(|| ());
    assert_eq!(text(&p, "said"), "removed");
    assert!(state(&mut p)["surface"].is_null());
}

#[test]
fn a_driver_reveals_a_row_of_the_surface_and_opens_a_context_menu_by_its_node() {
    let ask = |p: &mut Presenter<()>, request: String| -> serde_json::Value {
        let reply = crate::agent::handle(p, &request);
        p.run_commands(|| ());
        serde_json::from_str(&reply).unwrap()
    };
    // A host with no shell refuses a context menu's popover, as it did.
    let mut desktop = boot();
    let held = id(&desktop, "held");
    let refused = ask(
        &mut desktop,
        format!(r#"{{"op":"tap","id":{held},"contextmenu":true}}"#),
    );
    assert_eq!(refused["error"], crate::navigation::POPOVER_UNSUPPORTED);

    let (mut p, _) = windowed();
    // The driver scrolls a target into view before it taps it. A row of the
    // open alert is no box of the page's: it is in view in its window.
    tap(&mut p, "ask").unwrap();
    let delete = id(&p, "confirm-delete");
    let revealed = ask(&mut p, format!(r#"{{"op":"reveal","id":{delete}}}"#));
    assert_eq!(revealed["revealed"], delete, "{revealed}");
    assert_eq!(revealed["scrolled"], false);
    let cancel = id(&p, "confirm-cancel");
    ask(&mut p, format!(r#"{{"op":"tap","id":{cancel}}}"#));
    assert!(p.surface().is_none());

    // `tap <node> contextmenu` is the finger held on it: its `contextmenu`
    // runs, the menu it names opens, and a row is chosen by its node.
    let held = id(&p, "held");
    let opened = ask(
        &mut p,
        format!(r#"{{"op":"tap","id":{held},"contextmenu":true}}"#),
    );
    assert_eq!(opened["tapped"], held, "{opened}");
    assert_eq!(text(&p, "aimed"), "row");
    assert_eq!(p.surface().map(|s| s.kind), Some(SurfaceKind::Menu));
    let share = id(&p, "menu-share");
    ask(&mut p, format!(r#"{{"op":"reveal","id":{share}}}"#));
    ask(&mut p, format!(r#"{{"op":"tap","id":{share}}}"#));
    assert_eq!(text(&p, "said"), "sent");
    // The row's own press did not run.
    assert!(p.surface().is_none());
}

/// The menu's motion grows from the corner by its anchor (D10, as ruled):
/// the statement says which, and the shell's window animates from it.
#[test]
fn a_menu_grows_from_the_corner_by_its_anchor() {
    let menu = (200.0, 96.0);
    // Under its anchor from its start; ending at its end; over it.
    let below = (100.0, 100.0, 40.0, 40.0);
    assert_eq!(look::origin(below, (100.0, 140.0), menu), 0, "top start");
    assert_eq!(
        look::origin((300.0, 100.0, 40.0, 40.0), (140.0, 140.0), menu),
        1
    );
    assert_eq!(
        look::origin((100.0, 700.0, 40.0, 40.0), (100.0, 604.0), menu),
        2
    );
    assert_eq!(
        look::origin((300.0, 700.0, 40.0, 40.0), (140.0, 604.0), menu),
        3
    );
    // A context menu opens from where the finger is.
    assert_eq!(look::origin((50.0, 50.0, 0.0, 0.0), (50.0, 50.0), menu), 0);
    // The statement carries it beside what dismisses the menu: this one
    // ends at its invoker's end and hangs under it.
    let (mut p, _) = windowed();
    tap(&mut p, "more").unwrap();
    p.tell_shell();
    let words = p.take_requests().unwrap();
    let at = words.iter().position(|w| *w == 7).unwrap();
    assert_eq!(words[at + 5], 0b0111, "Back and outside dismiss; top end");
    // An alert's has none.
    let (mut p, _) = windowed();
    tap(&mut p, "ask").unwrap();
    p.tell_shell();
    let words = p.take_requests().unwrap();
    let at = words.iter().position(|w| *w == 7).unwrap();
    assert_eq!(words[at + 5], 0b0001);
}

#[test]
fn popover_commands_use_the_existing_surface_and_report_toggles() {
    let (mut p, _) = windowed();
    let args = [exact_plan::Value::str("menu")];
    p.popover_command("showPopover", &args);
    p.popover_command("showPopover", &args);
    p.run_commands(|| ());
    assert!(p.surface().is_some());
    assert_eq!(
        p.host.runner().slot("toggled"),
        Some(&exact_plan::Value::str("closed>open"))
    );
    p.popover_command("hidePopover", &args);
    p.run_commands(|| ());
    assert!(p.surface().is_none());
    assert_eq!(
        p.host.runner().slot("toggled"),
        Some(&exact_plan::Value::str("open>closed"))
    );
    p.popover_command("showPopover", &args);
    p.run_commands(|| ());
    p.key_down("Escape", 3.0);
    p.run_commands(|| ());
    assert!(p.surface().is_none());
    assert_eq!(
        p.host.runner().slot("toggled"),
        Some(&exact_plan::Value::str("open>closed"))
    );
    p.popover_command("togglePopover", &args);
    p.popover_command("togglePopover", &args);
    p.run_commands(|| ());
    assert_eq!(
        p.host.runner().slot("toggled"),
        Some(&exact_plan::Value::str("closed>closed"))
    );
}

#[test]
fn focus_after_show_selects_the_shell_menu_row_and_escape_restores_the_trigger() {
    let (mut p, _) = windowed();
    let trigger = id(&p, "more");
    let second = id(&p, "menu-share");
    p.set_focus(Some(trigger), 0.0);
    p.popover_command("showPopover", &[Value::str("menu"), Value::str("trigger")]);
    assert_eq!(p.focus_command(&[Value::str("trigger")]), None);
    assert_eq!(p.focus(), None);
    let journal = crate::agent::handle(&mut p, r#"{"op":"logs"}"#);
    assert!(
        journal.contains(r#"focus \"trigger\" refused: page under surface is inert"#),
        "{journal}"
    );
    assert_eq!(p.focus_command(&[Value::str("second")]), Some(second));
    assert_eq!(p.top.open.as_ref().unwrap().focused, Some(1));
    assert_eq!(p.focus(), None, "the menu owns focus, not the page");
    p.key_down("ArrowDown", 1.0);
    assert_eq!(
        p.top.open.as_ref().unwrap().focused,
        Some(3),
        "advance from the selected row, skipping disabled"
    );
    p.key_down("Escape", 2.0);
    assert_eq!(p.focus(), Some(trigger));
}

#[test]
fn shell_top_layer_reports_viewport_frames_and_modal_visibility_to_reads() {
    let source = APP
        .replace(
            "  state said",
            r#"  state hit = ""
  state hits = []
  state shown = []
  state bounds = []
  action probe(x: number, y: number)
    hit = match elementFromPoint(x, y) { case some(id) => id, case none => "none" }
    hits = elementsFromPoint(x, y)
    shown = querySelectorAll("[data-paint~=\"accent\"]")
    bounds = [frame("menu").x, frame("menu").y, frame("menu").width, frame("menu").height]
  state said"#,
        )
        .replace(
            "main navigationKey",
            "main id=\"page\" data-paint=\"accent\" navigationKey",
        )
        .replace(
            "id=\"menu\" popover",
            "id=\"menu\" data-paint=\"accent\" popover",
        )
        .replace(
            "id=\"confirm\" role",
            "id=\"confirm\" data-paint=\"accent\" role",
        )
        .replace(
            "dialog id=\"light\"",
            "column id=\"light\" popover=\"auto\" data-paint=\"accent\"",
        )
        .replace(
            "commandfor=\"light\" command=\"close\"",
            "popovertarget=\"light\" popovertargetaction=\"hide\"",
        );
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(&source),
        (),
        (400., 800.),
        2.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.serve_requests();
    p.serve_windows(Box::new(Pixels(Rc::new(RefCell::new(Vec::new())))));
    let probe = |p: &mut Presenter<()>, x: f64, y: f64| {
        p.host
            .runner_mut()
            .act("probe", vec![Value::Number(x), Value::Number(y)])
            .unwrap();
    };
    probe(&mut p, 1.0, 1.0);
    assert_eq!(
        p.host.runner().slot("shown"),
        Some(&Value::list(vec![Value::str("page")]))
    );
    tap(&mut p, "more").unwrap();
    for (width, height) in [(400.0, 800.0), (320.0, 400.0)] {
        assert!(p
            .set_window(super::super::window::Window {
                size: (width, height),
                bars: [24.0, 0.0, 0.0, 13.0],
                ..Default::default()
            })
            .is_none());
        let frame = told(&mut p).into_iter().find(|s| s.1 == 2).unwrap().2;
        let [px, py, ..] = p.chrome().frame;
        let (x, y) = (
            f64::from(frame[0]) / 2.0 - f64::from(px) + 1.0,
            f64::from(frame[1]) / 2.0 - f64::from(py) + 1.0,
        );
        probe(&mut p, x, y);
        assert_eq!(p.host.runner().slot("hit"), Some(&Value::str("menu")));
        assert_eq!(
            p.host.runner().slot("bounds"),
            Some(&Value::list(vec![
                Value::Number(x - 1.0),
                Value::Number(y - 1.0),
                Value::Number(f64::from(frame[2]) / 2.0),
                Value::Number(f64::from(frame[3]) / 2.0),
            ]))
        );
        assert_eq!(
            p.host.runner().slot("hits"),
            Some(&Value::list(vec![Value::str("menu"), Value::str("page")]))
        );
        assert_eq!(
            p.host.runner().slot("shown"),
            Some(&Value::list(vec![Value::str("page"), Value::str("menu")]))
        );
        probe(&mut p, 1.0, 1.0);
        assert_eq!(p.host.runner().slot("hit"), Some(&Value::str("page")));
    }
    p.key_down("Escape", 1.0);
    probe(&mut p, 1.0, 1.0);
    assert_eq!(p.host.runner().slot("hit"), Some(&Value::str("page")));
    assert_eq!(
        p.host.runner().slot("shown"),
        Some(&Value::list(vec![Value::str("page")]))
    );
    tap(&mut p, "ask").unwrap();
    probe(&mut p, 1.0, 1.0);
    assert_eq!(p.host.runner().slot("hit"), Some(&Value::str("none")));
    assert_eq!(p.host.runner().slot("hits"), Some(&Value::list(vec![])));
    assert_eq!(
        p.host.runner().slot("shown"),
        Some(&Value::list(vec![Value::str("confirm")]))
    );
    let serial = p.surface().unwrap().serial;
    p.surface_event(serial, event::REFUSED, 0.0, 0.0, 2.0);
    probe(&mut p, 1.0, 1.0);
    assert_eq!(
        p.host.runner().slot("shown"),
        Some(&Value::list(vec![Value::str("page")]))
    );
    p.popover_command("showPopover", &[Value::str("light")]);
    assert_eq!(p.surface().unwrap().kind, SurfaceKind::Alert);
    probe(&mut p, 1.0, 1.0);
    assert_eq!(p.host.runner().slot("hit"), Some(&Value::str("page")));
    assert_eq!(
        p.host.runner().slot("shown"),
        Some(&Value::list(vec![Value::str("page"), Value::str("light")]))
    );
}

#[test]
fn shell_surface_geometry_tracks_insets_cutouts_and_scale() {
    let source = r#"component App
  state bounds = []
  state hit = ""
  action probe(x: number, y: number)
    bounds = [frame("menu").x, frame("menu").y, frame("menu").width, frame("menu").height]
    hit = match elementFromPoint(x, y) { case some(id) => id, case none => "none" }
  action done
    hit = "done"
  view
    column viewport-fit="cover" width="100%" height="100%"
      button popovertarget="menu" testId="more" position="absolute" left=20 top=0 width=100 height=40
        text "More"
      column id="menu" popover="auto" role="menu"
        button press=done popovertarget="menu" popovertargetaction="hide"
          text "Done"
"#;
    let mut failures = Vec::new();
    for (name, anchor, bars, cutout, scale) in [
        ("top bar", 0, [96., 0., 48., 0.], [0.; 4], 1.),
        ("top cutout", 0, [48., 0., 48., 0.], [96., 0., 0., 0.], 1.),
        ("bottom bar", 740, [48., 0., 160., 0.], [0.; 4], 1.),
        (
            "bottom cutout",
            740,
            [48., 0., 48., 0.],
            [0., 0., 160., 0.],
            1.,
        ),
        ("scale", 0, [48., 0., 48., 0.], [0.; 4], 2.),
    ] {
        let source = source.replace("top=0", &format!("top={anchor}"));
        let (mut p, error) = Presenter::boot_with(
            &crate::test_plan(&source),
            (),
            (400., 800.),
            1.,
            PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(error.is_none(), "{error:?}");
        p.serve_requests();
        p.serve_windows(Box::new(Pixels(Rc::new(RefCell::new(Vec::new())))));
        let window = Window {
            size: (400., 800.),
            bars: [48., 0., 48., 0.],
            ..Default::default()
        };
        assert!(p.set_window(window).is_none());
        tap(&mut p, "more").unwrap();
        let initial = told(&mut p).into_iter().find(|s| s.1 == 2).unwrap().2;
        assert!(p
            .set_window(Window {
                bars,
                cutout,
                ..window
            })
            .is_none());
        assert!(p.resize_scaled(400., 800., scale).is_none());
        let sent = told(&mut p);
        let updated = sent.iter().find(|s| s.1 == 2).map(|s| s.2);
        // Compare with the last statement delivered, even when no update arrived.
        let last = updated.unwrap_or(initial);
        let rect = last.map(|v| f64::from(v) / f64::from(scale));
        p.host
            .runner_mut()
            .act(
                "probe",
                vec![Value::Number(rect[0] + 1.), Value::Number(rect[1] + 1.)],
            )
            .unwrap();
        let expected = Value::list(rect.into_iter().map(Value::Number).collect());
        if updated.is_none()
            || p.host.runner().slot("bounds") != Some(&expected)
            || p.host.runner().slot("hit") != Some(&Value::str("menu"))
        {
            failures.push(format!(
                "{name}: shell={last:?}, updated={updated:?}, runner={:?}, hit={:?}",
                p.host.runner().slot("bounds"),
                p.host.runner().slot("hit")
            ));
        }
        assert!(
            told(&mut p).is_empty(),
            "an unchanged surface is not sent twice"
        );
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn mouse_invocation_keeps_the_nearest_press_handler() {
    let mut actual = Vec::new();
    let mut expected = Vec::new();
    for handler in ["child", "invoker", "parent"] {
        for feedback in ["", "-exact-press-opacity=0.5"] {
            let at = |name| if handler == name { "press=record" } else { "" };
            let source = format!(
                r#"component App
  state seen = ""
  action record
    seen = `${{seen}}press `
  view
    column width=300 height=200
      box width=200 height=80 {parent} {feedback}
        button testId="invoker" popovertarget="menu" width=160 height=60 {invoker} {feedback}
          box testId="child" width=100 height=40 {child} {feedback}
      column id="menu" popover="auto" role="menu"
        button press=record
          text "First"
"#,
                parent = at("parent"),
                invoker = at("invoker"),
                child = at("child"),
            );
            let (mut p, error) = Presenter::boot_with(
                &crate::test_plan(&source),
                (),
                (300., 200.),
                1.,
                PathBuf::new(),
                PainterChoice::Cpu,
            )
            .unwrap();
            assert!(error.is_none(), "{error:?}");
            p.serve_requests();
            p.serve_windows(Box::new(Pixels(Rc::new(RefCell::new(Vec::new())))));
            let (x, y, w, h) = p.rect_of(id(&p, "child")).unwrap();
            p.pointer_down(x + w / 2., y + h / 2., 1.).unwrap();
            p.pointer_up(x + w / 2., y + h / 2., 2.).unwrap();
            actual.push((
                handler,
                feedback,
                p.host.runner().slot("seen").cloned(),
                p.surface().is_some(),
            ));
            expected.push((handler, feedback, Some(Value::str("press ")), true));
        }
    }
    assert_eq!(actual, expected);
}
