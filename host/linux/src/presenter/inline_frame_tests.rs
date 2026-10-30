//! Action reads of the acknowledged inline picture, with DejaVu Sans.
use super::range_frame_tests::{boot, boot_at, fixture_fonts, slot, tap, view};
use super::*;

fn app(text: &str, attrs: &str, around: &str, ids: &[&str]) -> String {
    let mut s = String::from("component App\n");
    for (i, id) in ids.iter().enumerate() {
        s += &format!("  state r{i} = \"\"\n  action m{i}\n    let f = frame(\"{id}\")\n    r{i} = f.unavailable ? \"unavailable\" : `${{f.x}} ${{f.y}} ${{f.width}} ${{f.height}}`\n");
    }
    s += "  view\n    column\n";
    for i in 0..ids.len() {
        s += &format!("      button testId=\"m{i}\" press=m{i}\n        text \"measure\"\n");
    }
    let width = if attrs.contains("width=") {
        ""
    } else {
        "width=260"
    };
    let line_height = if attrs.contains("line-height=") {
        ""
    } else {
        "line-height=\"20px\""
    };
    s += &format!("      column {around}\n        text testId=\"paragraph\" font-size=16 {line_height} {width} {attrs}\n");
    for line in text.lines() {
        s += &format!("          {line}\n");
    }
    s
}

fn measure(p: &mut Presenter<()>, i: usize) -> Option<[f64; 4]> {
    tap(p, &format!("m{i}"));
    let got = slot(p, &format!("r{i}"));
    let got = got.as_str().unwrap();
    (got != "unavailable").then(|| {
        let v: Vec<f64> = got.split(' ').map(|s| s.parse().unwrap()).collect();
        [v[0], v[1], v[2], v[3]]
    })
}

fn relative(p: &mut Presenter<()>, i: usize) -> [f64; 4] {
    let mut r = measure(p, i).expect("a painted inline run has a frame");
    let parent = p.rect_of(view(p, "paragraph")).unwrap();
    r[0] -= f64::from(parent.0);
    r[1] -= f64::from(parent.1);
    r
}

fn close(got: [f64; 4], expected: [f64; 4], tolerance: f64) {
    for (a, b) in got.into_iter().zip(expected) {
        assert!((a - b).abs() <= tolerance, "{got:?}, expected {expected:?}");
    }
}

#[test]
fn inline_frame_matches_chromes_font_boxes_and_wrap_union() {
    // Chrome hints the fixture font to whole CSS px; Parley keeps fractional advances and metrics.
    for (text, attrs, expected) in [
        (
            "text \"link\" id=\"run\" href=\"https://example.com\"",
            "",
            [0., 0., 27., 19.],
        ),
        (
            "text \"one two three\" id=\"run\"",
            "width=90",
            [0., 0., 64., 39.],
        ),
        (
            "text \"Hi\"\ntext \"\" id=\"run\"\ntext \" there\"",
            "",
            [16., 0., 0., 19.],
        ),
        (
            "text \"Hi\"\ntext \" there\" id=\"run\"",
            "width=50",
            [0., 20., 43., 19.],
        ),
        (
            "text id=\"run\" font-size=12\n  text \"BIG\" font-size=48",
            "line-height=\"60px\"",
            [0., 36., 84., 14.],
        ),
        (
            "text \"link\" id=\"run\" font-size=12\ntext \"BIG\" font-size=48",
            "line-height=\"60px\"",
            [0., 36., 21., 14.],
        ),
    ] {
        let mut p = boot(&app(text, attrs, "", &["run"]));
        close(relative(&mut p, 0), expected, 1.5);
    }
}

#[test]
fn inline_frame_empty_run_on_a_blank_hard_line() {
    for separator in [r"\n", r"\u{d}\n"] {
        let text = format!("text \"a{separator}\"\ntext \"\" id=\"run\"\ntext \"{separator}b\"");
        let mut p = boot(&app(&text, "white-space=\"pre\"", "", &["run"]));
        close(relative(&mut p, 0), [0., 20., 0., 19.], 1.5);
    }
}

#[test]
fn inline_frame_pre_wrap_keeps_chromes_hanging_spaces_and_tabs() {
    for (text, width, expected) in [
        ("one two three", 90, [0., 0., 69., 39.]),
        ("one two   three", 90, [0., 0., 79., 39.]),
        (r"Hi\tthere", 50, [0., 0., 43., 39.]),
        (r"Hi\t\tthere", 50, [0., 0., 80., 39.]),
    ] {
        let mut p = boot(&app(
            &format!("text \"{text}\" id=\"run\""),
            &format!("white-space=\"pre-wrap\" width={width}"),
            "",
            &["run"],
        ));
        close(relative(&mut p, 0), expected, 1.5);
    }
}

#[test]
fn inline_frame_publication_visits_each_box_once() {
    for count in [16, 64] {
        let mut source = String::from("component App\n  view\n    column\n");
        for i in 0..count {
            source += &format!("      text height=20\n        text \"line\" id=\"r{i}\"\n      textarea height=20 value=\"field\"\n");
        }
        let mut p = boot(&source);
        p.resize(400., 6000.);
        p.frame();
        assert_eq!(p.brush.inline_text.len(), count);
        assert_eq!(p.brush.field_text.len(), count);
        PUBLICATION_BOX_VISITS.set(0);
        p.publish_ranges();
        assert_eq!(p.ranges.inline.borrow().len(), count);
        assert_eq!(p.ranges.fields.borrow().len(), count);
        assert_eq!(PUBLICATION_BOX_VISITS.get(), p.boxes.len());
    }
}

#[test]
fn inline_frame_scroll_scale_and_paint_only_ellipsis() {
    let source = app(
        "text \"one two three\" id=\"run\"",
        "",
        "padding-top=400 height=1200",
        &["run"],
    );
    let mut p = boot(&source);
    let before = measure(&mut p, 0).unwrap();
    p.scroll_to(PAGE, (0., 100.)).unwrap();
    p.frame();
    close(
        measure(&mut p, 0).unwrap(),
        [before[0], before[1] - 100., before[2], before[3]],
        0.001,
    );
    close(
        measure(&mut boot_at(&source, 2.), 0).unwrap(),
        before,
        0.001,
    );
    let text = "text \"one two three\" id=\"run\"";
    let mut plain = boot(&app(text, "white-space=\"nowrap\"", "", &["run"]));
    let mut clipped = boot(&app(
        text,
        "white-space=\"nowrap\" width=30 overflow=\"hidden\" text-overflow=\"ellipsis\"",
        "",
        &["run"],
    ));
    close(relative(&mut clipped, 0), relative(&mut plain, 0), 0.001);
}

#[test]
fn inline_frame_rotates_each_line_before_bounding() {
    let text =
        "text id=\"all\"\n  text \"abcdefgh\" id=\"a\"\n  text \"\\n\"\n  text \"ij\" id=\"b\"";
    let mut p = boot(&app(
        text,
        "white-space=\"pre-wrap\" rotate=\"30deg\"",
        "padding=70",
        &["all", "a", "b"],
    ));
    let all = measure(&mut p, 0).unwrap();
    let a = measure(&mut p, 1).unwrap();
    let b = measure(&mut p, 2).unwrap();
    let mut plain = boot(&app(
        text,
        "white-space=\"pre-wrap\"",
        "padding=70",
        &["all", "a", "b"],
    ));
    let parent = plain.rect_of(view(&plain, "paragraph")).unwrap();
    let (cx, cy) = (
        f64::from(parent.0 + parent.2 / 2.),
        f64::from(parent.1 + parent.3 / 2.),
    );
    let (sin, cos) = (0.5, 3_f64.sqrt() / 2.);
    for (i, rotated) in [(1, a), (2, b)] {
        let [x, y, w, h] = measure(&mut plain, i).unwrap();
        close(
            rotated,
            [
                cx + (x - cx) * cos - (y + h - cy) * sin,
                cy + (x - cx) * sin + (y - cy) * cos,
                w * cos + h * sin,
                w * sin + h * cos,
            ],
            0.001,
        );
    }
    let (x, y) = (a[0].min(b[0]), a[1].min(b[1]));
    close(
        all,
        [
            x,
            y,
            (a[0] + a[2]).max(b[0] + b[2]) - x,
            (a[1] + a[3]).max(b[1] + b[3]) - y,
        ],
        0.001,
    );
}

#[test]
fn inline_frame_is_unavailable_without_its_painted_paragraph() {
    let text = "text \"link\" id=\"run\" testId=\"run\"";
    let mut hidden = boot(&app(text, "display=\"none\"", "", &["run"]));
    assert_eq!(measure(&mut hidden, 0), None);
    let source = app(text, "", "perspective=400", &["run"]);
    let projected = app(text, "rotate=\"x 30deg\"", "perspective=400", &["run"]);
    assert_eq!(measure(&mut boot(&projected), 0), None);
    let mut p = boot(&source);
    let before = measure(&mut p, 0).unwrap();
    let id = view(&p, "run");
    p.reload(
        &crate::test_plan(&app(text, "", "padding-top=50", &["run"])),
        (),
    )
    .unwrap();
    assert_eq!(view(&p, "run"), id);
    assert!(p.ranges.frame(id).unavailable);
    // (the reload took the host's fonts: the fixture's again, so the box compares like with like; it lays out and
    // paints too)
    fixture_fonts(&mut p);
    let after = measure(&mut p, 0).unwrap();
    close(
        after,
        [before[0], before[1] + 50., before[2], before[3]],
        0.001,
    );
}

#[test]
fn inline_frame_places_each_column_without_clipping_overflow() {
    let text =
        "text id=\"all\"\n  text \"one\\ntwo\\n\" id=\"first\"\n  text \"three\" id=\"last\"";
    let source = app(
        text,
        "width=\"auto\" white-space=\"pre\"",
        "column-count=2 column-gap=20 height=40 column-fill=\"auto\"",
        &["all", "first", "last"],
    )
    .replace("      column column-count", "      view column-count");
    let mut p = boot(&source);
    let all = relative(&mut p, 0);
    let first = relative(&mut p, 1);
    let last = relative(&mut p, 2);
    assert!(last[0] > 200., "the second column: {last:?}");
    close(
        [last[1], first[1], last[3], all[3]],
        [first[1], first[1], 18.625, 38.625],
        0.01,
    );
    close(
        all,
        [first[0], first[1], last[0] + last[2] - first[0], first[3]],
        0.001,
    );
}

#[test]
fn inline_frame_stays_with_the_acknowledged_picture_and_its_fonts() {
    let source = app("text id=\"run\" testId=\"run\" font-size=(large ? 48 : 16)\n  text (large ? \"Hello\" : \"Hi\")", "line-height=\"60px\"", "", &["run"])
        .replace("component App\n", "component App\n  state large = false\n  action change\n    large = ! large\n")
        .replace("    column\n", "    column\n      button testId=\"change\" press=change\n        text \"change\"\n");
    let mut p = boot(&source);
    let first = p.display_frame().unwrap();
    assert!(p.display_complete(&first));
    let before = measure(&mut p, 0).unwrap();
    tap(&mut p, "change");
    let pending = p.display_frame().unwrap();
    close(measure(&mut p, 0).unwrap(), before, 0.001);
    // C's live font and ownership must not be used to answer B after its ACK.
    tap(&mut p, "change");
    assert!(p.display_complete(&pending));
    let id = view(&p, "run");
    let b = p.ranges.frame(id);
    assert!(!b.unavailable);
    assert!((b.height - before[3] * 3.).abs() < 0.001, "{b:?}");
    assert!(b.width > before[2] * 3., "{b:?}");
    let latest = p.display_frame().unwrap();
    assert!(p.display_complete(&latest));
    close(measure(&mut p, 0).unwrap(), before, 0.001);
}

#[test]
fn inline_frame_gives_a_cross_run_ligature_to_its_first_character() {
    let mut p = boot(&app(
        "text id=\"all\"\n  text \"f\" id=\"f\"\n  text \"i\" id=\"i\"",
        "",
        "",
        &["all", "f", "i"],
    ));
    let id = view(&p, "paragraph");
    let paragraph = p.paragraph(id).unwrap();
    let advancing: Vec<_> = paragraph
        .layout_runs()
        .next()
        .unwrap()
        .glyphs
        .iter()
        .filter(|g| g.w > 0.)
        .collect();
    assert_eq!(advancing.len(), 1, "the fixture must shape a ligature");
    assert_eq!(advancing[0].range(), 0..2);
    let all = measure(&mut p, 0).unwrap();
    let f = measure(&mut p, 1).unwrap();
    let i = measure(&mut p, 2).unwrap();
    close(f, all, 0.001);
    close(i, [all[0] + all[2], all[1], 0., all[3]], 0.001);
}

#[test]
fn inline_frame_is_unavailable_for_an_unmounted_virtual_row() {
    let rows = (0..200)
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let source = format!(
        r#"component App
  state rows = [{rows}]
  state result = false
  action measure
    let f = frame("run-199")
    result = f.unavailable
  view
    column
      button testId="measure" press=measure
        text "measure"
      list virtualized=true width=260 height=100
        for x in rows key=x
          text height=30
            text `row ${{x}}` id=`run-${{x}}`
"#
    );
    let mut p = boot(&source);
    tap(&mut p, "measure");
    assert_eq!(slot(&p, "result"), true);
}
