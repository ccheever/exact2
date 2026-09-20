//! Whole-group availability with ancestor closure, independent of camera and executor.
use clod_format::{ORIGINAL, Reader};

pub fn groups(reader: &Reader<'_>, pages: &[bool]) -> Result<Vec<u32>, String> {
    if pages.len() != reader.pages.len() || !pages[0] {
        return Err("residency requires page zero and exactly one bit per page".into());
    }
    let mut ready: Vec<u32> = reader
        .groups
        .iter()
        .map(|g| {
            u32::from(
                reader.clusters
                    [g.first_cluster as usize..(g.first_cluster + g.cluster_count) as usize]
                    .iter()
                    .all(|c| pages[c.page as usize]),
            )
        })
        .collect();
    // Coarse groups have higher IDs. A child can be entered only if every
    // parent is available: all clusters replacing it then make the same decision.
    for (id, g) in reader.groups.iter().enumerate().rev() {
        for c in
            &reader.clusters[g.first_cluster as usize..(g.first_cluster + g.cluster_count) as usize]
        {
            if c.refined != ORIGINAL {
                ready[c.refined as usize] &= ready[id];
            }
        }
    }
    Ok(ready)
}
