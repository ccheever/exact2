//! A stream's drawing apart from its definitions. A stream carries both:
//! what the frame draws (matrices, shapes, clips, references to kept rows,
//! slots and layers) and what the reader makes once and keeps (faces,
//! pictures, rows, slots' and layers' drawings, keyframes, frees). A paint
//! that draws what the last stream drew and defines nothing new (a touch
//! down that changes nothing shown, after a stream that defined rows) is no
//! new frame for the reader: no ingest, no replay, no redraw of the window.
use super::layer::{CLOCK, LAYER_FREE, LAYER_REF, LAYER_SET, TRACKS};
use super::*;
use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard};

/// Earlier frames kept by a driver, or by a stream the definition walker cannot split.
static STREAMS: Mutex<VecDeque<Vec<u32>>> = Mutex::new(VecDeque::new());

fn streams() -> MutexGuard<'static, VecDeque<Vec<u32>>> {
    STREAMS.lock().unwrap_or_else(|e| e.into_inner())
}

pub(super) fn keep(ops: Option<Vec<u32>>) {
    if let Some(ops) = ops {
        streams().push_back(ops);
    }
}

/// Forget unread streams and pictures before a new reader reuses their IDs.
#[cfg(any(target_os = "android", test))]
pub(super) fn reader_replaced() {
    FINISHED.with(|f| *f.borrow_mut() = None);
    streams().clear();
    pending().clear();
}

pub(super) fn next_stream() -> Option<Vec<u32>> {
    streams().pop_front()
}

pub(super) fn waiting() -> bool {
    !streams().is_empty()
}

/// A turn can itself have queued older frames; its result follows them.
pub(super) fn after_turn(latest: Option<Vec<u32>>) -> Option<Vec<u32>> {
    let mut pending = streams();
    if pending.is_empty() {
        return latest;
    }
    pending.extend(latest);
    pending.pop_front()
}

/// Where the op at `i` ends, or `None` for a malformed stream.
pub(super) fn end(b: &[u32], i: usize) -> Option<usize> {
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
        FONT_AXES => {
            let len = i + 5 + 2 * at(4)?;
            len + 1 + words(*b.get(len)? as usize)
        }
        IMAGE_DEF => i + 4,
        STROKE => path(i + 5)?,
        IMAGE_RRECT => i + 18,
        DASH => i + 3 + at(2)?,
        gradient::SHADER => i + 2 + at(1)?,
        glyphs::BLUR => i + 2,
        ANIMATED => i + 3 + words(at(2)?),
        BACKDROP => i + 14 + 2 * at(1)?,
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

/// Retained drawing that samples or contains the world must stay in HWUI.
pub(super) fn needs_world(b: &[u32]) -> bool {
    let mut i = 0;
    while i < b.len() {
        let Some(j) = end(b, i).filter(|j| *j <= b.len()) else {
            return true;
        };
        match b[i] {
            BACKDROP => return true,
            NATIVE if b[i + 2] & 255 == 3 => return true,
            ROW_BEGIN if needs_world(&b[i + 6..j - 1]) => return true,
            SLOT_SET if needs_world(&b[i + 3..j]) => return true,
            LAYER_SET if needs_world(&b[i + 19..j]) => return true,
            _ => (),
        }
        i = j;
    }
    false
}

/// Whether op `op` defines something the reader keeps (rather than draws).
fn defines(op: u32) -> bool {
    matches!(
        op,
        FONT | FONT_AXES
            | IMAGE_DEF
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

/// Carry only retained definitions into a newer frame. Unknown streams stay whole.
fn carry_definitions(older: &[u32], newer: &mut Vec<u32>) -> bool {
    let mut definitions = Vec::new();
    let mut row_sets = false;
    let mut i = 0;
    while i < older.len() {
        let Some(j) = end(older, i).filter(|j| *j <= older.len()) else {
            return false;
        };
        if defines(older[i]) {
            row_sets = match older[i] {
                ROW_BEGIN => older[i + 2] & (1 << 31) == 0,
                SLOT_SET | LAYER_SET | TRACKS | CLOCK => row_sets,
                _ => false,
            };
            definitions.extend_from_slice(&older[i..j]);
        } else if row_sets {
            // Android defers contiguous sets after an unshown row; keep their boundary.
            return false;
        }
        i = j;
    }
    if row_sets
        && newer
            .first()
            .is_some_and(|op| matches!(*op, SLOT_SET | LAYER_SET | TRACKS | CLOCK))
    {
        return false;
    }
    if !definitions.is_empty() {
        definitions.append(newer);
        *newer = definitions;
    }
    true
}

/// What a frame's recording learns of whether its GPU canvas may be presented directly (op 28's
/// direct bit): the frame's only canvas, filling the window, with nothing composited into it.
#[derive(Default)]
pub(super) struct Direct {
    /// The window, in points.
    pub(super) size: (f32, f32),
    /// Opacity layers open where the recording is.
    pub(super) isolation: usize,
    /// Where an eligible canvas's NATIVE op starts in the recording.
    pub(super) at: Option<usize>,
    /// GPU canvases recorded.
    pub(super) surfaces: usize,
    /// Whether a backdrop filter reads what is under it.
    pub(super) backdrop: bool,
}

impl Recorder {
    pub(super) fn finish_stream(&mut self) -> Result<Pixmap, String> {
        self.layers_finish();
        if self.direct.surfaces == 1
            && !self.direct.backdrop
            && !self
                .kept
                .values()
                .any(|(ops, _)| needs_world(ops.get(3..).unwrap_or(&[u32::MAX])))
            && !self.slots.values().any(|ops| needs_world(ops))
            && !self.layers.needs_world()
        {
            if let Some(at) = self.direct.at {
                self.ops[at + 2] |= 1 << 8;
            }
        }
        let mut ops = std::mem::take(&mut self.ops);
        FINISHED.with(|f| {
            let mut pending = f.borrow_mut();
            let old = pending.take();
            #[cfg(target_os = "android")]
            let driven = agent::driven();
            #[cfg(not(target_os = "android"))]
            let driven = false;
            if let Some(old) = old {
                if driven || !carry_definitions(&old, &mut ops) {
                    keep(Some(old));
                }
            }
            *pending = Some(ops);
        });
        Pixmap::new(1, 1).ok_or_else(|| "placeholder".to_string())
    }
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

    #[test]
    fn ordered_backdrops_keep_the_following_drawing_aligned() {
        use exact_kernel::style::{BackdropFilter, BackdropOp};
        let mut recorder = Recorder::new();
        let shape = Shape::new((0.0, 0.0, 40.0, 30.0), [4.0; 4]);
        let filter = BackdropFilter(vec![BackdropOp::Saturate(1.8), BackdropOp::Blur(4.0)]);
        recorder.backdrop_filter(&shape, &filter, Transform::identity());
        let backdrop_end = recorder.ops.len();
        assert!(drawing(&recorder.ops[..backdrop_end - 1]).is_none());
        recorder.fill(&shape, [255, 0, 0, 255], Transform::identity());
        let (ops, definitions) = drawing(&recorder.ops).expect("backdrop followed by fill");
        assert!(!definitions);
        assert_eq!(ops, recorder.ops);
        assert_eq!(ops[backdrop_end], RRECT);
        // Radius-only readers cannot silently misinterpret the list's count.
        assert_ne!(BACKDROP, 27);
        assert!(drawing(&[27, 0]).is_none());
    }

    #[test]
    fn a_face_with_axes_is_a_definition() {
        let ital = u32::from_be_bytes(*b"ital");
        // Key 1, index 0, weight 400, one axis, then a five-byte path.
        let face = [FONT_AXES, 1, 0, 400, 1, ital, 1f32.to_bits(), 5, 0, 0];
        let glyphs = [GLYPHS, 1, 0, 0, 0, 1, 7, 0, 0];
        let stream: Vec<u32> = face.iter().chain(&glyphs).copied().collect();
        let (d, defs) = drawing(&stream).expect("known");
        assert!(defs);
        assert_eq!(d, glyphs);
        assert!(drawing(&face[..8]).is_none(), "a cut path is malformed");
    }

    #[test]
    fn every_retained_definition_precedes_the_newer_drawing() {
        let mut definitions = vec![FONT, 1, 0, 400, 0, IMAGE_DEF, 2, 10, 10];
        definitions.extend([FONT_AXES, 3, 0, 400, 0, 0]);
        definitions.extend([ANIMATED, 2, 0, ROW_BEGIN, 4, 0, 10, 10, 2, RESTORE, ROW_END]);
        definitions.extend([SLOT_SET, 5, 2, SLOT, 2, LAYER_SET, 6, 16]);
        definitions.extend([0; 16]);
        definitions.extend([
            TRACKS, 6, 0, CLOCK, 7, 0, IMAGE_FREE, 8, ROW_FREE, 9, LAYER_FREE, 10,
        ]);
        let matrix = [MATRIX, 1, 0, 0, 1, 0, 0];
        let older: Vec<_> = matrix
            .into_iter()
            .chain(definitions.iter().copied())
            .collect();
        let drawing = vec![ROW_DRAW, 4, 20, 20, LAYER_REF, 6];
        let mut latest = drawing.clone();
        assert!(carry_definitions(&older, &mut latest));
        definitions.extend(drawing);
        assert_eq!(latest, definitions);
    }

    #[test]
    fn deferred_row_sets_keep_their_drawing_and_frame_boundaries() {
        let row = [ROW_BEGIN, 7, 0, 10, 10, 1, ROW_END];
        let sets = [
            vec![SLOT_SET, 8, 2, SLOT, 1],
            [vec![LAYER_SET, 9, 16], vec![0; 16]].concat(),
            vec![TRACKS, 9, 0],
            vec![CLOCK, 9, 0],
        ];
        for set in &sets {
            for boundary in [
                vec![ROW_DRAW, 7, 0, 0],
                vec![MATRIX, 1, 0, 0, 1, 0, 0],
                vec![GROUP_END],
            ] {
                let older = [row.as_slice(), &sets[0], &boundary, set, &[ROW_FREE, 7]].concat();
                let mut latest = vec![ROW_DRAW, 10, 0, 0];
                let before = latest.clone();
                assert!(!carry_definitions(&older, &mut latest));
                assert_eq!(latest, before, "a rejected carry is atomic");
            }
            // The end of the older stream is also a boundary for Android's deferred sets.
            let older = [row.as_slice(), &sets[0]].concat();
            let mut latest = set.clone();
            let before = latest.clone();
            assert!(!carry_definitions(&older, &mut latest));
            assert_eq!(latest, before);
        }
        // A shown row cannot defer its following sets.
        let mut shown = row;
        shown[2] = 1 << 31;
        let older = [shown.as_slice(), &[ROW_DRAW, 7, 0, 0]].concat();
        assert!(carry_definitions(&older, &mut sets[0].clone()));
        // A retained non-set ends the first row's scope, but the next unshown row opens one.
        let older = [
            row.as_slice(),
            &[FONT, 1, 0, 400, 0],
            row.as_slice(),
            &[ROW_DRAW, 7, 0, 0],
        ]
        .concat();
        assert!(!carry_definitions(&older, &mut Vec::new()));
        let older = [row.as_slice(), &[FONT, 1, 0, 400, 0, ROW_DRAW, 7, 0, 0]].concat();
        assert!(carry_definitions(&older, &mut sets[0].clone()));
    }

    #[test]
    fn an_unknown_or_cut_stream_never_partially_changes_the_latest_frame() {
        for older in [
            vec![99],
            vec![IMAGE_DEF, 1, 10, 10, 99],
            vec![IMAGE_DEF, 1, 10, 10, GLYPHS, 1, 0, 0, 0, 5],
        ] {
            let mut latest = vec![ROW_DRAW, 2, 0, 0];
            let before = latest.clone();
            assert!(!carry_definitions(&older, &mut latest));
            assert_eq!(latest, before);
        }
    }

    #[test]
    fn overwritten_recordings_keep_definitions_and_order_unsplittable_frames() {
        FINISHED.with(|f| *f.borrow_mut() = None);
        streams().clear();
        let mut recorder = Recorder::new();
        let face = vec![FONT, 1, 0, 400, 0];
        let matrix = [MATRIX, 1, 0, 0, 1, 0, 0];
        let glyphs = vec![GLYPHS, 1, 12, 0xff000000, 0, 1, 7, 0, 0];
        recorder.ops = face.iter().copied().chain(matrix).collect();
        recorder.finish_stream().unwrap();
        recorder.ops = glyphs.clone();
        recorder.finish_stream().unwrap();
        let expected: Vec<_> = face.into_iter().chain(glyphs).collect();
        assert_eq!(FINISHED.with(|f| f.borrow_mut().take()), Some(expected));
        assert!(!waiting());

        recorder.ops = matrix.to_vec();
        recorder.finish_stream().unwrap();
        recorder.ops = vec![ROW_DRAW, 2, 0, 0];
        recorder.finish_stream().unwrap();
        assert_eq!(
            FINISHED.with(|f| f.borrow_mut().take()),
            Some(vec![ROW_DRAW, 2, 0, 0])
        );

        recorder.ops = vec![IMAGE_DEF, 1, 10, 10];
        recorder.finish_stream().unwrap();
        recorder.ops = vec![IMAGE_FREE, 1, IMAGE_DEF, 2, 10, 10];
        recorder.finish_stream().unwrap();
        recorder.ops = vec![IMAGE, 2, 0, 0, 10, 10];
        recorder.finish_stream().unwrap();
        assert_eq!(
            FINISHED.with(|f| f.borrow_mut().take()),
            Some(vec![
                IMAGE_DEF, 1, 10, 10, IMAGE_FREE, 1, IMAGE_DEF, 2, 10, 10, IMAGE, 2, 0, 0, 10, 10,
            ])
        );

        for older in [
            vec![99],
            vec![IMAGE_DEF, 1, 10],
            vec![IMAGE_DEF, 1, 10, 10, 99],
        ] {
            recorder.ops = older.clone();
            recorder.finish_stream().unwrap();
            let latest = vec![ROW_DRAW, 2, 0, 0];
            recorder.ops = latest.clone();
            recorder.finish_stream().unwrap();
            assert!(waiting());
            let newest = FINISHED.with(|f| f.borrow_mut().take());
            assert_eq!(after_turn(newest), Some(older));
            assert_eq!(next_stream(), Some(latest));
            assert!(!waiting());
        }

        let deferred = vec![
            ROW_BEGIN, 7, 0, 10, 10, 1, ROW_END, SLOT_SET, 8, 2, SLOT, 1, ROW_DRAW, 7, 0, 0,
            SLOT_SET, 9, 2, SLOT, 2,
        ];
        recorder.ops = deferred.clone();
        recorder.finish_stream().unwrap();
        recorder.ops = vec![ROW_FREE, 7];
        recorder.finish_stream().unwrap();
        let newest = FINISHED.with(|f| f.borrow_mut().take());
        assert_eq!(after_turn(newest), Some(deferred));
        assert_eq!(next_stream(), Some(vec![ROW_FREE, 7]));
        assert!(!waiting());

        for op in [99, 98, 97] {
            recorder.ops = vec![op];
            recorder.finish_stream().unwrap();
        }
        let newest = FINISHED.with(|f| f.borrow_mut().take());
        assert_eq!(after_turn(newest), Some(vec![99]));
        assert_eq!(next_stream(), Some(vec![98]));
        assert_eq!(next_stream(), Some(vec![97]));
        assert!(!waiting());

        // Adoption discards both queued and finished old-reader definitions before IDs restart.
        let old = Picture::Canvas(Arc::new(Pixmap::new(1, 1).unwrap()));
        assert_eq!(recorder.image_id(&old), 1);
        recorder.finish_stream().unwrap();
        keep(FINISHED.with(|f| f.borrow_mut().take()));
        recorder.ops = vec![IMAGE, 1, 0, 0, 1, 1];
        recorder.finish_stream().unwrap();
        assert!(waiting());
        assert!(pending().contains_key(&1));
        reader_replaced();
        assert!(!waiting());
        assert!(FINISHED.with(|f| f.borrow().is_none()));
        assert!(pending().is_empty());

        let new = Picture::Canvas(Arc::new(Pixmap::new(2, 1).unwrap()));
        let mut recorder = Recorder::new();
        assert_eq!(recorder.image_id(&new), 1);
        recorder.finish_stream().unwrap();
        let newest = FINISHED.with(|f| f.borrow_mut().take());
        assert_eq!(after_turn(newest), Some(vec![IMAGE_DEF, 1, 2, 1]));
        assert_eq!(pending().remove(&1).unwrap().key(), new.key());
        assert!(next_stream().is_none());
    }
}
