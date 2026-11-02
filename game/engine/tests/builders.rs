use exact_game::*;
#[test]
fn literal_builders_and_tuples_have_identical_saved_rows_hashes_and_agent_state() {
    let mut a = World::new(120, 7);
    let mut b = World::new(120, 7);
    a.prop("plinth-1", Mesh::cylinder(0.9, 0.2))
        .at(2., 0.1, 0.)
        .color("#4d6669");
    let [r, g, blue] =
        [0x4d, 0x66, 0x69].map(|n| exact_color::srgb_to_linear(n as f64 / 255.) as f32);
    b.spawn_named(
        "plinth-1",
        (
            Transform::at(2., 0.1, 0.),
            Mesh::cylinder(0.9, 0.2),
            Material::rgb(r, g, blue),
        ),
    );
    let sprite = a
        .sprite("player", "strip.tex", [24., 32.])
        .at(0., 20.)
        .entity();
    let tuple = b.spawn_named(
        "player",
        (
            Transform::at(0., 20., 0.),
            Sprite::new("strip.tex", [24., 32.]),
        ),
    );
    assert_eq!(sprite, tuple);
    assert_eq!(a.hash(), b.hash());
    assert_eq!(a.save(), b.save());
    assert_eq!(
        json::to_string(&*a.require::<Material>("plinth-1")).unwrap(),
        json::to_string(&*b.require::<Material>("plinth-1")).unwrap()
    );
    assert_eq!(
        json::to_string(&*a.require::<Sprite>("player")).unwrap(),
        json::to_string(&*b.require::<Sprite>("player")).unwrap()
    );
    assert_eq!(
        a.journal().iter().map(|e| &e.line).collect::<Vec<_>>(),
        b.journal().iter().map(|e| &e.line).collect::<Vec<_>>()
    );
    assert_eq!(
        a.prop("default", Mesh::cube(1.)).entity(),
        b.spawn_named(
            "default",
            (Transform::default(), Mesh::cube(1.), Material::default())
        )
    );
    assert_eq!(a.hash(), b.hash());
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        a.prop("bad", Mesh::cube(1.)).color("silver");
    }))
    .is_err());
}

struct Scene<const BUILD: bool>;
impl<const BUILD: bool> Game for Scene<BUILD> {
    const ID: &'static str = "builder-scene";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        if BUILD {
            w.prop("box", Mesh::cube(1.)).at(2., 0.1, 0.).color("#fff");
            w.sprite("player", "strip.tex", [24., 32.]).at(0., 20.);
        } else {
            w.spawn_named(
                "box",
                (
                    Transform::at(2., 0.1, 0.),
                    Mesh::cube(1.),
                    Material::rgb(1., 1., 1.),
                ),
            );
            w.spawn_named(
                "player",
                (
                    Transform::at(0., 20., 0.),
                    Sprite::new("strip.tex", [24., 32.]),
                ),
            );
        }
    }
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
#[test]
fn builders_and_tuples_publish_identical_agent_state() {
    let mut a = Sim::<Scene<true>>::new(()).unwrap();
    let mut b = Sim::<Scene<false>>::new(()).unwrap();
    for request in [
        r#"{"op":"state"}"#,
        r#"{"op":"tree"}"#,
        r#"{"op":"state","entity":"box"}"#,
    ] {
        assert_eq!(a.agent(request), b.agent(request));
    }
}
