//! The neutral look on a host that paints its own controls (LLP 1115.000
//! D17), a binary of its own: the look is the process's, said once before a
//! plan boots, and every other test of this host holds the look of a process
//! that said none, which is Android's reader's. One test, so nothing here
//! reads a colour or a metric while the switch is moving.
mod fields;
mod tabs;

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
      button appearance="auto" buttonStyle="plain" padding=0 testId="flush"
        text "Go"
      button appearance="auto" buttonStyle="plain" padding-top=0 testId="one-side"
        text "Go"
      button appearance="auto" padding=0 testId="outline"
        text "Go"
      row role="radiogroup" align-items="flex-start"
        button role=(taps % 2 == 0 ? "radio" : "button") buttonStyle="plain" padding=0 press=go testId="segment-padded"
          text "Go"
        button role="radio" buttonStyle="plain" testId="segment-unsaid"
          text "Go"
      column role=(taps % 2 == 0 ? "menu" : "group") align-items="flex-start"
        button buttonStyle="plain" padding=0 testId="menu-padded"
          text "Go"
        button buttonStyle="plain" testId="menu-unsaid"
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
        input type="checkbox" role="switch" testId="dark-switch"
        input type="checkbox" role="switch" checked=true testId="dark-switch-on"
      button appearance="auto" -exact-control-size="mini" testId="mini"
        text "Go"
      button appearance="auto" -exact-control-size="small" testId="small"
        text "Go"
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
      row height=0 background-color="-exact-background" border-bottom="1px solid -exact-separator" testId="edge-reference"
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
    for (scheme, ground) in [("light", [255; 3]), ("dark", [18; 3])] {
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
            // Read under its own roles: the test's other sessions live too.
            let _roles = p.host().runner().bind_roles();
            assert!(error.is_none(), "{error:?}");
            // The CSS border occupies room, unlike a list's painted divider.
            assert_eq!(size(&mut p, "twin"), (width, 41.0), "{scheme}");
            assert_eq!(p.rect_of(id(&p, "after")).unwrap().1, 41.0);
            assert_eq!(pixel(&mut p, "twin", width - 2.0, 2.0), ground);
            assert_eq!(pixel(&mut p, "twin", width - 2.0, 39.0), ground);
            // The same translucent role goes through the ordinary border painter.
            assert_eq!(
                pixel(&mut p, "twin", width - 2.0, 40.0),
                pixel(&mut p, "edge-reference", width - 2.0, 0.0)
            );
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
    // This binary has one test; pin before any engine reads installed fonts.
    std::env::set_var(
        "EXACT_FONTS",
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/fixtures/fonts/assets"
        ),
    );
    std::env::set_var("EXACT_FONT", "DejaVu Sans");
    exact_linux::app::show_neutral_look();
    fields::focus_changes_during_flow_damage();
    fields::select_and_date_focus();
    invalid_non_fields_keep_their_accent_ring();
    disabled_glass_and_invalid_fields_follow_the_neutral_look();
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
    // Read under its own roles: the test's other sessions live too.
    let _roles = p.host().runner().bind_roles();
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
    assert_eq!(size(&mut p, "flush").1, 20.0);
    assert_eq!(size(&mut p, "one-side").1, 27.0);
    assert_eq!(size(&mut p, "outline").1, 22.0);
    for role in ["segment", "menu"] {
        assert_eq!(size(&mut p, &format!("{role}-padded")), (36.0, 36.0));
        assert_eq!(size(&mut p, &format!("{role}-unsaid")).1, 36.0);
    }
    for expected in [20.0, 36.0] {
        p.tap(id(&p, "segment-padded")).unwrap();
        assert_eq!(size(&mut p, "segment-padded").1, expected);
        assert_eq!(size(&mut p, "menu-padded").1, expected);
    }
    // The look's lengths are of the control's font size, which a control
    // inherits as it does on a page.
    assert_eq!(size(&mut p, "large").1, 45.0);
    assert_eq!(size(&mut p, "large-check"), (25.0, 25.0));
    // A field has no floor: its line, its padding and its edge.
    assert_eq!(size(&mut p, "large-field").1, 25.0 + 2.0 * 8.75 + 2.0);
    // A mini or small button is titled as the page titles it at a 16 px root,
    // in the subheadline's 14 px: its line, its padding and its edge.
    for name in ["mini", "small"] {
        assert_eq!(
            size(&mut p, name).1,
            14.0 * 1.25 + 2.0 * 14.0 * 0.4375 + 2.0,
            "{name}"
        );
    }

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
    // The switch: the control's line with a white thumb at its start; on,
    // the accent with the thumb at its end.
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
    // A switch's thumb is white there too, off or on.
    assert_eq!(pixel(&mut p, "dark-switch", 10.0, 10.0), white);
    assert_eq!(pixel(&mut p, "dark-switch-on", 26.0, 10.0), white);

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
    // Held, a filled button shows UIKit's highlight instead: its fill at 75%
    // in light, mixed 20% with white in dark.
    for (name, held) in [("filled", [72, 140, 224]), ("dark-filled", [134, 183, 254])] {
        let (x, y, _, _) = p.rect_of(id(&p, name)).unwrap();
        assert!(p.pointer_down(x + 4.0, y + 4.0, 0.0).unwrap());
        assert_eq!(pixel(&mut p, name, 3.0, 18.0), held, "{name} held");
        p.pointer_cancel(0.0).unwrap();
    }

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
    route_header_geometry_matches_the_browser();
    route_header_sticky_geometry_and_hits_follow_the_browser();
    route_header_padding_is_the_containing_width_of_a_windowed_list();
    route_header_classes_restore_defaults();
    route_header_context_changes_invalidate_retained_nodes();
    route_header_effective_rows_reach_layout_admission_and_motion();
    tabs::narrow_tab_bars_follow_the_neutral_paint_rules();
    tabs::narrow_tab_geometry_follows_root_metrics_and_authored_rows();
}

fn route_header_classes_restore_defaults() {
    use exact_kernel::StyleId;
    let source = r##"style Authored
  color="#aa00cc"
  background-color="#abcdef"
  border-bottom="2px solid #654321"
style Plain
  opacity=1
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
component Header
  props
    on: bool
  view
    header class=(on ? Authored : Plain) height=20 min-height=0 padding=0 testId="head"
      text "Ink" testId="label"
"##;
    let plan = contract::compile(source).unwrap();
    let (mut p, error) = Presenter::boot_with(
        &plan.encode(),
        (),
        (320.0, 150.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    // Read under its own roles: the test's other sessions live too.
    let _roles = p.host().runner().bind_roles();
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
            let expected = if authored {
                exact_kernel::Color::rgba(0xaa, 0, 0xcc, 255)
            } else {
                exact_kernel::Color::rgba(0, 0, 0, 255)
            };
            assert_eq!(
                node.computed_row(StyleId::TextColor, |s| s.text_color.resolve(false)),
                expected
            );
            assert_eq!(
                node.computed_style(exact_kernel::StyleMask::INHERITED)
                    .text_color
                    .resolve(false),
                expected
            );
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
            } else if id == 6 {
                NodeType::Control
            } else {
                NodeType::View
            },
        })
        .collect();
    ops.extend([
        prop(1, PropId::NavigationKey, ""),
        prop(2, PropId::SemanticTag, "header"),
        prop(6, PropId::Type, "button"),
        Op::SetStyle {
            id: 1,
            patch: Box::new(style(&[(
                StyleId::TextColor,
                StyleValue::Text("#aa00cc".into()),
            )])),
        },
        Op::SetStyle {
            id: 2,
            patch: Box::new(style(&[
                (StyleId::Height, StyleValue::Number(20.0)),
                (StyleId::MinHeight, StyleValue::Number(0.0)),
                (StyleId::PaddingTop, StyleValue::Number(0.0)),
                (StyleId::PaddingRight, StyleValue::Number(0.0)),
                (StyleId::PaddingBottom, StyleValue::Number(0.0)),
                (StyleId::PaddingLeft, StyleValue::Number(0.0)),
            ])),
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
    let button_node = k.node(6).unwrap();
    assert_eq!(
        exact_kernel::ControlKind::of(button_node.node_type, button_node.props),
        Some(exact_kernel::ControlKind::Button)
    );
    let check = |k: &mut Kernel, height, says, source| {
        k.compute_layout(1, Offer::definite(320.0, 150.0)).unwrap();
        let header = k.node(2).unwrap();
        assert_eq!(header.frame.height, height);
        assert_eq!(k.sticky_nodes().contains(&header.key), height == 21.0);
        assert_eq!(k.sticky_constraint(header.key).is_some(), height == 21.0);
        let n = k.node(3).unwrap();
        assert_eq!(n.says(StyleId::TextColor), says);
        assert_eq!(n.source_of(StyleId::TextColor), source);
        let expected = source.map_or(exact_kernel::Color::rgba(0, 0, 0, 255), |id| {
            k.node(id).unwrap().style.text_color.resolve(false)
        });
        assert_eq!(
            n.computed_row(StyleId::TextColor, |s| s.text_color.resolve(false)),
            expected
        );
        assert_eq!(
            n.computed_style(StyleMask::INHERITED)
                .text_color
                .resolve(false),
            expected
        );
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
        if written {
            k.apply(
                0,
                1,
                &[Op::SetStyle {
                    id: 1,
                    patch: Box::new(style(&[(
                        StyleId::TextColor,
                        StyleValue::Text("#123456".into()),
                    )])),
                }],
            )
            .unwrap();
            check(&mut k, 21.0, true, Some(1));
        }
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
    // A written clear blocks the platform row while ordinary removal restores it.
    for row in [StyleId::PositionType, StyleId::Top] {
        for written in [true, false] {
            k.apply(
                0,
                1,
                &[Op::ClearStyle {
                    id: 2,
                    mask: StyleMask::of(row),
                    written,
                }],
            )
            .unwrap();
            k.compute_layout(1, Offer::definite(320.0, 150.0)).unwrap();
            let node = k.node(2).unwrap();
            assert!(!node.style.mask.has(row));
            assert_eq!(node.style.written.has(row), written);
            let sticky = !(row == StyleId::PositionType && written);
            assert_eq!(k.sticky_nodes().contains(&header), sticky);
            let constraint = k.sticky_constraint(header);
            assert_eq!(constraint.is_some(), sticky);
            if let Some(c) = constraint {
                assert_eq!(
                    c.insets[0],
                    if row == StyleId::Top && written {
                        None
                    } else {
                        Some(0.0)
                    }
                );
            }
        }
    }
}

// These public kernel seams must see the same defaults as Taffy and paint.
// They share this process's Neutral column; the ordinary kernel suite does not.
fn route_header_effective_rows_reach_layout_admission_and_motion() {
    use exact_kernel::{
        ContentRegion, Kernel, NodeType, Offer, Op, PropId, StyleId, StyleProps, StyleValue,
    };
    use exact_motion::{Property, Value};
    let create = |id| Op::CreateView {
        id,
        node_type: NodeType::View,
    };
    let prop = |id, prop, value: &str| Op::SetProp {
        id,
        prop,
        value: value.into(),
    };
    let children = |id, children: &[u32]| Op::SetChildren {
        id,
        children: children.to_vec(),
    };
    let style = |id, rows: &[(StyleId, &str)]| {
        let mut patch = StyleProps::default();
        for (row, text) in rows {
            let value = text
                .parse::<f64>()
                .map_or_else(|_| StyleValue::Text((*text).into()), StyleValue::Number);
            patch.set_dynamic(*row, &value).unwrap();
            patch.written.set(*row);
        }
        Op::SetStyle {
            id,
            patch: Box::new(patch),
        }
    };
    let mut k = Kernel::with_monospace();
    k.apply(
        0,
        1,
        &[
            create(1),
            create(2),
            create(3),
            create(4),
            create(5),
            prop(1, PropId::NavigationKey, ""),
            prop(3, PropId::Id, "photo"),
            prop(5, PropId::TransformDragFor, "photo"),
            style(
                2,
                &[
                    (StyleId::Width, "100px"),
                    (StyleId::Height, "100px"),
                    (StyleId::PositionType, "relative"),
                    (StyleId::PaddingTop, "0"),
                    (StyleId::PaddingRight, "0"),
                    (StyleId::PaddingBottom, "0"),
                    (StyleId::PaddingLeft, "0"),
                    (StyleId::OverflowX, "hidden"),
                    (StyleId::OverflowY, "hidden"),
                    (
                        StyleId::Transition,
                        "background-color 1s linear, border-bottom-color 1s linear",
                    ),
                ],
            ),
            style(
                3,
                &[
                    (StyleId::Width, "100%"),
                    (StyleId::Height, "100%"),
                    (StyleId::BoxSizing, "border-box"),
                ],
            ),
            style(4, &[(StyleId::PositionType, "absolute")]),
            children(1, &[2]),
            children(2, &[3, 4]),
            children(3, &[5]),
            Op::AttachRoot { id: 1 },
        ],
    )
    .unwrap();
    let handle = k.node(5).unwrap().key;
    let binding = ContentRegion {
        owner: k.node(2).unwrap().key,
        content: k.node(3).unwrap().key,
        pending: k.node(4).unwrap().key,
    };
    assert!(k.transform_drag_binding(handle).is_some());
    k.set_content_region(Some(binding)).unwrap();
    k.set_content_region(None).unwrap();
    k.apply(0, 1, &[prop(2, PropId::SemanticTag, "header")])
        .unwrap();
    assert!(
        k.transform_drag_binding(handle).is_none(),
        "the clip now has a border"
    );
    assert!(
        k.set_content_region(Some(binding)).is_err(),
        "the owner is no longer unpadded"
    );
    let node = k.node(2).unwrap();
    assert!(!node.style.mask.has(StyleId::BackgroundColor));
    assert!(!node.style.written.has(StyleId::BorderWidthBottom));
    for (dark, ground, edge) in [
        (false, [255, 255, 255, 255], [0, 0, 0, 31]),
        (true, [18, 18, 18, 255], [255, 255, 255, 41]),
    ] {
        let targets = exact_kernel::motion::color_targets(&node, dark);
        for (property, [r, g, b, a]) in [
            (Property::BackgroundColor, ground),
            (Property::BorderBottomColor, edge),
        ] {
            assert_eq!(
                targets.iter().find(|(p, _)| *p == property).unwrap().1,
                Some(Value::rgba8(r, g, b, a))
            );
        }
    }
    k.apply(
        0,
        1,
        &[style(2, &[(StyleId::BorderColorBottom, "currentcolor")])],
    )
    .unwrap();
    assert_eq!(
        k.current_color_sides(binding.owner),
        [Property::BorderBottomColor]
    );
    k.apply(0, 1, &[style(2, &[(StyleId::BorderStyleBottom, "none")])])
        .unwrap();
    assert!(
        k.transform_drag_binding(handle).is_some(),
        "an authored absent edge restores zero insets"
    );
    k.set_content_region(Some(binding)).unwrap();
    k.set_content_region(None).unwrap();
    // With the size floor disabled, an auto-height header still paints
    // decoration; the existing multicol refusal must keep that box whole.
    let mut k = Kernel::with_monospace();
    k.apply(
        0,
        1,
        &[
            create(1),
            create(2),
            create(3),
            prop(1, PropId::NavigationKey, ""),
            prop(2, PropId::SemanticTag, "header"),
            style(
                1,
                &[
                    (StyleId::Width, "200px"),
                    (StyleId::Height, "40px"),
                    (StyleId::ColumnCount, "2"),
                    (StyleId::ColumnFill, "auto"),
                ],
            ),
            style(2, &[(StyleId::MinHeight, "0")]),
            style(3, &[(StyleId::Height, "80px")]),
            children(1, &[2]),
            children(2, &[3]),
            Op::AttachRoot { id: 1 },
        ],
    )
    .unwrap();
    k.compute_layout(1, Offer::definite(200.0, 40.0)).unwrap();
    let header = k.node(2).unwrap().key;
    assert_eq!(
        k.fragment_refusal(header),
        Some(exact_kernel::fragment::FragmentRefusal::Decoration)
    );
    k.apply(
        0,
        1,
        &[Op::ClearProp {
            id: 2,
            prop: PropId::SemanticTag,
        }],
    )
    .unwrap();
    k.compute_layout(1, Offer::definite(200.0, 40.0)).unwrap();
    assert_eq!(k.fragment_refusal(header), None);

    // An implied sticky establishes the same containing block and stacking
    // context as an authored sticky, even with no z-index.
    let mut k = Kernel::with_monospace();
    let mut ops: Vec<_> = (1..=6).map(create).collect();
    ops.extend([
        prop(1, PropId::NavigationKey, ""),
        prop(2, PropId::SemanticTag, "header"),
        prop(4, PropId::SemanticTag, "header"),
        style(1, &[(StyleId::Width, "200px"), (StyleId::Height, "200px")]),
        style(4, &[(StyleId::PositionType, "sticky")]),
        style(6, &[(StyleId::Height, "30px")]),
        children(1, &[6, 2, 4]),
        children(2, &[3]),
        children(4, &[5]),
        Op::AttachRoot { id: 1 },
    ]);
    for child in [3, 5] {
        ops.push(style(
            child,
            &[
                (StyleId::PositionType, "absolute"),
                (StyleId::Left, "5px"),
                (StyleId::Top, "3px"),
                (StyleId::Width, "10px"),
                (StyleId::Height, "10px"),
            ],
        ));
    }
    k.apply(0, 1, &ops).unwrap();
    k.compute_layout(1, Offer::definite(200.0, 200.0)).unwrap();
    for (parent, child, y) in [(2, 3, 30.0), (4, 5, 75.0)] {
        let parent = k.node(parent).unwrap();
        let child = k.node(child).unwrap();
        assert_eq!(parent.frame.y, y);
        assert_eq!(
            (
                child.frame.x - parent.frame.x,
                child.frame.y - parent.frame.y
            ),
            (5.0, 3.0)
        );
        let order = exact_kernel::paint_order::own(k.arena(), parent.key.index);
        assert!(order.positioned && order.stacks);
        assert_eq!(order.z, None);
    }
    // In a column flow, sticky is relative. Its absolute descendant must
    // still follow the column containing that positioned header's content.
    k.apply(
        0,
        1,
        &[
            create(7),
            style(
                1,
                &[
                    (StyleId::Height, "40px"),
                    (StyleId::ColumnCount, "2"),
                    (StyleId::ColumnFill, "auto"),
                ],
            ),
            style(
                2,
                &[
                    (StyleId::MinHeight, "0"),
                    (StyleId::PaddingTop, "0"),
                    (StyleId::PaddingRight, "0"),
                    (StyleId::PaddingBottom, "0"),
                    (StyleId::PaddingLeft, "0"),
                    (StyleId::BackgroundColor, "transparent"),
                    (StyleId::BorderStyleBottom, "none"),
                ],
            ),
            style(3, &[(StyleId::Top, "20px")]),
            style(7, &[(StyleId::Height, "60px")]),
            children(1, &[6, 2]),
            children(2, &[3, 7]),
        ],
    )
    .unwrap();
    k.compute_layout(1, Offer::definite(200.0, 40.0)).unwrap();
    let reference = k.node(3).unwrap().frame;
    assert!(reference.x > 5.0 && reference.y < 50.0, "{reference:?}");
    assert!(k.columns(k.node(1).unwrap().key).unwrap().columns.len() >= 2);
    assert_eq!(k.fragment_refusal(k.node(2).unwrap().key), None);
    for op in [
        style(2, &[(StyleId::PositionType, "sticky")]),
        Op::ClearStyle {
            id: 2,
            mask: exact_kernel::StyleMask::of(StyleId::PositionType),
            written: false,
        },
    ] {
        k.apply(0, 1, &[op]).unwrap();
        k.compute_layout(1, Offer::definite(200.0, 40.0)).unwrap();
        assert_eq!(k.node(3).unwrap().frame, reference);
    }
}

// Chrome 155.0.8059.80, 2026-10-14: ordinary emitted app, two widths and both schemes.
// These boxes distinguish a content minimum from a border-box minimum.
const HEADER_GEOMETRY: &str = r##"style Own
  padding="2px 4px"
  min-height=0
style Plain
  opacity=1
component App
  state own = false
  action toggle
    own = !own
  view
    column navigationKey="route" width="100%" color-scheme="SCHEME" background-color="#abcdef"
      button press=toggle testId="toggle" width=80 height=24
        text "Toggle"
      header testId="empty"
      header testId="short"
        row width=20 height=8 background-color="#008800" testId="short-child"
      header testId="tall"
        row width=20 height=60 background-color="#008800" testId="tall-child"
      header height=8 testId="height-only"
      header box-sizing="border-box" testId="border-box"
        row width=20 height=8 testId="border-child"
      header padding="3px 5px" padding-left=9 min-height=0 testId="authored"
        row width=20 height=8 testId="authored-child"
      header class=(own ? Own : Plain) testId="dynamic"
        row width=20 height=8 testId="dynamic-child"
      header padding=0 min-height=0 testId="zero"
        row width=20 height=8
      column
        header testId="indirect"
          row width=20 height=8
      text (own ? "Own" : "Plain") testId="mode"
"##;

fn route_header_geometry_matches_the_browser() {
    use exact_kernel::{Dimension, StyleId};
    for scheme in ["light", "dark"] {
        let plan = contract::compile(&HEADER_GEOMETRY.replace("SCHEME", scheme)).unwrap();
        for width in [320.0, 640.0] {
            let (mut p, error) = Presenter::boot_with(
                &plan.encode(),
                (),
                (width, 640.0),
                1.0,
                std::path::PathBuf::new(),
                PainterChoice::Cpu,
            )
            .unwrap();
            // Read under its own roles: the test's other sessions live too.
            let _roles = p.host().runner().bind_roles();
            assert!(error.is_none(), "{error:?}");
            for authored in [false, true, false] {
                for (name, height) in [
                    ("empty", 45.0),
                    ("short", 45.0),
                    ("tall", 73.0),
                    ("height-only", 45.0),
                    ("border-box", 32.0),
                    ("authored", 15.0),
                    ("dynamic", if authored { 13.0 } else { 45.0 }),
                    ("zero", 9.0),
                    ("indirect", 8.0),
                ] {
                    assert_eq!(
                        size(&mut p, name),
                        (width, height),
                        "{name}, {scheme}, authored={authored}"
                    );
                }
                for (parent, child, left, top) in [
                    ("short", "short-child", 16.0, 6.0),
                    ("tall", "tall-child", 16.0, 6.0),
                    ("border-box", "border-child", 16.0, 6.0),
                    ("authored", "authored-child", 9.0, 3.0),
                    (
                        "dynamic",
                        "dynamic-child",
                        if authored { 4.0 } else { 16.0 },
                        if authored { 2.0 } else { 6.0 },
                    ),
                ] {
                    let a = p.rect_of(id(&p, parent)).unwrap();
                    let b = p.rect_of(id(&p, child)).unwrap();
                    assert_eq!((b.0 - a.0, b.1 - a.1), (left, top), "{parent}");
                }
                let k = p.host().kernel();
                let node = k.node(id(&p, "empty")).unwrap();
                for row in [
                    StyleId::MinHeight,
                    StyleId::PaddingTop,
                    StyleId::PositionType,
                    StyleId::Top,
                ] {
                    assert!(!node.style.mask.has(row) && !node.style.written.has(row));
                    assert!(!node.says(row));
                    assert_eq!(node.source_of(row), None);
                }
                assert_eq!(
                    node.computed_row(StyleId::MinHeight, |s| s.min_height),
                    Dimension::Points(32.0)
                );
                p.tap(id(&p, "toggle")).unwrap();
            }
        }
    }
}

// A top bound to an absent optional value restores the same default as an
// unwritten top above. Explicit static/auto stay authored opt-outs.
const HEADER_STICKY: &str = r##"style Plain
  opacity=1
style Static
  position="static"
component App
  state mode = 0
  state taps = 0
  action next
    mode = mode == 3 ? 0 : mode + 1
  action go
    taps = taps + 1
  view
    column width="100%" color-scheme="SCHEME"
      button press=next testId="next" width=80 height=24
        text "Next"
      scroll testId="port" width="100%" height=160 scrollbar-width="none"
        column navigationKey="route" testId="section" height=300 display="block" background-color="#abcdef"
          row height=20 background-color="#336699"
          header testId="sticky" class=(mode == 1 ? Static : Plain) top=(mode == 2 ? "12px" : (mode == 3 ? "auto" : none))
            button press=go width=40 height=20 testId="pinned"
              text "Tap"
          row height=220 background-color="#dd7755" testId="later"
        row height=200 background-color="#3355aa" testId="tail"
      text `Mode ${mode}` testId="mode"
      text `Taps ${taps}` testId="taps"
"##;

fn route_header_sticky_geometry_and_hits_follow_the_browser() {
    for (scheme, ground) in [("light", [255; 3]), ("dark", [18; 3])] {
        let plan = contract::compile(&HEADER_STICKY.replace("SCHEME", scheme)).unwrap();
        for width in [320.0, 640.0] {
            let (mut p, error) = Presenter::boot_with(
                &plan.encode(),
                (),
                (width, 320.0),
                1.0,
                std::path::PathBuf::new(),
                PainterChoice::Cpu,
            )
            .unwrap();
            // Read under its own roles: the test's other sessions live too.
            let _roles = p.host().runner().bind_roles();
            assert!(error.is_none(), "{error:?}");
            let (port, head, pinned) = (id(&p, "port"), id(&p, "sticky"), id(&p, "pinned"));
            let mut taps = 0;
            for phase in 0..5 {
                let mode = phase % 4;
                let key = p.host().kernel().node(head).unwrap().key;
                assert_eq!(p.host().kernel().sticky_nodes().contains(&key), mode != 1);
                let constraint = p.host().kernel().sticky_constraint(key);
                assert_eq!(constraint.is_some(), mode != 1);
                if let Some(c) = constraint {
                    assert_eq!(c.scroller, port);
                    assert_eq!(c.natural, [0.0, 20.0, width, 65.0]);
                    assert_eq!(
                        c.insets,
                        [
                            if mode == 3 {
                                None
                            } else {
                                Some(if mode == 2 { 12.0 } else { 0.0 })
                            },
                            None,
                            None,
                            None
                        ]
                    );
                }
                for (scroll, expected) in [
                    (0.0, 20.0),
                    (
                        60.0,
                        match mode {
                            1 | 3 => -40.0,
                            2 => 12.0,
                            _ => 0.0,
                        },
                    ),
                    (270.0, if matches!(mode, 1 | 3) { -250.0 } else { -15.0 }),
                ] {
                    let dy = constraint.map_or(0.0, |c| c.offset((0.0, scroll)).1);
                    assert_eq!(20.0 + dy - scroll, expected, "mode{mode}, scroll{scroll}");
                    assert_eq!(p.scroll_to(port, (0.0, scroll)), Some((0.0, scroll)));
                    p.frame();
                    if scroll == 60.0 || (scroll == 270.0 && matches!(mode, 0 | 2)) {
                        let port_rect = p.rect_of(port).unwrap();
                        let head_rect = p.rect_of(head).unwrap();
                        assert_eq!(
                            (
                                head_rect.0,
                                head_rect.1 - port_rect.1,
                                head_rect.2,
                                head_rect.3
                            ),
                            (0.0, expected, width, 45.0)
                        );
                    }
                    if scroll == 60.0 && matches!(mode, 0 | 2) {
                        assert_eq!(
                            pixel(&mut p, "sticky", width - 2.0, 1.0),
                            ground,
                            "header paints above later in-flow content"
                        );
                        if mode == 0 {
                            let (x, y, w, h) = p.rect_of(pinned).unwrap();
                            assert!(p
                                .pointer_down(x + w / 2.0, y + h / 2.0, phase as f64 * 100.0)
                                .unwrap());
                            p.pointer_up(x + w / 2.0, y + h / 2.0, phase as f64 * 100.0 + 1.0)
                                .unwrap();
                            taps += 1;
                            let k = p.host().kernel();
                            let text: String = k
                                .node(id(&p, "taps"))
                                .unwrap()
                                .text_runs()
                                .iter()
                                .map(|r| &*r.text)
                                .collect();
                            assert_eq!(text, format!("Taps {taps}"));
                        }
                    }
                }
                p.tap(id(&p, "next")).unwrap();
            }
        }
    }
}

fn route_header_padding_is_the_containing_width_of_a_windowed_list() {
    let rows = (0..100)
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let mut outcomes = Vec::new();
    for padding in ["", "padding=\"6px 16px\""] {
        let source = format!(
            r#"component App
  derive rows = [{rows}]
  view
    column navigationKey="route" width=320
      header {padding}
        list virtualized=true height=100 width="100%" box-sizing="border-box" padding-left="10%" padding-right="10%" testId="feed"
          for item in rows key=item
            row height=24 testId=`row-${{item}}`
"#
        );
        let plan = contract::compile(&source).unwrap();
        let (mut p, error) = Presenter::boot_with(
            &plan.encode(),
            (),
            (320.0, 300.0),
            1.0,
            std::path::PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        // Read under its own roles: the test's other sessions live too.
        let _roles = p.host().runner().bind_roles();
        assert!(error.is_none(), "{error:?}");
        let feed = id(&p, "feed");
        for end in [false, true] {
            if end {
                p.wheel(feed, 0.0, 100_000.0).unwrap();
            }
            for _ in 0..16 {
                assert!(p.pump(p.host().now()).is_none());
                if p.dirty() {
                    p.frame();
                }
                if !p.dirty() && !p.host().collection(feed).unwrap().pending {
                    break;
                }
            }
            let snapshot = p.host().collection(feed).unwrap();
            assert!(!snapshot.pending && !p.dirty());
            assert_eq!(snapshot.count, 100);
            assert!(snapshot.rows.len() < 100 && snapshot.rows.iter().all(|r| r.measured));
            assert_eq!(size(&mut p, "feed").0, 288.0);
            // The header's 320px box has 16px on each side: the list's
            // 10% padding is 28.8px, leaving 230.4px for its row wrappers.
            let kernel = p.host().kernel();
            let row = kernel.node(snapshot.rows[0].view).unwrap();
            let port = kernel.node(feed).unwrap();
            assert!((row.frame.x - port.frame.x - 28.8).abs() < 0.01);
            assert!((row.frame.width - 230.4).abs() < 0.01);
            if end {
                let offset = p.scroll_of(feed).1;
                assert!(offset > 2_000.0);
                outcomes.push(offset);
            }
        }
    }
    assert_eq!(
        outcomes[0], outcomes[1],
        "unsaid and authored header padding report the same list origin and end"
    );
}

fn disabled_glass_and_invalid_fields_follow_the_neutral_look() {
    let mut failures = Vec::new();
    for scheme in ["light", "dark"] {
        for style in [
            "glass",
            "clear-glass",
            "prominent-glass",
            "prominent-clear-glass",
        ] {
            let material = if style.contains("clear") {
                "light-dark(#ffffff26, #ffffff1a)"
            } else {
                "light-dark(#ffffffb3, #262629b3)"
            };
            let source = format!(
                r##"component App
  state disabled = false
  action toggle
    disabled = !disabled
  view
    column width=300 height=180 padding=20 gap=10 align-items="flex-start" color-scheme="{scheme}" background-color="#204060"
      button "MMMM" buttonStyle="{style}" disabled=disabled accent-color="#008800" width=160 height=40 testId="glass"
      row width=160 height=40 background-color="{material}" testId="material"
      row width=10 height=10 background-color="GrayText" testId="gray"
      button "Toggle" press=toggle testId="toggle"
"##
            );
            let plan = contract::compile(&source).unwrap();
            let (mut p, error) = Presenter::boot_with(
                &plan.encode(),
                (),
                (300.0, 180.0),
                1.0,
                std::path::PathBuf::new(),
                PainterChoice::Cpu,
            )
            .unwrap();
            let _roles = p.host().runner().bind_roles();
            assert!(error.is_none(), "{error:?}");
            let enabled = text_pixels(&mut p, "glass");
            if style.starts_with("prominent") {
                assert_eq!(pixel(&mut p, "glass", 8.0, 20.0), [0, 136, 0]);
            }
            p.tap(id(&p, "toggle")).unwrap();
            let actual = pixel(&mut p, "glass", 8.0, 20.0);
            let expected = pixel(&mut p, "material", 8.0, 20.0);
            if actual != expected {
                failures.push(format!(
                    "{scheme} disabled {style} fill: {actual:?} != {expected:?}"
                ));
            }
            let gray = pixel(&mut p, "gray", 5.0, 5.0);
            if !text_pixels(&mut p, "glass").contains(&gray) {
                failures.push(format!(
                    "{scheme} disabled {style} has no GrayText ink {gray:?}"
                ));
            }
            p.tap(id(&p, "toggle")).unwrap();
            assert_eq!(
                text_pixels(&mut p, "glass"),
                enabled,
                "{scheme} {style} re-enabled"
            );
        }
        for tag in ["input", "textarea"] {
            for (chrome, expected_invalid) in [
                ("", "-exact-system-red"),
                ("appearance=\"none\"", "#008800"),
                ("outline=\"none\"", "-exact-secondary-grouped-background"),
            ] {
                let source = format!(
                    r##"component App
  state invalid = false
  action change
    invalid = !invalid
    focus("field")
  action focusField
    focus("field")
  view
    column width=300 height=200 padding=20 gap=10 align-items="flex-start" color-scheme="{scheme}" background-color="#204060"
      {tag} id="field" testId="field" value="Ada" width=160 height=40 accent-color="#008800" aria-invalid=(invalid ? "true" : "false") {chrome}
      row width=20 height=20 background-color="{expected_invalid}" testId="reference"
      button "Focus" press=focusField testId="focus"
      button "Change" press=change testId="change"
"##
                );
                let plan = contract::compile(&source).unwrap();
                let (mut p, error) = Presenter::boot_with(
                    &plan.encode(),
                    (),
                    (300.0, 200.0),
                    1.0,
                    std::path::PathBuf::new(),
                    PainterChoice::Cpu,
                )
                .unwrap();
                let _roles = p.host().runner().bind_roles();
                assert!(error.is_none(), "{error:?}");
                p.tap(id(&p, "focus")).unwrap();
                p.run_commands(<()>::default);
                // The border is one pixel; the focus ring occupies the next pixel too.
                let valid = pixel(&mut p, "field", 80.0, 1.0);
                if !chrome.starts_with("outline") {
                    assert_eq!(valid, [0, 136, 0], "{scheme} {tag} {chrome}");
                }
                let outline = pixel(&mut p, "field", 80.0, -1.0);
                p.tap(id(&p, "change")).unwrap();
                p.run_commands(<()>::default);
                let actual = pixel(&mut p, "field", 80.0, 1.0);
                let expected = pixel(&mut p, "reference", 10.0, 10.0);
                if actual != expected {
                    failures.push(format!(
                        "{scheme} invalid {tag} {chrome} ring: {actual:?} != {expected:?}"
                    ));
                }
                if chrome.starts_with("outline") {
                    assert_eq!(
                        pixel(&mut p, "field", 80.0, -1.0),
                        outline,
                        "authored outline"
                    );
                }
                p.tap(id(&p, "change")).unwrap();
                p.run_commands(<()>::default);
                assert_eq!(pixel(&mut p, "field", 80.0, 1.0), valid, "valid again");
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn invalid_non_fields_keep_their_accent_ring() {
    let mut failures = Vec::new();
    for scheme in ["light", "dark"] {
        for (tag, chrome) in [
            ("link", ""),
            ("link", "appearance=\"none\""),
            ("button", "appearance=\"auto\""),
            ("button", "appearance=\"none\""),
        ] {
            let source = format!(
                r##"component App
  state invalid = false
  action change
    invalid = !invalid
  view
    column width=300 height=240 padding=20 gap=10 align-items="flex-start" color-scheme="{scheme}"
      button "Before" testId="before"
      {tag} testId="control" tabindex=0 aria-invalid=(invalid ? "true" : "false") accent-color="#008800" width=160 height=40 {chrome}
        text "Control"
      button "Change validity" press=change testId="change"
"##
            );
            let plan = contract::compile(&source).unwrap();
            let (mut p, error) = Presenter::boot_with(
                &plan.encode(),
                (),
                (300.0, 240.0),
                1.0,
                std::path::PathBuf::new(),
                PainterChoice::Cpu,
            )
            .unwrap();
            let _roles = p.host().runner().bind_roles();
            assert!(error.is_none(), "{error:?}");
            for invalid in [false, true, false] {
                let before = id(&p, "before");
                p.type_key(before, "Tab", "Tab", true, false).unwrap();
                p.type_key(before, "Tab", "Tab", false, false).unwrap();
                assert_eq!(p.focus(), Some(id(&p, "control")));
                let ring = pixel(&mut p, "control", 80.0, 1.0);
                if ring != [0, 136, 0] {
                    failures.push(format!("{scheme} {tag} {chrome}, invalid={invalid}: ring {ring:?} is not the authored green accent"));
                }
                p.tap(id(&p, "change")).unwrap();
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
