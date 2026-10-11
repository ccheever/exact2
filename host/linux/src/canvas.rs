//! Exact geometry painted by Android's `Canvas` (LLP 1076 §3.3): the kernel
//! lays out, Parley shapes, and the paint walk runs as everywhere else,
//! but the backend records what it would draw — rounded rects, paths,
//! clips, layers, pictures and positioned glyph runs — into one flat op
//! stream that the app's `View` replays in `onDraw`. HWUI (Skia on the
//! RenderThread, with its glyph atlas) does the drawing; text is drawn with
//! `Canvas.drawGlyphs` from the same font files Parley shaped with, so
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
//! - `41 FONT_AXES key index weight n (tag value)×n len utf8-path(padded to 4)` — `FONT` for a
//!   face drawn at variation settings fontique chose (`tag` an OpenType axis tag as a big-endian
//!   `u32`, `value` in the axis's units: `ital` 1 for Roboto's italic, `wght`, `slnt`, `wdth`).
//!   The reader sets them on its `Font`; `wght`, when absent, is `weight` as for `FONT`. In
//!   place of `FONT` for such a face, so a reader that predates it stops rather than drawing
//!   the default instance at the instance's advances. GLYPHS' `skew` stays for an oblique
//!   fontique synthesized (a face with neither axis)
//! - `11 IMAGE_DEF id w h` — fetch its pixels with [`CanvasHost::image`]
//! - `12 IMAGE_FREE id`
//! - `13 STROKE color width cap join n (tag coords…)×n` — caps/joins as SVG (0 butt/miter, 1 round, 2 square/bevel)
//! - `14 IMAGE_RRECT id dst(x y w h) region(x y w h) radii×8` — the picture mapped to `dst`, drawn
//!   only over `region` with those corner radii (a clip-free rounded image; `clip.rs`)
//! - `26 ANIMATED id len utf8-path(padded to 4)` — after `IMAGE_DEF`: the picture is a GIF's or
//!   WebP's first frame; a reader may draw that file's animated drawable in its place
//! - `42 BACKDROP n (kind amount)×n x y w h radii×8` — filter what this recording drew so far
//!   inside that rounded rect, in order: kind 0 blur (local units), 1 saturation (multiplier)
//!   (replaces the radius-only op 27; an older reader must refuse the new op)
//! - `28 NATIVE view kind x y w h radii×8 len utf8-json` — a platform element (`paint/native.rs`;
//!   kind 0 video, 1 web view, 2 module) shown in that rounded rect: the reader draws the
//!   platform view's own drawing there
//! - `40 SHADOW color sigma x y w h radii×8 bx by bw bh bradii×8` — an outer
//!   `box-shadow`: the rounded rect blurred by `sigma` (local units), drawn only
//!   outside the border box `b…` (`canvas/shadow.rs`)
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
/// `FONT` with variation settings: key, index, weight, count and (tag
/// value) pairs, then the path.
const FONT_AXES: u32 = 41;
const IMAGE_DEF: u32 = 11;
const IMAGE_FREE: u32 = 12;
const STROKE: u32 = 13;
const IMAGE_RRECT: u32 = 14;
/// The next STROKE's dash: phase, count, lengths.
const DASH: u32 = 25;
/// A picture's animated file: id, length, path.
const ANIMATED: u32 = 26;
/// Ordered backdrop functions (kind, amount), then the rounded rect.
const BACKDROP: u32 = 42;
/// A platform element: view, kind, rounded rect, props.
const NATIVE: u32 = 28;
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
/// With `EXACT_MOVE_TRACKS=1`, `CLOCK` and `TRACKS` ops may follow: layers of
/// the last stream whose plays changed since.
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
#[path = "canvas/jni.rs"]
pub mod jni;
#[path = "canvas/keys.rs"]
mod keys;
#[path = "canvas/layer.rs"]
mod layer;
pub use jni_sys;
#[path = "canvas/picture.rs"]
mod picture;
#[path = "canvas/refine.rs"]
mod refine;
#[path = "canvas/shadow.rs"]
mod shadow;
#[path = "canvas/slot.rs"]
mod slot;
#[path = "canvas/stream.rs"]
mod stream;
pub use picture::Picture;
use picture::WeakPicture;
pub use stream::drawing;

thread_local! {
    /// The last finished recording and the pictures it introduced.
    static FINISHED: RefCell<Option<Vec<u32>>> = const { RefCell::new(None) };
    /// The last finished recording's scrollers and the offsets drawn at.
    static GROUPS: RefCell<Vec<(u32, (f32, f32))>> = const { RefCell::new(Vec::new()) };
}

/// Pictures the reader has not fetched yet, by id: one map for the process,
/// as a reader may fetch them on its own thread while the host runs on
/// another (LLP 1072 on Android).
static PENDING: std::sync::Mutex<std::collections::BTreeMap<u32, Picture>> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());

fn pending() -> std::sync::MutexGuard<'static, std::collections::BTreeMap<u32, Picture>> {
    PENDING.lock().unwrap_or_else(|e| e.into_inner())
}

/// The recording backend.
pub struct Recorder {
    scale: f32,
    ops: Vec<u32>,
    /// The matrix last written into `ops` (points → device pixels).
    matrix: Option<[f32; 6]>,
    /// Faces announced to the reader, by (file, index, weight, axes).
    fonts: HashMap<FontKey, u32>,
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
    /// Each kept row's slots, and the frame's recording while one of them
    /// is drawn again without its row (`canvas/slot.rs`).
    slot_contexts: HashMap<u32, Vec<(ViewId, slot::Context)>>,
    again: Option<slot::Again>,
    /// Nodes the reader animates, apart (`crate::host::lower`).
    layers: layer::Layers,
}

/// A picture slot recording: its id, the row's ops so far, the row's matrix,
/// and which clips were written when it began.
type SlotRecording = (u32, Vec<u32>, Option<[f32; 6]>, Vec<bool>);

/// A face as the reader makes it: file, collection index, weight, and up to
/// three axes as (tag, value bits), unused ones zero.
type FontKey = (Arc<str>, u32, u16, [(u32, u32); 3]);

struct RowRecording {
    clips: Vec<clip::Clip>,
    matrix: Option<[f32; 6]>,
    at: usize,
    id: u32,
    previous: Option<u32>,
    images: Vec<usize>,
    slots: Vec<(ViewId, slot::Context)>,
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
            slot_contexts: HashMap::new(),
            again: None,
            layers: Default::default(),
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
    fn face_file(&mut self, font: &parley::FontData) -> Option<(Arc<str>, u32)> {
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

    fn font(
        &mut self,
        file: &(Arc<str>, u32),
        weight: u16,
        synthesis: &fontique::Synthesis,
    ) -> u32 {
        // Fontique sets at most three axes (`wdth`, `wght`, `ital` or `slnt`).
        let mut axes = [(0u32, 0u32); 3];
        let vars = synthesis.variation_settings();
        for (slot, (tag, value)) in axes.iter_mut().zip(vars) {
            *slot = (u32::from_be_bytes(tag.to_be_bytes()), value.to_bits());
        }
        let n = vars.len().min(axes.len());
        let key = (file.0.clone(), file.1, weight, axes);
        if let Some(k) = self.fonts.get(&key) {
            return *k;
        }
        let k = self.fonts.len() as u32 + 1;
        self.fonts.insert(key, k);
        if n == 0 {
            self.ops.extend([FONT, k, file.1, u32::from(weight)]);
        } else {
            self.ops
                .extend([FONT_AXES, k, file.1, u32::from(weight), n as u32]);
            for (tag, value) in &axes[..n] {
                self.ops.extend([*tag, *value]);
            }
        }
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
        match &mut self.row {
            Some(row) => row.images.push(key),
            None => self.again_image(key),
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
        pending().insert(id, image.clone());
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
        self.again = None;
        self.slot_sets.clear();
        self.layers.begin();
        self.recorded = None;
        GROUPS.with(|g| g.borrow_mut().clear());
        self.frames += 1;
        // Pictures the presenter dropped, and those not drawn for a while
        // that the reader copied: its copy goes, and is sent again when it
        // draws again. Kept rows hold their own reference.
        let frames = self.frames;
        let dead: Vec<(usize, u32)> = self
            .images
            .iter()
            .filter(|(_, (_, weak, used))| {
                !weak.alive() || (frames - used > self.idle && !weak.shared())
            })
            .map(|(k, (id, _, _))| (*k, *id))
            .collect();
        for (k, id) in dead {
            self.images.remove(&k);
            self.ops.extend([IMAGE_FREE, id]);
            pending().remove(&id);
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

    fn blurred_shadow(
        &mut self,
        shape: &Shape,
        color: [u8; 4],
        sigma: f32,
        outer: &Shape,
        ts: Transform,
    ) -> bool {
        self.shadow(shape, color, sigma, outer, ts)
    }

    fn backdrop_filter(
        &mut self,
        shape: &Shape,
        filter: &exact_kernel::style::BackdropFilter,
        ts: Transform,
    ) {
        // What is beneath is what the reader already drew in this row; it
        // filters that, clipped to the shape (LLP 1053.000 D2).
        if filter.is_none() || shape.rect.2 <= 0.0 || shape.rect.3 <= 0.0 {
            return;
        }
        let bounds = clip::map(shape.rect, ts);
        if self.culled(bounds) {
            return;
        }
        self.need(bounds);
        self.transform(ts);
        self.ops.push(BACKDROP);
        self.ops.push(filter.0.len() as u32);
        for op in &filter.0 {
            let (kind, amount) = match op {
                exact_kernel::style::BackdropOp::Blur(n) => (0, *n),
                exact_kernel::style::BackdropOp::Saturate(n) => (1, *n),
            };
            self.ops.push(kind);
            self.f(amount);
        }
        self.rect_radii(shape);
    }

    fn native(
        &mut self,
        id: ViewId,
        kind: crate::paint::NativeKind,
        shape: &Shape,
        ts: Transform,
        props: &str,
    ) {
        let bounds = clip::map(shape.rect, ts);
        if self.culled(bounds) {
            return;
        }
        self.need(bounds);
        self.transform(ts);
        self.ops.extend([NATIVE, id, kind as u32]);
        self.rect_radii(shape);
        self.string(props);
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
            let font = self.font(&file, run.weight, &run.synthesis);
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
                // A layer playing the dash offset finds its phase here.
                self.layers.note_dash(self.ops.len());
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
        // The restore puts back the reader's matrix from before the layer.
        self.matrix = None;
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
            slots: Vec::new(),
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
            slots,
        }) = self.row.take()
        else {
            return 0;
        };
        // The reader's node reaches from the origin to the far edge of what
        // the row covers; a row that recorded nothing (a list's spacer for
        // the rows it has not mounted, a million px tall) covers nothing.
        let s = self.scale;
        let (w, h) = if self.ops.len() == at + 6 {
            (1.0, 1.0)
        } else {
            (bounds.0 + bounds.2 + ROW_PAD, bounds.1 + bounds.3 + ROW_PAD)
        };
        self.ops[at + 3] = (w.max(1.0) * s).to_bits();
        self.ops[at + 4] = (h.max(1.0) * s).to_bits();
        self.ops.push(ROW_END);
        self.ops[at + 5] = (self.ops.len() - at - 6) as u32;
        self.clips = clips;
        self.matrix = matrix;
        self.origin = (0.0, 0.0);
        // Size and body: the same as the row's last recording, the reader's
        // node for it stands.
        let body = &self.ops[at + 3..];
        let mut sets = std::mem::take(&mut self.slot_sets);
        sets.append(&mut self.layers.sets);
        if let Some(old) = previous.filter(|p| self.kept.get(p).is_some_and(|k| k.0[..] == *body)) {
            self.ops.truncate(at);
            self.ops.extend(sets);
            self.kept.get_mut(&old).expect("kept").1 = images;
            self.keep_slots(old, slots);
            self.layers.row_end(old);
            return old;
        }
        let body = body.to_vec();
        self.kept.insert(id, (body, images));
        self.keep_slots(id, slots);
        self.recorded = Some((id, at));
        self.ops.extend(sets);
        self.layers.row_end(id);
        id
    }

    fn slot_begin(&mut self, id: ViewId) {
        self.slot_open(id);
    }

    fn slot_end(&mut self) {
        self.slot_close();
    }

    fn slot_again(&mut self, row: u32, id: ViewId) -> bool {
        self.slot_reopen(row, id)
    }

    fn layer_begin(&mut self, key: u64, ts: Transform, pivot: (f32, f32), base: [f32; 7]) -> bool {
        self.layer_open(key, ts, pivot, base);
        true
    }

    fn layer_recolored(&mut self) {
        self.layers.note_recolored();
    }

    fn layer_end(&mut self) {
        self.layer_close();
    }

    fn row_culled(&self, bounds: Rect4) -> bool {
        // A scroller's rows all draw so a move needs no paint, unless its
        // clips leave nothing (a page out of view): no move shows them.
        match self.group {
            None => self.culled(Some(bounds)),
            Some(_) => self.clipped_out(),
        }
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
        self.slot_contexts.remove(&id);
        self.layers.row_free(id);
        self.ops.extend([ROW_FREE, id]);
    }

    fn finish(&mut self) -> Result<Pixmap, String> {
        self.layers_finish();
        let ops = std::mem::take(&mut self.ops);
        FINISHED.with(|f| *f.borrow_mut() = Some(ops));
        Pixmap::new(1, 1).ok_or_else(|| "placeholder".to_string())
    }
}

/// `CLOCK_MONOTONIC` now, in nanoseconds: the clock `Instant` and the
/// reader's frame times both read.
fn monotonic_ns() -> i64 {
    let mut t = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `t` is a valid out pointer for the call's duration.
    #[allow(unsafe_code)]
    unsafe {
        libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut t);
    }
    t.tv_sec * 1_000_000_000 + t.tv_nsec
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
    /// When the last scroll step came, whether a pass waits for more,
    /// whether the last pass left rows to build, whether it led, and whether
    /// a lead's rows are still to retire.
    scrolled_at: f64,
    waiting: bool,
    leftover: bool,
    led: bool,
    settling: bool,
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
    /// The feed's travel, and the frame (a count of `moved`) by which a
    /// paint must show rows a pass mounted out of view ([`crate::travel`]).
    travel: crate::travel::Travel,
    /// Scratch for a pass's row count ([`CanvasHost::refine_slice`]).
    rows_before: Vec<(ViewId, u64)>,
    rows_after: Vec<(ViewId, u64)>,
    paint_by: Option<u32>,
    /// The reader takes layers' tracks after a move (`EXACT_MOVE_TRACKS=1`,
    /// which its launcher sets): a row that comes into view then starts its
    /// animations without a paint, and a paint is owed only by the frame
    /// rows mounted out of view can first show ([`CanvasHost::unhurried`]).
    move_tracks: bool,
    /// The last pass was a slice: it may have left rows of its window unbuilt.
    sliced: bool,
    /// A GPU canvas wants another frame (Android: they present each frame).
    surfaces: bool,
    /// GPU canvases wait for the host's own thread: it booted on another
    /// ([`CanvasHost::set_borrowed`]), and a GPU module is the thread's that
    /// loads it.
    borrowed: bool,
    /// The frame that makes the canvases a booting thread left is owed.
    owed_surfaces: bool,
    /// The monotonic clock at `started` (ns), sent once (`CLOCK`), and each
    /// live layer's keyframes as last sent (`TRACKS`).
    origin_ns: i64,
    clock_sent: bool,
    tracks: HashMap<u32, Vec<u32>>,
    /// The scroller [`CanvasHost::scroll`] moves.
    feed: Option<ViewId>,
    /// Whether the windows' lead is back after the first frame (and after
    /// a touch's response).
    lead: bool,
    /// A frame has painted. Not `painted`, which is `None` while anything
    /// moves (a transition): the lead and the canvases a first frame or a
    /// touch left must not wait on that, or `next_due` asks for a turn at
    /// once until the motion ends.
    painted_once: bool,
    /// The last frame left its GPU canvases for a device still being made.
    gpu_waiting: bool,
    /// A touch came: until its frame is out, collection passes build only
    /// the rows that show (a slice of 0); the lead follows that frame.
    responding: bool,
    tracks_epoch: u64,
}

struct Painted {
    still: crate::presenter::still::Still,
    scroll: std::collections::BTreeMap<ViewId, (f32, f32)>,
    groups: Vec<(u32, (f32, f32))>,
}

/// At most this many frames move the last paint before one paints again:
/// what it leaves stale (boxes, hits, which pictures show) stays this fresh.
const MOVES: u32 = 30;
/// The same, once rows have mounted out of view since the last paint, where
/// the frame they can first show is not known ([`CanvasHost::unhurried`]).
const MOVES_MOUNTED: u32 = 6;

impl<D: DataSource + Default> CanvasHost<D> {
    /// Boot `D`'s app over a view of `size` pixels at `scale` pixels per
    /// point. Assets use the Linux environment; the carrier directly selects
    /// its Canvas recorder and enables the reader's native motion lowering.
    pub fn boot(
        plan: &'static [u8],
        compat: &'static str,
        size: (u32, u32),
        scale: f32,
    ) -> Result<CanvasHost<D>, String> {
        let started = std::time::Instant::now();
        let origin_ns = monotonic_ns();
        // Motion lowering still reads this policy; painter construction is direct.
        std::env::set_var("EXACT_PAINTER", "canvas");
        // Eleven viewports of decoded pictures: the reader's copy is a GPU
        // buffer (no heap copy, no upload), a picture decoded again costs
        // more than the memory it holds, and pictures are kept at the size
        // they show at (`image::png_decode::DecodePlan`). Twelve until they
        // were asked for at mount (below): heavy then ended 10 MB higher.
        if std::env::var_os("EXACT_IMAGE_VIEWPORTS").is_none() {
            std::env::set_var("EXACT_IMAGE_VIEWPORTS", "11");
        }
        // A picture is asked for when its row is built (`presenter::images`).
        if std::env::var_os("EXACT_PICTURES_AT_MOUNT").is_none() {
            std::env::set_var("EXACT_PICTURES_AT_MOUNT", "1");
        }
        std::env::set_var("EXACT_SCALE", scale.to_string());
        let viewport = (size.0 as f32 / scale, size.1 as f32 / scale);
        // The first frame realizes only the rows that show; the window's lead
        // follows once it is out ([`CanvasHost::frame`]).
        exact_runner::set_lead_scale(0.0);
        // A new list builds the rows the view can show before its first
        // layout, not in a second commit after it.
        exact_runner::set_bootstrap_extent(size.1.max(size.0) as f64 / scale as f64);
        // A retiring row is rebound to the next item instead of a row built
        // for it (LLP 1078), unless `EXACT_ROW_REUSE=0`.
        crate::app::row_reuse_from_env(true);
        #[cfg(target_os = "android")]
        crate::surfaces::prepare_gpu(compat);
        let mut config = crate::app::Config::from_env_static(plan, compat);
        config.scale = scale;
        let (p, error) = crate::app::boot_presenter_with_painter::<D>(
            &mut config,
            viewport,
            crate::presenter::PainterBoot::canvas(),
        )?;
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
            scrolled_at: 0.0,
            waiting: false,
            leftover: false,
            led: false,
            settling: false,
            force: false,
            scrolled: false,
            prefetching: false,
            quiet: None,
            travel: crate::travel::Travel::default(),
            rows_before: Vec::new(),
            rows_after: Vec::new(),
            paint_by: None,
            move_tracks: std::env::var("EXACT_MOVE_TRACKS").is_ok_and(|v| v == "1"),
            sliced: false,
            moves: std::env::var("EXACT_MOVES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(MOVES),
            surfaces: false,
            borrowed: false,
            owed_surfaces: false,
            origin_ns,
            clock_sent: false,
            tracks: HashMap::new(),
            feed: None,
            tracks_epoch: 0,
            lead: false,
            painted_once: false,
            gpu_waiting: false,
            responding: false,
        })
    }

    /// A reader's `ANativeWindow` of `size` pixels for canvas `view`, alive
    /// until [`CanvasHost::detach_window`]; whether `view` is a GPU canvas.
    #[cfg(target_os = "android")]
    pub fn attach_window(&mut self, view: u32, window: usize, size: (u32, u32)) -> bool {
        self.surfaces = true;
        self.p.attach_window(view, window, size)
    }

    /// The window of canvas `view` is going.
    #[cfg(target_os = "android")]
    pub fn detach_window(&mut self, view: u32) {
        self.p.detach_window(view);
    }

    fn now(&self) -> f64 {
        self.started.elapsed().as_secs_f64() * 1000.0
    }

    /// One turn: the presenter's work, then a recording when anything
    /// changed. `Some` is the new op stream (valid until the next call).
    pub fn frame(&mut self) -> Option<Vec<u32>> {
        // After the first frame: the windows' lead, realized off the frame.
        if self.painted_once && !self.lead && !self.responding {
            self.lead = true;
            exact_runner::set_lead_scale(1.0);
            self.p.refine_deferred(true);
        }
        let p = &mut self.p;
        let now = self.started.elapsed().as_secs_f64() * 1000.0;
        let _frame = Section::begin(c"exact frame");
        p.hold_collections(self.scrolled);
        let frame = self.frame_held(now);
        self.p.hold_collections(false);
        if std::mem::take(&mut self.responding) {
            self.p.slice_collections(None, 0.0);
        }
        // Every attached GPU canvas (none: at once); a canvas with nothing
        // new draws nothing.
        #[cfg(target_os = "android")]
        {
            let _s = Section::begin(c"exact surfaces");
            // GPU canvases follow the tree (made, bound, given their
            // assets) outside a scroll's frame, as the Linux loop does.
            // `lead` turns on at the frame after the first paint.
            // The GPU device still being made (on its own thread, since boot):
            // the canvases wait a frame rather than this thread waiting for it.
            self.gpu_waiting = crate::surfaces::gpu_device_pending();
            if self.borrowed || !self.lead || self.gpu_waiting {
                // On a booting thread, or the first frame: the canvases come
                // with the next frame (made then, as a picture arrives after
                // first content, not before it).
                self.owed_surfaces = true;
            } else {
                self.owed_surfaces = false;
                if !self.scrolled {
                    self.p.sync_surfaces();
                }
                self.surfaces = self.p.render_surfaces(now);
            }
        }
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
            // The pump may have drained the wake of a data source that finished loading.
            if p.module_pending() {
                p.first_pixel();
            }
        }
        if let Some(mut shift) = self.shift(now) {
            // A play that changed since the last stream (a row that came
            // into view starts its animations): its layer's tracks go with
            // the move, where the reader takes them there.
            if self.move_tracks && self.p.host().lowered_epoch() != self.tracks_epoch {
                self.layer_tracks(&mut shift);
            }
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
        self.painted_once = true;
        self.painted = p.still().map(|still| Painted {
            still,
            scroll: p.scroll_offsets().clone(),
            groups: GROUPS.with(|g| g.borrow().clone()),
        });
        self.moved = 0;
        self.paint_by = None;
        self.force = false;
        self.quiet = None;
        let mut ops = FINISHED.with(|f| f.borrow_mut().take())?;
        self.layer_tracks(&mut ops);
        Some(ops)
    }

    /// After a paint: each live layer's keyframes, where they changed, and
    /// the clock they run on, once (`crate::host::lower`).
    fn layer_tracks(&mut self, ops: &mut Vec<u32>) {
        let alive = layer::ALIVE.with(|a| a.borrow().clone());
        if alive.is_empty() && self.tracks.is_empty() {
            return;
        }
        if !self.clock_sent {
            self.clock_sent = true;
            ops.extend([
                layer::CLOCK,
                self.origin_ns as u32,
                (self.origin_ns >> 32) as u32,
            ]);
        }
        let ids: std::collections::HashSet<u32> = alive.iter().map(|a| a.0).collect();
        self.tracks.retain(|id, _| ids.contains(id));
        // Plays change only in a sync: until one, only new layers' tracks;
        // after one, the layers of the nodes it changed.
        let epoch = self.p.host().lowered_epoch();
        let since = self.tracks_epoch;
        self.tracks_epoch = epoch;
        for (id, key, base) in alive {
            let key = exact_kernel::motion::node_key(key);
            if self.tracks.contains_key(&id)
                && (epoch == since || !self.p.host().lowered_changed_after(key, since))
            {
                continue;
            }
            let mut p = crate::paint::Presented::IDENTITY;
            p.translate = (base[0], base[1]);
            p.scale = base[2];
            p.rotate = base[3];
            p.opacity = base[4];
            let words = self.p.host().layer_tracks(key, &p);
            if self.tracks.get(&id) != Some(&words) {
                ops.extend([layer::TRACKS, id, words.len() as u32]);
                ops.extend(&words);
                self.tracks.insert(id, words);
            }
        }
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
        // The executor fd also wakes when a data source finishes loading.
        if self.p.module_pending() {
            self.p.first_pixel();
        }
        self.animating()
    }

    /// Whether another frame is wanted at the next vsync.
    pub fn animating(&self) -> bool {
        // A moved paint owes a paint: the frame after scrolling stops.
        self.p.dirty()
            || self.p.needs_animation_frame()
            || self.p.host().wants_frames()
            || self.surfaces
    }

    fn owed_at(&self, now: f64) -> bool {
        self.moved > 0 && self.moves < 1000 && now - self.moved_at >= 12.0
    }

    fn shift(&mut self, at: f64) -> Option<Vec<u32>> {
        let p = &self.p;
        // Rows mounted out of view since the paint (a quiet epoch) show at the
        // next one, so it comes sooner: they may scroll in within a quarter
        // second.
        let limit = if self.quiet.is_some() && !self.unhurried() {
            self.moves.min(MOVES_MOUNTED)
        } else {
            self.moves
        }
        .min(self.paint_by.unwrap_or(u32::MAX));
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

    /// Whether this host runs, for now, on a thread that only boots it: its
    /// GPU canvases wait (see `canvas_jni!`'s `startAsync`). Clearing it
    /// owes a frame.
    pub fn set_borrowed(&mut self, borrowed: bool) {
        if self.borrowed && !borrowed {
            self.force = true;
            self.owed_surfaces = true;
        }
        self.borrowed = borrowed;
    }

    /// Milliseconds until the next timer, if any.
    pub fn next_due(&self) -> Option<f64> {
        // The windows' lead is owed a turn as soon as the first frame is out,
        // and the GPU canvases a booting thread left.
        if self.owed_surfaces && self.gpu_waiting && self.lead {
            // Asked again in a few ms, not at once: a turn now would only find
            // the device still being made.
            return Some(4.0);
        }
        if (self.painted_once && !self.lead) || self.owed_surfaces {
            return Some(0.0);
        }
        let timer = (self.p.host().timer_due_ms()).map(|due| (due - self.now()).max(0.0));
        self.settle_due(timer)
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
        self.scrolled_at = self.now();
        self.travel.scrolled(dy / self.scale, self.scrolled_at);
        self.p.hold_collections(self.prefetching);
        // The feed: the scroller the first wheel at the centre took, moved
        // directly after (a nested list under the centre would take it).
        let moved = self
            .feed
            .is_some_and(|id| self.p.scroll_by(id, dy / self.scale));
        if !moved && self.feed.is_none() {
            self.p.wheel_at(x, y, 0.0, dy / self.scale);
            self.feed = self.p.last_wheel();
        }
        self.p.hold_collections(false);
    }

    /// A touch (0 down, 1 up, 2 move, 3 cancel) at pixels.
    pub fn touch(&mut self, action: i32, x: f32, y: f32) {
        let _s = Section::begin(c"exact touch");
        self.force |= action != 2;
        if action != 2 && self.painted_once {
            self.p.slice_collections(Some(0), 0.0);
            self.responding = true;
            self.lead = false;
            self.travel.stopped();
        }
        let now = self.now();
        let (x, y) = (x / self.scale, y / self.scale);
        let r = match action {
            0 => self.p.pointer_down(x, y, now).map(|_| ()),
            1 => self.p.pointer_up(x, y, now).map(|_| ()),
            2 => self.p.pointer_move(x, y, now).map(|_| ()),
            _ => self.p.pointer_cancel(now),
        };
        self.touched(action, x, y, now, r);
    }

    /// A picture announced by `IMAGE_DEF`, once (premultiplied RGBA rows).
    pub fn image(id: u32) -> Option<Picture> {
        image(id)
    }

    /// The view is `size` pixels after all (the reader booted at the size it
    /// expected).
    pub fn resize(&mut self, size: (u32, u32)) {
        let viewport = (size.0 as f32 / self.scale, size.1 as f32 / self.scale);
        if viewport == self.viewport {
            return;
        }
        self.viewport = viewport;
        if let Some(e) = self.p.resize(viewport.0, viewport.1) {
            eprintln!("exact: resize: {e}");
        }
        self.force = true;
    }
}

/// The pictures announced and not yet fetched: id, width, height.
pub fn pending_pictures() -> Vec<[u32; 3]> {
    pending()
        .iter()
        .map(|(id, p)| [*id, p.width(), p.height()])
        .collect()
}

/// A picture announced by `IMAGE_DEF`, once (premultiplied RGBA rows).
fn image(id: u32) -> Option<Picture> {
    pending().remove(&id)
}
