//! A single-joint skinned cube; no game or generated game assets.
pub fn skinned_model() -> exact_game::asset::Model {
    use exact_game::asset::{Model, Node, Skin};
    let mut model: Model =
        exact_game::bin::from_slice(include_bytes!("../../../bake/tests/fixtures/crate.model"))
            .unwrap();
    let joint = model.nodes.len() as u32;
    model.nodes.push(Node {
        name: "joint".into(),
        ..Default::default()
    });
    for node in &mut model.nodes {
        if node.mesh.is_some() {
            node.skin = Some(0);
        }
    }
    model.skins = vec![Skin {
        joints: vec![joint],
        inverse_binds: exact_game::Mat4::IDENTITY.to_cols_array().to_vec(),
        ..Default::default()
    }];
    for mesh in &mut model.meshes {
        let vertices = mesh.positions.len() / 3;
        mesh.joints = vec![0; vertices * 4];
        mesh.weights = [1., 0., 0., 0.].repeat(vertices);
    }
    model.textures[0] = "fox/0-srgb-straight.tex".into();
    model
}

/// Clip/IK/offset fixture with rotated rest TRS and a mirror under a posed joint.
#[allow(dead_code)]
pub fn posable_model() -> exact_game::asset::Model {
    use exact_game::{asset::*, Mat4, Quat, Vec3};
    let mesh = MeshData {
        positions: vec![0., 0., 0., 0.4, 0., 0., 0., 0.4, 0.],
        normals: [0., 0., 1.].repeat(3),
        uvs: [0.; 2].repeat(3),
        indices: vec![0, 1, 2],
        bounds: [0., 0., 0., 0.4, 0.4, 0.],
        ..Default::default()
    };
    Model {
        meshes: vec![
            mesh.clone(),
            MeshData {
                material: 1,
                ..mesh
            },
        ],
        materials: vec![MaterialData::default(); 2],
        nodes: vec![
            Node {
                name: "root".into(),
                transform: Mat4::from_rotation_z(0.2).to_cols_array(),
                ..Default::default()
            },
            Node {
                name: "arm".into(),
                parent: Some(0),
                posable: true,
                transform: Mat4::from_scale_rotation_translation(
                    Vec3::new(1., 2., 1.),
                    Quat::from_rotation_z(0.3),
                    Vec3::X,
                )
                .to_cols_array(),
                ..Default::default()
            },
            Node {
                name: "hand".into(),
                parent: Some(1),
                mesh: Some(0),
                transform: Mat4::from_translation(Vec3::X).to_cols_array(),
                ..Default::default()
            },
            Node {
                parent: Some(1),
                transform: Mat4::from_scale(Vec3::new(-1., 1., 1.)).to_cols_array(),
                ..Default::default()
            },
            Node {
                name: "eye".into(),
                parent: Some(3),
                mesh: Some(0),
                ..Default::default()
            },
            Node {
                name: "lone".into(),
                parent: Some(3),
                mesh: Some(1),
                ..Default::default()
            },
        ],
        clips: vec![Clip {
            name: "bend".into(),
            tracks: vec![Track {
                node: 0,
                path: TrackPath::Rotation,
                times: vec![0., 1.],
                values: [
                    Quat::from_rotation_z(0.2).to_array(),
                    Quat::from_rotation_z(0.9).to_array(),
                ]
                .concat(),
                ..Default::default()
            }],
            ..Default::default()
        }],
        bounds: [-2., -2., -1., 3., 3., 1.],
        ..Default::default()
    }
}

/// Twenty nodes, six rigid parts and two materials for presentation cost proofs.
#[allow(dead_code)]
pub fn avatar_pose_model() -> exact_game::asset::Model {
    use exact_game::{asset::Node, Mat4, Vec3};
    let mut model = posable_model();
    model.clips.clear();
    model.nodes[1].name = "armL".into();
    for name in ["legL", "legR", "armR", "torso"] {
        model.nodes.push(Node {
            name: name.into(),
            parent: Some(0),
            posable: true,
            transform: Mat4::from_translation(Vec3::Y).to_cols_array(),
            ..Default::default()
        });
    }
    for i in 0..3 {
        model.nodes.push(Node {
            parent: Some(6 + i),
            mesh: Some(0),
            ..Default::default()
        });
    }
    while model.nodes.len() < 20 {
        model.nodes.push(Node {
            parent: Some(0),
            ..Default::default()
        });
    }
    model
}
