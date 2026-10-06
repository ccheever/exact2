//! @ref LLP 1053 §0 G4, G5 — CSS white space, `nowrap`, `pre-line`, `text-overflow`,
//! tabular figures and inline backgrounds, with the browser as the oracle.
use super::*;
use exact_kernel::{StyleProps, WhiteSpace};

fn engine(font: &'static [u8], family: &str) -> TextEngine {
    TextEngine::with_catalog(catalog::Catalog::from_bytes(&[font], family))
}
const INTER: &[u8] = include_bytes!("../../tests/fonts/Inter-Regular.ttf");
const DEJAVU: &[u8] = include_bytes!("../../../../scripts/fixtures/fonts/assets/DejaVuSans.ttf");

fn spec(text: &str, white_space: WhiteSpace) -> Spec {
    crate::paint::text_spec(
        &StyleProps {
            font_size: 16.0,
            white_space,
            ..Default::default()
        },
        text,
    )
}

#[test]
fn normal_and_nowrap_collapse_as_the_browser_renders() {
    let mut e = engine(INTER, "Inter");
    let clean = e.measure(&spec("a b c", WhiteSpace::Normal), AxisOffer::MaxContent);
    for source in ["  a \n\t b\r\nc  ", "a  b   c", "a\tb\nc"] {
        for ws in [WhiteSpace::Normal, WhiteSpace::Nowrap] {
            let s = spec(source, ws);
            assert_eq!(s.runs[0].text, "a b c", "{source:?}");
            assert_eq!(
                e.measure(&s, AxisOffer::MaxContent),
                clean,
                "{source:?} {ws:?}"
            );
        }
    }
    // pre-wrap keeps every character; NBSP is never collapsible.
    assert_eq!(spec("  a \n", WhiteSpace::PreWrap).runs[0].text, "  a \n");
    assert_eq!(
        spec("a\u{a0}\u{a0}b", WhiteSpace::Normal).runs[0].text,
        "a\u{a0}\u{a0}b"
    );
    // Only white space is empty: no line box, as a `<div> </div>`.
    assert_eq!(
        e.measure(&spec(" \n\t ", WhiteSpace::Normal), AxisOffer::MaxContent),
        TextMetrics::default()
    );
    // Across runs the first space is kept in its own run.
    let mut s = spec("", WhiteSpace::Normal);
    s.runs = ["a ", " b", "  c"]
        .iter()
        .map(|t| {
            Run::from_style(
                t,
                exact_kernel::TextStyle::from_style(&StyleProps::default()),
            )
        })
        .collect();
    let s = s.collapse_white_space();
    assert_eq!(
        s.runs.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
        ["a ", "b", " c"]
    );
}

#[test]
fn nowrap_min_content_is_max_content_and_no_soft_break_is_taken() {
    let mut e = engine(INTER, "Inter");
    let text = "a nowrap label that is much wider than its box";
    let nowrap = spec(text, WhiteSpace::Nowrap);
    let min = e.measure(&nowrap, AxisOffer::MinContent);
    let max = e.measure(&nowrap, AxisOffer::MaxContent);
    assert_eq!(min, max);
    let narrow = e.measure(&nowrap, AxisOffer::Definite(60.0));
    assert_eq!(narrow.height, max.height, "one line at any width");
    assert!(narrow.width > 60.0, "the line overflows");
    let normal = spec(text, WhiteSpace::Normal);
    assert!(e.measure(&normal, AxisOffer::MinContent).width < min.width);
    assert!(e.measure(&normal, AxisOffer::Definite(60.0)).height > max.height);
}

#[test]
fn pre_line_keeps_line_feeds_wraps_and_sizes_by_forced_lines() {
    // Chrome 154's innerText for each source under `white-space: pre-line`.
    for (source, rendered) in [
        ("a    b", "a b"),
        ("a  \n  b", "a\nb"),
        ("a\n\nb", "a\n\nb"),
        ("\nfirst", "\nfirst"),
        ("  lead\n  mid  \ntrail  ", "lead\nmid\ntrail"),
        ("a\r\nb", "a\nb"),
        ("a\tb\t\nc", "a b\nc"),
    ] {
        assert_eq!(spec(source, WhiteSpace::PreLine).runs[0].text, rendered);
    }
    let mut e = engine(INTER, "Inter");
    let one = |e: &mut TextEngine, t: &str| {
        e.measure(&spec(t, WhiteSpace::Normal), AxisOffer::MaxContent)
    };
    let line = one(&mut e, "one two three");
    let s = spec("one two three  \n  four", WhiteSpace::PreLine);
    let max = e.measure(&s, AxisOffer::MaxContent);
    assert_eq!(
        max.width, line.width,
        "max-content is the longest forced line"
    );
    // Lines as the collapsed text's forced breaks, preserved, give them.
    let pre = |e: &mut TextEngine, t: &str, w| e.measure(&spec(t, WhiteSpace::PreWrap), w);
    assert_eq!(
        max,
        pre(&mut e, "one two three\nfour", AxisOffer::MaxContent)
    );
    let min = e.measure(&s, AxisOffer::MinContent);
    assert_eq!(
        min.width,
        one(&mut e, "three").width,
        "min-content the longest word"
    );
    let offer = one(&mut e, "one two").width + 1.0;
    let narrow = e.measure(&s, AxisOffer::Definite(offer));
    assert_eq!(
        narrow,
        pre(&mut e, "one two\nthree\nfour", AxisOffer::MaxContent),
        "and it wraps"
    );
    // A blank line keeps its line box, as in the browser.
    let blank = e.measure(&spec("a\n\nb", WhiteSpace::PreLine), AxisOffer::MaxContent);
    assert_eq!(blank, pre(&mut e, "a\n\nb", AxisOffer::MaxContent));
    assert!(blank.height > 2.5 * line.height);
}

#[test]
fn pre_keeps_spaces_and_tabs_and_breaks_only_at_line_feeds() {
    // @ref LLP 1053 G5 — Chrome 154's widths at 16px DejaVu Sans (the
    // pinned face, as a web font): kept spaces, trailing ones included,
    // tab stops every eight spaces from the line's start, and lines only
    // at line feeds.
    let mut e = engine(DEJAVU, "DejaVu Sans");
    for (text, chrome, lines) in [
        ("  lead   inner  ", 111.0_f64, 1),
        ("ab   ", 35.21875, 1),
        ("abcdefghij\tk", 131.328125, 1),
        ("a\tb", 50.84375, 1),
        ("\t\tx", 90.84375, 1),
        ("one\n\nthree  ", 52.5, 3),
    ] {
        let s = spec(text, WhiteSpace::Pre);
        assert_eq!(s.runs[0].text, text, "pre collapses nothing");
        let max = e.measure(&s, AxisOffer::MaxContent);
        // This engine holds a width in whole pixels, up (`text.rs`).
        assert_eq!(
            f64::from(max.width),
            chrome.ceil(),
            "{text:?} against Chrome's {chrome}"
        );
        // As many lines as `pre-wrap` takes at max-content: the line feeds'.
        let wrap = e.measure(&spec(text, WhiteSpace::PreWrap), AxisOffer::MaxContent);
        assert_eq!(max.height, wrap.height, "{text:?}, {lines} lines");
        assert_eq!(
            e.measure(&s, AxisOffer::MinContent),
            max,
            "min-content is max-content"
        );
        assert_eq!(
            e.measure(&s, AxisOffer::Definite(20.0)).height,
            max.height,
            "no width wraps it"
        );
    }
}

#[test]
fn content_ending_within_a_64th_of_a_pixel_past_the_width_fits() {
    // Blink decides fit in LayoutUnits (1/64 px), and content may end one
    // unit past the available width: Chrome keeps a message line it measures
    // at exactly 200.00 px on one line where the float advances sum a few
    // thousandths over (LLP 1085.000 host parity).
    let mut e = engine(INTER, "Inter");
    let s = spec("fits within a unit", WhiteSpace::Normal);
    let full = e
        .paragraph(&s, None)
        .layout_runs()
        .map(|r| r.line_w)
        .fold(0.0, f32::max);
    let mut lines = |w: f32| e.paragraph(&s, Some(w)).layout_runs().count();
    assert_eq!(lines(full), 1);
    assert_eq!(lines(full - 0.005), 1, "within one unit of {full}");
    assert_eq!(lines(full - 0.03), 2, "past one unit of {full}");
}

#[test]
fn ellipsis_ends_an_over_wide_nowrap_line_in_paint_only() {
    let mut e = engine(INTER, "Inter");
    let s = spec(
        "a nowrap label that is much wider than its box",
        WhiteSpace::Nowrap,
    );
    let p = e.paragraph(&s, Some(100.0));
    assert!(p.width > 100.0);
    let shown = p.ellipsized(100.0).expect("over-wide");
    assert!(shown.width <= 100.0 + 0.5, "{}", shown.width);
    assert_eq!(shown.height, p.height, "paint keeps the measured line box");
    assert!(
        Rc::ptr_eq(&shown, &p.ellipsized(100.0).unwrap()),
        "kept per width"
    );
    // A line that fits is painted as measured.
    assert!(e
        .paragraph(&spec("fits", WhiteSpace::Nowrap), Some(100.0))
        .ellipsized(100.0)
        .is_none());
    let visible: String = shown
        .layout_runs()
        .flat_map(|r| r.glyphs.iter().map(|g| g.glyph_id))
        .map(|g| g.to_string())
        .collect();
    let plain: String = p
        .layout_runs()
        .flat_map(|r| r.glyphs.iter().map(|g| g.glyph_id))
        .map(|g| g.to_string())
        .collect();
    assert_ne!(visible, plain);
}

fn digits(e: &mut TextEngine, text: &str, numeric: u8) -> f32 {
    let mut s = spec(text, WhiteSpace::Normal);
    s.runs[0].font_variant_numeric = numeric;
    e.measure(&s, AxisOffer::MaxContent).width
}

#[test]
fn tabular_nums_is_the_faces_tnum_feature() {
    // Inter: proportional figures by default, `tnum` present.
    let mut e = engine(INTER, "Inter");
    assert!(digits(&mut e, "111", 0) < digits(&mut e, "888", 0));
    assert_eq!(digits(&mut e, "111", 1), digits(&mut e, "888", 1));
    // DejaVu Sans: already tabular figures and no `tnum`: nothing changes.
    let mut e = engine(DEJAVU, "DejaVu Sans");
    assert_eq!(digits(&mut e, "111", 0), digits(&mut e, "111", 1));
    assert_eq!(digits(&mut e, "111", 1), digits(&mut e, "888", 1));
}

#[test]
fn tabular_nums_is_a_distinct_shape_identity() {
    let mut e = engine(INTER, "Inter");
    let a = spec("1111", WhiteSpace::Normal);
    let mut b = a.clone();
    b.runs[0].font_variant_numeric = 1;
    let pa = e.paragraph(&a, None);
    let pb = e.paragraph(&b, None);
    assert!(!Rc::ptr_eq(&pa, &pb));
    assert_ne!(pa.width, pb.width);
}

#[test]
fn an_inline_background_covers_each_line_fragment_of_its_run() {
    let mut e = engine(INTER, "Inter");
    let mut s = spec("", WhiteSpace::Normal);
    let style = exact_kernel::TextStyle::from_style(&StyleProps::default());
    s.runs = ["plain ", "list_viewport code run", " after"]
        .iter()
        .map(|t| Run::from_style(t, style))
        .collect();
    let p = e.paragraph(&s, Some(150.0));
    assert!(p.layout_runs().count() > 1, "the code run wraps");
    let gray = [242, 242, 247, 255];
    let rects = p.run_backgrounds(&[None, Some(gray), None]);
    assert_eq!(rects.len(), 2, "one rectangle per line fragment: {rects:?}");
    let (ascent, descent, _) = p.source.data.run_metrics[1];
    for ((x, y, w, h), color) in &rects {
        assert_eq!(*color, gray);
        assert!(*w > 0.0 && *x >= 0.0 && x + w <= 150.5);
        assert!(
            (h - (ascent + descent)).abs() < 0.01,
            "the run font's content area"
        );
        assert!(*y >= -0.01);
    }
    assert!(
        rects[1].0 .1 > rects[0].0 .1,
        "second fragment on the next line"
    );
    assert!(rects[0].0 .0 > 1.0, "starts after the plain run");
    assert!(p.run_backgrounds(&[None, None, None]).is_empty());
}

#[test]
fn document_language_replaces_the_shaping_catalog_and_cached_paragraphs() {
    use exact_kernel::TextMeasurer;
    let shared = TextEngine::shared();
    let mut measurer = super::Measurer(shared.clone());
    let old = shared.borrow().catalog.clone();
    measurer.set_language("ar");
    let engine = shared.borrow();
    assert_eq!(engine.catalog.borrow().locale, "ar");
    assert!(!std::rc::Rc::ptr_eq(&old, &engine.catalog));
    drop(engine);
    measurer.set_language("en");
    assert_eq!(shared.borrow().catalog.borrow().locale, "en");
}

/// The reader diary: a line broken at a soft hyphen shows one (the face's
/// own `-`, its advance part of the line), as Chrome does, and breaks there
/// only if the hyphen fits too; one not chosen stays invisible.
#[test]
fn a_line_broken_at_a_soft_hyphen_shows_the_faces_hyphen() {
    let mut e = engine(INTER, "Inter");
    let dash = e.paragraph(&spec("-", WhiteSpace::Normal), None);
    let dash = dash.layout_runs().next().unwrap().glyphs[0];
    let fits = e
        .paragraph(&spec("an incom-", WhiteSpace::Normal), None)
        .layout_runs()
        .next()
        .unwrap()
        .line_w;
    let s = spec(
        "an extra\u{ad}ordinary incom\u{ad}prehensibly",
        WhiteSpace::Normal,
    );
    let shy = |l: &LayoutRun<'_>, g: &LayoutGlyph| &l.text[g.range()] == "\u{ad}";
    let wide = |e: &mut TextEngine, width: f32| {
        let s = spec("an incom\u{ad}prehensibly", WhiteSpace::Normal);
        let p = e.paragraph(&s, Some(width));
        let first = p.layout_runs().next().unwrap();
        (
            first
                .glyphs
                .last()
                .map(|g| (shy(&first, g), g.glyph_id, g.w)),
            first.line_w,
        )
    };
    let (last, line_w) = wide(&mut e, fits + 0.5);
    assert_eq!(
        last.map(|l| (l.0, l.1)),
        Some((true, dash.glyph_id)),
        "breaks at the soft hyphen and shows `-`"
    );
    assert!(
        (last.unwrap().2 - dash.w).abs() < 1e-3 && (line_w - fits).abs() < 0.01,
        "its advance is the line's"
    );
    let (last, _) = wide(&mut e, fits - 0.5);
    assert_eq!(
        last.map(|l| l.0),
        Some(false),
        "where the hyphen would not fit the word moves whole"
    );
    let p = e.paragraph(&s, Some(1000.0));
    for line in p.layout_runs() {
        for g in line.glyphs.iter().filter(|g| shy(&line, g)) {
            assert_eq!(g.w, 0.0, "an unchosen soft hyphen stays invisible");
        }
    }
    // `hyphens: none` reaches the runs as U+034F (the kernel's): no break there.
    let none = spec("an incom\u{34f}prehensibly", WhiteSpace::Normal);
    let p = e.paragraph(&none, Some(fits + 0.5));
    let first = p.layout_runs().next().unwrap();
    assert!(
        first.glyphs.iter().all(|g| &first.text[g.range()] != "p"),
        "the word moves whole"
    );
}

/// CSS `text-indent`: the first line starts that far in and has that much
/// less room; it counts in max-content and in the first word's min-content.
#[test]
fn text_indent_insets_the_first_line_only() {
    let mut e = engine(INTER, "Inter");
    let mut s = spec("aaa bbb ccc ddd eee fff", WhiteSpace::Normal);
    let plain = e.measure(&s, AxisOffer::MaxContent);
    let word = e.measure(&s, AxisOffer::MinContent);
    s.text_indent = 24.0;
    let indented = e.measure(&s, AxisOffer::MaxContent);
    assert_eq!(indented.width, (plain.width + 24.0).ceil());
    // The widest piece is now the first word with the indent before it.
    let first = e.measure(&spec("aaa", WhiteSpace::Normal), AxisOffer::MaxContent);
    let min = e.measure(&s, AxisOffer::MinContent);
    assert!(
        min.width > word.width && min.width >= first.width - 1.0 + 24.0,
        "{min:?}"
    );
    let p = e.paragraph(&s, Some(100.0));
    let lines: Vec<_> = p.layout_runs().collect();
    assert!(lines.len() >= 2);
    assert_eq!(lines[0].glyphs[0].x, 24.0, "the first line is inset");
    assert_eq!(lines[1].glyphs[0].x, 0.0, "the second is not");
    assert!(
        lines[0].line_w <= 100.0,
        "the indent took room from the line"
    );
    // Under `rtl` the start edge is the right.
    s.direction = exact_kernel::Direction::Rtl;
    s.align = TextAlign::Right;
    let p = e.paragraph(&s, Some(100.0));
    let first = p.layout_runs().next().unwrap();
    let right = first.glyphs.iter().map(|g| g.x + g.w).fold(0.0, f32::max);
    assert!(
        (right - 76.0).abs() < 0.5,
        "rtl first line ends 24 in from the right: {right}"
    );
}

/// LLP 1045 D4: a Markdown list item's lines all start at its level's
/// indent (the browser's `padding-inline-start: 40px` per `<ul>`/`<ol>`),
/// its marker hung before the first, ending at the indent, so ordered
/// numbers right-align as the browser's outside markers do; the measured
/// width is the painted one.
#[test]
fn a_markdown_list_items_lines_start_at_its_indent() {
    let mut e = engine(DEJAVU, "DejaVu Sans");
    let source = "Top\n\n- one two three four five six seven eight\n  - nested\n\n9. nine\n10. ten";
    let s = spec(source, WhiteSpace::PreWrap).markdown();
    let p = e.paragraph(&s, Some(200.0));
    // Each visual line: its first glyph's run, where it starts, and where
    // the marker (a hung run) ends.
    let lines: Vec<(String, f32, Option<f32>)> = p
        .layout_runs()
        .filter(|line| !line.glyphs.is_empty())
        .map(|line| {
            let text = |g: &LayoutGlyph| &s.runs[g.run()];
            let marker = line
                .glyphs
                .iter()
                .filter(|g| text(g).hang)
                .map(|g| g.x + g.w)
                .reduce(f32::max);
            let first = line
                .glyphs
                .iter()
                .find(|g| !text(g).hang)
                .map_or(0.0, |g| g.x);
            let from = line
                .glyphs
                .iter()
                .find(|g| !text(g).hang)
                .map_or(0, |g| g.start as usize);
            (line.text[from..].trim().to_string(), first, marker)
        })
        .collect();
    let at = |prefix: &str| {
        lines
            .iter()
            .find(|l| l.0.starts_with(prefix))
            .unwrap_or_else(|| panic!("{prefix}: {lines:?}"))
    };
    assert_eq!(at("Top").1, 0.0);
    let (one, wrapped) = (
        at("one"),
        lines
            .iter()
            .find(|l| l.2.is_none() && l.1 > 0.0 && !l.0.starts_with("one"))
            .unwrap(),
    );
    assert!(
        (one.1 - 40.0).abs() < 0.01 && (one.2.unwrap() - 40.0).abs() < 0.01,
        "{lines:?}"
    );
    assert!(
        (wrapped.1 - 40.0).abs() < 0.01,
        "a wrapped line starts at the indent: {lines:?}"
    );
    let nested = at("nested");
    assert!(
        (nested.1 - 80.0).abs() < 0.01 && (nested.2.unwrap() - 80.0).abs() < 0.01,
        "{lines:?}"
    );
    for number in ["nine", "ten"] {
        let l = at(number);
        assert!(
            (l.1 - 40.0).abs() < 0.01 && (l.2.unwrap() - 40.0).abs() < 0.01,
            "{number}: {lines:?}"
        );
    }
    // Measure answers what paint lays out.
    let measured = e.measure(&s, AxisOffer::Definite(200.0));
    assert_eq!((measured.height, measured.width), (p.height, p.width));
    assert!(p.width <= 200.0, "{}", p.width);
}
