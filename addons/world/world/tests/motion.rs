use exact_world::{bin, math, Rng};

#[test]
fn rng_is_reproducible_bounded_and_round_trips() {
    let mut a = Rng::new(123);
    let mut b = Rng::new(123);
    for _ in 0..10000 {
        assert_eq!(a.next_u32(), b.next_u32());
        let n = a.next_f32();
        assert!((0.0..1.0).contains(&n));
        assert_eq!(n, b.next_f32());
    }
    let mut saved: Rng = bin::from_slice(&bin::to_vec(&a).unwrap()).unwrap();
    assert_eq!(a.next_u32(), saved.next_u32());
}
#[test]
fn math_uses_the_declared_functions() {
    assert_eq!(math::sin(1.0).to_bits(), libm::sinf(1.0).to_bits());
    assert_eq!(math::exp(2.0).to_bits(), libm::expf(2.0).to_bits());
    assert_eq!(
        math::wrap_angle(std::f32::consts::PI),
        -std::f32::consts::PI
    );
    assert_eq!(math::lerp(0.0, 10.0, 0.3), 3.0);
}
