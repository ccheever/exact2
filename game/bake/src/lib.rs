//! Build-time glTF importer; no runtime crate depends on this crate.
use exact_game::{asset::*, Vec3};
use std::path::Path;
pub mod compress;
mod files;
mod geometry;
pub use files::{bake_game_levels, check_size};
mod sound;
pub use sound::sound;
mod textures;

/// Import a model into plain Data, refusing unsupported features by name.
pub fn model(path: impl AsRef<Path>) -> Result<Model, String> {
    let path = path.as_ref();
    assets(path)
        .map(|(model, _)| model)
        .map_err(|e| format!("{}: {e}", path.display()))
}
/// Bake the model metadata and independently deliverable texture payloads:
/// each texture's authored RGBA8 name plus its `.bc.tex` and `.astc.tex`
/// payloads (`TextureFamily::name`), of which a device fetches one.
pub fn assets(
    path: &Path,
) -> Result<(Model, std::collections::BTreeMap<String, TextureData>), String> {
    let (model, sources) = sources(path)?;
    let mut textures = std::collections::BTreeMap::new();
    for (name, (full, channels)) in sources {
        textures.extend(compress::variants(&name, &full, channels, false)?);
    }
    Ok((model, textures))
}

/// A model's full-resolution texture chains by authored name, with the
/// channels their slots read, before block compression.
type ModelSources = std::collections::BTreeMap<String, (TextureData, compress::Channels)>;

fn sources(path: &Path) -> Result<(Model, ModelSources), String> {
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
    let sources = textures::materials(
        &doc,
        &images,
        &mut model,
        stem,
        path.parent().unwrap_or(Path::new(".")),
        &used_materials,
    )?;
    let textures: ModelSources = sources
        .into_iter()
        .map(|(name, (full, slots, cut))| (name, (full, textures::channels(slots, cut))))
        .collect();
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
    // Importers may declare source units once; runtime models use metres.
    let units = source
        .document
        .as_json()
        .asset
        .extras
        .as_ref()
        .map(|raw| serde_json::from_str::<serde_json::Value>(raw.get()).map_err(|e| e.to_string()))
        .transpose()?
        .and_then(|v| v.get("metersPerUnit").cloned());
    if let Some(units) = units {
        let scale = units
            .as_f64()
            .ok_or("metersPerUnit must be a positive number")? as f32;
        if !scale.is_finite() || scale <= 0. {
            return Err("metersPerUnit must be a positive finite number".into());
        }
        for mesh in &mut model.meshes {
            for value in &mut mesh.positions {
                *value *= scale;
            }
            for value in &mut mesh.bounds {
                *value *= scale;
            }
        }
        for node in &mut model.nodes {
            for value in &mut node.transform[12..15] {
                *value *= scale;
            }
        }
        for skin in &mut model.skins {
            for matrix in skin.inverse_binds.chunks_exact_mut(16) {
                for value in &mut matrix[12..15] {
                    *value *= scale;
                }
            }
        }
        for clip in &mut model.clips {
            for track in &mut clip.tracks {
                if matches!(track.path, TrackPath::Translation) {
                    for value in &mut track.values {
                        *value *= scale;
                    }
                }
            }
        }
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
    // Name each texture by its content, so identical images are one asset
    // across models (and within one) and no model's names depend on another's.
    let mut renamed = std::collections::BTreeMap::new();
    let mut named = ModelSources::new();
    for (name, (full, channels)) in textures {
        let shared = texture_name(&full, channels);
        renamed.insert(name, shared.clone());
        named.entry(shared).or_insert((full, channels));
    }
    share_textures(&mut model, &renamed);
    model.validate()?;
    Ok((model, named))
}

/// `textures/<digest>.tex`: the digest of the full RGBA8 chain, sampler included,
/// and of the channels the material reads (which choose the block formats).
fn texture_name(full: &TextureData, channels: compress::Channels) -> String {
    let mut bytes = exact_game::bin::to_vec(full);
    bytes.extend(format!("{channels:?}").bytes());
    format!("textures/{}.tex", &digest(&bytes)[..16])
}

/// A sprite's authored RGBA8 name and its per-family payloads. Block formats
/// are used only where they decode to exactly the authored texels.
pub fn sprite_variants(
    name: &str,
    path: impl AsRef<Path>,
) -> Result<Vec<(String, TextureData)>, String> {
    compress::variants(name, &sprite(path)?, compress::Channels::ColorAlpha, true)
}

/// How a standalone PNG is sampled, from the `art/` folder it sits in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PngKind {
    /// Anywhere else: a pixel-exact sprite (sRGB, clamped, nearest).
    Sprite,
    /// Under `art/textures/`: a material colour texture shared by any model or
    /// generated mesh: sRGB, repeating, linearly filtered, box-filtered mips.
    Color,
    /// Under `art/data/`: the same, but linear values (normal maps, masks).
    Data,
    /// Under `art/data/rgbm/`: an RGBM environment map, linear, its mips
    /// box-filtered as radiance (decoded, averaged, re-encoded), not per channel.
    Rgbm,
}
impl PngKind {
    /// The kind a PNG's path inside `art/` selects.
    pub fn of(art: &Path, path: &Path) -> Self {
        let mut parts = path
            .strip_prefix(art)
            .ok()
            .into_iter()
            .flat_map(|p| p.components())
            .map(|c| c.as_os_str().to_str());
        let (first, second) = (parts.next().flatten(), parts.next().flatten());
        let nested = path.parent() != Some(art);
        match (first, second) {
            (Some("textures"), _) if nested => Self::Color,
            (Some("data"), Some("rgbm")) if path.parent() != Some(&art.join("data")) => Self::Rgbm,
            (Some("data"), _) if nested => Self::Data,
            _ => Self::Sprite,
        }
    }
}
/// A material texture's authored RGBA8 name and its per-family payloads,
/// block-compressed within the usual quality bound.
pub fn material_texture_variants(
    name: &str,
    path: impl AsRef<Path>,
    kind: PngKind,
) -> Result<Vec<(String, TextureData)>, String> {
    let mut texture = sprite(path)?;
    let color = kind == PngKind::Color;
    texture.srgb = color;
    texture.wrap = [Wrap::Repeat; 2];
    texture.filter = [Filter::Linear; 3];
    let top = texture.mips.swap_remove(0);
    texture.mips = if kind == PngKind::Rgbm {
        textures::rgbm_mips(texture.width, texture.height, top)
    } else {
        textures::mips(texture.width, texture.height, top, color, true, None)
    };
    texture.validate()?;
    compress::variants(name, &texture, compress::Channels::ColorAlpha, false)
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
        format: TextureFormat::Rgba8,
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
/// glTF/GLB inputs become models; standalone PNG inputs become sprite textures;
/// WAV and Ogg Vorbis inputs become `.sound` records.
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
                Some("glb" | "gltf" | "png" | "wav" | "ogg")
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
    let previous: std::collections::BTreeMap<String, String> = match std::fs::read(&manifest) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| format!(
            "generated-output manifest: {e}; regenerate by removing .baked-assets.json and its generated outputs, then bake art again"
        ))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Default::default(),
        Err(e) => return Err(e.to_string()),
    };
    if previous.keys().any(|n| !asset_name(n)) {
        return Err("invalid generated-output manifest".into());
    }
    // A build script reruns whenever the gameplay it links changes, which
    // changes no input of this bake: an unchanged stamp skips the re-encode.
    let stamp = std::env::var_os("OUT_DIR").map(|dir| Path::new(&dir).join("art-stamp"));
    if let Some(stamp) = &stamp {
        if std::fs::read_to_string(stamp).ok() == Some(art_stamp(app, previous.keys())) {
            return Ok(());
        }
    }
    let mut outputs = std::collections::BTreeMap::new();
    for path in files {
        if matches!(
            path.extension().and_then(|v| v.to_str()),
            Some("wav" | "ogg")
        ) {
            let name = format!("{}.sound", art_stem(&path)?);
            if !asset_name(&name) {
                return Err(format!("invalid sound asset name `{name}`"));
            }
            if outputs
                .insert(name.clone(), encode(&name, &sound(&path)?)?)
                .is_some()
            {
                return Err(format!("duplicate art stem {name}"));
            }
            continue;
        }
        if path.extension().and_then(|v| v.to_str()) == Some("png") {
            let name = format!("{}.tex", art_stem(&path)?);
            if !asset_name(&name) {
                return Err(format!("invalid sprite asset name `{name}`"));
            }
            let variants = match PngKind::of(&art, &path) {
                PngKind::Sprite => sprite_variants(&name, &path)?,
                kind => material_texture_variants(&name, &path, kind)?,
            };
            for (name, texture) in variants {
                if outputs
                    .insert(name.clone(), encode(&name, &texture)?)
                    .is_some()
                {
                    return Err(format!("duplicate art stem {name}"));
                }
            }
            continue;
        }
        let (model, textures) = sources(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let name = format!("{}.model", art_stem(&path)?);
        // Content-named: a texture another model already baked is the same file.
        for (texture, (full, channels)) in textures {
            if outputs.contains_key(&texture) {
                continue;
            }
            for (name, texture) in compress::variants(&texture, &full, channels, false)? {
                let bytes = encode(&name, &texture)?;
                outputs.insert(name, bytes);
            }
        }
        if outputs
            .insert(name.clone(), encode(&name, &model)?)
            .is_some()
        {
            return Err(format!("duplicate art stem {name}"));
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
            Ok(current) if previous.get(name) == Some(&digest(&current)) => {}
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
    // The next app resolution can drop this baker after the last art is removed.
    if !art.exists() {
        return std::fs::remove_file(manifest).map_err(|e| e.to_string());
    }
    let digests: std::collections::BTreeMap<_, _> =
        outputs.iter().map(|(n, b)| (n, digest(b))).collect();
    let bytes = serde_json::to_vec(&digests).unwrap();
    write_changed(&manifest, &bytes)?;
    match stamp {
        Some(stamp) => write_changed(&stamp, art_stamp(app, outputs.keys()).as_bytes()),
        None => Ok(()),
    }
}

/// What an art bake read and wrote, each file by path, size and modification
/// time: the art; the baker's and the engine's sources and manifests (the
/// encoding); the locks (the encoders' versions); the generated manifest and
/// its outputs. Garden's 225 files re-encode in about 0.4 s of CPU.
fn art_stamp<'a>(app: &Path, outputs: impl Iterator<Item = &'a String>) -> String {
    use std::fmt::Write;
    fn file(path: &Path, stamp: &mut String) {
        let _ = match std::fs::metadata(path) {
            Ok(meta) => {
                let modified = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok());
                writeln!(
                    stamp,
                    "{} {} {}",
                    path.display(),
                    meta.len(),
                    modified.map_or(0, |d| d.as_nanos())
                )
            }
            Err(_) => writeln!(stamp, "{} -", path.display()),
        };
    }
    fn tree(dir: &Path, stamp: &mut String) {
        let mut paths: Vec<_> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .collect();
        paths.sort();
        for path in paths {
            if path.is_dir() {
                tree(&path, stamp)
            } else {
                file(&path, stamp)
            }
        }
    }
    let (mut stamp, baker) = (String::from("1\n"), Path::new(env!("CARGO_MANIFEST_DIR")));
    for dir in [
        app.join("art"),
        baker.join("src"),
        baker.join("../engine/src"),
    ] {
        tree(&dir, &mut stamp);
    }
    for path in [
        baker.join("Cargo.toml"),
        baker.join("../engine/Cargo.toml"),
        app.join(".shells/Cargo.lock"),
        app.join("Cargo.lock"),
        app.join(".baked-assets.json"),
    ] {
        file(&path, &mut stamp);
    }
    for name in outputs {
        file(&app.join("assets").join(name), &mut stamp);
    }
    stamp
}

/// Point a model's textures at the names `renamed` gives them, merging any
/// that become the same name so each is listed once.
fn share_textures(model: &mut Model, renamed: &std::collections::BTreeMap<String, String>) {
    if renamed.is_empty() {
        return;
    }
    let mut names: Vec<String> = Vec::new();
    let index: Vec<u32> = model
        .textures
        .iter()
        .map(|name| {
            let name = renamed.get(name).unwrap_or(name);
            let at = names.iter().position(|n| n == name).unwrap_or_else(|| {
                names.push(name.clone());
                names.len() - 1
            });
            at as u32
        })
        .collect();
    for m in &mut model.materials {
        for i in [
            &mut m.base_color_texture,
            &mut m.normal_texture,
            &mut m.metallic_roughness_texture,
            &mut m.emissive_texture,
            &mut m.occlusion_texture,
        ]
        .into_iter()
        .flatten()
        {
            *i = index[*i as usize];
        }
    }
    model.textures = names;
}

fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
/// The same bounded encoding for both CLI and generated shell builds.
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

pub fn encode(name: &str, value: &impl exact_game::Data) -> Result<Vec<u8>, String> {
    let bytes = exact_game::bin::to_vec(value);
    check_size(name, bytes.len())?;
    Ok(bytes)
}

fn art_stem(path: &Path) -> Result<&str, String> {
    path.file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "invalid art stem".into())
}
#[cfg(all(test, unix))]
mod stem_tests {
    #[test]
    fn png_non_utf8_stem_returns_an_error() {
        use std::os::unix::ffi::OsStrExt;
        let path = std::path::Path::new(std::ffi::OsStr::from_bytes(b"bad\xff.png"));
        assert_eq!(super::art_stem(path).unwrap_err(), "invalid art stem");
    }
}

#[cfg(test)]
mod png_kind_tests {
    use super::*;
    #[test]
    fn pngs_under_textures_and_data_bake_as_filtered_repeating_material_textures() {
        let art = Path::new("/game/art");
        assert_eq!(PngKind::of(art, &art.join("strip.png")), PngKind::Sprite);
        assert_eq!(PngKind::of(art, &art.join("ui/strip.png")), PngKind::Sprite);
        assert_eq!(
            PngKind::of(art, &art.join("textures/soil.png")),
            PngKind::Color
        );
        assert_eq!(PngKind::of(art, &art.join("data/sky.png")), PngKind::Data);
        assert_eq!(
            PngKind::of(art, &art.join("data/rgbm/sky.png")),
            PngKind::Rgbm
        );
        assert_eq!(PngKind::of(art, &art.join("data/rgbm.png")), PngKind::Data);
        let dir = std::env::temp_dir().join(format!("exact-bake-png-kind-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("soil.png");
        let mut image = image::RgbaImage::new(8, 8);
        for (x, _, p) in image.enumerate_pixels_mut() {
            *p = image::Rgba(if x < 4 {
                [255, 0, 0, 255]
            } else {
                [0, 0, 255, 255]
            });
        }
        image.save(&path).unwrap();
        for kind in [PngKind::Color, PngKind::Data] {
            let variants = material_texture_variants("soil.tex", &path, kind).unwrap();
            let (name, texture) = &variants[0];
            assert_eq!(name, "soil.tex");
            assert_eq!(texture.srgb, kind == PngKind::Color);
            assert_eq!(texture.wrap, [Wrap::Repeat; 2]);
            assert_eq!(texture.filter, [Filter::Linear; 3]);
            // Box-filtered: the 1x1 level averages both halves.
            let last = texture.mips.last().unwrap();
            assert!(last[0] > 100 && last[2] > 100, "{last:?}");
            assert_eq!(variants.len(), 3, "RGBA8, BC and ASTC families");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
    #[test]
    fn rgbm_mips_average_radiance_not_channels() {
        // A dim texel (rgb 1.0 under multiplier 0.2) beside black at full multiplier.
        let mips = textures::rgbm_mips(2, 1, vec![255, 255, 255, 51, 0, 0, 0, 255]);
        let p = &mips[1];
        let radiance = f32::from(p[0]) / 255. * f32::from(p[3]) / 255.;
        // Per-channel averaging would give rgb 0.5 under 0.6: 0.3.
        assert!((radiance - 0.1).abs() < 0.005, "{radiance} from {p:?}");
    }
}
