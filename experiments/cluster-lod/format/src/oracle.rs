//! Decoded triangle edge oracle shared by bake and GPU-cut checks.
#[derive(Default, Debug)]
pub struct Edges {
    pub bad: usize,
    pub boundary: usize,
    pub max_use: usize,
    pub degenerate: usize,
}
pub fn edges(triangles: &[[u32; 3]]) -> Edges {
    let mut edges = Vec::with_capacity(triangles.len() * 3);
    let mut stats = Edges::default();
    for t in triangles {
        if t[0] == t[1] || t[1] == t[2] || t[2] == t[0] {
            stats.degenerate += 1;
        }
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            edges.push(((a.min(b) as u64) << 32) | a.max(b) as u64);
        }
    }
    edges.sort_unstable();
    let mut i = 0;
    while i < edges.len() {
        let mut end = i + 1;
        while end < edges.len() && edges[end] == edges[i] {
            end += 1;
        }
        let uses = end - i;
        stats.max_use = stats.max_use.max(uses);
        if uses != 2 {
            stats.bad += 1;
            if uses == 1 {
                stats.boundary += 1;
            }
        }
        i = end;
    }
    stats
}
