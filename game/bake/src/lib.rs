//! Build-time glTF importer; no runtime crate depends on this crate.
use exact_game::{asset::*, Vec3};
use std::path::Path;
mod geometry;
mod textures;

/// Import a model into plain Data, refusing unsupported features by name.
pub fn model(path: impl AsRef<Path>) -> Result<Model, String> {
    let path = path.as_ref();
    assets(path)
        .map(|(model, _)| model)
        .map_err(|e| format!("{}: {e}", path.display()))
}
/// Bake the model metadata and independently deliverable texture payloads.
pub fn assets(
    path: &Path,
) -> Result<(Model, std::collections::BTreeMap<String, TextureData>), String> {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("invalid art stem")?;
    if !asset_name(&format!("{stem}.model")) {
        return Err(format!("art stem `{stem}`: invalid asset name"));
    }
    // Inspect before import so sparse and unknown required extensions are named by us.
    let source = gltf::Gltf::open(path).map_err(|e| e.to_string())?;
    for extension in source
        .document
        .extensions_used()
        .chain(source.document.extensions_required())
    {
        if !matches!(
            extension,
            "KHR_materials_emissive_strength" | "KHR_texture_transform"
        ) {
            return Err(format!("unsupported extension {extension}"));
        }
    }
    for accessor in source.document.accessors() {
        if accessor.sparse().is_some() {
            return Err(format!(
                "sparse accessor {} is unsupported",
                accessor.index()
            ));
        }
    }
    let (doc, buffers, images) = gltf::import(path).map_err(|e| e.to_string())?;
    let mut model = Model::default();
    if doc.scenes().len() != 1 {
        return Err(format!("{}: expected one scene", path.display()));
    }
    let scene = doc
        .default_scene()
        .ok_or_else(|| format!("{}: no default scene", path.display()))?;
    fn visit(node: gltf::Node<'_>, reached: &mut std::collections::BTreeSet<usize>) {
        if reached.insert(node.index()) {
            for child in node.children() {
                visit(child, reached);
            }
        }
    }
    let mut reached = std::collections::BTreeSet::new();
    for root in scene.nodes() {
        visit(root, &mut reached);
    }
    let remap: std::collections::BTreeMap<_, _> = reached
        .iter()
        .enumerate()
        .map(|(i, &old)| (old, i as u32))
        .collect();
    let used_meshes: std::collections::BTreeSet<_> = doc
        .nodes()
        .filter(|n| reached.contains(&n.index()))
        .filter_map(|n| n.mesh().map(|m| m.index()))
        .collect();
    let used_materials = doc
        .meshes()
        .filter(|m| used_meshes.contains(&m.index()))
        .flat_map(|m| {
            m.primitives()
                .filter_map(|p| p.material().index())
                .collect::<Vec<_>>()
        })
        .collect();
    let textures = textures::materials(&doc, &images, &mut model, stem, &used_materials)?;
    let skin_map: std::collections::BTreeMap<_, _> = doc
        .nodes()
        .filter(|n| reached.contains(&n.index()))
        .filter_map(|n| n.skin().map(|s| s.index()))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .enumerate()
        .map(|(i, old)| (old, i as u32))
        .collect();
    let mut meshes = Vec::new();
    for mesh in doc.meshes() {
        if !used_meshes.contains(&mesh.index()) {
            meshes.push(Vec::new());
            continue;
        }
        let mut parts: Vec<MeshData> = Vec::new();
        for primitive in mesh.primitives() {
            let data = geometry::primitive(&primitive, &buffers, model.materials.len() as u32 - 1)?;
            // Merge only primitives with identical material and vertex attribute presence.
            if let Some(old) = parts.iter_mut().find(|m| {
                m.material == data.material
                    && m.joints.is_empty() == data.joints.is_empty()
                    && m.weights.is_empty() == data.weights.is_empty()
            }) {
                geometry::merge(old, data);
            } else {
                parts.push(data);
            }
        }
        let start = model.meshes.len() as u32;
        model.meshes.extend(parts);
        meshes.push((start..model.meshes.len() as u32).collect::<Vec<_>>());
    }
    // Reachable nodes, skins and tracks are remapped together. Additional primitive
    // nodes inherit their original node's transform through an identity child.
    model.nodes = doc
        .nodes()
        .filter(|n| reached.contains(&n.index()))
        .map(|n| Node {
            name: n.name().unwrap_or("").into(),
            transform: n
                .transform()
                .matrix()
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
            skin: n.skin().map(|s| skin_map[&s.index()]),
            ..Node::default()
        })
        .collect();
    for node in doc.nodes().filter(|n| reached.contains(&n.index())) {
        let node_index = remap[&node.index()] as usize;
        for child in node.children() {
            model.nodes[remap[&child.index()] as usize].parent = Some(node_index as u32);
        }
        if let Some(mesh) = node.mesh() {
            for (i, &part) in meshes[mesh.index()].iter().enumerate() {
                if i == 0 {
                    model.nodes[node_index].mesh = Some(part);
                } else {
                    model.nodes.push(Node {
                        name: format!("{}:{i}", node.name().unwrap_or("mesh")),
                        parent: Some(node_index as u32),
                        mesh: Some(part),
                        skin: node.skin().map(|s| skin_map[&s.index()]),
                        ..Node::default()
                    });
                }
            }
        }
    }
    for skin in doc.skins().filter(|s| skin_map.contains_key(&s.index())) {
        let joints: Vec<u32> = skin
            .joints()
            .map(|n| {
                remap.get(&n.index()).copied().ok_or_else(|| {
                    format!(
                        "skin joint `{}` is outside the default scene",
                        n.name().unwrap_or("unnamed")
                    )
                })
            })
            .collect::<Result<_, _>>()?;
        let reader = skin.reader(|b| Some(buffers[b.index()].0.as_slice()));
        let inverse_binds = reader
            .read_inverse_bind_matrices()
            .map(|v| v.flatten().flatten().collect())
            .unwrap_or_else(|| {
                joints
                    .iter()
                    .flat_map(|_| {
                        [
                            1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
                        ]
                    })
                    .collect()
            });
        model.skins.push(Skin {
            name: skin.name().unwrap_or("").into(),
            joints,
            inverse_binds,
        });
    }
    for animation in doc.animations() {
        let mut clip = Clip {
            name: animation.name().unwrap_or("").into(),
            tracks: Vec::new(),
            markers: animation
                .extras()
                .as_ref()
                .map(|v| {
                    let value: serde_json::Value =
                        serde_json::from_str(v.get()).map_err(|e| e.to_string())?;
                    value
                        .get("markers")
                        .map(|v| serde_json::from_value(v.clone()).map_err(|e| e.to_string()))
                        .unwrap_or(Ok(Vec::new()))
                })
                .transpose()?
                .unwrap_or_default(),
        };
        for channel in animation.channels() {
            let Some(&node) = remap.get(&channel.target().node().index()) else {
                continue;
            };
            let reader = channel.reader(|b| Some(buffers[b.index()].0.as_slice()));
            let times = reader
                .read_inputs()
                .ok_or("animation has no times")?
                .collect();
            use gltf::animation::util::ReadOutputs;
            let (path, values) = match reader.read_outputs().ok_or("animation has no values")? {
                ReadOutputs::Translations(v) => (TrackPath::Translation, v.flatten().collect()),
                ReadOutputs::Rotations(v) => {
                    (TrackPath::Rotation, v.into_f32().flatten().collect())
                }
                ReadOutputs::Scales(v) => (TrackPath::Scale, v.flatten().collect()),
                ReadOutputs::MorphTargetWeights(_) => {
                    return Err("morph target animation is unsupported".into())
                }
            };
            let interpolation = match channel.sampler().interpolation() {
                gltf::animation::Interpolation::Step => Interpolation::Step,
                gltf::animation::Interpolation::Linear => Interpolation::Linear,
                gltf::animation::Interpolation::CubicSpline => Interpolation::CubicSpline,
            };
            clip.tracks.push(Track {
                node,
                path,
                interpolation,
                times,
                values,
            });
        }
        model.clips.push(clip);
    }
    let used: std::collections::BTreeSet<_> =
        model.meshes.iter().map(|m| m.material as usize).collect();
    let remap: std::collections::BTreeMap<_, _> = used
        .iter()
        .enumerate()
        .map(|(i, &old)| (old, i as u32))
        .collect();
    model.materials = model
        .materials
        .into_iter()
        .enumerate()
        .filter_map(|(i, m)| used.contains(&i).then_some(m))
        .collect();
    for mesh in &mut model.meshes {
        mesh.material = remap[&(mesh.material as usize)];
    }
    let offsets = model.offsets()?;
    let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    for (node, offset) in model.nodes.iter().zip(offsets) {
        if let Some(mesh) = node.mesh {
            for p in model.meshes[mesh as usize].positions.chunks_exact(3) {
                let p = offset.transform_point3(Vec3::from_slice(p));
                lo = lo.min(p);
                hi = hi.max(p);
            }
        }
    }
    if !lo.is_finite() {
        return Err("model contains no mesh nodes".into());
    }
    model.bounds = [lo.x, lo.y, lo.z, hi.x, hi.y, hi.z];
    model.validate()?;
    Ok((model, textures))
}

/// Bake a standalone PNG sprite: sRGB, straight alpha, nearest min/mag/mips,
/// clamp-to-edge on both axes. Nearest mip levels preserve the authored palette.
pub fn sprite(path: impl AsRef<Path>) -> Result<TextureData, String> {
    let path = path.as_ref();
    let image = image::ImageReader::open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .decode()
        .map_err(|e| format!("{}: {e}", path.display()))?
        .into_rgba8();
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 || width > 2048 || height > 2048 {
        return Err(format!(
            "{}: sprite dimensions must be 1..=2048",
            path.display()
        ));
    }
    let mut texture = TextureData {
        width,
        height,
        srgb: true,
        wrap: [Wrap::Clamp; 2],
        filter: [Filter::Nearest; 3],
        mips: vec![image.into_raw()],
    };
    let (mut w, mut h) = (width, height);
    while w > 1 || h > 1 {
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let previous = texture.mips.last().unwrap();
        let mut next = vec![0; (nw * nh * 4) as usize];
        for y in 0..nh {
            for x in 0..nw {
                let from = ((y * 2).min(h - 1) * w + (x * 2).min(w - 1)) as usize * 4;
                let to = (y * nw + x) as usize * 4;
                next[to..to + 4].copy_from_slice(&previous[from..from + 4]);
            }
        }
        texture.mips.push(next);
        (w, h) = (nw, nh);
    }
    texture.validate()?;
    Ok(texture)
}

/// A generated GPU shell's build.rs calls this before host scripts copy assets/.
/// glTF/GLB inputs become models; standalone PNG inputs become sprite textures.
pub fn bake_art(app: impl AsRef<Path>) -> Result<(), String> {
    let app = app.as_ref();
    let art = app.join("art");
    println!("cargo:rerun-if-changed={}", art.display());
    fn visit(dir: &Path, files: &mut Vec<std::path::PathBuf>) -> Result<(), String> {
        for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.is_dir() {
                visit(&path, files)?;
            } else if matches!(
                path.extension().and_then(|v| v.to_str()),
                Some("glb" | "gltf" | "png")
            ) {
                files.push(path);
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    if art.exists() {
        visit(&art, &mut files)?;
    }
    files.sort();
    let manifest = app.join(".baked-assets.json");
    if !art.exists() && !manifest.exists() {
        return Ok(());
    }
    let mut legacy = std::collections::BTreeSet::<String>::new();
    let previous: std::collections::BTreeMap<String, String> = match std::fs::read(&manifest) {
        Ok(bytes) => match serde_json::from_slice(&bytes) {
            Ok(map) => map,
            Err(_) => {
                legacy = serde_json::from_slice::<Vec<String>>(&bytes)
                    .map_err(|e| format!("generated-output manifest: {e}"))?
                    .into_iter()
                    .collect();
                Default::default()
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Default::default(),
        Err(e) => return Err(e.to_string()),
    };
    if previous.keys().chain(legacy.iter()).any(|n| !asset_name(n)) {
        return Err("invalid generated-output manifest".into());
    }
    let mut outputs = std::collections::BTreeMap::new();
    for path in files {
        if path.extension().and_then(|v| v.to_str()) == Some("png") {
            let name = format!("{}.tex", path.file_stem().unwrap().to_str().unwrap());
            if !asset_name(&name) {
                return Err(format!("invalid sprite asset name `{name}`"));
            }
            let texture = sprite(&path)?;
            if outputs
                .insert(name.clone(), encode(&name, &texture)?)
                .is_some()
            {
                return Err(format!("duplicate art stem {name}"));
            }
            continue;
        }
        let (model, textures) = assets(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let name = format!("{}.model", path.file_stem().unwrap().to_str().unwrap());
        if outputs
            .insert(name.clone(), encode(&name, &model)?)
            .is_some()
        {
            return Err(format!("duplicate art stem {name}"));
        }
        for (name, texture) in textures {
            let bytes = encode(&name, &texture)?;
            outputs.insert(name, bytes);
        }
    }
    let root = app.join("assets");
    // Validate every mutation before writing or pruning anything.
    for name in previous.keys().chain(outputs.keys()) {
        let desired = outputs.get(name);
        if let Some(bytes) = desired {
            check_size(name, bytes.len())?;
        }
        match std::fs::read(root.join(name)) {
            Ok(current)
                if previous.get(name) == Some(&digest(&current))
                    || (legacy.contains(name) && desired == Some(&current)) => {}
            Ok(_) => {
                return Err(format!(
                    "generated asset `{name}` collides with an authored asset"
                ))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("asset `{name}`: {e}")),
        }
    }
    for (name, bytes) in &outputs {
        let out = root.join(name);
        std::fs::create_dir_all(out.parent().unwrap()).map_err(|e| e.to_string())?;
        write_changed(&out, bytes)?;
    }

    for old in previous.keys().filter(|n| !outputs.contains_key(*n)) {
        match std::fs::remove_file(root.join(old)) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    let digests: std::collections::BTreeMap<_, _> =
        outputs.iter().map(|(n, b)| (n, digest(b))).collect();
    let bytes = serde_json::to_vec(&digests).unwrap();
    write_changed(&manifest, &bytes)
}
fn write_changed(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    if std::fs::read(path).ok().as_deref() == Some(bytes) {
        return Ok(());
    }
    let tmp = path.with_file_name(format!(
        ".{}.{}.tmp",
        path.file_name().unwrap().to_string_lossy(),
        std::process::id()
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(|e| format!("{}: {e}", tmp.display()))?;
    let result = file
        .write_all(bytes)
        .and_then(|()| std::fs::rename(&tmp, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result.map_err(|e| format!("{}: {e}", path.display()))
}

fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
/// Refuse oversized carriers before decoding or writing them.
pub fn check_size(name: &str, size: usize) -> Result<(), String> {
    if size > 64 * 1024 * 1024 {
        Err(format!("asset `{name}` exceeds 64 MiB"))
    } else {
        Ok(())
    }
}
/// The same bounded encoding for both CLI and generated shell builds.
pub fn encode(name: &str, value: &impl exact_game::Data) -> Result<Vec<u8>, String> {
    let bytes = exact_game::bin::to_vec(value);
    check_size(name, bytes.len())?;
    Ok(bytes)
}
