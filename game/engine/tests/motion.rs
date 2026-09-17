use exact_game::{bin, hash, math, Rng, Spring, SpringConfig, Transform, Vec3, World};

#[test]
fn spring_seeks_have_no_sampling_history() {
    for damping in [3.0, 20.0, 40.0] {
        let mut spring = Spring {
            config: SpringConfig {
                stiffness: 100.0,
                damping,
                mass: 1.0,
            },
            ..Spring::new(3.0)
        };
        spring.set_target(0, 60, 15.0);
        let direct = spring.value(120, 60);
        let before = hash::of(&spring);
        let mut last = 0.0;
        for tick in 0..=120 {
            last = spring.value(tick, 60);
        }
        assert_eq!(last.to_bits(), direct.to_bits());
        assert_eq!(hash::of(&spring), before);
        let old_value = spring.value(60, 60);
        spring.set_target(60, 60, 20.0);
        assert!((spring.value(60, 60) - old_value).abs() < 1e-12);
        let loaded: Spring = bin::from_slice(&bin::to_vec(&spring)).unwrap();
        assert_eq!(
            loaded.value(120, 60).to_bits(),
            spring.value(120, 60).to_bits()
        );
    }
    assert!(Spring::new(8.0).at_rest(0, 60));
}
#[test]
fn rng_is_reproducible_bounded_and_round_trips() {
    let mut a = Rng::new(123);
    let mut b = Rng::new(123);
    for _ in 0..10000 {
        assert_eq!(a.next_u32(), b.next_u32());
        let n = a.next_f32();
        assert!((0.0..1.0).contains(&n));
        assert_eq!(n, b.next_f32());
        let n = a.range(-50i32..30);
        assert!((-50..30).contains(&n));
        assert_eq!(n, b.range(-50i32..30));
        assert!((-3.0..4.0).contains(&a.range(-3.0..4.0)));
        b.range(-3.0..4.0);
        assert!((f32::MIN..f32::MAX).contains(&a.range(f32::MIN..f32::MAX)));
        b.range(f32::MIN..f32::MAX);
    }
    let mut saved: Rng = bin::from_slice(&bin::to_vec(&a)).unwrap();
    assert_eq!(a.next_u32(), saved.next_u32());
    assert!(a.pick::<u8>(&[]).is_none());
    assert_eq!(a.pick(&[7]), Some(&7));
    assert!(!a.chance(0.0));
    assert!(a.chance(1.0));
    let f = 1.0f32;
    assert_eq!(a.range(f..f32::from_bits(f.to_bits() + 1)), f);
}
#[test]
fn transform_defaults_aim_and_deep_chains() {
    assert_eq!(Transform::default().scale, Vec3::ONE);
    let aimed = Transform::default()
        .looking_at(Vec3::X, Vec3::Y)
        .with_scale(2.0);
    assert!((aimed.rotation * Vec3::NEG_Z - Vec3::X).length() < 1e-6);
    assert_eq!(aimed.scale, Vec3::splat(2.0));
    let mut w = World::new(60, 0);
    let mut child = w.spawn((Transform::at(1.0, 0.0, 0.0),));
    let first = child;
    for _ in 0..2000 {
        let parent = w.spawn((Transform::at(1.0, 0.0, 0.0),));
        w.insert(child, exact_game::Parent(parent));
        child = parent;
    }
    w.propagate();
    assert_eq!(w.global(first).unwrap().translation.x, 2001.0);
}
#[test]
fn math_uses_the_declared_functions() {
    assert_eq!(math::sin(1.0).to_bits(), libm::sinf(1.0).to_bits());
    assert_eq!(math::exp(2.0).to_bits(), libm::expf(2.0).to_bits());
    assert_eq!(math::smoothstep(0.0, 1.0, 0.5), 0.5);
    assert_eq!(
        math::wrap_angle(std::f32::consts::PI),
        -std::f32::consts::PI
    );
    assert_eq!(math::lerp(0.0, 10.0, 0.3), 3.0);
}
