//! The wide colour table reclaims its slots (LLP 1121 §5 stage 4, as built:
//! mark and sweep). One test, in its own process: the table is the
//! process's, and parallel tests would share it.

use exact_kernel::motion::{motion_node, PaintMotion};
use exact_kernel::style::wide::{self, Keep, WideMark};
use exact_kernel::{ColorValue, Kernel, NodeType, Op, RowValue, StyleId, StyleProps, StyleValue};
use exact_motion::{Engine, Property};

const CAP: usize = 4096;

/// A distinct display-p3 colour for each `i` below 10,000.
fn p3(i: usize) -> String {
    format!(
        "color(display-p3 {} {} 0.25)",
        (i % 100) as f64 / 100.0,
        (i / 100) as f64 / 100.0
    )
}

/// The table's text for `p3(i)`.
fn p3_text(i: usize) -> String {
    match ColorValue::parse_light_dark(&p3(i)) {
        Some(ColorValue::Wide(id)) => wide::wide(id).unwrap().text.to_string(),
        other => panic!("{other:?}"),
    }
}

fn background(id: u32, css: &str) -> Op {
    let mut s = StyleProps::default();
    s.set_dynamic(StyleId::BackgroundColor, &StyleValue::Text(css.into()))
        .unwrap_or_else(|e| panic!("{css}: {e:?}"));
    Op::SetStyle {
        id,
        patch: Box::new(s),
    }
}

fn kernel_with(colour: &str, batch: &mut u64) -> Kernel {
    let mut k = Kernel::with_monospace();
    *batch += 1;
    k.apply(
        0,
        *batch,
        &[
            Op::CreateView {
                id: 1,
                node_type: NodeType::View,
            },
            background(1, colour),
        ],
    )
    .unwrap();
    k
}

fn shown(k: &Kernel) -> String {
    match k.node(1).unwrap().computed(StyleId::BackgroundColor) {
        RowValue::ColorValue(ColorValue::Wide(id)) => wide::wide(id).unwrap().text.to_string(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_wide_table_reclaims_what_no_holder_holds() {
    let mut batch = 0;
    // An idle kernel holding one colour, made first and never touched again.
    let idle = kernel_with(&p3(9_999), &mut batch);
    let idle_colour = shown(&idle);
    // A colour a `Keep` holds (as a queued patch or a role table does).
    let kept_colour = p3(9_998);
    let ColorValue::Wide(kept_id) = ColorValue::parse_light_dark(&kept_colour).unwrap() else {
        panic!("not wide")
    };
    let keep = Keep::of(|m| m.color(ColorValue::Wide(kept_id)));

    // A colour transition, from one wide colour to another, started now.
    let mut moving = Kernel::with_monospace();
    let style = |rows: &[(StyleId, &str)]| {
        let mut s = StyleProps::default();
        for (id, text) in rows {
            s.set_dynamic(*id, &StyleValue::Text((*text).into()))
                .unwrap();
        }
        Box::new(s)
    };
    batch += 1;
    let receipt = moving
        .apply(
            0,
            batch,
            &[
                Op::CreateView {
                    id: 1,
                    node_type: NodeType::View,
                },
                Op::SetStyle {
                    id: 1,
                    patch: style(&[
                        (StyleId::BackgroundColor, "color(display-p3 1 0 0)"),
                        (StyleId::Transition, "all 1s linear"),
                    ]),
                },
                Op::AttachRoot { id: 1 },
            ],
        )
        .unwrap();
    let Some(ColorValue::Wide(red)) = ColorValue::parse_light_dark("color(display-p3 1 0 0)")
    else {
        panic!("not wide")
    };
    let mut engine = Engine::new();
    let mut paint = PaintMotion::default();
    moving.motion_sync(&receipt).apply(&mut engine).unwrap();
    paint.adopt(&moving, receipt.created.iter().copied(), &mut engine);
    batch += 1;
    let receipt = moving
        .apply(0, batch, &[background(1, "color(display-p3 0 0 1)")])
        .unwrap();
    moving.motion_sync(&receipt).apply(&mut engine).unwrap();
    paint.sync(&moving, &receipt, &mut engine);
    // The red style goes from its kernel's table too.
    moving.trim();
    let node = motion_node(moving.node(1).unwrap().key);
    assert!(paint.owns(node, Property::BackgroundColor));
    engine.advance(0.5).unwrap();
    let midway = paint.shown(&engine, node, Property::BackgroundColor);
    assert!(midway.is_some());

    // More than the table's capacity, set and replaced in turn on one node:
    // every colour is admitted and the table stays bounded.
    let mut busy = kernel_with(&p3(0), &mut batch);
    for i in 1..6_000 {
        batch += 1;
        busy.apply(0, batch, &[background(1, &p3(i))]).unwrap();
        assert!(wide::used() <= CAP, "{} slots after {i}", wide::used());
        assert_eq!(
            shown(&busy),
            wide::wide(match ColorValue::parse_light_dark(&p3(i)) {
                Some(ColorValue::Wide(id)) => id,
                _ => panic!(),
            })
            .unwrap()
            .text
            .to_string()
        );
    }
    // The transition kept its endpoints through every sweep: the red it
    // left, whose slot was reclaimed (interned again, it is another id), and
    // the blue it goes to.
    assert_eq!(
        paint.shown(&engine, node, Property::BackgroundColor),
        midway
    );
    assert_ne!(
        ColorValue::parse_light_dark("color(display-p3 1 0 0)"),
        Some(ColorValue::Wide(red))
    );
    engine.advance(1.0).unwrap();
    assert_eq!(shown(&moving), "color(display-p3 0 0 1)");
    // An id parsed, then held back across other commits on its thread
    // (which close its batch), survives sweeps for `GRACE` interns.
    let in_flight = background(1, &p3(7_777));
    for i in 0..(wide::GRACE as usize / 2) {
        batch += 1;
        busy.apply(0, batch, &[background(1, &p3(3_000 + i))])
            .unwrap();
    }
    batch += 1;
    busy.apply(0, batch, &[in_flight]).unwrap();
    assert_eq!(shown(&busy), p3_text(7_777));
    // The idle kernel's colour and the kept one survived every sweep.
    assert_eq!(shown(&idle), idle_colour);
    assert!(wide::wide(kept_id).is_some());
    drop(keep);

    // A full table of held colours refuses a new one, and the refusal is
    // the caller's to report: no patch can be made of it, so the row keeps
    // its old value.
    let mut many: Vec<Kernel> = Vec::new();
    let mut i = 6_000;
    let refused = loop {
        let css = p3(i);
        let mut s = StyleProps::default();
        if s.set_dynamic(
            StyleId::BackgroundColor,
            &StyleValue::Text(css.as_str().into()),
        )
        .is_err()
        {
            break css;
        }
        many.push(kernel_with(&css, &mut batch));
        i += 1;
        assert!(i < 6_000 + 2 * CAP, "never refused");
    };
    assert!(wide::used() <= CAP);
    // The refusal is counted for the caller to report.
    let refusals = wide::refusals();
    assert!(refusals > 0);
    let before = shown(&busy);
    let mut s = StyleProps::default();
    assert!(s
        .set_dynamic(
            StyleId::BackgroundColor,
            &StyleValue::Text(refused.as_str().into())
        )
        .is_err());
    assert_eq!(wide::refusals(), refusals + 1);
    assert_eq!(shown(&busy), before);
    // Once holders let go, there is room again.
    many.clear();
    batch += 1;
    busy.apply(0, batch, &[background(1, &p3(1))]).unwrap();
    let mut s = StyleProps::default();
    assert!(s
        .set_dynamic(
            StyleId::BackgroundColor,
            &StyleValue::Text(refused.as_str().into())
        )
        .is_ok());
    let _ = WideMark::default();
}
