//! The `animation` row (LLP 1057): its bytes, its refusals, and the seam that
//! hands it to the engine — which must never restart an animation the row
//! still names.

use exact_kernel::wire::codec::{Reader, Writer};
use exact_kernel::{
    motion_node, wire, Animations, ApplyError, DecodeError, Kernel, KernelError, NodeType, Op,
    StyleId, StyleProps, StyleValue,
};
use exact_motion::{AnimationError, Engine, Property, Value};

const RULES: &str = "@keyframes breathe{from{opacity:0.4;scale:0.9}50%{opacity:1;animation-timing-function:steps(3, jump-none)}to{opacity:0.4}} \
                     @keyframes rise{0%{translate:0px 8px;rotate:-4deg;animation-timing-function:linear(0, 0.8 30%, 1)}}";

fn rows(shorthand: &str) -> Animations {
    Animations::parse(&format!("{shorthand} {RULES}")).unwrap()
}

fn patch(animations: &Animations) -> Box<StyleProps> {
    let mut s = StyleProps::default();
    s.animation = animations.clone();
    s.mask.set(StyleId::Animation);
    Box::new(s)
}

#[test]
fn the_row_round_trips_through_exwf_bit_for_bit() {
    for text in [
        "breathe 1.6s ease-in-out infinite",
        "rise 300ms cubic-bezier(0.2, 0, 0, 1) -50ms 2.5 alternate-reverse both paused, breathe 2s",
        "none",
    ] {
        // Times and values ride the wire as f32 like every row (LLP 1002
        // §5): one pass fixes them, and every later pass keeps every bit.
        let row = through_the_wire(&rows(text));
        assert_eq!(through_the_wire(&row), row);
        let ops = vec![
            Op::CreateView {
                id: 1,
                node_type: NodeType::View,
            },
            Op::SetStyle {
                id: 1,
                patch: patch(&row),
            },
            Op::AttachRoot { id: 1 },
        ];
        let mut k = Kernel::with_monospace();
        k.apply_frame(&wire::encode(0, 1, &ops)).unwrap();
        assert_eq!(k.node(1).unwrap().style.animation, row, "{text}");
    }
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

#[test]
fn bytes_outside_the_grammar_are_refused_on_decode() {
    let mut w = Writer::new();
    w.animations(&rows("breathe 1s"));
    let good = w.into_vec();
    // count u8, name (u32 + 7), duration, delay, easing tag, iterations, then
    // the direction byte.
    let direction = 1 + 4 + "breathe".len() + 4 + 4 + 1 + 4;
    let mut bad = good.clone();
    bad[direction] = 9;
    assert_eq!(
        Reader::new(&bad).animations(),
        Err(DecodeError::UnknownAnimationValue(9))
    );
    // A spring is a `transition` extension; an animation's easing is CSS's.
    let easing = 1 + 4 + "breathe".len() + 4 + 4;
    let mut spring = good[..easing].to_vec();
    spring.push(7);
    spring.extend_from_slice(&[0; 12]);
    spring.extend_from_slice(&good[easing + 1..]);
    assert_eq!(
        Reader::new(&spring).animations(),
        Err(DecodeError::UnknownEasing(7))
    );
    assert_eq!(
        Reader::new(&[9]).animations(),
        Err(DecodeError::TooManyAnimations(9))
    );
}

#[test]
fn a_structured_patch_is_validated_like_the_bytes() {
    let mut row = rows("breathe 1s");
    row.0[0].duration = -1.0;
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
                patch: patch(&row),
            },
        ],
    );
    assert!(matches!(
        result,
        Err(KernelError::Apply(ApplyError::InvalidAnimation {
            op_index: 1,
            error: AnimationError::NegativeDuration
        }))
    ));
}

#[test]
fn the_dynamic_form_is_the_rows_text_and_a_bad_one_changes_nothing() {
    let mut s = StyleProps::default();
    let row = rows("breathe 1.6s ease-in-out infinite");
    s.set_dynamic(StyleId::Animation, &StyleValue::Text(row.text()))
        .unwrap();
    assert_eq!(s.animation, row);
    let before = s.clone();
    assert!(s
        .set_dynamic(StyleId::Animation, &StyleValue::Text("pulse 1s".into()))
        .is_err());
    assert_eq!(s, before);
}

#[test]
fn the_seam_plays_the_row_and_an_unrelated_update_never_restarts_it() {
    let mut k = Kernel::with_monospace();
    let receipt = k
        .apply(
            0,
            1,
            &[
                Op::CreateView {
                    id: 1,
                    node_type: NodeType::View,
                },
                Op::SetStyle {
                    id: 1,
                    patch: patch(&rows("breathe 2s linear infinite")),
                },
                Op::AttachRoot { id: 1 },
            ],
        )
        .unwrap();
    let node = motion_node(k.node(1).unwrap().key);
    let mut engine = Engine::new();
    k.motion_sync(&receipt).apply(&mut engine).unwrap();
    assert_eq!(
        engine.value(node, Property::Opacity),
        Some(Value::scalar(0.4))
    );
    engine.advance(0.5).unwrap();
    // The node is touched for something else; the row is unchanged.
    let mut other = StyleProps::default();
    other.width = exact_kernel::Dimension::Points(20.0);
    other.mask.set(StyleId::Width);
    let receipt = k
        .apply(
            0,
            2,
            &[Op::SetStyle {
                id: 1,
                patch: Box::new(other),
            }],
        )
        .unwrap();
    k.motion_sync(&receipt).apply(&mut engine).unwrap();
    let opacity = engine.value(node, Property::Opacity).unwrap().x;
    assert!((opacity - 0.4).abs() > 0.1, "restarted: {opacity}");
    // Clearing the row stops it: the node shows its own opacity again.
    let mut mask = exact_kernel::StyleMask::EMPTY;
    mask.set(StyleId::Animation);
    let receipt = k.apply(0, 3, &[Op::ClearStyle { id: 1, mask }]).unwrap();
    k.motion_sync(&receipt).apply(&mut engine).unwrap();
    assert_eq!(
        engine.value(node, Property::Opacity),
        Some(Value::scalar(1.0))
    );
    assert!(engine.quiescent());
}
