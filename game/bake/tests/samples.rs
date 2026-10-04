//! Khronos glTF-Sample-Assets, fetched into an external cache, never vendored.
//! BoxTextured: Cesium, CC BY 4.0. Fox: PixelMannen, CC0; conversion by @tommy3b.
//! DamagedHelmet: ctxwing (2018), CC BY 4.0; original theblueturtle_ (2016), CC BY-NC 4.0.
//! Metadata/licences: https://github.com/KhronosGroup/glTF-Sample-Assets/tree/main/Models
use exact_game::{asset::Model, bin};
use flate2::{write::GzEncoder, Compression};
use sha2::{Digest, Sha256};
use std::{io::Write, path::PathBuf, process::Command};

/// Shared source and baked-payload cache, also consumed by renderer fixtures.
pub fn cache() -> PathBuf {
    #[cfg(windows)]
    let root = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(|p| PathBuf::from(p).join("AppData/Local")))
        .expect("sample fixtures need LOCALAPPDATA or USERPROFILE");
    #[cfg(not(windows))]
    let root = PathBuf::from(std::env::var_os("HOME").expect("sample fixtures need HOME"))
        .join("Library/Caches");
    root.join("exact2-game/gltf-samples")
}

fn temporary(path: &std::path::Path) -> PathBuf {
    path.with_extension(format!(
        "{}-{:?}.tmp",
        std::process::id(),
        std::thread::current().id()
    ))
}

fn write_cached(path: &std::path::Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let tmp = temporary(path);
    std::fs::write(&tmp, bytes).unwrap();
    // Readers see a whole payload when concurrent fixtures bake the same model.
    std::fs::rename(tmp, path).unwrap();
}

fn gzip_bytes(bytes: &[u8]) -> usize {
    let mut gzip = GzEncoder::new(Vec::new(), Compression::best());
    gzip.write_all(bytes).unwrap();
    gzip.finish().unwrap().len()
}
pub fn sample(name: &str) -> Model {
    let cache = cache();
    std::fs::create_dir_all(&cache).unwrap();
    let path = cache.join(format!("{name}.glb"));
    if !path.exists() {
        let tmp = temporary(&path);
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
    let expected = match name {
        "BoxTextured" => "b510eca2e2ef33f62f9ed57d6e7ce2d10ebb2bdebc4a8e59d347719ba81abdf4",
        "DamagedHelmet" => "a1e3b04de97b11de564ce6e53b95f02954a297f0008183ac63a4f5974f6b32d8",
        "Fox" => "d97044e701822bac5a62696459b27d7b375aada5de8574ed4362edbba94771f7",
        _ => panic!("unpinned sample {name}"),
    };
    assert_eq!(
        format!("{:x}", Sha256::digest(std::fs::read(&path).unwrap())),
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
        write_cached(&out, &bytes);
        let gzip = gzip_bytes(&bytes);
        t.validate().unwrap();
        assert_eq!(
            t.mips.len(),
            (32 - t.width.max(t.height).leading_zeros()) as usize
        );
        let quality = quality(name, t, &textures);
        eprintln!(
            "{name}: {:?} {}x{} raw={} gzip={}{quality}",
            t.format,
            t.width,
            t.height,
            bytes.len(),
            gzip
        );
    }
    if name == "DamagedHelmet" {
        assert!(bytes.len() < 1_000_000);
    }
    assert!(!model.meshes.is_empty());
    assert!(model.bounds[3] > model.bounds[0]);
    let baked = cache.join(format!("{name}.model"));
    write_cached(&baked, &bytes);
    let gzip = gzip_bytes(&bytes);
    eprintln!("{name}: meshes={} materials={} textures={} nodes={} skins={} clips={} bounds={:?}; raw={} gzip={}",model.meshes.len(),model.materials.len(),model.textures.len(),model.nodes.len(),model.skins.len(),model.clips.len(),model.bounds,bytes.len(),gzip);
    model
}
/// A family payload's PSNR against its RGBA8 fallback, over the channels its
/// material reads, at the top level (which must exceed 40 dB) and the worst level.
fn quality(
    name: &str,
    t: &exact_game::asset::TextureData,
    all: &std::collections::BTreeMap<String, exact_game::asset::TextureData>,
) -> String {
    use exact_game::asset::TextureFormat;
    use exact_game_bake::compress::{decode, psnr};
    let Some(stem) = name
        .strip_suffix(".bc.tex")
        .or_else(|| name.strip_suffix(".astc.tex"))
    else {
        return String::new();
    };
    let rgba = &all[&format!("{stem}.tex")];
    let channels: &[usize] = match all[&format!("{stem}.bc.tex")].format {
        TextureFormat::Bc4 => &[0],
        TextureFormat::Bc5 => &[0, 1],
        _ if name.contains("-alpha") => &[0, 1, 2, 3],
        _ => &[0, 1, 2],
    };
    let skip = t.mips.len() - rgba.mips.len();
    let levels: Vec<f64> = (0..rgba.mips.len())
        .map(|level| psnr(&rgba.mips[level], &decode(t, level + skip), channels))
        .collect();
    let worst = levels.iter().copied().fold(f64::INFINITY, f64::min);
    // Small levels hold the most detail per block; 4×4 codecs measure ~30 dB
    // there in both families, which the gate reports rather than bounds.
    assert!(levels[0] > 40., "{name}: {levels:?} dB");
    format!(" psnr={:.1}dB worst-level={worst:.1}dB", levels[0])
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
