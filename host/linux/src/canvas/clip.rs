//! Clips the recorder emits only when something needs them. A rounded clip in
//! HWUI is a path clip, the expensive kind; Android's own views avoid it by
//! drawing rounded content directly. So a pushed clip stays pending until a
//! drawing inside it reaches outside its rectangle or into a rounded corner;
//! a picture whose corners coincide with the clips' corners is drawn as a
//! rounded rect filled by the picture (`IMAGE_RRECT`), needing no clip at all.
//! Pending clips are materialized outermost first, so the emitted clips are
//! always a prefix of the stack and saves and restores nest.
use super::*;

/// A clip the walk pushed: the shape in its transform, and whether it has
/// been written into the stream (as `CLIP_RRECT` after a save).
pub(super) struct Clip {
    shape: Shape,
    ts: Transform,
    /// Page-space rect and radii, when the transform is a scale and translate.
    page: Option<(Rect4, [(f32, f32); 4])>,
    emitted: bool,
}

/// `ts` as (sx, sy, tx, ty) when it only scales and translates.
fn axis(ts: Transform) -> Option<(f32, f32, f32, f32)> {
    (ts.kx == 0.0 && ts.ky == 0.0 && ts.sx > 0.0 && ts.sy > 0.0)
        .then_some((ts.sx, ts.sy, ts.tx, ts.ty))
}

/// `r` in page space under `ts`.
pub(super) fn map(r: Rect4, ts: Transform) -> Option<Rect4> {
    let (sx, sy, tx, ty) = axis(ts)?;
    Some((r.0 * sx + tx, r.1 * sy + ty, r.2 * sx, r.3 * sy))
}

fn intersect(a: Rect4, b: Rect4) -> Rect4 {
    let x0 = a.0.max(b.0);
    let y0 = a.1.max(b.1);
    let x1 = (a.0 + a.2).min(b.0 + b.2);
    let y1 = (a.1 + a.3).min(b.1 + b.3);
    (x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0))
}

/// The four corners of `r` (top-left, top-right, bottom-right, bottom-left)
/// and, for each, the square a radius `(rx, ry)` cuts into.
fn corner_square(r: Rect4, k: usize, (rx, ry): (f32, f32)) -> Rect4 {
    match k {
        0 => (r.0, r.1, rx, ry),
        1 => (r.0 + r.2 - rx, r.1, rx, ry),
        2 => (r.0 + r.2 - rx, r.1 + r.3 - ry, rx, ry),
        _ => (r.0, r.1 + r.3 - ry, rx, ry),
    }
}

fn corner(r: Rect4, k: usize) -> (f32, f32) {
    match k {
        0 => (r.0, r.1),
        1 => (r.0 + r.2, r.1),
        2 => (r.0 + r.2, r.1 + r.3),
        _ => (r.0, r.1 + r.3),
    }
}

fn overlaps(a: Rect4, b: Rect4) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

const EPS: f32 = 0.01;

/// Whether drawing within `b` (page space) is unaffected by the clip.
fn unclipped(b: Rect4, (r, radii): (Rect4, [(f32, f32); 4])) -> bool {
    let inside = b.0 >= r.0 - EPS
        && b.1 >= r.1 - EPS
        && b.0 + b.2 <= r.0 + r.2 + EPS
        && b.1 + b.3 <= r.1 + r.3 + EPS;
    inside
        && (0..4).all(|k| {
            radii[k].0 <= 0.0 || radii[k].1 <= 0.0 || !overlaps(b, corner_square(r, k, radii[k]))
        })
}

impl Clip {
    pub(super) fn emitted(&self) -> bool {
        self.emitted
    }

    /// Written and closed again (inside a picture slot): pending as before.
    pub(super) fn reopen(&mut self) {
        self.emitted = false;
    }
}

impl Recorder {
    pub(super) fn clip_push(&mut self, s: &Shape, ts: Transform) {
        let page = map(s.rect, ts).map(|r| {
            let (sx, sy, _, _) = axis(ts).expect("mapped");
            (r, s.radii.map(|(x, y)| (x * sx, y * sy)))
        });
        self.clips.push(Clip {
            shape: *s,
            ts,
            page,
            emitted: false,
        });
        if page.is_none() {
            let depth = self.clips.len() - 1;
            self.materialize(depth);
        }
    }

    pub(super) fn clip_pop(&mut self) {
        if self.clips.pop().is_some_and(|c| c.emitted) {
            self.ops.push(RESTORE);
        }
    }

    /// Write every pending clip up to and including `depth`, outermost first.
    pub(super) fn materialize(&mut self, depth: usize) {
        for i in 0..=depth.min(self.clips.len().saturating_sub(1)) {
            if self.clips.is_empty() || self.clips[i].emitted {
                continue;
            }
            let (shape, ts) = (self.clips[i].shape, self.clips[i].ts);
            self.transform(ts);
            self.ops.push(CLIP_RRECT);
            self.rect_radii(&shape);
            self.clips[i].emitted = true;
        }
    }

    /// Whether a drawing within `bounds` (page space) would be clipped away
    /// entirely: outside some clip's rectangle (the list's overscan rows, out
    /// of its viewport). Such drawings are not recorded at all.
    pub(super) fn culled(&self, bounds: Option<Rect4>) -> bool {
        let Some(b) = bounds else { return false };
        self.clips
            .iter()
            .any(|c| c.page.is_some_and(|(r, _)| !overlaps(b, r)))
    }

    /// Before drawing within `bounds` (page space; `None` when unknown):
    /// write the pending clips the drawing needs.
    pub(super) fn need(&mut self, bounds: Option<Rect4>) {
        let deepest = self.clips.iter().rposition(|c| {
            !c.emitted
                && match (bounds, c.page) {
                    (Some(b), Some(page)) => !unclipped(b, page),
                    _ => true,
                }
        });
        if let Some(d) = deepest {
            self.materialize(d);
        }
    }

    /// A picture drawn as a rounded rect, its corners the clips' own corners,
    /// when every clip around it allows that; `false` to draw it as before.
    pub(super) fn image_rrect(
        &mut self,
        id: u32,
        dst: Rect4,
        own: &[Shape],
        ts: Transform,
    ) -> bool {
        let Some(d) = map(dst, ts) else { return false };
        let (sx, sy, _, _) = axis(ts).expect("mapped");
        let mut region = d;
        let mut shapes: Vec<(Rect4, [(f32, f32); 4])> = Vec::new();
        for c in own {
            let Some(r) = map(c.rect, ts) else {
                return false;
            };
            shapes.push((r, c.radii.map(|(x, y)| (x * sx, y * sy))));
        }
        for c in self.clips.iter().filter(|c| !c.emitted) {
            let Some(p) = c.page else { return false };
            shapes.push(p);
        }
        for (r, _) in &shapes {
            region = intersect(region, *r);
        }
        if region.2 <= 0.0 || region.3 <= 0.0 || self.culled(Some(region)) {
            return true;
        }
        let mut radii = [(0.0f32, 0.0f32); 4];
        for (r, rr) in &shapes {
            for k in 0..4 {
                if rr[k].0 <= 0.0
                    || rr[k].1 <= 0.0
                    || !overlaps(region, corner_square(*r, k, rr[k]))
                {
                    continue;
                }
                let (a, b) = (corner(region, k), corner(*r, k));
                if (a.0 - b.0).abs() > 0.5 || (a.1 - b.1).abs() > 0.5 {
                    return false;
                }
                radii[k] = (radii[k].0.max(rr[k].0), radii[k].1.max(rr[k].1));
            }
        }
        // Page space; inside a row, the matrix moves it to the row's origin.
        self.transform(Transform::identity());
        self.ops.extend([IMAGE_RRECT, id]);
        for v in [d.0, d.1, d.2, d.3, region.0, region.1, region.2, region.3] {
            self.f(v);
        }
        for (x, y) in radii {
            self.f(x);
            self.f(y);
        }
        true
    }
}
