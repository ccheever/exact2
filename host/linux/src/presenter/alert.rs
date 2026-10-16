//! Desktop Neutral confirmations. Admission shares the windowed shape reader;
//! layout and action order remain the authored tree (LLP 1115.000 D13.4, D17).
use super::*;
use exact_kernel::{BlockAxis, HostBlock, HostCover, NodeKey, StyleMask};

pub(super) struct Open {
    key: NodeKey,
    restore: Option<NodeKey>,
    title: Option<(String, Rect4)>,
    measured: Option<Inputs>,
}
#[derive(PartialEq)]
struct Inputs {
    epoch: u64,
    env: exact_kernel::Env,
    frame: exact_kernel::Frame,
    viewport: (f32, f32),
    root_font: f32,
    scale: f32,
    fonts: usize,
    dark: bool,
}
impl<D: DataSource> Presenter<D> {
    pub(crate) fn neutral_alerts(&self) -> bool {
        crate::paint::look::neutral() && !self.has_windows() && !self.has_bars()
    }
    pub(super) fn alert_root(&self) -> Option<ViewId> {
        self.alert
            .as_ref()
            .and_then(|a| self.host.kernel().node_by_key(a.key))
            .map(|n| n.id)
    }
    pub(super) fn alert_allows(&self, id: ViewId) -> bool {
        let Some(open) = &self.alert else {
            return true;
        };
        let mut at = self.host.kernel().node(id);
        while let Some(node) = at {
            if node.key == open.key {
                return true;
            }
            at = node.parent.and_then(|p| self.host.kernel().node(p));
        }
        false
    }
    pub(super) fn alert_mode(&self, target: NodeKey) -> Option<bool> {
        self.alert
            .as_ref()
            .filter(|open| open.key == target)
            .map(|_| true)
    }
    fn alert_admitted(&self, id: ViewId) -> bool {
        let k = self.host.kernel();
        !self.host.route_visibility(id).1
            && top::shape::neutral_alert(k, id, &|id| {
                self.host
                    .runner()
                    .handlers_of(id)
                    .contains(&EventKind::Press)
            })
    }
    pub(super) fn alert_present(
        &mut self,
        target: Option<NodeKey>,
        name: &str,
        invoker: Option<ViewId>,
        now: f64,
    ) -> bool {
        let node = target.and_then(|key| self.host.kernel().node_by_key(key));
        let Some((id, key)) = node
            .map(|n| (n.id, n.key))
            .filter(|(id, _)| self.alert_admitted(*id))
        else {
            self.host.log(format!("showModal {name:?} refused: this host presents only alert-shaped alertdialog confirmations"));
            return false;
        };
        if self.alert.as_ref().is_some_and(|a| a.key == key) {
            return true;
        }
        if self.host.kernel().host_block(id).is_some()
            || self
                .host
                .kernel()
                .arena()
                .slot_of(id)
                .is_some_and(|s| self.host.kernel().arena().cover(s).is_some())
        {
            self.host
                .log("showModal refused: dialog already has a host container");
            return false;
        }
        self.alert_close(now);
        let restore = self
            .focus
            .or(invoker)
            .and_then(|id| self.host.kernel().node(id).map(|n| n.key));
        if let Err(e) = self.pointer_cancel(now) {
            self.host.log(e);
        }
        if let Some(e) = self.set_focus(None, now) {
            self.host.log(e);
        }
        // A blur handler can destroy or replace the dialog.
        if self.host.kernel().node_by_key(key).is_none() || !self.alert_admitted(id) {
            return false;
        }
        self.alert = Some(Open {
            key,
            restore,
            title: None,
            measured: None,
        });
        if let Err(e) = self.alert_layout() {
            self.host.log(e);
            self.alert_close(now);
            return false;
        }
        self.dirty = true;
        if let Some(e) = self.move_focus(false, now) {
            self.host.log(e);
        }
        self.back_changed();
        true
    }
    pub(super) fn alert_close(&mut self, now: f64) {
        let Some(open) = self.alert.take() else {
            return;
        };
        self.brush.alert = None;
        if let Some(node) = self.host.kernel().node_by_key(open.key) {
            let id = node.id;
            let k = self.host.runner_mut().kernel_mut();
            let _ = k.set_host_cover(id, None);
            let _ = k.set_host_block(id, None);
            if let Err(e) = self.host.layout() {
                self.host.log(e);
            }
        }
        if let Some(e) = self.set_focus(None, now) {
            self.host.log(e);
        }
        let restore = open
            .restore
            .and_then(|key| self.host.kernel().node_by_key(key))
            .map(|n| n.id)
            .filter(|id| self.focusable(*id) && !self.host.route_visibility(*id).1);
        if let Some(e) = self.set_focus(restore, now) {
            self.host.log(e);
        }
        self.dirty = true;
        self.back_changed();
    }
    pub(super) fn alert_sync(&mut self) {
        if self.alert.is_none() {
            return;
        }
        if self.alert_root().is_none_or(|id| !self.alert_admitted(id)) {
            self.alert_close(self.host.now());
            return;
        }
        if let Err(e) = self.alert_layout() {
            self.host.log(e);
            self.alert_close(self.host.now());
        }
    }
    fn alert_layout(&mut self) -> Result<(), String> {
        let Some(id) = self.alert_root() else {
            return Ok(());
        };
        if self.alert.as_ref().and_then(|a| a.measured.as_ref()) == self.alert_inputs(id).as_ref() {
            return Ok(());
        }
        // Desktop covers are otherwise owned only by the bars path, excluded
        // at admission. Neither the cover nor block writes authored rows.
        self.host
            .runner_mut()
            .kernel_mut()
            .set_host_block(
                id,
                Some(HostBlock {
                    width: BlockAxis::Fit(self.viewport.0),
                    height: BlockAxis::Fit(self.viewport.1),
                }),
            )
            .map_err(|e| format!("alert block: {e:?}"))?;
        self.host.layout()?;
        let k = self.host.kernel();
        let node = k.node(id).ok_or("alert removed during layout")?;
        let key = node.key;
        let label = node
            .props
            .str(PropId::AccessibilityLabel)
            .map(str::to_owned);
        let mut style = node.computed_style(StyleMask::INHERITED);
        let (left, _, right, _) = k.resolved_padding(key).unwrap_or_default();
        let gap = style.row_gap;
        let title = label.map(|label| {
            style.font_size = exact_kernel::style::metrics::length_in(
                exact_kernel::generated::metric::ALERT_TITLE,
                exact_kernel::style::metrics::Column::Neutral,
                style.font_size,
                k.root_font_size(),
            )
            .unwrap();
            style.font_weight = 600;
            let p = self.text.borrow_mut().paragraph(
                &crate::paint::text_spec(&style, &label),
                Some((node.frame.width - left - right).max(0.)),
            );
            (label, p)
        });
        let covered = title.as_ref().map_or(0., |(_, p)| p.height + gap);
        self.host
            .runner_mut()
            .kernel_mut()
            .set_host_cover(id, Some(HostCover::Edges([covered, 0., 0., 0.])))
            .map_err(|e| format!("alert title cover: {e:?}"))?;
        self.host.layout()?;
        let node = self
            .host
            .kernel()
            .node_by_key(key)
            .ok_or("alert removed during title layout")?;
        let f = node.frame;
        let origin = (
            (self.viewport.0 - f.width) / 2.,
            (self.viewport.1 - f.height) / 2.,
        );
        let offset = (f.x - origin.0, f.y - origin.1);
        // Resolved CSS padding excludes the title cover.
        let own_top = self
            .host
            .kernel()
            .resolved_padding(key)
            .unwrap_or_default()
            .1;
        let dark = self.host.presented(id).dark.unwrap_or(self.dark());
        let title = title.map(|(label, p)| {
            let rect = (origin.0 + left, origin.1 + own_top, p.width, p.height);
            self.alert.as_mut().unwrap().title = Some((label, rect));
            (
                p,
                rect,
                crate::text::RunPaint {
                    color: crate::paint::rgba(style.text_color.resolve(dark)),
                    source: id,
                },
            )
        });
        if title.is_none() {
            self.alert.as_mut().unwrap().title = None;
        }
        self.brush.alert = Some(crate::paint::AlertPicture { key, offset, title });
        self.alert.as_mut().unwrap().measured = self.alert_inputs(id);
        Ok(())
    }
    fn alert_inputs(&self, id: ViewId) -> Option<Inputs> {
        let k = self.host.kernel();
        Some(Inputs {
            epoch: k.epoch(),
            env: k.env(),
            frame: k.node(id)?.frame,
            viewport: self.viewport,
            root_font: k.root_font_size(),
            scale: self.brush.scale,
            fonts: self.host.runner().runtime_font_count(),
            dark: self.host.presented(id).dark.unwrap_or(self.dark()),
        })
    }
    pub(super) fn alert_key(&mut self, name: &str, now: f64) -> bool {
        let Some(_) = self.alert_root() else {
            return false;
        };
        let opening = self.alert.as_ref().unwrap().key;
        let focus = self
            .focus
            .and_then(|id| self.host.kernel().node(id).map(|n| n.key));
        let (error, prevented) = self.key_event(EventKind::Key, name, "", false, now);
        if let Some(error) = error {
            self.host.log(error);
        }
        if prevented
            || self.alert.as_ref().is_none_or(|a| a.key != opening)
            || name != "Escape"
                && self
                    .focus
                    .and_then(|id| self.host.kernel().node(id).map(|n| n.key))
                    != focus
        {
            return true;
        }
        match name {
            "Escape" => {
                if self
                    .alert_root()
                    .and_then(|id| self.host.kernel().node(id))
                    .is_some_and(|n| n.props.str(PropId::Closedby) != Some("none"))
                {
                    self.alert_close(now);
                }
            }
            "Tab" => {
                if let Some(e) = self.move_focus(self.modifiers().shift, now) {
                    self.host.log(e);
                }
            }
            "Enter" | " " => {
                if let Some(id) = self.focus.filter(|id| self.alert_allows(*id)) {
                    self.invoke(id, now, false);
                }
            }
            _ => {}
        }
        true
    }
    pub(super) fn alert_pointer(&mut self, x: f32, y: f32, now: f64) -> bool {
        if self.alert.is_none() || self.hit(x, y).is_some() {
            return false;
        }
        if self
            .alert_root()
            .and_then(|id| self.host.kernel().node(id))
            .is_some_and(|n| n.props.str(PropId::Closedby) == Some("any"))
        {
            self.alert_close(now);
        }
        true
    }
    pub(crate) fn alert_json(&self) -> serde_json::Value {
        let Some(id) = self.alert_root() else {
            return serde_json::Value::Null;
        };
        let title = self
            .alert
            .as_ref()
            .and_then(|a| a.title.as_ref())
            .map(|(text, rect)| serde_json::json!({"text":text,"rect":rect}));
        let rows: Vec<_> = self
            .boxes
            .iter()
            .filter(|b| self.alert_allows(b.id))
            .map(|b| serde_json::json!({"id":b.id,"rect":b.rect}))
            .collect();
        serde_json::json!({"kind":"alert", "node":id, "title":title, "rows":rows})
    }
}
