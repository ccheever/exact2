//! Exact geometry painted by Android's `Canvas` (LLP 1076 §3.3): the kernel
//! lays out, cosmic-text shapes, and the paint walk runs as everywhere else,
//! but the backend records what it would draw — rounded rects, paths,
//! clips, layers, pictures and positioned glyph runs — into one flat op
//! stream that the app's `View` replays in `onDraw`. HWUI (Skia on the
//! RenderThread, with its glyph atlas) does the drawing; text is drawn with
//! `Canvas.drawGlyphs` from the same font files cosmic-text shaped with, so
//! measurement and pixels agree.
//!
//! The presenter lives on the Android main thread ([`CanvasHost`]); every
//! call comes from there.
//!
//! The stream is `u32` words (floats as bits), little-endian:
//! - `1 MATRIX a b c d e f` — device pixels; the rest of the stream until
//!   the next MATRIX is drawn under it (saved/restored by the reader around
//!   each clip scope as Canvas does).
//! - `2 RRECT color x y w h r0x r0y r1x r1y r2x r2y r3x r3y`
//! - `3 PATH color rule n (tag coords…)×n` — tags 0 move 1 line 2 cubic 3 close
//! - `4 CLIP_RRECT x y w h radii×8` (save + clip)
//! - `5 CLIP_PATH rule n (tag coords…)×n` (save + clip)
//! - `6 RESTORE`
//! - `7 LAYER alpha` (save layer)
//! - `8 IMAGE id x y w h`
//! - `9 GLYPHS font size color skew n (glyph x y)×n`
//! - `10 FONT key index weight len utf8-path(padded to 4)` — once per face
//! - `11 IMAGE_DEF id w h` — fetch its pixels with [`CanvasHost::image`]
//! - `12 IMAGE_FREE id`
//! - `13 STROKE color width cap join n (tag coords…)×n` — caps/joins as SVG (0 butt/miter, 1 round, 2 square/bevel)
//! - `14 IMAGE_RRECT id dst(x y w h) region(x y w h) radii×8` — the picture mapped to `dst`, drawn
//!   only over `region` with those corner radii (a clip-free rounded image; `clip.rs`)
//! - `26 ANIMATED id len utf8-path(padded to 4)` — after `IMAGE_DEF`: the picture is a GIF's or
//!   WebP's first frame; a reader may draw that file's animated drawable in its place
//! - `25 DASH phase n d×n` — the next STROKE is dashed: `n` (even) on/off lengths and the
//!   offset into them, in the stroke's own units (SVG `stroke-dasharray`, `stroke-dashoffset`)
//!
//! Geometry is in device pixels; colours are ARGB.
//!
//! @ref LLP 1076 §3.3 (Exact plus Android Canvas)

use crate::image::Bitmap;
use crate::paint::border::{BorderFill, PathOp};
use crate::paint::{Backend, GradientPaint, Rect4, Shape};
use crate::presenter::Presenter;
use crate::text::{Paragraph, RunPaint, TextEngine};
use exact_kernel::ViewId;
use exact_runner::DataSource;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use tiny_skia::{Pixmap, Transform};

const MATRIX: u32 = 1;
const RRECT: u32 = 2;
const PATH: u32 = 3;
const CLIP_RRECT: u32 = 4;
const CLIP_PATH: u32 = 5;
const RESTORE: u32 = 6;
const LAYER: u32 = 7;
const IMAGE: u32 = 8;
const GLYPHS: u32 = 9;
const FONT: u32 = 10;
const IMAGE_DEF: u32 = 11;
const IMAGE_FREE: u32 = 12;
const STROKE: u32 = 13;
const IMAGE_RRECT: u32 = 14;
/// The next STROKE's dash: phase, count, lengths.
const DASH: u32 = 25;
/// A picture's animated file: id, length, path.
const ANIMATED: u32 = 26;
/// A row's recording: id, the id of the row it replaces (0 for none; the top
/// bit set when the row shows in this frame, so the reader makes it before
/// drawing), width and height (device pixels), then the count of words up to
/// and including its `ROW_END`. Kept by the reader until freed; a row not in
/// view may be made later, the row it replaces drawn meanwhile.
const ROW_BEGIN: u32 = 15;
const ROW_END: u32 = 16;
/// Draw a kept row: id, x, y (device pixels).
const ROW_DRAW: u32 = 17;
/// A kept row no longer drawn: freed once the next stream arrives.
const ROW_FREE: u32 = 18;
/// A filled ring: color, outer x y w h and radii, inner x y w h and radii.
/// An even-odd path HWUI would rasterize into a mask every frame.
const RING: u32 = 19;

/// A scroller's rows follow: its view id. They are all drawn, culled or not,
/// so the drawing can be moved without painting again.
const GROUP_BEGIN: u32 = 20;
const GROUP_END: u32 = 21;
/// In a row: image node id's picture slot, drawn here (the reader's node for
/// it, recorded from the last `SLOT_SET`).
const SLOT: u32 = 23;
/// A slot's drawing: id, word count, then its ops (row coordinates).
const SLOT_SET: u32 = 24;
/// Instead of a stream: the last one moved. A count, then per scroller its
/// id and the move (device pixels) of its rows from where they were drawn.
const SHIFT: u32 = 22;

/// Room (points) left of and above a row's origin in its recording, so what
/// paints a little outside the row (a shadow) stays inside the reader's node,
/// which clips to its bounds: that is what lets the reader skip a row that
/// is out of view.
const ROW_PAD: f32 = 48.0;

/// `EXACT_SLOTS=0` draws pictures in their rows (no slots), to compare.
static SLOTS: std::sync::LazyLock<bool> =
    std::sync::LazyLock::new(|| !std::env::var("EXACT_SLOTS").is_ok_and(|v| v == "0"));

/// Paints a picture goes undrawn before the reader's copy (heap pixels and
/// the texture HWUI makes from them) is freed; drawn again, it is sent again
/// from the decoded picture, a copy rather than a decode.
const IDLE_FRAMES: u64 = 10;

#[path = "canvas/clip.rs"]
mod clip;
#[path = "canvas/picture.rs"]
mod picture;
pub use picture::Picture;
use picture::WeakPicture;

thread_local! {
    /// The last finished recording and the pictures it introduced.
    static FINISHED: RefCell<Option<Vec<u32>>> = const { RefCell::new(None) };
    /// The last finished recording's scrollers and the offsets drawn at.
    static GROUPS: RefCell<Vec<(u32, (f32, f32))>> = const { RefCell::new(Vec::new()) };
    /// Pictures the reader has not fetched yet, by id.
    static PENDING: RefCell<std::collections::BTreeMap<u32, Picture>> =
        const { RefCell::new(std::collections::BTreeMap::new()) };
}

/// The recording backend.
pub struct Recorder {
    scale: f32,
    ops: Vec<u32>,
    /// The matrix last written into `ops` (points → device pixels).
    matrix: Option<[f32; 6]>,
    /// Faces announced to the reader, by (file, index, weight).
    fonts: HashMap<(Arc<str>, u32, u16), u32>,
    /// Files standing in for faces loaded from bytes, by blob id.
    face_files: HashMap<u64, Option<Arc<str>>>,
    /// Pictures announced to the reader, by allocation, with a weak handle
    /// so a dropped picture is freed on the reader too, and the frame each
    /// was last drawn in.
    images: HashMap<usize, (u32, WeakPicture, u64)>,
    /// Frames begun: when a picture was last drawn.
    frames: u64,
    /// The scroller whose rows are being drawn, if any.
    group: Option<u32>,
    /// Frames a picture goes undrawn before the reader's copy is freed
    /// (`EXACT_IDLE_FRAMES`, else [`IDLE_FRAMES`]).
    idle: u64,
    next_image: u32,
    /// The clips pushed and not popped, pending until a drawing needs them.
    clips: Vec<clip::Clip>,
    /// The recording row's origin (viewport points): what its ops are
    /// relative to. Zero outside a row.
    origin: (f32, f32),
    /// While a row records: the frame's clips and matrix, where the row's
    /// header is, its id and the id it was.
    row: Option<RowRecording>,
    /// The last row recorded in this stream: its id and where its header is.
    recorded: Option<(u32, usize)>,
    /// Each kept row's recording, so one recorded again the same is kept,
    /// and the pictures it draws (by allocation), drawn whenever it is.
    kept: HashMap<u32, (Vec<u32>, Vec<usize>)>,
    /// While a picture slot records: the row's ops so far, its matrix, and
    /// which clips were written when it began.
    slot: Option<SlotRecording>,
    /// Each slot's drawing as last sent, and those to send after this row.
    slots: HashMap<u32, Vec<u32>>,
    slot_sets: Vec<u32>,
}

/// A picture slot recording: its id, the row's ops so far, the row's matrix,
/// and which clips were written when it began.
type SlotRecording = (u32, Vec<u32>, Option<[f32; 6]>, Vec<bool>);

struct RowRecording {
    clips: Vec<clip::Clip>,
    matrix: Option<[f32; 6]>,
    at: usize,
    id: u32,
    previous: Option<u32>,
    images: Vec<usize>,
}

impl Default for Recorder {
    fn default() -> Self {
        Self::new()
    }
}

impl Recorder {
    /// An empty recorder.
    pub fn new() -> Recorder {
        Recorder {
            scale: 1.0,
            ops: Vec::new(),
            matrix: None,
            fonts: HashMap::new(),
            face_files: HashMap::new(),
            images: HashMap::new(),
            next_image: 1,
            frames: 0,
            group: None,
            idle: std::env::var("EXACT_IDLE_FRAMES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(IDLE_FRAMES),
            clips: Vec::new(),
            origin: (0.0, 0.0),
            row: None,
            recorded: None,
            kept: HashMap::new(),
            slot: None,
            slots: HashMap::new(),
            slot_sets: Vec::new(),
        }
    }

    fn f(&mut self, v: f32) {
        self.ops.push(v.to_bits());
    }

    fn color(c: [u8; 4]) -> u32 {
        u32::from_be_bytes([c[3], c[0], c[1], c[2]])
    }

    /// Write the transform for the next op if it differs from the last.
    fn transform(&mut self, ts: Transform) {
        let s = self.scale;
        let m = [
            ts.sx * s,
            ts.ky * s,
            ts.kx * s,
            ts.sy * s,
            (ts.tx - self.origin.0) * s,
            (ts.ty - self.origin.1) * s,
        ];
        if self.matrix != Some(m) {
            self.matrix = Some(m);
            self.ops.push(MATRIX);
            for v in m {
                self.f(v);
            }
        }
    }

    fn rect_radii(&mut self, s: &Shape) {
        for v in [s.rect.0, s.rect.1, s.rect.2, s.rect.3] {
            self.f(v);
        }
        for (rx, ry) in s.radii {
            self.f(rx);
            self.f(ry);
        }
    }

    fn path(&mut self, ops: &[PathOp]) {
        self.ops.push(ops.len() as u32);
        for op in ops {
            match *op {
                PathOp::Move(x, y) => {
                    self.ops.push(0);
                    self.f(x);
                    self.f(y);
                }
                PathOp::Line(x, y) => {
                    self.ops.push(1);
                    self.f(x);
                    self.f(y);
                }
                PathOp::Cubic(a, b, c, d, e, g) => {
                    self.ops.push(2);
                    for v in [a, b, c, d, e, g] {
                        self.f(v);
                    }
                }
                PathOp::Close => self.ops.push(3),
            }
        }
    }

    /// A face loaded from bytes (a declared `font`) has no file for Android's
    /// `Font`: its bytes are written once to `$HOME/.exact-fonts/`, named by
    /// their hash, and that file stands in.
    fn face_file(&mut self, font: &cosmic_text::PenikoFont) -> Option<(Arc<str>, u32)> {
        let blob = font.data.id();
        if let Some(path) = self.face_files.get(&blob) {
            return path.clone().map(|p| (p, font.index));
        }
        let bytes = font.data.data();
        let hash = bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
            (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
        });
        let dir = std::path::Path::new(&std::env::var("HOME").unwrap_or_else(|_| "/tmp".into()))
            .join(".exact-fonts");
        let path = dir.join(format!("{hash:016x}.ttf"));
        let ok = path.exists()
            || (std::fs::create_dir_all(&dir).is_ok() && std::fs::write(&path, bytes).is_ok());
        let path: Option<Arc<str>> = ok.then(|| Arc::from(path.to_string_lossy().as_ref()));
        self.face_files.insert(blob, path.clone());
        path.map(|p| (p, font.index))
    }

    fn font(&mut self, file: &(Arc<str>, u32), weight: u16) -> u32 {
        let key = (file.0.clone(), file.1, weight);
        if let Some(k) = self.fonts.get(&key) {
            return *k;
        }
        let k = self.fonts.len() as u32 + 1;
        self.fonts.insert(key, k);
        self.ops.extend([FONT, k, file.1, u32::from(weight)]);
        self.string(&file.0);
        k
    }

    /// Length, then UTF-8 bytes padded to whole words.
    fn string(&mut self, s: &str) {
        let bytes = s.as_bytes();
        self.ops.push(bytes.len() as u32);
        for chunk in bytes.chunks(4) {
            let mut w = [0u8; 4];
            w[..chunk.len()].copy_from_slice(chunk);
            self.ops.push(u32::from_le_bytes(w));
        }
    }

    /// A picture mapped onto `dst` under `clips`.
    fn picture(&mut self, picture: &Picture, dst: Rect4, clips: &[Shape], ts: Transform) {
        let id = self.image_id(picture);
        if self.image_rrect(id, dst, clips, ts) {
            return;
        }
        self.need(None);
        self.transform(ts);
        for c in clips {
            self.ops.push(CLIP_RRECT);
            self.rect_radii(c);
        }
        self.ops.extend([IMAGE, id]);
        for v in [dst.0, dst.1, dst.2, dst.3] {
            self.f(v);
        }
        for _ in clips {
            self.ops.push(RESTORE);
        }
    }

    fn image_id(&mut self, image: &Picture) -> u32 {
        let key = image.key();
        let now = self.frames;
        if let Some(row) = &mut self.row {
            row.images.push(key);
        }
        if let Some((id, weak, used)) = self.images.get_mut(&key) {
            if weak.is(image) {
                *used = now;
                return *id;
            }
        }
        let id = self.next_image;
        self.next_image += 1;
        self.images.insert(key, (id, image.downgrade(), now));
        self.ops
            .extend([IMAGE_DEF, id, image.width(), image.height()]);
        if let Picture::Bitmap(b) = image {
            if let Some(file) = b.animation_file() {
                let path = file.to_string_lossy();
                self.ops.extend([ANIMATED, id]);
                self.string(&path);
            }
        }
        PENDING.with(|p| p.borrow_mut().insert(id, image.clone()));
        id
    }
}

impl Backend for Recorder {
    fn name(&self) -> &'static str {
        "canvas"
    }

    fn begin(&mut self, _width: f32, _height: f32, scale: f32) {
        self.scale = scale;
        self.ops.clear();
        self.matrix = None;
        self.clips.clear();
        self.origin = (0.0, 0.0);
        self.row = None;
        self.group = None;
        self.slot = None;
        self.slot_sets.clear();
        self.recorded = None;
        GROUPS.with(|g| g.borrow_mut().clear());
        self.frames += 1;
        // Pictures the presenter dropped since the last frame, and those not
        // drawn for a while: the reader's copy (heap and texture) goes; the
        // decoded picture, if the presenter still caches it, is sent again
        // when it draws again. Kept rows hold their own reference.
        let frames = self.frames;
        let dead: Vec<(usize, u32)> = self
            .images
            .iter()
            .filter(|(_, (_, weak, used))| !weak.alive() || frames - used > self.idle)
            .map(|(k, (id, _, _))| (*k, *id))
            .collect();
        for (k, id) in dead {
            self.images.remove(&k);
            self.ops.extend([IMAGE_FREE, id]);
            PENDING.with(|p| p.borrow_mut().remove(&id));
        }
    }

    fn fill(&mut self, s: &Shape, color: [u8; 4], ts: Transform) {
        if s.rect.2 <= 0.0 || s.rect.3 <= 0.0 || color[3] == 0 {
            return;
        }
        let bounds = clip::map(s.rect, ts);
        if self.culled(bounds) {
            return;
        }
        self.need(bounds);
        self.transform(ts);
        self.ops.extend([RRECT, Self::color(color)]);
        self.rect_radii(s);
    }

    fn fill_gradient(&mut self, s: &Shape, g: &GradientPaint, ts: Transform) {
        // Not in this benchmark's rows: the first stop stands in (a gap).
        if let Some(&(_, c)) = g.stops.first() {
            self.fill(s, crate::paint::rgba(c), ts);
        }
    }

    fn fill_border(&mut self, part: &BorderFill, ts: Transform) {
        let bounds = path_bounds(&part.region).and_then(|b| clip::map(b, ts));
        if self.culled(bounds) {
            return;
        }
        self.need(bounds);
        self.transform(ts);
        if let (Some(((o, or), (i, ir))), None) = (&part.ring, &part.clip) {
            self.ops.extend([RING, Self::color(part.color)]);
            for v in [o.0, o.1, o.2, o.3] {
                self.f(v);
            }
            for (x, y) in or {
                self.f(*x);
                self.f(*y);
            }
            for v in [i.0, i.1, i.2, i.3] {
                self.f(v);
            }
            for (x, y) in ir {
                self.f(*x);
                self.f(*y);
            }
            return;
        }
        if let Some(clip) = &part.clip {
            self.ops.extend([CLIP_PATH, 0]);
            self.path(clip);
        }
        self.ops.extend([PATH, Self::color(part.color), 1]);
        self.path(&part.region);
        if part.clip.is_some() {
            self.ops.push(RESTORE);
        }
    }

    fn image(
        &mut self,
        image: &Arc<Bitmap>,
        dst: Rect4,
        clips: &[Shape],
        ts: Transform,
        _tint: Option<[u8; 4]>,
    ) {
        if dst.2 <= 0.0 || dst.3 <= 0.0 {
            return;
        }
        self.picture(&Picture::Bitmap(image.clone()), dst, clips, ts);
    }

    fn canvas(&mut self, pixels: &Arc<Pixmap>, dst: Rect4, clips: &[Shape], ts: Transform) {
        // A canvas is drawn by the presenter into pixels (Canvas 2D in Rust);
        // the reader shows them as a picture, sent again when they change.
        if dst.2 <= 0.0 || dst.3 <= 0.0 || pixels.width() == 0 || pixels.height() == 0 {
            return;
        }
        self.picture(&Picture::Canvas(pixels.clone()), dst, clips, ts);
    }

    fn text(
        &mut self,
        text: &mut TextEngine,
        paragraph: &Paragraph,
        palette: &[RunPaint],
        origin: (f32, f32),
        ts: Transform,
    ) {
        // Glyphs can overhang their box a little (italics, accents).
        let bounds = (
            origin.0 - 2.0,
            origin.1 - 2.0,
            paragraph.width + 4.0,
            paragraph.height + 4.0,
        );
        let bounds = clip::map(bounds, ts);
        if self.culled(bounds) {
            return;
        }
        self.need(bounds);
        self.transform(ts.pre_translate(origin.0, origin.1));
        for run in text.glyph_runs(paragraph, palette) {
            if run.paint.color[3] == 0 {
                continue;
            }
            let Some(file) = run.file.clone().or_else(|| self.face_file(&run.font)) else {
                continue;
            };
            let font = self.font(&file, run.weight);
            self.ops.extend([GLYPHS, font]);
            self.f(run.size);
            self.ops.push(Self::color(run.paint.color));
            self.f(if run.synthetic_italic { -0.25 } else { 0.0 });
            self.ops.push(run.glyphs.len() as u32);
            for (id, x, y) in &run.glyphs {
                self.ops.push(*id);
                self.f(*x);
                self.f(*y);
            }
        }
    }

    fn svg_path(&mut self, s: &crate::paint::SvgPaint<'_>, ts: Transform) {
        // Solid inks only (symbols, plain SVG shapes); a gradient or pattern
        // draws nothing here (a gap of this backend).
        let ops: Vec<PathOp> = s
            .path
            .0
            .iter()
            .map(|seg| match *seg {
                exact_kernel::svg::Seg::Move(x, y) => PathOp::Move(x, y),
                exact_kernel::svg::Seg::Line(x, y) => PathOp::Line(x, y),
                exact_kernel::svg::Seg::Cubic(a, b, c, d, x, y) => PathOp::Cubic(a, b, c, d, x, y),
                exact_kernel::svg::Seg::Close => PathOp::Close,
            })
            .collect();
        let grow = if s.stroke.is_some() { s.width } else { 0.0 };
        self.need(path_bounds(&ops).and_then(|(x, y, w, h)| {
            clip::map((x - grow, y - grow, w + 2.0 * grow, h + 2.0 * grow), ts)
        }));
        self.transform(ts);
        if let Some(crate::paint::Ink::Solid(c)) = &s.fill {
            self.ops
                .extend([PATH, Self::color(*c), u32::from(s.even_odd)]);
            self.path(&ops);
        }
        if let Some(crate::paint::Ink::Solid(c)) = &s.stroke {
            if !s.dash.is_empty() {
                self.ops.push(DASH);
                self.f(s.phase);
                self.ops.push(s.dash.len() as u32);
                for d in &s.dash {
                    self.f(*d);
                }
            }
            self.ops.extend([STROKE, Self::color(*c)]);
            self.f(s.width);
            self.ops.extend([u32::from(s.cap), u32::from(s.join)]);
            self.path(&ops);
        }
    }

    fn push_clip(&mut self, s: &Shape, ts: Transform) {
        self.clip_push(s, ts);
    }

    fn pop_clip(&mut self) {
        self.clip_pop();
    }

    fn push_opacity(&mut self, alpha: f32) {
        // A layer's bounds are unknown here: every pending clip applies to it.
        self.need(None);
        self.ops.push(LAYER);
        self.f(alpha);
    }

    fn pop_opacity(&mut self) {
        self.ops.push(RESTORE);
    }

    fn pointer(&mut self, _x: f32, _y: f32) {}

    fn rows(&self) -> bool {
        true
    }

    fn row_begin(&mut self, id: u32, origin: (f32, f32), previous: Option<u32>) {
        self.row = Some(RowRecording {
            clips: std::mem::take(&mut self.clips),
            matrix: self.matrix.take(),
            at: self.ops.len(),
            id,
            previous,
            images: Vec::new(),
        });
        self.origin = (origin.0 - ROW_PAD, origin.1 - ROW_PAD);
        self.ops
            .extend([ROW_BEGIN, id, previous.unwrap_or(0), 0, 0, 0]);
    }

    fn row_end(&mut self, bounds: Rect4) -> u32 {
        let Some(RowRecording {
            clips,
            matrix,
            at,
            id,
            previous,
            images,
        }) = self.row.take()
        else {
            return 0;
        };
        // The reader's node reaches from the origin to the far edge of what
        // the row covers.
        let s = self.scale;
        self.ops[at + 3] = ((bounds.0 + bounds.2 + ROW_PAD).max(1.0) * s).to_bits();
        self.ops[at + 4] = ((bounds.1 + bounds.3 + ROW_PAD).max(1.0) * s).to_bits();
        self.ops.push(ROW_END);
        self.ops[at + 5] = (self.ops.len() - at - 6) as u32;
        self.clips = clips;
        self.matrix = matrix;
        self.origin = (0.0, 0.0);
        // Size and body: the same as the row's last recording, the reader's
        // node for it stands.
        let body = &self.ops[at + 3..];
        let sets = std::mem::take(&mut self.slot_sets);
        if let Some(old) = previous.filter(|p| self.kept.get(p).is_some_and(|k| k.0[..] == *body)) {
            self.ops.truncate(at);
            self.ops.extend(sets);
            self.kept.get_mut(&old).expect("kept").1 = images;
            return old;
        }
        let body = body.to_vec();
        self.kept.insert(id, (body, images));
        self.recorded = Some((id, at));
        self.ops.extend(sets);
        id
    }

    fn slot_begin(&mut self, id: ViewId) {
        if self.row.is_none() || self.slot.is_some() || !*SLOTS {
            return;
        }
        self.ops.extend([SLOT, id]);
        let row = std::mem::take(&mut self.ops);
        let emitted = self.clips.iter().map(|c| c.emitted()).collect();
        self.slot = Some((id, row, self.matrix.take(), emitted));
    }

    fn slot_end(&mut self) {
        let Some((id, row, matrix, emitted)) = self.slot.take() else {
            return;
        };
        // Clips the picture wrote close inside its slot: outside, pending again.
        for i in (0..self.clips.len()).rev() {
            if self.clips[i].emitted() && !emitted.get(i).copied().unwrap_or(false) {
                self.ops.push(RESTORE);
                self.clips[i].reopen();
            }
        }
        let drawing = std::mem::replace(&mut self.ops, row);
        self.matrix = matrix;
        if self.slots.get(&id) != Some(&drawing) {
            self.slot_sets.extend([SLOT_SET, id, drawing.len() as u32]);
            self.slot_sets.extend(&drawing);
            self.slots.insert(id, drawing);
        }
    }

    fn row_culled(&self, bounds: Rect4) -> bool {
        self.group.is_none() && self.culled(Some(bounds))
    }

    fn group_begin(&mut self, id: ViewId, scroll: (f32, f32)) {
        // The clips around the rows stay where they are when the rows move:
        // written before the group, never inside it.
        if !self.clips.is_empty() {
            self.materialize(self.clips.len() - 1);
        }
        self.group = Some(id);
        self.ops.extend([GROUP_BEGIN, id]);
        GROUPS.with(|g| g.borrow_mut().push((id, scroll)));
    }

    fn group_end(&mut self) {
        self.group = None;
        self.ops.push(GROUP_END);
    }

    fn row_draw(&mut self, id: u32, origin: (f32, f32), bounds: Rect4) {
        // Just recorded and in view: the reader makes it before this frame.
        if let Some((_, at)) = self.recorded.take().filter(|(r, _)| *r == id) {
            if !self.culled(Some(bounds)) {
                self.ops[at + 2] |= 1 << 31;
            }
        }
        // Its pictures are drawn too: the reader keeps them.
        if let Some((_, images)) = self.kept.get(&id) {
            for key in images {
                if let Some(image) = self.images.get_mut(key) {
                    image.2 = self.frames;
                }
            }
        }
        self.need(Some(bounds));
        let s = self.scale;
        self.ops.extend([ROW_DRAW, id]);
        self.f((origin.0 - ROW_PAD) * s);
        self.f((origin.1 - ROW_PAD) * s);
    }

    fn row_free(&mut self, id: u32) {
        self.kept.remove(&id);
        self.ops.extend([ROW_FREE, id]);
    }

    fn finish(&mut self) -> Result<Pixmap, String> {
        let ops = std::mem::take(&mut self.ops);
        FINISHED.with(|f| *f.borrow_mut() = Some(ops));
        Pixmap::new(1, 1).ok_or_else(|| "placeholder".to_string())
    }
}

/// An atrace section for the rest of a scope.
struct Section;
impl Section {
    fn begin(name: &'static core::ffi::CStr) -> Section {
        crate::android::section_begin(name);
        Section
    }
}
impl Drop for Section {
    fn drop(&mut self) {
        crate::android::section_end();
    }
}

/// The bounds of a path's points (control points included).
fn path_bounds(ops: &[PathOp]) -> Option<Rect4> {
    let mut b: Option<(f32, f32, f32, f32)> = None;
    let mut add = |x: f32, y: f32| {
        b = Some(match b {
            None => (x, y, x, y),
            Some((a, c, d, e)) => (a.min(x), c.min(y), d.max(x), e.max(y)),
        })
    };
    for op in ops {
        match *op {
            PathOp::Move(x, y) | PathOp::Line(x, y) => add(x, y),
            PathOp::Cubic(a, c, d, e, f, g) => {
                add(a, c);
                add(d, e);
                add(f, g);
            }
            PathOp::Close => {}
        }
    }
    b.map(|(x0, y0, x1, y1)| (x0, y0, x1 - x0, y1 - y0))
}

/// The presenter on the Android main thread, painting through [`Recorder`].
pub struct CanvasHost<D: DataSource> {
    p: Presenter<D>,
    started: std::time::Instant,
    scale: f32,
    viewport: (f32, f32),
    /// The last paint: what it showed besides scroll offsets, the offsets,
    /// and its scrollers with the offsets their rows were drawn at.
    painted: Option<Painted>,
    /// Frames since the last paint that moved it instead, and when the
    /// last of them was (ms).
    moved: u32,
    moved_at: f64,
    /// A touch began or ended: paint the next frame.
    force: bool,
    /// A scroll came since the last collection pass: the frame drawing it
    /// leaves the pass for [`CanvasHost::refine`], after the frame. Only once
    /// the reader calls `refine` (`prefetching`).
    scrolled: bool,
    prefetching: bool,
    /// The kernel epoch a collection pass left, when nothing else changed
    /// since the paint: rows mounted or retired out of view, which the next
    /// paint (at most [`MOVES`] frames on) shows; until then frames move.
    quiet: Option<u64>,
    /// Frames that may move before one paints (`EXACT_MOVES`, else
    /// [`MOVES`]); 1000 or more also leaves the last move standing, to check
    /// a moved frame against a painted one.
    moves: u32,
}

struct Painted {
    still: crate::presenter::still::Still,
    scroll: std::collections::BTreeMap<ViewId, (f32, f32)>,
    groups: Vec<(u32, (f32, f32))>,
}

/// At most this many frames move the last paint before one paints again:
/// what it leaves stale (boxes, hits, which pictures show) stays this fresh.
const MOVES: u32 = 30;
/// The same, once rows have mounted out of view since the last paint.
const MOVES_MOUNTED: u32 = 6;

impl<D: DataSource + Default> CanvasHost<D> {
    /// Boot `D`'s app over a view of `size` pixels at `scale` pixels per
    /// point. The environment is read as on Linux (`EXACT_ASSETS`, …);
    /// `EXACT_PAINTER` is set to `canvas` here.
    pub fn boot(
        plan: &'static [u8],
        compat: &'static str,
        size: (u32, u32),
        scale: f32,
    ) -> Result<CanvasHost<D>, String> {
        let started = std::time::Instant::now();
        std::env::set_var("EXACT_PAINTER", "canvas");
        // Four viewports of decoded pictures, not Apple's eight: the reader
        // holds its own copy of each one in use, so a larger cache costs twice.
        if std::env::var_os("EXACT_IMAGE_VIEWPORTS").is_none() {
            std::env::set_var("EXACT_IMAGE_VIEWPORTS", "4");
        }
        std::env::set_var("EXACT_SCALE", scale.to_string());
        let viewport = (size.0 as f32 / scale, size.1 as f32 / scale);
        let mut config = crate::app::Config::from_env(plan, compat);
        config.scale = scale;
        let (p, error) = crate::app::boot_presenter::<D>(&mut config, viewport)?;
        if let Some(e) = error {
            eprintln!("exact: {e}");
        }
        eprintln!(
            "exact: canvas {}x{} px, scale {scale}, boot {:.1} ms",
            size.0,
            size.1,
            started.elapsed().as_secs_f64() * 1000.0
        );
        Ok(CanvasHost {
            p,
            started,
            scale,
            viewport,
            painted: None,
            moved: 0,
            moved_at: 0.0,
            force: false,
            scrolled: false,
            prefetching: false,
            quiet: None,
            moves: std::env::var("EXACT_MOVES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(MOVES),
        })
    }

    fn now(&self) -> f64 {
        self.started.elapsed().as_secs_f64() * 1000.0
    }

    /// One turn: the presenter's work, then a recording when anything
    /// changed. `Some` is the new op stream (valid until the next call).
    pub fn frame(&mut self) -> Option<Vec<u32>> {
        let p = &mut self.p;
        let now = self.started.elapsed().as_secs_f64() * 1000.0;
        let _frame = Section::begin(c"exact frame");
        p.hold_collections(self.scrolled);
        let frame = self.frame_held(now);
        self.p.hold_collections(false);
        frame
    }

    fn frame_held(&mut self, now: f64) -> Option<Vec<u32>> {
        // A frame a scroll step asks for: replies and pictures arrive through
        // [`CanvasHost::poll`] when their fds wake, so this one skips them.
        let quick = self.scrolled;
        let p = &mut self.p;
        if !quick {
            if let Some(e) = p.pump(now) {
                eprintln!("exact: {e}");
            }
            p.poll_update();
            p.run_commands(D::default);
        }
        if p.host().wants_frames() {
            if let Some(e) = p.animation_frame(now) {
                eprintln!("exact: {e}");
            }
        } else if p.host().timer_due_ms().is_some_and(|due| due <= now) {
            if let Some(e) = p.advance(now) {
                eprintln!("exact: {e}");
            }
        }
        if p.needs_animation_frame() {
            p.tick(now);
        }
        if !quick {
            p.poll_images();
        }
        if let Some(shift) = self.shift(now) {
            return Some(shift);
        }
        if !self.p.dirty() && !self.owed_at(now) {
            return None;
        }
        let p = &mut self.p;
        let frame = crate::android::trace(c"exact paint", || p.display_frame())?;
        p.display_complete(&frame);
        if p.module_pending() {
            p.first_pixel();
        }
        self.painted = p.still().map(|still| Painted {
            still,
            scroll: p.scroll_offsets().clone(),
            groups: GROUPS.with(|g| g.borrow().clone()),
        });
        self.moved = 0;
        self.force = false;
        self.quiet = None;
        FINISHED.with(|f| f.borrow_mut().take())
    }

    /// Drain the wakes (executor replies, decoded images) without painting;
    /// whether a frame is now wanted.
    pub fn poll(&mut self) -> bool {
        let now = self.now();
        if let Some(e) = self.p.pump(now) {
            eprintln!("exact: {e}");
        }
        self.p.run_commands(D::default);
        self.p.poll_images();
        self.animating()
    }

    /// Whether another frame is wanted at the next vsync.
    pub fn animating(&self) -> bool {
        // A moved paint owes a paint: the frame after scrolling stops.
        self.p.dirty() || self.p.needs_animation_frame() || self.p.host().wants_frames()
    }

    /// When only scrollers whose rows the last paint drew moved since it,
    /// and nothing else it showed changed: the move, instead of a paint.
    /// The collection pass a scroll left for after its frame (rows mount and
    /// retire there, not inside the next frame's scroll); from now on scrolls
    /// leave it. Whether a frame is wanted after it.
    pub fn refine(&mut self) -> bool {
        let _s = Section::begin(c"exact refine");
        self.scrolled = false;
        self.prefetching = true;
        // Pictures coming into view while frames move: requested now, where
        // their rows are, not at the next paint.
        if self.moved > 0 {
            if let Some(painted) = &self.painted {
                let now = self.p.scroll_offsets();
                let moved: std::collections::BTreeMap<ViewId, (f32, f32)> = painted
                    .groups
                    .iter()
                    .map(|(id, at)| {
                        let to = now.get(id).copied().unwrap_or((0.0, 0.0));
                        (*id, (at.0 - to.0, at.1 - to.1))
                    })
                    .collect();
                if let Some(e) = self.p.sync_images_moved(&moved) {
                    eprintln!("exact: {e}");
                }
            }
        }
        let before = self.p.still();
        let wanted = self.p.refine_deferred(true);
        // Only the pass changed the kernel (rows out of view): no paint now.
        let after = self.p.still();
        let painted = self.painted.as_ref().map(|p| &p.still);
        match (before, after, painted) {
            (Some(b), Some(a), Some(p))
                if (b == *p || self.quiet.is_some_and(|e| p.same_at(&b, e))) && b != a =>
            {
                let epoch = self.p.host().kernel().epoch();
                if p.same_at(&a, epoch) {
                    self.quiet = Some(epoch);
                    return false;
                }
                wanted
            }
            _ => wanted,
        }
    }

    /// Whether a moved paint owes a paint: once moves pause (a frame
    /// without one), the paint brings boxes, hits and pictures up to date.
    pub fn owed(&self) -> bool {
        self.moved > 0 && self.moves < 1000
    }

    fn owed_at(&self, now: f64) -> bool {
        self.owed() && now - self.moved_at >= 12.0
    }

    fn shift(&mut self, at: f64) -> Option<Vec<u32>> {
        let p = &self.p;
        // Rows mounted out of view since the paint (a quiet epoch) show at the
        // next one, so it comes sooner: they may scroll in within a quarter
        // second.
        let limit = if self.quiet.is_some() {
            self.moves.min(MOVES_MOUNTED)
        } else {
            self.moves
        };
        if !p.dirty() || self.force || self.moved >= limit {
            return None;
        }
        let painted = self.painted.as_ref()?;
        let now = p.scroll_offsets();
        let grouped = |id: &ViewId| painted.groups.iter().any(|(g, _)| g == id);
        let others = |m: &std::collections::BTreeMap<ViewId, (f32, f32)>| {
            m.iter()
                .filter(|(id, _)| !grouped(id))
                .map(|(id, o)| (*id, o.0.to_bits(), o.1.to_bits()))
                .collect::<Vec<_>>()
        };
        let still = p.still()?;
        let same = still == painted.still
            || self
                .quiet
                .is_some_and(|epoch| painted.still.same_at(&still, epoch));
        if others(now) != others(&painted.scroll) || !same {
            return None;
        }
        let s = self.scale;
        let mut ops = vec![SHIFT, painted.groups.len() as u32];
        for (id, at) in &painted.groups {
            let to = now.get(id).copied().unwrap_or((0.0, 0.0));
            ops.extend([
                *id,
                ((at.0 - to.0) * s).to_bits(),
                ((at.1 - to.1) * s).to_bits(),
            ]);
        }
        self.p.moved_without_paint();
        self.moved += 1;
        self.moved_at = at;
        Some(ops)
    }

    /// Milliseconds until the next timer, if any.
    pub fn next_due(&self) -> Option<f64> {
        self.p
            .host()
            .timer_due_ms()
            .map(|due| (due - self.now()).max(0.0))
    }

    /// The descriptors that wake the presenter (executor replies, decoded images).
    pub fn fds(&self) -> [i32; 2] {
        [self.p.executor_fd(), self.p.image_fd()]
    }

    /// Scroll what is under the viewport's center by `dy` pixels.
    pub fn scroll(&mut self, dy: f32) {
        let (x, y) = (self.viewport.0 / 2.0, self.viewport.1 / 2.0);
        let _s = Section::begin(c"exact scroll");
        self.scrolled = self.prefetching;
        self.p.hold_collections(self.prefetching);
        self.p.wheel_at(x, y, 0.0, dy / self.scale);
        self.p.hold_collections(false);
    }

    /// A touch (0 down, 1 up, 2 move, 3 cancel) at pixels.
    pub fn touch(&mut self, action: i32, x: f32, y: f32) {
        self.force |= action != 2;
        let now = self.now();
        let (x, y) = (x / self.scale, y / self.scale);
        let r = match action {
            0 => self.p.pointer_down(x, y, now).map(|_| ()),
            1 => self.p.pointer_up(x, y, now).map(|_| ()),
            2 => self.p.pointer_move(x, y, now).map(|_| ()),
            _ => self.p.pointer_cancel(now),
        };
        if let Err(e) = r {
            eprintln!("exact: {e}");
        }
    }

    /// A picture announced by `IMAGE_DEF`, once (premultiplied RGBA rows).
    pub fn image(id: u32) -> Option<Picture> {
        PENDING.with(|p| p.borrow_mut().remove(&id))
    }
}
