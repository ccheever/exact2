//! Paint motion on the Linux host (LLP 1062): the engine samples colours and
//! the shadow like any property; the painter paints a presented value over
//! its row while the two differ. An animating `color` also reaches the views
//! and inline runs that inherit it, as a browser's inheriting element shows
//! it, and each `currentcolor` border side among them. An SVG element's
//! inherited `fill` and `stroke` reach its descendants through the scene's
//! resolver (LLP 1055.000 D15).

use super::Host;
use exact_kernel::motion::{layout_presented, motion_node, node_key};
use exact_kernel::{CommitReceipt, StyleId, ViewId};
use exact_motion::{Presentation, Property, Value};
use exact_runner::DataSource;

impl<D: DataSource> Host<D> {
    /// A path's `d` while a transition moves it (LLP 1055.000 D15); `None`
    /// paints the attribute.
    pub fn presented_path(&self, id: ViewId) -> Option<exact_motion::PathValue> {
        let key = self.runner.kernel().node(id)?.key;
        self.engine.presented_path(motion_node(key))
    }

    /// Adopt a commit's paint, after its `motion_sync` set the rows.
    pub(super) fn sync_paint(&mut self, receipt: &CommitReceipt) {
        let _roles = self.runner.bind_roles(); // Its own (LLP 1121 stage 5).
        let retired = self
            .paint
            .sync(self.runner.kernel(), receipt, &mut self.engine);
        self.retire_paint(retired);
    }

    /// Adopt the whole tree's paint at boot.
    pub(super) fn boot_paint(&mut self) {
        let _roles = self.runner.bind_roles(); // Its own (LLP 1121 stage 5).
        self.paint.adopt(
            self.runner.kernel(),
            self.keys.keys().copied(),
            &mut self.engine,
        );
    }

    fn retire_paint(&mut self, retired: Vec<(u64, Property)>) {
        for (node, property) in retired {
            let views = self.inheritors_of(node, property);
            let own = self.keys.get(&node_key(node)).copied();
            for view in own.into_iter().chain(views) {
                match property {
                    Property::Color => self.paint_color(view, None),
                    p => self.paint_over(view, p, None),
                };
            }
        }
    }

    /// Report the session's appearance: first quietly, then transitioning
    /// `light-dark()` targets under their rows (LLP 1062 D4).
    pub fn set_scheme(&mut self, dark: bool) {
        let _roles = self.runner.bind_roles(); // Its own (LLP 1121 stage 5).
        if let Some(retired) = self.paint.set_scheme(
            self.runner.kernel(),
            &mut self.engine,
            dark,
            self.now_ms / 1000.0,
        ) {
            self.flow_damage.repaint();
            self.retire_paint(retired);
            self.present();
        }
    }

    /// The platform's colours resolve differently (the device's shell
    /// reported its palette, LLP 1115.001 D4): every paint owner re-targets
    /// under its appearance, as the Apple host's does (LLP 1095 D6).
    /// `first`: the first report, a correction in place without motion.
    pub(crate) fn colors_changed(&mut self, first: bool) {
        let _roles = self.runner.bind_roles(); // Its own (LLP 1121 stage 5).
        if let Some(retired) = self.paint.colors_changed(
            self.runner.kernel(),
            &mut self.engine,
            self.now_ms / 1000.0,
            first,
        ) {
            self.retire_paint(retired);
            self.present();
        }
        self.flow_damage.repaint();
    }

    /// Report a view's own appearance, following the same first-report
    /// correction and later transitions as the session's.
    pub fn set_view_scheme(&mut self, view: ViewId, dark: bool) {
        let _roles = self.runner.bind_roles(); // Its own (LLP 1121 stage 5).
        let Some(key) = self.runner.kernel().node(view).map(|n| n.key) else {
            return;
        };
        if let Some(retired) = self.paint.set_view_scheme(
            self.runner.kernel(),
            &mut self.engine,
            key,
            dark,
            self.now_ms / 1000.0,
        ) {
            self.flow_damage.repaint();
            self.retire_paint(retired);
            self.present();
        }
    }

    /// One paint presentation, over its row while it differs from the target.
    pub(super) fn present_paint(&mut self, p: Presentation) -> bool {
        let settled = self.engine.target(p.node, p.property) == Some(p.value);
        let value = (!settled).then_some(p.value);
        let mut views: Vec<ViewId> = self
            .keys
            .get(&node_key(p.node))
            .copied()
            .into_iter()
            .collect();
        let mut changed = false;
        if p.property != Property::Color {
            views.extend(self.inheritors_of(p.node, p.property));
            for view in views {
                // A settled `currentcolor` side paints the presented `color`.
                let follows = value.is_none()
                    && self.runner.kernel().node(view).is_some_and(|n| {
                        self.runner
                            .kernel()
                            .current_color_sides(n.key)
                            .contains(&p.property)
                    });
                let value = match follows {
                    true => self.presented(view).colors.value(Property::Color),
                    false => value,
                };
                changed |= self.paint_over(view, p.property, value);
            }
            return changed;
        }
        views.extend(self.inheritors_of(p.node, Property::Color));
        for view in views {
            changed |= self.paint_color(view, value);
        }
        changed
    }

    /// `color` on `view`, and on its `currentcolor` border sides; a side
    /// that has its own colour and no motion of its own shows its row.
    fn paint_color(&mut self, view: ViewId, value: Option<Value>) -> bool {
        let mut changed = self.paint_over(view, Property::Color, value);
        let kernel = self.runner.kernel();
        let Some(key) = kernel.node(view).map(|n| n.key) else {
            return changed;
        };
        let current = kernel.current_color_sides(key);
        for side in [
            Property::BorderTopColor,
            Property::BorderRightColor,
            Property::BorderBottomColor,
            Property::BorderLeftColor,
        ] {
            // A side moving under its own row shows its own value.
            if self.engine.is_active(motion_node(key), side) {
                continue;
            }
            if current.contains(&side) {
                changed |= self.paint_over(view, side, value);
            } else if !self.paint.owns(motion_node(key), side) {
                changed |= self.paint_over(view, side, None);
            }
        }
        changed
    }

    fn paint_over(&mut self, view: ViewId, property: Property, value: Option<Value>) -> bool {
        if self.presented(view).colors.value(property) == value {
            return false;
        }
        if let Some(n) = self.runner.kernel().node(view) {
            self.row_dirty.node(n.key);
        }
        let base = self.unpresented(view);
        let entry = self.presented.entry(view).or_insert(base);
        entry.colors.set(property, value);
        true
    }

    /// The views below `node` whose `color` is `node`'s: no own row on the
    /// way, no platform start that stops it (a field's), and no paint
    /// motion of their own.
    fn inheritors_of(&self, node: u64, property: Property) -> Vec<ViewId> {
        let row = match property {
            Property::Color => StyleId::TextColor,
            _ => return Vec::new(),
        };
        let kernel = self.runner.kernel();
        let Some(source) = kernel.node_by_key(node_key(node)) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut stack = source.children();
        while let Some(id) = stack.pop() {
            let Some(child) = kernel.node(id) else {
                continue;
            };
            if child.style.mask.has(row)
                || child.starts(row)
                || self.paint.owns(motion_node(child.key), property)
            {
                continue;
            }
            out.push(id);
            stack.extend(child.children());
        }
        out
    }
    /// Every presentation value the engine changed, kept by node.
    pub(super) fn present(&mut self) -> bool {
        // Holds and commits both reconcile reader tracks before publishing.
        let epoch = self.lowered_epoch;
        self.play_transitions();
        let mut changed = self.lowered_epoch != epoch;
        for p in self.engine.frame() {
            let key = node_key(p.node);
            let Some(view) = self.keys.get(&key).copied() else {
                continue;
            };
            if p.property == Property::Height || self.presents_nothing(view, &p) {
                continue;
            }
            // A path's `d` is read from the engine where it is painted.
            if p.property == Property::D {
                changed = true;
                self.row_dirty.node(key);
                continue;
            }
            if Property::PAINT.contains(&p.property) {
                changed |= self.present_paint(p);
                continue;
            }
            changed = true;
            self.row_dirty.node(key);
            let base = self.unpresented(view);
            let entry = self.presented.entry(view).or_insert(base);
            match p.property {
                Property::Translate => {
                    entry.translate = (p.value.x as f32, p.value.y as f32);
                    entry.translate_percent = (p.value.z as f32, p.value.w as f32);
                }
                Property::Layout => {
                    entry.layout = layout_presented(&self.engine, p.node, p.value).map(|v| v as f32)
                }
                Property::Scale => entry.scale = p.value.x as f32,
                Property::Rotate => entry.rotate = p.value.x as f32,
                Property::Opacity => entry.opacity = p.value.x as f32,
                Property::R => entry.svg[0] = Some(p.value.x as f32),
                Property::StrokeDashoffset => entry.svg[1] = Some(p.value.x as f32),
                // @ref LLP 1055.000 D15 — geometry, in `Presented::svg`'s order.
                Property::Cx => entry.svg[2] = Some(p.value.x as f32),
                Property::Cy => entry.svg[3] = Some(p.value.x as f32),
                Property::X => entry.svg[4] = Some(p.value.x as f32),
                Property::Y => entry.svg[5] = Some(p.value.x as f32),
                Property::Rx => entry.svg[6] = Some(p.value.x as f32),
                Property::Ry => entry.svg[7] = Some(p.value.x as f32),
                _ => unreachable!("height is projected through layout; paint is above"),
            }
        }
        changed
    }

    /// A step sampler can restate its value on unrelated timer turns. Such a
    /// sample changes neither the kept row nor the next painted frame.
    fn presents_nothing(&self, view: ViewId, p: &exact_motion::Presentation) -> bool {
        let s = self.presented(view);
        let v = p.value;
        match p.property {
            Property::Translate => {
                s.translate == (v.x as f32, v.y as f32)
                    && s.translate_percent == (v.z as f32, v.w as f32)
            }
            Property::Scale => s.scale == v.x as f32,
            Property::Rotate => s.rotate == v.x as f32,
            Property::Opacity => s.opacity == v.x as f32,
            Property::Layout => {
                s.layout == layout_presented(&self.engine, p.node, v).map(|v| v as f32)
            }
            Property::R => s.svg[0] == Some(v.x as f32),
            Property::StrokeDashoffset => s.svg[1] == Some(v.x as f32),
            Property::Cx => s.svg[2] == Some(v.x as f32),
            Property::Cy => s.svg[3] == Some(v.x as f32),
            Property::X => s.svg[4] == Some(v.x as f32),
            Property::Y => s.svg[5] == Some(v.x as f32),
            Property::Rx => s.svg[6] == Some(v.x as f32),
            Property::Ry => s.svg[7] == Some(v.x as f32),
            _ => false,
        }
    }
}
