//! What the paint walk draws through.
use super::*;

/// The consumer of a completed paint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameTarget {
    /// Return the complete image without presenting it to a window.
    Pixels,
    /// Present to the window, or return pixels for its carrier to present.
    Window,
    /// Refresh painted geometry without changing the visible window image.
    Geometry,
}

/// UI below a split window canvas, with storage retained for its next repaint.
pub enum WindowLayer {
    /// Pixels contain the complete painted layer.
    Pixels(Pixmap),
    /// Every pixel has this premultiplied RGBA value; storage need not contain it.
    Solid {
        /// Premultiplied RGBA bytes.
        color: [u8; 4],
        /// Allocation available for the next frame.
        storage: Pixmap,
    },
}

impl WindowLayer {
    /// Materialize the layer for a CPU reader.
    pub fn into_pixels(self) -> Pixmap {
        match self {
            Self::Pixels(pixels) => pixels,
            Self::Solid { color, mut storage } => {
                let color = tiny_skia::PremultipliedColorU8::from_rgba(
                    color[0], color[1], color[2], color[3],
                )
                .expect("a window clear is premultiplied");
                storage.pixels_mut().fill(color);
                storage
            }
        }
    }

    #[cfg(windows)]
    pub(crate) fn storage(self) -> Pixmap {
        match self {
            Self::Pixels(pixels) => pixels,
            Self::Solid { storage, .. } => storage,
        }
    }

    #[cfg(windows)]
    pub(crate) fn bytes(&self) -> &[u8] {
        match self {
            Self::Pixels(pixels) => pixels.data(),
            Self::Solid { color, .. } => color,
        }
    }
}

/// What draws the walk's output. Coordinates are viewport points with a
/// transform (points → points); a backend applies the device scale itself.
pub trait Backend {
    /// Collect completed devices without repainting; return those still retiring.
    #[cfg(windows)]
    fn collect_retired_devices(&mut self) -> usize {
        0
    }
    /// The display's interval, with backoff while occluded.
    #[cfg(windows)]
    fn presentation_retry_delay(&self) -> Option<std::time::Duration> {
        None
    }
    /// Only device removal invalidates a directly presenting device's resources.
    #[cfg(windows)]
    fn device_lost(&self) -> bool {
        false
    }
    /// Displayed-frame time since process creation, when the device exposes it.
    /// Explicit measurement only: this may wait for the compositor.
    #[cfg(windows)]
    fn displayed_process_ms(&self) -> Result<Option<f64>, String> {
        Ok(None)
    }
    /// Attach the carrier's owned window when the device can present directly.
    #[cfg(windows)]
    fn attach_window(
        &mut self,
        _window: exact_ink::device::d3d11::Window,
        _size: [u32; 2],
    ) -> Result<bool, String> {
        Ok(false)
    }
    /// The selected backend's name.
    fn name(&self) -> &'static str;
    /// Whether geometry queries must draw through this backend to keep its
    /// recordings or native state current. Stateless raster backends may opt out.
    fn needs_geometry_paint(&self) -> bool {
        true
    }
    /// The page's own colour, told before each frame begins: what a frame
    /// is cleared to, so an unset background is the platform's surface for
    /// the scheme and never white under a dark one (LLP 1115 D2; LLP
    /// 1115.001 D3.6). A backend that draws over a shell's own surface (the
    /// Canvas recorder, whose reader paints it behind the view) clears
    /// nothing and ignores it.
    fn page(&mut self, _color: [u8; 4]) {}
    /// A new frame of this size in points at this scale, cleared to the
    /// page's colour ([`Backend::page`]; white until one is told).
    fn begin(&mut self, width: f32, height: f32, scale: f32);
    /// Release idle renderer storage on memory pressure or backgrounding.
    fn trim(&mut self) {}
    /// Storage from a presented frame after its last reader releases it.
    fn recycle_frame(&mut self, _pixels: Pixmap) {}
    /// Seed a clipped repaint from an accepted frame; false means full repaint.
    fn damage(&mut self, _previous: &Pixmap, _rects: &[Rect4]) -> bool {
        false
    }
    /// Begin from an accepted frame, clipped; false leaves a white frame to repaint.
    fn begin_damage(
        &mut self,
        width: f32,
        height: f32,
        scale: f32,
        previous: &Pixmap,
        rects: &[Rect4],
    ) -> bool {
        self.begin(width, height, scale);
        self.damage(previous, rects)
    }
    /// Fill a shape.
    fn fill(&mut self, shape: &Shape, color: [u8; 4], ts: Transform);
    /// Fill a shape with a gradient placed in its coordinates (LLP 1066).
    fn fill_gradient(&mut self, shape: &Shape, gradient: &gradient::GradientPaint, ts: Transform);
    /// CSS `backdrop-filter` (LLP 1053.000 D2): apply the ordered functions
    /// to what is painted under `shape`, then replace it under the current clip.
    fn backdrop_filter(&mut self, _shape: &Shape, _filter: &BackdropFilter, _ts: Transform) {}
    /// One outer `box-shadow` by the backend's own blur: `shape` blurred by
    /// `sigma` in `color`, painted only outside `outer` (the border box).
    /// Whether it drew it; else the painter draws it as bands.
    fn blurred_shadow(
        &mut self,
        _shape: &Shape,
        _color: [u8; 4],
        _sigma: f32,
        _outer: &Shape,
        _ts: Transform,
    ) -> bool {
        false
    }
    /// Draw the glyphs `text` draws from here on through a Gaussian blur of
    /// `sigma` points, 0 none, where the backend blurs glyphs as it draws
    /// them (a numeric roll's, LLP 1077 D15); the rest draw them sharp.
    fn text_blur(&mut self, _sigma: f32) {}
    /// Fill one colour's share of a border (LLP 1053 G2): region even-odd,
    /// clipped by its side (non-zero) and optional ring (even-odd).
    fn fill_border(&mut self, part: &border::BorderFill, ts: Transform);
    /// Draw a picture scaled into `dst`, clipped to every shape in `clips`.
    /// An explicit tint replaces its RGB through the picture's alpha alone.
    fn image(
        &mut self,
        image: &Arc<Bitmap>,
        dst: Rect4,
        clips: &[Shape],
        ts: Transform,
        tint: Option<[u8; 4]>,
    );
    /// An SVG island's pixels over `dst` in `ts`'s space, blended by `mode`.
    fn island_image(&mut self, _pixels: Arc<Pixmap>, _dst: Rect4, _ts: Transform, _mode: u8) {}
    /// Composite a rendered canvas child (not a decoded image asset).
    fn surface_image(&mut self, _pixels: Arc<Pixmap>, _dst: Rect4) {}
    /// Composite a 2D canvas's bitmap into `dst`, clipped (LLP 1056 D7).
    fn canvas(&mut self, _pixels: &Arc<Pixmap>, _dst: Rect4, _clips: &[Shape], _ts: Transform) {}
    /// Split an untransformed full-window canvas from the UI, if supported.
    /// A refusal leaves the painter unchanged.
    fn split_window_canvas(&mut self, _dst: Rect4, _clips: &[Shape], _ts: Transform) -> bool {
        false
    }
    /// Reuse storage released by the previous window picture.
    fn recycle_window_layers(
        &mut self,
        _below: Pixmap,
        _above: Option<Pixmap>,
        _rows: Option<(u32, u32)>,
    ) {
    }
    /// UI painted below the last successfully split window canvas.
    fn take_window_underlay(&mut self) -> Option<WindowLayer> {
        None
    }
    /// Device rows that may contain nontransparent overlay pixels after finish.
    /// `None` means unknown; an empty interval means a transparent overlay.
    fn window_overlay_rows(&self) -> Option<(u32, u32)> {
        None
    }
    /// A platform element (`paint/native.rs`) in `shape`; `props` is JSON.
    fn native(
        &mut self,
        _id: ViewId,
        _kind: NativeKind,
        _shape: &Shape,
        _ts: Transform,
        _props: &str,
        _direct: bool,
    ) {
    }
    /// Paint a paragraph with its top-left at `origin`.
    fn text(
        &mut self,
        text: &mut TextEngine,
        paragraph: &Paragraph,
        palette: &[RunPaint],
        origin: (f32, f32),
        ts: Transform,
    );
    /// Paint one SVG shape in `ts`'s space (LLP 1055 D4).
    fn svg_path(&mut self, _shape: &SvgPaint<'_>, _ts: Transform) {}
    /// Clip to an SVG `clipPath` in `ts`'s space until as many pops as this
    /// returns (LLP 1055.000 D10): the union of its shapes, intersected with
    /// its own clip and with whatever clip is in force.
    fn push_svg_clip(&mut self, _clip: &exact_kernel::svg::scene::Clip, _ts: Transform) -> usize {
        0
    }
    /// Clip everything until the matching pop to a shape.
    fn push_clip(&mut self, shape: &Shape, ts: Transform);
    /// Push the kernel's validated CSS path in border-box coordinates.
    /// Returns whether a clip was pushed (recording/custom backends may opt out).
    fn push_css_clip(&mut self, _path: &exact_kernel::clip::ClipPath, _ts: Transform) -> bool {
        false
    }
    /// End a clip.
    fn pop_clip(&mut self);
    /// Composite everything until the matching pop at an opacity.
    fn push_opacity(&mut self, alpha: f32);
    /// CSS `mask-image` (LLP 1077 D2): what paints until [`Backend::pop_mask`]
    /// is one group, clipped to `shape`, the border box.
    fn push_mask(&mut self, _shape: &Shape, _ts: Transform) {
        self.push_opacity(1.0);
    }
    /// The group, kept where `mask` (a gradient, or one colour) is opaque
    /// over `shape`, the border box, and nowhere outside it.
    fn pop_mask(&mut self, _shape: &Shape, _mask: &Result<GradientPaint, [u8; 4]>, _ts: Transform) {
        self.pop_opacity();
    }
    /// End an opacity layer.
    fn pop_opacity(&mut self);
    /// The pointer arrow at a point.
    fn pointer(&mut self, x: f32, y: f32);
    /// The frame's pixels.
    fn finish(&mut self) -> Result<Pixmap, String>;
    /// Complete a paint for its consumer. `Pixels` requires an image; `Window`
    /// permits `None` only after successful presentation. `Geometry` may omit
    /// pixels, but must not present. CPU backends can keep their ordinary path.
    fn finish_for(&mut self, _target: FrameTarget) -> Result<Option<Pixmap>, String> {
        self.finish().map(Some)
    }
    /// Whether this backend keeps rows' recordings ([`rows`]).
    fn rows(&self) -> bool {
        false
    }
    /// Whether recordings can be made at the local origin. Device-rasterized
    /// text needs the viewport origin to preserve its hinting phase.
    fn row_local(&self) -> bool {
        true
    }
    /// Whether a retained recording remains usable at this viewport origin.
    fn row_reusable(&self, _id: u32, _origin: (f32, f32)) -> bool {
        true
    }
    /// Record row `id`, whose ops are relative to `origin` (viewport points;
    /// zero when the row is walked in its own coordinates); the row was
    /// `previous` until something in it changed.
    fn row_begin(&mut self, _id: u32, _origin: (f32, f32), _previous: Option<u32>) {}
    /// The row ends; `bounds` (relative to its origin) is what it covers.
    /// The row's id: `previous` when it recorded the same, else the new one.
    fn row_end(&mut self, _bounds: Rect4) -> u32 {
        0
    }
    /// Whether drawing within `bounds` would be clipped away entirely.
    fn row_culled(&self, _bounds: Rect4) -> bool {
        false
    }
    /// Draw kept row `id` with its top-left at `origin`, covering `bounds`.
    fn row_draw(&mut self, _id: u32, _origin: (f32, f32), _bounds: Rect4) {}
    /// Kept row `id` is not drawn again.
    fn row_free(&mut self, _id: u32) {}
    /// The rows of scroller `id`, painted at its `scroll` offset, follow
    /// until [`Backend::group_end`].
    fn group_begin(&mut self, _id: ViewId, _scroll: (f32, f32)) {}
    /// The scroller's rows end.
    fn group_end(&mut self) {}
    /// Image node `id`'s picture (or none, not yet decoded) is drawn until
    /// [`Backend::slot_end`]: a backend that keeps rows may keep it apart, so
    /// a picture arriving changes the slot and not the row.
    fn slot_begin(&mut self, _id: ViewId) {}
    /// The picture's drawing ends.
    fn slot_end(&mut self) {}
    /// Begin image node `id`'s slot again, outside its kept row `row`, as
    /// that row's recording drew it (the same calls follow, then
    /// [`Backend::slot_end`]): a picture arrived and nothing else in the row
    /// changed. False: this backend cannot, and the row is recorded again.
    fn slot_again(&mut self, _row: u32, _id: ViewId) -> bool {
        false
    }
    /// The layer just begun is one its reader records again in each frame's
    /// colour (a `color` or `background-color` transition): it keeps the
    /// drawing.
    fn layer_recolored(&mut self) {}
    /// A node its reader animates (`crate::host::lower`): what follows, to
    /// [`Backend::layer_end`], is its layer `key`, moved and faded about
    /// `pivot` in `ts`'s space from `base` (translate x, y, scale, rotate,
    /// opacity, a circle's radius, path units per dash offset unit). False:
    /// drawn where it is.
    fn layer_begin(
        &mut self,
        _key: u64,
        _ts: Transform,
        _pivot: (f32, f32),
        _base: [f32; 7],
    ) -> bool {
        false
    }
    /// The layer ends.
    fn layer_end(&mut self) {}
    /// The last frame's (encode + render, readback) milliseconds, on a
    /// backend that has them.
    fn last_frame_ms(&self) -> Option<(f64, f64)> {
        None
    }
}

impl super::Painter {
    #[cfg(windows)]
    pub(crate) fn collect_retired_devices(&mut self) -> usize {
        self.backend.collect_retired_devices()
    }
    #[cfg(windows)]
    pub(crate) fn presentation_retry_delay(&self) -> Option<std::time::Duration> {
        self.backend.presentation_retry_delay()
    }
    #[cfg(windows)]
    pub(crate) fn device_lost(&self) -> bool {
        self.backend.device_lost()
    }
    #[cfg(windows)]
    pub(crate) fn displayed_process_ms(&self) -> Result<Option<f64>, String> {
        self.backend.displayed_process_ms()
    }
    #[cfg(windows)]
    pub(crate) fn attach_window(
        &mut self,
        window: exact_ink::device::d3d11::Window,
        size: [u32; 2],
    ) -> Result<bool, String> {
        self.backend.attach_window(window, size)
    }

    /// Release idle renderer storage while preserving kept rows.
    pub fn trim(&mut self) {
        self.backend.trim();
    }
}
