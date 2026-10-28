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

#[cfg(target_os = "windows")]
#[test]
fn windows_system_menu_chord_does_not_choose_a_dialog_button() {
    for (button, alt, release_alt_first) in [
        ("dpm-cancel", "AltLeft", false),
        ("dpm-keep", "AltRight", true),
    ] {
        let mut p = windowed();
        p.serve_overlays();
        let frame = p.display_frame().unwrap();
        assert!(p.display_complete(&frame));
        invoke(&mut p, "open-dpm");
        if button == "dpm-keep" {
            p.hardware_key("Tab", "Tab", true, false);
            p.hardware_key("Tab", "Tab", false, false);
        }
        let focused = |p: &Presenter<()>| {
            let open = p.top.open.as_ref().expect("dialog remains open");
            open.layout.targets[open.focused.unwrap()].node
        };
        assert_eq!(focused(&p), id(&p, button));
        p.hardware_key(alt, "Alt", true, false);
        for repeat in [false, true] {
            p.hardware_key("Space", "Space", true, repeat);
            p.run_commands(|| ());
            assert_eq!(focused(&p), id(&p, button));
            assert_eq!(p.host().runner().slot("said"), Some(&Value::str("nothing")));
        }
        if release_alt_first {
            p.hardware_key(alt, "Alt", false, false);
        }
        p.hardware_key("Space", "Space", false, false);
        p.hardware_key(alt, "Alt", false, false);
        assert_eq!(focused(&p), id(&p, button));
        p.hardware_key("Space", "Space", true, false);
        p.hardware_key("Space", "Space", false, false);
        p.run_commands(|| ());
        assert!(
            p.top.open.is_none(),
            "ordinary Space still chooses the button"
        );
        let said = if button == "dpm-keep" {
            "kept"
        } else {
            "nothing"
        };
        assert_eq!(p.host().runner().slot("said"), Some(&Value::str(said)));
    }
}

#[test]
fn desktop_menu_overflow_keeps_focus_pointer_and_accessibility_inside_the_window() {
    for scale in [1., 2.25] {
        let mut p = windowed();
        p.serve_overlays();
        assert!(p.resize(360., 140.).is_none());
        assert!(p.set_text_scale(scale).is_none());
        p.frame();
        invoke(&mut p, "native-cmd");
        let visible = |p: &Presenter<()>, index: usize| {
            let open = p.top.open.as_ref().unwrap();
            let (_, top, _, height) = open.layout.menu_clip().unwrap();
            let (_, y, _, h) = open.layout.targets[index].rect;
            assert!(y >= top - 0.01 && y + h <= top + height + 0.01);
            assert!(open.frame[1] >= 4 && open.frame[1] + open.frame[3] <= 136);
        };
        visible(&p, 0);
        assert!(p.reader_access(1., true).is_some());
        p.surface_key("End", 1.);
        visible(&p, 3);
        assert!(
            p.reader_access(1., false).is_some(),
            "scrolling invalidates accessible geometry"
        );
        let nodes = p.access_tree().0;
        assert!(nodes.iter().any(|n| n.name == "Ask first"));
        assert!(nodes
            .iter()
            .all(|n| n.rect[1] >= 0. && n.rect[1] + n.rect[3] <= 132.));
        p.surface_key("ArrowUp", 2.);
        visible(&p, 2);
        p.surface_key("Home", 3.);
        visible(&p, 0);
        p.surface_key("ArrowUp", 4.);
        visible(&p, 3);
        p.surface_key("Home", 5.);
        let open = p.top.open.as_ref().unwrap();
        let [left, top, width, height] = open.frame.map(|v| v as f32 / open.scale);
        let revision = open.revision;
        p.wheel_at(left - 1., top + 20., 0., 1000.);
        assert_eq!(p.top.open.as_ref().unwrap().revision, revision);
        p.wheel_at(left + 20., top + 20., 0., 1000.);
        visible(&p, 3);
        p.surface_key("Home", 5.1);
        visible(&p, 0);
        p.reader_access(1., false);
        let scroller = p
            .access_told()
            .iter()
            .find(|n| n.role == access::role::SCROLLER)
            .unwrap()
            .id;
        assert!(p.access_act(scroller, access::act::FORWARD, 0., 5.2));
        assert!(p.reader_access(1., false).is_some());
        assert!(p.top.open.as_ref().unwrap().layout.menu_scroll().unwrap().0 > 0.);
        p.wheel_at(left + 20., top + 20., 0., -1000.);
        visible(&p, 0);
        let x = left + width - 7.;
        p.overlay_pointer(0, x, top + 8., 6.);
        p.overlay_pointer(2, x, top + height - 8., 7.);
        p.overlay_pointer(1, x, top + height - 8., 8.);
        visible(&p, 3);
        assert!(p.surface().is_some(), "dragging the thumb chooses no item");
        p.surface_key("End", 9.);
        assert!(p.resize(360., 120.).is_none());
        p.frame();
        visible(&p, 3);
        p.surface_key("Home", 10.);
        let open = p.top.open.as_ref().unwrap();
        let target = open.layout.targets[0].rect;
        let x = open.frame[0] as f32 / open.scale + target.0 + 8.;
        let y = open.frame[1] as f32 / open.scale + target.1 + 8.;
        p.overlay_pointer(0, x, y, 11.);
        p.overlay_pointer(1, x, y, 12.);
        assert!(p.surface().is_none());
        assert_eq!(p.host().runner().slot("said"), Some(&Value::str("renamed")));
    }
}

#[test]
fn dialog_opened_from_a_menu_restores_focus_after_escape_and_text_scaling() {
    for scale in [false, true] {
        let mut p = windowed();
        p.serve_overlays();
        let frame = p.display_frame().unwrap();
        assert!(p.display_complete(&frame));
        let trigger = id(&p, "native-cmd");
        invoke(&mut p, "native-cmd");
        p.choose(3, 1., None);
        assert_eq!(open(&p).as_deref(), Some("dpm"));
        if scale {
            for factor in [2.25, 1.] {
                assert!(p.set_text_scale(factor).is_none());
                let frame = p.display_frame().unwrap();
                assert!(p.display_complete(&frame));
            }
        }
        p.hardware_key("Escape", "Escape", true, false);
        p.hardware_key("Escape", "Escape", false, false);
        p.run_commands(|| ());
        assert!(p.surface().is_none());
        assert_eq!(p.focus(), Some(trigger));
        p.hardware_key("Tab", "Tab", true, false);
        p.hardware_key("Tab", "Tab", false, false);
        assert_eq!(p.focus(), Some(id(&p, "wrapper")));
        p.set_focus(Some(trigger), 2.);
        invoke(&mut p, "native-cmd");
        p.hardware_key("Escape", "Escape", true, false);
        p.hardware_key("Escape", "Escape", false, false);
        assert!(p.surface().is_none());
        assert_eq!(p.focus(), Some(trigger));
    }
}

#[test]
fn desktop_alert_pointer_motion_preserves_focus_and_tab_follows_visual_order() {
    let mut p = windowed();
    p.serve_overlays();
    let frame = p.display_frame().unwrap();
    assert!(p.display_complete(&frame));
    invoke(&mut p, "open-dpm");
    let cancel = id(&p, "dpm-cancel");
    let keep = id(&p, "dpm-keep");
    let focused = |p: &Presenter<()>| {
        let open = p.top.open.as_ref().unwrap();
        open.layout.targets[open.focused.unwrap()].node
    };
    assert_eq!(focused(&p), cancel);
    p.overlay_pointer(2, 1., 1., 1.);
    assert_eq!(focused(&p), cancel);
    let open = p.top.open.as_ref().unwrap();
    let index = open
        .layout
        .targets
        .iter()
        .position(|t| t.node == keep)
        .unwrap();
    let target = open.layout.targets[index].rect;
    let x = open.frame[0] as f32 / open.scale + target.0 + 8.;
    let y = open.frame[1] as f32 / open.scale + target.1 + 8.;
    p.overlay_pointer(2, x, y, 1.1);
    assert_eq!(focused(&p), cancel);
    assert_eq!(p.top.open.as_ref().unwrap().hovered, Some(index));
    let revision = p.top.open.as_ref().unwrap().revision;
    p.dirty = false;
    p.overlay_pointer(2, x + 1., y + 1., 1.2);
    assert_eq!(p.top.open.as_ref().unwrap().revision, revision);
    assert!(!p.dirty, "unchanged hover does not repaint the overlay");
    p.pointer_lost(1.21).unwrap();
    assert_eq!(p.top.open.as_ref().unwrap().hovered, None);
    assert_eq!(focused(&p), cancel);
    p.overlay_pointer(0, x, y, 1.22);
    p.pointer_cancel(1.23).unwrap();
    assert_eq!(p.top.open.as_ref().unwrap().pressed, None);
    p.overlay_pointer(1, x, y, 1.24);
    assert!(
        p.surface().is_some(),
        "a lost capture cannot activate on release"
    );
    p.overlay_pointer(0, x, y, 1.3);
    assert_eq!(p.top.open.as_ref().unwrap().pressed, Some(index));
    p.overlay_pointer(2, 1., 1., 1.4);
    assert_eq!(p.top.open.as_ref().unwrap().pressed, None);
    p.overlay_pointer(1, x, y, 1.5);
    assert!(
        p.surface().is_some(),
        "dragging away cancels the button press"
    );
    p.surface_key("Tab", 2.);
    assert_eq!(focused(&p), keep);
    p.surface_key("Tab", 3.);
    assert_eq!(focused(&p), cancel);
    p.surface_key("Enter", 4.);
    assert!(p.surface().is_none());
    assert_eq!(p.host.runner().slot("said"), Some(&Value::str("nothing")));
}

#[cfg(target_os = "windows")]
#[test]
fn windows_dialog_focus_outline_follows_input_without_losing_action() {
    fn outlined(p: &mut Presenter<()>) -> bool {
        let open = p.top.open.as_ref().unwrap();
        let (x, y, _, h) = open.layout.targets[open.focused.unwrap()].rect;
        let x = open.frame[0] as f32 / open.scale + x - 2.;
        let y = open.frame[1] as f32 / open.scale + y + h / 2.;
        p.frame().pixel(x as u32, y as u32).unwrap().red() < 100
    }
    for keyboard in [false, true] {
        let mut p = windowed();
        p.serve_overlays();
        p.frame();
        if keyboard {
            p.set_focus(Some(id(&p, "open-dpm")), 0.);
            p.key_down("Enter", 1.);
            p.key_up("Enter", "Enter", 2.);
            p.run_commands(|| ());
        } else {
            invoke(&mut p, "open-dpm");
        }
        assert_eq!(
            outlined(&mut p),
            keyboard,
            "opening input controls the ring"
        );
        p.key_down("Tab", 3.);
        p.key_up("Tab", "Tab", 4.);
        assert!(outlined(&mut p));
        let open = p.top.open.as_ref().unwrap();
        let keep = open.layout.targets[open.focused.unwrap()].node;
        assert_eq!(keep, id(&p, "dpm-keep"));
        let x = open.frame[0] as f32 / open.scale + 30.;
        let y = open.frame[1] as f32 / open.scale + 40.;
        p.overlay_pointer(2, x, y, 5.);
        assert!(outlined(&mut p), "hover does not hide keyboard feedback");
        p.overlay_pointer(0, x, y, 6.);
        p.overlay_pointer(1, x, y, 7.);
        assert!(!outlined(&mut p), "a body click hides the ring");
        let open = p.top.open.as_ref().unwrap();
        assert_eq!(open.layout.targets[open.focused.unwrap()].node, keep);
        p.key_down("Enter", 8.);
        p.key_up("Enter", "Enter", 9.);
        p.run_commands(|| ());
        assert!(p.top.open.is_none());
        assert_eq!(p.host().runner().slot("said"), Some(&Value::str("kept")));
    }
}

#[test]
fn desktop_dialog_wheel_is_confined_to_content_and_does_not_move_focus() {
    let source = APP.replace("Modally.", &"A long explanation. ".repeat(100));
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(&source),
        (),
        (400., 500.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none());
    p.serve_overlays();
    let frame = p.display_frame().unwrap();
    assert!(p.display_complete(&frame));
    invoke(&mut p, "open-dpm");
    let open = p.top.open.as_ref().unwrap();
    let (left, top) = (
        open.frame[0] as f32 / open.scale,
        open.frame[1] as f32 / open.scale,
    );
    let button = open.layout.targets[0].rect;
    let (revision, focused) = (open.revision, open.focused);
    for (x, y) in [(1., 1.), (left + button.0 + 4., top + button.1 + 4.)] {
        p.wheel_at(x, y, 0., 120.);
        assert_eq!(p.top.open.as_ref().unwrap().revision, revision);
    }
    p.wheel_at(left + 24., top + 24., 0., 120.);
    let open = p.top.open.as_ref().unwrap();
    assert!(open.revision > revision);
    assert_eq!(open.focused, focused);
    p.surface_key("End", 1.);
    let revision = p.top.open.as_ref().unwrap().revision;
    p.wheel_at(left + 24., top + 24., 0., 120.);
    assert_eq!(
        p.top.open.as_ref().unwrap().revision,
        revision,
        "scrolling at the end does no work"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn windows_dialog_casts_a_shadow_outside_its_hit_area() {
    fn shadow(p: &Presenter<()>) -> Arc<Pixmap> {
        p.brush
            .top_surface
            .as_ref()
            .unwrap()
            .shadow
            .as_ref()
            .unwrap()
            .0
            .clone()
    }
    let mut p = windowed();
    p.serve_overlays();
    p.frame();
    invoke(&mut p, "open-dpm");
    let [left, top, width, height] = p.top.open.as_ref().unwrap().frame;
    let x = (left + width / 2) as u32;
    let bottom = (top + height) as u32;
    let pixels = p.frame();
    let near = pixels.pixel(x, bottom + 20).unwrap().red();
    let far = pixels.pixel(x, bottom + 100).unwrap().red();
    assert!(
        far > near + 10,
        "the dialog casts a soft shadow: {near}, {far}"
    );
    p.overlay_pointer(0, x as f32, bottom as f32 + 20., 1.);
    p.overlay_pointer(1, x as f32, bottom as f32 + 20., 2.);
    assert_eq!(
        p.top.open.as_ref().unwrap().frame,
        [left, top, width, height]
    );
    assert_eq!(p.host().runner().slot("said"), Some(&Value::str("nothing")));
    let light = shadow(&p);
    p.surface_key("Tab", 2.1);
    p.frame();
    assert!(Arc::ptr_eq(&light, &shadow(&p)), "focus reuses the shadow");
    p.set_system_scheme(true);
    let mut preferences = p.host().runner().viewport().preferences;
    preferences.dark = true;
    assert_eq!(p.set_preferences(preferences), None);
    p.frame();
    let dark = shadow(&p);
    assert!(!Arc::ptr_eq(&light, &dark), "theme invalidates the shadow");
    let sample = |pixels: &Pixmap, scale: f32| {
        pixels
            .pixel(pixels.width() / 2, ((height + 52) as f32 * scale) as u32)
            .unwrap()
            .alpha()
    };
    assert!(
        sample(&dark, 1.) > sample(&light, 1.),
        "dark theme has a stronger shadow"
    );
    for scale in [1.25, 1.5, 2.] {
        assert_eq!(p.resize_scaled(400., 800., scale), None);
        p.frame();
        let scaled = shadow(&p);
        assert_eq!(scaled.width(), (dark.width() as f32 * scale).round() as u32);
        assert_eq!(
            scaled.height(),
            (dark.height() as f32 * scale).round() as u32
        );
        assert!(sample(&scaled, scale).abs_diff(sample(&dark, 1.)) <= 1);
    }
    let released = Arc::downgrade(&shadow(&p));
    p.surface_key("Escape", 3.);
    assert!(p.top.open.is_none());
    p.frame();
    assert!(p.brush.top_surface.is_none());
    assert!(
        released.upgrade().is_none(),
        "dismissal frees the shadow cache"
    );
}

#[test]
fn desktop_menu_home_and_end_skip_disabled_rows_and_keep_focus_inside() {
    let source = r#"component App
  state said = "nothing"
  action rename
    said = "renamed"
  action send
    said = "sent"
  view
    column
      button testId="trigger" popovertarget="menu"
        text "More"
      column id="menu" popover="auto" role="menu"
        button press=send disabled=true
          text "Unavailable first"
        button press=rename testId="rename" popovertarget="menu" popovertargetaction="hide"
          text "Rename"
        hr
        button press=send disabled=true
          text "Unavailable middle"
        button press=send testId="send" popovertarget="menu" popovertargetaction="hide"
          text "Send"
        button press=send disabled=true
          text "Unavailable last"
"#;
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (360., 500.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.serve_overlays();
    p.frame();
    let trigger = id(&p, "trigger");
    p.set_focus(Some(trigger), 0.);
    p.key_down("Enter", 0.);
    p.key_up("Enter", "Enter", 0.);
    let open = p.top.open.as_ref().unwrap();
    assert_eq!(
        open.layout.targets[open.focused.unwrap()].node,
        id(&p, "rename")
    );
    for (at, key, name) in [
        (1., "End", "send"),
        (3., "Home", "rename"),
        (5., "Home", "rename"),
    ] {
        p.key_down(key, at);
        p.key_up(key, key, at + 1.);
        let open = p.top.open.as_ref().unwrap();
        assert_eq!(
            open.layout.targets[open.focused.unwrap()].node,
            id(&p, name)
        );
        assert_eq!(p.focus(), None);
        assert_eq!(p.host.runner().slot("said"), Some(&Value::str("nothing")));
    }
    p.key_down("Enter", 7.);
    p.key_up("Enter", "Enter", 8.);
    assert!(p.surface().is_none());
    assert_eq!(p.host.runner().slot("said"), Some(&Value::str("renamed")));
    assert_eq!(p.focus(), Some(trigger));

    p.tap(trigger).unwrap();
    assert_eq!(p.top.open.as_ref().unwrap().focused, None);
    p.key_down("End", 9.);
    p.key_up("End", "End", 10.);
    p.key_down("ArrowUp", 11.);
    p.key_up("ArrowUp", "ArrowUp", 12.);
    let open = p.top.open.as_ref().unwrap();
    assert_eq!(
        open.layout.targets[open.focused.unwrap()].node,
        id(&p, "rename")
    );
    p.key_down("Escape", 13.);
    p.key_up("Escape", "Escape", 14.);
    assert!(p.surface().is_none());
    assert_eq!(p.focus(), Some(trigger));
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

#[test]
fn a_shell_dismissal_fires_cancel_but_cannot_keep_the_window_open() {
    let source = APP.replace("  state said", "  state events = \"\"\n  action cancelled\n    preventDefault()\n    events = `${events}cancel,`\n  action closed\n    events = `${events}close,`\n  state said")
        .replace("dialog id=\"dpm\"", "dialog cancel=cancelled close=closed id=\"dpm\"")
        .replace("text said testId=\"said\"", "text events testId=\"events\"\n      text said testId=\"said\"");
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
    p.serve_windows(Box::new(Nothing));
    invoke(&mut p, "open-dpm");
    let serial = p.surface().unwrap().serial;
    p.key_down("Escape", 0.);
    assert_eq!(p.surface().unwrap().serial, serial);
    assert!(p.system_back(0.));
    assert_eq!(p.surface().unwrap().serial, serial);
    p.surface_event(serial, top::event::DISMISSED, 0., 0., 0.);
    p.run_commands(|| ());
    assert!(p.surface().is_none());
    assert_eq!(p.focus(), Some(id(&p, "open-dpm")));
    let events = p
        .host
        .kernel()
        .node(id(&p, "events"))
        .unwrap()
        .text_runs()
        .iter()
        .map(|r| &*r.text)
        .collect::<String>();
    assert_eq!(events, "cancel,cancel,cancel,close,");
    invoke(&mut p, "open-dpm");
    let reopened = p.surface().unwrap().serial;
    p.surface_event(serial, top::event::DISMISSED, 0., 0., 0.);
    assert_eq!(p.surface().unwrap().serial, reopened);
}

/// LLP 1021 D2: a row of the shell's menu chosen by a touch hears where the
/// touch was, in the page's points, the menu's frame being in the window's
/// pixels: at 2.75 a row of a menu at (100, 200) is near (110, 212), not
/// its pixels.
#[test]
fn a_menu_rows_press_point_is_in_points_at_any_scale() {
    let source = r#"component App
  state at = ""
  action pick(e: MouseEvent)
    at = `${e.clientX},${e.clientY}`
  view
    column width="100%" height="100%"
      button popovertarget="m" testId="more" margin-left=100 margin-top=180 width=40 height=20
        text "More"
      column id="m" popover="auto" role="menu"
        button press=pick popovertarget="m" popovertargetaction="hide" testId="row"
          text "Row"
      text at testId="at"
"#;
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
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
    assert_eq!(p.resize_scaled(400., 800., 2.75), None);
    let more = id(&p, "more");
    p.tap(more).unwrap();
    p.run_commands(|| ());
    let open = p.top.open.as_ref().expect("the menu is open");
    let (x, y, w, h) = open.layout.targets[0].rect;
    let [px, py, ..] = p.chrome().frame;
    let want = (
        open.frame[0] as f32 / open.scale + x + w / 2. - px,
        open.frame[1] as f32 / open.scale + y + h / 2. - py,
    );
    assert!(want.0 < 400. && want.1 < 800., "{want:?} is in the page");
    choose(&mut p, "row");
    let node = p.host().kernel().node(id(&p, "at")).unwrap();
    let said: String = node.text_runs().iter().map(|r| &*r.text).collect();
    let got: Vec<f32> = said.split(',').map(|n| n.parse().unwrap()).collect();
    assert!(
        (got[0] - want.0).abs() < 0.01 && (got[1] - want.1).abs() < 0.01,
        "{said} want {want:?}"
    );
}
