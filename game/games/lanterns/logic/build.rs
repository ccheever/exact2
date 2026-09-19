fn main() {
    exact_game_bake::bake_art("..").expect("bake Lanterns art");
    let digests: std::collections::BTreeMap<String, String> =
        serde_json::from_slice(&std::fs::read("../.baked-assets.json").expect("asset manifest"))
            .expect("asset digests");
    println!("cargo:rustc-env=FOX_MODEL_SHA256={}", digests["fox.model"]);
}
