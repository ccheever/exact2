use crate::*;
use sha2::{Digest, Sha256};
use std::{fmt, mem::size_of};

#[derive(Debug, Clone)]
pub struct Error(pub String);
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}
fn require(ok: bool, message: &str) -> Result<(), Error> {
    if ok {
        Ok(())
    } else {
        Err(Error(message.into()))
    }
}
fn section<T: Pod>(data: &[u8], offset: u64, count: u32) -> Result<&[T], Error> {
    let start = usize::try_from(offset).map_err(|_| Error("section offset overflow".into()))?;
    let end = (count as usize)
        .checked_mul(size_of::<T>())
        .and_then(|n| start.checked_add(n))
        .ok_or_else(|| Error("section size overflow".into()))?;
    let bytes = data
        .get(start..end)
        .ok_or_else(|| Error("section outside file".into()))?;
    bytemuck::try_cast_slice(bytes).map_err(|_| Error("unaligned section".into()))
}
fn valid_bounds(b: &Bounds) -> bool {
    b.center.iter().all(|v| v.is_finite())
        && b.radius.is_finite()
        && b.radius >= 0.0
        && b.error.is_finite()
        && b.error >= 0.0
}
pub(crate) fn align16(n: usize) -> Result<usize, Error> {
    n.checked_add(15)
        .map(|n| n & !15)
        .ok_or_else(|| Error("alignment overflow".into()))
}
fn zero_padding(bytes: &[u8], start: usize, end: usize) -> Result<(), Error> {
    require(
        bytes
            .get(start..end)
            .is_some_and(|b| b.iter().all(|x| *x == 0)),
        "nonzero or missing alignment padding",
    )
}
/// Tolerance covers float32 sphere construction: eight ulps at the coordinate/radius scale.
fn contains(center: [f32; 3], radius: f32, child: [f32; 3], child_radius: f32) -> bool {
    let magnitude = center
        .iter()
        .chain(&child)
        .map(|x| (*x as f64).abs())
        .fold(radius.max(child_radius) as f64, f64::max);
    let distance = center
        .iter()
        .zip(child)
        .map(|(&a, b)| (a as f64 - b as f64).powi(2))
        .sum::<f64>()
        .sqrt();
    distance + child_radius as f64 <= radius as f64 + 8.0 * f32::EPSILON as f64 * magnitude
}

/// Borrows aligned bytes; validates all offsets, indices, digests and graph references.
/// Unaligned input is rejected rather than copied or dereferenced unsafely.
pub struct Reader<'a> {
    bytes: &'a [u8],
    pub header: &'a Header,
    pub clusters: &'a [Cluster],
    pub groups: &'a [Group],
    pub pages: &'a [Page],
    pub nodes: &'a [Node],
}
impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Result<Self, Error> {
        require(
            cfg!(target_endian = "little"),
            "little-endian host required",
        )?;
        let h = &section::<Header>(bytes, 0, 1)?[0];
        require(
            h.magic == MAGIC
                && h.version == VERSION
                && h.header_bytes as usize == size_of::<Header>(),
            "unsupported header",
        )?;
        require(h.file_bytes == bytes.len() as u64, "file length mismatch")?;
        require(
            h.source_vertices > 0
                && h.source_triangles > 0
                && h.cluster_count > 0
                && h.group_count > 0
                && h.page_count > 0,
            "empty mesh",
        )?;
        require(
            h.flags & !HAS_COLOR == 0 && h.reserved == [0; 2],
            "unknown flags or reserved words",
        )?;
        let cfg = &h.config;
        require(
            (4..=128).contains(&cfg.max_triangles)
                && cfg.page_bytes >= 4096
                && cfg.page_bytes <= 128 * 1024 * 1024
                && cfg.vertex_encoding == 1
                && cfg.partition_size == 16
                && cfg.reserved == [0; 2],
            "unsupported bake config",
        )?;
        require(
            [
                cfg.normal_weight,
                cfg.color_weight,
                cfg.simplify_ratio,
                cfg.simplify_threshold,
                cfg.error_merge_previous,
                cfg.error_merge_additive,
            ]
            .iter()
            .all(|v| v.is_finite() && *v >= 0.0),
            "invalid config floats",
        )?;
        let clusters = section(bytes, h.clusters_offset, h.cluster_count)?;
        let groups = section(bytes, h.groups_offset, h.group_count)?;
        let pages = section(bytes, h.pages_offset, h.page_count)?;
        let nodes = section(bytes, h.nodes_offset, h.node_count)?;
        let mut next = align16(size_of::<Header>())?;
        zero_padding(bytes, size_of::<Header>(), next)?;
        for (offset, count, stride) in [
            (h.clusters_offset, h.cluster_count, size_of::<Cluster>()),
            (h.groups_offset, h.group_count, size_of::<Group>()),
            (h.pages_offset, h.page_count, size_of::<Page>()),
            (h.nodes_offset, h.node_count, size_of::<Node>()),
        ] {
            require(
                offset == next as u64,
                "noncanonical or overlapping sections",
            )?;
            let end = next
                .checked_add(
                    (count as usize)
                        .checked_mul(stride)
                        .ok_or_else(|| Error("size overflow".into()))?,
                )
                .ok_or_else(|| Error("size overflow".into()))?;
            next = align16(end)?;
            zero_padding(bytes, end, next)?;
        }
        require(h.geometry_offset == next as u64, "bad geometry offset")?;
        let r = Self {
            bytes,
            header: h,
            clusters,
            groups,
            pages,
            nodes,
        };
        r.validate(next)?;
        Ok(r)
    }
    fn validate(&self, mut next: usize) -> Result<(), Error> {
        let mut cluster_cursor = 0usize;
        let mut has_color = false;
        for (pi, p) in self.pages.iter().enumerate() {
            require(
                p.offset == next as u64
                    && p.byte_length > 0
                    && p.byte_length <= self.header.config.page_bytes as u64
                    && p.reserved == [0; 2],
                "invalid page extent",
            )?;
            let length =
                usize::try_from(p.byte_length).map_err(|_| Error("page length overflow".into()))?;
            next = next
                .checked_add(length)
                .ok_or_else(|| Error("page overflow".into()))?;
            require(
                next <= self.bytes.len() && next.is_multiple_of(16),
                "page outside file or unaligned",
            )?;
            require(
                p.first_cluster as usize == cluster_cursor && p.cluster_count > 0,
                "page cluster partition",
            )?;
            let end = cluster_cursor
                .checked_add(p.cluster_count as usize)
                .ok_or_else(|| Error("cluster overflow".into()))?;
            let cs = self
                .clusters
                .get(cluster_cursor..end)
                .ok_or_else(|| Error("page clusters outside table".into()))?;
            let data = &self.bytes[p.offset as usize..next];
            require(
                <[u8; 32]>::from(Sha256::digest(data)) == p.sha256,
                "page digest mismatch",
            )?;
            require(
                p.vertex_count as usize <= length / size_of::<Vertex>()
                    && p.indices_offset as usize <= length
                    && p.index_count as usize <= length - p.indices_offset as usize,
                "page counts exceed extent",
            )?;
            require(
                p.vertices_offset == 0
                    && p.indices_offset as usize
                        == align16(p.vertex_count as usize * size_of::<Vertex>())?
                    && align16(p.indices_offset as usize + p.index_count as usize)? == length,
                "page layout mismatch",
            )?;
            let vertices = section::<Vertex>(data, p.vertices_offset as u64, p.vertex_count)?;
            zero_padding(
                data,
                p.vertex_count as usize * size_of::<Vertex>(),
                p.indices_offset as usize,
            )?;
            zero_padding(
                data,
                p.indices_offset as usize + p.index_count as usize,
                length,
            )?;
            has_color |= vertices.iter().any(|v| v.color != u32::MAX);
            require(
                vertices
                    .iter()
                    .all(|v| v.position.iter().all(|x| x.is_finite())),
                "nonfinite position",
            )?;
            let indices = section::<u8>(data, p.indices_offset as u64, p.index_count)?;
            let mut vc = 0usize;
            let mut tc = 0usize;
            for c in cs {
                require(
                    c.page as usize == pi
                        && c.vertex_offset as usize == vc
                        && c.triangle_offset as usize == tc,
                    "cluster geometry partition",
                )?;
                require(
                    c.vertex_count > 0
                        && c.vertex_count <= 256
                        && c.triangle_count > 0
                        && c.triangle_count <= self.header.config.max_triangles
                        && c.reserved == [0; 3],
                    "invalid cluster counts",
                )?;
                vc += c.vertex_count as usize;
                let te = tc + c.triangle_count as usize * 3;
                require(vc <= vertices.len(), "cluster vertices out of range")?;
                require(
                    vertices[c.vertex_offset as usize..vc].iter().all(|v| {
                        contains(
                            [c.sphere[0], c.sphere[1], c.sphere[2]],
                            c.sphere[3],
                            v.position,
                            0.0,
                        )
                    }),
                    "culling sphere excludes vertices",
                )?;
                let local = indices
                    .get(tc..te)
                    .ok_or_else(|| Error("cluster indices out of range".into()))?;
                require(
                    local.iter().all(|v| (*v as u32) < c.vertex_count),
                    "local index out of range",
                )?;
                tc = te;
            }
            require(
                vc == vertices.len() && tc == indices.len(),
                "unclaimed page geometry",
            )?;
            cluster_cursor = end;
        }
        require(
            next == self.bytes.len() && cluster_cursor == self.clusters.len(),
            "unclaimed bytes or clusters",
        )?;
        require(
            has_color == (self.header.flags & HAS_COLOR != 0),
            "color flag disagrees with vertex data",
        )?;
        let mut group_use = vec![false; self.clusters.len()];
        let mut referenced = vec![false; self.groups.len()];
        let mut originals = 0u64;
        for (gi, g) in self.groups.iter().enumerate() {
            require(
                valid_bounds(&g.simplified)
                    && g.simplified.error > 0.0
                    && g.cluster_count > 0
                    && (g.depth as usize) < self.groups.len(),
                "invalid group",
            )?;
            let start = g.first_cluster as usize;
            let end = start
                .checked_add(g.cluster_count as usize)
                .ok_or_else(|| Error("group overflow".into()))?;
            let cs = self
                .clusters
                .get(start..end)
                .ok_or_else(|| Error("group clusters outside table".into()))?;
            for (ci, c) in cs.iter().enumerate() {
                require(
                    !group_use[start + ci]
                        && c.group as usize == gi
                        && c.depth == g.depth
                        && c.simplified == g.simplified,
                    "group membership mismatch",
                )?;
                group_use[start + ci] = true;
                require(
                    c.sphere
                        .iter()
                        .chain(&c.cone_apex)
                        .chain(&c.cone_axis)
                        .all(|x| x.is_finite())
                        && c.sphere[3] >= 0.0
                        && c.cone_cutoff.is_finite()
                        && (-1.0..=1.0).contains(&c.cone_cutoff),
                    "invalid culling bounds",
                )?;
                if c.refined == ORIGINAL {
                    require(
                        c.refined_bounds == Bounds::default() && c.depth == 0,
                        "invalid original sentinel",
                    )?;
                    originals += c.triangle_count as u64;
                } else {
                    let child = self
                        .groups
                        .get(c.refined as usize)
                        .ok_or_else(|| Error("refined group outside table".into()))?;
                    require(
                        (c.refined as usize) < gi
                            && child.depth < g.depth
                            && c.refined_bounds == child.simplified
                            && child.simplified.error <= g.simplified.error,
                        "invalid or cyclic refinement",
                    )?;
                    referenced[c.refined as usize] = true;
                    require(
                        contains(
                            g.simplified.center,
                            g.simplified.radius,
                            child.simplified.center,
                            child.simplified.radius,
                        ),
                        "ancestor sphere excludes descendant",
                    )?;
                }
                require(
                    g.simplified.error != f32::MAX || c.page == 0,
                    "terminal group absent from page zero",
                )?;
            }
        }
        require(
            group_use.iter().all(|x| *x) && originals == self.header.source_triangles as u64,
            "group coverage or source triangle count mismatch",
        )?;
        require(
            self.groups
                .iter()
                .zip(referenced)
                .all(|(g, r)| r == (g.simplified.error != f32::MAX)),
            "terminal/refinement reference mismatch",
        )?;
        self.validate_nodes()
    }
    fn validate_nodes(&self) -> Result<(), Error> {
        if self.nodes.is_empty() {
            return require(self.header.root_count == 0, "roots without nodes");
        }
        let levels = self.groups.iter().map(|g| g.depth).max().unwrap_or(0) + 1;
        require(
            self.header.root_count == levels && levels as usize <= self.nodes.len(),
            "invalid BVH roots",
        )?;
        let mut seen = vec![false; self.nodes.len()];
        let mut groups = vec![false; self.groups.len()];
        let mut pending: Vec<_> = (0..levels).map(|i| (i as usize, i)).collect();
        while let Some((id, depth)) = pending.pop() {
            let n = self
                .nodes
                .get(id)
                .ok_or_else(|| Error("BVH node outside table".into()))?;
            require(
                !seen[id] && valid_bounds(&n.bounds),
                "cyclic, shared or invalid BVH node",
            )?;
            seen[id] = true;
            if n.group != ORIGINAL {
                let g = self
                    .groups
                    .get(n.group as usize)
                    .ok_or_else(|| Error("BVH group outside table".into()))?;
                require(
                    !groups[n.group as usize]
                        && g.depth == depth
                        && n.bounds == g.simplified
                        && n.child_count == 0
                        && n.child_offset == 0,
                    "invalid BVH leaf",
                )?;
                groups[n.group as usize] = true;
            } else {
                let end = (n.child_offset as usize)
                    .checked_add(n.child_count as usize)
                    .ok_or_else(|| Error("BVH overflow".into()))?;
                require(
                    n.child_count > 0 && n.child_count <= 8 && end <= self.nodes.len(),
                    "invalid BVH children",
                )?;
                for i in n.child_offset as usize..end {
                    pending.push((i, depth));
                }
            }
        }
        require(
            seen.iter().all(|x| *x) && groups.iter().all(|x| *x),
            "unreachable BVH nodes or groups",
        )
    }
    pub fn page_bytes(&self, page: usize) -> Option<&'a [u8]> {
        let p = self.pages.get(page)?;
        let start = usize::try_from(p.offset).ok()?;
        let end = usize::try_from(p.offset.checked_add(p.byte_length)?).ok()?;
        self.bytes.get(start..end)
    }
    pub fn geometry(&self, cluster: usize) -> Option<(&'a [Vertex], &'a [u8])> {
        let c = self.clusters.get(cluster)?;
        let p = self.pages.get(c.page as usize)?;
        let b = self.page_bytes(c.page as usize)?;
        let v = section::<Vertex>(b, p.vertices_offset as u64, p.vertex_count).ok()?;
        let indices = b.get(p.indices_offset as usize..)?;
        Some((
            v.get(c.vertex_offset as usize..c.vertex_offset.checked_add(c.vertex_count)? as usize)?,
            indices.get(
                c.triangle_offset as usize
                    ..(c.triangle_offset as usize)
                        .checked_add((c.triangle_count as usize).checked_mul(3)?)?,
            )?,
        ))
    }
}
