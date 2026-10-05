use super::*;
#[path = "surface_controls_tests/wheel.rs"]
mod wheel;
#[derive(Default)]
struct NoData;
impl DataSource for NoData {
    fn query(
        &mut self,
        n: &str,
        _: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, exact_runner::DataError> {
        Err(exact_runner::DataError::UnknownSource(n.into()))
    }
}
fn find(p: &Presenter<NoData>, name: &str) -> u32 {
    p.host
        .preorder()
        .into_iter()
        .find(|id| {
            p.host
                .kernel()
                .node(*id)
                .unwrap()
                .props
                .str(exact_kernel::PropId::TestId)
                == Some(name)
        })
        .unwrap()
}
fn fixture() -> (Presenter<NoData>, PathBuf) {
    fixture_with_hud_removal(false)
}
fn fixture_with_hud_removal(remove_hud: bool) -> (Presenter<NoData>, PathBuf) {
    let source = r#"component Controls
  state removed = false
  state text = ""
  state spins = 0
  action spun(e: WheelEvent)
    spins = spins + 1
  action change(value: string)
    text = value
  action remove
    removed = true
  view
    column
      input testId="editor" value=text input=change width=100 height=30
      canvas testId="a" width=100 height=100
        when !removed
          button testId="a-jump" action="jump" width=100 height=100
            box testId="label" width=100 height=100
      canvas testId="b" width=100 height=100
        button testId="b-jump" action="jump" width=100 height=100
      canvas testId="raw" wheel=spun width=100 height=100
        button testId="hud-remove" press=remove width=100 height=30
      button testId="remove" press=remove width=100 height=30
"#;
    let source = if remove_hud {
        source.replace(
            "        button testId=\"hud-remove\" press=remove width=100 height=30",
            "        when !removed\n          button testId=\"hud-remove\" press=remove width=100 height=30",
        )
    } else {
        source.to_owned()
    };
    boot(&source, &["a", "b", "raw"])
}
/// A presenter over `source` whose canvases named by testId are worlds of the probe module.
fn boot(source: &str, canvases: &[&str]) -> (Presenter<NoData>, PathBuf) {
    let (path, compat) = super::tests::fixture();
    let plan = contract::compile(source).unwrap();
    let (mut p, _) = Presenter::boot(
        &plan.encode(),
        NoData,
        (100., 360.),
        1.,
        path.parent().unwrap().into(),
    )
    .unwrap();
    p.surfaces
        .abis
        .insert(String::new(), Abi::open_path(&path, &compat, "").unwrap());
    for (i, name) in canvases.iter().enumerate() {
        let view = find(&p, name);
        p.surfaces.canvases.insert(
            view,
            Canvas {
                id: i as u32 + 1,
                name: (*name).into(),
                artifact: String::new(),
                owner: true,
                since: 0,
                held: Default::default(),
                restored_controls: Default::default(),
                restore_error: None,
                restore_input: false,
                restore_bytes: None,
                restore_logged: false,
            },
        );
    }
    p.boxes();
    (p, path)
}
fn restore(p: &mut Presenter<NoData>, name: &str, contacts: Value) {
    let id = find(p, name);
    let c = p.surfaces.canvases.get_mut(&id).unwrap();
    c.restore_input = true;
    c.finish_restore(
        None,
        Some(&json!({"world":{"restored":true,"input":{"controlContacts":contacts}}})),
    );
    p.restore_controls();
}
fn done(p: Presenter<NoData>, path: PathBuf) {
    drop(p);
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
#[test]
fn surface_qualified_contacts_and_repeated_restore() {
    let (mut p, path) = fixture();
    for name in ["a", "b"] {
        restore(&mut p, name, json!([{"id":7,"action":"jump"}]));
    }
    assert_eq!(p.control_bindings.len(), 2);
    let b = find(&p, "b");
    assert_eq!(
        p.control_tap(&json!({"id":b,"contact":7,"phase":"up"}))
            .unwrap()["delivery"],
        "recognized"
    );
    assert_eq!(p.control_bindings.len(), 1);
    restore(&mut p, "a", json!([]));
    assert!(p.control_bindings.is_empty());
    done(p, path);
}
#[test]
fn restored_lookup_stays_inside_surface_and_missing_control_cancels() {
    let (mut p, path) = fixture();
    let button = find(&p, "b-jump");
    restore(&mut p, "b", json!([{"id":7,"action":"jump"}]));
    assert_eq!(
        p.control_bindings.values().next().unwrap().view,
        Some(button)
    );
    assert!(p.control_input(button, "up", 0., 0., 7, 0.));
    restore(&mut p, "a", json!([{"id":8,"action":"old-action"}]));
    p.cancel_removed_controls();
    assert!(p.control_bindings.is_empty());
    done(p, path);
}
#[test]
fn replaced_canvas_drops_down_before_next_press() {
    let (mut p, path) = fixture();
    let a = find(&p, "a");
    let button = find(&p, "a-jump");
    assert!(p.control_input(button, "down", 0., 0., 1, 0.));
    p.surfaces.canvases.get_mut(&a).unwrap().id = 99;
    p.cancel_removed_controls();
    assert!(p.control_bindings.is_empty());
    assert!(p.control_input(button, "down", 0., 0., 1, 0.));
    p.surfaces.canvases.remove(&a);
    p.cancel_removed_controls();
    assert!(p.control_bindings.is_empty());
    done(p, path);
}
#[test]
fn passive_control_descendant_taps_and_editor_keeps_focus() {
    let (mut p, path) = fixture();
    let label = find(&p, "label");
    let editor = find(&p, "editor");
    p.focus = Some(editor);
    assert!(p.tap(label).is_ok());
    assert_eq!(p.focus(), Some(editor));
    done(p, path);
}
#[test]
fn blur_cancels_restored_keyboard_hold_without_focus() {
    let (mut p, path) = fixture();
    restore(&mut p, "a", json!([{"id":4294967294u32,"action":"jump"}]));
    p.focus = None;
    p.blur();
    let abi = &p.surfaces.abis[""];
    let cancels = unsafe { abi.symbol::<unsafe extern "C" fn() -> u32>(b"test_cancels")() };
    assert_eq!(cancels, 1);
    assert!(p.control_bindings.is_empty());
    done(p, path);
}

#[test]
fn restored_contact_cancels_when_control_unmounts() {
    let (mut p, path) = fixture();
    restore(&mut p, "a", json!([{"id":7,"action":"jump"}]));
    let button = find(&p, "a-jump");
    let remove = find(&p, "remove");
    assert!(p
        .host
        .dispatch_at(remove, exact_runner::Event::Press, 0.)
        .is_none());
    assert!(p.host.kernel().node(button).is_none());
    p.cancel_removed_controls();
    assert!(p.control_bindings.is_empty());
    let abi = &p.surfaces.abis[""];
    let cancels = unsafe { abi.symbol::<unsafe extern "C" fn() -> u32>(b"test_cancels")() };
    assert_eq!(cancels, 1);
    done(p, path);
}

#[test]
fn raw_canvas_press_preserves_active_editor() {
    let (mut p, path) = fixture();
    let editor = find(&p, "editor");
    let canvas = find(&p, "raw");
    p.focus = Some(editor);
    p.tap(canvas).unwrap();
    assert_eq!(p.focus(), Some(editor));
    done(p, path);
}

#[test]
fn another_canvas_release_or_unmount_keeps_the_active_contact() {
    let (mut p, path) = fixture();
    for name in ["a", "b"] {
        restore(&mut p, name, json!([{"id":1,"action":"jump"}]));
    }
    let a = find(&p, "a");
    let b_button = find(&p, "b-jump");
    p.control_contact = Some((b_button, 0., 0.));
    p.control_tap(&json!({"id":a,"contact":1,"phase":"up"}))
        .unwrap();
    assert_eq!(p.control_contact, Some((b_button, 0., 0.)));
    restore(&mut p, "a", json!([{"id":1,"action":"jump"}]));
    let remove = find(&p, "remove");
    assert!(p
        .host
        .dispatch_at(remove, exact_runner::Event::Press, 0.)
        .is_none());
    p.cancel_removed_controls();
    assert_eq!(p.control_contact, Some((b_button, 0., 0.)));
    done(p, path);
}

#[test]
fn r12_pressed_control_routes_space_without_stealing_editor() {
    let (mut p, path) = fixture();
    let editor = find(&p, "editor");
    let button = find(&p, "a-jump");
    let a = find(&p, "a");
    p.focus = Some(editor);
    assert!(p.control_input(button, "down", 0., 0., 7, 0.));
    p.hardware_key("Space", "Space", true, false);
    assert_eq!(p.focus(), Some(editor));
    assert_eq!(
        p.control_bindings
            .get(&(a, u32::MAX - 1))
            .map(|b| b.name.as_str()),
        Some("jump")
    );
    p.hardware_key("Space", "Space", false, false);
    assert!(!p.control_bindings.contains_key(&(a, u32::MAX - 1)));
    p.type_key(button, "Space", "Space", true, false).unwrap();
    assert_eq!(p.focus(), Some(editor));
    done(p, path);
}
#[test]
fn r12_duplicate_restored_actions_do_not_guess_an_owner() {
    let (mut p, path) = fixture();
    let a = find(&p, "a");
    let b = find(&p, "b");
    let first = find(&p, "a-jump");
    let second = find(&p, "b-jump");
    p.host.apply_test_ops(&[
        exact_kernel::Op::SetChildren {
            id: b,
            children: vec![],
        },
        exact_kernel::Op::SetChildren {
            id: a,
            children: vec![first, second],
        },
    ]);
    restore(&mut p, "a", json!([{"id":7,"action":"jump"}]));
    assert_eq!(p.control_bindings[&(a, 7)].view, None);
    for (key, contact) in [("Space", u32::MAX - 1), ("Enter", u32::MAX - 2)] {
        p.hardware_key(key, key, true, false);
        assert_eq!(p.control_bindings[&(a, contact)].name, "jump");
        p.hardware_key(key, key, false, false);
        assert!(!p.control_bindings.contains_key(&(a, contact)));
    }
    p.host.apply_test_ops(&[exact_kernel::Op::SetChildren {
        id: a,
        children: vec![],
    }]);
    p.cancel_removed_controls();
    assert!(!p.control_bindings.contains_key(&(a, 7)));

    done(p, path);
}
#[test]
fn r12_reparent_cancels_original_owner() {
    let (mut p, path) = fixture();
    let a = find(&p, "a");
    let b = find(&p, "b");
    let first = find(&p, "a-jump");
    let second = find(&p, "b-jump");
    assert!(p.control_input(first, "down", 0., 0., 7, 0.));
    p.host.apply_test_ops(&[
        exact_kernel::Op::SetChildren {
            id: a,
            children: vec![],
        },
        exact_kernel::Op::SetChildren {
            id: b,
            children: vec![first, second],
        },
    ]);
    p.cancel_removed_controls();
    assert!(p.control_bindings.is_empty());
    done(p, path);
}

#[test]
fn r13_named_and_empty_arguments_reach_linux_gpu_binding() {
    for call in ["world(restart=false, seed=7, paused=true)", "world()"] {
        let (path, compat) = super::tests::fixture();
        let plan = contract::compile(&format!(
            "component App\n  view\n    canvas surface={call} width=100 height=100\n"
        ))
        .unwrap();
        let (mut p, _) = Presenter::boot(
            &plan.encode(),
            NoData,
            (100., 100.),
            1.,
            path.parent().unwrap().into(),
        )
        .unwrap();
        p.surfaces
            .abis
            .insert(String::new(), Abi::open_path(&path, &compat, "").unwrap());
        p.surfaces.attempted.insert(String::new());
        p.surfaces.sync(&mut p.host, &p.compat, &p.assets);
        let text = unsafe {
            let ptr = p.surfaces.abis[""]
                .symbol::<unsafe extern "C" fn() -> *const std::ffi::c_char>(b"test_bound")(
            );
            std::ffi::CStr::from_ptr(ptr).to_str().unwrap().to_owned()
        };
        let expected = if call == "world()" {
            json!({})
        } else {
            json!({"restart":false,"seed":7,"paused":true})
        };
        assert_eq!(serde_json::from_str::<Value>(&text).unwrap(), expected);
        done(p, path);
    }
}

/// The platformer's diary, R8: a key typed at a world's canvas reaches the
/// canvas's `key` handler as on the web, and the world unless one prevents it.
#[test]
fn a_world_key_reaches_the_canvas_key_handler_first() {
    let (mut p, path) = boot(
        r#"component Keys
  state heard = ""
  action key(k: string)
    heard = heard + k
    if k == "x"
      preventDefault()
  view
    canvas testId="world" key=key width=100 height=100
      text heard testId="heard"
"#,
        &["world"],
    );
    let world = find(&p, "world");
    p.type_key(world, "KeyO", "o", true, false).unwrap();
    p.type_key(world, "KeyX", "x", true, false).unwrap();
    let held = &p.surfaces.canvases[&world].held;
    assert!(
        held.contains("KeyO"),
        "an unprevented key reaches the world"
    );
    assert!(!held.contains("KeyX"), "a prevented key goes no further");
    let text = find(&p, "heard");
    assert_eq!(
        p.host
            .kernel()
            .node(text)
            .unwrap()
            .props
            .str(exact_kernel::PropId::Text),
        Some("ox")
    );
    done(p, path);
}

/// b6 review B1: at a world's canvas an `aria-keyshortcuts` button takes
/// its key before the `key` handlers and the world, down and up, as the
/// web's capture listener and macOS's `routeKey` do.
#[test]
fn a_shortcut_button_takes_a_world_key_before_its_handlers() {
    let (mut p, path) = boot(
        r#"component Keys
  state heard = ""
  state paused = 0
  action key(k: string)
    heard = heard + k
  action pause
    paused = paused + 1
  view
    column
      button aria-keyshortcuts="Escape" press=pause testId="pause"
        text `${paused}` testId="paused"
      canvas testId="world" key=key width=100 height=100
        text heard testId="heard"
"#,
        &["world"],
    );
    let world = find(&p, "world");
    let text = |p: &super::Presenter<_>, id: &str| {
        p.host
            .kernel()
            .node(find(p, id))
            .unwrap()
            .props
            .str(exact_kernel::PropId::Text)
            .unwrap_or("")
            .to_string()
    };
    p.type_key(world, "KeyO", "o", true, false).unwrap();
    p.type_key(world, "Escape", "Escape", true, false).unwrap();
    assert!(!p.surfaces.canvases[&world].held.contains("Escape"));
    p.type_key(world, "Escape", "Escape", false, false).unwrap();
    assert_eq!(text(&p, "paused"), "1", "the button pressed");
    assert_eq!(text(&p, "heard"), "o", "no key handler heard Escape");
    assert!(p.surfaces.canvases[&world].held.contains("KeyO"));
    done(p, path);
}

/// A hardware keyup returns before `type_key`, which is what forgets a
/// shortcut's code. The down inserts it; once that button is gone, an agent
/// key of the same code delivers the down and swallows the up, so the world
/// keeps the key down.
#[test]
fn a_hardware_shortcut_keyup_does_not_stick_the_key_in_the_world() {
    let (mut p, path) = boot(
        r#"component Keys
  state gone = false
  state heard = ""
  action key(k: string)
    heard = heard + k
  action go
    gone = true
  view
    column
      when !gone
        button aria-keyshortcuts="Escape" press=go testId="go" height=24
          text "go"
      canvas testId="world" key=key width=100 height=100
        text heard testId="heard"
"#,
        &["world"],
    );
    let world = find(&p, "world");
    let text = |p: &Presenter<NoData>, id: &str| {
        p.host
            .kernel()
            .node(find(p, id))
            .unwrap()
            .props
            .str(exact_kernel::PropId::Text)
            .unwrap_or("")
            .to_string()
    };
    let shown = |p: &Presenter<NoData>, name: &str| {
        p.host.preorder().into_iter().any(|id| {
            p.host
                .kernel()
                .node(id)
                .unwrap()
                .props
                .str(exact_kernel::PropId::TestId)
                == Some(name)
        })
    };
    p.focus = Some(world);
    p.hardware_key("Escape", "Escape", true, false);
    assert!(!shown(&p, "go"), "the shortcut pressed its button away");
    assert_eq!(text(&p, "heard"), "", "the world did not hear the shortcut");
    assert!(
        !p.surfaces.canvases[&world].held.contains("Escape"),
        "a shortcut down does not reach the world"
    );
    p.hardware_key("Escape", "Escape", false, false);
    assert!(
        p.shortcut_keys.is_empty(),
        "the hardware keyup forgets the shortcut code"
    );
    p.type_key(world, "Escape", "Escape", true, false).unwrap();
    assert!(
        p.surfaces.canvases[&world].held.contains("Escape"),
        "with the button gone the key reaches the world"
    );
    p.type_key(world, "Escape", "Escape", false, false).unwrap();
    assert!(
        !p.surfaces.canvases[&world].held.contains("Escape"),
        "its up reaches the world too"
    );
    assert_eq!(text(&p, "heard"), "Escape");
    done(p, path);
}

#[test]
fn e10_contract_button_consumes_all_activation_keys() {
    for key in ["Space", "Enter", "NumpadEnter"] {
        let (mut p, path) = fixture();
        let button = find(&p, "remove");
        let child = find(&p, "a-jump");
        p.type_key(button, key, key, true, false).unwrap();
        p.type_key(button, key, key, false, false).unwrap();
        assert!(
            p.host.kernel().node(child).is_none(),
            "{key} activates the focused Contract button"
        );
        assert!(p.surfaces.canvases.values().all(|c| c.held.is_empty()));
        done(p, path);
    }
}

#[test]
fn r15_pointer_hud_button_releases_focus_but_keyboard_keeps_it() {
    let (mut p, path) = fixture();
    let button = find(&p, "hud-remove");
    p.tap(button).unwrap();
    assert_eq!(p.focus(), Some(find(&p, "raw")));
    p.hardware_key("Space", "Space", true, false);
    assert!(p
        .surfaces
        .canvases
        .values()
        .any(|c| c.held.contains("Space")));
    p.hardware_key("Space", "Space", false, false);
    // Tab from the stop before it (LLP 1088 D7.3: Tab moves the focus).
    p.type_key(find(&p, "b-jump"), "Tab", "Tab", true, false)
        .unwrap();
    assert_eq!(p.focus(), Some(button));
    p.hardware_key("Space", "Space", true, false);
    p.hardware_key("Space", "Space", false, false);
    assert_eq!(p.focus(), Some(button));
    assert!(p.surfaces.canvases.values().all(|c| c.held.is_empty()));
    done(p, path);
}

#[test]
fn r15_pointer_completion_preserves_a_replacement_buttons_autofocus() {
    let (path, _) = super::tests::fixture();
    let plan = contract::compile(
        r#"component Test
  state done = false
  action finish
    done = true
  view
    column
      when done
        button autofocus testId="next" width=100 height=30
      else
        button press=finish testId="start" width=100 height=30
"#,
    )
    .unwrap();
    let (mut p, _) = Presenter::boot(
        &plan.encode(),
        NoData,
        (100., 60.),
        1.,
        path.parent().unwrap().into(),
    )
    .unwrap();
    let start = find(&p, "start");
    p.tap(start).unwrap();
    assert_eq!(p.focus(), Some(find(&p, "next")));
    done(p, path);
}

#[test]
fn e11_pointer_control_keeps_focus_for_keyboard_input() {
    let (mut p, path) = fixture();
    let button = find(&p, "b-jump");
    p.tap(button).unwrap();
    assert_eq!(p.focus(), Some(button));
    p.type_key(button, "Space", "Space", true, false).unwrap();
    p.type_key(button, "Space", "Space", false, false).unwrap();
    assert_eq!(p.focus(), Some(button));
    done(p, path);
}

#[test]
fn e11_pointer_press_keeps_ordinary_focus_and_allows_new_autofocus() {
    for overlay in [false, true] {
        let (path, _) = super::tests::fixture();
        let plan = contract::compile(&format!(
            r#"component Test
  state done = false
  action finish
    done = true
  view
    column
      button press=finish testId="start" width=100 height=30
      when done
        button {} testId="next" width=100 height=30
"#,
            if overlay { "autofocus" } else { "" }
        ))
        .unwrap();
        let (mut p, _) = Presenter::boot(
            &plan.encode(),
            NoData,
            (100., 60.),
            1.,
            path.parent().unwrap().into(),
        )
        .unwrap();
        let start = find(&p, "start");
        p.tap(start).unwrap();
        assert_eq!(
            p.focus(),
            Some(if overlay { find(&p, "next") } else { start })
        );
        done(p, path);
    }
}

#[test]
fn hardware_release_follows_raw_owner_after_focus_moves() {
    let (mut p, path) = fixture();
    let raw = find(&p, "raw");
    p.focus = Some(raw);
    p.hardware_key("KeyW", "KeyW", true, false);
    assert!(p.surfaces.canvases[&raw].held.contains("KeyW"));
    p.focus = Some(find(&p, "editor"));
    p.hardware_key("KeyW", "KeyW", false, false);
    assert!(p.surfaces.canvases[&raw].held.is_empty());
    done(p, path);
}

#[test]
fn letter_release_does_not_release_enter_control() {
    let (mut p, path) = fixture();
    let button = find(&p, "a-jump");
    p.focus = Some(button);
    p.hardware_key("Enter", "Enter", true, false);
    assert_eq!(p.control_bindings.len(), 1);
    p.hardware_key("KeyW", "KeyW", false, false);
    assert_eq!(p.control_bindings.len(), 1);
    p.hardware_key("Enter", "Enter", false, false);
    assert!(p.control_bindings.is_empty());
    done(p, path);
}

#[test]
fn blur_clears_raw_hold_without_focus() {
    let (mut p, path) = fixture();
    let raw = find(&p, "raw");
    p.focus = Some(raw);
    p.hardware_key("KeyW", "KeyW", true, false);
    p.focus = None;
    p.blur();
    assert!(p.surfaces.canvases[&raw].held.is_empty());
    done(p, path);
}

fn last_input(p: &Presenter<NoData>) -> (u32, u32, Value) {
    let abi = &p.surfaces.abis[""];
    unsafe {
        let text = std::ffi::CStr::from_ptr(abi
            .symbol::<unsafe extern "C" fn() -> *const std::ffi::c_char>(b"test_input")(
        ))
        .to_str()
        .unwrap();
        (
            abi.symbol::<unsafe extern "C" fn() -> u32>(b"test_input_id")(),
            abi.symbol::<unsafe extern "C" fn() -> u32>(b"test_input_count")(),
            serde_json::from_str(text).unwrap(),
        )
    }
}

#[test]
fn hardware_keys_reach_module_with_code_character_repeat_and_clock() {
    let (mut p, path) = fixture();
    let raw = find(&p, "raw");
    p.focus = Some(raw);
    p.advance(25.).unwrap_or_default();
    for (down, repeat) in [(true, false), (true, true), (false, false)] {
        p.hardware_key("KeyW", "W", down, repeat);
        let (id, _, event) = last_input(&p);
        assert_eq!(id, p.surfaces.canvases[&raw].id);
        assert_eq!(
            event,
            json!({"t":"key","code":"KeyW","key":"W","down":down,"repeat":repeat,"at":25.})
        );
    }
    assert!(p.surfaces.canvases[&raw].held.is_empty());
    p.hardware_key("Escape", "Escape", true, false);
    assert_eq!(last_input(&p).2["code"], "Escape");
    assert!(p.surfaces.canvases[&raw].held.contains("Escape"));
    p.hardware_key("Escape", "Escape", false, false);
    done(p, path);
}

#[test]
fn hardware_typing_stays_in_editor_and_repeats_but_button_repeat_does_not_activate() {
    let (mut p, path) = fixture();
    let editor = find(&p, "editor");
    p.focus = Some(editor);
    for (code, key, down, repeat) in [
        ("KeyW", "W", true, false),
        ("KeyW", "W", true, true),
        ("KeyW", "w", false, false),
        ("Backspace", "Backspace", true, false),
        ("Space", " ", true, false),
        ("Space", " ", false, false),
    ] {
        p.hardware_key(code, key, down, repeat);
    }
    assert_eq!(
        p.host
            .kernel()
            .node(editor)
            .unwrap()
            .props
            .str(exact_kernel::PropId::Value),
        Some("W ")
    );
    assert!(p.surfaces.canvases.values().all(|c| c.held.is_empty()));
    p.hardware_key("Escape", "Escape", true, false);
    assert_eq!(p.focus(), None);
    let button = find(&p, "remove");
    let child = find(&p, "a-jump");
    p.focus = Some(button);
    p.hardware_key("Enter", "Enter", true, true);
    assert!(p.host.kernel().node(child).is_some());
    p.hardware_key("Enter", "Enter", true, false);
    assert!(p.host.kernel().node(child).is_none());
    done(p, path);
}

#[test]
fn rejected_key_does_not_gain_ownership_and_duplicate_release_is_not_forwarded() {
    let (mut p, path) = fixture();
    let raw = find(&p, "raw");
    assert!(p.type_key(raw, "RejectKey", "x", true, false).is_err());
    assert!(p.surfaces.canvases[&raw].held.is_empty());
    p.focus = Some(raw);
    p.hardware_key("KeyW", "w", true, false);
    p.hardware_key("KeyW", "w", false, false);
    let count = last_input(&p).1;
    p.hardware_key("KeyW", "w", false, false);
    assert_eq!(last_input(&p).1, count);
    p.hardware_key("Tab", "Tab", true, false);
    assert_eq!(last_input(&p).1, count);
    done(p, path);
}

#[cfg(target_os = "linux")]
#[test]
fn evdev_wasd_edges_reach_canvas_and_release_after_focus_moves() {
    use crate::input::{InputEvent, Keyboard};
    let (mut p, path) = fixture();
    let raw = find(&p, "raw");
    let editor = find(&p, "editor");
    let mut keyboard = Keyboard::default();
    for physical in [17, 30, 31, 32] {
        p.focus = Some(raw);
        for value in [1, 2, 0] {
            if value == 0 {
                p.focus = Some(editor);
            }
            let InputEvent::Key {
                code,
                shift,
                down,
                repeat,
            } = keyboard.event(physical, value, None).unwrap()
            else {
                panic!("key event");
            };
            let (code, key) = crate::input::key(code, shift).unwrap();
            p.hardware_key(code, key, down, repeat);
            assert_eq!(last_input(&p).2["down"], down);
            assert_eq!(last_input(&p).2["code"], code);
        }
        assert!(p.surfaces.canvases[&raw].held.is_empty());
    }
    done(p, path);
}

fn removing_hud_button_returns_input_to_its_canvas(keyboard: bool) {
    let (mut p, path) = fixture_with_hud_removal(true);
    let button = find(&p, "hud-remove");
    let canvas = find(&p, "raw");
    if keyboard {
        p.type_key(button, "Enter", "Enter", true, false).unwrap();
        p.hardware_key("Enter", "Enter", false, false);
    } else {
        p.tap(button).unwrap();
    }
    assert!(p.host.kernel().node(button).is_none());
    assert_eq!(p.focus(), Some(canvas));
    p.hardware_key("KeyD", "d", true, false);
    assert!(p.surfaces.canvases[&canvas].held.contains("KeyD"));
    assert!(p
        .surfaces
        .canvases
        .iter()
        .all(|(id, c)| *id == canvas || c.held.is_empty()));
    p.hardware_key("KeyD", "d", false, false);
    assert!(p.surfaces.canvases.values().all(|c| c.held.is_empty()));
    done(p, path);
}

#[test]
fn removed_hud_pointer_press_returns_focus_to_its_canvas() {
    removing_hud_button_returns_input_to_its_canvas(false);
}

#[test]
fn removed_hud_keyboard_press_returns_focus_to_its_canvas() {
    removing_hud_button_returns_input_to_its_canvas(true);
}

#[test]
fn a_declared_module_is_routed_by_surface_and_verified_by_its_own_card() {
    // LLP 1009 D6: every surface a module does not list is the primary's, and
    // each artifact is admitted only by its own signed digest.
    use sha2::{Digest, Sha256};
    let dir = std::env::temp_dir().join(format!("d6-gpu-modules-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (night, world) = (dir.join("libnight.dylib"), dir.join("libworld.dylib"));
    std::fs::write(&night, b"night").unwrap();
    std::fs::write(&world, b"world").unwrap();
    let card = |bytes: &[u8]| json!({"app":"app","cohort":"cohort","trust":"production","sha256":format!("{:x}", Sha256::digest(bytes))});
    let compat = json!({"id":"cohort","inputs":{"app":"app","gpuModules":{"world":["world","arena"]}},
        "embedded":{"gpu":card(b"night"),"gpuModules":{"world":card(b"world")}}});
    assert_eq!(artifact_of(&compat, "arena"), "world");
    assert_eq!(artifact_of(&compat, "night"), "");
    assert_eq!(
        artifact_of(&json!({}), "world"),
        "",
        "one artifact owns every surface"
    );
    verify_module(&night, &compat, "").unwrap();
    verify_module(&world, &compat, "world").unwrap();
    assert!(verify_module(&world, &compat, "")
        .unwrap_err()
        .contains("digest mismatch"));
    assert!(verify_module(&night, &compat, "world")
        .unwrap_err()
        .contains("digest mismatch"));
    assert!(verify_module(&world, &compat, "other")
        .unwrap_err()
        .contains("missing baked identity"));
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A held contact on a canvas is the canvas's own (rivals diary, limit 7): the
/// presenter used to retire it at its first move (`contact:false`) and the
/// world saw neither the down nor any move, so mouse look could not turn.
#[test]
fn held_canvas_contact_streams_down_moves_and_up_to_the_canvas() {
    let (mut p, path) = fixture();
    let raw = find(&p, "raw");
    let (ox, oy, _, _) = p.rect_of(raw).unwrap();
    let ask = |p: &mut Presenter<NoData>, q: Value| -> Value {
        serde_json::from_str(&crate::agent::contact::answer(p, &q)).unwrap()
    };
    let before = unsafe {
        p.surfaces.abis[""].symbol::<unsafe extern "C" fn() -> u32>(b"test_input_count")()
    };
    let down = ask(
        &mut p,
        json!({"op":"tap","id":raw,"phase":"down","x":ox + 50.,"y":oy + 70.}),
    );
    assert_eq!(down["contact"], true, "{down}");
    let (id, count, event) = last_input(&p);
    assert_eq!(id, p.surfaces.canvases[&raw].id);
    assert_eq!(count, before + 1);
    assert_eq!(
        (event["phase"].as_str(), event["buttons"].as_u64()),
        (Some("down"), Some(1))
    );
    assert_eq!(
        (event["x"].as_f64(), event["y"].as_f64()),
        (Some(50.), Some(70.))
    );
    // 100 points over 100 ms: seven samples, every one a move the world sees.
    let moved = ask(
        &mut p,
        json!({"op":"tap","phase":"move","dx":100,"dy":0,"ms":100}),
    );
    assert_eq!(moved["contact"], true, "{moved}");
    assert_eq!(moved["delivery"], "presenter");
    let (_, count, event) = last_input(&p);
    assert_eq!(count, before + 8);
    assert_eq!(
        (event["phase"].as_str(), event["buttons"].as_u64()),
        (Some("move"), Some(1))
    );
    assert_eq!(
        event["x"].as_f64(),
        Some(150.),
        "the canvas keeps the contact off its edge"
    );
    let up = ask(&mut p, json!({"op":"tap","phase":"up"}));
    assert_eq!(up["contact"], false, "{up}");
    let (_, count, event) = last_input(&p);
    assert_eq!(count, before + 9, "the up adds no zero-length move");
    assert_eq!(
        (event["phase"].as_str(), event["buttons"].as_u64()),
        (Some("up"), Some(0))
    );
    assert_eq!(event["x"].as_f64(), Some(150.));
    // A click without movement is still one down and one up, and Escape (a
    // cancel) ends a held contact with the canvas's `cancel`.
    assert!(p.pointer_down(ox + 20., oy + 80., 200.).unwrap());
    p.pointer_up(ox + 20., oy + 80., 210.).unwrap();
    let (_, count, event) = last_input(&p);
    assert_eq!((count, event["phase"].as_str()), (before + 11, Some("up")));
    assert!(p.pointer_down(ox + 20., oy + 80., 220.).unwrap());
    p.pointer_cancel(230.).unwrap();
    let (_, count, event) = last_input(&p);
    assert_eq!(
        (count, event["phase"].as_str()),
        (before + 13, Some("cancel"))
    );
    assert!(p.contact_position().is_none());
    done(p, path);
}

/// A canvas sees the pointer's motion beside its position (the device's own,
/// from evdev, past the screen's edge), its hover, and the secondary and
/// middle buttons as the web's chorded buttons (rivals diary, limit 2).
#[test]
fn canvas_pointer_carries_motion_hover_and_the_other_buttons() {
    let (mut p, path) = fixture();
    let raw = find(&p, "raw");
    let (ox, oy, _, _) = p.rect_of(raw).unwrap();
    let phase = |p: &Presenter<NoData>| {
        let (_, _, e) = last_input(p);
        (
            e["phase"].as_str().unwrap().to_string(),
            e["buttons"].as_u64().unwrap(),
            e["dx"].as_f64().unwrap(),
            e["dy"].as_f64().unwrap(),
        )
    };
    p.pointer_move(ox + 10., oy + 60., 0.).unwrap();
    assert_eq!(phase(&p), ("move".into(), 0, 0., 0.), "a hover");
    p.pointer_move(ox + 14., oy + 57., 1.).unwrap();
    assert_eq!(phase(&p), ("move".into(), 0, 4., -3.));
    // At the screen's edge the position stops; the device's motion does not.
    p.raw_motion(25., 0.);
    p.pointer_move(ox + 14., oy + 57., 2.).unwrap();
    assert_eq!(phase(&p), ("move".into(), 0, 25., 0.));
    // A held contact pinned at the edge still turns, and motion never lingers.
    assert!(p.pointer_down(ox + 14., oy + 57., 2.5).unwrap());
    p.raw_motion(30., 0.);
    p.pointer_move(ox + 14., oy + 57., 2.6).unwrap();
    assert_eq!(phase(&p), ("move".into(), 1, 30., 0.));
    p.pointer_up(ox + 14., oy + 57., 2.7).unwrap();
    p.raw_motion(9., 9.);
    p.pointer_move(ox + 500., oy + 57., 2.8).unwrap();
    p.pointer_move(ox + 14., oy + 57., 2.9).unwrap();
    assert_eq!(
        phase(&p),
        ("move".into(), 0, 0., 0.),
        "off the canvas, then back"
    );
    p.pointer_aux(2, true, ox + 14., oy + 57., 3.);
    assert_eq!(
        phase(&p),
        ("down".into(), 2, 0., 0.),
        "right alone is a down"
    );
    assert!(p.pointer_down(ox + 14., oy + 57., 4.).unwrap());
    assert_eq!(phase(&p), ("move".into(), 3, 0., 0.), "a chord is a move");
    p.pointer_aux(4, true, ox + 14., oy + 57., 5.);
    assert_eq!(phase(&p), ("move".into(), 7, 0., 0.));
    p.pointer_aux(2, false, ox + 14., oy + 57., 6.);
    p.pointer_aux(4, false, ox + 14., oy + 57., 7.);
    assert_eq!(phase(&p), ("move".into(), 1, 0., 0.));
    p.pointer_up(ox + 14., oy + 57., 8.).unwrap();
    assert_eq!(phase(&p), ("up".into(), 0, 0., 0.), "the last button up");
    done(p, path);
}

/// A lost pointer (Escape, a dropped evdev report) holds no button, and a
/// secondary press on the canvas is released to it wherever the pointer is
/// (pointer capture). Before: a stale secondary bit turned the next primary
/// down into a move and the cancel went out with buttons 2.
#[test]
fn a_lost_pointer_holds_no_button_and_the_canvas_hears_its_release() {
    let (mut p, path) = fixture();
    let raw = find(&p, "raw");
    let (ox, oy, _, _) = p.rect_of(raw).unwrap();
    let last = |p: &Presenter<NoData>| {
        let (_, count, e) = last_input(p);
        (
            count,
            e["phase"].as_str().unwrap().to_string(),
            e["buttons"].as_u64().unwrap(),
        )
    };
    p.pointer_aux(2, true, ox + 20., oy + 60., 1.);
    assert!(p.pointer_down(ox + 20., oy + 60., 2.).unwrap());
    p.pointer_lost(3.).unwrap();
    let (_, phase, buttons) = last(&p);
    assert_eq!((phase.as_str(), buttons), ("cancel", 0));
    assert!(p.pointer_down(ox + 20., oy + 60., 4.).unwrap());
    let (_, phase, buttons) = last(&p);
    assert_eq!((phase.as_str(), buttons), ("down", 1), "no stale secondary");
    p.pointer_up(ox + 20., oy + 60., 5.).unwrap();
    // Pressed on the canvas, released off it: the canvas hears the up.
    p.pointer_aux(2, true, ox + 20., oy + 60., 6.);
    p.pointer_move(ox + 500., oy + 60., 7.).unwrap();
    let (count, _, _) = last(&p);
    p.pointer_aux(2, false, ox + 500., oy + 60., 8.);
    assert_eq!(last(&p), (count + 1, "up".into(), 0));
    // Lost with only the secondary held: a cancel, nothing held after.
    p.pointer_aux(4, true, ox + 20., oy + 60., 9.);
    p.pointer_lost(10.).unwrap();
    assert_eq!(last(&p).1, "cancel");
    p.pointer_aux(4, true, ox + 20., oy + 60., 11.);
    assert_eq!(last(&p).1, "down", "the middle button begins again");
    done(p, path);
}

/// The agent's taps and contacts reach a canvas as a finger on every host (the
/// web's CDP touch, iOS's touches); its hover is a mouse with nothing held.
/// Before: a Linux agent tap was a mouse with the primary button, so the world
/// held MouseLeft there and not on the web or iOS.
#[test]
fn the_agents_canvas_taps_are_a_finger_and_its_hover_a_mouse() {
    let (mut p, path) = fixture();
    let raw = find(&p, "raw");
    let (ox, oy, _, _) = p.rect_of(raw).unwrap();
    let kind = |p: &Presenter<NoData>| {
        let (_, _, e) = last_input(p);
        (
            e["phase"].as_str().unwrap().to_string(),
            e["kind"].as_str().unwrap().to_string(),
        )
    };
    crate::agent::answer(
        &mut p,
        &format!(
            r#"{{"op":"tap","id":{raw},"phase":"down","x":{},"y":{}}}"#,
            ox + 50.,
            oy + 70.
        ),
    );
    assert_eq!(kind(&p), ("down".into(), "touch".into()));
    crate::agent::answer(&mut p, r#"{"op":"tap","phase":"up"}"#);
    assert_eq!(kind(&p), ("up".into(), "touch".into()));
    p.pointer_move(ox + 40., oy + 60., 1.).unwrap();
    assert_eq!(
        kind(&p),
        ("move".into(), "mouse".into()),
        "a device's hover"
    );
    done(p, path);
}

#[test]
fn agent_contextmenu_delivers_a_secondary_mouse_click_and_refuses_controls() {
    let (mut p, path) = fixture();
    let raw = find(&p, "raw");
    let reply: Value = serde_json::from_str(&crate::agent::answer(
        &mut p,
        &format!(r#"{{"op":"tap","id":{raw},"contextmenu":true}}"#),
    ))
    .unwrap();
    assert_eq!(reply["delivery"], "presenter", "{reply}");
    let (canvas, count, up) = last_input(&p);
    let down: Value = unsafe {
        let abi = &p.surfaces.abis[""];
        serde_json::from_str(
            std::ffi::CStr::from_ptr(abi
                .symbol::<unsafe extern "C" fn() -> *const std::ffi::c_char>(
                    b"test_previous_input",
                )())
            .to_str()
            .unwrap(),
        )
        .unwrap()
    };
    assert_eq!(canvas, 3);
    assert_eq!(count, 3, "move, right down, right up");
    for (event, phase, buttons) in [(&down, "down", 2), (&up, "up", 0)] {
        assert_eq!(event["id"], 1);
        assert_eq!(event["kind"], "mouse");
        assert_eq!(event["phase"], phase);
        assert_eq!(event["buttons"], buttons);
        assert_eq!(
            (event["x"].as_f64(), event["y"].as_f64()),
            (Some(50.), Some(50.))
        );
    }
    assert!(p.contact_position().is_none());
    for name in ["a-jump", "editor"] {
        let id = find(&p, name);
        let refusal: Value = serde_json::from_str(&crate::agent::answer(
            &mut p,
            &format!(r#"{{"op":"tap","id":{id},"contextmenu":true}}"#),
        ))
        .unwrap();
        assert!(refusal["error"].as_str().unwrap().contains("contextmenu"));
        assert_eq!(last_input(&p).1, count, "refused without canvas input");
    }
    let (x, y, w, h) = p.rect_of(raw).unwrap();
    assert!(p.pointer_down(x + w / 2., y + h / 2., 1.).unwrap());
    let count = last_input(&p).1;
    assert!(p
        .contextmenu(raw, None)
        .unwrap_err()
        .contains("held contact"));
    assert_eq!(last_input(&p).1, count);
    p.pointer_lost(2.).unwrap();
    done(p, path);
}

#[test]
fn agent_contextmenu_uses_the_requested_point_and_refuses_invalid_points_without_input() {
    let (mut p, path) = fixture();
    let raw = find(&p, "raw");
    let (x, y, _, _) = p.rect_of(raw).unwrap();
    let reply: Value = serde_json::from_str(&crate::agent::answer(
        &mut p,
        &format!(r#"{{"op":"tap","id":{raw},"contextmenu":true,"at":[25,75]}}"#),
    ))
    .unwrap();
    assert_eq!(
        (reply["at"][0].as_f64(), reply["at"][1].as_f64()),
        (Some(f64::from(x + 25.)), Some(f64::from(y + 75.))),
        "{reply}"
    );
    let (_, count, up) = last_input(&p);
    assert_eq!(count, 3);
    assert_eq!((up["x"].as_f64(), up["y"].as_f64()), (Some(25.), Some(75.)));
    assert_eq!(up["buttons"], 0);
    // The first point is covered by the HUD button; others are outside the
    // target or not an exact finite pair. Refusal must not leak a mouse event.
    for at in [
        "[25,15]",
        "[-1,75]",
        "[100,75]",
        "[25,100]",
        "null",
        "[]",
        "[25]",
        "[25,75,0]",
        r#"["25",75]"#,
        "[1e100,75]",
    ] {
        let reply: Value = serde_json::from_str(&crate::agent::answer(
            &mut p,
            &format!(r#"{{"op":"tap","id":{raw},"contextmenu":true,"at":{at}}}"#),
        ))
        .unwrap();
        assert!(reply.get("error").is_some(), "accepted {at}: {reply}");
        assert_eq!(last_input(&p).1, count, "{at} delivered input");
    }
    assert!(p.contextmenu(raw, Some((f32::NAN, 75.))).is_err());
    assert!(p.contextmenu(raw, Some((25., f32::INFINITY))).is_err());
    p.resize(100., y + 60.);
    assert!(p
        .contextmenu(raw, Some((25., 75.)))
        .unwrap_err()
        .contains("viewport"));
    assert_eq!(last_input(&p).1, count);
    done(p, path);
}

#[test]
fn agent_primary_mouse_after_touch_preserves_device_identity_and_refusal_atomicity() {
    let (mut p, path) = fixture();
    let raw = find(&p, "raw");
    crate::agent::answer(&mut p, &format!(r#"{{"op":"tap","id":{raw}}}"#));
    assert_eq!(last_input(&p).2["kind"], "touch");
    for (mode, buttons) in [("mouse", 1), ("contextmenu", 2)] {
        let reply: Value = serde_json::from_str(&crate::agent::answer(
            &mut p,
            &format!(r#"{{"op":"tap","id":{raw},"{mode}":true,"at":[25,75]}}"#),
        ))
        .unwrap();
        assert_eq!(reply["delivery"], "presenter", "{reply}");
        let (_, _, up) = last_input(&p);
        let down: Value = unsafe {
            let abi = &p.surfaces.abis[""];
            serde_json::from_str(
                std::ffi::CStr::from_ptr(abi
                    .symbol::<unsafe extern "C" fn() -> *const std::ffi::c_char>(
                        b"test_previous_input",
                    )())
                .to_str()
                .unwrap(),
            )
            .unwrap()
        };
        for (event, phase, buttons) in [(&down, "down", buttons), (&up, "up", 0)] {
            assert_eq!(event["id"], 1);
            assert_eq!(event["kind"], "mouse");
            assert_eq!(event["phase"], phase);
            assert_eq!(event["buttons"], buttons);
            assert_eq!(
                (event["x"].as_f64(), event["y"].as_f64()),
                (Some(25.), Some(75.))
            );
        }
        assert!(p.contact_position().is_none());
    }
    let count = last_input(&p).1;
    for extra in [
        r#""at":[25,15]"#,
        r#""at":[-1,75]"#,
        r#""at":[100,75]"#,
        r#""at":null"#,
        r#""at":[25]"#,
        r#""at":[1e100,75]"#,
        // A phase with `mouse` is a held contact (9d75806c8, review A1),
        // which takes x/y, never a click's `at`.
        r#""phase":"down","at":[25,75]"#,
        r#""contextmenu":true"#,
        r#""resize":[200,200]"#,
    ] {
        let reply: Value = serde_json::from_str(&crate::agent::answer(
            &mut p,
            &format!(r#"{{"op":"tap","id":{raw},"mouse":true,{extra}}}"#),
        ))
        .unwrap();
        assert!(reply.get("error").is_some(), "{reply}");
        assert_eq!(last_input(&p).1, count);
    }
    let control = find(&p, "a-jump");
    assert!(p.mouse_click(control, None, false).is_err());
    let (x, y, _, _) = p.rect_of(raw).unwrap();
    p.pointer_down(x + 25., y + 75., 0.).unwrap();
    let count = last_input(&p).1;
    assert!(p
        .mouse_click(raw, Some((25., 75.)), false)
        .unwrap_err()
        .contains("held contact"));
    assert_eq!(last_input(&p).1, count);
    p.pointer_lost(1.).unwrap();
    done(p, path);
}
