//! The neutral look on a host that paints its own controls (LLP 1115.000
//! D17), a binary of its own: the look is the process's, said once before a
//! plan boots, and every other test of this host holds the look of a process
//! that said none, which is Android's reader's. One test, so nothing here
//! reads a colour or a metric while the switch is moving.
use exact_kernel::ViewId;
use exact_linux::presenter::PainterChoice;
use exact_linux::Presenter;

/// One of each control whose size or colour the look gives, with nothing
/// written but where each sits; the same under a larger font, and under the
/// dark scheme.
const APP: &str = r##"component App
  state taps = 0
  action go
    taps = taps + 1
  view
    column width=400 padding=20 gap=10 align-items="flex-start"
      list width=300 testId="rows"
        text "One" height=40 testId="row-1"
        row press=go height=40 testId="row-2"
          text "Two"
        text "Its own line" height=40 border-bottom="2px solid -exact-system-red" testId="row-own"
        row press=go -exact-press-scale=0.96 height=40 testId="row-scaled"
          text "Its own press"
        text "Last" height=40 testId="row-last"
      list appearance="none" width=300 testId="rows-none"
        row press=go height=40 testId="none-1"
          text "The author's"
        text "rows" height=40
      button appearance="auto" testId="unsaid"
        text "Go"
      button appearance="auto" buttonStyle="filled" testId="filled"
        text "Go"
      button appearance="auto" buttonStyle="gray" disabled=true testId="disabled"
        text "Go"
      button appearance="auto" width=84 height=24 testId="sized"
        text "Go"
      input type="checkbox" testId="check"
      input type="checkbox" checked=true testId="checked"
      input type="radio" testId="radio"
      input type="checkbox" role="switch" testId="switch"
      input type="checkbox" role="switch" checked=true testId="switch-on"
      input type="range" value=50 width=160 testId="range"
      input value="Ada" testId="field"
      input value="Ada" disabled=true testId="field-disabled"
      select testId="select"
        option "One"
      column font-size=20 gap=10 align-items="flex-start"
        button appearance="auto" testId="large"
          text "Go"
        input type="checkbox" testId="large-check"
        input value="Ada" testId="large-field"
      column color-scheme="dark" width=200 padding=10 gap=10 align-items="flex-start" background-color="-exact-background" testId="dark"
        button appearance="auto" buttonStyle="filled" testId="dark-filled"
          text "Go"
        input type="checkbox" testId="dark-check"
"##;

// neutral.css styles every direct route header, including one whose two
// headings make it ineligible for a native bar. A button may be nested.
const HEADERS: &str = r##"component App
  state taps = 0
  action go
    taps = taps + 1
  view
    column navigationKey="route" width="100%" color-scheme="SCHEME" color="#aa00cc" background-color="#fedcba"
      header testId="twin" height=40 min-height=0 padding=0 box-sizing="content-box" display="flex" align-items="center" gap=8
        text "MMMM" role="heading" font-size=20 font-weight=700 line-height=1 width=72 height=24 margin=0 testId="ink"
        text "Two" role="heading" font-size=20 margin=0
        row gap=8
          button press=go width=48 height=24 testId="nested"
            text "Go"
          button width=48 height=24 border="2px solid #123456" background-color="#abcdef" testId="own-button"
            text "Own"
      text "After" height=10 testId="after"
      header testId="own" height=32 min-height=0 padding=0 box-sizing="content-box" color="#aa00cc" background-color="#abcdef" border-bottom="2px solid #654321"
        text "MMMM" font-size=20 font-weight=700 line-height=1 width=72 height=24 margin=0 testId="own-ink"
      header testId="none" height=20 min-height=0 padding=0 box-sizing="content-box" border-bottom="none"
      column
        header testId="not-direct" height=20
      text "MMMM" role="heading" font-size=20 font-weight=700 line-height=1 width=72 height=24 margin=0 color="-exact-label" background-color="-exact-background" testId="ink-reference"
      text "MMMM" font-size=20 font-weight=700 line-height=1 width=72 height=24 margin=0 color="#aa00cc" background-color="#abcdef" testId="own-ink-reference"
      text `Taps ${taps}` testId="taps"
"##;

fn id(p: &Presenter<()>, name: &str) -> ViewId {
    let k = p.host().kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}

fn size(p: &mut Presenter<()>, name: &str) -> (f32, f32) {
    let (_, _, w, h) = p.rect_of(id(p, name)).unwrap();
    (w, h)
}

/// The pixel `(x, y)` into `name`'s box.
fn pixel(p: &mut Presenter<()>, name: &str, x: f32, y: f32) -> [u8; 3] {
    let (left, top, _, _) = p.rect_of(id(p, name)).unwrap();
    let shot = p.frame();
    let c = shot.pixel((left + x) as u32, (top + y) as u32).unwrap();
    [c.red(), c.green(), c.blue()]
}

fn text_pixels(p: &mut Presenter<()>, name: &str) -> Vec<[u8; 3]> {
    let (left, top, width, height) = p.rect_of(id(p, name)).unwrap();
    let shot = p.frame();
    let mut pixels = Vec::new();
    for y in 0..height as u32 {
        for x in 0..width as u32 {
            let c = shot.pixel(left as u32 + x, top as u32 + y).unwrap();
            pixels.push([c.red(), c.green(), c.blue()]);
        }
    }
    pixels
}

fn route_headers_keep_the_page_tree_and_take_the_neutral_defaults() {
    for (scheme, ground, separator) in [("light", [255; 3], [224; 3]), ("dark", [18; 3], [56; 3])] {
        let plan = contract::compile(&HEADERS.replace("SCHEME", scheme)).unwrap();
        for width in [320.0, 640.0] {
            let (mut p, error) = Presenter::boot_with(
                &plan.encode(),
                (),
                (width, 220.0),
                1.0,
                std::path::PathBuf::new(),
                PainterChoice::Cpu,
            )
            .unwrap();
            assert!(error.is_none(), "{error:?}");
            // The CSS border occupies room, unlike a list's painted divider.
            assert_eq!(size(&mut p, "twin"), (width, 41.0), "{scheme}");
            assert_eq!(p.rect_of(id(&p, "after")).unwrap().1, 41.0);
            assert_eq!(pixel(&mut p, "twin", width - 2.0, 2.0), ground);
            assert_eq!(pixel(&mut p, "twin", width - 2.0, 39.0), ground);
            assert_eq!(pixel(&mut p, "twin", width - 2.0, 40.0), separator);
            assert_eq!(
                text_pixels(&mut p, "ink"),
                text_pixels(&mut p, "ink-reference"),
                "the header replaces inherited page ink, {scheme} at {width}"
            );
            assert_eq!(size(&mut p, "nested"), (48.0, 24.0));
            assert_eq!(pixel(&mut p, "nested", 0.0, 12.0), ground);
            assert_eq!(size(&mut p, "own-button"), (52.0, 28.0));
            assert_eq!(pixel(&mut p, "own-button", 0.0, 12.0), [0x12, 0x34, 0x56]);
            assert_eq!(pixel(&mut p, "own-button", 4.0, 12.0), [0xab, 0xcd, 0xef]);
            assert_eq!(size(&mut p, "own"), (width, 34.0));
            assert_eq!(pixel(&mut p, "own", width - 2.0, 31.0), [0xab, 0xcd, 0xef]);
            assert_eq!(pixel(&mut p, "own", width - 2.0, 33.0), [0x65, 0x43, 0x21]);
            // Written ink wins even when it equals the inherited page ink.
            assert_eq!(
                text_pixels(&mut p, "own-ink"),
                text_pixels(&mut p, "own-ink-reference")
            );
            assert_eq!(size(&mut p, "none"), (width, 20.0));
            assert_eq!(pixel(&mut p, "none", width - 2.0, 19.0), ground);
            assert_eq!(size(&mut p, "not-direct"), (width, 20.0));
            assert_eq!(
                pixel(&mut p, "not-direct", width - 2.0, 19.0),
                [0xfe, 0xdc, 0xba]
            );
            p.tap(id(&p, "nested")).unwrap();
            let k = p.host().kernel();
            let text: String = k
                .node(id(&p, "taps"))
                .unwrap()
                .text_runs()
                .iter()
                .map(|r| &*r.text)
                .collect();
            assert_eq!(text, "Taps 1", "the nested control keeps its page action");
        }
    }
}

#[test]
fn a_host_with_no_toolkit_under_it_draws_the_webs_neutral_look() {
    exact_linux::app::show_neutral_look();
    let plan = contract::compile(APP).unwrap();
    let (mut p, error) = Presenter::boot_with(
        &plan.encode(),
        (),
        (400., 1400.),
        1.,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");

    // The kernel sizes what the author left unsaid as the page's sheet
    // does: a control is 36 px at the body size whatever the font's own
    // line, a mark the line's height, a switch 36 by 20.
    assert_eq!(size(&mut p, "unsaid").1, 36.0);
    assert!(size(&mut p, "unsaid").0 > 36.0, "its title and the padding");
    assert_eq!(size(&mut p, "filled").1, 36.0);
    assert_eq!(size(&mut p, "check"), (20.0, 20.0));
    assert_eq!(size(&mut p, "radio"), (20.0, 20.0));
    assert_eq!(size(&mut p, "switch"), (36.0, 20.0));
    assert_eq!(size(&mut p, "range"), (160.0, 20.0));
    assert_eq!(size(&mut p, "field").1, 36.0);
    assert_eq!(size(&mut p, "select").1, 36.0);
    // A size the author wrote is the box: the look's floor and padding step
    // aside.
    assert_eq!(size(&mut p, "sized"), (84.0, 24.0));
    // The look's lengths are of the control's font size, which a control
    // inherits as it does on a page.
    assert_eq!(size(&mut p, "large").1, 45.0);
    assert_eq!(size(&mut p, "large-check"), (25.0, 25.0));
    // A field has no floor: its line, its padding and its edge.
    assert_eq!(size(&mut p, "large-field").1, 25.0 + 2.0 * 8.75 + 2.0);

    // The colours are the role table's neutral column. The page's ground is
    // white; a line that marks a control's edge is the label's ink at 45%.
    let (white, line, blue) = ([255; 3], [140; 3], [0x0b, 0x66, 0xd6]);
    assert_eq!(pixel(&mut p, "unsaid", 3.0, 18.0), white, "an outline");
    assert_eq!(pixel(&mut p, "unsaid", 0.0, 18.0), line);
    assert_eq!(pixel(&mut p, "filled", 3.0, 18.0), blue);
    // Disabled, `gray` keeps its neutral fill (`-exact-secondary-fill`).
    assert_eq!(pixel(&mut p, "disabled", 3.0, 18.0), [229; 3]);
    assert_eq!(pixel(&mut p, "check", 10.0, 10.0), white);
    assert_eq!(pixel(&mut p, "check", 10.0, 0.0), line);
    assert_eq!(pixel(&mut p, "checked", 10.0, 1.0), blue);
    assert_eq!(pixel(&mut p, "radio", 10.0, 0.0), line);
    // The switch: the control's line with a thumb in the page's ground at
    // its start; on, the accent with the thumb at its end.
    assert_eq!(pixel(&mut p, "switch", 30.0, 10.0), line);
    assert_eq!(pixel(&mut p, "switch", 10.0, 10.0), white);
    assert_eq!(pixel(&mut p, "switch-on", 10.0, 10.0), blue);
    assert_eq!(pixel(&mut p, "switch-on", 26.0, 10.0), white);
    // A range: a thin track in the line, nothing filled before the thumb,
    // which is the accent at the value.
    assert_eq!(pixel(&mut p, "range", 20.0, 10.0), line);
    assert_eq!(pixel(&mut p, "range", 20.0, 4.0), white);
    assert_eq!(pixel(&mut p, "range", 80.0, 10.0), blue);
    // A field: its own ground inside the line; disabled, the raised ground
    // inside a divider.
    assert_eq!(pixel(&mut p, "field", 4.0, 4.0), white);
    assert_eq!(pixel(&mut p, "field", 20.0, 0.0), line);
    assert_eq!(pixel(&mut p, "field-disabled", 4.0, 4.0), [0xf5; 3]);

    // Dark: the page's ground, the dark accent, and black ink on it.
    assert_eq!(pixel(&mut p, "dark", 2.0, 2.0), [0x12; 3]);
    assert_eq!(pixel(&mut p, "dark-filled", 3.0, 18.0), [0x68, 0xa5, 0xfe]);
    assert_eq!(pixel(&mut p, "dark-check", 10.0, 10.0), [0x1e; 3]);

    // A state adds one thing: where a pointer hovers, a wash of the label's
    // ink over the outline button; held down, a stronger one.
    let unsaid = id(&p, "unsaid");
    p.hover(unsaid).unwrap();
    assert!(p.dirty(), "a hover paints again");
    assert_eq!(pixel(&mut p, "unsaid", 3.0, 18.0), [245; 3]);
    let (x, y, _, _) = p.rect_of(unsaid).unwrap();
    assert!(p.pointer_down(x + 4.0, y + 4.0, 0.0).unwrap());
    assert_eq!(pixel(&mut p, "unsaid", 3.0, 18.0), [237; 3]);
    p.pointer_cancel(0.0).unwrap();
    assert_eq!(pixel(&mut p, "filled", 3.0, 18.0), blue, "and no other");

    // A list's rows (LLP 1115.000 D16), as the page's sheet draws them: one
    // line in the separator's role inside each row's bottom edge but the
    // last's, taking no room; none where the row's author wrote that edge;
    // none in a list whose author opted out.
    let separator = [224; 3];
    assert_eq!(size(&mut p, "row-1"), (300.0, 40.0), "a row keeps its box");
    assert_eq!(pixel(&mut p, "row-1", 150.0, 39.0), separator);
    assert_eq!(pixel(&mut p, "row-1", 150.0, 38.0), white, "one pixel");
    assert_eq!(pixel(&mut p, "row-1", 0.0, 39.0), separator, "edge to edge");
    assert_eq!(pixel(&mut p, "row-1", 299.0, 39.0), separator);
    assert_eq!(pixel(&mut p, "row-2", 150.0, 39.0), separator);
    assert_ne!(pixel(&mut p, "row-own", 150.0, 39.0), separator, "its own");
    assert_eq!(pixel(&mut p, "row-own", 150.0, 37.0), white, "and no other");
    assert_eq!(pixel(&mut p, "row-last", 150.0, 39.0), white, "none last");
    assert_eq!(pixel(&mut p, "none-1", 150.0, 39.0), white, "opted out");
    // What presses in a row washes where a pointer hovers and more while
    // held, as the look's controls do, and the line stays over the wash. A
    // row with its own press, and a row of a list that is no cell list,
    // show none of it.
    let row = id(&p, "row-2");
    p.hover(row).unwrap();
    assert_eq!(pixel(&mut p, "row-2", 290.0, 20.0), [245; 3]);
    assert_eq!(pixel(&mut p, "row-1", 290.0, 20.0), white, "and no other");
    let (x, y, _, _) = p.rect_of(row).unwrap();
    assert!(p.pointer_down(x + 290.0, y + 20.0, 0.0).unwrap());
    assert_eq!(pixel(&mut p, "row-2", 290.0, 20.0), [237; 3]);
    assert_ne!(pixel(&mut p, "row-2", 290.0, 39.0), [237; 3], "the line");
    p.pointer_cancel(0.0).unwrap();
    for (name, why) in [("row-scaled", "its own press"), ("none-1", "no row")] {
        let node = id(&p, name);
        p.hover(node).unwrap();
        assert_eq!(pixel(&mut p, name, 290.0, 20.0), white, "{name}: {why}");
        let (x, y, _, _) = p.rect_of(node).unwrap();
        assert!(p.pointer_down(x + 290.0, y + 20.0, 0.0).unwrap());
        assert_eq!(pixel(&mut p, name, 290.0, 20.0), white, "{name}: {why}");
        p.pointer_cancel(0.0).unwrap();
    }
    route_headers_keep_the_page_tree_and_take_the_neutral_defaults();
    route_header_classes_restore_defaults();
    route_header_context_changes_invalidate_retained_nodes();
}

fn route_header_classes_restore_defaults() {
    use exact_kernel::StyleId;
    let source = r##"style Authored
  color="#aa00cc"
  background-color="#abcdef"
  border-bottom="2px solid #654321"
style Plain
  opacity=1
component Header
  props
    on: bool
  view
    header class=(on ? Authored : Plain) height=20 min-height=0 padding=0 testId="head"
      text "Ink" testId="label"
component App
  state on = false
  action toggle
    on = !on
  view
    column navigationKey="route" color="#aa00cc" width=320
      if true
        Header(on=on)
      button press=toggle testId="toggle"
        text "Toggle"
"##;
    let plan = contract::compile(&source).unwrap();
    let (mut p, error) = Presenter::boot_with(
        &plan.encode(),
        (),
        (320.0, 150.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    for authored in [false, true, false, true, false] {
        assert_eq!(size(&mut p, "head").1, if authored { 22.0 } else { 21.0 });
        assert_eq!(
            pixel(&mut p, "head", 310.0, 2.0),
            if authored {
                [0xab, 0xcd, 0xef]
            } else {
                [255; 3]
            }
        );
        let k = p.host().kernel();
        for name in ["head", "label"] {
            let node = k.node(id(&p, name)).unwrap();
            assert_eq!(node.says(StyleId::TextColor), authored);
            assert_eq!(
                node.source_of(StyleId::TextColor),
                authored.then(|| id(&p, "head"))
            );
        }
        assert!(!k
            .node(id(&p, "label"))
            .unwrap()
            .style
            .written
            .has(StyleId::TextColor));
        p.tap(id(&p, "toggle")).unwrap();
    }
}

// Kernel transactions isolate live topology/prop intent from Contract's
// route restrictions. The compiled fixture above covers normal expansion.
fn route_header_context_changes_invalidate_retained_nodes() {
    use exact_kernel::{
        Kernel, NodeType, Offer, Op, PropId, PropValue, StyleId, StyleMask, StyleProps, StyleValue,
    };
    let style = |rows: &[(StyleId, StyleValue)]| {
        let mut s = StyleProps::default();
        for (id, value) in rows {
            s.set_dynamic(*id, value).unwrap();
            s.written.set(*id);
        }
        s
    };
    let prop = |id, prop, text: &str| Op::SetProp {
        id,
        prop,
        value: PropValue::Str(text.into()),
    };
    let mut k = Kernel::with_monospace();
    let mut ops: Vec<_> = (1..=6)
        .map(|id| Op::CreateView {
            id,
            node_type: if id == 3 {
                NodeType::Text
            } else {
                NodeType::View
            },
        })
        .collect();
    ops.extend([
        prop(1, PropId::NavigationKey, ""),
        prop(2, PropId::SemanticTag, "header"),
        Op::SetStyle {
            id: 1,
            patch: Box::new(style(&[(
                StyleId::TextColor,
                StyleValue::Text("#aa00cc".into()),
            )])),
        },
        Op::SetStyle {
            id: 2,
            patch: Box::new(style(&[(StyleId::Height, StyleValue::Number(20.0))])),
        },
        Op::SetChildren {
            id: 2,
            children: vec![3, 5],
        },
        Op::SetChildren {
            id: 5,
            children: vec![6],
        },
        Op::SetChildren {
            id: 1,
            children: vec![2, 4],
        },
        Op::AttachRoot { id: 1 },
    ]);
    k.apply(0, 1, &ops).unwrap();
    let check = |k: &mut Kernel, height, says, source| {
        k.compute_layout(1, Offer::definite(320.0, 150.0)).unwrap();
        assert_eq!(k.node(2).unwrap().frame.height, height);
        let n = k.node(3).unwrap();
        assert_eq!(n.says(StyleId::TextColor), says);
        assert_eq!(n.source_of(StyleId::TextColor), source);
    };
    check(&mut k, 21.0, false, None);
    let header = k.node(2).unwrap().key;
    let leaf = k.node(3).unwrap().key;
    let button = k.node(6).unwrap().key;
    // An explicit inherited clear keeps the parent as its value source,
    // including after its parent's actual color changes.
    for written in [true, false, true, false] {
        let r = k
            .apply(
                0,
                1,
                &[Op::ClearStyle {
                    id: 2,
                    mask: StyleMask::of(StyleId::TextColor),
                    written,
                }],
            )
            .unwrap();
        assert!(r.touched.contains(&header) && r.touched.contains(&leaf));
        check(&mut k, 21.0, written, written.then_some(1));
    }
    for present in [false, true] {
        let op = if present {
            prop(1, PropId::NavigationKey, "")
        } else {
            Op::ClearProp {
                id: 1,
                prop: PropId::NavigationKey,
            }
        };
        let r = k.apply(0, 1, &[op]).unwrap();
        assert!(r.layout_invalidated);
        assert!(
            r.touched.contains(&header) && r.touched.contains(&leaf) && r.touched.contains(&button)
        );
        check(
            &mut k,
            if present { 21.0 } else { 20.0 },
            !present,
            (!present).then_some(1),
        );
    }
    for tag in ["section", "header"] {
        let r = k.apply(0, 1, &[prop(2, PropId::SemanticTag, tag)]).unwrap();
        assert!(r.layout_invalidated && r.touched.contains(&button));
        check(
            &mut k,
            if tag == "header" { 21.0 } else { 20.0 },
            tag != "header",
            (tag != "header").then_some(1),
        );
    }
    for parent in [4, 1] {
        let children = if parent == 1 { vec![2, 4] } else { vec![2] };
        let r = k
            .apply(
                0,
                1,
                &[Op::SetChildren {
                    id: parent,
                    children,
                }],
            )
            .unwrap();
        assert!(r.layout_invalidated && r.touched.contains(&header) && r.touched.contains(&button));
        check(
            &mut k,
            if parent == 1 { 21.0 } else { 20.0 },
            parent != 1,
            (parent != 1).then_some(1),
        );
    }
    // A wrapper's move changes only its descendants' bar-edge context;
    // equal authored ink must not hide the required paint notification.
    k.apply(
        0,
        1,
        &[Op::SetStyle {
            id: 5,
            patch: Box::new(style(&[(
                StyleId::TextColor,
                StyleValue::Text("#aa00cc".into()),
            )])),
        }],
    )
    .unwrap();
    for parent in [4, 2] {
        let r = k
            .apply(
                0,
                1,
                &[Op::SetChildren {
                    id: parent,
                    children: if parent == 2 { vec![3, 5] } else { vec![5] },
                }],
            )
            .unwrap();
        assert!(r.touched.contains(&button));
        assert_eq!(k.node(6).unwrap().source_of(StyleId::TextColor), Some(5));
    }
}
