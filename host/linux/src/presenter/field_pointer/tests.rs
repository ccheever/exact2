use super::*;

fn fixture(value: &str, extra: &str) -> (Presenter<()>, ViewId) {
    let source = format!(
        r#"component App
  state value = {}
  state heard = ""
  action edit(v: string)
    value = v
  action selected(e: InputEvent)
    heard = `${{e.selectionStart}}-${{e.selectionEnd}}`
  action block
    preventDefault()
  view
    column
      textarea testId="editor" value=value input=edit width=350 height=140 padding=8 font-size=16 line-height="24px" appearance="none" {extra}
      button "Other" testId="other"
      text heard testId="heard"
"#,
        serde_json::to_string(value).unwrap()
    );
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(&source),
        (),
        (400., 240.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.frame();
    let k = p.host.kernel();
    let id = k.node_by_key(k.find_by_test_id("editor")[0]).unwrap().id;
    (p, id)
}

fn point(p: &mut Presenter<()>, id: ViewId, at: u32, share: f32) -> (f32, f32) {
    p.frame();
    let field = &p.brush.field_text[&id];
    let r = field.range_frame(at, at + 1).unwrap();
    (r.0 + r.2 * share, r.1 + r.3 * 0.5)
}

fn selected(p: &Presenter<()>, id: ViewId) -> String {
    let value = p.field_text(id);
    let mark = p.field_selection(id);
    value[byte_at(&value, mark.start)..byte_at(&value, mark.end)].into()
}

fn click(p: &mut Presenter<()>, point: (f32, f32), at: f64, count: u32) {
    p.pointer_down_with_clicks(point.0, point.1, at, count)
        .unwrap();
    p.pointer_up(point.0, point.1, at + 1.).unwrap();
}

#[test]
fn double_click_uses_the_character_under_both_halves_of_a_word_edge() {
    let (mut p, id) = fixture("one two three", "");
    for share in [0.1, 0.9] {
        let at = point(&mut p, id, 6, share);
        click(&mut p, at, 10., 2);
        assert_eq!(selected(&p, id), "two");
    }
    p.field_key(id, "X", 20.);
    assert_eq!(p.field_text(id), "one X three");
    p.hold_modifier("ControlLeft", true);
    p.field_key(id, "z", 21.);
    p.hold_modifier("ControlLeft", false);
    assert_eq!(p.field_text(id), "one two three");
}

#[test]
fn character_drag_and_shift_click_keep_the_anchor_and_reverse_direction() {
    let (mut p, id) = fixture("one two three", "");
    let start = point(&mut p, id, 4, 0.1);
    let end = point(&mut p, id, 6, 0.9);
    p.pointer_down_with_clicks(start.0, start.1, 1., 1).unwrap();
    p.pointer_move(end.0, end.1, 2.).unwrap();
    p.pointer_up(end.0, end.1, 3.).unwrap();
    assert_eq!(selected(&p, id), "two");
    p.hold_modifier("ShiftLeft", true);
    let before = point(&mut p, id, 0, 0.1);
    click(&mut p, before, 4., 1);
    p.hold_modifier("ShiftLeft", false);
    assert_eq!(selected(&p, id), "one ");
    assert_eq!(
        p.field_selection(id).direction,
        SelectionDirection::Backward
    );
}

#[test]
fn word_drag_extends_whole_words_in_both_directions_and_stops_on_up() {
    let (mut p, id) = fixture("one two three", "");
    let start = point(&mut p, id, 5, 0.5);
    let last = point(&mut p, id, 9, 0.5);
    let first = point(&mut p, id, 1, 0.5);
    p.pointer_down_with_clicks(start.0, start.1, 1., 2).unwrap();
    p.pointer_move(last.0, last.1, 2.).unwrap();
    assert_eq!(selected(&p, id), "two three");
    p.pointer_move(first.0, first.1, 3.).unwrap();
    assert_eq!(selected(&p, id), "one two");
    assert_eq!(
        p.field_selection(id).direction,
        SelectionDirection::Backward
    );
    p.pointer_up(first.0, first.1, 4.).unwrap();
    p.pointer_move(last.0, last.1, 5.).unwrap();
    assert_eq!(selected(&p, id), "one two");
}

#[test]
fn mouse_selection_delivers_select_and_respects_disabled_canceled_and_touch_input() {
    let (mut p, id) = fixture("one two three", "select=selected");
    let at = point(&mut p, id, 5, 0.5);
    click(&mut p, at, 1., 2);
    p.run_commands(<()>::default);
    let k = p.host.kernel();
    assert_eq!(
        k.node_by_key(k.find_by_test_id("heard")[0])
            .unwrap()
            .props
            .str(PropId::Text),
        Some("4-7")
    );
    for extra in ["disabled=true", "pointerdown=block", ""] {
        let (mut p, id) = fixture("one two three", extra);
        let at = point(&mut p, id, 5, 0.5);
        if extra.is_empty() {
            p.set_pointer_kind(PointerKind::Touch);
        }
        p.pointer_down_with_clicks(at.0, at.1, 1., 2).unwrap();
        assert!(p.field_drag.is_none(), "{extra}");
        assert!(selected(&p, id).is_empty(), "{extra}");
        if extra == "pointerdown=block" {
            assert_eq!(p.focus, None);
            p.pointer_cancel(2.).unwrap();
            p.set_focus(Some(id), 3.);
            p.mark_field(id, "one two three", FieldSelection::caret(1), false);
            click(&mut p, at, 4., 2);
            assert_eq!(p.field_selection(id), FieldSelection::caret(1));
        }
    }
}

#[test]
fn character_click_and_drag_keep_the_upstream_caret_at_a_soft_wrap() {
    let (mut p, id) = fixture(&"one two three four five six seven ".repeat(3), "");
    let end = p
        .paragraph(id)
        .unwrap()
        .layout_runs()
        .next()
        .unwrap()
        .glyphs
        .last()
        .unwrap()
        .end;
    let at = point(&mut p, id, end - 1, 1.2);
    click(&mut p, at, 1., 1);
    assert_eq!(p.field_cursor(id).offset, end);
    assert!(p.field_cursor(id).upstream);
    let start = point(&mut p, id, 0, 0.1);
    p.pointer_down_with_clicks(start.0, start.1, 3., 1).unwrap();
    p.pointer_move(at.0, at.1, 4.).unwrap();
    assert_eq!(p.field_cursor(id).offset, end);
    assert!(p.field_cursor(id).upstream);
    p.pointer_move(start.0, start.1, 5.).unwrap();
    assert!(p.field_drag.as_ref().unwrap().moved(start));
}

#[test]
fn triple_click_selects_the_paragraph_and_readonly_text_remains_selectable() {
    let (mut p, id) = fixture("one two\nthree four", "readonly=true");
    let at = point(&mut p, id, 5, 0.5);
    click(&mut p, at, 1., 3);
    assert_eq!(selected(&p, id), "one two\n");
    p.field_key(id, "X", 3.);
    assert_eq!(p.field_text(id), "one two\nthree four");
}

#[test]
fn selection_drag_retires_on_cancel_value_change_or_focus_loss() {
    let (mut p, id) = fixture("one two three", "");
    let at = point(&mut p, id, 5, 0.5);
    let next = point(&mut p, id, 9, 0.5);
    p.pointer_down_with_clicks(at.0, at.1, 1., 2).unwrap();
    p.pointer_cancel(2.).unwrap();
    p.pointer_move(next.0, next.1, 3.).unwrap();
    assert_eq!(selected(&p, id), "two");
    p.pointer_down_with_clicks(at.0, at.1, 4., 2).unwrap();
    p.field_key(id, "X", 5.);
    p.pointer_move(next.0, next.1, 6.).unwrap();
    assert!(p.field_drag.is_none());
    assert_eq!(p.field_text(id), "one X three");
    p.pointer_cancel(7.).unwrap();
    p.pointer_down_with_clicks(at.0, at.1, 8., 2).unwrap();
    p.set_focus(None, 9.);
    p.pointer_move(next.0, next.1, 10.).unwrap();
    assert!(p.field_drag.is_none());
}

#[test]
fn word_and_paragraph_units_preserve_unicode_and_password_privacy() {
    assert_eq!(range("🦀 two", 0, Unit::Word), (0, 2));
    let value = "café 🦀 two\r\nnext";
    assert_eq!(range(value, 2, Unit::Word), (0, 4));
    assert_eq!(range(value, 5, Unit::Word), (5, 7));
    assert_eq!(range(value, 9, Unit::Word), (8, 11));
    assert_eq!(range(value, 9, Unit::Paragraph), (0, 13));
    assert_eq!(range(value, 2, Unit::Password), (0, utf16_len(value)));
}
