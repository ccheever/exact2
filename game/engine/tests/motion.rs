use exact_game::{bin, hash, math, Now, Rng, Spring, SpringConfig, Transform, Vec3, World};

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
        spring.set_target(Now { tick: 0, hz: 60 }, 15.0);
        let direct = spring.value(Now { tick: 120, hz: 60 });
        let before = hash::of(&spring);
        let mut last = 0.0;
        for tick in 0..=120 {
            last = spring.value(Now { tick, hz: 60 });
        }
        assert_eq!(last.to_bits(), direct.to_bits());
        assert_eq!(hash::of(&spring), before);
        let old_value = spring.value(Now { tick: 60, hz: 60 });
        spring.set_target(Now { tick: 60, hz: 60 }, 20.0);
        assert!((spring.value(Now { tick: 60, hz: 60 }) - old_value).abs() < 1e-12);
        let loaded: Spring = bin::from_slice(&bin::to_vec(&spring)).unwrap();
        assert_eq!(
            loaded.value(Now { tick: 120, hz: 60 }).to_bits(),
            spring.value(Now { tick: 120, hz: 60 }).to_bits()
        );
    }
    assert!(Spring::new(8.0).at_rest(Now { tick: 0, hz: 60 }));
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

#[test]
fn named_movement_accelerates_turns_brakes_and_preserves_vertical_velocity() {
    use exact_game::motion::Move;
    let movement = Move {
        speed: 4.0,
        accel: 12.0,
        brake: 20.0,
    };
    let mut velocity = Vec3::new(0.0, 7.0, 0.0);
    movement.step(&mut velocity, Vec3::X, 0.125);
    assert_eq!(velocity, Vec3::new(1.5, 7.0, 0.0));
    movement.step(&mut velocity, Vec3::X, 1.0);
    assert_eq!(velocity, Vec3::new(4.0, 7.0, 0.0));
    movement.step(&mut velocity, Vec3::NEG_X, 0.25);
    assert_eq!(velocity.x, 1.0);
    movement.step(&mut velocity, Vec3::ZERO, 0.025);
    assert_eq!(velocity.x, 0.5);
    movement.step(&mut velocity, Vec3::ZERO, 0.1);
    assert_eq!(velocity, Vec3::new(0.0, 7.0, 0.0));
    for _ in 0..60 {
        movement.step(&mut velocity, Vec3::ZERO, 1.0 / 60.0);
    }
    assert_eq!(velocity, Vec3::new(0.0, 7.0, 0.0));
    velocity.x = 4.0;
    for _ in 0..13 {
        movement.step(&mut velocity, Vec3::ZERO, 1.0 / 60.0);
    }
    assert_eq!(velocity, Vec3::new(0.0, 7.0, 0.0));
    movement.step(&mut velocity, Vec3::new(10.0, 99.0, 10.0), 1.0);
    assert!((Vec3::new(velocity.x, 0.0, velocity.z).length() - 4.0).abs() < 1e-6);
    movement.step(&mut velocity, Vec3::new(0.0, 99.0, 0.5), 1.0);
    assert_eq!(velocity, Vec3::new(0.0, 7.0, 2.0));
    movement.step(&mut velocity, Vec3::X, 0.0);
    assert_eq!(velocity, Vec3::new(0.0, 7.0, 2.0));
}

#[test]
fn named_jump_reaches_authored_height_and_gravity_preserves_planar_velocity() {
    use exact_game::motion::{Gravity, Jump};
    let mut velocity = Vec3::new(2.0, -8.0, 3.0);
    Jump {
        height: 1.2,
        gravity: 9.81,
    }
    .start(&mut velocity);
    assert_eq!(
        velocity.y.to_bits(),
        libm::sqrtf(2.0 * 9.81 * 1.2).to_bits()
    );
    let apex = velocity.y / 9.81;
    let rise = velocity.y * apex - 0.5 * 9.81 * apex * apex;
    assert!((rise - 1.2).abs() < 1e-6);
    Gravity(9.81).step(&mut velocity, apex);
    assert_eq!(velocity, Vec3::new(2.0, 0.0, 3.0));
    Gravity(9.81).step(&mut velocity, 1.0);
    assert_eq!(velocity, Vec3::new(2.0, -9.81, 3.0));
    Jump {
        height: 0.0,
        gravity: 9.81,
    }
    .start(&mut velocity);
    assert_eq!(velocity, Vec3::new(2.0, 0.0, 3.0));
}
