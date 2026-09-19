fn main() {
    exact_game_bake::bake_art("..").expect("bake Lanterns art");
    let digests: std::collections::BTreeMap<String, String> =
        serde_json::from_slice(&std::fs::read("../.baked-assets.json").expect("asset manifest"))
            .expect("asset digests");
    println!("cargo:rustc-env=FOX_MODEL_SHA256={}", digests["fox.model"]);

    println!("cargo:rerun-if-changed=../game.rs");
    let original = std::fs::read_to_string("../game.rs").unwrap();
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    for (name, edits) in [
        (
            "physics",
            vec![
                ("6.4", "8.0"),
                ("12.0 * dt", "18.0 * dt"),
                ("-12.0, 0.0", "-18.0, 0.0"),
            ],
        ),
        (
            "clip",
            vec![
                ("\"Run\"", "\"Walk\""),
                ("Animation::play(\"Survey\")", "Animation::play(\"Walk\")"),
                (
                    "if !grounded || speed >= 2.8 {\n        \"Walk\"\n    } else if speed > 0.08",
                    "if !grounded || speed > 0.08",
                ),
            ],
        ),
        ("appearance", vec![("glow * 5.0", "glow * 9.0")]),
    ] {
        let mut source = original.split("#[cfg(test)]").next().unwrap().to_owned();
        for (from, to) in edits {
            assert!(source.contains(from), "missing I3 edit anchor {from}");
            source = source.replace(from, to);
        }
        std::fs::write(output.join(format!("i3_{name}.rs")), source).unwrap();
    }
}
