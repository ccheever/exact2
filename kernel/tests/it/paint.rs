//! Paint motion's seam (LLP 1062): only a node whose `transition` or
//! `animation` names a paint property owns it in the engine; its targets are
//! computed colours resolved by the host's appearance, and an appearance
//! change re-targets them the way a browser's computed value changes.

use exact_kernel::motion::{motion_node, PaintMotion, PaintOwners};
use exact_kernel::{wire, Kernel, NodeType, Op, StyleId, StyleProps, StyleValue};
use exact_motion::{Engine, Property, TransitionProperty, Transitions, Value};

fn patch(rows: &[(StyleId, &str)]) -> Box<StyleProps> {
    let mut s = StyleProps::default();
    for (id, text) in rows {
        // An animation row names keyframes the runner resolves (LLP 1055 D5).
        if text.contains("@keyframes") {
            let row = crate::keyframed(text);
            match id {
                StyleId::ExitAnimation => s.exit_animation = row,
                _ => s.animation = row,
            }
            s.mask.set(*id);
            continue;
        }
        s.set_dynamic(*id, &StyleValue::Text((*text).into()))
            .unwrap();
    }
    Box::new(s)
}

fn rgba(r: u8, g: u8, b: u8, a: f64) -> Value {
    let unit = |c: u8| c as f64 / 255.0;
    Value::rgba(unit(r), unit(g), unit(b), a)
}

/// A root (1) with a painted child (2) and a plain one (3).
fn tree() -> (Kernel, exact_kernel::CommitReceipt) {
    let mut k = Kernel::with_monospace();
    let ops = [
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::CreateView {
            id: 2,
            node_type: NodeType::View,
        },
        Op::CreateView {
            id: 3,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: patch(&[(StyleId::TextColor, "#112233")]),
        },
        Op::SetStyle {
            id: 2,
            patch: patch(&[
                (StyleId::BackgroundColor, "light-dark(#ffffff, #000000)"),
                (StyleId::ShadowColor, "0 4px 12px #00000080"),
                (StyleId::ShadowOffset, "0 4px 12px #00000080"),
                (StyleId::ShadowRadius, "0 4px 12px #00000080"),
                (StyleId::ShadowOpacity, "0 4px 12px #00000080"),
                (
                    StyleId::Transition,
                    "background-color 200ms, color 1s, border-color 1s, box-shadow 320ms",
                ),
            ]),
        },
        Op::SetStyle {
            id: 3,
            patch: patch(&[(StyleId::BackgroundColor, "#ff0000")]),
        },
        Op::SetChildren {
            id: 1,
            children: vec![2, 3],
        },
        Op::AttachRoot { id: 1 },
    ];
    let receipt = k.apply(0, 1, &ops).unwrap();
    (k, receipt)
}

#[test]
fn native_appearance_reports_snap_first_then_transition_per_view() {
    let (k, receipt) = tree();
    let key = k.node(2).unwrap().key;
    let node = motion_node(key);
    let mut engine = Engine::new();
    let mut paint = PaintMotion::default();
    k.motion_sync(&receipt).apply(&mut engine).unwrap();
    paint.adopt(&k, receipt.created.iter().copied(), &mut engine);
    paint.set_scheme(&k, &mut engine, true, 0.0).unwrap();
    assert!(engine.quiescent(), "boot is corrected without motion");
    assert_eq!(
        engine.target(node, Property::BackgroundColor),
        Some(rgba(0, 0, 0, 1.0))
    );
    paint
        .set_view_scheme(&k, &mut engine, key, false, 0.0)
        .unwrap();
    assert!(engine.quiescent(), "a view's first report also snaps");
    assert!(!paint.dark(key));
    paint.set_scheme(&k, &mut engine, false, 0.0).unwrap();
    paint.set_scheme(&k, &mut engine, true, 0.0).unwrap();
    assert!(
        !paint.dark(key),
        "a session change preserves the view's appearance"
    );
    assert_eq!(
        engine.target(node, Property::BackgroundColor),
        Some(rgba(255, 255, 255, 1.0))
    );
    paint
        .set_view_scheme(&k, &mut engine, key, true, 0.0)
        .unwrap();
    engine.advance(0.1).unwrap();
    let shown = paint
        .shown(&engine, node, Property::BackgroundColor)
        .unwrap();
    assert!(
        shown.x > 0.0 && shown.x < 1.0,
        "rejoining the session transitions: {shown:?}"
    );
    assert!(paint
        .set_view_scheme(&k, &mut engine, key, true, 0.1)
        .is_none());
}

#[test]
fn a_destroyed_paint_owner_cannot_pass_appearance_to_a_reused_slot() {
    let (mut k, receipt) = tree();
    let key = k.node(2).unwrap().key;
    let mut engine = Engine::new();
    let mut paint = PaintMotion::default();
    k.motion_sync(&receipt).apply(&mut engine).unwrap();
    paint.sync(&k, &receipt, &mut engine);
    paint.set_scheme(&k, &mut engine, false, 0.0);
    paint.set_view_scheme(&k, &mut engine, key, true, 0.0);
    let receipt = k
        .apply(
            0,
            2,
            &[
                Op::DestroyView { id: 2 },
                Op::CreateView {
                    id: 2,
                    node_type: NodeType::View,
                },
                Op::SetStyle {
                    id: 2,
                    patch: patch(&[
                        (StyleId::BackgroundColor, "light-dark(#ffffff, #000000)"),
                        (StyleId::Transition, "background-color 1s linear"),
                    ]),
                },
                Op::SetChildren {
                    id: 1,
                    children: vec![2, 3],
                },
            ],
        )
        .unwrap();
    k.motion_sync(&receipt).apply(&mut engine).unwrap();
    paint.sync(&k, &receipt, &mut engine);
    let fresh = k.node(2).unwrap().key;
    assert_eq!(key.index, fresh.index);
    assert_ne!(key.generation, fresh.generation);
    assert!(!paint.dark(key), "destroyed appearance is forgotten");
    assert!(!paint.dark(fresh));
    assert!(!paint.owns(motion_node(key), Property::BackgroundColor));
    assert_eq!(
        engine.target(motion_node(fresh), Property::BackgroundColor),
        Some(rgba(255, 255, 255, 1.0))
    );
    assert!(paint
        .set_view_scheme(&k, &mut engine, key, true, 0.0)
        .is_none());
}

#[test]
fn only_named_paint_is_adopted_with_computed_resolved_targets() {
    let (k, receipt) = tree();
    let mut owners = PaintOwners::default();
    let sync = k.paint_sync(&receipt, false, &mut owners);
    let two = motion_node(k.node(2).unwrap().key);
    let got = |p: Property| {
        sync.changes
            .iter()
            .find(|c| c.node == two && c.property == p)
            .map(|c| c.value)
    };
    assert!(
        sync.changes.iter().all(|c| c.node == two),
        "3 names nothing"
    );
    assert_eq!(
        got(Property::BackgroundColor),
        Some(rgba(255, 255, 255, 1.0))
    );
    // `color` inherited from the root; a `currentcolor` border's target is it too.
    assert_eq!(got(Property::Color), Some(rgba(0x11, 0x22, 0x33, 1.0)));
    assert_eq!(got(Property::BorderLeftColor), got(Property::Color));
    assert_eq!(
        got(Property::BoxShadow),
        Some(Value::four(0.0, 4.0, 12.0, 0.0))
    );
    assert_eq!(
        got(Property::ShadowColor),
        Some(rgba(0, 0, 0, 128.0 / 255.0))
    );
    assert_eq!(got(Property::TintColor), None, "not named");
    assert!(owners.owns(two, Property::BackgroundColor));
    assert!(!owners.owns(two, Property::TintColor));
    // Under a dark appearance the pair resolves the other way.
    let dark = k.paint_sync(&receipt, true, &mut PaintOwners::default());
    let bg = dark
        .changes
        .iter()
        .find(|c| c.property == Property::BackgroundColor);
    assert_eq!(bg.map(|c| c.value), Some(rgba(0, 0, 0, 1.0)));
}

#[test]
fn an_appearance_change_transitions_and_a_dropped_row_retires() {
    let (mut k, receipt) = tree();
    let mut owners = PaintOwners::default();
    let mut e = Engine::new();
    k.motion_sync(&receipt).apply(&mut e).unwrap();
    k.paint_sync(&receipt, false, &mut owners)
        .apply(&mut e)
        .unwrap();
    let two = motion_node(k.node(2).unwrap().key);
    // light → dark: the 200ms background transition runs, as in a browser.
    k.paint_resync(true, &mut owners).apply(&mut e).unwrap();
    e.advance(0.1).unwrap();
    let mid = e.value(two, Property::BackgroundColor).unwrap();
    assert!(mid.x > 0.0 && mid.x < 1.0, "{mid:?}");
    e.advance(0.2).unwrap();
    assert_eq!(
        e.value(two, Property::BackgroundColor),
        Some(rgba(0, 0, 0, 1.0))
    );
    // A row that no longer names paint retires exactly what it owned.
    let receipt = k
        .apply(
            0,
            2,
            &[Op::SetStyle {
                id: 2,
                patch: patch(&[(StyleId::Transition, "opacity 1s")]),
            }],
        )
        .unwrap();
    k.motion_sync(&receipt).apply(&mut e).unwrap();
    let sync = k.paint_sync(&receipt, true, &mut owners);
    assert!(sync.changes.is_empty());
    assert_eq!(sync.retired.len(), 8, "{:?}", sync.retired);
    assert!(!owners.owns(two, Property::BackgroundColor));
    sync.apply(&mut e).unwrap();
    assert_eq!(e.value(two, Property::BackgroundColor), None);
}

#[test]
fn paint_transition_names_and_colour_keyframes_round_trip_the_wire() {
    let t = Transitions::parse("border-color 1s, box-shadow 2s, tint-color 3s, color 4s").unwrap();
    assert_eq!(t.0[0].property, TransitionProperty::BorderColor);
    let a = crate::keyframed(
        "k 1s @keyframes k{from{background-color:light-dark(rgba(255,0,0,1),rgba(0,128,0,1))}to{--exact-tint:rgba(0,0,255,0.5)}}",
    );
    let ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: Box::new({
                let mut s = StyleProps::default();
                s.transition = t.clone();
                s.animation = a.clone();
                s.mask.set(StyleId::Transition);
                s.mask.set(StyleId::Animation);
                s
            }),
        },
        Op::AttachRoot { id: 1 },
    ];
    let mut k = Kernel::with_monospace();
    k.apply_frame(&wire::encode(0, 1, &ops)).unwrap();
    let style = k.node(1).unwrap().style;
    assert_eq!(style.transition, t);
    // Keyframe values ride the wire as f32.
    let to = &style.animation.0[0].keyframes.0[1].values[0];
    assert_eq!(to.0, Property::TintColor);
    assert!(
        (to.1.w - 0.5).abs() < 1e-6 && (to.1.z - 0.5).abs() < 1e-6,
        "{to:?}"
    );
    // A `light-dark()` colour carries its dark value (LLP 1062 D9).
    let from = &style.animation.0[0].keyframes.0[0];
    assert_eq!(from.values[0].1.x, 1.0);
    assert_eq!(from.dark[0].0, Property::BackgroundColor);
    assert!((from.dark[0].1.y - 128.0 / 255.0).abs() < 1e-6, "{from:?}");
}

/// An `exit-animation` that names a colour owns it while the node lives, so
/// the engine has the value its exit plays over when the node leaves (LLP
/// 1063). A `currentcolor` side that draws is the host's to paint in the
/// presented `color`.
#[test]
fn an_exit_owns_its_colours_and_currentcolor_sides_follow_color() {
    let mut k = Kernel::with_monospace();
    let ops = [
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: {
                let mut s = patch(&[
                    (StyleId::BackgroundColor, "#ffffff"),
                    (StyleId::BorderColorLeft, "#ff0000"),
                    (StyleId::BorderStyleTop, "solid"),
                    (StyleId::BorderStyleLeft, "solid"),
                    (
                        StyleId::ExitAnimation,
                    "fade 200ms both @keyframes fade{to{background-color:rgba(0,0,0,0);opacity:0}}",
                    ),
                ]);
                for id in [StyleId::BorderWidthTop, StyleId::BorderWidthLeft] {
                    s.set_dynamic(id, &StyleValue::Number(2.0)).unwrap();
                }
                s
            },
        },
        Op::AttachRoot { id: 1 },
    ];
    let receipt = k.apply(0, 1, &ops).unwrap();
    let key = k.node(1).unwrap().key;
    let mut owners = PaintOwners::default();
    let sync = k.paint_sync(&receipt, false, &mut owners);
    let one = motion_node(key);
    assert!(owners.owns(one, Property::BackgroundColor));
    assert_eq!(
        sync.changes.iter().map(|c| c.property).collect::<Vec<_>>(),
        [Property::BackgroundColor]
    );
    // Top draws in currentcolor; left has its own; right and bottom draw nothing.
    assert_eq!(k.current_color_sides(key), [Property::BorderTopColor]);
}
