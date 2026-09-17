use exact_game::{Transform, World};
#[test]
fn renderer_copies_present_transform_runs() {
    let mut w = World::new(60, 0);
    let entities: Vec<_> = (0..2100)
        .map(|i| w.spawn(Transform::at(i as f32, 2.0, 3.0)))
        .collect();
    for &i in &[0, 5, 63, 64, 1023, 1024, 2048, 2099] {
        w.remove::<Transform>(entities[i]);
    }
    let pages = w.pages::<Transform>();
    let mut copied = 0;
    for page in pages.iter() {
        for (first, floats) in page.float_runs() {
            assert_eq!(floats.len() % 10, 0);
            for (i, t) in floats.chunks_exact(10).enumerate() {
                let value = w.get::<Transform>(entities[first as usize + i]).unwrap();
                assert_eq!(&t[0..3], value.position.to_array());
                assert_eq!(&t[3..7], value.rotation.to_array());
                assert_eq!(&t[7..10], value.scale.to_array());
                copied += 1;
            }
        }
    }
    assert_eq!(copied, 2092);
}
