use crate::*;
use reader::align16;
use sha2::{Digest, Sha256};
use std::mem::size_of;

pub struct PageData {
    pub first_cluster: u32,
    pub cluster_count: u32,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u8>,
}
fn append<T: Pod>(out: &mut Vec<u8>, data: &[T]) {
    out.extend_from_slice(bytemuck::cast_slice(data));
    out.resize(align16(out.len()), 0);
}
/// Produces a canonical file and validates it before returning; no filesystem dependency.
pub fn encode(
    mut header: Header,
    clusters: &[Cluster],
    groups: &[Group],
    nodes: &[Node],
    pages: &[PageData],
) -> Result<Vec<u8>, Error> {
    header.magic = MAGIC;
    header.version = VERSION;
    header.header_bytes = size_of::<Header>() as u32;
    header.cluster_count =
        u32::try_from(clusters.len()).map_err(|_| Error("too many clusters".into()))?;
    header.group_count =
        u32::try_from(groups.len()).map_err(|_| Error("too many groups".into()))?;
    header.node_count = u32::try_from(nodes.len()).map_err(|_| Error("too many nodes".into()))?;
    header.page_count = u32::try_from(pages.len()).map_err(|_| Error("too many pages".into()))?;
    let mut out = vec![0; align16(size_of::<Header>())];
    header.clusters_offset = out.len() as u64;
    append(&mut out, clusters);
    header.groups_offset = out.len() as u64;
    append(&mut out, groups);
    header.pages_offset = out.len() as u64;
    out.resize(align16(out.len() + size_of::<Page>() * pages.len()), 0);
    header.nodes_offset = out.len() as u64;
    append(&mut out, nodes);
    header.geometry_offset = out.len() as u64;
    let mut table = Vec::with_capacity(pages.len());
    for p in pages {
        let offset = out.len();
        append(&mut out, &p.vertices);
        let indices_offset = out.len() - offset;
        append(&mut out, &p.indices);
        table.push(Page {
            offset: offset as u64,
            byte_length: (out.len() - offset) as u64,
            sha256: Sha256::digest(&out[offset..]).into(),
            first_cluster: p.first_cluster,
            cluster_count: p.cluster_count,
            vertex_count: u32::try_from(p.vertices.len())
                .map_err(|_| Error("page too large".into()))?,
            index_count: u32::try_from(p.indices.len())
                .map_err(|_| Error("page too large".into()))?,
            vertices_offset: 0,
            indices_offset: u32::try_from(indices_offset)
                .map_err(|_| Error("page too large".into()))?,
            reserved: [0; 2],
        });
    }
    header.file_bytes = out.len() as u64;
    out[..size_of::<Header>()].copy_from_slice(bytemuck::bytes_of(&header));
    let start = header.pages_offset as usize;
    out[start..start + table.len() * size_of::<Page>()]
        .copy_from_slice(bytemuck::cast_slice(&table));
    Reader::new(&out)?;
    Ok(out)
}
