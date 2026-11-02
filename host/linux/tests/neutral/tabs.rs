use super::*;

// The browser oracle uses this exact source in each ambient/authored scheme.
const TAB_BARS: &str = r##"style Own
  background-color="#abcdef"
  border-top="2px solid #654321"
style Plain
  opacity=1
component App
  state picked = 0
  state own = false
  state taps = 0
  state gate = 0
  action pick(n: number)
    picked = n
    taps = taps + 1
  action cycle
    gate = (gate + 1) % 4
  action toggle
    own = !own
  view
    column width=320 gap=4 color-scheme="SCHEME" color="#123456" background-color="#fedcba" accent-color="#0055cc"
      row role="tablist" height=40 box-sizing="border-box" testId="tabs"
        button role="tab" aria-controls="panel-a" aria-selected=(picked == 0) press=pick(0) width=64 height=32 min-height=0 padding=0 font-size=16 testId="tab-a"
          text "MMMM"
        button role="tab" aria-controls="panel-b" aria-selected=(picked == 1) press=pick(1) width=64 height=32 min-height=0 padding=0 font-size=16 testId="tab-b"
          text "MMMM"
        button role="tab" aria-controls="panel-c" disabled=true press=pick(2) width=64 height=32 min-height=0 padding=0 font-size=16 testId="disabled"
          text "MMMM"
        button role="tab" aria-controls="panel-c" buttonStyle="gray" width=64 height=32 min-height=0 padding=0 font-size=16 testId="gray"
          text "MMMM"
      row role="tablist" height=40 box-sizing="border-box" testId="references"
        button role="tab" aria-controls="panel-a" width=64 height=32 min-height=0 padding=0 font-size=16 testId="ref-a"
          text "MMMM" color=(picked == 0 ? "#0055cc" : "-exact-secondary-label")
        button role="tab" aria-controls="panel-b" width=64 height=32 min-height=0 padding=0 font-size=16 testId="ref-b"
          text "MMMM" color=(picked == 1 ? "#0055cc" : "-exact-secondary-label")
        button role="tab" aria-controls="panel-c" disabled=true width=64 height=32 min-height=0 padding=0 font-size=16 testId="ref-disabled"
          text "MMMM" color="light-dark(#707070, #929292)"
        button role="tab" aria-controls="panel-c" buttonStyle="gray" width=64 height=32 min-height=0 padding=0 font-size=16 testId="ref-gray"
          text "MMMM" color="-exact-secondary-label"
        button role="tab" aria-controls="panel-a" width=64 height=32 min-height=0 padding=0 font-size=16 testId="ref-hover"
          text "MMMM" color="-exact-label"
      column role="tabpanel" id="panel-a" display=(picked == 0 ? "block" : "none")
        text "Panel A" testId="panel-a-text"
      column role="tabpanel" id="panel-b" display=(picked == 1 ? "block" : "none")
        text "Panel B" testId="panel-b-text"
      column role="tabpanel" id="panel-c" display=(picked == 2 ? "block" : "none")
        text "Panel C" testId="panel-c-text"
      text `Taps ${taps}` testId="taps"
      row role="tablist" class=(own ? Own : Plain) height=40 box-sizing="border-box" testId="own-bar"
        button role="tab" aria-controls="missing-panel" aria-selected=true width=64 height=32 min-height=0 padding=0 font-size=16 testId="own-ink" color="#123456"
          text "MMMM"
        button role="tab" aria-controls="missing-panel" appearance="none" width=64 height=32 box-sizing="border-box" border="2px solid #123456" background-color="#abcdef" testId="custom"
          text "Own"
        button role="tab" aria-controls="missing-panel" buttonStyle="filled" width=64 height=32 min-height=0 padding=0 testId="filled"
          text "Fill"
      row role=(gate == 1 ? "group" : "tablist") height=12 box-sizing="border-box" testId="broad"
        if gate == 3
          text "" role="tab" width=1 height=1
        else
          text "" role=(gate == 2 ? "button" : "tab") aria-controls="" width=1 height=1
      row role="tablist" height=12 box-sizing="border-box" testId="indirect"
        column
          text "" role="tab" aria-controls="missing-panel" width=1 height=1
      row role="tablist" height=12 box-sizing="border-box" border-top="none" testId="no-edge"
        text "" role="tab" aria-controls="" width=1 height=1
      row height=12 box-sizing="border-box" background-color="-exact-background" border-top="1px solid -exact-separator" testId="surface-reference"
      row
        button press=toggle width=80 height=24 testId="toggle"
          text "Own / Plain"
        text (own ? "Own" : "Plain") testId="mode"
      row
        button press=cycle width=80 height=24 testId="cycle"
          text "Cycle role"
        text `Gate ${gate}` testId="gate"
      row
        button width=64 height=32 padding=0 font-size=16 testId="outside"
          text "MMMM"
"##;

// Fixed authored boxes isolate narrow tab paint from the deferred tab metrics.
pub(super) fn narrow_tab_bars_follow_the_neutral_paint_rules() {
    use exact_kernel::StyleId;
    for (scheme, ground) in [("light", [255; 3]), ("dark", [18; 3])] {
        let plan = contract::compile(&TAB_BARS.replace("SCHEME", scheme)).unwrap();
        let (mut p, error) = Presenter::boot_with(
            &plan.encode(),
            (),
            (320.0, 640.0),
            1.0,
            std::path::PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        // Read under its own roles: the test's other sessions live too.
        let _roles = p.host().runner().bind_roles();
        assert!(error.is_none(), "{error:?}");
        assert!(p
            .set_preferences(exact_runner::Preferences {
                dark: scheme == "dark",
                ..Default::default()
            })
            .is_none());
        p.set_system_scheme(scheme == "dark");
        let outside = text_pixels(&mut p, "outside");
        for name in ["tabs", "references", "own-bar", "broad"] {
            assert_eq!(pixel(&mut p, name, 318.0, 4.0), ground, "{name} {scheme}");
            assert_eq!(
                pixel(&mut p, name, 318.0, 0.0),
                pixel(&mut p, "surface-reference", 318.0, 0.0)
            );
        }
        assert_eq!(size(&mut p, "tabs"), (320.0, 40.0));
        assert_eq!(pixel(&mut p, "indirect", 318.0, 4.0), [0xfe, 0xdc, 0xba]);
        assert_eq!(pixel(&mut p, "no-edge", 318.0, 0.0), ground);
        for (target, reference) in [
            ("tab-a", "ref-a"),
            ("tab-b", "ref-b"),
            ("disabled", "ref-disabled"),
            ("gray", "ref-gray"),
        ] {
            assert_eq!(size(&mut p, target), (64.0, 32.0));
            assert_eq!(
                text_pixels(&mut p, target),
                text_pixels(&mut p, reference),
                "{target} ink {scheme}"
            );
        }
        for name in ["tab-a", "tab-b", "disabled"] {
            assert_eq!(pixel(&mut p, name, 0.0, 16.0), ground, "no ordinary edge");
        }
        assert_eq!(
            pixel(&mut p, "gray", 32.0, 2.0),
            ground,
            "gray tabs have no fill"
        );
        assert!(text_pixels(&mut p, "own-ink").contains(&[0x12, 0x34, 0x56]));
        assert_eq!(pixel(&mut p, "custom", 0.0, 16.0), [0x12, 0x34, 0x56]);
        assert_eq!(pixel(&mut p, "custom", 60.0, 16.0), [0xab, 0xcd, 0xef]);
        assert_eq!(pixel(&mut p, "filled", 4.0, 16.0), [0, 0x55, 0xcc]);
        let k = p.host().kernel();
        for name in ["tabs", "references", "own-bar", "broad"] {
            let node = k.node(id(&p, name)).unwrap();
            assert_eq!(
                node.computed_row(StyleId::BackgroundColor, |s| s.background_color),
                Some(exact_kernel::ColorValue::Role(
                    exact_kernel::style::roles::role("-exact-background").unwrap()
                ))
            );
            for row in [
                StyleId::BackgroundColor,
                StyleId::BorderWidthTop,
                StyleId::BorderStyleTop,
                StyleId::BorderColorTop,
            ] {
                assert!(!node.style.mask.has(row) && !node.style.written.has(row));
            }
        }
        assert!(k
            .node(id(&p, "own-ink"))
            .unwrap()
            .style
            .written
            .has(StyleId::TextColor));
        assert!(k
            .node(id(&p, "no-edge"))
            .unwrap()
            .style
            .written
            .has(StyleId::BorderStyleTop));
        p.hover(id(&p, "tab-b")).unwrap();
        // Hide only the software cursor; keep the real hover and its paint state.
        p.set_pointer(None);
        assert_eq!(
            text_pixels(&mut p, "tab-b"),
            text_pixels(&mut p, "ref-hover")
        );
        assert_eq!(
            pixel(&mut p, "tab-b", 4.0, 16.0),
            ground,
            "no tab hover wash"
        );
        p.hover(id(&p, "tab-a")).unwrap();
        p.set_pointer(None);
        assert_eq!(text_pixels(&mut p, "tab-a"), text_pixels(&mut p, "ref-a"));
        let (x, y, w, h) = p.rect_of(id(&p, "disabled")).unwrap();
        p.pointer_down(x + w / 2.0, y + h / 2.0, 0.0).unwrap();
        p.pointer_up(x + w / 2.0, y + h / 2.0, 1.0).unwrap();
        assert_eq!(
            p.host().kernel().bar_text(id(&p, "taps")).as_deref(),
            Some("Taps 0")
        );
        p.tap(id(&p, "tab-b")).unwrap();
        p.hover(id(&p, "tabs")).unwrap();
        p.set_pointer(None);
        assert_eq!(
            p.host().kernel().bar_text(id(&p, "taps")).as_deref(),
            Some("Taps 1")
        );
        assert_eq!(p.focus(), Some(id(&p, "tab-b")));
        // A press's focus shows no ring; a key then shows it.
        assert_ne!(pixel(&mut p, "tab-b", 32.0, 0.0), [0, 0x55, 0xcc]);
        let tab = id(&p, "tab-b");
        p.type_key(tab, "KeyX", "x", true, false).unwrap();
        p.type_key(tab, "KeyX", "x", false, false).unwrap();
        assert_eq!(pixel(&mut p, "tab-b", 32.0, 0.0), [0, 0x55, 0xcc]);
        // Compare the selected face after observing and releasing its focus ring.
        p.blur();
        assert!(p.focus().is_none());
        for (target, reference) in [("tab-a", "ref-a"), ("tab-b", "ref-b")] {
            assert_eq!(
                text_pixels(&mut p, target),
                text_pixels(&mut p, reference),
                "selected ink changes"
            );
        }
        for own in [true, false] {
            p.tap(id(&p, "toggle")).unwrap();
            assert_eq!(size(&mut p, "own-bar"), (320.0, 40.0));
            assert_eq!(
                pixel(&mut p, "own-bar", 318.0, 4.0),
                if own { [0xab, 0xcd, 0xef] } else { ground }
            );
            let edge = if own {
                [0x65, 0x43, 0x21]
            } else {
                pixel(&mut p, "surface-reference", 318.0, 0.0)
            };
            assert_eq!(pixel(&mut p, "own-bar", 318.0, 0.0), edge);
            let k = p.host().kernel();
            let bar = k.node(id(&p, "own-bar")).unwrap();
            for row in [
                StyleId::BackgroundColor,
                StyleId::BorderWidthTop,
                StyleId::BorderStyleTop,
                StyleId::BorderColorTop,
            ] {
                assert_eq!(bar.style.mask.has(row), own);
                assert_eq!(bar.style.written.has(row), own);
            }
        }
        // The CSS surface rule does not require a native button, a real
        // target, or a navigation projection. Prop removal must restore it.
        for gate in [1, 2, 3, 0] {
            p.tap(id(&p, "cycle")).unwrap();
            let expected = if gate == 0 {
                ground
            } else {
                [0xfe, 0xdc, 0xba]
            };
            assert_eq!(pixel(&mut p, "broad", 318.0, 4.0), expected);
            let node = p.host().kernel();
            let bar = node.node(id(&p, "broad")).unwrap();
            assert!(!bar.style.mask.has(StyleId::BackgroundColor));
            assert!(!bar.style.written.has(StyleId::BackgroundColor));
        }
        let broad = id(&p, "broad");
        let child = p.host().kernel().node(broad).unwrap().children()[0];
        for present in [false, true] {
            use exact_kernel::{Op, PropId, PropValue};
            let op = if present {
                Op::SetProp {
                    id: child,
                    prop: PropId::AccessibilityControls,
                    value: PropValue::Str("".into()),
                }
            } else {
                Op::ClearProp {
                    id: child,
                    prop: PropId::AccessibilityControls,
                }
            };
            let k = p.host_mut().runner_mut().kernel_mut();
            let key = k.node(broad).unwrap().key;
            let receipt = k.apply(0, 1, &[op]).unwrap();
            assert!(receipt.touched.contains(&key));
            let bar = k.node(broad).unwrap();
            assert_eq!(
                bar.computed_row(StyleId::BackgroundColor, |s| s
                    .background_color
                    .map_or(0, |color| color.resolve(scheme == "dark").a())),
                if present { 255 } else { 0 }
            );
            assert!(!bar.style.mask.has(StyleId::BackgroundColor));
            assert!(!bar.style.written.has(StyleId::BackgroundColor));
        }
        // A retained qualifying child changes both old and new surfaces.
        // The nested qualifying grandchild alone does not style its tablist.
        let indirect = id(&p, "indirect");
        let k = p.host_mut().runner_mut().kernel_mut();
        let wrapper = k.node(indirect).unwrap().children()[0];
        for (parent, children, broad_on, indirect_on, changed) in [
            (
                indirect,
                vec![wrapper, child],
                false,
                true,
                vec![broad, indirect],
            ),
            (indirect, vec![wrapper], false, false, vec![indirect]),
            (broad, vec![child], true, false, vec![broad]),
        ] {
            let receipt = k
                .apply(
                    0,
                    1,
                    &[exact_kernel::Op::SetChildren {
                        id: parent,
                        children,
                    }],
                )
                .unwrap();
            for changed in changed {
                assert!(receipt.touched.contains(&k.node(changed).unwrap().key));
            }
            for (surface, present) in [(broad, broad_on), (indirect, indirect_on)] {
                let bar = k.node(surface).unwrap();
                assert_eq!(
                    bar.computed_row(StyleId::BackgroundColor, |s| s
                        .background_color
                        .map_or(0, |color| color.resolve(scheme == "dark").a())),
                    if present { 255 } else { 0 }
                );
                assert_eq!(
                    bar.computed_row(StyleId::BorderWidthTop, |s| s.border_width_top),
                    if present {
                        1.0
                    } else {
                        exact_kernel::StyleProps::default().border_width_top
                    }
                );
                assert_eq!(
                    bar.computed_row(StyleId::BorderStyleTop, |s| s.border_style_top),
                    if present {
                        exact_kernel::BorderStyle::Solid
                    } else {
                        exact_kernel::BorderStyle::None
                    }
                );
                for row in [
                    StyleId::BackgroundColor,
                    StyleId::BorderWidthTop,
                    StyleId::BorderStyleTop,
                    StyleId::BorderColorTop,
                ] {
                    assert!(!bar.style.mask.has(row) && !bar.style.written.has(row));
                }
            }
        }
        let clear_rows = exact_kernel::StyleMask::of(StyleId::BackgroundColor)
            .union(exact_kernel::StyleMask::of(StyleId::BorderStyleTop));
        for written in [true, false] {
            k.apply(
                0,
                1,
                &[exact_kernel::Op::ClearStyle {
                    id: broad,
                    mask: clear_rows,
                    written,
                }],
            )
            .unwrap();
            let bar = k.node(broad).unwrap();
            assert_eq!(
                bar.computed_row(StyleId::BackgroundColor, |s| s
                    .background_color
                    .map_or(0, |color| color.resolve(scheme == "dark").a())),
                if written { 0 } else { 255 }
            );
            assert_eq!(
                bar.computed_row(StyleId::BorderStyleTop, |s| s.border_style_top),
                if written {
                    exact_kernel::BorderStyle::None
                } else {
                    exact_kernel::BorderStyle::Solid
                }
            );
            for row in [StyleId::BackgroundColor, StyleId::BorderStyleTop] {
                assert!(!bar.style.mask.has(row));
                assert_eq!(bar.style.written.has(row), written);
            }
        }
        let receipt = k
            .apply(0, 1, &[exact_kernel::Op::DestroyView { id: child }])
            .unwrap();
        let bar = k.node(broad).unwrap();
        assert!(receipt.touched.contains(&bar.key));
        assert_eq!(
            bar.computed_row(StyleId::BorderStyleTop, |s| s.border_style_top),
            exact_kernel::BorderStyle::None
        );
        assert!(bar.children().is_empty());
        assert_eq!(
            text_pixels(&mut p, "outside"),
            outside,
            "ordinary controls are unchanged"
        );
    }
    route_header_tab_bar_defaults_compose();
}

fn route_header_tab_bar_defaults_compose() {
    use exact_kernel::{BorderStyle, StyleId};
    let plan = contract::compile(
        r##"component App
  view
    column navigationKey="route" width=320
      header role="tablist" height=40 min-height=0 padding=0 testId="both"
        text "" role="tab" aria-controls="" width=1 height=1
"##,
    )
    .unwrap();
    let (mut p, error) = Presenter::boot_with(
        &plan.encode(),
        (),
        (320.0, 100.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    // Read under its own roles: the test's other sessions live too.
    let _roles = p.host().runner().bind_roles();
    assert!(error.is_none(), "{error:?}");
    assert_eq!(size(&mut p, "both"), (320.0, 42.0));
    let node = p.host().kernel().node(id(&p, "both")).unwrap();
    assert_eq!(
        node.computed_row(StyleId::BorderStyleTop, |s| s.border_style_top),
        BorderStyle::Solid
    );
    assert_eq!(
        node.computed_row(StyleId::BorderStyleBottom, |s| s.border_style_bottom),
        BorderStyle::Solid
    );
    for row in [StyleId::BorderWidthTop, StyleId::BorderWidthBottom] {
        assert!(!node.style.mask.has(row) && !node.style.written.has(row));
    }
}

// Measured in Chrome 155 with matching ambient/authored light and dark schemes.
const TAB_GEOMETRY: &str = r##"style OwnGeometry
  width=80
  height=30
  min-height=0
  padding=2
  font-size=18
style Plain
  opacity=1
component App
  state own = false
  state gate = 0
  state taps = 0
  state rootSize = "medium"
  action pressTab
    taps = taps + 1
  action toggle
    own = !own
  action cycle
    gate = (gate + 1) % 4
  action larger
    setRootFontSize("125%")
    rootSize = "125%"
  action reset
    setRootFontSize("medium")
    rootSize = "medium"
  view
    column width="100%" gap=8 color-scheme="SCHEME" background-color="-exact-background" testId="page"
      row role="tablist" testId="basic"
        button role="tab" aria-controls="missing" press=pressTab flex=1 testId="leading"
          image "symbol:book" testId="leading-image"
          text "Library" testId="leading-title"
        button role="tab" aria-controls="" buttonStyle="gray" flex=1 testId="title-only"
          text "Reading" testId="title-only-title"
        button role="tab" aria-controls="missing" aria-label="Library" flex=1 testId="image-only"
          image "symbol:book" testId="image-only-image"
      row role="tablist" align-items="flex-start" testId="natural"
        button role="tab" aria-controls="missing" testId="natural-leading"
          image "symbol:book" testId="natural-leading-image"
          text "Library" testId="natural-leading-title"
        button role="tab" aria-controls="missing" testId="trailing"
          text "Library" testId="trailing-title"
          image "symbol:book" testId="trailing-image"
        button role="tab" aria-controls="missing" testId="tiny"
          text "I" testId="tiny-title"
      row role="tablist" align-items="flex-start" testId="sizing"
        button role="tab" aria-controls="missing" width=80 height=24 testId="size-only"
          text "Size" testId="size-only-title"
        button role="tab" aria-controls="missing" width=80 height=24 min-height=0 testId="size-min"
          text "Size" testId="size-min-title"
        button role="tab" aria-controls="missing" width=80 height=24 min-height=0 padding=0 testId="size-padding"
          text "Size" testId="size-padding-title"
      row role="tablist" align-items="flex-start" testId="font-local"
        button role="tab" aria-controls="missing" font-size=20 testId="local20"
          image "symbol:book" testId="local20-image"
          text "Library" testId="local20-title"
        button role="tab" aria-controls="missing" testId="child20"
          image "symbol:book" testId="child20-image"
          text "Library" font-size=20 testId="child20-title"
      row role="tablist" align-items="flex-start" testId="overrides"
        button role="tab" aria-controls="missing" class=(own ? OwnGeometry : Plain) testId="class-tab"
          image "symbol:book" testId="class-image"
          text "Library" testId="class-title"
        button role="tab" aria-controls="missing" flex-direction="row" gap=5 testId="own-row"
          image "symbol:book" testId="own-row-image"
          text "Library" testId="own-row-title"
        button role="tab" aria-controls="missing" flex-direction="column" gap=5 testId="own-column"
          image "symbol:book" testId="own-column-image"
          text "Library" testId="own-column-title"
      row role="tablist" align-items="flex-start" testId="customs"
        button role="tab" aria-controls="missing" appearance="none" testId="custom"
          text "Custom"
        button role="tab" aria-controls="missing" buttonStyle="filled" testId="filled"
          image "symbol:book" testId="filled-image"
          text "Filled" testId="filled-title"
        column
          button role="tab" aria-controls="missing" testId="wrapped"
            image "symbol:book" testId="wrapped-image"
            text "Wrapped" testId="wrapped-title"
      row role=(gate == 1 ? "group" : "tablist") align-items="flex-start" testId="membership"
        if gate == 3
          button role="tab" testId="member-absent"
            image "symbol:book" testId="member-absent-image"
            text "Member" testId="member-absent-title"
        else
          button role=(gate == 2 ? "button" : "tab") aria-controls="" testId="member"
            image "symbol:book" testId="member-image"
            text "Member" testId="member-title"
      row align-items="flex-start" testId="outside-row"
        button testId="outside"
          image "symbol:book" testId="outside-image"
          text "Outside" testId="outside-title"
      text `Taps ${taps}` testId="taps"
      text (own ? "Own" : "Plain") testId="mode"
      text `Gate ${gate}` testId="gate"
      text rootSize testId="root-size"
      row flex-wrap="wrap" gap=4
        button press=toggle testId="toggle"
          text "Own / Plain"
        button press=cycle testId="cycle"
          text "Cycle membership"
        button press=larger testId="larger"
          text "Root 125%"
        button press=reset testId="reset"
          text "Root medium"
"##;

fn close_tab_length(actual: f32, expected: f32, name: &str) {
    // Chrome allocates flex lengths in 1/64px units; this is not a pixel tolerance.
    assert!(
        (actual - expected).abs() < 0.02,
        "{name}: {actual} != {expected}"
    );
}

fn tab_action(p: &mut Presenter<()>, name: &str) {
    p.tap(id(p, name)).unwrap();
    p.blur();
    p.set_pointer(None);
}

// Glyphs may differ between font engines. Require positive ink in each measured
// image/title box and none outside their union, allowing only pixel/box intersection.
fn tab_ink_boxes(
    p: &mut Presenter<()>,
    name: &str,
    ground: [u8; 3],
    regions: &[(f32, f32, f32, f32)],
) {
    let (left, top, width, height) = p.rect_of(id(p, name)).unwrap();
    let shot = p.frame();
    let mut counts = vec![0; regions.len()];
    for y in top.ceil() as u32..(top + height).floor() as u32 {
        for x in left.ceil() as u32..(left + width).floor() as u32 {
            let c = shot.pixel(x, y).unwrap();
            if [c.red(), c.green(), c.blue()] == ground {
                continue;
            }
            let mut inside = false;
            for (i, &(rx, ry, rw, rh)) in regions.iter().enumerate() {
                if (x as f32) < left + rx + rw
                    && x as f32 + 1.0 > left + rx
                    && (y as f32) < top + ry + rh
                    && y as f32 + 1.0 > top + ry
                {
                    counts[i] += 1;
                    inside = true;
                }
            }
            assert!(inside, "{name}: ink outside measured face boxes at {x},{y}");
        }
    }
    assert!(
        counts.iter().all(|n| *n > 0),
        "{name}: missing face ink {counts:?}"
    );
}

fn tab_geometry_at_root(p: &mut Presenter<()>, width: f32, root: f32, ground: [u8; 3]) {
    let (height, symbol, line, padding) = if root == 16.0 {
        (56.0, 22.0, 15.0, 8.0)
    } else {
        (70.0, 27.5, 18.75, 10.0)
    };
    assert_eq!(p.host().kernel().root_font_size(), root);
    assert_eq!(size(p, "basic"), (width, height + 1.0));
    let mut shared = 0.0;
    for name in ["leading", "title-only", "image-only"] {
        let (w, h) = size(p, name);
        close_tab_length(w, width / 3.0, name);
        assert_eq!(h, height, "{name}");
        shared += w;
    }
    close_tab_length(shared, width, "authored flex shares available width");
    for name in [
        "natural-leading",
        "trailing",
        "own-row",
        "own-column",
        "class-tab",
        "member",
    ] {
        assert_eq!(size(p, name).1, height, "{name}");
    }
    close_tab_length(
        size(p, "tiny").0,
        if root == 16.0 { 27.0 } else { 33.75 },
        "tiny minimum",
    );
    assert_eq!(size(p, "size-only"), (80.0, height));
    assert_eq!(size(p, "size-min"), (80.0, 24.0));
    assert_eq!(size(p, "size-padding"), (80.0, 24.0));
    for name in ["size-min", "size-padding"] {
        tab_ink_boxes(p, name, ground, &[(0.0, (24.0 - line) / 2.0, 80.0, line)]);
    }
    for name in ["local20", "child20"] {
        assert_eq!(
            size(p, name).1,
            if root == 16.0 { 61.0 } else { 70.0 },
            "{name}"
        );
        let rows = p.host().kernel().button_face_style(id(p, name)).unwrap();
        assert_eq!(rows.title.font_size, 20.0);
        assert!(rows.title.mask.has(exact_kernel::StyleId::FontSize));
    }
    // These differences cancel the common title's host-specific measured width.
    let leading = size(p, "natural-leading").0;
    let trailing = size(p, "trailing").0;
    close_tab_length(
        trailing - leading,
        symbol,
        "trailing image stays beside the title",
    );
    close_tab_length(size(p, "own-row").0 - trailing, 5.0, "authored row gap");
    close_tab_length(size(p, "own-column").0, leading, "authored column width");
    for name in ["filled", "wrapped", "outside"] {
        assert_eq!(
            size(p, name).1,
            if root == 16.0 { 36.0 } else { 45.0 },
            "{name}"
        );
    }
    for (name, gap, title_line) in [
        ("leading", 2.0, line),
        ("natural-leading", 2.0, line),
        ("own-column", 5.0, line),
        ("local20", 2.0, 25.0),
        ("child20", 2.0, 25.0),
    ] {
        let (w, h) = size(p, name);
        let y = (h - symbol - gap - title_line) / 2.0;
        tab_ink_boxes(
            p,
            name,
            ground,
            &[
                ((w - symbol) / 2.0, y, symbol, symbol),
                (padding, y + symbol + gap, w - 2.0 * padding, title_line),
            ],
        );
    }
    let (w, h) = size(p, "image-only");
    tab_ink_boxes(
        p,
        "image-only",
        ground,
        &[((w - symbol) / 2.0, (h - symbol) / 2.0, symbol, symbol)],
    );
    let (w, h) = size(p, "title-only");
    tab_ink_boxes(
        p,
        "title-only",
        ground,
        &[(padding, (h - line) / 2.0, w - padding * 2.0, line)],
    );
    let (w, h) = size(p, "trailing");
    tab_ink_boxes(
        p,
        "trailing",
        ground,
        &[
            (padding, (h - line) / 2.0, w - padding * 2.0 - symbol, line),
            (w - padding - symbol, (h - symbol) / 2.0, symbol, symbol),
        ],
    );
    let (w, h) = size(p, "own-row");
    tab_ink_boxes(
        p,
        "own-row",
        ground,
        &[
            (padding, (h - symbol) / 2.0, symbol, symbol),
            (
                padding + symbol + 5.0,
                (h - line) / 2.0,
                w - padding * 2.0 - symbol - 5.0,
                line,
            ),
        ],
    );
}

pub(super) fn narrow_tab_geometry_follows_root_metrics_and_authored_rows() {
    use exact_kernel::StyleId;
    for (scheme, ground) in [("light", [255; 3]), ("dark", [18; 3])] {
        let plan = contract::compile(&TAB_GEOMETRY.replace("SCHEME", scheme)).unwrap();
        for width in [320.0, 640.0] {
            let (mut p, error) = Presenter::boot_with(
                &plan.encode(),
                (),
                (width, 1000.0),
                1.0,
                std::path::PathBuf::new(),
                PainterChoice::Cpu,
            )
            .unwrap();
            // Read under its own roles: the test's other sessions live too.
            let _roles = p.host().runner().bind_roles();
            assert!(error.is_none(), "{error:?}");
            assert!(p
                .set_preferences(exact_runner::Preferences {
                    dark: scheme == "dark",
                    ..Default::default()
                })
                .is_none());
            p.set_system_scheme(scheme == "dark");
            tab_geometry_at_root(&mut p, width, 16.0, ground);
            let raw: Vec<_> = [
                "leading",
                "title-only",
                "image-only",
                "local20",
                "child20",
                "own-row",
                "own-column",
            ]
            .into_iter()
            .map(|name| {
                let node = p.host().kernel().node(id(&p, name)).unwrap();
                assert_eq!(
                    exact_kernel::ControlKind::of(node.node_type, node.props),
                    Some(exact_kernel::ControlKind::Button)
                );
                for row in [
                    StyleId::MinHeight,
                    StyleId::PaddingTop,
                    StyleId::PaddingRight,
                    StyleId::PaddingBottom,
                    StyleId::PaddingLeft,
                ] {
                    assert!(
                        !node.style.mask.has(row) && !node.style.written.has(row),
                        "{name}: {row:?}"
                    );
                }
                (name, node.style.mask, node.style.written)
            })
            .collect();
            let local_width = size(&mut p, "local20").0;
            tab_action(&mut p, "larger");
            assert_eq!(
                p.host().kernel().bar_text(id(&p, "root-size")).as_deref(),
                Some("125%")
            );
            tab_geometry_at_root(&mut p, width, 20.0, ground);
            close_tab_length(
                size(&mut p, "local20").0 - local_width,
                4.0,
                "root padding with fixed local font",
            );
            for (name, mask, written) in raw {
                let node = p.host().kernel().node(id(&p, name)).unwrap();
                assert_eq!(
                    (node.style.mask, node.style.written),
                    (mask, written),
                    "{name}"
                );
            }
            tab_action(&mut p, "reset");
            tab_geometry_at_root(&mut p, width, 16.0, ground);
            tab_action(&mut p, "leading");
            assert_eq!(
                p.host().kernel().bar_text(id(&p, "taps")).as_deref(),
                Some("Taps 1")
            );

            let original = size(&mut p, "class-tab");
            for own in [true, false] {
                tab_action(&mut p, "toggle");
                assert_eq!(
                    size(&mut p, "class-tab"),
                    if own { (80.0, 30.0) } else { original }
                );
                let node = p.host().kernel().node(id(&p, "class-tab")).unwrap();
                for row in [
                    StyleId::Width,
                    StyleId::Height,
                    StyleId::MinHeight,
                    StyleId::PaddingTop,
                    StyleId::PaddingRight,
                    StyleId::PaddingBottom,
                    StyleId::PaddingLeft,
                    StyleId::FontSize,
                ] {
                    assert_eq!(node.style.mask.has(row), own, "{row:?}");
                    assert_eq!(node.style.written.has(row), own, "{row:?}");
                }
                let rows = p.host().kernel().button_face_style(node.id).unwrap();
                if own {
                    assert_eq!(rows.title.font_size, 18.0);
                    assert_eq!(
                        [
                            rows.button.padding_top,
                            rows.button.padding_right,
                            rows.button.padding_bottom,
                            rows.button.padding_left
                        ],
                        [exact_kernel::Dimension::Points(2.0); 4]
                    );
                }
            }
            let member = id(&p, "member");
            for gate in [1, 2, 3, 0] {
                tab_action(&mut p, "cycle");
                assert_eq!(
                    p.host().kernel().bar_text(id(&p, "gate")).as_deref(),
                    Some(format!("Gate {gate}").as_str())
                );
                if gate == 3 {
                    // This is child replacement. Segmented geometry remains a separate slice.
                    assert!(p.host().kernel().find_by_test_id("member").is_empty());
                    assert!(!p
                        .host()
                        .kernel()
                        .node(id(&p, "member-absent"))
                        .unwrap()
                        .is_neutral_tab());
                } else {
                    if gate != 0 {
                        assert_eq!(id(&p, "member"), member);
                    }
                    assert_eq!(
                        size(&mut p, "member").1,
                        if gate == 0 { 56.0 } else { 36.0 }
                    );
                }
            }
            tab_retained_membership_changes(&mut p, width);
        }
    }
}

fn tab_retained_membership_changes(p: &mut Presenter<()>, width: f32) {
    use exact_kernel::{Op, PropId, PropValue};
    let member = id(p, "member");
    let parent = id(p, "membership");
    let outside = id(p, "outside-row");
    let (raw, key, siblings) = {
        let k = p.host().kernel();
        let node = k.node(member).unwrap();
        (
            (node.style.mask, node.style.written),
            node.key,
            k.node(outside).unwrap().children().to_vec(),
        )
    };
    for style in [Some("filled"), None] {
        let op = match style {
            Some(style) => Op::SetProp {
                id: member,
                prop: PropId::ButtonStyle,
                value: PropValue::Str(style.into()),
            },
            None => Op::ClearProp {
                id: member,
                prop: PropId::ButtonStyle,
            },
        };
        let receipt = p
            .host_mut()
            .runner_mut()
            .kernel_mut()
            .apply(0, 1, &[op])
            .unwrap();
        assert!(receipt.layout_invalidated && receipt.touched.contains(&key));
        assert!(p.resize(width, 1000.0).is_none());
        assert_eq!(
            size(p, "member").1,
            if style.is_some() { 36.0 } else { 56.0 }
        );
        let node = p.host().kernel().node(member).unwrap();
        assert_eq!((node.style.mask, node.style.written), raw);
    }
    for present in [false, true] {
        let op = if present {
            Op::SetProp {
                id: member,
                prop: PropId::AccessibilityControls,
                value: PropValue::Str("".into()),
            }
        } else {
            Op::ClearProp {
                id: member,
                prop: PropId::AccessibilityControls,
            }
        };
        let receipt = p
            .host_mut()
            .runner_mut()
            .kernel_mut()
            .apply(0, 1, &[op])
            .unwrap();
        assert!(receipt.layout_invalidated && receipt.touched.contains(&key));
        // Same-size host layout consumes the low-level mutation without changing the viewport.
        assert!(p.resize(width, 1000.0).is_none());
        let node = p.host().kernel().node(member).unwrap();
        assert_eq!(node.is_neutral_tab(), present);
        assert_eq!((node.style.mask, node.style.written), raw);
        if present {
            assert_eq!(size(p, "member").1, 56.0);
        }
    }
    for in_bar in [false, true] {
        let children = if in_bar {
            vec![member]
        } else {
            siblings.iter().copied().chain([member]).collect()
        };
        let receipt = p
            .host_mut()
            .runner_mut()
            .kernel_mut()
            .apply(
                0,
                1,
                &[Op::SetChildren {
                    id: if in_bar { parent } else { outside },
                    children,
                }],
            )
            .unwrap();
        assert!(receipt.layout_invalidated && receipt.touched.contains(&key));
        assert!(p.resize(width, 1000.0).is_none());
        assert_eq!(id(p, "member"), member);
        assert_eq!(size(p, "member").1, if in_bar { 56.0 } else { 36.0 });
        let node = p.host().kernel().node(member).unwrap();
        assert_eq!(node.is_neutral_tab(), in_bar);
        assert_eq!((node.style.mask, node.style.written), raw);
    }
    assert_eq!(
        p.host().kernel().node(outside).unwrap().children(),
        siblings.as_slice()
    );
}
