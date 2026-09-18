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
