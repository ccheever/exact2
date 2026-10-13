// Focus transitions through the public host, including a commit that replaces its focused node.
use exact_kernel::{PropId, ViewId};
use exact_linux::presenter::{PainterChoice, Presenter};
use std::path::PathBuf;
fn id(p: &Presenter<()>, name: &str) -> ViewId {
    let k = p.host().kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}
fn log(p: &Presenter<()>) -> String {
    let k = p.host().kernel();
    k.node(id(p, "log"))
        .unwrap()
        .props
        .str(PropId::Text)
        .unwrap()
        .to_string()
}
#[test]
fn autofocus_bubbles_on_mount_and_after_focused_removal() {
    for (initial, keep_button) in [(false, false), (false, true), (true, false)] {
        let source = r#"component App
  state editing = false
  state replaced = false
  state log = ""
  action entered(e: FocusEvent)
    log = `${log}in:${e.target};`
  action left(e: FocusEvent)
    log = `${log}out:${e.target};`
  action edit
    editing = true
  action replace(k: string)
    if k == "Enter"
      replaced = true
  view
    column focusin=entered focusout=left width=200
      when !editing
        button "Edit" press=edit testId="edit" height=32
      when editing
        when !replaced
          input autofocus id="a" testId="a" key=replace height=32
        when replaced
          input autofocus id="b" testId="b" height=32
      text log testId="log" height=20
"#
        .replace(
            "state editing = false",
            &format!("state editing = {initial}"),
        );
        let source = if keep_button {
            source.replace("when !editing", "when true")
        } else {
            source
        };
        let (mut p, error) = Presenter::boot_with(
            &contract::compile(&source).unwrap().encode(),
            (),
            (400., 400.),
            1.,
            PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(error.is_none(), "{error:?}");
        if !initial {
            p.tap(id(&p, "edit")).unwrap();
        }
        let a = id(&p, "a");
        assert_eq!(p.focus(), Some(a));
        let prefix = if initial { "" } else { "in:;out:;" };
        assert_eq!(log(&p), format!("{prefix}in:a;"));
        p.type_key(a, "Enter", "Enter", true, false).unwrap();
        assert_eq!(p.focus(), Some(id(&p, "b")));
        assert_eq!(log(&p), format!("{prefix}in:a;out:a;in:b;"));
    }
}

#[test]
fn focus_handlers_stop_before_the_outer_wrapper() {
    let source = include_str!("../../../contract/corpus/focus-bubble.contract");
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(source).unwrap().encode(),
        (),
        (400., 400.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    for target in ["stopA", "stopB", "after"] {
        p.tap(id(&p, target)).unwrap();
    }
    assert_eq!(
        log(&p),
        "stopped:stopA;stopped:stopA;stopped:stopB;stopped:stopB;"
    );
}

#[test]
fn autofocus_keeps_each_bubbling_events_stop_flag() {
    let source = r#"component App
  state replaced = false
  state log = ""
  action entered(e: FocusEvent)
    log = `${log}in:${e.target};`
  action left(e: FocusEvent)
    stopPropagation()
    log = `${log}out:${e.target};`
  action outerIn(e: FocusEvent)
    log = `${log}outer-in:${e.target};`
  action outerOut(e: FocusEvent)
    log = `${log}outer-out:${e.target};`
  action replace(k: string)
    if k == "Enter"
      replaced = true
  view
    column focusin=outerIn focusout=outerOut width=200
      column focusin=entered focusout=left
        when !replaced
          input autofocus id="a" testId="a" key=replace height=32
        when replaced
          input autofocus id="b" testId="b" height=32
      text log testId="log" height=20
"#;
    for stopped in [false, true] {
        let source = if stopped {
            source.to_owned()
        } else {
            source.replace("    stopPropagation()\n", "")
        };
        let (mut p, error) = Presenter::boot_with(
            &contract::compile(&source).unwrap().encode(),
            (),
            (400., 400.),
            1.,
            PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(error.is_none(), "{error:?}");
        let a = id(&p, "a");
        p.type_key(a, "Enter", "Enter", true, false).unwrap();
        let outer = if stopped { "" } else { "outer-out:a;" };
        assert_eq!(
            log(&p),
            format!("in:a;outer-in:a;out:a;{outer}in:b;outer-in:b;")
        );
    }
}

#[test]
fn press_blur_leaves_no_focus_for_pointer_and_keyboard() {
    let source = r#"component App
  state log = ""
  action entered(e: FocusEvent)
    log = `${log}in:${e.target};`
  action left(e: FocusEvent)
    log = `${log}out:${e.target};`
  action leave
    blur()
  view
    column focusin=entered focusout=left width=200
      button "Leave" id="b" testId="b" press=leave height=32
      text log testId="log" height=20
"#;
    for pointer in [true, false] {
        let (mut p, error) = Presenter::boot_with(
            &contract::compile(source).unwrap().encode(),
            (),
            (400., 400.),
            1.,
            PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(error.is_none(), "{error:?}");
        let b = id(&p, "b");
        if pointer {
            p.tap(b).unwrap();
        } else {
            p.type_key(b, "Enter", "Enter", true, false).unwrap();
        }
        p.run_commands(|| ());
        assert_eq!(log(&p), "in:b;out:b;");
        assert_eq!(
            p.focus(),
            None,
            "pointer={pointer}: explicit blur survives the press"
        );
    }
}
