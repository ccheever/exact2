use super::*;
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
    let (path, compat) = super::tests::fixture();
    let source = r#"component Controls
  state removed = false
  state text = ""
  action change(value: string) writes text
    text = value
  action remove writes removed
    removed = true
  view
    column
      input testId="editor" value=text change=change width=100 height=30
      canvas testId="a" width=100 height=100
        when !removed
          button testId="a-jump" action="jump" width=100 height=100
            box testId="label" width=100 height=100
      canvas testId="b" width=100 height=100
        button testId="b-jump" action="jump" width=100 height=100
      canvas testId="raw" width=100 height=100
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
    let plan = contract::compile(&source).unwrap();
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
    for (i, name) in ["a", "b", "raw"].iter().enumerate() {
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
    p.type_key(button, "Tab", "Tab", true, false).unwrap();
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
  action finish writes done
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
  action finish writes done
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
