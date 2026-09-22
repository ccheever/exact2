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
