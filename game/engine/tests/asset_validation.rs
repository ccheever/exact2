use exact_game::asset::*;

#[test]
fn malformed_models_refuse_every_simulation_and_upload_hazard() {
    let mut failures = Vec::new();
    let mut check = |name: &str, m: Model| {
        if m.validate().is_ok() {
            failures.push(name.to_owned());
        }
    };
    let mut m = Model::default();
    m.materials.push(MaterialData {
        roughness: f32::NAN,
        ..Default::default()
    });
    check("material scalar", m);
    let m = Model {
        bounds: [1., 0., 0., -1., 0., 0.],
        ..Default::default()
    };
    check("ordered bounds", m);
    let mut m = Model::default();
    m.nodes.push(Node {
        name: "flattened".into(),
        transform: [0.; 16],
        ..Default::default()
    });
    check("singular node", m);
    for (name, track) in [
        (
            "clip node",
            Track {
                node: 1,
                times: vec![0.],
                values: vec![0.; 3],
                ..Default::default()
            },
        ),
        (
            "clip arity",
            Track {
                times: vec![0.],
                values: vec![0.; 2],
                ..Default::default()
            },
        ),
        (
            "clip time",
            Track {
                times: vec![1., 0.],
                values: vec![0.; 6],
                ..Default::default()
            },
        ),
        (
            "clip finite",
            Track {
                times: vec![0.],
                values: vec![f32::NAN; 3],
                ..Default::default()
            },
        ),
    ] {
        let mut m = Model::default();
        m.nodes.push(Node::default());
        m.clips.push(Clip {
            name: name.into(),
            tracks: vec![track],
            markers: Vec::new(),
        });
        check(name, m);
    }
    let mut m = Model::default();
    m.nodes.push(Node::default());
    m.skins.push(Skin {
        name: "bad bind".into(),
        joints: vec![0],
        inverse_binds: vec![f32::NAN; 16],
    });
    check("inverse bind", m);
    assert!(failures.is_empty(), "accepted: {failures:?}");
}

#[test]
fn joints_and_weights_are_paired_and_mesh_bounds_are_ordered() {
    let model: Model =
        exact_game::bin::from_slice(include_bytes!("../../bake/tests/fixtures/crate.model"))
            .unwrap();
    let mut joints = model.clone();
    joints.meshes[0].joints = vec![0; joints.meshes[0].positions.len() / 3 * 4];
    assert!(joints.validate().is_err());
    let mut bounds = model;
    bounds.meshes[0].bounds[0] = 100.;
    assert!(bounds.validate().is_err());
}

#[test]
fn triangle_indices_cannot_leave_a_trailing_partial_face() {
    let model: Model =
        exact_game::bin::from_slice(include_bytes!("../../bake/tests/fixtures/crate.model"))
            .unwrap();
    model.validate().unwrap();
    for trailing in [1, 2] {
        let mut bad = model.clone();
        bad.meshes[0].indices.extend(vec![0; trailing]);
        assert_eq!(
            bad.validate().unwrap_err(),
            "model: invalid mesh upload range"
        );
    }
}

#[test]
fn clip_scale_preserves_rest_winding_through_keys_and_cubic_overshoot() {
    use exact_game::{Mat4, Vec3};
    for mirrored in [false, true] {
        let sign = if mirrored { -1. } else { 1. };
        let mut model = Model {
            nodes: vec![
                Node {
                    name: "ancestor".into(),
                    transform: Mat4::from_scale(Vec3::new(sign, 1., 1.)).to_cols_array(),
                    ..Default::default()
                },
                Node {
                    name: "joint".into(),
                    parent: Some(0),
                    posable: true,
                    ..Default::default()
                },
            ],
            clips: vec![Clip {
                name: "scale-flip".into(),
                tracks: vec![Track {
                    node: 0,
                    path: TrackPath::Scale,
                    times: vec![0., 1.],
                    values: vec![sign, 1., 1., sign * 2., 1., 1.],
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        for mode in [
            Interpolation::Linear,
            Interpolation::Step,
            Interpolation::CubicSpline,
        ] {
            let track = &mut model.clips[0].tracks[0];
            track.interpolation = mode;
            track.values = if matches!(mode, Interpolation::CubicSpline) {
                vec![
                    0.,
                    0.,
                    0.,
                    sign,
                    1.,
                    1.,
                    0.,
                    0.,
                    0.,
                    0.,
                    0.,
                    0.,
                    sign * 2.,
                    1.,
                    1.,
                    0.,
                    0.,
                    0.,
                ]
            } else {
                vec![sign, 1., 1., sign * 2., 1., 1.]
            };
            model.validate().unwrap();
            let original = model.clips[0].tracks[0].values.clone();
            let at = if matches!(mode, Interpolation::CubicSpline) {
                12
            } else {
                3
            };
            for value in [0., -sign] {
                model.clips[0].tracks[0].values[at] = value;
                let error = model.validate().unwrap_err();
                assert!(
                    error.contains("scale-flip")
                        && error.contains("ancestor")
                        && error.contains("rest-axis sign"),
                    "{error}"
                );
            }
            model.clips[0].tracks[0].values = original;
            if matches!(mode, Interpolation::CubicSpline) {
                // Positive endpoint values do not exclude a negative interior.
                model.clips[0].tracks[0].values[6] = -12. * sign;
                model.clips[0].tracks[0].values[9] = 12. * sign;
                assert!(model.validate().unwrap_err().contains("scale-flip"));
            }
        }
    }
}
