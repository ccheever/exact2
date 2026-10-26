//! `rangeFrame` read from the field as painted, with fixture fonts.
use super::*;
use std::path::PathBuf;

pub(super) fn boot(source: &str) -> Presenter<()> {
    boot_at(source, 1.)
}

pub(super) fn boot_at(source: &str, scale: f32) -> Presenter<()> {
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        (),
        (400., 600.),
        scale,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    fixture_fonts(&mut p);
    p
}

/// The fixture fonts (DejaVu Sans) measured and painted from here on, the presenter laid out and painted again: a
/// boot's, and a reload's after it, since a reload takes the new plan's own text engine (the host's fonts).
pub(super) fn fixture_fonts(p: &mut Presenter<()>) {
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
}

pub(super) fn view(p: &Presenter<()>, test_id: &str) -> ViewId {
    let k = p.host.kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

pub(super) fn tap(p: &mut Presenter<()>, test_id: &str) {
    let id = view(p, test_id);
    p.tap(id).unwrap();
    p.run_commands(|| ());
}

pub(super) fn slot(p: &Presenter<()>, name: &str) -> serde_json::Value {
    let state: serde_json::Value =
        serde_json::from_str(&p.host.agent("{\"op\":\"state\"}")).unwrap();
    state["slots"][name].clone()
}

/// An app whose field `composer` (`attrs`, value `value`) is measured
/// over each of `ranges` by its own button, each answer in its own slot as
/// `x y width height`, or `unavailable`.
fn app(attrs: &str, value: &str, ranges: &[(u32, u32)], around: &str) -> String {
    let mut s = String::from("component App\n");
    for (i, _) in ranges.iter().enumerate() {
        s += &format!("  state r{i} = \"\"\n");
    }
    for (i, (start, end)) in ranges.iter().enumerate() {
        s += &format!(
            "  action m{i}\n    let f = rangeFrame(\"composer\", {start}, {end})\n    r{i} = f.unavailable ? \"unavailable\" : `${{f.x}} ${{f.y}} ${{f.width}} ${{f.height}}`\n"
        );
    }
    s += &format!("  view\n    column{around}\n");
    // The buttons first, so a scaled field covers none of them.
    for (i, _) in ranges.iter().enumerate() {
        s += &format!("      button testId=\"m{i}\" press=m{i}\n        text \"m\"\n");
    }
    s += &format!(
        "      textarea id=\"composer\" testId=\"field\" value=\"{value}\" appearance=\"none\" padding=0 border-width=0 font-size=16 {attrs}\n"
    );
    s
}

/// Each range's answer, in order.
fn measure(p: &mut Presenter<()>, count: usize) -> Vec<Option<[f64; 4]>> {
    (0..count)
        .map(|i| {
            tap(p, &format!("m{i}"));
            let got = slot(p, &format!("r{i}"));
            let got = got.as_str().unwrap().to_owned();
            (got != "unavailable").then(|| {
                let n: Vec<f64> = got.split(' ').map(|v| v.parse().unwrap()).collect();
                [n[0], n[1], n[2], n[3]]
            })
        })
        .collect()
}

/// The field's painted glyphs: each one's text offset, left edge and
/// advance, and its line's top and height.
fn glyphs(p: &Presenter<()>, id: ViewId) -> Vec<(u32, f32, f32, f32, f32)> {
    p.paragraph(id)
        .unwrap()
        .layout_runs()
        .flat_map(|line| {
            line.glyphs
                .iter()
                .map(|g| (g.start, g.x, g.w, line.line_top, line.line_height))
                .collect::<Vec<_>>()
        })
        .collect()
}

const VALUE: &str = "hi @alice.bsky.social see https://bsky.app #art";

#[test]
fn a_caret_and_a_word_are_read_from_the_glyphs_painted() {
    let ranges = [(3, 3), (3, 21), (0, 47), (0, 48), (5, 3)];
    let mut p = boot(&app("width=340 height=120", VALUE, &ranges, " padding=24"));
    let id = view(&p, "field");
    let field = p.rect_of(id).unwrap();
    let got = measure(&mut p, ranges.len());
    let glyphs = glyphs(&p, id);
    let at = |offset| glyphs.iter().find(|g| g.0 == offset).copied().unwrap();
    let (_, x, _, top, height) = at(3);
    let caret = got[0].unwrap();
    assert_eq!(
        [caret[0], caret[2]],
        [f64::from(field.0 + x), 0.],
        "the caret before `@`: its glyph's left edge, width 0"
    );
    let (line_top, line_bottom) = (f64::from(field.1 + top), f64::from(field.1 + top + height));
    assert!(
        caret[1] >= line_top - 1e-3 && caret[1] + caret[3] <= line_bottom + 1e-3,
        "{caret:?}"
    );
    let word = got[1].unwrap();
    let advances: f32 = glyphs
        .iter()
        .filter(|g| (3..21).contains(&g.0))
        .map(|g| g.2)
        .sum();
    assert_eq!([word[0], word[1], word[3]], [caret[0], caret[1], caret[3]]);
    assert!(
        (word[2] - f64::from(advances)).abs() < 1e-3,
        "{word:?} {advances}"
    );
    assert!(got[2].is_some(), "the whole value");
    assert_eq!(got[3], None, "past the text");
    assert_eq!(got[4], None, "a reversed range");
}

/// Chrome's text box is its font's content area, not the line's box: under
/// `line-height="40px"` a word and a caret stand centred in their line, 40
/// apart from the next, as `createValueRange` measures them.
#[test]
fn a_range_is_its_texts_box_not_its_lines() {
    let ranges = [(0, 3), (1, 1), (4, 7)];
    let mut p = boot(&app(
        "width=340 height=200 line-height=\"40px\"",
        "abc\\ndef",
        &ranges,
        "",
    ));
    let field = p.rect_of(view(&p, "field")).unwrap();
    let got = measure(&mut p, ranges.len());
    let (word, caret, next) = (got[0].unwrap(), got[1].unwrap(), got[2].unwrap());
    assert!(
        word[3] < 30.,
        "the font's height, not the line's 40: {word:?}"
    );
    let centred = f64::from(field.1) + (40. - word[3]) / 2.;
    assert!((word[1] - centred).abs() < 1., "{word:?} {centred}");
    assert_eq!(
        [caret[1], caret[3]],
        [word[1], word[3]],
        "a caret is its text's box too"
    );
    assert_eq!(next[1] - word[1], 40., "the next line");
}

#[test]
fn a_line_break_bounds_as_chrome_bounds_it() {
    // Chrome (createValueRange, 2026-10-20): `ab\ncd` 0..3 is `ab` alone,
    // the break's box empty; 2..3, the break alone, is its zero-width box at
    // the line's end; 1..4 spans both lines.
    let ranges = [(0, 3), (2, 3), (1, 4), (0, 2), (3, 5)];
    let mut p = boot(&app("width=200 height=120", "ab\\ncd", &ranges, ""));
    let got: Vec<[f64; 4]> = measure(&mut p, ranges.len())
        .into_iter()
        .map(Option::unwrap)
        .collect();
    let (ab, cd) = (got[3], got[4]);
    assert_eq!(got[0], ab, "the break adds nothing to `ab`");
    assert_eq!(got[1], [ab[0] + ab[2], ab[1], 0., ab[3]], "the break alone");
    let x = ab[0].min(cd[0]);
    let right = (ab[0] + ab[2]).max(cd[0] + cd[2]);
    assert_eq!(
        got[2],
        [x, ab[1], right - x, cd[1] + cd[3] - ab[1]],
        "both lines"
    );
}

#[test]
fn a_caret_at_a_wrap_is_on_the_line_it_ends() {
    // Each offset's character, then each offset's caret.
    let n = VALUE.len() as u32;
    let mut ranges: Vec<(u32, u32)> = (0..n).map(|k| (k, k + 1)).collect();
    ranges.extend((0..n).map(|k| (k, k)));
    let mut p = boot(&app("width=120 height=300", VALUE, &ranges, ""));
    let got = measure(&mut p, ranges.len());
    let y = |i: usize| got[i].unwrap()[1];
    let wrap = (1..n as usize)
        .find(|&k| y(k) > y(0))
        .expect("the value wraps");
    let caret = n as usize;
    assert_eq!(
        y(caret + wrap),
        y(wrap - 1),
        "the wrap's caret ends the first line"
    );
    assert_eq!(y(caret + wrap + 1), y(wrap), "past it, the next");
}

/// A rotation bounds each fragment, then their union, as CSSOM does: the
/// two lines of a range measure as the union of each line's own range.
#[test]
fn a_rotated_range_bounds_its_fragments() {
    let ranges = [(0, 5), (0, 2), (3, 5)];
    let mut p = boot(&app(
        "width=200 height=120 rotate=\"30deg\"",
        "abcdefgh\\nij",
        &[(0, 11), (0, 8), (9, 11)],
        " padding=60",
    ));
    let _ = ranges;
    let got: Vec<[f64; 4]> = measure(&mut p, 3).into_iter().map(Option::unwrap).collect();
    let (a, b) = (got[1], got[2]);
    let x = a[0].min(b[0]);
    let y = a[1].min(b[1]);
    let union = [
        x,
        y,
        (a[0] + a[2]).max(b[0] + b[2]) - x,
        (a[1] + a[3]).max(b[1] + b[3]) - y,
    ];
    for (g, u) in got[0].iter().zip(union) {
        assert!((g - u).abs() < 1e-3, "{:?} {union:?}", got[0]);
    }
}

#[test]
fn an_input_is_unavailable() {
    let source =
        app("width=340 height=120", VALUE, &[(0, 1)], "").replace("textarea id=", "input id=");
    assert_eq!(measure(&mut boot(&source), 1), [None]);
}

#[test]
fn a_range_moves_with_the_page_and_its_transform_and_not_with_the_scale() {
    let ranges = [(3, 21)];
    let source = app(
        "width=340 height=120",
        VALUE,
        &ranges,
        " padding-top=400 height=1200",
    );
    let mut p = boot(&source);
    let before = measure(&mut p, 1)[0].unwrap();
    p.scroll_to(PAGE, (0., 100.)).unwrap();
    p.frame();
    let after = measure(&mut p, 1)[0].unwrap();
    assert_eq!(
        after,
        [before[0], before[1] - 100., before[2], before[3]],
        "scrolled, unclipped"
    );
    let mut doubled = boot_at(&source, 2.);
    assert_eq!(
        measure(&mut doubled, 1)[0].unwrap(),
        before,
        "CSS px at any display scale"
    );
    let mut scaled = boot(&app(
        "width=340 height=120 scale=2 transform-origin=\"0 0\"",
        VALUE,
        &ranges,
        "",
    ));
    let plain = measure(
        &mut boot(&app("width=340 height=120", VALUE, &ranges, "")),
        1,
    )[0]
    .unwrap();
    let big = measure(&mut scaled, 1)[0].unwrap();
    assert_eq!(
        [big[2], big[3]],
        [plain[2] * 2., plain[3] * 2.],
        "the transform's bounds"
    );
}

#[test]
fn a_field_not_painted_is_unavailable_and_a_reload_reads_only_its_own() {
    let ranges = [(0, 1)];
    let mut p = boot(&app(
        "width=340 height=120 display=\"none\"",
        VALUE,
        &ranges,
        "",
    ));
    assert_eq!(measure(&mut p, 1), [None], "display: none");
    let mut p = boot(&app("width=340 height=120", VALUE, &ranges, ""));
    let first = measure(&mut p, 1)[0].unwrap();
    let id = view(&p, "field");
    // Another plan whose field (the same view id) stands lower: until its
    // runner's first paint is shown, nothing is read, never the old box.
    let moved = app("width=340 height=120", VALUE, &ranges, " padding-top=50");
    p.reload(&crate::test_plan(&moved), ()).unwrap();
    assert_eq!(view(&p, "field"), id, "the new runner reuses the view id");
    assert!(
        p.ranges.range_frame(id, 0, 1).unavailable,
        "the old paint is not read"
    );
    fixture_fonts(&mut p);
    p.frame();
    let after = measure(&mut p, 1)[0].unwrap();
    assert_eq!(after[1], first[1] + 50., "the new paint is");
}

#[test]
fn a_field_under_a_perspective_is_unavailable() {
    let ranges = [(0, 1)];
    let flat = app("width=340 height=120", VALUE, &ranges, " perspective=400");
    let turned = flat.replace(
        "      textarea",
        "      column rotate=\"x 30deg\"\n        textarea",
    );
    assert_ne!(flat, turned);
    assert!(
        measure(&mut boot(&flat), 1)[0].is_some(),
        "a perspective alone"
    );
    assert_eq!(
        measure(&mut boot(&turned), 1),
        [None],
        "turned out of the screen's plane"
    );
}

/// Conform's decorations fixture answers its font-independent facts here as
/// on the web (theme2's 17:27 ruling), each of them true.
#[test]
fn the_conform_fixture_reads_its_facts() {
    let mut p = boot(include_str!(
        "../../../web-js/conformance/decorations.contract"
    ));
    tap(&mut p, "measure");
    assert_eq!(
        slot(&p, "geometry"),
        "caret width 0 mention on its line true from it true wide true inside true past true"
    );
    tap(&mut p, "measure-lines");
    assert_eq!(
        slot(&p, "lines"),
        "empty line at the start true between true height true width 0; empty field where a typed one starts true width 0"
    );
}

/// Where two directions meet the caret ends the run its offset closes, as
/// Chrome takes the first fragment (createValueRange, `abc אבג`: 4 is the
/// space's end, 30.23, not the Hebrew run's start at its right; 7 is the
/// Hebrew run's left edge).
#[test]
fn a_caret_between_directions_ends_the_run_before_it() {
    let value = "abc \u{5d0}\u{5d1}\u{5d2}";
    let ranges = [(4, 4), (7, 7), (0, 0)];
    let mut p = boot(&app("width=340 height=120", value, &ranges, ""));
    let id = view(&p, "field");
    let field = p.rect_of(id).unwrap();
    let got = measure(&mut p, ranges.len());
    let glyphs = glyphs(&p, id);
    let space = glyphs.iter().find(|g| g.0 == 3).unwrap();
    let hebrew = glyphs
        .iter()
        .filter(|g| g.0 >= 4)
        .map(|g| g.1)
        .fold(f32::MAX, f32::min);
    assert_eq!(got[0].unwrap()[0], f64::from(field.0 + space.1 + space.2));
    assert_eq!(got[1].unwrap()[0], f64::from(field.0 + hebrew));
    assert_eq!(got[2].unwrap()[0], f64::from(field.0));
}

/// A collapsed range answers where the caret is painted (theme2's 18:42
/// ruling, G3): an empty field's at its start, an empty line's on that
/// line, with the text's height there, as the web's glue now measures it
/// too (conform's decorations fixture compares the facts).
#[test]
fn an_empty_field_and_an_empty_lines_caret_are_where_it_is_painted() {
    let mut p = boot(&app("width=200 height=120", "", &[(0, 0)], ""));
    let field = p.rect_of(view(&p, "field")).unwrap();
    let empty = measure(&mut p, 1)[0].unwrap();
    assert_eq!(
        [empty[0], empty[2]],
        [f64::from(field.0), 0.],
        "at the start: {empty:?}"
    );
    assert!(
        empty[1] >= f64::from(field.1) && empty[3] > 10.,
        "{empty:?}"
    );
    let ranges = [(3, 3), (0, 2)];
    let mut p = boot(&app("width=200 height=120", "ab\\n", &ranges, ""));
    let got = measure(&mut p, 2);
    let (last, ab) = (got[0].unwrap(), got[1].unwrap());
    assert!(
        last[1] >= ab[1] + ab[3],
        "the final empty line, below `ab`: {last:?} {ab:?}"
    );
    assert_eq!([last[2], last[3]], [0., ab[3]]);
    let ranges = [(2, 2), (0, 1), (3, 4)];
    let mut p = boot(&app("width=200 height=120", "a\\n\\nb", &ranges, ""));
    let got = measure(&mut p, 3);
    let (middle, a, b) = (got[0].unwrap(), got[1].unwrap(), got[2].unwrap());
    assert!(
        middle[1] > a[1] && middle[1] < b[1],
        "the middle line: {got:?}"
    );
}

/// Under `direction: rtl` the first fragment left to right still wins
/// (Chrome, measured: `אבג abc` at 4 is `abc`'s left edge, 145.69 of 200,
/// not the space's end at its right; `abc אבג` at 3 is the space's right
/// edge, left of `abc`, not `c`'s end).
#[test]
fn a_caret_between_directions_in_a_right_to_left_line() {
    let ranges = [(4, 4)];
    let value = "\u{5d0}\u{5d1}\u{5d2} abc";
    let mut p = boot(&app(
        "width=200 height=120 direction=\"rtl\"",
        value,
        &ranges,
        "",
    ));
    let id = view(&p, "field");
    let field = p.rect_of(id).unwrap();
    let got = measure(&mut p, 1)[0].unwrap();
    let latin = glyphs(&p, id)
        .iter()
        .filter(|g| g.0 >= 7)
        .map(|g| g.1)
        .fold(f32::MAX, f32::min);
    assert_eq!(got[0], f64::from(field.0 + latin), "`abc`'s left edge");
    let ranges = [(3, 3)];
    let mut p = boot(&app(
        "width=200 height=120 direction=\"rtl\"",
        "abc \u{5d0}\u{5d1}\u{5d2}",
        &ranges,
        "",
    ));
    let id = view(&p, "field");
    let field = p.rect_of(id).unwrap();
    let got = measure(&mut p, 1)[0].unwrap();
    let space = *glyphs(&p, id).iter().find(|g| g.0 == 3).unwrap();
    assert_eq!(
        got[0],
        f64::from(field.0 + space.1 + space.2),
        "the space's right edge"
    );
}
