//! The recorder's image members (LLP 1056 D9, §3 stage 2): `drawImage` over
//! image handles, `createPattern`, and raw pixels (`ImageData`,
//! `putImageData`).
//!
//! A handle is the string an `image` node's `src` takes. Its readiness is
//! the environment's answer at the call, pinned for the draw: a handle not
//! yet decoded draws nothing, as an incomplete `HTMLImageElement` does, and
//! asking for it is what makes the host decode it and redraw the canvas
//! (`frame.cause` `"image"`).

use super::{lock, throw, Context2d, DomException, Inner, Paint};
use crate::geom::Matrix;
use crate::list::{text_operands, Op};
use std::sync::{Arc, Mutex};

/// The decoded image handles a context can draw. [`ImageTable`] is the one
/// the runner keeps; a test may put its own in an [`ImageSlot`].
pub trait Images: Send + Sync {
    /// The natural size of `src`, in image pixels, when it is decoded and
    /// drawable; `None` while it is not (not yet decoded, broken, or
    /// zero-sized). Asking for one the host has not seen requests it for
    /// canvas `canvas`, which is drawn again when it decodes.
    fn size(&self, canvas: u64, src: &str) -> Option<(u32, u32)>;
    /// The handles asked for since the last take, for the host to decode.
    fn take_requests(&self) -> Vec<String> {
        Vec::new()
    }
    /// Whether a requested handle is unanswered.
    fn pending(&self) -> bool {
        false
    }
    /// The host's answer for `src`; returns the canvases (lifetimes) that
    /// asked for it, with `also`.
    fn answer(&self, src: &str, result: Result<(u32, u32), String>, also: &[u64]) -> Vec<u64> {
        let _ = (src, result);
        also.to_vec()
    }
    /// `src: why` for each broken handle canvas `canvas` asked for.
    fn broken(&self, canvas: u64) -> Vec<String> {
        let _ = canvas;
        Vec::new()
    }
}

/// Where a runner's image table lives. It is made by the first image call
/// a draw makes ([`images_in`]), so an app whose canvases draw no image
/// links none of it (LLP 1047 D2: linked by use).
pub type ImageSlot = Arc<Mutex<Option<Arc<dyn Images>>>>;

/// The table in `slot`, made on first use.
pub fn images_in(slot: &ImageSlot) -> Arc<dyn Images> {
    let mut s = slot.lock().unwrap_or_else(|e| e.into_inner());
    s.get_or_insert_with(|| Arc::new(ImageTable::default()))
        .clone()
}

/// The image handles 2D canvases draw (LLP 1056 D9): decoded, broken, or
/// requested of the host, and which canvases asked. A context reads it at
/// the call, so readiness is pinned per draw.
#[derive(Default)]
pub struct ImageTable {
    inner: Mutex<ImageState>,
}

#[derive(Default)]
struct ImageState {
    ready: Vec<(String, (u32, u32))>,
    broken: Vec<(String, String)>,
    pending: Vec<String>,
    requests: Vec<String>,
    subscribers: Vec<(String, Vec<u64>)>,
}

impl ImageState {
    fn subscribers(&mut self, src: &str) -> &mut Vec<u64> {
        let i = match self.subscribers.iter().position(|(s, _)| s == src) {
            Some(i) => i,
            None => {
                self.subscribers.push((src.to_string(), Vec::new()));
                self.subscribers.len() - 1
            }
        };
        &mut self.subscribers[i].1
    }
}

impl ImageTable {
    fn state(&self) -> std::sync::MutexGuard<'_, ImageState> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Images for ImageTable {
    fn size(&self, canvas: u64, src: &str) -> Option<(u32, u32)> {
        let mut s = self.state();
        if let Some((_, size)) = s.ready.iter().find(|(r, _)| r == src) {
            return Some(*size);
        }
        let subs = s.subscribers(src);
        if !subs.contains(&canvas) {
            subs.push(canvas);
        }
        if !s.broken.iter().any(|(b, _)| b == src) && !s.pending.iter().any(|p| p == src) {
            s.pending.push(src.to_string());
            s.requests.push(src.to_string());
        }
        None
    }

    fn take_requests(&self) -> Vec<String> {
        std::mem::take(&mut self.state().requests)
    }

    fn pending(&self) -> bool {
        !self.state().pending.is_empty()
    }

    fn answer(&self, src: &str, result: Result<(u32, u32), String>, also: &[u64]) -> Vec<u64> {
        let mut s = self.state();
        s.pending.retain(|p| p != src);
        s.ready.retain(|(r, _)| r != src);
        s.broken.retain(|(b, _)| b != src);
        match result {
            Ok(size) if size.0 > 0 && size.1 > 0 => s.ready.push((src.to_string(), size)),
            Ok(_) => s
                .broken
                .push((src.to_string(), "the image is zero-sized".into())),
            Err(why) => s.broken.push((src.to_string(), why)),
        }
        let subs = s.subscribers(src);
        for l in also {
            if !subs.contains(l) {
                subs.push(*l);
            }
        }
        subs.clone()
    }

    fn broken(&self, canvas: u64) -> Vec<String> {
        let s = self.state();
        s.broken
            .iter()
            .filter(|(src, _)| {
                s.subscribers
                    .iter()
                    .any(|(x, l)| x == src && l.contains(&canvas))
            })
            .map(|(src, why)| format!("{src}: {why}"))
            .collect()
    }
}

/// `ImageData`: raw RGBA pixels, non-premultiplied, sRGB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageData {
    /// Pixels per row.
    pub width: u32,
    /// Rows.
    pub height: u32,
    /// `width × height × 4` bytes.
    pub data: Vec<u8>,
}

fn dimension(v: f64, which: &str) -> Result<u32, DomException> {
    if !v.is_finite() {
        return throw("TypeError", "The provided double value is non-finite.");
    }
    let n = v.trunc().abs();
    if n == 0.0 {
        return throw(
            "IndexSizeError",
            format!("The source {which} is zero or not a number."),
        );
    }
    if n > u32::MAX as f64 / 4.0 {
        return throw("RangeError", "Out of memory at ImageData creation.");
    }
    Ok(n as u32)
}

impl ImageData {
    /// `new ImageData(sw, sh)`: transparent black.
    pub fn new_with_sw(sw: u32, sh: u32) -> Result<ImageData, DomException> {
        if sw == 0 || sh == 0 {
            return throw("IndexSizeError", "The source width or height is zero.");
        }
        Ok(ImageData {
            width: sw,
            height: sh,
            data: vec![0; sw as usize * sh as usize * 4],
        })
    }

    /// `new ImageData(data, sw, sh?)`: `data`'s length must be a whole
    /// number of rows of `sw` pixels.
    pub fn new_with_u8_clamped_array_and_sh(
        data: Vec<u8>,
        sw: u32,
        sh: Option<u32>,
    ) -> Result<ImageData, DomException> {
        if data.is_empty() || !data.len().is_multiple_of(4) {
            return throw(
                "InvalidStateError",
                "The input data length is not a non-zero multiple of 4.",
            );
        }
        let pixels = data.len() / 4;
        if sw == 0 || !pixels.is_multiple_of(sw as usize) {
            return throw(
                "IndexSizeError",
                "The input data length is not a multiple of (4 * width).",
            );
        }
        let rows = (pixels / sw as usize) as u32;
        if sh.is_some_and(|h| h != rows) {
            return throw(
                "IndexSizeError",
                "The input data length is not equal to (4 * width * height).",
            );
        }
        Ok(ImageData {
            width: sw,
            height: rows,
            data,
        })
    }
}

/// A `CanvasPattern`: an id in its context's list.
#[derive(Clone)]
pub struct CanvasPattern {
    pub(crate) id: u32,
    pub(crate) inner: Arc<Mutex<Inner>>,
}

impl std::fmt::Debug for CanvasPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CanvasPattern({})", self.id)
    }
}

impl PartialEq for CanvasPattern {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl CanvasPattern {
    /// `setTransform(DOMMatrix2DInit)`: the pattern space's matrix, applied
    /// at later paints.
    pub fn set_transform(&self, m: &Matrix) -> Result<(), DomException> {
        if !m.operands().iter().all(|v| v.is_finite()) {
            return throw("TypeError", "The matrix has non-finite entries.");
        }
        let mut ops = vec![self.id as f64];
        ops.extend_from_slice(&m.operands());
        lock(&self.inner).op(Op::PatternTransform, &ops);
        Ok(())
    }
}

impl Inner {
    /// The natural size of a handle this draw may use.
    fn image_size(&self, src: &str) -> Option<(u32, u32)> {
        images_in(&self.env.images)
            .size(self.env.canvas, src)
            .filter(|(w, h)| *w > 0 && *h > 0)
    }

    /// The handle's id in this generation, recording it on first use.
    fn image_id(&mut self, src: &str) -> u32 {
        if let Some((_, id)) = self.image_ids.iter().find(|(s, _)| s == src) {
            return *id;
        }
        let id = self.next_object();
        let mut ops = vec![id as f64];
        text_operands(src, &mut ops);
        self.op(Op::Image, &ops);
        self.image_ids.push((src.to_string(), id));
        id
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_image(
        &mut self,
        src: &str,
        source: Option<[f64; 4]>,
        dest: [f64; 2],
        size: Option<[f64; 2]>,
    ) {
        let all = source
            .iter()
            .flatten()
            .chain(dest.iter())
            .chain(size.iter().flatten());
        if !all.clone().all(|v| v.is_finite()) || !self.paints() {
            return;
        }
        let Some((w, h)) = self.image_size(src) else {
            return;
        };
        let (w, h) = (w as f64, h as f64);
        let [mut sx, mut sy, mut sw, mut sh] = source.unwrap_or([0.0, 0.0, w, h]);
        let [mut dx, mut dy] = dest;
        let [mut dw, mut dh] = size.unwrap_or([sw, sh]);
        for (p, l) in [
            (&mut sx, &mut sw),
            (&mut sy, &mut sh),
            (&mut dx, &mut dw),
            (&mut dy, &mut dh),
        ] {
            if *l < 0.0 {
                *p += *l;
                *l = -*l;
            }
        }
        if sw == 0.0 || sh == 0.0 || dw == 0.0 || dh == 0.0 {
            return;
        }
        // Clip the source to the image, the destination with it.
        let (kx, ky) = (dw / sw, dh / sh);
        let (x0, y0) = (sx.max(0.0), sy.max(0.0));
        let (x1, y1) = ((sx + sw).min(w), (sy + sh).min(h));
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        dx += (x0 - sx) * kx;
        dy += (y0 - sy) * ky;
        dw = (x1 - x0) * kx;
        dh = (y1 - y0) * ky;
        let id = self.image_id(src);
        self.op(
            Op::DrawImage,
            &[id as f64, x0, y0, x1 - x0, y1 - y0, dx, dy, dw, dh],
        );
    }
}

impl Context2d {
    /// `drawImage(image, dx, dy)` with an image handle (LLP 1056 D9).
    pub fn draw_image_with_image_handle(
        &self,
        src: &str,
        dx: f64,
        dy: f64,
    ) -> Result<(), DomException> {
        self.g().draw_image(src, None, [dx, dy], None);
        Ok(())
    }

    /// `drawImage(image, dx, dy, dw, dh)`.
    pub fn draw_image_with_image_handle_and_dw_and_dh(
        &self,
        src: &str,
        dx: f64,
        dy: f64,
        dw: f64,
        dh: f64,
    ) -> Result<(), DomException> {
        self.g().draw_image(src, None, [dx, dy], Some([dw, dh]));
        Ok(())
    }

    /// `drawImage(image, sx, sy, sw, sh, dx, dy, dw, dh)`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_image_with_image_handle_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
        &self,
        src: &str,
        sx: f64,
        sy: f64,
        sw: f64,
        sh: f64,
        dx: f64,
        dy: f64,
        dw: f64,
        dh: f64,
    ) -> Result<(), DomException> {
        self.g()
            .draw_image(src, Some([sx, sy, sw, sh]), [dx, dy], Some([dw, dh]));
        Ok(())
    }

    /// `createPattern(image, repetition)`: `SyntaxError` for an unknown
    /// repetition; `None` while the image is not decoded.
    pub fn create_pattern_with_image_handle(
        &self,
        src: &str,
        repetition: &str,
    ) -> Result<Option<CanvasPattern>, DomException> {
        let rep = match repetition {
            "" | "repeat" => 0,
            "repeat-x" => 1,
            "repeat-y" => 2,
            "no-repeat" => 3,
            other => {
                return throw(
                    "SyntaxError",
                    format!("The provided type ('{other}') is not one of 'repeat', 'no-repeat', 'repeat-x', or 'repeat-y'."),
                )
            }
        };
        let mut g = self.g();
        if g.image_size(src).is_none() {
            return Ok(None);
        }
        let image = g.image_id(src);
        let id = g.next_object();
        g.op(Op::Pattern, &[id as f64, image as f64, rep as f64]);
        Ok(Some(CanvasPattern {
            id,
            inner: self.inner.clone(),
        }))
    }

    fn set_style_pattern(&self, fill: bool, pattern: &CanvasPattern) {
        let mut g = self.g();
        if !Arc::ptr_eq(&pattern.inner, &self.inner) {
            g.note("a pattern from another canvas was ignored".into());
            return;
        }
        let p = Paint::Pattern(pattern.id);
        g.paint_op(fill, &p);
        if fill {
            g.state.fill = p;
        } else {
            g.state.stroke = p;
        }
    }

    /// `fillStyle = pattern`.
    pub fn set_fill_style_canvas_pattern(&self, v: &CanvasPattern) {
        self.set_style_pattern(true, v)
    }

    /// `strokeStyle = pattern`.
    pub fn set_stroke_style_canvas_pattern(&self, v: &CanvasPattern) {
        self.set_style_pattern(false, v)
    }

    /// `createImageData(sw, sh)`: backing pixels (LLP 1056 D6), transparent
    /// black; the magnitudes, truncated; zero is `IndexSizeError`.
    pub fn create_image_data_with_sw_and_sh(
        &self,
        sw: f64,
        sh: f64,
    ) -> Result<ImageData, DomException> {
        ImageData::new_with_sw(dimension(sw, "width")?, dimension(sh, "height")?)
    }

    /// `createImageData(imagedata)`: the same size, transparent black.
    pub fn create_image_data_with_imagedata(
        &self,
        data: &ImageData,
    ) -> Result<ImageData, DomException> {
        ImageData::new_with_sw(data.width, data.height)
    }

    /// `putImageData(imagedata, dx, dy)`.
    pub fn put_image_data(&self, data: &ImageData, dx: f64, dy: f64) -> Result<(), DomException> {
        self.put_image_data_with_dirty_x_and_dirty_y_and_dirty_width_and_dirty_height(
            data,
            dx,
            dy,
            0.0,
            0.0,
            data.width as f64,
            data.height as f64,
        )
    }

    /// `putImageData(imagedata, dx, dy, dirtyX, dirtyY, dirtyWidth,
    /// dirtyHeight)`: raw backing pixels, copied now; the transform, clip,
    /// alpha, compositing and shadows do not apply.
    #[allow(clippy::too_many_arguments)]
    pub fn put_image_data_with_dirty_x_and_dirty_y_and_dirty_width_and_dirty_height(
        &self,
        data: &ImageData,
        dx: f64,
        dy: f64,
        dirty_x: f64,
        dirty_y: f64,
        dirty_width: f64,
        dirty_height: f64,
    ) -> Result<(), DomException> {
        let all = [dx, dy, dirty_x, dirty_y, dirty_width, dirty_height];
        if !all.iter().all(|v| v.is_finite()) {
            return throw("TypeError", "Value is not of type 'long'.");
        }
        let [dx, dy, mut x, mut y, mut w, mut h] = all.map(|v| v.trunc());
        if w < 0.0 {
            x += w;
            w = -w;
        }
        if h < 0.0 {
            y += h;
            h = -h;
        }
        if x < 0.0 {
            w += x;
            x = 0.0;
        }
        if y < 0.0 {
            h += y;
            y = 0.0;
        }
        w = w.min(data.width as f64 - x);
        h = h.min(data.height as f64 - y);
        if w <= 0.0 || h <= 0.0 {
            return Ok(());
        }
        let (x0, y0, w, h) = (x as usize, y as usize, w as usize, h as usize);
        let mut ops = Vec::with_capacity(4 + w * h);
        ops.extend_from_slice(&[dx + x, dy + y, w as f64, h as f64]);
        for row in y0..y0 + h {
            for col in x0..x0 + w {
                let i = (row * data.width as usize + col) * 4;
                let p = &data.data[i..i + 4];
                ops.push(u32::from_be_bytes([p[0], p[1], p[2], p[3]]) as f64);
            }
        }
        self.g().op(Op::PutImageData, &ops);
        Ok(())
    }
}
