//! Decoded triangle edge oracle shared by bake and GPU-cut checks.
#[derive(Default, Debug)]
pub struct Edges {
    pub bad: usize,
    pub boundary: usize,
    pub max_use: usize,
    pub degenerate: usize,
    /// Balanced edges with more than two incident triangles (not failures).
    pub pinches: Vec<[u32; 2]>,
}
pub fn edges(triangles: &[[u32; 3]]) -> Edges {
    let mut edges = Vec::with_capacity(triangles.len() * 3);
    let mut stats = Edges::default();
    for t in triangles {
        if t[0] == t[1] || t[1] == t[2] || t[2] == t[0] {
            stats.degenerate += 1;
        }
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            edges.push((
                ((a.min(b) as u64) << 32) | a.max(b) as u64,
                if a < b {
                    1i32
                } else if a > b {
                    -1
                } else {
                    0
                },
            ));
        }
    }
    edges.sort_unstable();
    let mut i = 0;
    while i < edges.len() {
        let mut end = i + 1;
        while end < edges.len() && edges[end].0 == edges[i].0 {
            end += 1;
        }
        let uses = end - i;
        stats.max_use = stats.max_use.max(uses);
        let winding: i32 = edges[i..end].iter().map(|e| e.1).sum();
        if winding != 0 {
            stats.bad += 1;
            if uses == 1 {
                stats.boundary += 1;
            }
        }
        if uses > 2 && winding == 0 {
            stats
                .pinches
                .push([(edges[i].0 >> 32) as u32, edges[i].0 as u32]);
        }
        i = end;
    }
    stats
}

pub fn overlaps(reader: &crate::Reader<'_>, selected: &[bool]) -> usize {
    // Mark every descendant group of every selected cluster in reverse topological order.
    let mut descendant = vec![false; reader.groups.len()];
    for (c, &s) in reader.clusters.iter().zip(selected) {
        if s && c.refined != crate::ORIGINAL {
            descendant[c.refined as usize] = true;
        }
    }
    let mut count = 0;
    for (gi, g) in reader.groups.iter().enumerate().rev() {
        if !descendant[gi] {
            continue;
        }
        let range = g.first_cluster as usize..(g.first_cluster + g.cluster_count) as usize;
        for (c, &is_selected) in reader.clusters[range.clone()].iter().zip(&selected[range]) {
            count += usize::from(is_selected);
            if c.refined != crate::ORIGINAL {
                descendant[c.refined as usize] = true;
            }
        }
    }
    count
}

#[cfg(test)]
mod tests {
    #[test]
    fn oriented_chains_distinguish_pinches_holes_and_flips() {
        let tetra = [[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]];
        let mut flipped = tetra;
        flipped[0].swap(0, 1);
        let pinched = [
            tetra.as_slice(),
            &[[0, 4, 1], [0, 1, 5], [1, 4, 5], [4, 0, 5]],
        ]
        .concat();
        let odd = [tetra.as_slice(), &[[0, 2, 1]]].concat();
        let cases = [
            ("closed", tetra.to_vec(), 0, 0, 2),
            ("pinch", pinched, 0, 1, 4),
            ("hole", tetra[1..].to_vec(), 3, 0, 2),
            ("flip", flipped.to_vec(), 3, 0, 2),
            ("odd", odd, 3, 0, 3),
        ];
        let mut failures = Vec::new();
        for (name, triangles, bad, pinches, worst) in cases {
            let e = super::edges(&triangles);
            println!(
                "chain_case={name} triangles={} nonzero={} pinches={} worst={}",
                triangles.len(),
                e.bad,
                e.pinches.len(),
                e.max_use
            );
            if (e.bad, e.pinches.len(), e.max_use) != (bad, pinches, worst) {
                failures.push(name);
            }
        }
        assert!(failures.is_empty(), "{failures:?}");
    }
}
