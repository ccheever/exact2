//! The recorder's layers (`crate::host::lower`): a node its reader animates
//! is drawn by reference (`LAYER_REF`) from its row, its drawing sent apart
//! (`LAYER_SET`) only when it changes, with the transform, pivot and base
//! values the reader moves it from. A layer no kept row or frame draws any
//! more is freed (`LAYER_FREE`); its keyframes (`TRACKS`) come from the host
//! after the paint.
use super::{Recorder, RESTORE};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use tiny_skia::Transform;

/// `[29, id]`: draw layer `id` here (its reader's node, recorded from the
/// last `LAYER_SET`).
pub(super) const LAYER_REF: u32 = 29;
/// `[30, id, len, m0..m5, px, py, tx, ty, scale, rotate, opacity, r, dash,
/// at, ops…]`: a layer's matrix (points → the stream's pixels), its pivot and
/// base values in points before the matrix (`dash`: path units per dash
/// offset unit), where its drawing's dash phase is (a word index into the
/// ops, or `u32::MAX`), then its drawing.
pub(super) const LAYER_SET: u32 = 30;
/// `[31, id, len, words…]`: a layer's keyframes (`Host::layer_tracks`);
/// none: it shows its base values.
pub(crate) const TRACKS: u32 = 31;
/// `[32, id]`: layer `id` is gone.
pub(super) const LAYER_FREE: u32 = 32;
/// `[33, lo, hi]`: the monotonic clock's nanoseconds at the host's zero, so
/// the reader samples tracks on the engine's clock.
pub(crate) const CLOCK: u32 = 33;

thread_local! {
    /// The layers alive after the last finished recording: id, node, base.
    pub(crate) static ALIVE: RefCell<Vec<(u32, u64, [f32; 7])>> = const { RefCell::new(Vec::new()) };
}

/// The recorder's layer state.
#[derive(Default)]
pub(super) struct Layers {
    ids: HashMap<u64, u32>,
    info: HashMap<u32, (u64, [f32; 7])>,
    next: u32,
    /// Each layer's header and drawing as last sent.
    sent: HashMap<u32, Vec<u32>>,
    /// Layers recording, innermost last.
    open: Vec<Open>,
    /// `LAYER_SET`s to write after the row (or the outermost layer).
    pub(super) sets: Vec<u32>,
    /// The layers each kept row draws, those the recording row draws, and
    /// those drawn outside rows this frame.
    rows: HashMap<u32, Vec<u32>>,
    row: Vec<u32>,
    frame: Vec<u32>,
}

struct Open {
    id: u32,
    dash: Option<u32>,
    header: Vec<u32>,
    ops: Vec<u32>,
    matrix: Option<[f32; 6]>,
    emitted: Vec<bool>,
}

impl Layers {
    /// A new frame: nothing open, nothing drawn outside rows yet.
    pub(super) fn begin(&mut self) {
        self.open.clear();
        self.sets.clear();
        self.frame.clear();
        self.row.clear();
    }

    /// The recording row ended as kept row `id`.
    pub(super) fn row_end(&mut self, id: u32) {
        self.rows.insert(id, std::mem::take(&mut self.row));
    }

    /// Kept row `id` is gone.
    pub(super) fn row_free(&mut self, id: u32) {
        self.rows.remove(&id);
    }

    /// A dash phase is written at `at` (an index into the recording ops):
    /// the innermost open layer's first is the one its reader moves.
    pub(super) fn note_dash(&mut self, at: usize) {
        if let Some(open) = self.open.last_mut() {
            open.dash.get_or_insert(at as u32);
        }
    }
}

impl Recorder {
    /// Open layer `key` (see [`crate::paint::Backend::layer_begin`]).
    pub(super) fn layer_open(
        &mut self,
        key: u64,
        ts: Transform,
        pivot: (f32, f32),
        base: [f32; 7],
    ) {
        let l = &mut self.layers;
        let id = *l.ids.entry(key).or_insert_with(|| {
            l.next += 1;
            l.next
        });
        l.info.insert(id, (key, base));
        if self.row.is_some() {
            l.row.push(id);
        } else {
            l.frame.push(id);
        }
        self.ops.extend([LAYER_REF, id]);
        let s = self.scale;
        let m = [
            ts.sx * s,
            ts.ky * s,
            ts.kx * s,
            ts.sy * s,
            (ts.tx - self.origin.0) * s,
            (ts.ty - self.origin.1) * s,
        ];
        let header = m
            .into_iter()
            .chain([pivot.0, pivot.1])
            .chain(base)
            .map(f32::to_bits)
            .collect();
        let emitted = self.clips.iter().map(|c| c.emitted()).collect();
        let ops = std::mem::take(&mut self.ops);
        let matrix = self.matrix.take();
        self.layers.open.push(Open {
            id,
            dash: None,
            header,
            ops,
            matrix,
            emitted,
        });
    }

    /// Close the innermost open layer.
    pub(super) fn layer_close(&mut self) {
        let Some(Open {
            id,
            dash,
            header,
            ops,
            matrix,
            emitted,
        }) = self.layers.open.pop()
        else {
            return;
        };
        // Clips the drawing wrote close inside the layer: outside, pending again.
        for i in (0..self.clips.len()).rev() {
            if self.clips[i].emitted() && !emitted.get(i).copied().unwrap_or(false) {
                self.ops.push(RESTORE);
                self.clips[i].reopen();
            }
        }
        let body = std::mem::replace(&mut self.ops, ops);
        self.matrix = matrix;
        let mut full = header;
        full.push(dash.unwrap_or(u32::MAX));
        full.extend(body);
        let l = &mut self.layers;
        if l.sent.get(&id) != Some(&full) {
            l.sets.extend([LAYER_SET, id, full.len() as u32]);
            l.sets.extend(&full);
            l.sent.insert(id, full);
        }
        // Outside rows and other layers, the drawing follows at once.
        if self.row.is_none() && l.open.is_empty() {
            let sets = std::mem::take(&mut l.sets);
            self.ops.extend(sets);
        }
    }

    /// At the frame's end: free layers nothing draws, and publish the rest.
    pub(super) fn layers_finish(&mut self) {
        let l = &mut self.layers;
        let alive: HashSet<u32> = l.rows.values().flatten().chain(&l.frame).copied().collect();
        let dead: Vec<u32> = l
            .info
            .keys()
            .filter(|id| !alive.contains(id))
            .copied()
            .collect();
        for id in dead {
            if let Some((key, _)) = l.info.remove(&id) {
                l.ids.remove(&key);
            }
            l.sent.remove(&id);
            self.ops.extend([LAYER_FREE, id]);
        }
        let list = l
            .info
            .iter()
            .map(|(id, (key, base))| (*id, *key, *base))
            .collect();
        ALIVE.with(|a| *a.borrow_mut() = list);
    }
}
