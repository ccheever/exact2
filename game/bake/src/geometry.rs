use exact_game::{asset::MeshData, Vec3};
pub fn primitive(
    p: &gltf::Primitive<'_>,
    buffers: &[gltf::buffer::Data],
    default: u32,
) -> Result<MeshData, String> {
    if p.morph_targets().next().is_some() {
        return Err(format!(
            "primitive {}: morph targets unsupported",
            p.index()
        ));
    }
    if p.mode() != gltf::mesh::Mode::Triangles {
        return Err(format!(
            "primitive {}: {:?} unsupported",
            p.index(),
            p.mode()
        ));
    }
    for (semantic, _) in p.attributes() {
        if !matches!(
            semantic,
            gltf::Semantic::Positions
                | gltf::Semantic::Normals
                | gltf::Semantic::Tangents
                | gltf::Semantic::TexCoords(0)
                | gltf::Semantic::Joints(0)
                | gltf::Semantic::Weights(0)
                | gltf::Semantic::Colors(0)
        ) {
            return Err(format!("primitive {}: {semantic:?} unsupported", p.index()));
        }
    }
    let material = p.material();
    let pbr = material.pbr_metallic_roughness();
    let textured = pbr.base_color_texture().is_some()
        || pbr.metallic_roughness_texture().is_some()
        || material.normal_texture().is_some()
        || material.emissive_texture().is_some()
        || material.occlusion_texture().is_some();
    if textured && p.get(&gltf::Semantic::TexCoords(0)).is_none() {
        return Err(format!(
            "material {:?}: texture requires TEXCOORD_0",
            material.index()
        ));
    }
    if p.get(&gltf::Semantic::Joints(0)).is_some() != p.get(&gltf::Semantic::Weights(0)).is_some() {
        return Err("JOINTS_0/WEIGHTS_0 mismatch".into());
    }
    let r = p.reader(|b| Some(buffers[b.index()].0.as_slice()));
    let positions: Vec<f32> = r
        .read_positions()
        .ok_or("missing POSITION")?
        .flatten()
        .collect();
    let count = positions.len() / 3;
    let indices: Vec<u32> = r
        .read_indices()
        .map(|v| v.into_u32().collect())
        .unwrap_or_else(|| (0..count as u32).collect());
    if indices.iter().any(|&i| i as usize >= count) || !indices.len().is_multiple_of(3) {
        return Err("invalid triangle indices".into());
    }
    let uvs: Vec<f32> = r
        .read_tex_coords(0)
        .map(|v| v.into_f32().flatten().collect())
        .unwrap_or_else(|| vec![0.; count * 2]);
    let normals = r
        .read_normals()
        .map(|v| v.flatten().collect())
        .unwrap_or_else(|| normals(&positions, &indices));
    let mut mesh = MeshData {
        positions,
        colors: r
            .read_colors(0)
            .map(|v| v.into_rgba_f32().flatten().collect())
            .unwrap_or_default(),
        normals,
        uvs,
        joints: r
            .read_joints(0)
            .map(|v| v.into_u16().flatten().collect())
            .unwrap_or_default(),
        weights: r
            .read_weights(0)
            .map(|v| v.into_f32().flatten().collect())
            .unwrap_or_default(),
        indices,
        material: p.material().index().map_or(default, |i| i as u32),
        ..Default::default()
    };
    for weights in mesh.weights.chunks_exact_mut(4) {
        let sum = weights.iter().sum::<f32>();
        if !sum.is_finite() || sum <= 0.0 || weights.iter().any(|w| *w < 0.0) {
            return Err("skin weights must be nonnegative with positive sum".into());
        }
        for w in weights {
            *w /= sum;
        }
    }
    bounds(&mut mesh);
    Ok(mesh)
}
fn bounds(m: &mut MeshData) {
    let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    for p in m.positions.chunks_exact(3) {
        let p = Vec3::from_slice(p);
        lo = lo.min(p);
        hi = hi.max(p);
    }
    m.bounds = [lo.x, lo.y, lo.z, hi.x, hi.y, hi.z];
}
pub fn merge(a: &mut MeshData, b: MeshData) {
    let base = (a.positions.len() / 3) as u32;
    if !a.colors.is_empty() || !b.colors.is_empty() {
        if a.colors.is_empty() {
            a.colors.resize(base as usize * 4, 1.);
        }
        if b.colors.is_empty() {
            a.colors
                .extend(std::iter::repeat_n(1., b.positions.len() / 3 * 4));
        } else {
            a.colors.extend(b.colors);
        }
    }
    a.positions.extend(b.positions);
    a.normals.extend(b.normals);
    a.uvs.extend(b.uvs);
    a.joints.extend(b.joints);
    a.weights.extend(b.weights);
    a.indices.extend(b.indices.into_iter().map(|i| base + i));
    bounds(a);
}
fn normals(p: &[f32], indices: &[u32]) -> Vec<f32> {
    let mut n = vec![Vec3::ZERO; p.len() / 3];
    for tri in indices.chunks_exact(3) {
        let [a, b, c] = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
        let v = |i| Vec3::from_slice(&p[i * 3..i * 3 + 3]);
        let normal = (v(b) - v(a)).cross(v(c) - v(a));
        for i in [a, b, c] {
            n[i] += normal;
        }
    }
    n.into_iter()
        .flat_map(|n| n.try_normalize().unwrap_or(Vec3::Y).to_array())
        .collect()
}
