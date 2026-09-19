mod support;
use std::fs;
fn temp() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!(
        "asset-outputs-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    fs::create_dir_all(p.join("art")).unwrap();
    p
}
const CRATE: &str = include_str!("fixtures/crate.gltf");
#[test]
fn generated_manifest_prunes_renames_deletions_and_absent_art_without_touching_authored_assets() {
    let app = temp();
    fs::write(app.join("art/crate.gltf"), CRATE).unwrap();
    exact_game_bake::bake_art(&app).unwrap();
    assert!(app.join("assets/crate/0-srgb-straight.tex").exists());
    fs::write(app.join("assets/authored.bin"), b"keep").unwrap();
    fs::rename(app.join("art/crate.gltf"), app.join("art/renamed.gltf")).unwrap();
    exact_game_bake::bake_art(&app).unwrap();
    assert!(!app.join("assets/crate.model").exists());
    assert!(!app.join("assets/crate/0-srgb-straight.tex").exists());
    fs::remove_dir_all(app.join("art")).unwrap();
    exact_game_bake::bake_art(&app).unwrap();
    assert!(!app.join("assets/renamed.model").exists());
    assert!(!app.join("assets/renamed/0-srgb-straight.tex").exists());
    assert_eq!(fs::read(app.join("assets/authored.bin")).unwrap(), b"keep");
    assert!(!app.join(".baked-assets.json").exists());
    exact_game_bake::bake_art(&app).unwrap();
    fs::remove_dir_all(app).unwrap();
}
#[test]
fn authored_collision_and_invalid_stems_refuse_by_name() {
    let app = temp();
    fs::write(app.join("art/crate.gltf"), CRATE).unwrap();
    fs::create_dir_all(app.join("assets")).unwrap();
    fs::write(app.join("assets/crate.model"), b"authored").unwrap();
    let error = exact_game_bake::bake_art(&app).unwrap_err();
    assert!(
        error.contains("crate.model") && error.contains("authored"),
        "{error}"
    );
    assert_eq!(
        fs::read(app.join("assets/crate.model")).unwrap(),
        b"authored"
    );
    fs::rename(app.join("art/crate.gltf"), app.join("art/雪.gltf")).unwrap();
    assert!(exact_game_bake::bake_art(&app).unwrap_err().contains("雪"));
    fs::remove_dir_all(app).unwrap();
}

#[test]
fn sixteen_pixel_crate_pins_model_and_texture_bytes() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/crate.gltf");
    let (model, textures) = exact_game_bake::assets(&path).unwrap();
    assert_eq!(
        exact_game::bin::to_vec(&model),
        include_bytes!("fixtures/crate.model")
    );
    assert_eq!(textures.len(), 1);
    assert_eq!(
        exact_game::bin::to_vec(&textures["crate/0-srgb-straight.tex"]),
        include_bytes!("fixtures/crate/0-srgb-straight.tex")
    );
}

#[test]
fn oversize_texture_refuses_by_name_and_nearest_filters_survive() {
    let app = temp();
    let mut source: serde_json::Value = serde_json::from_str(CRATE).unwrap();
    source["images"][0]["uri"] = serde_json::json!("wide.png");
    image::RgbaImage::new(2049, 1)
        .save(app.join("art/wide.png"))
        .unwrap();
    let path = app.join("art/oversize.gltf");
    fs::write(&path, source.to_string()).unwrap();
    let error = exact_game_bake::assets(&path).unwrap_err();
    assert!(
        error.contains("oversize/0-srgb-straight.tex") && error.contains("2048"),
        "{error}"
    );
    let mut source: serde_json::Value = serde_json::from_str(CRATE).unwrap();
    source["samplers"] = serde_json::json!([{"magFilter":9728,"minFilter":9984}]);
    source["textures"][0]["sampler"] = serde_json::json!(0);
    fs::write(&path, source.to_string()).unwrap();
    let (_, textures) = exact_game_bake::assets(&path).unwrap();
    assert_eq!(
        textures.values().next().unwrap().filter,
        [exact_game::asset::Filter::Nearest; 3]
    );
    fs::remove_dir_all(app).unwrap();
}

#[test]
fn renamed_source_never_prunes_a_replaced_authored_output() {
    let app = temp();
    fs::write(app.join("art/crate.gltf"), CRATE).unwrap();
    exact_game_bake::bake_art(&app).unwrap();
    fs::rename(app.join("art/crate.gltf"), app.join("art/renamed.gltf")).unwrap();
    fs::write(app.join("assets/crate.model"), b"authored replacement").unwrap();
    let error = exact_game_bake::bake_art(&app).unwrap_err();
    assert!(
        error.contains("crate.model") && error.contains("authored"),
        "{error}"
    );
    assert_eq!(
        fs::read(app.join("assets/crate.model")).unwrap(),
        b"authored replacement"
    );
    assert!(!app.join("assets/renamed.model").exists());
    fs::remove_dir_all(app).unwrap();
}

#[test]
fn rebake_never_overwrites_a_replaced_authored_output() {
    let app = temp();
    fs::write(app.join("art/crate.gltf"), CRATE).unwrap();
    exact_game_bake::bake_art(&app).unwrap();
    fs::write(app.join("assets/crate.model"), b"authored replacement").unwrap();
    let error = exact_game_bake::bake_art(&app).unwrap_err();
    assert!(
        error.contains("crate.model") && error.contains("authored"),
        "{error}"
    );
    assert_eq!(
        fs::read(app.join("assets/crate.model")).unwrap(),
        b"authored replacement"
    );
    fs::remove_dir_all(app).unwrap();
}

#[test]
fn cli_and_shell_encoding_share_the_carrier_limit() {
    assert!(exact_game_bake::check_size("huge.model", 64 * 1024 * 1024).is_ok());
    let error = exact_game_bake::check_size("huge.model", 64 * 1024 * 1024 + 1).unwrap_err();
    assert!(error.contains("huge.model") && error.contains("64 MiB"));
}

#[test]
fn content_equal_without_ownership_never_adopts_authored_output() {
    let app = temp();
    fs::write(app.join("art/crate.gltf"), CRATE).unwrap();
    exact_game_bake::bake_art(&app).unwrap();
    fs::remove_file(app.join(".baked-assets.json")).unwrap();
    let error = exact_game_bake::bake_art(&app).unwrap_err();
    assert!(error.contains("authored"), "{error}");
    fs::write(app.join(".baked-assets.json"), r#"{"crate.model":"stale"}"#).unwrap();
    assert!(exact_game_bake::bake_art(&app)
        .unwrap_err()
        .contains("authored"));
    fs::remove_dir_all(app).unwrap();
}
#[test]
fn obsolete_manifest_refuses_before_changing_outputs() {
    let app = temp();
    fs::write(app.join("art/crate.gltf"), CRATE).unwrap();
    exact_game_bake::bake_art(&app).unwrap();
    fs::write(app.join("assets/old.tex"), b"cannot prove ownership").unwrap();
    fs::write(
        app.join(".baked-assets.json"),
        r#"["crate.model","crate/0-srgb-straight.tex","old.tex"]"#,
    )
    .unwrap();
    let before = fs::read(app.join("assets/crate.model")).unwrap();
    let error = exact_game_bake::bake_art(&app).unwrap_err();
    assert!(error.contains("regenerate"), "{error}");
    assert_eq!(fs::read(app.join("assets/crate.model")).unwrap(), before);
    assert_eq!(
        fs::read(app.join("assets/old.tex")).unwrap(),
        b"cannot prove ownership"
    );
    fs::remove_dir_all(app).unwrap();
}

#[test]
fn clean_sprites_checkout_bakes_without_generated_ownership() {
    let app = temp();
    let image = image::RgbaImage::from_fn(8, 2, |x, y| {
        image::Rgba([x as u8 * 20, y as u8 * 100, 128, 255])
    });
    image.save(app.join("art/strip.png")).unwrap();
    assert!(!app.join(".baked-assets.json").exists());
    exact_game_bake::bake_art(&app).unwrap();
    let texture: exact_game::asset::TextureData =
        exact_game::bin::from_slice(&fs::read(app.join("assets/strip.tex")).unwrap()).unwrap();
    assert_eq!((texture.width, texture.height), (8, 2));
    assert_eq!(texture.mips[0], image.into_raw());
    fs::remove_dir_all(app).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn png_non_utf8_stem_returns_an_error() {
    use std::os::unix::ffi::OsStrExt;
    let app = temp();
    fs::write(
        app.join("art")
            .join(std::ffi::OsStr::from_bytes(b"bad\xff.png")),
        b"unused",
    )
    .unwrap();
    assert_eq!(
        exact_game_bake::bake_art(&app).unwrap_err(),
        "invalid art stem"
    );
    fs::remove_dir_all(app).unwrap();
}

#[test]
fn source_units_normalize_geometry_and_real_test_payloads() {
    let app = temp();
    let path = app.join("art/crate.gltf");
    let mut source: serde_json::Value = serde_json::from_str(CRATE).unwrap();
    fs::write(&path, source.to_string()).unwrap();
    let before = exact_game_bake::model(&path).unwrap();
    source["asset"]["extras"] = serde_json::json!({"metersPerUnit":0.025});
    fs::write(&path, source.to_string()).unwrap();
    let data = support::assets(&path).unwrap();
    let after: exact_game::asset::Model =
        exact_game::bin::from_slice(&data["crate.model"]).unwrap();
    for (a, b) in after.meshes[0]
        .positions
        .iter()
        .zip(&before.meshes[0].positions)
    {
        assert_eq!(*a, *b * 0.025);
    }
    assert_eq!(after.bounds, before.bounds.map(|v| v * 0.025));
    assert!(data.contains_key(&after.textures[0]));
    source["asset"]["extras"]["metersPerUnit"] = serde_json::json!(0);
    fs::write(&path, source.to_string()).unwrap();
    assert!(exact_game_bake::model(&path)
        .unwrap_err()
        .contains("metersPerUnit"));
    fs::remove_dir_all(app).unwrap();
}
