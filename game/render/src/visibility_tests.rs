use exact_game::{Camera, Emitter, Parent, Placed, Sprite, Transform, Visible, World};

#[test]
fn sprites_emitters_and_placed_children_hide_and_reveal_without_resetting_state() {
    let mut w = World::new(60, 0);
    let root = w.spawn(Transform::default());
    let e = w.spawn((
        Transform::at(0., 0., -2.),
        Parent(root),
        Sprite::new("sample.tex", [1., 1.]),
        Emitter::default(),
        Placed::child(0),
    ));
    let camera = w.spawn((Transform::default(), Camera::default(), Visible(false)));
    w.propagate();
    let mut sprites = Vec::new();
    let mut emitters = Vec::new();
    let mut placements = crate::placed::Placements::default();
    placements.child(0, "", None, [0., 0., 100., 50.]);
    for (visible, births) in [(true, 0), (false, 1), (true, 2)] {
        w.insert(root, Visible(visible));
        // The simulation continues producing births while presentation is hidden.
        w.get_mut::<Emitter>(e)
            .unwrap()
            .state
            .births
            .resize(births, Default::default());
        crate::quads::feed::<Sprite>(&w, &mut sprites, false, false, false);
        crate::quads::feed::<Emitter>(&w, &mut emitters, false, false, false);
        placements.feed(&w).unwrap();
        placements.headless(glam::Vec2::splat(200.), 1.);
        assert_eq!(sprites.len(), usize::from(visible));
        assert_eq!(emitters.len(), usize::from(visible));
        assert_eq!(placements.placement(0).unwrap().hidden, !visible);
        if visible {
            assert_eq!(emitters[0].value.state.births.len(), births);
        }
        assert_eq!(w.get::<Emitter>(e).unwrap().state.births.len(), births);
        assert!(!w.is_visible(camera)); // Still selected by headless placements.
    }
    let hidden = w.spawn((Transform::default(), Visible(false)));
    w.insert(e, Parent(hidden));
    w.propagate();
    crate::quads::feed::<Sprite>(&w, &mut sprites, false, false, true);
    crate::quads::feed::<Emitter>(&w, &mut emitters, false, false, true);
    placements.feed(&w).unwrap();
    placements.headless(glam::Vec2::splat(200.), 1.);
    assert!(sprites.is_empty() && emitters.is_empty());
    assert!(placements.placement(0).unwrap().hidden);
}
