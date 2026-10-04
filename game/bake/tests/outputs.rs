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
/// The crate's one texture, named by its content.
const CRATE_TEX: &str = "textures/86b6f64da4937176.tex";
#[test]
fn app_cli_bakes_paths_with_spaces_and_refuses_authored_replacements() {
    let app = std::env::temp_dir().join(format!(
        "asset CLI outputs {} {:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    fs::create_dir_all(app.join("art")).unwrap();
    fs::write(app.join("art/crate.gltf"), CRATE).unwrap();
    let run = || {
        std::process::Command::new(env!("CARGO_BIN_EXE_exact-game-bake"))
            .arg("--art")
            .arg(&app)
            .output()
            .unwrap()
    };
    let output = run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(app.join("assets/crate.model").exists());
    assert!(app.join(".baked-assets.json").exists());
    fs::write(app.join("assets/crate.model"), b"authored replacement").unwrap();
    let output = run();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("authored asset"));
    assert_eq!(
        fs::read(app.join("assets/crate.model")).unwrap(),
        b"authored replacement"
    );
    fs::remove_dir_all(app).unwrap();
}
#[test]
fn generated_manifest_prunes_renames_deletions_and_absent_art_without_touching_authored_assets() {
    let app = temp();
    fs::write(app.join("art/crate.gltf"), CRATE).unwrap();
    exact_game_bake::bake_art(&app).unwrap();
    assert!(app.join("assets").join(CRATE_TEX).exists());
    fs::write(app.join("assets/authored.bin"), b"keep").unwrap();
    fs::rename(app.join("art/crate.gltf"), app.join("art/renamed.gltf")).unwrap();
    exact_game_bake::bake_art(&app).unwrap();
    assert!(!app.join("assets/crate.model").exists());
    // The renamed model's texture keeps its content name.
    assert!(app.join("assets").join(CRATE_TEX).exists());
    fs::remove_dir_all(app.join("art")).unwrap();
    exact_game_bake::bake_art(&app).unwrap();
    assert!(!app.join("assets/renamed.model").exists());
    assert!(!app.join("assets").join(CRATE_TEX).exists());
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
    use exact_game::asset::{TextureFamily, TextureFormat};
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/crate.gltf");
    let (model, textures) = exact_game_bake::assets(&path).unwrap();
    assert_eq!(
        exact_game::bin::to_vec(&model),
        include_bytes!("fixtures/crate.model")
    );
    assert_eq!(model.textures, [CRATE_TEX]);
    let rgba = &textures[CRATE_TEX];
    assert_eq!(
        exact_game::bin::to_vec(rgba),
        include_bytes!("fixtures/crate/0-srgb-straight.tex")
    );
    // Encoders may differ bit-wise across SIMD variants: pin structure and quality.
    for (family, format) in [
        (TextureFamily::Bc, TextureFormat::Bc7),
        (TextureFamily::Astc, TextureFormat::Astc4x4),
    ] {
        let name = family.name(CRATE_TEX);
        let t = &textures[&name];
        assert_eq!(
            (t.format, t.width, t.height, t.srgb),
            (format, 16, 16, true)
        );
        assert_eq!((t.wrap, t.filter), (rgba.wrap, rgba.filter));
        t.validate().unwrap();
        let decoded = exact_game_bake::compress::decode(t, 0);
        let db = exact_game_bake::compress::psnr(&rgba.mips[0], &decoded, &[0, 1, 2]);
        assert!(db > 40., "{name}: {db:.1} dB");
    }
    assert_eq!(textures.len(), 3);
}

#[test]
fn block_textures_reach_4096_and_rgba8_payloads_stop_at_2048() {
    use exact_game::asset::{TextureFamily, TextureFormat};
    let app = temp();
    let mut source: serde_json::Value = serde_json::from_str(CRATE).unwrap();
    source["images"][0]["uri"] = serde_json::json!("wide.png");
    let path = app.join("art/wide.gltf");
    image::RgbaImage::from_fn(4096, 4, |x, y| {
        image::Rgba([(x / 16) as u8, (y * 60) as u8, 200, 255])
    })
    .save(app.join("art/wide.png"))
    .unwrap();
    fs::write(&path, source.to_string()).unwrap();
    let (model, textures) = exact_game_bake::assets(&path).unwrap();
    let wide = &model.textures[0];
    let rgba = &textures[wide];
    assert_eq!(
        (rgba.format, rgba.width, rgba.height),
        (TextureFormat::Rgba8, 2048, 2)
    );
    assert_eq!(rgba.mips.len(), 12);
    for family in [TextureFamily::Bc, TextureFamily::Astc] {
        let name = family.name(wide);
        let t = &textures[&name];
        assert_ne!(t.format, TextureFormat::Rgba8, "{name}");
        assert_eq!((t.width, t.height, t.mips.len()), (4096, 4, 13), "{name}");
        // The fallback's levels are the block chain's tail.
        let decoded = exact_game_bake::compress::decode(t, 1);
        let db = exact_game_bake::compress::psnr(&decoded, &rgba.mips[0], &[0, 1, 2]);
        assert!(db > 40., "{name}: {db:.1} dB");
    }
    // Whole blocks are WebGPU's rule: other sizes ship RGBA8 in every file.
    image::RgbaImage::from_pixel(4094, 6, image::Rgba([90, 140, 200, 255]))
        .save(app.join("art/wide.png"))
        .unwrap();
    let (_, textures) = exact_game_bake::assets(&path).unwrap();
    for t in textures.values() {
        assert_eq!(
            (t.format, t.width, t.height),
            (TextureFormat::Rgba8, 2047, 3)
        );
    }
    fs::remove_dir_all(app).unwrap();
}

#[test]
fn sprites_use_blocks_only_where_they_decode_exactly() {
    use exact_game::asset::TextureFormat;
    let app = temp();
    let tiles = image::RgbaImage::from_pixel(8, 8, image::Rgba([200, 30, 90, 255]));
    let noisy = image::RgbaImage::from_fn(8, 8, |x, y| {
        image::Rgba([
            (x * 37) as u8,
            (y * 53) as u8,
            ((x * y * 91) % 256) as u8,
            200,
        ])
    });
    for (stem, image) in [("tiles", &tiles), ("noisy", &noisy)] {
        image.save(app.join(format!("art/{stem}.png"))).unwrap();
    }
    exact_game_bake::bake_art(&app).unwrap();
    let read = |name: &str| -> exact_game::asset::TextureData {
        exact_game::bin::from_slice(&fs::read(app.join("assets").join(name)).unwrap()).unwrap()
    };
    // A solid colour is ASTC's void-extent block at every level: exact, so compressed.
    assert_eq!(read("tiles.astc.tex").format, TextureFormat::Astc4x4);
    for stem in ["tiles", "noisy"] {
        let authored = read(&format!("{stem}.tex"));
        for family in ["bc", "astc"] {
            let t = read(&format!("{stem}.{family}.tex"));
            for level in 0..t.mips.len() {
                assert_eq!(
                    exact_game_bake::compress::decode(&t, level),
                    authored.mips[level],
                    "{stem}.{family}.tex level {level} keeps the palette"
                );
            }
        }
    }
    assert_eq!(read("noisy.astc.tex").format, TextureFormat::Rgba8);
    assert_eq!(read("noisy.bc.tex").format, TextureFormat::Rgba8);
    fs::remove_dir_all(app).unwrap();
}

#[test]
fn oversize_texture_refuses_by_name_and_nearest_filters_survive() {
    let app = temp();
    let mut source: serde_json::Value = serde_json::from_str(CRATE).unwrap();
    source["images"][0]["uri"] = serde_json::json!("wide.png");
    image::RgbaImage::new(4097, 1)
        .save(app.join("art/wide.png"))
        .unwrap();
    let path = app.join("art/oversize.gltf");
    fs::write(&path, source.to_string()).unwrap();
    let error = exact_game_bake::assets(&path).unwrap_err();
    assert!(
        error.contains("oversize/0-srgb-straight.tex") && error.contains("4096"),
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
        r#"["crate.model","textures/86b6f64da4937176.tex","old.tex"]"#,
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

#[test]
fn masked_coverage_survives_block_encoding() {
    let app = temp();
    let mut source: serde_json::Value = serde_json::from_str(CRATE).unwrap();
    source["images"][0]["uri"] = serde_json::json!("leaf.png");
    source["materials"][0]["alphaMode"] = serde_json::json!("MASK");
    // A soft-edged disc: the kind of foliage cut-out whose mips thin without care.
    image::RgbaImage::from_fn(64, 64, |x, y| {
        let d = ((x as f32 - 31.5).powi(2) + (y as f32 - 31.5).powi(2)).sqrt();
        image::Rgba([
            60,
            140,
            40,
            (255. * (1. - (d - 20.) / 12.).clamp(0., 1.)) as u8,
        ])
    })
    .save(app.join("art/leaf.png"))
    .unwrap();
    let path = app.join("art/leaf.gltf");
    fs::write(&path, source.to_string()).unwrap();
    let (model, textures) = exact_game_bake::assets(&path).unwrap();
    let name = &model.textures[0];
    let stem = name.strip_suffix(".tex").unwrap();
    let rgba = &textures[name];
    let covered = |texels: &[u8]| {
        let n = texels.chunks_exact(4).filter(|p| p[3] >= 128).count();
        n as f64 / (texels.len() / 4) as f64
    };
    for family in ["bc", "astc"] {
        let t = &textures[&format!("{stem}.{family}.tex")];
        for level in 0..t.mips.len() - 2 {
            let expected = covered(&rgba.mips[level]);
            let actual = covered(&exact_game_bake::compress::decode(t, level));
            eprintln!("{family} level {level}: {actual:.3} vs {expected:.3}");
            assert!(
                (actual - expected).abs() <= 1. / 16.,
                "{family} level {level}: coverage {actual:.3} vs {expected:.3}"
            );
        }
    }
    fs::remove_dir_all(app).unwrap();
}

#[test]
fn identical_model_textures_bake_once_under_their_content_name() {
    let app = temp();
    // Same texels and sampler in two models; a third clamps, so it differs.
    let clamped = CRATE
        .replacen("\"source\": 0", "\"source\": 0, \"sampler\": 0", 1)
        .replacen(
            "\"textures\": [",
            "\"samplers\": [{ \"wrapS\": 33071 }],\n  \"textures\": [",
            1,
        );
    fs::write(app.join("art/crate.gltf"), CRATE).unwrap();
    fs::write(app.join("art/fence.gltf"), &clamped).unwrap();
    exact_game_bake::bake_art(&app).unwrap();
    let model = |name: &str| {
        let bytes = fs::read(app.join("assets").join(name)).unwrap();
        exact_game::bin::from_slice::<exact_game::asset::Model>(&bytes).unwrap()
    };
    let fence = model("fence.model").textures;
    assert_eq!(model("crate.model").textures, [CRATE_TEX]);
    assert_ne!(fence, [CRATE_TEX]);
    let files = |app: &std::path::Path| {
        let mut names: Vec<_> = fs::read_dir(app.join("assets/textures"))
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    };
    let before = files(&app);
    // A model sharing the crate's texture adds no file and renames none.
    fs::write(app.join("art/barrel.gltf"), CRATE).unwrap();
    exact_game_bake::bake_art(&app).unwrap();
    assert_eq!(model("barrel.model").textures, [CRATE_TEX]);
    assert_eq!(model("fence.model").textures, fence);
    assert_eq!(files(&app), before);
    for family in ["tex", "bc.tex", "astc.tex"] {
        let name = CRATE_TEX.replace(".tex", &format!(".{family}"));
        assert!(app.join("assets").join(name).exists());
    }
    // Removing one sharer keeps the file for the other.
    fs::remove_file(app.join("art/crate.gltf")).unwrap();
    exact_game_bake::bake_art(&app).unwrap();
    assert!(app.join("assets").join(CRATE_TEX).exists());
    fs::remove_dir_all(app).unwrap();
}

#[test]
fn one_model_using_one_image_twice_lists_it_once() {
    let mut source: serde_json::Value = serde_json::from_str(CRATE).unwrap();
    // A second glTF texture over the same image, sampled as emission (also sRGB colour).
    source["textures"] = serde_json::json!([{"source": 0}, {"source": 0}]);
    source["materials"][0]["emissiveTexture"] = serde_json::json!({"index": 1});
    source["materials"][0]["emissiveFactor"] = serde_json::json!([1.0, 1.0, 1.0]);
    let app = temp();
    let path = app.join("art/twice.gltf");
    fs::write(&path, source.to_string()).unwrap();
    let (model, _) = exact_game_bake::assets(&path).unwrap();
    assert_eq!(model.textures, [CRATE_TEX]);
    let m = &model.materials[0];
    assert_eq!(
        (m.base_color_texture, m.emissive_texture),
        (Some(0), Some(0))
    );
    fs::remove_dir_all(app).unwrap();
}

#[test]
fn gltf_images_under_art_textures_sample_the_one_standalone_texture() {
    let app = temp();
    fs::create_dir_all(app.join("art/textures")).unwrap();
    fs::create_dir_all(app.join("art/models")).unwrap();
    image::RgbaImage::from_pixel(16, 16, image::Rgba([120, 90, 60, 255]))
        .save(app.join("art/textures/soil.png"))
        .unwrap();
    let mut source: serde_json::Value = serde_json::from_str(CRATE).unwrap();
    source["images"][0]["uri"] = serde_json::json!("../textures/soil.png");
    for stem in ["bed", "pot"] {
        fs::write(
            app.join(format!("art/models/{stem}.gltf")),
            source.to_string(),
        )
        .unwrap();
    }
    let model = exact_game_bake::model(app.join("art/models/bed.gltf")).unwrap();
    assert_eq!(model.textures, ["soil.tex"]);
    exact_game_bake::bake_art(&app).unwrap();
    assert!(app.join("assets/soil.tex").exists());
    assert!(app.join("assets/pot.model").exists());
    for stem in ["bed", "pot"] {
        assert!(
            !app.join(format!("assets/{stem}")).exists(),
            "{stem} embeds a copy"
        );
    }
    // A colour texture cannot be a model's normal map.
    let mut normal = source.clone();
    normal["materials"][0]["normalTexture"] = serde_json::json!({ "index": 0 });
    fs::write(app.join("art/models/bed.gltf"), normal.to_string()).unwrap();
    let error = exact_game_bake::model(app.join("art/models/bed.gltf")).unwrap_err();
    assert!(error.contains("art/textures/"), "{error}");
    fs::remove_dir_all(app).unwrap();
}
