//! Whether a symbol image draws in its author's `color` when nobody tinted
//! it (LLP 1121 stage 9 B, D10): its own written `color`, or the nearest
//! ancestor's that sets one; not a colour no author wrote, and not on a
//! native control's face.

use exact_kernel::{
    Kernel, MonospaceMeasurer, NodeType, Op, PropId, PropValue, StyleId, StyleMask, StyleProps,
    StyleValue,
};

/// `color` (or another row) set to `value`, written by its author or not.
fn row(id: StyleId, value: &str, written: bool) -> Box<StyleProps> {
    let mut style = StyleProps::default();
    style
        .set_dynamic(id, &StyleValue::Text(value.into()))
        .unwrap();
    if written {
        style.written.set(id);
    }
    Box::new(style)
}

fn set(k: &mut Kernel, id: u32, patch: Box<StyleProps>) -> Vec<u32> {
    let receipt = k.apply(0, 0, &[Op::SetStyle { id, patch }]).unwrap();
    receipt
        .touched
        .iter()
        .filter_map(|key| k.node_by_key(*key).map(|n| n.id))
        .collect()
}

fn source(id: u32, value: &str) -> Op {
    Op::SetProp {
        id,
        prop: PropId::ImageSource,
        value: PropValue::Str(value.into()),
    }
}

/// 1 > 2 > 3 > { 4 `symbol:star`, 5 a photo, 6 a native button > 7 `symbol:star` }.
fn tree() -> Kernel {
    let mut k = Kernel::new(Box::new(MonospaceMeasurer::default()));
    let views = [
        (1, NodeType::View),
        (2, NodeType::View),
        (3, NodeType::View),
        (4, NodeType::Image),
        (5, NodeType::Image),
        (6, NodeType::Control),
        (7, NodeType::Image),
    ];
    let mut ops: Vec<Op> = views
        .iter()
        .map(|&(id, node_type)| Op::CreateView { id, node_type })
        .collect();
    ops.extend([
        source(4, "symbol:star"),
        source(5, "photo.png"),
        Op::SetProp {
            id: 6,
            prop: PropId::Type,
            value: PropValue::Str("button".into()),
        },
        source(7, "symbol:star"),
        Op::SetChildren {
            id: 1,
            children: vec![2],
        },
        Op::SetChildren {
            id: 2,
            children: vec![3],
        },
        Op::SetChildren {
            id: 3,
            children: vec![4, 5, 6],
        },
        Op::SetChildren {
            id: 6,
            children: vec![7],
        },
        Op::AttachRoot { id: 1 },
    ]);
    k.apply(0, 0, &ops).unwrap();
    k
}

fn said(k: &Kernel, id: u32) -> bool {
    let node = k.node(id).unwrap();
    assert_eq!(node.facts().symbol_color, node.symbol_color_said());
    node.symbol_color_said()
}

#[test]
fn nobody_wrote_a_colour() {
    let k = tree();
    assert!(!said(&k, 4));
}

#[test]
fn an_ancestors_written_colour_reaches_a_symbol_and_its_clear_touches_it() {
    let mut k = tree();
    let touched = set(&mut k, 1, row(StyleId::TextColor, "red", true));
    assert!(
        touched.contains(&4),
        "a host re-emits the symbol: {touched:?}"
    );
    assert!(said(&k, 4));
    // Not a photo, and not a native button's face, which keeps the
    // control's tint (D9).
    assert!(!said(&k, 5) && !said(&k, 7));
    // A token or a role is written as any colour is.
    set(&mut k, 1, row(StyleId::TextColor, "AccentColor", true));
    assert!(said(&k, 4));
    let receipt = k
        .apply(
            0,
            0,
            &[Op::ClearStyle {
                id: 1,
                mask: StyleMask::of(StyleId::TextColor),
                written: false,
            }],
        )
        .unwrap();
    let touched: Vec<u32> = receipt
        .touched
        .iter()
        .filter_map(|key| k.node_by_key(*key).map(|n| n.id))
        .collect();
    assert!(touched.contains(&4), "{touched:?}");
    assert!(!said(&k, 4));
}

#[test]
fn the_nearest_ancestor_that_sets_a_colour_decides() {
    let mut k = tree();
    set(&mut k, 1, row(StyleId::TextColor, "red", true));
    // A nearer colour no author wrote (a sheet's) is not the author's.
    set(&mut k, 3, row(StyleId::TextColor, "blue", false));
    assert!(!said(&k, 4));
    // Written there, it is.
    set(&mut k, 3, row(StyleId::TextColor, "blue", true));
    assert!(said(&k, 4));
}

#[test]
fn a_nearer_tint_does_not_decide_the_colour() {
    let mut k = tree();
    set(&mut k, 3, row(StyleId::TintColor, "green", true));
    assert!(!said(&k, 4));
    set(&mut k, 1, row(StyleId::TextColor, "red", true));
    assert!(said(&k, 4));
}

#[test]
fn its_own_written_colour() {
    let mut k = tree();
    set(&mut k, 4, row(StyleId::TextColor, "red", true));
    assert!(said(&k, 4));
    // Not the face of a native button, even its own.
    set(&mut k, 7, row(StyleId::TextColor, "red", true));
    assert!(!said(&k, 7));
}

fn touched_by(k: &mut Kernel, ops: &[Op]) -> Vec<u32> {
    let receipt = k.apply(0, 0, ops).unwrap();
    receipt
        .touched
        .iter()
        .filter_map(|key| k.node_by_key(*key).map(|n| n.id))
        .collect()
}

/// A move between two parents whose colours are equal, one written and one
/// a sheet's, changes whether the symbol is said with no inherited value
/// changing: the image is touched, so a host sends it again, both ways.
#[test]
fn a_move_under_an_equal_colour_of_another_provenance_touches_the_symbol() {
    let mut k = tree();
    k.apply(
        0,
        0,
        &[
            Op::CreateView {
                id: 8,
                node_type: NodeType::View,
            },
            Op::SetChildren {
                id: 1,
                children: vec![2, 8],
            },
        ],
    )
    .unwrap();
    set(&mut k, 2, row(StyleId::TextColor, "red", true));
    set(&mut k, 8, row(StyleId::TextColor, "red", false));
    assert!(said(&k, 4));
    let touched = touched_by(
        &mut k,
        &[Op::SetChildren {
            id: 8,
            children: vec![4],
        }],
    );
    assert!(touched.contains(&4), "{touched:?}");
    assert!(!said(&k, 4));
    let touched = touched_by(
        &mut k,
        &[Op::SetChildren {
            id: 3,
            children: vec![4, 5, 6],
        }],
    );
    assert!(touched.contains(&4), "{touched:?}");
    assert!(said(&k, 4));
}

/// Out of a native button's face and back, under a colour that reaches both.
#[test]
fn a_move_off_a_buttons_face_touches_the_symbol() {
    let mut k = tree();
    set(&mut k, 1, row(StyleId::TextColor, "red", true));
    assert!(!said(&k, 7));
    let touched = touched_by(
        &mut k,
        &[Op::SetChildren {
            id: 3,
            children: vec![4, 5, 6, 7],
        }],
    );
    assert!(touched.contains(&7), "{touched:?}");
    assert!(said(&k, 7));
    let touched = touched_by(
        &mut k,
        &[Op::SetChildren {
            id: 6,
            children: vec![7],
        }],
    );
    assert!(touched.contains(&7), "{touched:?}");
    assert!(!said(&k, 7));
}
