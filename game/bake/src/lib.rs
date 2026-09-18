//! Build-time glTF importer; no runtime crate depends on this crate.
use exact_game::{asset::*, Vec3};
use std::path::Path;
mod geometry;
mod textures;

/// Import a model into plain Data, refusing unsupported features by name.
pub fn model(path: impl AsRef<Path>) -> Result<Model, String> {
    let path = path.as_ref();
    bake(path).map_err(|e| format!("{}: {e}", path.display()))
}
fn bake(path: &Path) -> Result<Model, String> {
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
    textures::materials(&doc, &images, &mut model)?;
    let mut meshes = Vec::new();
    for mesh in doc.meshes() {
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
    // Original node indices remain stable for skins and animation tracks. Additional
    // primitive nodes inherit their original node's transform through an identity child.
    model.nodes = doc
        .nodes()
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
            skin: n.skin().map(|s| s.index() as u32),
            ..Node::default()
        })
        .collect();
    for node in doc.nodes() {
        for child in node.children() {
            model.nodes[child.index()].parent = Some(node.index() as u32);
        }
        if let Some(mesh) = node.mesh() {
            for (i, &part) in meshes[mesh.index()].iter().enumerate() {
                if i == 0 {
                    model.nodes[node.index()].mesh = Some(part);
                } else {
                    model.nodes.push(Node {
                        name: format!("{}:{i}", node.name().unwrap_or("mesh")),
                        parent: Some(node.index() as u32),
                        mesh: Some(part),
                        skin: node.skin().map(|s| s.index() as u32),
                        ..Node::default()
                    });
                }
            }
        }
    }
    for skin in doc.skins() {
        let joints: Vec<u32> = skin.joints().map(|n| n.index() as u32).collect();
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
        };
        for channel in animation.channels() {
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
                node: channel.target().node().index() as u32,
                path,
                interpolation,
                times,
                values,
            });
        }
        model.clips.push(clip);
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
    Ok(model)
}

/// A generated GPU shell's build.rs calls this before host scripts copy assets/.
/// Only glTF/GLB inputs are models; their buffers and images are import dependencies.
pub fn bake_art(app: impl AsRef<Path>) -> Result<(), String> {
    let app = app.as_ref();
    let art = app.join("art");
    if !art.exists() {
        println!("cargo:rerun-if-changed={}", app.join("app.json").display());
        return Ok(());
    }
    println!("cargo:rerun-if-changed={}", art.display());
    fn visit(dir: &Path, files: &mut Vec<std::path::PathBuf>) -> Result<(), String> {
        for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.is_dir() {
                visit(&path, files)?;
            } else if matches!(
                path.extension().and_then(|v| v.to_str()),
                Some("glb" | "gltf")
            ) {
                files.push(path);
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    visit(&art, &mut files)?;
    files.sort();
    let mut names = std::collections::BTreeSet::new();
    std::fs::create_dir_all(app.join("assets")).map_err(|e| e.to_string())?;
    for path in files {
        let stem = path
            .file_stem()
            .unwrap()
            .to_str()
            .ok_or("non-UTF8 art filename")?;
        if !names.insert(stem.to_owned()) {
            return Err(format!("duplicate art stem {stem}"));
        }
        let bytes = exact_game::bin::to_vec(&model(&path)?);
        let out = app.join("assets").join(format!("{stem}.model"));
        if std::fs::read(&out).ok().as_deref() != Some(&bytes) {
            std::fs::write(&out, bytes).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
