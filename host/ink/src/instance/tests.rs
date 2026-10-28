use super::*;

fn quad() -> Instance {
    Instance::rounded([0., 0., 10., 10.], [2.; 4], [1.; 4])
}
fn clip(x: f32) -> RoundedClip {
    RoundedClip {
        rect: [x, x, 20., 20.],
        radii: [2.031, 3.5, 4.25, 5.],
    }
}

#[test]
fn batches_preserve_order_and_break_only_for_scissor_or_bindings() {
    let mut e = Encoder::new(100, 100);
    let a = Bindings::default();
    let b = Bindings {
        image: TextureId(7),
        ..a
    };
    e.push(quad(), a).unwrap();
    e.push(quad(), a).unwrap();
    e.push_clip(Clip::Rounded(clip(3.))).unwrap();
    e.push(quad(), a).unwrap();
    e.push(quad(), b).unwrap();
    e.push_clip(Clip::Scissor(Scissor {
        x: 10,
        y: 20,
        width: 100,
        height: 15,
    }))
    .unwrap();
    e.push(quad(), b).unwrap();
    e.pop_clip();
    e.push(quad(), b).unwrap();
    let f = e.finish();
    assert_eq!(
        f.batches
            .iter()
            .map(|b| b.instances.clone())
            .collect::<Vec<_>>(),
        [0..3, 3..4, 4..5, 5..6]
    );
    assert_eq!(
        f.batches[2].scissor,
        Scissor {
            x: 10,
            y: 20,
            width: 90,
            height: 15
        }
    );
    assert_eq!(f.instances[2].clip0, clip(3.).rect);
    assert_eq!(f.instances[0].flags & flags::CLIP0, 0);
}

#[test]
fn clips_fill_slots_refuse_overflow_and_restore_after_pop() {
    let mut e = Encoder::new(100, 100);
    let mask = CoverageClip {
        rect: [1., 2., 30., 40.],
        uv: [0.1, 0.2, 0.3, 0.4],
    };
    e.push_clip(Clip::Rounded(clip(1.))).unwrap();
    e.push_clip(Clip::Rounded(clip(2.))).unwrap();
    e.push_clip(Clip::Coverage(mask)).unwrap();
    e.push(quad(), Bindings::default()).unwrap();
    for extra in [
        Clip::Rounded(clip(3.)),
        Clip::Coverage(mask),
        Clip::Transformed,
    ] {
        assert_eq!(e.push_clip(extra), Err(NeedsLayer));
        assert_eq!(e.push(quad(), Bindings::default()), Err(NeedsLayer));
        e.pop_clip();
    }
    e.pop_clip();
    e.pop_clip();
    e.push(quad(), Bindings::default()).unwrap();
    let f = e.finish();
    assert_eq!(f.instances.len(), 2);
    assert_eq!(f.instances[0].clip0, clip(2.).rect);
    assert_eq!(f.instances[0].clip1, clip(1.).rect);
    assert_eq!(f.instances[0].mask_rect, mask.rect);
    assert_eq!(f.instances[1].flags & (flags::CLIP1 | flags::MASK), 0);
    assert_eq!(
        f.instances[0].clip_radii,
        [
            32 | (56 << 16),
            68 | (80 << 16),
            32 | (56 << 16),
            68 | (80 << 16)
        ]
    );
}

#[test]
fn replay_preserves_inner_clips_and_refuses_a_third_without_emitting() {
    let mut e = Encoder::new(20, 20);
    let mut row = quad();
    row.set_clip(0, clip(1.)).unwrap();
    e.push_clip(Clip::Rounded(clip(2.))).unwrap();
    e.push(row, Bindings::default()).unwrap();
    e.push_clip(Clip::Rounded(clip(3.))).unwrap();
    assert_eq!(e.push(row, Bindings::default()), Err(NeedsLayer));
    let f = e.finish();
    assert_eq!(f.instances.len(), 1);
    assert_eq!(f.instances[0].clip0, row.clip0);
    assert_eq!(f.instances[0].clip1, clip(2.).rect);
    assert_eq!(pack_radii([5000.; 4]), Err(NeedsLayer));
}

#[test]
fn disjoint_scissors_emit_nothing_and_pop_restores_the_viewport() {
    let mut e = Encoder::new(10, 10);
    e.push_clip(Clip::Scissor(Scissor {
        x: 20,
        y: 0,
        width: 10,
        height: 10,
    }))
    .unwrap();
    e.push(quad(), Bindings::default()).unwrap();
    e.pop_clip();
    e.push(quad(), Bindings::default()).unwrap();
    assert_eq!(e.finish().instances.len(), 1);
}
