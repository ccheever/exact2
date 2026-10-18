//! The web editor's compact adapter to the shared UTF-16 range rules.

use exact_kernel::text::decorations::{Edit, Range};

/// Map UTF-16 ranges through the one replacement between two source strings.
/// Each input triple is `(start, end, fragile)`. Fragile hidden/atomic ranges
/// touching the edit are omitted. Output triples retain their input index.
pub fn reconcile(before: &str, current: &str, ranges: &[u32]) -> Vec<u32> {
    let edit = Edit::between(before, current);
    let old_length = before.encode_utf16().count() as u32;
    let length = current.encode_utf16().count() as u32;
    ranges
        .chunks_exact(3)
        .enumerate()
        .filter_map(|(index, triple)| {
            let raw = Range {
                start: triple[0],
                end: triple[1],
            };
            if before != current && triple[2] != 0 && edit.intersects(raw) {
                return None;
            }
            let range = raw.clamped(old_length)?;
            let mapped = Range {
                start: edit.boundary(range.start),
                end: edit.boundary(range.end),
            }
            .clamped(length)?;
            Some([index as u32, mapped.start, mapped.end])
        })
        .flatten()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::reconcile;

    #[test]
    fn stale_ranges_share_the_kernels_scalar_safe_edit_and_fragile_edges() {
        assert_eq!(
            reconcile("a😀b", "a😁b", &[1, 3, 0, 1, 3, 1, 3, 4, 0]),
            [0, 1, 3, 2, 3, 4]
        );
        assert_eq!(reconcile("hi @a", "hi !@a", &[3, 5, 0, 3, 5, 1]), [0, 3, 6]);
        assert_eq!(reconcile("hi @a", "hi @a", &[3, 99, 1]), [0, 3, 5]);
    }
}
