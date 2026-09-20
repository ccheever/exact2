use crate::{Mesh, Result};
use base64::Engine;
use std::path::Path;

pub fn read(bytes: &[u8], path: &Path) -> Result<Mesh> {
    let document = gltf::Gltf::from_slice(bytes)?;
    let mut buffers = Vec::new();
    for b in document.buffers() {
        let data = match b.source() {
            gltf::buffer::Source::Bin => document.blob.clone().ok_or("GLB binary chunk missing")?,
            gltf::buffer::Source::Uri(uri) => {
                if uri.starts_with("data:") {
                    let (prefix, data) = uri.split_once(',').ok_or("invalid data URI")?;
                    if !prefix.ends_with(";base64") {
                        return Err("glTF data URI must use base64".into());
                    }
                    base64::engine::general_purpose::STANDARD.decode(data)?
                } else {
                    if uri.contains("://") || uri.contains('%') {
                        return Err(
                            "glTF requires plain local buffer paths or base64 data URIs".into()
                        );
                    }
                    std::fs::read(path.parent().unwrap_or(Path::new(".")).join(uri))?
                }
            }
        };
        if data.len() < b.length() {
            return Err("truncated glTF buffer".into());
        }
        buffers.push(data);
    }
    let source = document.meshes().next().ok_or("glTF has no mesh")?;
    let mut mesh = Mesh::default();
    let mut all_normals = true;
    for primitive in source.primitives() {
        if primitive.mode() != gltf::mesh::Mode::Triangles {
            return Err("glTF requires triangle primitives".into());
        }
        let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()][..]));
        let positions: Vec<_> = reader
            .read_positions()
            .ok_or("glTF POSITION missing")?
            .collect();
        let base = mesh.positions.len() as u32;
        let count = positions.len();
        if let Some(normals) = reader.read_normals() {
            mesh.normals.extend(normals);
        } else {
            all_normals = false;
        }
        if let Some(colors) = reader.read_colors(0) {
            let cs = mesh
                .colors
                .get_or_insert_with(|| vec![[255; 4]; base as usize]);
            cs.extend(colors.into_rgba_u8());
        } else if let Some(cs) = &mut mesh.colors {
            cs.extend(std::iter::repeat_n([255; 4], count));
        }
        let local: Vec<u32> = reader
            .read_indices()
            .map(|i| i.into_u32().collect())
            .unwrap_or_else(|| (0..count as u32).collect());
        if local.iter().any(|i| *i as usize >= count) {
            return Err("glTF index out of primitive range".into());
        }
        mesh.indices.extend(local.into_iter().map(|i| base + i));
        mesh.positions.extend(positions);
    }
    if !all_normals {
        mesh.normals.clear();
    }
    mesh.validate()?;
    Ok(mesh)
}
