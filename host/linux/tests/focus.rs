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
      if !editing
        button "Edit" press=edit testId="edit" height=32
      if editing
        if !replaced
          input autofocus id="a" testId="a" key=replace height=32
        if replaced
          input autofocus id="b" testId="b" height=32
      text log testId="log" height=20
"#
        .replace(
            "state editing = false",
            &format!("state editing = {initial}"),
        );
        let source = if keep_button {
            source.replace("if !editing", "if true")
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
        if !replaced
          input autofocus id="a" testId="a" key=replace height=32
        if replaced
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

#[test]
fn blur_removes_wrapper_before_focusout() {
    let source = r#"component App
  state shown = true
  state log = ""
  action remove
    log = `${log}blur;`
    shown = false
  action left(label: string, e: FocusEvent)
    log = `${log}${label}:${e.target}:${match e.relatedTarget { case some(id) => id, case none => "-" }};`
  action entered(e: FocusEvent)
    log = `${log}in:${e.target}:${match e.relatedTarget { case some(id) => id, case none => "-" }};`
  view
    column focusout=left("outer") width=200
      if shown
        column focusout=left("wrapper") testId="wrapper"
          input id="a" testId="a" blur=remove height=32
      input id="b" testId="b" focusin=entered height=32
      text log testId="log" height=20
"#;
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
    p.tap(id(&p, "a")).unwrap();
    p.tap(id(&p, "b")).unwrap();
    assert_eq!(log(&p), "blur;wrapper:a:b;in:b:a;");
    assert!(p.host().kernel().find_by_test_id("wrapper").is_empty());
}

#[test]
fn change_removal_does_not_dispatch_a_detached_blur_binding() {
    let source = r#"component App
  state shown = true
  action remove(value: string)
    shown = false
  action left
  view
    column width=200
      if shown
        column focusout=left
          input id="a" testId="a" change=remove height=32
      input id="b" testId="b" height=32
"#;
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
    p.type_text(id(&p, "a"), "edited").unwrap();
    p.tap(id(&p, "b")).unwrap();
    assert_eq!(p.focus(), Some(id(&p, "b")));
    let journal = p.host().runner().journal().collect::<Vec<_>>().join("\n");
    assert!(!journal.contains("focusout"), "{journal}");
}

#[test]
fn detached_focusout_does_not_visit_an_ancestor_removed_later() {
    let source = r#"component App
  state page = true
  state shown = true
  state log = ""
  action remove
    shown = false
  action closePage(label: string)
    log = `${log}${label};`
    page = false
  view
    column width=200
      if page
        column focusout=closePage("outer")
          if shown
            column focusout=closePage("wrapper")
              input id="a" testId="a" blur=remove height=32
      input id="b" testId="b" height=32
      text log testId="log" height=20
"#;
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
    p.tap(id(&p, "a")).unwrap();
    p.tap(id(&p, "b")).unwrap();
    assert_eq!(log(&p), "wrapper;");
    let journal = p.host().runner().journal().collect::<Vec<_>>().join("\n");
    assert!(!journal.contains("UnknownView"), "{journal}");
}

#[test]
fn detached_focusout_survives_autofocus_during_blur_commit() {
    let source = r#"component App
  state shown = true
  state log = ""
  action remove
    shown = false
  action leave
    blur()
  action left
    log = `${log}wrapper;`
  action entered
    log = `${log}focus;`
  view
    column width=200
      if shown
        column focusout=left
          input id="a" testId="a" blur=remove submit=leave height=32
      if !shown
        input autofocus id="c" testId="c" focus=entered height=32
      text log testId="log" height=20
"#;
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
    p.tap(id(&p, "a")).unwrap();
    p.type_key(id(&p, "a"), "Enter", "Enter", true, false)
        .unwrap();
    p.run_commands(|| ());
    assert_eq!(log(&p), "focus;wrapper;");
    assert_eq!(p.focus(), Some(id(&p, "c")));
}
