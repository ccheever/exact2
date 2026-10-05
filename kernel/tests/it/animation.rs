//! What the grnl port added to the `animation` row's bytes (LLP 1055 D5):
//! a `light-dark()` keyframe's dark value and `box-shadow`'s two halves
//! round-trip bit for bit (LLP 1062 D9), and an `exit-animation` that never
//! ends is refused on both ingress paths (LLP 1063 D2).

use exact_kernel::wire::codec::{Reader, Writer};
use exact_kernel::{
    wire, ApplyError, Kernel, KernelError, NodeType, Op, StyleId, StyleProps, StyleValue,
};
use exact_motion::{AnimationError, Animations, Keyframes};

const LIT: &str = "from{color:light-dark(#4f6657, #b7c9ac);box-shadow:0 6px 32px light-dark(#171b171a, #0000004d)}to{color:#171b17;box-shadow:none}";

fn rows(shorthand: &str) -> Animations {
    let rule = Keyframes::parse(LIT).unwrap();
    let mut a = Animations::parse(shorthand).unwrap();
    assert!(a.resolve(|_| Some(&rule)).is_empty());
    a
}

fn through_the_wire(row: &Animations) -> Animations {
    let mut w = Writer::new();
    w.animations(row);
    let bytes = w.into_vec();
    let mut r = Reader::new(&bytes);
    let read = r.animations().unwrap();
    assert_eq!(r.remaining(), 0);
    read
}

fn patch(id: StyleId, animations: &Animations) -> Box<StyleProps> {
    let mut s = StyleProps::default();
    match id {
        StyleId::ExitAnimation => s.rare.exit_animation = animations.clone(),
        _ => s.animation = animations.clone(),
    }
    s.mask.set(id);
    Box::new(s)
}

#[test]
fn dark_values_and_shadows_round_trip_through_exwf_bit_for_bit() {
    let row = through_the_wire(&rows("lit 900ms linear 120ms both"));
    assert_eq!(through_the_wire(&row), row);
    let first = &row.0[0].keyframes.0[0];
    assert_eq!(first.dark.len(), 2, "the colour's and the shadow's");
    let ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: patch(StyleId::Animation, &row),
        },
        Op::AttachRoot { id: 1 },
    ];
    let mut k = Kernel::with_monospace();
    k.apply_frame(&wire::encode(0, 1, &ops)).unwrap();
    assert_eq!(k.node(1).unwrap().style.animation, row);
}

#[test]
fn an_exit_that_never_ends_is_refused_on_both_paths() {
    for text in ["lit 1s infinite", "lit 1s paused"] {
        let row = rows(text);
        let mut k = Kernel::with_monospace();
        let result = k.apply(
            0,
            1,
            &[
                Op::CreateView {
                    id: 1,
                    node_type: NodeType::View,
                },
                Op::SetStyle {
                    id: 1,
                    patch: patch(StyleId::ExitAnimation, &row),
                },
            ],
        );
        assert!(
            matches!(
                result,
                Err(KernelError::Apply(ApplyError::InvalidAnimation {
                    op_index: 1,
                    error: AnimationError::Endless
                }))
            ),
            "{text}: {result:?}"
        );
        let mut s = StyleProps::default();
        assert!(s
            .set_dynamic(StyleId::ExitAnimation, &StyleValue::Text(text.into()))
            .is_err());
        // The same row is an ordinary `animation`.
        assert!(s
            .set_dynamic(StyleId::Animation, &StyleValue::Text(text.into()))
            .is_ok());
    }
}
