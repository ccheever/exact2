//! A stream's drawing apart from its definitions. A stream carries both:
//! what the frame draws (matrices, shapes, clips, references to kept rows,
//! slots and layers) and what the reader makes once and keeps (faces,
//! pictures, rows, slots' and layers' drawings, keyframes, frees). A paint
//! that draws what the last stream drew and defines nothing new (a touch
//! down that changes nothing shown, after a stream that defined rows) is no
//! new frame for the reader: no ingest, no replay, no redraw of the window.
use super::layer::{CLOCK, LAYER_FREE, LAYER_REF, LAYER_SET, TRACKS};
use super::*;

/// Where the op at `i` ends, or `None` for a malformed stream.
fn end(b: &[u32], i: usize) -> Option<usize> {
    let at = |k: usize| b.get(i + k).map(|v| *v as usize);
    // A path's tagged segments from its count at `p`.
    let path = |p: usize| -> Option<usize> {
        let n = *b.get(p)? as usize;
        let mut k = p + 1;
        for _ in 0..n {
            k += match *b.get(k)? {
                0 | 1 => 3,
                2 => 7,
                _ => 1,
            };
        }
        Some(k)
    };
    let words = |len: usize| len.div_ceil(4);
    Some(match *b.get(i)? {
        MATRIX => i + 7,
        RRECT => i + 14,
        PATH => path(i + 3)?,
        CLIP_RRECT => i + 13,
        CLIP_PATH => path(i + 2)?,
        RESTORE | ROW_END | GROUP_END => i + 1,
        LAYER | IMAGE_FREE | ROW_FREE | GROUP_BEGIN | SLOT | LAYER_FREE | LAYER_REF => i + 2,
        IMAGE => i + 6,
        GLYPHS => i + 6 + 3 * at(5)?,
        FONT => i + 5 + words(at(4)?),
        IMAGE_DEF => i + 4,
        STROKE => path(i + 5)?,
        IMAGE_RRECT => i + 18,
        DASH => i + 3 + at(2)?,
        ANIMATED => i + 3 + words(at(2)?),
        BACKDROP => i + 14,
        NATIVE => i + 16 + words(at(15)?),
        40 => i + 27, // SHADOW
        ROW_BEGIN => i + 6 + at(5)?,
        ROW_DRAW => i + 4,
        RING => i + 26,
        SLOT_SET | LAYER_SET | TRACKS => i + 3 + at(2)?,
        CLOCK => i + 3,
        _ => return None,
    })
}

/// Whether op `op` defines something the reader keeps (rather than draws).
fn defines(op: u32) -> bool {
    matches!(
        op,
        FONT | IMAGE_DEF
            | IMAGE_FREE
            | ANIMATED
            | ROW_BEGIN
            | ROW_FREE
            | SLOT_SET
            | LAYER_SET
            | TRACKS
            | LAYER_FREE
            | CLOCK
    )
}

/// The stream's drawing (its definitions left out), and whether it defines
/// anything; `None` for a stream this walk does not know (never skipped).
pub fn drawing(b: &[u32]) -> Option<(Vec<u32>, bool)> {
    let mut out = Vec::with_capacity(b.len());
    let mut defs = false;
    let mut i = 0;
    while i < b.len() {
        let j = end(b, i).filter(|j| *j <= b.len())?;
        if defines(b[i]) {
            defs = true;
        } else {
            out.extend_from_slice(&b[i..j]);
        }
        i = j;
    }
    Some((out, defs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definitions_apart() {
        let row = [ROW_BEGIN, 7, 0, 0, 0, 2, RESTORE, ROW_END];
        let mut with = vec![MATRIX, 0, 0, 0, 0, 0, 0];
        with.extend(row);
        with.extend([ROW_DRAW, 7, 0, 0, TRACKS, 3, 1, 9]);
        let (d, defs) = drawing(&with).expect("known");
        assert!(defs);
        assert_eq!(d, vec![MATRIX, 0, 0, 0, 0, 0, 0, ROW_DRAW, 7, 0, 0]);
        let (again, defs) = drawing(&d).expect("known");
        assert!(!defs);
        assert_eq!(again, d);
        assert!(drawing(&[99]).is_none());
        assert!(drawing(&[GLYPHS, 1, 0, 0, 0, 5]).is_none());
    }
}
