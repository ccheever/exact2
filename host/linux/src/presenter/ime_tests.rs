use super::*;

fn id(p: &Presenter<()>, name: &str) -> ViewId {
    let k = p.host.kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}

fn boot(source: &str) -> (Presenter<()>, ViewId) {
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (420., 240.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    let editor = id(&p, "editor");
    p.set_focus(Some(editor), 0.);
    p.frame();
    (p, editor)
}

fn fixture(attrs: &str) -> (Presenter<()>, ViewId) {
    boot(&format!("component App\n  view\n    column\n      textarea testId=\"editor\" value=\"a🦀z\" width=220 height=64 padding=4 appearance=\"none\" outline=\"none\" font-size=16 line-height=\"24px\" {attrs}\n      input testId=\"other\" value=\"other\"\n"))
}

fn replace_crab(p: &mut Presenter<()>, editor: ViewId) -> Target {
    p.mark_field(
        editor,
        "a🦀z",
        FieldSelection {
            start: 1,
            end: 3,
            direction: SelectionDirection::Forward,
        },
        false,
    );
    let target = p.ime_target().unwrap();
    assert!(p.ime_begin(target));
    target
}

#[test]
fn unconverted_ranges_are_relative_clamped_and_replaced_with_each_preedit() {
    let (mut p, editor) = fixture("");
    replace_crab(&mut p, editor);
    p.ime_preedit("に🦀abc", None);
    p.ime_unconverted(&[(3, 20), (0, 1), (u32::MAX, u32::MAX)]);
    assert_eq!(p.ime_paint().unwrap().unconverted, [(1, 2), (4, 7)]);
    p.ime_unconverted(&[(2, 4), (0, 2), (3, 6)]);
    assert_eq!(p.ime_paint().unwrap().unconverted, [(1, 7)]);
    p.ime_preedit("日本", None);
    assert!(p.ime_paint().unwrap().unconverted.is_empty());
    p.ime_unconverted(&[(0, 2)]);
    p.ime_preedit("", None);
    assert!(p.ime_paint().unwrap().unconverted.is_empty());
    p.ime_end();
    p.ime_unconverted(&[(0, 2)]);
    assert!(p.ime_paint().is_none());
}

#[test]
fn composition_replaces_selection_and_commits_once_with_one_undo() {
    let (mut p, editor) = fixture("");
    replace_crab(&mut p, editor);
    p.ime_preedit("に", Some((3, 3)));
    assert_eq!(p.field_text(editor), "aにz");
    assert_eq!(p.field_selection(editor), FieldSelection::caret(2));
    p.ime_preedit("日本🦀", Some((6, 10)));
    assert_eq!(p.field_text(editor), "a日本🦀z");
    let selection = p.field_selection(editor);
    assert_eq!((selection.start, selection.end), (3, 5));
    p.frame();
    assert!(
        p.ime_cursor_area().is_some(),
        "selected clauses still anchor candidates"
    );
    assert!(p.caret_frame().is_none());
    p.ime_preedit("", None);
    assert_eq!(p.field_text(editor), "a🦀z");
    p.ime_commit("日本語");
    p.ime_end();
    assert_eq!(p.field_text(editor), "a日本語z");
    assert_eq!(p.field_selection(editor), FieldSelection::caret(4));
    p.history_action(editor, false, 1.);
    assert_eq!(p.field_text(editor), "a🦀z");
    assert_eq!(
        (
            p.field_selection(editor).start,
            p.field_selection(editor).end
        ),
        (1, 3)
    );
    assert!(!p
        .field_history
        .available(editor, &p.field_text(editor), false));
    p.history_action(editor, true, 2.);
    assert_eq!(p.field_text(editor), "a日本語z");
}

#[test]
fn cancellation_restores_text_selection_and_prior_undo() {
    let (mut p, editor) = fixture("");
    p.type_text(editor, "typed").unwrap();
    let before = p.field_selection(editor);
    assert!(p.ime_begin(p.ime_target().unwrap()));
    p.ime_preedit("unfinished", None);
    p.frame();
    assert!(p.caret_frame().is_none(), "the platform can hide the caret");
    p.ime_preedit("", None);
    p.ime_end();
    assert_eq!(p.field_text(editor), "typed");
    assert_eq!(p.field_selection(editor), before);
    p.history_action(editor, false, 1.);
    assert_eq!(p.field_text(editor), "a🦀z");
}

#[test]
fn input_handlers_hear_composition_but_their_rewrites_wait_until_commit() {
    let (mut p, editor) = boot(
        r#"component App
  state value = ""
  state heard = ""
  action edited(v: string, e: InputEvent)
    value = v.toLowerCase()
    heard = e.inputType
  action before(e: InputEvent)
    preventDefault()
  view
    column
      textarea testId="editor" value=value input=edited beforeinput=before width=200 height=64
      text heard testId="heard"
"#,
    );
    assert!(p.ime_begin(p.ime_target().unwrap()));
    p.ime_preedit("AB", Some((2, 2)));
    assert_eq!(p.bound_text(editor), "ab");
    assert_eq!(p.field_text(editor), "AB");
    p.frame();
    assert_eq!(p.brush.composition.as_ref().unwrap().value, "AB");
    let heard = id(&p, "heard");
    assert_eq!(
        p.host.kernel().node(heard).unwrap().props.str(PropId::Text),
        Some("insertCompositionText")
    );
    p.ime_commit("ABC");
    assert_eq!(p.field_text(editor), "abc");
    assert_eq!(
        p.host.kernel().node(heard).unwrap().props.str(PropId::Text),
        Some("insertFromComposition")
    );
    assert!(p.ime.is_none());
}

#[test]
fn ime_requires_an_editable_target_and_commits_with_field_limits() {
    for attrs in [
        "readonly=true",
        "disabled=true",
        "type=\"password\"",
        "inputmode=\"none\"",
    ] {
        let (p, _) = fixture(attrs);
        assert!(p.ime_target().is_none(), "{attrs}");
    }
    let (mut p, editor) = fixture("maxlength=4");
    replace_crab(&mut p, editor);
    p.ime_preedit("にほんご", Some((12, 12)));
    assert_eq!(
        p.field_text(editor),
        "aにほんごz",
        "preedit is not truncated"
    );
    p.ime_commit("日🦀本");
    assert_eq!(
        p.field_text(editor),
        "a日z",
        "UTF-16 limits cannot split a scalar"
    );
    let (mut p, editor) = boot("component App\n  view\n    input testId=\"editor\" value=\"\"\n");
    p.ime_begin(p.ime_target().unwrap());
    p.ime_commit("A\r\nB\rC");
    assert_eq!(p.field_text(editor), "ABC");
}

#[test]
fn final_result_after_blur_belongs_to_the_original_field_and_stale_targets_fail() {
    let (mut p, editor) = fixture("");
    let target = replace_crab(&mut p, editor);
    p.ime_preedit("に", Some((3, 3)));
    let other = id(&p, "other");
    p.set_focus(Some(other), 1.);
    assert!(
        p.ime_begin(target),
        "a late result can finish its original owner"
    );
    p.ime_commit("日");
    assert_eq!(p.field_text(editor), "a日z");
    assert_eq!(p.field_text(other), "other");
    assert_eq!(p.focus, Some(other));
    assert!(!p.ime_begin(target));
    let current = p.ime_target().unwrap();
    assert!(p.ime_begin(current));
    assert!(
        !p.ime_begin(target),
        "a stale stream cannot take over another composition"
    );
    p.ime_end();
    p.ime_forget(other);
    assert!(
        !p.ime_begin(current),
        "a renewed field rejects the old stream"
    );
    let renewed = p.ime_target().unwrap();
    p.replaced();
    assert!(
        !p.ime_begin(renewed),
        "runtime replacement rejects the old stream"
    );
}

#[test]
fn final_result_after_window_deactivation_preserves_text_and_undo() {
    let (mut p, editor) = fixture("");
    let target = replace_crab(&mut p, editor);
    p.ime_preedit("に", Some((3, 3)));
    p.set_window_active(false);
    assert!(p.ime_target().is_none());
    assert!(p.ime_begin(target));
    p.ime_preedit("", None);
    p.ime_commit("に");
    p.ime_end();
    assert_eq!(p.field_text(editor), "aにz");
    p.set_window_active(true);
    assert_eq!(p.focus, Some(editor));
    assert_eq!(p.field_selection(editor), FieldSelection::caret(2));
    p.history_action(editor, false, 1.);
    assert_eq!(p.field_text(editor), "a🦀z");
}

#[test]
fn preedit_cursor_offsets_are_clamped_at_utf8_boundaries_and_underline_is_removed() {
    let (mut p, editor) = fixture("");
    replace_crab(&mut p, editor);
    p.ime_preedit("日🦀", Some((1, usize::MAX)));
    assert_eq!(
        (
            p.field_selection(editor).start,
            p.field_selection(editor).end
        ),
        (1, 4)
    );
    p.ime_preedit("日🦀", Some((7, 7)));
    p.set_caret_visible(false);
    let composing = p.frame().data().to_vec();
    p.ime_commit("日🦀");
    let committed = p.frame().data().to_vec();
    assert_ne!(
        composing, committed,
        "composition ink must be visible and disappear on commit"
    );
    assert_eq!(p.field_text(editor), "a日🦀z");
}
