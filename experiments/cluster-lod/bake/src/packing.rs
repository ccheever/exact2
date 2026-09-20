use crate::{Mesh, Result, ffi::Built};
use bytemuck::Zeroable;
use clod_format::{Config, Header, PageData, Vertex, encode, pack_normal};
use std::time::Instant;

pub struct Bake {
    pub bytes: Vec<u8>,
    /// Checked transitions, direct boundary-chain rejects, and all stopped transitions.
    pub topology: [u32; 3],
    pub normals_seconds: f64,
    pub build_seconds: f64,
    pub encode_seconds: f64,
}
fn page_size(vertices: usize, indices: usize) -> usize {
    ((vertices * 20 + 15) & !15) + ((indices + 15) & !15)
}
pub fn bake(mesh: &mut Mesh, config: Config, source_sha256: [u8; 32]) -> Result<Bake> {
    mesh.validate()?;
    if !(4..=128).contains(&config.max_triangles)
        || config.page_bytes < 4096
        || config.page_bytes > 128 * 1024 * 1024
        || config
            != (Config {
                max_triangles: config.max_triangles,
                page_bytes: config.page_bytes,
                ..Config::default()
            })
    {
        return Err("unsupported bake config (max triangles 4..128, pages 4096..134217728 bytes; other fields use defaults)".into());
    }
    let start = Instant::now();
    mesh.compute_normals();
    mesh.validate()?;
    let normals_seconds = start.elapsed().as_secs_f64();
    let start = Instant::now();
    let built = Built::new(mesh, config)?;
    let build_seconds = start.elapsed().as_secs_f64();
    let start = Instant::now();
    let mut order: Vec<usize> = (0..built.groups().len()).collect();
    order.sort_by_key(|&i| {
        (
            built.groups()[i].simplified.error != f32::MAX,
            std::cmp::Reverse(built.groups()[i].depth),
            i,
        )
    });
    let mut groups = built.groups().to_vec();
    let mut clusters = Vec::with_capacity(built.clusters().len());
    let mut pages = vec![PageData {
        first_cluster: 0,
        cluster_count: 0,
        vertices: Vec::new(),
        indices: Vec::new(),
    }];
    for gi in order {
        let g = built.groups()[gi];
        groups[gi].first_cluster = clusters.len() as u32;
        for raw in &built.clusters()
            [g.first_cluster as usize..(g.first_cluster + g.cluster_count) as usize]
        {
            let p = pages.last().expect("page exists");
            if page_size(
                p.vertices.len() + raw.vertex_count as usize,
                p.indices.len() + raw.triangle_count as usize * 3,
            ) > config.page_bytes as usize
            {
                if g.simplified.error == f32::MAX {
                    return Err(
                        "terminal cut exceeds page budget; increase --page-mib (maximum 128)"
                            .into(),
                    );
                }
                pages.push(PageData {
                    first_cluster: clusters.len() as u32,
                    cluster_count: 0,
                    vertices: Vec::new(),
                    indices: Vec::new(),
                });
            }
            let mut c = *raw;
            c.page = (pages.len() - 1) as u32;
            let p = pages.last_mut().expect("page exists");
            c.vertex_offset = p.vertices.len() as u32;
            c.triangle_offset = p.indices.len() as u32;
            let ids = &built.vertices()
                [raw.vertex_offset as usize..(raw.vertex_offset + raw.vertex_count) as usize];
            for &id in ids {
                let id = id as usize;
                p.vertices.push(Vertex {
                    position: mesh.positions[id],
                    normal: pack_normal(mesh.normals[id]),
                    color: u32::from_le_bytes(mesh.colors.as_ref().map_or([255; 4], |cs| cs[id])),
                });
            }
            p.indices.extend_from_slice(
                &built.indices()[raw.triangle_offset as usize
                    ..raw.triangle_offset as usize + raw.triangle_count as usize * 3],
            );
            p.cluster_count += 1;
            clusters.push(c);
        }
    }
    let header = Header {
        source_sha256,
        source_vertices: mesh.positions.len() as u32,
        source_triangles: (mesh.indices.len() / 3) as u32,
        flags: pages.iter().flat_map(|p| &p.vertices).fold(0, |flags, v| {
            flags
                | (u32::from(v.color & 0x00ff_ffff != 0x00ff_ffff) * clod_format::HAS_COLOR)
                | (u32::from(v.color >> 24 != 255) * clod_format::HAS_AO)
        }),
        root_count: groups.iter().map(|g| g.depth).max().unwrap_or(0) + 1,
        config,
        ..Header::zeroed()
    };
    let bytes = encode(header, &clusters, &groups, built.nodes(), &pages)?;
    Ok(Bake {
        bytes,
        topology: built.topology(),
        normals_seconds,
        build_seconds,
        encode_seconds: start.elapsed().as_secs_f64(),
    })
}
