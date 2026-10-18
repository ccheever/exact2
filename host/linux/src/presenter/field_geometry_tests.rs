//! Field input and pixels against the paragraph actually painted, with fixture fonts.
use super::*;
use exact_runner::{FieldSelection, SelectionDirection};

fn boot(tag: &str, value: &str, extra: &str) -> (Presenter<()>, ViewId) {
    let source = format!(
        r##"component Fields
  view
    column width=400 height=600
      {tag} id="field" testId="field" value="{value}" appearance="none" width=180 height=120 padding=12 font-size=16 line-height="24px" color="#000000" caret-color="#ff0000" {extra}
"##
    );
    boot_source(&source)
}

fn boot_source(source: &str) -> (Presenter<()>, ViewId) {
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (400., 600.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    *p.text.borrow_mut() = TextEngine::from_font_files(
        &[std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/fixtures/fonts/assets"
        ))],
        "DejaVu Sans",
    );
    let kernel = p
        .host
        .kernel()
        .rehydrate(Box::new(Measurer(p.text.clone())));
    *p.host.runner_mut().kernel_mut() = kernel;
    p.resize(400., 600.);
    p.replaced();
    p.frame();
    let k = p.host.kernel();
    let id = k.node_by_key(k.find_by_test_id("field")[0]).unwrap().id;
    (p, id)
}

fn origin(p: &mut Presenter<()>, id: ViewId, multiline: bool, share: f32) -> (f32, f32) {
    let (x, y, _, _) = p.rect_of(id).unwrap();
    let paragraph = p.paragraph(id).unwrap();
    (
        x + 12.
            + if multiline {
                0.
            } else {
                share * (180. - paragraph.width)
            },
        y + 12.
            + if multiline {
                0.
            } else {
                (120. - paragraph.height) / 2.
            },
    )
}

fn key(p: &mut Presenter<()>, id: ViewId, name: &str) {
    p.type_key(id, name, name, true, false).unwrap();
    p.type_key(id, name, name, false, false).unwrap();
}

fn contact_field(tag: &str, feedback: &str, down: &str) -> (Presenter<()>, ViewId) {
    boot_source(&format!(
        r#"component Fields
  state seen = ""
  state shown = true
  state disabled = false
  state fieldDisplay = "block"
  state parentDisplay = "flex"
  state fieldVisibility = "visible"
  state parentVisibility = "visible"
  action record(phase: string)
    seen = `${{seen}}${{phase}}`
  action down
    seen = `${{seen}}down `
    {down}
  action pan(dx: number, dy: number)
    seen = `${{seen}}pan `
  view
    column width=400 height=600
      column display=parentDisplay visibility=parentVisibility
        if shown
          {tag} id="field" testId="field" value="abcd" appearance="none" width=180 height=120 padding=12 font-size=16 line-height="24px" display=fieldDisplay disabled=disabled pointerdown=down focus=record("focus ") press=record("press ") {feedback}
      box testId="pan" pan=pan press=record("pan-press ") touch-action="none" width=180 height=40
      input id="other" testId="other" value="other" width=180 height=40
      text seen testId="log"
"#
    ))
}

fn contact_log(p: &Presenter<()>) -> &str {
    let k = p.host.kernel();
    k.node_by_key(k.find_by_test_id("log")[0])
        .unwrap()
        .props
        .str(PropId::Text)
        .unwrap()
}

fn second_character(p: &mut Presenter<()>, id: ViewId, multiline: bool) -> (f32, f32) {
    let (ox, oy) = origin(p, id, multiline, 0.);
    let glyph = p
        .paragraph(id)
        .unwrap()
        .layout_runs()
        .next()
        .unwrap()
        .glyphs[1]
        .clone();
    (ox + glyph.x + glyph.w * 0.1, oy + 12.)
}

#[test]
fn mouse_field_down_focuses_and_places_caret_without_repeating_on_release() {
    for tag in ["input", "textarea"] {
        for feedback in ["", "-exact-press-scale=0.9"] {
            let (mut p, id) = contact_field(tag, feedback, "shown = shown");
            let (x, y) = second_character(&mut p, id, tag == "textarea");
            p.pointer_down(x, y, 1.).unwrap();
            assert_eq!(p.focus, Some(id), "{tag} {feedback}: down focuses");
            assert_eq!(p.field_selection(id), FieldSelection::caret(1));
            assert_eq!(contact_log(&p), "down focus ");

            key(&mut p, id, "End");
            assert_eq!(p.field_selection(id), FieldSelection::caret(4));
            p.pointer_up(x, y, 2.).unwrap();
            assert_eq!(p.field_selection(id), FieldSelection::caret(4));
            assert_eq!(contact_log(&p), "down focus press ");

            p.pointer_down(x, y, 3.).unwrap();
            let k = p.host.kernel();
            let other = k.node_by_key(k.find_by_test_id("other")[0]).unwrap().id;
            p.set_focus(Some(other), 3.);
            p.pointer_up(x, y, 4.).unwrap();
            assert_eq!(p.focus, Some(other), "release does not take focus back");
        }
    }
}

#[test]
fn cancelled_mouse_field_contact_keeps_focus_and_never_presses() {
    for end in ["cancel", "lost", "pan"] {
        for feedback in ["", "-exact-press-scale=0.9"] {
            let (mut p, id) = contact_field(
                "input",
                &format!("{feedback} pan=pan touch-action=\"none\""),
                "shown = shown",
            );
            let (x, y) = second_character(&mut p, id, false);
            p.pointer_down(x, y, 1.).unwrap();
            assert_eq!(p.focus, Some(id));
            let release_x = match end {
                "pan" => {
                    p.pointer_move(x + 40., y, 2.).unwrap();
                    x + 40.
                }
                "lost" => {
                    p.pointer_lost(2.).unwrap();
                    x
                }
                _ => {
                    p.pointer_cancel(2.).unwrap();
                    x
                }
            };
            p.pointer_up(release_x, y, 3.).unwrap();
            assert_eq!(p.focus, Some(id));
            assert_eq!(p.field_selection(id), FieldSelection::caret(1));
            assert_eq!(
                contact_log(&p),
                if end == "pan" {
                    "down focus pan "
                } else {
                    "down focus "
                }
            );
        }
    }
}

#[test]
fn field_down_revalidates_after_authored_pointerdown() {
    for (down, visibility, feedback_focus, feedback_log) in [
        ("shown = false", "", Some("other"), "down "),
        ("disabled = true", "", Some("other"), "down "),
        (
            "fieldDisplay = \"none\"",
            "",
            Some("field"),
            "down focus press ",
        ),
        (
            "parentDisplay = \"none\"",
            "",
            Some("field"),
            "down focus press ",
        ),
        (
            "fieldVisibility = \"hidden\"",
            "visibility=fieldVisibility",
            None,
            "down press ",
        ),
        ("parentVisibility = \"hidden\"", "", None, "down press "),
    ] {
        for feedback in ["", "-exact-press-scale=0.9"] {
            let (mut p, id) = contact_field("input", &format!("{feedback} {visibility}"), down);
            let k = p.host.kernel();
            let other = k.node_by_key(k.find_by_test_id("other")[0]).unwrap().id;
            p.set_focus(Some(other), 0.);
            let (x, y) = second_character(&mut p, id, false);
            p.pointer_down(x, y, 1.).unwrap();
            assert_eq!(p.focus, Some(other), "{down} {feedback}: down");
            assert_eq!(contact_log(&p), "down ", "{down} {feedback}: down");
            p.pointer_up(x, y, 2.).unwrap();
            // Press feedback retains its release target even when hit-testing hides it.
            let (focus, log) = if feedback.is_empty() {
                (Some(other), "down ")
            } else {
                let k = p.host.kernel();
                let focus = feedback_focus
                    .map(|name| k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id);
                (focus, feedback_log)
            };
            assert_eq!(p.focus, focus, "{down} {feedback}: up");
            assert_eq!(contact_log(&p), log, "{down} {feedback}: up");
        }
    }
}

#[test]
fn context_preview_field_prevents_mouse_focus_and_caret() {
    for feedback in ["", "-exact-press-scale=0.9"] {
        let (mut p, id) = boot_source(&format!(
            r#"component Fields
  state seen = ""
  action record(phase: string)
    seen = `${{seen}}${{phase}}`
  view
    column width=400 height=600
      input id="editor" testId="editor" value="other" width=180 height=40
      box position="absolute" top=100 width=300 height=220
        box contextTarget="editor" width=300 height=40
        button press=record("press ") {feedback}
          input id="field" testId="field" value="abcd" appearance="none" width=180 height=120 padding=12 font-size=16 line-height="24px" focus=record("focus ")
      text seen testId="log"
"#
        ));
        let k = p.host.kernel();
        let editor = k.node_by_key(k.find_by_test_id("editor")[0]).unwrap().id;
        p.set_focus(Some(editor), 0.);
        p.mark_field(editor, "other", FieldSelection::caret(2), false);
        p.mark_field(id, "abcd", FieldSelection::caret(3), false);
        let (x, y) = second_character(&mut p, id, false);
        assert_eq!(p.hit(x, y), Some(id));
        assert!(p.pointerdown_prevents_focus(id));
        p.pointer_down(x, y, 1.).unwrap();
        assert_eq!(p.focus, Some(editor));
        assert_eq!(p.field_selection(editor), FieldSelection::caret(2));
        assert_eq!(p.field_selection(id), FieldSelection::caret(3));
        assert_eq!(contact_log(&p), "");
        p.pointer_up(x, y, 2.).unwrap();
        assert_eq!(p.focus, Some(editor));
        assert_eq!(p.field_selection(editor), FieldSelection::caret(2));
        assert_eq!(p.field_selection(id), FieldSelection::caret(3));
        assert_eq!(contact_log(&p), "press ");
    }
}

#[test]
fn prevented_mouse_focus_keeps_the_field_caret() {
    let (mut p, id) = contact_field("input", "", "shown = shown");
    p.set_focus(Some(id), 0.);
    p.mark_field(id, "abcd", FieldSelection::caret(2), false);
    let k = p.host.kernel();
    let pan = k.node_by_key(k.find_by_test_id("pan")[0]).unwrap().id;
    let (x, y, _, _) = p.rect_of(pan).unwrap();
    assert!(p.pointerdown_prevents_focus(pan));
    p.pointer_down(x + 10., y + 10., 1.).unwrap();
    assert_eq!(p.focus, Some(id));
    assert_eq!(p.field_selection(id), FieldSelection::caret(2));
    p.pointer_up(x + 10., y + 10., 2.).unwrap();
    assert_eq!(p.focus, Some(id));
    assert_eq!(p.field_selection(id), FieldSelection::caret(2));
    assert_eq!(contact_log(&p), "focus pan-press ");
}

#[test]
fn direct_field_contacts_focus_only_on_tap_and_never_on_pan() {
    for simulated in [false, true] {
        for feedback in ["", "-exact-press-scale=0.9"] {
            let (mut p, id) = contact_field("input", feedback, "shown = shown");
            if simulated {
                p.agent_finger(true);
            } else {
                p.set_pointer_kind(PointerKind::Touch);
            }
            let (x, y) = second_character(&mut p, id, false);
            let before = p.field_selection(id);
            p.pointer_down(x, y, 1.).unwrap();
            assert_eq!(p.focus, None);
            assert_eq!(p.field_selection(id), before);
            assert_eq!(contact_log(&p), "down ");
            p.pointer_up(x, y, 2.).unwrap();
            assert_eq!(p.focus, Some(id));
            assert_eq!(p.field_selection(id), FieldSelection::caret(1));
            assert_eq!(contact_log(&p), "down focus press ");

            let (mut p, id) = contact_field(
                "input",
                &format!("{feedback} pan=pan touch-action=\"none\""),
                "shown = shown",
            );
            if simulated {
                p.agent_finger(true);
            } else {
                p.set_pointer_kind(PointerKind::Touch);
            }
            let (x, y) = second_character(&mut p, id, false);
            let before = p.field_selection(id);
            p.pointer_down(x, y, 1.).unwrap();
            p.pointer_move(x + 40., y, 2.).unwrap();
            p.pointer_up(x + 40., y, 3.).unwrap();
            assert_eq!(p.focus, None);
            assert_eq!(p.field_selection(id), before);
            assert_eq!(contact_log(&p), "down pan ");
        }
    }
}

#[test]
fn field_click_uses_alignment_padding_password_and_empty_value() {
    let mut actual = Vec::new();
    let mut expected = Vec::new();
    for (value, extra, share, masked) in [
        ("abcd", "text-align=\"left\"", 0., false),
        ("abcd", "text-align=\"center\"", 0.5, false),
        ("abcd", "text-align=\"right\"", 1., false),
        (
            "a😀bc",
            "type=\"password\" text-align=\"center\"",
            0.5,
            true,
        ),
        (
            "",
            "placeholder=\"some hint\" text-align=\"right\"",
            1.,
            false,
        ),
    ] {
        let (mut p, id) = boot("input", value, extra);
        let (ox, oy) = origin(&mut p, id, false, share);
        let glyphs = p
            .paragraph(id)
            .unwrap()
            .layout_runs()
            .next()
            .unwrap()
            .glyphs;
        let xs: Vec<_> = glyphs.iter().take(3).map(|g| g.x + g.w * 0.1).collect();
        for (index, x) in xs.into_iter().enumerate() {
            p.press_at(ox + x, oy + 12., 0.);
            actual.push(p.field_selection(id).start);
            expected.push(if value.is_empty() {
                0
            } else if masked && index >= 2 {
                index as u32 + 1
            } else {
                index as u32
            });
        }
    }
    assert_eq!(actual, expected);
}

#[test]
fn field_click_follows_soft_wraps_and_hard_breaks() {
    let (mut p, id) = boot("textarea", "abcd efgh ijkl mnop qrst uvwx\\nlast", "");
    let (ox, oy) = origin(&mut p, id, true, 0.);
    let mut points = Vec::new();
    let mut hard_start = 0;
    let mut hard = 0;
    let mut previous = "";
    for line in p.paragraph(id).unwrap().layout_runs() {
        if line.line_i != hard {
            hard_start += previous.len() as u32 + 1;
            hard = line.line_i;
        }
        previous = line.text;
        let first = line.glyphs.first().unwrap();
        let end = line.glyphs.iter().map(|g| g.end).max().unwrap();
        points.push((
            ox + first.x + first.w * 0.1,
            oy + line.line_top + 12.,
            hard_start + first.start,
        ));
        points.push((ox + 179., oy + line.line_top + 12., hard_start + end));
    }
    assert!(points.len() >= 6, "fixture wraps and has a hard break");
    let mut actual = Vec::new();
    for &(x, y, _) in &points {
        p.press_at(x, y, 0.);
        actual.push(p.field_selection(id).start);
    }
    assert_eq!(actual, points.iter().map(|p| p.2).collect::<Vec<_>>());
}

#[test]
fn field_caret_and_highlight_use_every_visual_line() {
    let (mut p, id) = boot("textarea", "abcd efgh ijkl mnop qrst uvwx\\nlast", "");
    let (ox, oy) = origin(&mut p, id, true, 0.);
    let lines: Vec<_> = p
        .paragraph(id)
        .unwrap()
        .layout_runs()
        .map(|l| {
            (
                l.glyphs.first().unwrap().start,
                l.glyphs.first().unwrap().x,
                l.line_top,
            )
        })
        .collect();
    p.set_focus(Some(id), 0.);
    let value = p.field_text(id);
    p.mark_field(id, &value, FieldSelection::caret(lines[1].0), false);
    let pixel = p
        .frame()
        .pixel(
            (ox + lines[1].1).ceil() as u32,
            (oy + lines[1].2 + 2.) as u32,
        )
        .unwrap();
    assert!(
        pixel.red() > 200 && pixel.green() < 50,
        "caret starts the wrapped line: {pixel:?}"
    );
    key(&mut p, id, "ArrowUp");
    key(&mut p, id, "End");
    assert_eq!(p.field_selection(id), FieldSelection::caret(lines[1].0));
    let first = p.paragraph(id).unwrap().layout_runs().next().unwrap();
    let last = first.glyphs.last().unwrap();
    let end_x = ox + last.x + last.w;
    let pixels = p.frame();
    let coverage: u32 = (end_x.floor() as u32..=end_x.ceil() as u32)
        .map(|x| 255 - u32::from(pixels.pixel(x, (oy + 2.) as u32).unwrap().green()))
        .sum();
    assert!(
        coverage >= 250,
        "End's one-pixel caret stays on its visual line: {coverage}"
    );
    p.mark_field(
        id,
        &value,
        FieldSelection {
            start: 0,
            end: value.len() as u32,
            direction: SelectionDirection::Forward,
        },
        false,
    );
    let pixels = p.frame();
    for (_, x, y) in lines {
        let pixel = pixels
            .pixel((ox + x + 2.) as u32, (oy + y + 2.) as u32)
            .unwrap();
        assert_eq!(
            [pixel.red(), pixel.green(), pixel.blue()],
            [0xb4, 0xd5, 0xfe],
            "highlight at line {y}"
        );
    }
}

#[test]
fn field_vertical_keys_keep_x_and_home_uses_the_visual_line() {
    let (mut p, id) = boot("textarea", "abcdefgh ijklmnop qrstuvwx\\nx\\nabcdefgh", "");
    p.set_focus(Some(id), 0.);
    let value = p.field_text(id);
    p.mark_field(id, &value, FieldSelection::caret(3), false);
    let starts: Vec<_> = p
        .paragraph(id)
        .unwrap()
        .layout_runs()
        .filter(|l| l.line_i == 0)
        .map(|l| l.glyphs.first().unwrap().start)
        .collect();
    assert_eq!(starts.len(), 2);
    key(&mut p, id, "ArrowDown");
    assert!(p.field_selection(id).start > starts[1]);
    key(&mut p, id, "Home");
    assert_eq!(p.field_selection(id), FieldSelection::caret(starts[1]));
    p.mark_field(id, &value, FieldSelection::caret(3), false);
    key(&mut p, id, "ArrowDown");
    key(&mut p, id, "ArrowDown");
    assert_eq!(p.field_selection(id).start, 28, "short hard line clamps x");
    key(&mut p, id, "ArrowDown");
    assert_eq!(p.field_selection(id).start, 32, "next long line restores x");
    p.hold_modifier("ShiftLeft", true);
    key(&mut p, id, "ArrowDown");
    assert_eq!(p.field_selection(id).end, value.len() as u32);
    key(&mut p, id, "ArrowUp");
    assert_eq!(
        p.field_selection(id),
        FieldSelection {
            start: 28,
            end: 32,
            direction: SelectionDirection::Backward
        }
    );
    p.hold_modifier("ShiftLeft", false);
    key(&mut p, id, "ArrowUp");
    key(&mut p, id, "ArrowUp");
    assert_eq!(p.field_selection(id), FieldSelection::caret(3));
    key(&mut p, id, "ArrowUp");
    assert_eq!(p.field_selection(id), FieldSelection::caret(0));
}

#[test]
fn field_control_arrows_move_by_word_and_shift_extends() {
    let (mut p, id) = boot("input", "one two  three", "");
    p.set_focus(Some(id), 0.);
    let value = p.field_text(id);
    p.mark_field(id, &value, FieldSelection::caret(0), false);
    p.hold_modifier("ControlLeft", true);
    key(&mut p, id, "ArrowRight");
    assert_eq!(p.field_selection(id), FieldSelection::caret(3));
    key(&mut p, id, "ArrowRight");
    assert_eq!(p.field_selection(id), FieldSelection::caret(7));
    p.hold_modifier("ShiftLeft", true);
    key(&mut p, id, "ArrowLeft");
    assert_eq!(
        p.field_selection(id),
        FieldSelection {
            start: 4,
            end: 7,
            direction: SelectionDirection::Backward
        }
    );
    p.hold_modifier("ShiftLeft", false);
    p.hold_modifier("ControlLeft", false);
}

#[test]
fn field_contact_inverts_transform_and_container_scroll() {
    let (mut p, id) = boot_source(
        r#"component Fields
  state presses = 0
  action pressed
    presses = presses + 1
  view
    scroll testId="port" width=400 height=150
      column
        box height=100
        input id="field" testId="field" value="abcdef" appearance="none" width=180 height=60 padding=12 font-size=16 line-height="24px" translate="50px 0px" scale=1.25 press=pressed
        box height=400
"#,
    );
    let k = p.host.kernel();
    let port = k.node_by_key(k.find_by_test_id("port")[0]).unwrap().id;
    assert_eq!(p.scroll_to(port, (0., 80.)), Some((0., 80.)));
    p.frame();
    let (x, y, _, _) = p.rect_of(id).unwrap();
    let glyph = p
        .paragraph(id)
        .unwrap()
        .layout_runs()
        .next()
        .unwrap()
        .glyphs[2];
    let point = (
        x + 1.25 * (12. + glyph.x + glyph.w * 0.1),
        y + 1.25 * (12. + 30.),
    );
    assert_eq!(p.hit(point.0, point.1), Some(id));
    p.pointer_down(point.0, point.1, 0.).unwrap();
    p.pointer_up(point.0, point.1, 1.).unwrap();
    assert_eq!(p.focus(), Some(id));
    assert_eq!(p.field_selection(id), FieldSelection::caret(2));
    key(&mut p, id, "X");
    assert_eq!(p.field_text(id), "abXcdef");
}

#[test]
fn field_empty_line_and_ligature_have_caret_boundaries() {
    let (mut p, id) = boot("textarea", "ffi\\n\\nlast", "text-align=\"right\"");
    let (ox, oy) = origin(&mut p, id, true, 0.);
    let first = p
        .paragraph(id)
        .unwrap()
        .layout_runs()
        .next()
        .unwrap()
        .glyphs[0];
    assert_eq!(
        (first.start, first.end),
        (0, 3),
        "fixture has an ffi ligature"
    );
    p.press_at(ox + first.x + first.w / 3., oy + 12., 0.);
    assert_eq!(p.field_selection(id), FieldSelection::caret(1));
    p.press_at(ox + 170., oy + 36., 0.);
    assert_eq!(p.field_selection(id), FieldSelection::caret(4));
    let value = p.field_text(id);
    p.mark_field(
        id,
        &value,
        FieldSelection {
            start: 0,
            end: value.len() as u32,
            direction: SelectionDirection::Forward,
        },
        false,
    );
    let pixel = p
        .frame()
        .pixel((ox + 181.) as u32, (oy + 26.) as u32)
        .unwrap();
    assert_eq!(
        [pixel.red(), pixel.green(), pixel.blue()],
        [0xb4, 0xd5, 0xfe],
        "selected blank line"
    );
}

#[test]
fn field_word_boundaries_include_punctuation_and_utf16() {
    // Chromium on Linux, with the same values and Control arrows.
    for (value, forward, backward) in [
        ("one... two", vec![3, 6, 10], vec![7, 3, 0]),
        ("a😀b café", vec![1, 3, 4, 9], vec![5, 3, 1, 0]),
    ] {
        let (mut p, id) = boot("input", value, "");
        p.set_focus(Some(id), 0.);
        p.mark_field(id, value, FieldSelection::caret(0), false);
        p.hold_modifier("ControlLeft", true);
        for at in forward {
            key(&mut p, id, "ArrowRight");
            assert_eq!(p.field_selection(id), FieldSelection::caret(at));
        }
        for at in backward {
            key(&mut p, id, "ArrowLeft");
            assert_eq!(p.field_selection(id), FieldSelection::caret(at));
        }
    }
}

#[test]
fn field_native_content_box_places_the_caret() {
    let (mut p, id) = boot_source(
        r#"component Fields
  view
    input id="field" testId="field" value="abcdef" width=200 height=60 font-size=16 text-align="center"
"#,
    );
    let (x, y, _, _) = p.rect_of(id).unwrap();
    let node = p.host.kernel().node(id).unwrap();
    let content = node.field_content_rect().unwrap();
    let paragraph = p.paragraph(id).unwrap();
    let glyph = paragraph.layout_runs().next().unwrap().glyphs[2];
    let point = (
        x + content.x + (content.width - paragraph.width) / 2. + glyph.x + glyph.w * 0.1,
        y + content.y + content.height / 2.,
    );
    p.press_at(point.0, point.1, 0.);
    assert_eq!(p.field_selection(id), FieldSelection::caret(2));
}

#[test]
fn field_caret_after_a_final_break_and_in_an_empty_input_is_visible() {
    for (tag, value, at, y) in [
        ("textarea", "abc\\n", 4, 38.),
        ("textarea", "\\n\\n", 2, 62.),
        ("input", "", 0, 62.),
    ] {
        let (mut p, id) = boot(tag, value, "");
        p.set_focus(Some(id), 0.);
        let value = p.field_text(id);
        p.mark_field(id, &value, FieldSelection::caret(at), false);
        let pixel = p.frame().pixel(12, y as u32).unwrap();
        assert!(
            pixel.red() > 200 && pixel.green() < 50,
            "{tag} {value:?} caret: {pixel:?}"
        );
        if tag == "textarea" {
            p.press_at(14., y + 10., 0.);
            assert_eq!(p.field_selection(id), FieldSelection::caret(at));
            key(&mut p, id, "ArrowUp");
            assert!(p.field_selection(id).start < at);
        }
    }
}

#[test]
fn field_navigation_uses_the_live_value_before_display_acknowledgement() {
    let (mut p, id) = boot("textarea", "abc", "");
    p.set_focus(Some(id), 0.);
    p.mark_field(id, "abc", FieldSelection::caret(3), false);
    let first = p.display_frame().unwrap();
    assert!(p.display_complete(&first));
    key(&mut p, id, "d");
    let pending = p.display_frame().unwrap();
    key(&mut p, id, "End");
    assert_eq!(p.field_selection(id), FieldSelection::caret(4));
    key(&mut p, id, "X");
    assert_eq!(p.field_text(id), "abcdX");
    assert!(p.display_complete(&pending));
    let latest = p.display_frame().unwrap();
    assert!(p.display_complete(&latest));
    assert_eq!(p.field_selection(id), FieldSelection::caret(5));
}

#[test]
fn field_click_uses_the_live_value_before_display_acknowledgement() {
    let (mut p, id) = boot("input", "iiiiii", "");
    p.set_focus(Some(id), 0.);
    let first = p.display_frame().unwrap();
    assert!(p.display_complete(&first));
    p.mark_field(
        id,
        "iiiiii",
        FieldSelection {
            start: 0,
            end: 6,
            direction: SelectionDirection::Forward,
        },
        false,
    );
    key(&mut p, id, "W");
    let pending = p.display_frame().unwrap();
    let stale = p.brush.field_text[&id].hit(17., 72.).unwrap().offset.min(1);
    assert_ne!(
        stale, 0,
        "the point must distinguish a live hit from clamping a stale hit"
    );
    p.press_field(id, 17., 72.);
    assert_eq!(p.field_selection(id), FieldSelection::caret(0));
    p.press_field(id, 180., 72.);
    assert_eq!(p.field_selection(id), FieldSelection::caret(1));
    assert!(p.display_complete(&pending));
}

#[test]
fn field_empty_caret_keeps_text_indent() {
    for tag in ["input", "textarea"] {
        for placeholder in ["", "placeholder=\"hint\""] {
            let (mut p, id) = boot(tag, "", &format!("text-indent=\"20px\" {placeholder}"));
            p.set_focus(Some(id), 0.);
            p.mark_field(id, "", FieldSelection::caret(0), false);
            let y = if tag == "input" { 62 } else { 14 };
            let pixel = p.frame().pixel(32, y).unwrap();
            assert!(
                pixel.red() > 200 && pixel.green() < 50,
                "{tag} {placeholder}: {pixel:?}"
            );
        }
    }
}

#[test]
fn field_horizontal_arrows_visit_both_wrap_positions() {
    let (mut p, id) = boot("textarea", "abcd efgh ijkl mnop qrst uvwx", "");
    let (ox, oy) = origin(&mut p, id, true, 0.);
    let first = p.paragraph(id).unwrap().layout_runs().next().unwrap();
    let last = first.glyphs.last().unwrap();
    let boundary = last.end;
    let end_x = ox + last.x + last.w;
    p.set_focus(Some(id), 0.);
    let value = p.field_text(id);
    p.mark_field(id, &value, FieldSelection::caret(0), false);
    key(&mut p, id, "End");
    assert_eq!(p.field_selection(id), FieldSelection::caret(boundary));
    let pixels = p.frame();
    let coverage: u32 = (end_x.floor() as u32..=end_x.ceil() as u32)
        .map(|x| 255 - u32::from(pixels.pixel(x, (oy + 2.) as u32).unwrap().green()))
        .sum();
    assert!(coverage >= 250, "End caret: {coverage}");
    key(&mut p, id, "ArrowRight");
    assert_eq!(p.field_selection(id), FieldSelection::caret(boundary));
    assert!(!p.field_cursor(id).upstream);
    let pixel = p.frame().pixel(ox as u32, (oy + 26.) as u32).unwrap();
    assert!(
        pixel.red() > 200 && pixel.green() < 50,
        "next line: {pixel:?}"
    );
    key(&mut p, id, "ArrowLeft");
    assert_eq!(p.field_selection(id), FieldSelection::caret(boundary));
    assert!(p.field_cursor(id).upstream);
    key(&mut p, id, "ArrowRight");
    key(&mut p, id, "ArrowRight");
    key(&mut p, id, "ArrowLeft");
    assert_eq!(p.field_selection(id), FieldSelection::caret(boundary));
    assert!(p.field_cursor(id).upstream);
    p.hold_modifier("ShiftLeft", true);
    key(&mut p, id, "ArrowRight");
    assert_eq!(p.field_selection(id), FieldSelection::caret(boundary));
    key(&mut p, id, "ArrowRight");
    assert_eq!(
        (p.field_selection(id).start, p.field_selection(id).end),
        (boundary, boundary + 1)
    );
}

#[test]
fn field_horizontal_arrows_and_delete_use_graphemes() {
    for cluster in ["e\u{301}", "🇺🇸", "👩\u{200d}💻"] {
        let value = format!("{cluster}x");
        let (mut p, id) = boot("input", &value, "");
        p.set_focus(Some(id), 0.);
        p.mark_field(id, &value, FieldSelection::caret(0), false);
        key(&mut p, id, "ArrowRight");
        assert_eq!(
            p.field_selection(id),
            FieldSelection::caret(utf16_len(cluster)),
            "{cluster}"
        );
        key(&mut p, id, "ArrowLeft");
        assert_eq!(p.field_selection(id), FieldSelection::caret(0));
        key(&mut p, id, "Delete");
        assert_eq!(p.field_text(id), "x");
    }
    assert_eq!(
        delete(
            "e\u{301}",
            FieldSelection::caret(2),
            false,
            TextBoundary::Grapheme
        )
        .unwrap()
        .0,
        "e"
    );
}

#[test]
fn field_word_arrows_group_adjacent_emoji_as_chrome() {
    // Chrome 151.0.7922.34 on Linux, measured with Control+ArrowRight/Left.
    for (value, forward, backward) in [
        ("😀👍", vec![4], vec![0]),
        ("a😀👍b", vec![1, 5, 6], vec![5, 1, 0]),
        ("a…… b", vec![1, 3, 5], vec![4, 1, 0]),
        ("a😀...👍b", vec![1, 8, 9], vec![8, 1, 0]),
    ] {
        let (mut p, id) = boot("input", value, "");
        p.set_focus(Some(id), 0.);
        p.mark_field(id, value, FieldSelection::caret(0), false);
        p.hold_modifier("ControlLeft", true);
        for at in forward {
            key(&mut p, id, "ArrowRight");
            assert_eq!(p.field_selection(id), FieldSelection::caret(at), "{value}");
        }
        for at in backward {
            key(&mut p, id, "ArrowLeft");
            assert_eq!(p.field_selection(id), FieldSelection::caret(at), "{value}");
        }
    }
}

#[test]
fn field_pending_display_uses_live_wraps_alignment_and_password_offsets() {
    for (tag, value, extra, insertion) in [
        ("textarea", "abcd efgh ijkl mnop qrst uvwx", "", "WWWW "),
        ("input", "abc", "text-align=\"center\"", "WW"),
        (
            "input",
            "abc",
            "type=\"password\" text-align=\"right\"",
            "😀",
        ),
    ] {
        let (mut shown, id) = boot(tag, value, extra);
        let (mut immediate, other) = boot(tag, value, extra);
        for (p, id) in [(&mut shown, id), (&mut immediate, other)] {
            p.set_focus(Some(id), 0.);
            p.mark_field(id, value, FieldSelection::caret(0), false);
        }
        let first = shown.display_frame().unwrap();
        assert!(shown.display_complete(&first));
        for c in insertion.chars() {
            key(&mut shown, id, &c.to_string());
            key(&mut immediate, other, &c.to_string());
        }
        let pending = shown.display_frame().unwrap();
        assert_eq!(shown.brush.field_text[&id].value, value);
        for name in ["ArrowDown", "Home", "End", "ArrowUp"] {
            key(&mut shown, id, name);
            key(&mut immediate, other, name);
            assert_eq!(
                shown.field_selection(id),
                immediate.field_selection(other),
                "{tag} {extra} {name}"
            );
        }
        for (x, y) in [(40., 24.), (85., 48.), (170., 72.)] {
            shown.press_field(id, x, y);
            immediate.press_field(other, x, y);
            assert_eq!(
                shown.field_selection(id),
                immediate.field_selection(other),
                "{tag} {extra} click {x},{y}"
            );
        }
        key(&mut shown, id, "X");
        key(&mut immediate, other, "X");
        assert_eq!(shown.field_text(id), immediate.field_text(other));
        assert!(shown.display_complete(&pending));
        let latest = shown.display_frame().unwrap();
        assert!(shown.display_complete(&latest));
        assert_eq!(shown.field_selection(id), immediate.field_selection(other));
    }
}

#[test]
fn field_password_arrows_and_delete_cross_one_bullet() {
    for value in ["e\u{301}", "🇺🇸", "👩\u{200d}💻"] {
        let first = value.chars().next().unwrap().len_utf16() as u32;
        let last = value.chars().next_back().unwrap().len_utf16() as u32;
        let len = utf16_len(value);
        let mut actual = Vec::new();
        for (name, at) in [
            ("ArrowRight", 0),
            ("ArrowLeft", len),
            ("Delete", 0),
            ("Backspace", len),
        ] {
            let (mut p, id) = boot("input", value, "type=\"password\"");
            p.set_focus(Some(id), 0.);
            p.mark_field(id, value, FieldSelection::caret(at), false);
            key(&mut p, id, name);
            actual.push((p.field_selection(id).start, p.field_text(id)));
        }
        assert_eq!(
            actual,
            [
                (first, value.to_owned()),
                (len - last, value.to_owned()),
                (0, value.chars().skip(1).collect::<String>()),
                (
                    len - last,
                    value[..value.len() - value.chars().next_back().unwrap().len_utf8()].to_owned()
                ),
            ],
            "{value}"
        );
    }
}

#[test]
fn field_texts_name_where_the_label_and_the_value_were_painted() {
    let field = |value: &str| {
        let (mut p, id) = boot_source(&format!(
            "component Fields\n  view\n    column width=400 height=600 padding=10\n      input testId=\"field\" value=\"{value}\" placeholder=\"Title\" width=280\n"
        ));
        (p.rect_of(id).unwrap(), p.field_texts(id).unwrap())
    };
    let (rect, texts) = field("Moby");
    let (label, value) = (texts.label.unwrap(), texts.value.unwrap());
    // Both start at the field's inset; the floated label is above the value,
    // and each baseline is in its own box.
    assert_eq!(label.rect.0, rect.0 + 16.0);
    assert_eq!(value.rect.0, rect.0 + 16.0);
    assert!(label.rect.1 + label.rect.3 <= value.rect.1, "{texts:?}");
    for t in [label, value] {
        assert!(t.baseline > t.rect.1 && t.baseline < t.rect.1 + t.rect.3);
    }
    // Empty and unfocused, the label rests lower and there is no value.
    let (_, empty) = field("");
    assert_eq!(empty.value, None);
    assert!(empty.label.unwrap().rect.1 > label.rect.1, "{empty:?}");
}
