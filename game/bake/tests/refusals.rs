use serde_json::json;
fn source() -> serde_json::Value {
    serde_json::from_str(include_str!("fixtures/crate.gltf")).unwrap()
}
fn bake(value: serde_json::Value) -> Result<exact_game::asset::Model, String> {
    let dir = std::env::temp_dir().join(format!(
        "exact-bake-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("fixture.gltf");
    std::fs::write(&path, value.to_string()).unwrap();
    let result = exact_game_bake::model(&path);
    std::fs::remove_dir_all(dir).unwrap();
    result
}
#[test]
fn unsupported_features_refuse_by_name() {
    let mut v = source();
    v["extensionsUsed"] = json!(["KHR_materials_unlit"]);
    assert!(bake(v).unwrap_err().contains("KHR_materials_unlit"));
    let mut v = source();
    v["meshes"][0]["primitives"][0]["targets"] = json!([{"POSITION":0}]);
    assert!(bake(v).unwrap_err().contains("morph targets"));
    let mut v = source();
    v["accessors"][0]["sparse"] = json!({"count":1,"indices":{"bufferView":3,"componentType":5123},"values":{"bufferView":0}});
    assert!(bake(v).unwrap_err().contains("sparse accessor"));
}
#[test]
fn permitted_extensions_are_baked_per_texture() {
    let mut v = source();
    v["extensionsUsed"] = json!(["KHR_materials_emissive_strength", "KHR_texture_transform"]);
    v["materials"][0]["emissiveFactor"] = json!([0.2, 0.3, 0.4]);
    v["materials"][0]["extensions"] =
        json!({"KHR_materials_emissive_strength":{"emissiveStrength":4.0}});
    v["materials"][0]["pbrMetallicRoughness"]["baseColorTexture"]["extensions"] =
        json!({"KHR_texture_transform":{"offset":[0.25,0.5],"scale":[2.0,3.0]}});
    let m = bake(v).unwrap();
    assert_eq!(m.materials[0].emissive, [0.8, 1.2, 1.6]);
    assert_eq!(m.materials[0].uv_transforms[0], [2., 0., 0., 3., 0.25, 0.5]);
    assert!(m.textures[0].starts_with("textures/") && m.textures[0].ends_with(".tex"));
}

#[test]
fn scene_selection_and_textured_uvs_are_not_silent() {
    let mut failures = Vec::new();
    let mut v = source();
    v["scenes"] = json!([{ "nodes": [0] }, { "nodes": [0] }]);
    if bake(v).is_ok() {
        failures.push("multiple scenes");
    }
    let mut v = source();
    v.as_object_mut().unwrap().remove("scene");
    if bake(v).is_ok() {
        failures.push("no default scene");
    }
    let mut v = source();
    v["meshes"][0]["primitives"][0]["attributes"]
        .as_object_mut()
        .unwrap()
        .remove("TEXCOORD_0");
    if bake(v).is_ok() {
        failures.push("textured mesh without UVs");
    }
    let mut v = source();
    v["nodes"]
        .as_array_mut()
        .unwrap()
        .push(json!({"mesh":0,"translation":[100,0,0]}));
    if bake(v).unwrap().bounds[3] > 10. {
        failures.push("orphan rendered");
    }
    assert!(failures.is_empty(), "accepted: {failures:?}");
}

#[test]
fn cubic_tracks_preserve_gltf_neighbour_tangents_through_bake() {
    use exact_game::{animation, asset::Interpolation};
    let mut v = source();
    let times = [0f32, 2., 5.];
    // Distinct unused/end and inner tangents make every wrong neighbour observable.
    let values: Vec<f32> = [99., 0., 3., 7., 4., 11., 13., 8., 77.]
        .into_iter()
        .flat_map(|x| [x, 0., 0.])
        .collect();
    let dir = std::env::temp_dir().join(format!("exact-cubic-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let bytes: Vec<u8> = times
        .iter()
        .chain(&values)
        .flat_map(|f| f.to_le_bytes())
        .collect();
    std::fs::write(dir.join("motion.bin"), &bytes).unwrap();
    let buffer = v["buffers"].as_array().unwrap().len();
    v["buffers"]
        .as_array_mut()
        .unwrap()
        .push(json!({"byteLength":bytes.len(),"uri":"motion.bin"}));
    let view = v["bufferViews"].as_array().unwrap().len();
    v["bufferViews"].as_array_mut().unwrap().extend([
        json!({"buffer":buffer,"byteLength":12}),
        json!({"buffer":buffer,"byteOffset":12,"byteLength":values.len()*4}),
    ]);
    let accessor = v["accessors"].as_array().unwrap().len();
    v["accessors"].as_array_mut().unwrap().extend([
        json!({"bufferView":view,"componentType":5126,"count":3,"type":"SCALAR","min":[0],"max":[5]}),
        json!({"bufferView":view+1,"componentType":5126,"count":9,"type":"VEC3"})]);
    v["animations"] = json!([{"name":"cubic","samplers":[{"input":accessor,"output":accessor+1,"interpolation":"CUBICSPLINE"}],"channels":[{"sampler":0,"target":{"node":0,"path":"translation"}}]}]);
    std::fs::write(dir.join("cubic.gltf"), v.to_string()).unwrap();
    let model = exact_game_bake::model(dir.join("cubic.gltf")).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
    assert!(matches!(
        model.clips[0].tracks[0].interpolation,
        Interpolation::CubicSpline
    ));
    assert_eq!(model.clips[0].tracks[0].values, values);
    let mut pose = vec![];
    animation::sample(
        &model.clips[0],
        2.75,
        &animation::bind_pose(&model),
        &mut pose,
    );
    assert_eq!(
        pose[0],
        0.84375 * 4. + 0.140625 * 3. * 11. + 0.15625 * 8. - 0.046875 * 3. * 13.
    );
}

#[test]
fn vertex_colors_survive_bake_and_merge_with_white_defaults() {
    let mut v = source();
    // A separate RGBA float accessor, shared by all 24 vertices.
    v["buffers"].as_array_mut().unwrap().push(json!({"byteLength":384,"uri":"data:application/octet-stream;base64,AACAPgAAAD8AAEA/AACAPwAAgD4AAAA/AABAPwAAgD8AAIA+AAAAPwAAQD8AAIA/AACAPgAAAD8AAEA/AACAPwAAgD4AAAA/AABAPwAAgD8AAIA+AAAAPwAAQD8AAIA/AACAPgAAAD8AAEA/AACAPwAAgD4AAAA/AABAPwAAgD8AAIA+AAAAPwAAQD8AAIA/AACAPgAAAD8AAEA/AACAPwAAgD4AAAA/AABAPwAAgD8AAIA+AAAAPwAAQD8AAIA/AACAPgAAAD8AAEA/AACAPwAAgD4AAAA/AABAPwAAgD8AAIA+AAAAPwAAQD8AAIA/AACAPgAAAD8AAEA/AACAPwAAgD4AAAA/AABAPwAAgD8AAIA+AAAAPwAAQD8AAIA/AACAPgAAAD8AAEA/AACAPwAAgD4AAAA/AABAPwAAgD8AAIA+AAAAPwAAQD8AAIA/AACAPgAAAD8AAEA/AACAPwAAgD4AAAA/AABAPwAAgD8AAIA+AAAAPwAAQD8AAIA/"}));
    v["bufferViews"]
        .as_array_mut()
        .unwrap()
        .push(json!({"buffer":1,"byteLength":384}));
    v["accessors"]
        .as_array_mut()
        .unwrap()
        .push(json!({"bufferView":4,"componentType":5126,"count":24,"type":"VEC4"}));
    let plain = v["meshes"][0]["primitives"][0].clone();
    v["meshes"][0]["primitives"][0]["attributes"]["COLOR_0"] = json!(4);
    v["meshes"][0]["primitives"]
        .as_array_mut()
        .unwrap()
        .push(plain);
    let baked = bake(v).unwrap();
    assert_eq!(baked.meshes.len(), 1);
    assert_eq!(
        &baked.meshes[0].colors[..96],
        &[0.25, 0.5, 0.75, 1.].repeat(24)
    );
    assert_eq!(&baked.meshes[0].colors[96..], &[1.; 96]);
}
