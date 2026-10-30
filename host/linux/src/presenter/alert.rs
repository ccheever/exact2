//! Authored desktop top-layer boxes, laid out and painted as ordinary subtrees.
//! Alerts keep their title prefix; dialogs add a scrim; popovers anchor to an invoker.
use super::*;
use exact_kernel::{BlockAxis, HostBlock, NodeKey};
use std::collections::BTreeSet;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Kind {
    Alert,
    Dialog,
    Popover,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Alert => "alert",
            Self::Dialog => "dialog",
            Self::Popover => "popover",
        }
    }
}
pub(super) struct Open {
    kind: Kind,
    modal: bool,
    invoker: Option<ViewId>,
    anchor: Option<Rect4>,
    reading_anchor: bool,
    point: Option<(f32, f32)>,
    serial: u32,
    key: NodeKey,
    pub(super) restore: Option<NodeKey>,
    /// A manual popover: no light dismiss, no close request, no focus back.
    manual: bool,
    /// A popover's untransformed placement in viewport points, as the
    /// runner's top layer reads it ([`Presenter::alert_popover_frame`]).
    placed: Option<[f64; 4]>,
    title: Option<(String, Rect4)>,
    measured: Option<Inputs>,
    placement: top::area::History,
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
    scroll: (f32, f32),
    anchor: Option<Rect4>,
}
impl<D: DataSource> Presenter<D> {
    fn alert_scroll(&self, id: ViewId) -> (f32, f32) {
        self.host
            .kernel()
            .node(id)
            .filter(|node| {
                let (x, y) = crate::paint::effective_overflow(node);
                x != exact_kernel::Overflow::Visible || y != exact_kernel::Overflow::Visible
            })
            .map_or((0., 0.), |_| self.scroll_of(id))
    }

    pub(crate) fn neutral_alerts(&self) -> bool {
        crate::paint::look::neutral() && !self.has_windows() && !self.has_bars()
    }
    pub(super) fn alert_root(&self) -> Option<ViewId> {
        self.alert
            .as_ref()
            .and_then(|a| self.host.kernel().node_by_key(a.key))
            .map(|n| n.id)
    }
    /// Whether the page under the open box takes input: not under a modal
    /// dialog or an alert, and under a popover, as HTML's page stays live.
    pub(crate) fn alert_allows(&self, id: ViewId) -> bool {
        !self.alert_inert() || self.alert_holds(id)
    }
    /// Whether the open box is modal, so the page under it is inert.
    pub(crate) fn alert_inert(&self) -> bool {
        self.alert.as_ref().is_some_and(|a| a.modal)
    }
    /// Whether the page is inert: under a modal box or a shape surface.
    pub(crate) fn page_inert(&self) -> bool {
        self.shape_surface_open() || self.alert_inert()
    }
    /// Whether `id` is the open box or inside it.
    pub(crate) fn alert_holds(&self, id: ViewId) -> bool {
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
            .map(|open| open.modal)
    }
    fn alert_kind(&self, id: ViewId, modal: bool) -> Option<Kind> {
        let k = self.host.kernel();
        let n = k.node(id)?;
        if n.props.bool(PropId::Inert) == Some(true)
            || n.parent.is_some_and(|p| self.host.route_visibility(p).1)
        {
            return None;
        }
        if !modal {
            return (n.props.str(PropId::Popover).is_some()
                && (!self.neutral_alerts()
                    || n.props.str(PropId::AccessibilityRole) != Some("menu")))
            .then_some(Kind::Popover);
        }
        if n.props.str(PropId::SemanticTag) != Some("dialog")
            || n.props.get(PropId::NavigationKey).is_some()
        {
            return None;
        }
        Some(
            if n.props.str(PropId::AccessibilityRole) == Some("alertdialog") {
                Kind::Alert
            } else {
                Kind::Dialog
            },
        )
    }
    pub(super) fn alert_present(
        &mut self,
        target: Option<NodeKey>,
        name: &str,
        invoker: Option<ViewId>,
        point: Option<(f32, f32)>,
        modal: bool,
        now: f64,
    ) -> bool {
        let node = target.and_then(|key| self.host.kernel().node_by_key(key));
        let Some((id, key, kind)) =
            node.and_then(|n| self.alert_kind(n.id, modal).map(|kind| (n.id, n.key, kind)))
        else {
            if !modal && self.neutral_alerts() {
                self.host.log(crate::navigation::POPOVER_UNSUPPORTED);
            } else {
                self.host.log(format!(
                    "showModal {name:?} refused: no available dialog has that id"
                ));
            }
            return false;
        };
        if self.alert.as_ref().is_some_and(|a| a.key == key) {
            return true;
        }
        if modal
            && (self.host.kernel().host_block(id).is_some()
                || self
                    .host
                    .kernel()
                    .arena()
                    .slot_of(id)
                    .is_some_and(|s| self.host.kernel().arena().cover(s).is_some()))
        {
            self.host
                .log("showModal refused: dialog already has a host container");
            return false;
        }
        let anchor = invoker.and_then(|id| self.box_of(id)).map(|b| b.rect);
        let restore = self.replace_surface(invoker, now);
        if let Err(e) = self.pointer_cancel(now) {
            self.host.log(e);
        }
        // HTML's popover leaves the focus where it was (its focusing steps
        // move it only to what autofocuses inside); a modal box takes it.
        if modal {
            if let Some(e) = self.set_focus(None, now) {
                self.host.log(e);
            }
        }
        // A surface presented by a synchronous blur handler owns focus and paint.
        if self.top_open() {
            return false;
        }
        // A blur handler can destroy or replace the dialog.
        if self.host.kernel().node_by_key(key).is_none() || self.alert_kind(id, modal) != Some(kind)
        {
            let restore = restore
                .and_then(|key| self.host.kernel().node_by_key(key))
                .map(|node| node.id)
                .filter(|id| self.focusable(*id) && self.focus_refusal(*id).is_none());
            if let Some(e) = self.set_focus(restore, now) {
                self.host.log(e);
            }
            return false;
        }
        let serial = self.next_surface_serial();
        let node = self.host.kernel().node(id);
        let manual = !modal && node.is_some_and(|n| n.props.str(PropId::Popover) == Some("manual"));
        // HTML runs the dialog focusing steps for a `dialog`, modal or a popover.
        let dialog =
            modal || node.is_some_and(|n| n.props.str(PropId::SemanticTag) == Some("dialog"));
        self.alert = Some(Open {
            kind,
            modal,
            invoker,
            anchor,
            reading_anchor: false,
            point,
            serial,
            key,
            restore,
            manual,
            placed: None,
            title: None,
            measured: None,
            placement: top::area::History::default(),
        });
        if !modal {
            if let Err(e) = self.host.show_authored_popover(Some(id)) {
                self.host.log(e);
                self.alert_close(now);
                return false;
            }
        }
        if let Err(e) = self.alert_layout() {
            self.host.log(e);
            self.alert_close(now);
            return false;
        }
        self.dirty = true;
        // An attached display still has the page's previous boxes. Initial
        // dialog focus follows its new layout before that picture is acknowledged.
        let boxed: BTreeSet<ViewId> = self
            .host
            .preorder()
            .into_iter()
            .filter(|id| self.alert_holds(*id) && !self.alert_message(*id))
            .filter(|id| {
                self.host
                    .kernel()
                    .node(*id)
                    .is_some_and(|n| n.frame.width > 0. && n.frame.height > 0.)
            })
            .collect();
        let autofocus = self.host.preorder().into_iter().find(|id| {
            boxed.contains(id)
                && self.focusable(*id)
                && self.focus_refusal(*id).is_none()
                && self
                    .host
                    .kernel()
                    .node(*id)
                    .is_some_and(|n| n.props.bool(PropId::Autofocus) == Some(true))
        });
        let error = match autofocus {
            Some(id) => self.set_focus(Some(id), now),
            None if dialog => self.move_focus_in(false, now, &boxed),
            None => None,
        };
        if let Some(e) = error {
            self.host.log(e);
        }
        // HTML's dialog focusing steps fall back to the dialog itself.
        if dialog
            && self.focus.is_none_or(|f| !self.alert_holds(f))
            && self.alert.as_ref().is_some_and(|a| a.serial == serial)
        {
            if let Some(e) = self.set_focus(Some(id), now) {
                self.host.log(e);
            }
        }
        self.queue_toggle(key, true);
        self.report_top_layer();
        self.back_changed();
        true
    }
    pub(super) fn alert_close(&mut self, now: f64) {
        // Taken before the cancel: its pointerup can remove, replace or reopen
        // the dialog, and the commit's sync must find nothing left to close.
        let Some(open) = self.alert.take() else {
            return;
        };
        self.queue_surface_closed(open.key, open.modal);
        if !open.modal {
            self.report_top_layer();
        }
        if let Err(e) = self.pointer_cancel(now) {
            self.host.log(e);
        }
        let opened = self.alert.as_ref().map(|new| new.key);
        if opened != Some(open.key) {
            if let Some(node) = self.host.kernel().node_by_key(open.key) {
                let id = node.id;
                let k = self.host.runner_mut().kernel_mut();
                if open.kind == Kind::Alert {
                    let _ = k.set_alert_prefix(id, false);
                }
                if open.modal {
                    let _ = k.set_host_block(id, None);
                }
                if let Err(e) = self.host.layout() {
                    self.host.log(e);
                }
            }
        }
        if !open.modal && self.alert.as_ref().is_none_or(|a| a.modal) {
            if let Err(e) = self.host.show_authored_popover(None) {
                self.host.log(e);
            }
        }
        // A new opening owns its own paint and focus.
        if opened.is_some() {
            return;
        }
        self.brush.alert = None;
        // A popover gives the focus back only from inside it, and a manual
        // one keeps nothing to give (HTML's hide popover steps); a modal box
        // always does.
        let inside = |p: &Self, id: ViewId| {
            let k = p.host.kernel();
            std::iter::successors(k.node(id), |n| n.parent.and_then(|q| k.node(q)))
                .any(|n| n.key == open.key)
        };
        if !open.modal && self.focus.is_none_or(|id| !inside(self, id)) {
            self.dirty = true;
            self.back_changed();
            return;
        }
        if let Some(e) = self.set_focus(None, now) {
            self.host.log(e);
        }
        if open.manual {
            self.dirty = true;
            self.back_changed();
            return;
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
    pub(super) fn alert_close_request(&mut self, now: f64) -> bool {
        let Some(root) = self.alert_root() else {
            return false;
        };
        let open = self.alert.as_ref().unwrap();
        // A manual popover hears no close request: Back and Escape are the page's.
        if open.manual {
            return false;
        }
        let (serial, modal) = (open.serial, open.modal);
        if self.host.kernel().node(root).is_some_and(|node| {
            if modal {
                node.props.str(PropId::Closedby) != Some("none")
            } else {
                node.props.str(PropId::Popover) != Some("manual")
            }
        }) && (!modal || !self.dialog_close_prevented(root, now))
            && self.alert.as_ref().is_some_and(|a| a.serial == serial)
        {
            self.alert_close(now);
        }
        true
    }
    pub(super) fn alert_sync(&mut self) {
        if self.alert.is_none() {
            return;
        }
        let open = self.alert.as_ref().unwrap();
        if self
            .alert_root()
            .is_none_or(|id| self.alert_kind(id, open.modal) != Some(open.kind))
        {
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
        let open = self.alert.as_ref().unwrap();
        if open.reading_anchor {
            return Ok(());
        }
        if let Some(invoker) = open.invoker.filter(|_| !open.modal && open.point.is_none()) {
            // box_of may paint fresh geometry, which synchronizes this surface again.
            self.alert.as_mut().unwrap().reading_anchor = true;
            // An attached display keeps its old hit boxes until acknowledgement.
            // During submission, place against the page being painted instead.
            if self.display.submitting() {
                let _ = self.frame_for(crate::paint::FrameTarget::Geometry);
            }
            let anchor = self.box_of(invoker).map(|b| b.rect);
            let Some(open) = self.alert.as_mut() else {
                return Ok(());
            };
            open.reading_anchor = false;
            open.anchor = anchor.or(open.anchor);
        }
        if self.alert.as_ref().and_then(|a| a.measured.as_ref()) == self.alert_inputs(id).as_ref() {
            return Ok(());
        }
        let (kind, modal) = self.alert.as_ref().map(|a| (a.kind, a.modal)).unwrap();
        if modal {
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
                .map_err(|e| format!("top-layer block: {e:?}"))?;
        }
        let label = self
            .host
            .kernel()
            .node(id)
            .filter(|_| kind == Kind::Alert)
            .and_then(|node| node.props.str(PropId::AccessibilityLabel))
            .map(str::to_owned);
        if kind == Kind::Alert {
            self.host
                .runner_mut()
                .kernel_mut()
                .set_alert_prefix(id, label.is_some())
                .map_err(|e| format!("alert title: {e:?}"))?;
        }
        self.host.layout()?;
        let k = self.host.kernel();
        let node = k.node(id).ok_or("alert removed during layout")?;
        let key = node.key;
        let style = node.alert_title_style();
        let prefix = k.alert_prefix_frame(id);
        let title = label.filter(|label| !label.is_empty()).map(|label| {
            let shown = style.hyphens.shown(style.text_transform.apply(&label, ""));
            let p = self.text.borrow_mut().paragraph(
                &crate::paint::text_spec(&style, &shown),
                Some(prefix.map_or(0., |frame| frame.width)),
            );
            (label, p)
        });
        let node = self
            .host
            .kernel()
            .node_by_key(key)
            .ok_or("alert removed during title layout")?;
        let f = node.frame;
        let open = self.alert.as_ref().unwrap();
        let origin = if let Some(point) = open.point {
            top::area::at_point(point, (f.width, f.height), self.viewport)
        } else if let Some(anchor) = open.anchor.filter(|_| !modal) {
            top::area::origin(
                &node.computed_style(exact_kernel::StyleMask::INHERITED),
                &k.env(),
                anchor,
                (f.width, f.height),
                self.viewport,
                &mut self.alert.as_mut().unwrap().placement,
            )
        } else {
            (
                (self.viewport.0 - f.width) / 2.,
                (self.viewport.1 - f.height) / 2.,
            )
        };
        let offset = (f.x - origin.0, f.y - origin.1);
        let dark = self.host.presented(id).dark.unwrap_or(self.dark());
        let scroll = self.alert_scroll(id);
        let title = title.map(|(label, p)| {
            let frame = prefix.expect("a nonempty label has a generated item");
            let rect = (frame.x, frame.y, frame.width, frame.height);
            let shown = (
                rect.0 - offset.0 - scroll.0,
                rect.1 - offset.1 - scroll.1,
                rect.2,
                rect.3,
            );
            self.alert.as_mut().unwrap().title = Some((label, shown));
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
        let scrim = modal.then(|| {
            self.colors.contrast.palette.map_or_else(
                || {
                    if self.desktop_overlays() {
                        [0, 0, 0, 77]
                    } else {
                        [
                            0,
                            0,
                            0,
                            (255.
                                * crate::paint::look::number(
                                    exact_kernel::generated::metric::SCRIM_ALPHA,
                                ))
                            .round() as u8,
                        ]
                    }
                },
                |p| [p.window[0], p.window[1], p.window[2], 204],
            )
        });
        self.brush.alert = Some(crate::paint::AlertPicture {
            key,
            offset,
            title,
            scrim,
        });
        let measured = self.alert_inputs(id);
        // The runner takes a block root's rectangle as its block, the root
        // sitting in it by its frame (margins included): the border box
        // then lands where it is drawn, before its own transform.
        let (bw, bh, fx, fy) = match self.host.kernel().host_block_size(id) {
            Some((w, h)) => (w, h, f.x, f.y),
            None => (f.width, f.height, 0., 0.),
        };
        let placed = [origin.0 - fx, origin.1 - fy, bw, bh].map(f64::from);
        let open = self.alert.as_mut().unwrap();
        open.measured = measured;
        // Reads see a popover where it is placed, before its own transform.
        let moved = !modal && open.placed.replace(placed) != Some(placed);
        self.dirty = true;
        if moved {
            self.report_top_layer();
        }
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
            scroll: self.alert_scroll(id),
            anchor: self.alert.as_ref()?.anchor,
        })
    }
    pub(crate) fn alert_message(&self, id: ViewId) -> bool {
        self.alert.as_ref().is_some_and(|a| a.kind == Kind::Alert)
            && self.alert_root().is_some_and(|root| {
                self.host.kernel().node(id).is_some_and(|node| {
                    node.parent == Some(root)
                        && node.props.str(PropId::AlertPart) == Some("message")
                })
            })
    }

    pub(crate) fn alert_message_overflows(&self, id: ViewId) -> bool {
        self.alert_message(id)
            && self.host.kernel().node(id).is_some_and(|node| {
                let bounds = self.brush.scroll_bounds(
                    self.host.kernel(),
                    self.host.content_region(),
                    &node,
                    None,
                );
                bounds.max.0 > 0. || bounds.max.1 > 0.
            })
    }

    fn alert_message_key(&mut self, name: &str) -> bool {
        let Some(id) = self.focus.filter(|id| self.alert_message(*id)).or_else(|| {
            (matches!(
                name,
                "ArrowLeft"
                    | "ArrowRight"
                    | "ArrowUp"
                    | "ArrowDown"
                    | "PageUp"
                    | "PageDown"
                    | "Home"
                    | "End"
            ) && !self.focus.is_some_and(|id| {
                self.host
                    .kernel()
                    .node(id)
                    .is_some_and(|n| n.node_type == NodeType::TextInput)
            }))
            .then(|| self.alert_root())
            .flatten()
        }) else {
            return false;
        };
        self.restore_scroll_anchors();
        let kernel = self.host.kernel();
        let Some(node) = kernel.node(id) else {
            return false;
        };
        let bounds = self
            .brush
            .scroll_bounds(kernel, self.host.content_region(), &node, None);
        let border = node
            .computed_style(exact_kernel::StyleMask::EMPTY)
            .border_widths_in(&kernel.env());
        // Chrome 151: arrows move 40 CSS px; Page and Space move 87.5% of
        // clientHeight (200 -> 175, 400 -> 350, 669 -> 585, 800 -> 700).
        let page = (node.frame.height - border[0] - border[2]).max(0.) * 0.875;
        let (dx, dy) = match name {
            "ArrowLeft" => (-40., 0.),
            "ArrowRight" => (40., 0.),
            "ArrowUp" => (0., -40.),
            "ArrowDown" => (0., 40.),
            "PageUp" => (0., -page),
            "PageDown" => (0., page),
            " " => (0., if self.modifiers().shift { -page } else { page }),
            "Home" => (0., -bounds.max.1),
            "End" => (0., bounds.max.1),
            _ => return false,
        };
        self.scroll_container_by(id, bounds, dx, dy);
        true
    }

    pub(super) fn alert_key(&mut self, name: &str, code: &str, repeat: bool, now: f64) -> bool {
        let Some(_) = self.alert_root() else {
            return false;
        };
        // Under a popover the page is live: only Escape is the popover's
        // (its close request); every other key takes the page's own path,
        // shortcuts and Back included.
        if !self.alert_inert() && (name != "Escape" || !self.alert_takes_back()) {
            return false;
        }
        let opening = self.alert.as_ref().unwrap().serial;
        let focus = self
            .focus
            .and_then(|id| self.host.kernel().node(id).map(|n| n.key));
        let (error, prevented) = self.key_event(EventKind::Key, name, code, repeat, now);
        if let Some(error) = error {
            self.host.log(error);
        }
        if prevented
            || self.alert.as_ref().is_none_or(|a| a.serial != opening)
            || name != "Escape"
                && self
                    .focus
                    .and_then(|id| self.host.kernel().node(id).map(|n| n.key))
                    != focus
        {
            return true;
        }
        if self.alert.as_ref().unwrap().kind != Kind::Alert {
            if name == "Escape" {
                self.alert_close_request(now);
            } else {
                self.key_default(name, code, repeat, now);
            }
            return true;
        }
        if !matches!(name, "Escape" | "Tab") {
            if let Some(id) = self.focus.filter(|id| {
                self.host
                    .kernel()
                    .node(*id)
                    .is_some_and(|n| n.node_type == NodeType::TextInput)
            }) {
                self.field_key(id, name, now);
                return true;
            }
        }
        if self.alert_message_key(name) {
            return true;
        }
        match name {
            "Escape" => {
                self.alert_close_request(now);
            }
            "Tab" => {
                if let Some(e) = self.move_focus(self.modifiers().shift, now) {
                    self.host.log(e);
                }
            }
            "Enter" | " " => {
                if let Some(id) = self
                    .focus
                    .filter(|id| self.alert_holds(*id) && !self.alert_message(*id))
                {
                    self.invoke(id, now, false);
                }
            }
            _ => {}
        }
        true
    }
    /// Whether a press outside the open box closes it: a modal dialog's only
    /// under `closedby="any"`, a popover's unless it is `manual`.
    pub(super) fn alert_light_dismiss(&self) -> bool {
        self.alert_root()
            .and_then(|id| self.host.kernel().node(id))
            .is_some_and(|n| {
                if self.alert.as_ref().unwrap().modal {
                    n.props.str(PropId::Closedby) == Some("any")
                } else {
                    n.props.str(PropId::Popover) != Some("manual")
                }
            })
    }
    pub(super) fn alert_pointer(&mut self, x: f32, y: f32, now: f64) -> bool {
        let Some(open) = &self.alert else {
            return false;
        };
        let modal = open.modal;
        let hit = self.hit(x, y);
        if hit.is_some_and(|id| self.alert_holds(id)) {
            return false;
        }
        // A popover's press outside it reaches the live page: one on its
        // invoker leaves it to the invoker's toggle, any other dismisses it
        // first (HTML's light dismiss), where an absorbed press would need a
        // second one.
        if !modal && hit.is_some_and(|id| self.alert_invoker_holds(id)) {
            return false;
        }
        if self.alert_light_dismiss() {
            self.alert_close_request(now);
        }
        modal
    }
    /// The open popover's root and placement, for the runner's top layer
    /// (structural reads); none for a modal box, which reads count as open.
    pub(super) fn alert_popover_frame(&self) -> Option<exact_runner::geometry::TopLayerFrame> {
        let id = self.alert_root().filter(|_| !self.alert_inert())?;
        Some((id, self.alert.as_ref()?.placed?, None))
    }
    /// Whether Back would close the open box: not a manual popover's.
    pub(crate) fn alert_takes_back(&self) -> bool {
        self.alert.as_ref().is_some_and(|a| !a.manual)
    }
    /// The sequential focus order, `(tabindex rank, tree position, view)`
    /// entries already sorted, with an open popover's stops, in their own
    /// order, right after its invoker's (HTML's popover focus scope).
    /// Unchanged with no popover, no invoker, an invoker inside the popover
    /// (Chromium's rule) or one that is no stop.
    pub(super) fn alert_tab_order(
        &self,
        mut order: Vec<(i64, usize, ViewId)>,
    ) -> Vec<(i64, usize, ViewId)> {
        let Some(invoker) = self
            .alert
            .as_ref()
            .filter(|a| !a.modal)
            .and_then(|a| a.invoker)
            .filter(|i| !self.alert_holds(*i) && order.iter().any(|o| o.2 == *i))
        else {
            return order;
        };
        let inside: Vec<_> = order
            .iter()
            .copied()
            .filter(|o| self.alert_holds(o.2))
            .collect();
        order.retain(|o| !self.alert_holds(o.2));
        let at = order
            .iter()
            .position(|o| o.2 == invoker)
            .map_or(order.len(), |i| i + 1);
        order.splice(at..at, inside);
        order
    }
    /// Whether `id` is inside the open popover's invoker or any button that
    /// targets it, whose press is no light dismiss (HTML): its toggle acts.
    fn alert_invoker_holds(&self, id: ViewId) -> bool {
        let Some(open) = self.alert.as_ref() else {
            return false;
        };
        let k = self.host.kernel();
        use exact_kernel::invocation::Action;
        std::iter::successors(k.node(id), |n| n.parent.and_then(|p| k.node(p))).any(|n| {
            Some(n.id) == open.invoker
                || exact_kernel::Invocation::of(k, &n).is_some_and(|i| {
                    i.target == open.key
                        && matches!(i.action, Action::Show | Action::Hide | Action::Toggle)
                })
        })
    }
    /// The agent's `tap` on a node under the open box: a press outside it,
    /// which dismisses it where a person's would. Under a modal box it
    /// presses nothing; under a popover it goes on to the page.
    pub(super) fn alert_outside_tap(&mut self, id: ViewId) -> Option<Result<String, String>> {
        let open = self.alert.as_ref()?;
        let (serial, modal) = (open.serial, open.modal);
        if self.alert_holds(id) || !self.alert_light_dismiss() {
            return None;
        }
        // Under a popover the tap goes on to the live page: its press
        // dismisses it where it lands outside it ([`Self::alert_pointer`]),
        // and a page control the popover covers is not reached.
        if !modal {
            return None;
        }
        self.alert_close_request(self.host.now());
        let dismissed = (self.alert.as_ref().map(|a| a.serial) != Some(serial)).then_some(serial);
        Some(Ok(
            serde_json::json!({"tapped": id, "dismissed": dismissed}).to_string(),
        ))
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
            .filter(|b| self.alert_holds(b.id))
            .map(|b| serde_json::json!({"id":b.id,"rect":b.rect}))
            .collect();
        let open = self.alert.as_ref().unwrap();
        serde_json::json!({"kind":open.kind.name(), "modal":open.modal, "node":id, "title":title, "rows":rows})
    }
}

#[cfg(test)]
#[path = "alert_input_tests.rs"]
mod input_tests;

#[cfg(test)]
#[path = "alert_popover_tests.rs"]
mod popover_tests;

#[cfg(test)]
#[path = "press_point_tests.rs"]
mod press_point_tests;
