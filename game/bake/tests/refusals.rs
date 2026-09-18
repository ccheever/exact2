use serde_json::json;
fn source() -> serde_json::Value {
    serde_json::from_str(include_str!("../../games/asset-fixture/art/crate.gltf")).unwrap()
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
    assert!(m.textures[0].srgb);
}
