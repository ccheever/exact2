//! The no-panic policy, exercised: mutated frames and envelopes are refused
//! with a typed error or accepted whole, never a panic, and a refused frame
//! changes nothing.

use exact_kernel::{
    export, wire, Dimension, Kernel, NodeType, Offer, Op, PropId, StyleId, StyleProps,
};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

fn sample_ops() -> Vec<Op> {
    let mut s = StyleProps::default();
    s.width = Dimension::Percent(40.0);
    s.mask.set(StyleId::Width);
    s.flex_grow = 1.0;
    s.mask.set(StyleId::FlexGrow);
    s.grid_template_columns = exact_kernel::GridTracks::equal(3);
    s.mask.set(StyleId::GridTemplateColumns);
    vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::CreateView {
            id: 2,
            node_type: NodeType::Text,
        },
        Op::SetStyle {
            id: 1,
            patch: Box::new(s),
        },
        Op::SetProp {
            id: 2,
            prop: PropId::Text,
            value: "mutate me".into(),
        },
        Op::SetProp {
            id: 2,
            prop: PropId::Disabled,
            value: true.into(),
        },
        Op::SetChildren {
            id: 1,
            children: vec![2],
        },
        Op::AttachRoot { id: 1 },
    ]
}

fn mutate(rng: &mut Rng, bytes: &[u8]) -> Vec<u8> {
    let mut out = bytes.to_vec();
    match rng.next() % 5 {
        0 => {
            // Flip a few bytes.
            for _ in 0..1 + rng.next() % 4 {
                let i = (rng.next() as usize) % out.len();
                out[i] ^= 1 << (rng.next() % 8);
            }
        }
        1 => {
            // Truncate.
            let n = (rng.next() as usize) % out.len();
            out.truncate(n);
        }
        2 => {
            // Splice a random run.
            let i = (rng.next() as usize) % out.len();
            let n = (rng.next() as usize) % 16;
            for k in 0..n {
                if i + k < out.len() {
                    out[i + k] = rng.next() as u8;
                }
            }
        }
        3 => {
            // Extend with garbage.
            for _ in 0..rng.next() % 64 {
                out.push(rng.next() as u8);
            }
        }
        _ => {
            // Overwrite a length-ish field with an extreme value.
            let i = ((rng.next() as usize) % (out.len() / 4).max(1)) * 4;
            let v: u32 = [0, 1, 7, 8, 0xffff, 0x7fff_ffff, 0xffff_ffff][(rng.next() % 7) as usize];
            if i + 4 <= out.len() {
                out[i..i + 4].copy_from_slice(&v.to_le_bytes());
            }
        }
    }
    out
}

#[test]
fn mutated_frames_never_panic_and_never_apply_partially() {
    let ops = sample_ops();
    let good = wire::encode(0, 1, &ops);
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let mut refused = 0;
    let mut accepted = 0;
    for _ in 0..5000 {
        let bytes = mutate(&mut rng, &good);
        let mut k = Kernel::with_monospace();
        let before = k.export(None).unwrap();
        match k.apply_frame(&bytes) {
            Ok(_) => {
                accepted += 1;
                // Whatever was accepted is a whole, consistent tree: layout runs on every root.
                for root in k.roots() {
                    k.compute_layout(root, Offer::definite(100.0, 100.0))
                        .unwrap();
                }
            }
            Err(_) => {
                refused += 1;
                assert_eq!(
                    k.export(None).unwrap(),
                    before,
                    "a refused frame changed the kernel"
                );
                assert_eq!(k.epoch(), 0);
                assert_eq!(k.live_count(), 0);
            }
        }
    }
    assert!(refused > 0, "the mutator never produced a malformed frame");
    assert!(
        accepted > 0,
        "the mutator never produced a still-valid frame (padding flips should)"
    );
}

#[test]
fn mutated_envelopes_never_panic() {
    let mut k = Kernel::with_monospace();
    k.apply(0, 1, &sample_ops()).unwrap();
    k.compute_layout(1, Offer::definite(100.0, 100.0)).unwrap();
    let good = k.export(None).unwrap();
    assert!(export::decode(&good).is_ok());
    let mut rng = Rng(0xdead_beef_cafe_f00d);
    let mut refused = 0;
    for _ in 0..5000 {
        let bytes = mutate(&mut rng, &good);
        if export::decode(&bytes).is_err() {
            refused += 1;
        }
    }
    assert!(refused > 0);
}

#[test]
fn random_bytes_are_refused() {
    let mut rng = Rng(42);
    for len in [0usize, 1, 7, 8, 39, 40, 41, 48, 56, 64, 128, 1024] {
        let bytes: Vec<u8> = (0..len).map(|_| rng.next() as u8).collect();
        let mut k = Kernel::with_monospace();
        assert!(k.apply_frame(&bytes).is_err());
        assert!(export::decode(&bytes).is_err());
    }
}
