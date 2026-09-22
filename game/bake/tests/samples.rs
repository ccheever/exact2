//! Khronos glTF-Sample-Assets, fetched into an external cache, never vendored.
//! BoxTextured: Cesium, CC BY 4.0. Fox: PixelMannen, CC0; conversion by @tommy3b.
//! DamagedHelmet: ctxwing (2018), CC BY 4.0; original theblueturtle_ (2016), CC BY-NC 4.0.
//! Metadata/licences: https://github.com/KhronosGroup/glTF-Sample-Assets/tree/main/Models
use exact_game::{asset::Model, bin};
use std::{path::PathBuf, process::Command};
pub fn sample(name: &str) -> Model {
    let cache = PathBuf::from(std::env::var_os("HOME").unwrap())
        .join("Library/Caches/exact2-game/gltf-samples");
    std::fs::create_dir_all(&cache).unwrap();
    let path = cache.join(format!("{name}.glb"));
    if !path.exists() {
        let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
        let url=format!("https://raw.githubusercontent.com/KhronosGroup/glTF-Sample-Assets/main/Models/{name}/glTF-Binary/{name}.glb");
        assert!(Command::new("curl")
            .args([
                "--fail",
                "--location",
                "--silent",
                "--show-error",
                "--max-time",
                "50",
                "--output"
            ])
            .arg(&tmp)
            .arg(url)
            .status()
            .unwrap()
            .success());
        std::fs::rename(tmp, &path).unwrap();
    }
    let digest = Command::new("shasum")
        .args(["-a", "256"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(digest.status.success());
    let expected = match name {
        "BoxTextured" => "b510eca2e2ef33f62f9ed57d6e7ce2d10ebb2bdebc4a8e59d347719ba81abdf4",
        "DamagedHelmet" => "a1e3b04de97b11de564ce6e53b95f02954a297f0008183ac63a4f5974f6b32d8",
        "Fox" => "d97044e701822bac5a62696459b27d7b375aada5de8574ed4362edbba94771f7",
        _ => panic!("unpinned sample {name}"),
    };
    assert_eq!(
        String::from_utf8(digest.stdout)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap(),
        expected,
        "sample input changed: {name}"
    );
    let (model, textures) = exact_game_bake::assets(&path).unwrap();
    let bytes = bin::to_vec(&model);
    let decoded: Model = bin::from_slice(&bytes).unwrap();
    assert_eq!(exact_game::hash::of(&model), exact_game::hash::of(&decoded));
    assert_eq!(model.skins.len(), decoded.skins.len());
    assert_eq!(model.clips.len(), decoded.clips.len());
    for (name, t) in &textures {
        let bytes = bin::to_vec(t);
        assert!(bytes.len() < 64 * 1024 * 1024);
        let out = cache.join(name);
        std::fs::create_dir_all(out.parent().unwrap()).unwrap();
        std::fs::write(&out, &bytes).unwrap();
        eprintln!("{name}: {} bytes", bytes.len());
        assert_eq!(t.mips.last().unwrap().len(), 4);
        assert_eq!(
            t.mips.len(),
            (32 - t.width.max(t.height).leading_zeros()) as usize
        );
    }
    if name == "DamagedHelmet" {
        assert!(bytes.len() < 1_000_000);
    }
    assert!(!model.meshes.is_empty());
    assert!(model.bounds[3] > model.bounds[0]);
    let baked = cache.join(format!("{name}.model"));
    std::fs::write(&baked, &bytes).unwrap();
    let gzip = Command::new("gzip")
        .args(["-9", "-c"])
        .arg(&baked)
        .output()
        .unwrap();
    assert!(gzip.status.success());
    eprintln!("{name}: meshes={} materials={} textures={} nodes={} skins={} clips={} bounds={:?}; raw={} gzip={}",model.meshes.len(),model.materials.len(),model.textures.len(),model.nodes.len(),model.skins.len(),model.clips.len(),model.bounds,bytes.len(),gzip.stdout.len());
    model
}
#[test]
fn khronos_samples_bake_round_trip_and_have_complete_mips() {
    let bounds = |model: &Model, expected: [f32; 6]| {
        assert!(
            model
                .bounds
                .iter()
                .zip(expected)
                .all(|(a, b)| (a - b).abs() < 0.0001),
            "{:?}",
            model.bounds
        );
    };
    let box_ = sample("BoxTextured");
    assert_eq!(box_.meshes.len(), 1);
    assert_eq!(box_.meshes[0].indices.len(), 36);
    assert_eq!(box_.textures.len(), 1);
    assert_eq!(box_.nodes.len(), 2);
    bounds(&box_, [-0.5, -0.5, -0.5, 0.5, 0.5, 0.5]);
    let helmet = sample("DamagedHelmet");
    assert_eq!(helmet.meshes.len(), 1);
    assert_eq!(helmet.textures.len(), 5);
    assert_eq!(helmet.nodes.len(), 1);
    bounds(
        &helmet,
        [
            -0.94745857,
            -0.9009741,
            -1.1871551,
            0.9424954,
            0.90099514,
            0.8128452,
        ],
    );
    let fox = sample("Fox");
    assert_eq!(fox.skins.len(), 1);
    assert_eq!(fox.clips.len(), 3);
    assert_eq!(fox.skins[0].joints.len(), 24);
    assert_eq!(fox.skins[0].inverse_binds.len(), 24 * 16);
    assert_eq!(fox.nodes.len(), 26);
    assert!(fox.clips.iter().all(|clip| clip.tracks.len() == 21));
    bounds(
        &fox,
        [
            -12.592718,
            -0.12174477,
            -88.095,
            12.592718,
            78.90719,
            66.62486,
        ],
    );
    assert!(!fox.meshes[0].weights.is_empty());
}
