//! Selected native content, successful-paint publication and autonomous wake.
use super::*;
use crate::paint::FrameTarget;
impl<D: DataSource> Presenter<D> {
    /// Reuse a presented CPU frame only after every other reader has released it.
    pub fn recycle_frame(&mut self, pixels: Arc<Pixmap>) {
        self.brush.recycle_frame(pixels);
    }
    /// The last frame's (paint, readback) milliseconds; CPU readback is zero.
    pub fn last_frame_ms(&self) -> Option<(f64, f64)> {
        self.brush.last_frame_ms()
    }
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
        let (mut presenter, error) = Self::boot_with_assets(
            crate::host::PlanBytes::Copied(plan),
            data,
            viewport,
            scale,
            Assets::embedded(assets),
            PainterBoot::selected(choice),
            None,
            "/",
            Some(region),
        )?;
        presenter.measure(); // LLP 1079 D1
        Ok((presenter, error))
    }
    /// Region completion readiness. No timer or input event is needed to resume.
    #[cfg(unix)]
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
    /// Paint a frame and publish its immutable pixels, hits and native source together.
    /// Text-cache eviction waits for the paint's end
    /// ([`crate::text::cache::deferring_eviction`]): a paint that misses walks
    /// the cache once, not once per paragraph.
    pub fn frame(&mut self) -> Arc<Pixmap> {
        self.frame_for(FrameTarget::Pixels)
            .expect("capture has pixels or a CPU fallback")
    }

    pub(crate) fn frame_for(&mut self, target: FrameTarget) -> Option<Arc<Pixmap>> {
        // Its session's roles, whatever else lives on the thread (LLP 1121 stage 5).
        let _roles = self.host.runner().bind_roles();
        crate::text::cache::deferring_eviction(|| {
            let frame = self.paint_frame(target);
            if self.last_frame_succeeded && self.reveal_field_cursor() {
                self.paint_frame(target)
            } else {
                frame
            }
        })
    }
    /// Whether the last paint produced a current picture rather than a fallback.
    pub fn last_frame_succeeded(&self) -> bool {
        self.last_frame_succeeded
    }
    fn paint_frame(&mut self, target: FrameTarget) -> Option<Arc<Pixmap>> {
        self.brush.composition = self.ime_paint();
        self.alert_sync();
        self.prepare_overlay();
        if self.brush.scrollbars.enabled {
            self.brush.scrollbars.limits = self.collection_scroll_limits();
            self.brush.scrollbars.pointer = self.hover_point;
        }
        self.brush.placements = self.surfaces.placements(&self.host);
        self.host
            .sync_canvases(self.brush.scale as f64, true, &self.assets);
        let previous = std::mem::replace(&mut self.brush.canvases, self.host.canvas_snapshots());
        self.brush.canvases.extend(self.surfaces.pixels(
            &mut self.host,
            self.brush.scale,
            previous,
            self.brush.window_canvas,
        ));
        // The hatches' overlays as published (LLP 1075.003.000.001 §2.2.1):
        // read here, at the start of the paint; the walk calls no hatch.
        self.brush.overlays = (self.hatches.overlays.iter())
            .map(|(id, shown)| (*id, shown.pixels.clone()))
            .collect();
        // The display carrier stages the paint's owners/boxes and publishes
        // them only on the matching flip. Headless/agent frames stay immediate.
        let deferred = self.display.submitting();
        if let Some(error) = self.poll_content_region() {
            self.host.log(error);
        }
        if let Some(error) = self.refine_collections() {
            self.host.log(error);
        }
        self.refresh_looks(None); // the focus's ring (LLP 1121 D8)
        self.restore_scroll_anchors();
        self.arm_scroll_anchors();
        let dirty = self.host.take_row_dirty();
        self.brush.numerals.now = self.host.now();
        self.brush.numerals.still = self.host.runner().viewport().preferences.reduced_motion;
        self.brush.rows_dirty(self.host.kernel(), dirty);
        let roots = self.host.roots();
        let collection_limits = if self.host.content_region().is_some() {
            self.collection_scroll_limits()
        } else {
            BTreeMap::new()
        };
        self.brush.flow_damage(
            &self.host,
            &self.boxes,
            self.viewport,
            self.page,
            &self.scroll,
            self.pointer,
            self.focus,
        );
        let model_scroll = deferred.then(|| self.collection_paint_scroll()).flatten();
        let menu = self.menu_paint();
        let selection = self.focus.map(|id| self.field_selection(id));
        let host = &self.host;
        let presented = |id: ViewId| host.presented(id);
        let scene = Scene {
            kernel: host.kernel(),
            hidden: &|id| host.route_visibility(id).0,
            roots: &roots,
            presented: &presented,
            paths: &|id| host.presented_path(id),
            scroll: model_scroll.as_ref().unwrap_or(&self.scroll),
            page: if deferred {
                self.model_page()
            } else {
                self.page
            },
            images: &self.images.bitmaps,
            focus: self.focus,
            selection,
            caret_upstream: self.focus.is_some_and(|id| self.field_cursor(id).upstream),
            pointer: self.pointer,
            controls: &self.controls,
            chosen: &self.chosen,
            menu,
        };
        let region = host.content_region();
        let feedback_before = region.is_some_and(|r| r.publication_painted());
        // Capture only at a successful current CPU picture, before display_frame
        // restores A. Retained replay never obtains new B handlers/arguments.
        let mut handlers = None;
        let mut capture = |key, kind| {
            let node = host.kernel().node_by_key(key)?;
            let handlers = handlers.get_or_insert_with(|| host.runner().handlers());
            handlers
                .get(&node.id)
                .filter(|events| events.contains(&kind))?;
            Some(host.runner().capture_action_binding(key, kind))
        };
        let eligible = |key| host.retained_action_eligible(key);
        let motion = |key, picture: &std::rc::Rc<()>| {
            self.retained_motion
                .as_ref()
                .is_some_and(|permit| permit.allows(host, key, picture))
        };
        let mut actions = crate::paint::RegionActions {
            capture: &mut capture,
            eligible: &eligible,
            motion: &motion,
        };
        let mut paint = |brush: &mut Painter| match region {
            Some(region) => brush
                .paint_region(
                    &scene,
                    self.viewport,
                    region,
                    &collection_limits,
                    &mut actions,
                )
                .map(|frame| frame.map_pixels(Some)),
            None => brush.paint_for(&scene, self.viewport, target),
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
        let notes = self.brush.take_notes();
        self.last_frame_succeeded = painted.is_ok();
        let (pixmap, boxes) = match painted {
            Ok(Frame { pixmap, boxes }) => {
                if region.is_some() && !deferred {
                    // One bounded-to-viewport retained surface, separate from
                    // glyph/font/image ledgers. No copy on ordinary opt-out.
                    self.last_region_frame = pixmap.clone();
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
                        Some(retained_surface(old, self.viewport, self.brush.scale)),
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
                    (Some(Arc::new(blank)), std::mem::take(&mut self.boxes))
                }
            }
        };
        for key in self.brush.flow_failures().collect::<Vec<_>>() {
            if let Some(node) = self.host.kernel().node_by_key(key) {
                self.host.log(format!(
                    "wrap-flow: text #{} is incomplete and uses ordinary layout",
                    node.id
                ));
            }
        }
        for note in notes {
            self.host.log(note);
        }
        self.boxes = boxes;
        self.boxes_serial += 1;
        if self.last_frame_succeeded {
            self.ranges.shown.set(self.ranges.session());
        }
        self.publish_ranges();
        if self.last_frame_succeeded {
            self.host.flow_damage.clear();
        }
        if self.last_frame_succeeded && !deferred {
            if let Some(region) = self.host.content_region() {
                // Replay clamps against the selected picture BEFORE drawing.
                // Publish precisely those offsets only after backend success;
                // failed B must not shrink A's still-visible scroll state.
                for b in &self.boxes {
                    if let Some(offset) = b.scroll {
                        if self.host.kernel().node(b.id).is_some_and(|n| {
                            self.brush.region_scroll_bounds(region, n.key).is_some()
                        }) && (offset != (0., 0.) || self.scroll.contains_key(&b.id))
                        {
                            self.scroll.insert(b.id, offset);
                        }
                    }
                }
            }
        }
        if !deferred
            && !feedback_before
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

fn retained_surface(old: &Arc<Pixmap>, viewport: (f32, f32), scale: f32) -> Arc<Pixmap> {
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
    Arc::new(next)
}
