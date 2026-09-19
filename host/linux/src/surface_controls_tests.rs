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
    let (path, compat) = super::tests::fixture();
    let plan = contract::compile(
        r#"component Controls
  state removed = false
  action remove writes removed
    removed = true
  view
    column
      input testId="editor" width=100 height=30
      canvas testId="a" width=100 height=100
        when !removed
          button testId="a-jump" action="jump" width=100 height=100
            box testId="label" width=100 height=100
      canvas testId="b" width=100 height=100
        button testId="b-jump" action="jump" width=100 height=100
      canvas testId="raw" width=100 height=100
      button testId="remove" press=remove width=100 height=30
"#,
    )
    .unwrap();
    let (mut p, _) = Presenter::boot(
        &plan.encode(),
        NoData,
        (100., 360.),
        1.,
        path.parent().unwrap().into(),
    )
    .unwrap();
    p.surfaces.abi = Some(Abi::open_path(&path, &compat).unwrap());
    for (i, name) in ["a", "b", "raw"].iter().enumerate() {
        let view = find(&p, name);
        p.surfaces.canvases.insert(
            view,
            Canvas {
                id: i as u32 + 1,
                name: (*name).into(),
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
fn restored_lookup_stays_inside_surface_and_missing_control_stays_held() {
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
    assert_eq!(p.control_bindings.len(), 1);
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
    let abi = p.surfaces.abi.as_ref().unwrap();
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
    let abi = p.surfaces.abi.as_ref().unwrap();
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
