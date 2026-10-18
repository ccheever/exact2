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
