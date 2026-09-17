//! Selected native content, successful-paint publication and autonomous wake.
use super::*;
impl<D: DataSource> Presenter<D> {
    /// Explicit opt-in, before the first layout, for a native consumer.
    #[allow(clippy::too_many_arguments)]
    pub fn boot_with_content_region(
        plan: &[u8],
        data: D,
        viewport: (f32, f32),
        scale: f32,
        assets: PathBuf,
        choice: PainterChoice,
        region: crate::content_region::ContentRegionRegistration,
    ) -> Result<(Self, Option<String>), HostError> {
        Self::boot_with_assets(
            plan,
            data,
            viewport,
            scale,
            Assets::embedded(assets),
            choice,
            None,
            "/",
            Some(region),
        )
    }
    /// Region completion readiness. No timer or input event is needed to resume.
    pub fn content_region_fd(&self) -> Option<std::os::unix::io::RawFd> {
        self.host.content_region_fd()
    }
    pub(super) fn poll_content_region(&mut self) -> Option<String> {
        if let Some(error) = self.host.content_region_paint_scale(self.brush.scale) {
            return Some(error);
        }
        match self.host.poll_content_region() {
            Ok(true) => {
                self.dirty = true;
                self.queue_collections();
                None
            }
            Ok(false) => None,
            Err(e) => Some(e),
        }
    }
    /// Paint a frame and publish its pixels, hits and native source together.
    pub fn frame(&mut self) -> Pixmap {
        if let Some(error) = self.poll_content_region() {
            self.host.log(error);
        }
        if let Some(error) = self.refine_collections() {
            self.host.log(error);
        }
        let roots = self.host.roots();
        let host = &self.host;
        let presented = |id: ViewId| host.presented(id);
        let scene = Scene {
            kernel: host.kernel(),
            hidden: &|id| host.route_visibility(id).0,
            roots: &roots,
            presented: &presented,
            scroll: &self.scroll,
            page: self.page,
            images: &self.images.bitmaps,
            focus: self.focus,
            pointer: self.pointer,
        };
        let region = host.content_region();
        let feedback_before = region.is_some_and(|r| r.publication_painted());
        let paint = |brush: &mut Painter| match region {
            Some(region) => brush.paint_region(&scene, self.viewport, region),
            None => brush.paint(&scene, self.viewport),
        };
        let mut painted = paint(&mut self.brush);
        if let Err(e) = &painted {
            if self.choice == PainterChoice::Auto && self.brush.backend() == "gpu" {
                eprintln!("exact: paint: {e}; painting on the CPU from here");
                self.brush.replace_backend(Box::new(Raster::new()));
                self.painter = cpu_info();
                painted = paint(&mut self.brush);
            }
        }
        self.last_frame_succeeded = painted.is_ok();
        let (pixmap, boxes) = match painted {
            Ok(Frame { pixmap, boxes }) => {
                if region.is_some() {
                    // One bounded-to-viewport retained surface, separate from
                    // glyph/font/image ledgers. No copy on ordinary opt-out.
                    self.last_region_frame = Some(pixmap.clone());
                    self.last_region_scale = Some(self.brush.scale.to_bits());
                    if let Some(e) = self.host.content_region_painted(&self.brush) {
                        self.host.log(e);
                    }
                }
                (pixmap, boxes)
            }
            Err(e) => {
                eprintln!("exact: paint: {e}");
                if let Some(old) = &self.last_region_frame {
                    // Preserve old pixels without stretching or stale outside
                    // margins in the DRM carrier. New viewport area is opaque;
                    // old source and hit positions keep their original origin.
                    (
                        retained_surface(old, self.viewport, self.brush.scale),
                        if self.last_region_scale == Some(self.brush.scale.to_bits()) {
                            std::mem::take(&mut self.boxes)
                        } else {
                            Vec::new()
                        },
                    )
                } else {
                    let w = ((self.viewport.0 * self.brush.scale).round() as u32).max(1);
                    let h = ((self.viewport.1 * self.brush.scale).round() as u32).max(1);
                    let mut blank = Pixmap::new(w, h).expect("a viewport has pixels");
                    blank.fill(tiny_skia::Color::WHITE);
                    (blank, std::mem::take(&mut self.boxes))
                }
            }
        };
        self.boxes = boxes;
        if !feedback_before
            && self.last_frame_succeeded
            && self
                .host
                .content_region()
                .is_some_and(|r| r.publication_painted())
        {
            self.queue_collections();
        }
        self.dirty = self.collection.pending();
        if let Some(e) = self.sync_images() {
            self.host.log(e);
        }
        pixmap
    }
}

fn retained_surface(old: &Pixmap, viewport: (f32, f32), scale: f32) -> Pixmap {
    let width = ((viewport.0 * scale).round() as u32).max(1);
    let height = ((viewport.1 * scale).round() as u32).max(1);
    if (width, height) == (old.width(), old.height()) {
        return old.clone();
    }
    let mut next = Pixmap::new(width, height).expect("a viewport has pixels");
    next.fill(tiny_skia::Color::WHITE);
    let copied = old.width().min(width) as usize * 4;
    for y in 0..old.height().min(height) as usize {
        next.data_mut()[y * width as usize * 4..][..copied]
            .copy_from_slice(&old.data()[y * old.width() as usize * 4..][..copied]);
    }
    next
}
