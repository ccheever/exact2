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

/// `animation-trigger: view`, the default (LLP 1055 D13): below a row a
/// list says it mounted out of its port, the row reaches motion paused; the
/// commit that names the row revealed resumes it. A node with `none`, and a
/// row with nothing waiting below it, are untouched.
#[test]
fn an_animation_waits_for_its_row_unless_its_trigger_is_none() {
    let row = rows("lit 900ms linear");
    let waiting = patch(StyleId::Animation, &row);
    let mut at_once = patch(StyleId::Animation, &row);
    at_once.rare.animation_trigger = exact_kernel::AnimationTrigger::None;
    at_once.mask.set(StyleId::AnimationTrigger);
    let view = |id| Op::CreateView {
        id,
        node_type: NodeType::View,
    };
    let ops = vec![
        view(1),
        view(2),
        view(3),
        view(4),
        view(5),
        Op::SetStyle {
            id: 3,
            patch: waiting,
        },
        Op::SetStyle {
            id: 5,
            patch: at_once,
        },
        Op::SetChildren {
            id: 2,
            children: vec![3],
        },
        Op::SetChildren {
            id: 4,
            children: vec![5],
        },
        Op::SetChildren {
            id: 1,
            children: vec![2, 4],
        },
        Op::AttachRoot { id: 1 },
    ];
    let mut k = Kernel::with_monospace();
    k.apply(0, 1, &ops).unwrap();
    let paused = |k: &Kernel, id| {
        let node = k.node(id).unwrap();
        k.animation_row(&node).0.iter().all(|a| a.paused)
    };
    assert!(!paused(&k, 3), "no list said its row is out of the port");
    assert!(k.await_view(2));
    assert!(!k.await_view(4), "nothing below row 4 waits");
    assert!(paused(&k, 3));
    assert!(!paused(&k, 5));
    let mut sync = Default::default();
    k.motion_sync_node(k.node(3).unwrap().key, &mut sync);
    assert!(sync.animations[0].1 .0[0].paused);

    assert!(k.reveal(2));
    assert!(!k.reveal(2), "once");
    let receipt = exact_kernel::CommitReceipt {
        revealed: vec![k.node(2).unwrap().key],
        ..Default::default()
    };
    let sync = k.motion_sync(&receipt);
    assert_eq!(sync.animations.len(), 1);
    assert_eq!(sync.animations[0].1, row, "running, from its start");
}
