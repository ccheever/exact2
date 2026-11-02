//! Desktop top-layer surfaces share the page's renderer and input dispatch.
use super::*;

#[derive(Default)]
pub(super) struct State {
    pub enabled: bool,
    pressed: bool,
    keyboard: bool,
    pixels: Option<((u32, u32, u32), Arc<Pixmap>)>,
    shadow: Option<crate::paint::DialogShadow>,
}

impl<D: DataSource> Presenter<D> {
    pub(in crate::presenter) fn overlay_input(&mut self, keyboard: bool) {
        if !cfg!(target_os = "windows")
            || !self.top.overlay.enabled
            || self.top.overlay.keyboard == keyboard
        {
            return;
        }
        self.top.overlay.keyboard = keyboard;
        if let Some(open) = self
            .top
            .open
            .as_mut()
            .filter(|o| o.kind == SurfaceKind::Alert)
        {
            open.revision += 1;
            self.dirty = true;
            self.executor.notify();
        }
    }

    pub(in crate::presenter) fn overlay_pointer_cancel(&mut self) {
        if !self.top.overlay.enabled {
            return;
        }
        self.top.overlay.pressed = false;
        let Some(open) = self.top.open.as_mut() else {
            return;
        };
        let pressed = open.pressed.take().is_some();
        let hovered = open.hovered.take().is_some();
        let scrollbar = open.layout.cancel_menu_pointer();
        if pressed || hovered || scrollbar {
            open.revision += 1;
            self.dirty = true;
            self.executor.notify();
        }
    }

    pub(in crate::presenter) fn overlay_wheel(&mut self, x: f32, y: f32, delta: f32) -> bool {
        if !self.top.overlay.enabled {
            return false;
        }
        let Some(open) = self.top.open.as_ref() else {
            return false;
        };
        let [left, top, ..] = open.frame.map(|v| v as f32 / open.scale);
        if open.layout.content_contains(x - left, y - top) {
            self.overlay_scroll(delta);
        }
        true
    }

    pub(in crate::presenter) fn overlay_scroll(&mut self, delta: f32) -> bool {
        if !self.top.overlay.enabled {
            return false;
        }
        let Some(open) = self.top.open.as_mut() else {
            return false;
        };
        if open.layout.scroll_content(delta) {
            open.hovered = None;
            open.pressed = None;
            open.revision += 1;
            self.dirty = true;
            self.executor.notify();
        }
        true
    }

    pub(in crate::presenter) fn overlay_scroll_key(&mut self, name: &str) -> bool {
        if !self.top.overlay.enabled {
            return false;
        }
        let Some(open) = self
            .top
            .open
            .as_ref()
            .filter(|o| o.kind == SurfaceKind::Alert)
        else {
            return false;
        };
        let delta = match name {
            "PageDown" => open.layout.size.1 / 2.,
            "PageUp" => -open.layout.size.1 / 2.,
            "Home" => -f32::MAX,
            "End" => f32::MAX,
            _ => return false,
        };
        self.overlay_scroll(delta)
    }

    /// Present menus and confirmations over the page in the desktop window.
    pub fn serve_overlays(&mut self) {
        self.top.overlay.enabled = true;
        self.host.top_layer(true);
    }

    pub(in crate::presenter) fn prepare_overlay(&mut self) {
        if !self.top.overlay.enabled {
            return;
        }
        let Some(open) = self.top.open.as_ref() else {
            self.top.overlay.pixels = None;
            self.top.overlay.shadow = None;
            self.brush.set_top_surface(None);
            return;
        };
        let stamp = (open.serial, open.revision, self.brush.scale.to_bits());
        if self.top.overlay.pixels.as_ref().map(|p| p.0) != Some(stamp) {
            let mut raster = Raster::new();
            raster.page([0; 4]);
            raster.begin(open.layout.size.0, open.layout.size.1, self.brush.scale);
            let feedback = look::Feedback {
                pressed: open.pressed,
                focused: open.focused.filter(|_| {
                    !cfg!(target_os = "windows")
                        || open.kind != SurfaceKind::Alert
                        || self.top.overlay.keyboard
                }),
                hovered: open.hovered,
            };
            open.layout
                .paint(&mut raster, &mut self.text.borrow_mut(), feedback);
            match raster.finish() {
                Ok(pixels) => self.top.overlay.pixels = Some((stamp, Arc::new(pixels))),
                Err(error) => {
                    self.host.log(error);
                    return;
                }
            }
        }
        let [x, y, w, h] = open.frame.map(|v| v as f32 / open.scale);
        let shadow = &mut self.top.overlay.shadow;
        if cfg!(target_os = "windows")
            && open.kind == SurfaceKind::Alert
            && open.inputs.contrast.is_none()
        {
            if !shadow
                .as_ref()
                .is_some_and(|s| s.matches((w, h), self.brush.scale, open.inputs.dark))
            {
                *shadow =
                    crate::paint::DialogShadow::new((w, h), self.brush.scale, open.inputs.dark);
            }
        } else {
            *shadow = None;
        }
        self.brush.set_top_surface(Some(crate::paint::TopSurface {
            rect: (x, y, w, h),
            pixels: self.top.overlay.pixels.as_ref().unwrap().1.clone(),
            modal: open.kind == SurfaceKind::Alert,
            scrim: open.inputs.contrast.map_or([0, 0, 0, 80], |p| {
                [p.window[0], p.window[1], p.window[2], 204]
            }),
            shadow: shadow
                .as_ref()
                .map(|s| (s.pixels.clone(), s.rect((x, y, w, h)))),
        }));
    }

    /// Capture the whole click, including the release after an outside dismissal.
    pub(in crate::presenter) fn overlay_pointer(
        &mut self,
        phase: i32,
        x: f32,
        y: f32,
        now: f64,
    ) -> bool {
        if !self.top.overlay.enabled {
            return false;
        }
        if phase == 0 {
            self.overlay_input(false);
        }
        self.sync_surface();
        let Some(open) = self.top.open.as_ref() else {
            return phase == 1 && std::mem::take(&mut self.top.overlay.pressed);
        };
        let [left, top, width, height] = open.frame.map(|v| v as f32 / open.scale);
        let (x, y) = (x - left, y - top);
        let inside = x >= 0. && y >= 0. && x < width && y < height;
        let before = (open.serial, open.revision);
        let open = self.top.open.as_mut().unwrap();
        let (scrollbar, changed) = open.layout.menu_pointer(phase, x, y);
        if changed {
            open.revision += 1;
            self.dirty = true;
        }
        if scrollbar {
            open.hovered = None;
            open.pressed = None;
            if phase == 0 || phase == 1 {
                self.top.overlay.pressed = phase == 0;
            }
            return true;
        }
        let at = inside
            .then(|| open.layout.target_at(x, y))
            .flatten()
            .filter(|i| open.layout.targets[*i].enabled);
        let open = self.top.open.as_mut().unwrap();
        if open.hovered != at {
            open.hovered = at;
            open.revision += 1;
        }
        match phase {
            0 => {
                self.top.overlay.pressed = true;
                if inside {
                    self.surface_touch(0, x, y, now);
                } else {
                    self.top_takes_pointer(now);
                }
            }
            1 => {
                if std::mem::take(&mut self.top.overlay.pressed) {
                    self.surface_touch(1, x, y, now);
                }
            }
            2 => {
                let open = self.top.open.as_mut().unwrap();
                if open.kind != SurfaceKind::Alert && open.focused != at {
                    open.focused = at;
                    open.revision += 1;
                }
                if self.top.overlay.pressed {
                    self.surface_touch(2, x, y, now);
                }
            }
            _ => {}
        }
        if self.top.open.as_ref().map(|o| (o.serial, o.revision)) != Some(before) {
            self.dirty = true;
        }
        true
    }
}
