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
            pixel(&mut p, "gray", 4.0, 16.0),
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
        assert_eq!(
            p.host().kernel().bar_text(id(&p, "taps")).as_deref(),
            Some("Taps 1")
        );
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
                    if present { 1.0 } else { 0.0 }
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
        assert_eq!(
            text_pixels(&mut p, "outside"),
            outside,
            "ordinary controls are unchanged"
        );
    }
}
