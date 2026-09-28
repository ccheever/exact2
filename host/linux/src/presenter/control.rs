//! Form controls, activated (LLP 1069.001 D4, D7, D9): a press on a
//! checkbox toggles it, as a click does on the web, and reports the new
//! state as HTML's `input` then `change`.
use super::*;

impl<D: DataSource> Presenter<D> {
    /// Toggle `id` if it is an enabled checkbox; false if it is not one. A
    /// bound checkbox draws the committed `checked`, so an action that
    /// refuses the toggle leaves it where it was; an unbound one keeps its
    /// own state here.
    pub(crate) fn toggle_control(&mut self, id: ViewId, now_ms: f64) -> bool {
        let Some(node) = self.host.kernel().node(id) else {
            return false;
        };
        if node.node_type != NodeType::Control {
            return false;
        }
        // A visible file input's press opens its picker (LLP 1069.002 D1).
        if node.props.str(PropId::Type) == Some("file") {
            if node.props.bool(PropId::Disabled) != Some(true) {
                match node.props.str(PropId::Id).map(str::to_owned) {
                    Some(name) => self.show_picker(&name),
                    None => self.host.log("picker: refused: a file input needs an id"),
                }
            }
            return true;
        }
        if node.props.bool(PropId::Disabled) == Some(true) {
            return true;
        }
        // A select's press opens its menu (D7).
        if node.props.str(PropId::Type) == Some("select") {
            self.menu = Some(id);
            self.dirty = true;
            return true;
        }
        let bound = node.props.bool(PropId::Checked);
        let on = !bound
            .or_else(|| self.controls.get(&id).copied())
            .unwrap_or(false);
        if bound.is_none() {
            self.controls.insert(id, on);
        }
        self.dirty = true;
        let mut dispatched = false;
        for (event, kind) in [
            (Event::Input(on.into()), EventKind::Input),
            (Event::Change(on.into()), EventKind::Change),
        ] {
            if self.host.runner().handlers_of(id).contains(&kind) {
                if let Some(e) = self.host.dispatch_at(id, event, now_ms) {
                    eprintln!("exact: {e}");
                }
                dispatched = true;
            }
        }
        if dispatched {
            if let Some(e) = self.after_commit() {
                eprintln!("exact: {e}");
            }
        }
        true
    }

    /// A control's value set as the platform would on a choice or a
    /// release (LLP 1069.001 D4, D9): HTML's `input` then `change`, each
    /// where the node hears it; the runner's refusal (a value no option
    /// has) is the error.
    pub(crate) fn set_control_value(&mut self, id: ViewId, value: &str) -> Result<String, String> {
        let node = self.host.kernel().node(id).ok_or(format!("no view {id}"))?;
        if node.props.bool(PropId::Disabled) == Some(true) {
            return Err(format!("view {id} is disabled"));
        }
        if node.props.str(PropId::Type) == Some("select") {
            let choices = self.host.kernel().select_choices(id);
            if !choices.iter().any(|c| c.value == value && !c.disabled) {
                return Err(format!(
                    "select {id} has no enabled option {value:?} (options: {})",
                    choices
                        .iter()
                        .map(|c| format!("{:?}", c.value))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
        self.menu = None;
        let now = self.host.now();
        let mut error = None;
        for (event, kind) in [
            (Event::Input(value.into()), EventKind::Input),
            (Event::Change(value.into()), EventKind::Change),
        ] {
            if self.host.runner().handlers_of(id).contains(&kind) {
                error = error.or(self.host.dispatch_at(id, event, now));
            }
        }
        let after = self.after_commit();
        if let Some(e) = error.or(after) {
            return Err(e);
        }
        let shown = self
            .host
            .kernel()
            .node(id)
            .and_then(|n| n.props.str(PropId::Value).map(str::to_owned))
            .unwrap_or_default();
        Ok(format!(
            "{{\"typed\":{id},\"value\":{},\"delivery\":\"recognized\"}}",
            {
                let mut s = String::new();
                quote(&shown, &mut s);
                s
            }
        ))
    }

    /// A press on a range moves its thumb to the pointer (D7): the value
    /// there, `input` then `change`. False when `id` is not a range.
    pub(crate) fn press_range(&mut self, id: ViewId, x: f32) -> bool {
        let Some(node) = self.host.kernel().node(id) else {
            return false;
        };
        if node.node_type != NodeType::Control || node.props.str(PropId::Type) != Some("range") {
            return false;
        }
        let range = exact_kernel::Range::of(node.props);
        let Some(b) = self.boxes.iter().find(|b| b.id == id).copied() else {
            return true;
        };
        let (bx, _, bw, _) = b.rect;
        let t = if bw > 16.0 {
            ((x - bx - 8.0) / (bw - 16.0)).clamp(0.0, 1.0) as f64
        } else {
            0.5
        };
        let value = range.min + t * (range.max - range.min);
        if let Err(e) = self.set_control_value(id, &value.to_string()) {
            self.host.log(format!("range: {e}"));
        }
        true
    }

    /// The open menu's rows and panel, under its select's painted box (above
    /// it when it would leave the viewport), as wide as the widest label.
    fn menu_geometry(&self) -> Option<(ViewId, crate::paint::control::MenuPaint)> {
        use crate::paint::control::{accent, MenuPaint, MENU_PAD};
        let id = self.menu?;
        let node = self.host.kernel().node(id)?;
        let b = self.boxes.iter().find(|b| b.id == id)?;
        let style = node.computed_style(exact_kernel::StyleMask::INHERITED);
        let choices = self.host.kernel().select_choices(id);
        let chosen = self.host.kernel().select_chosen(id).map(|c| c.view);
        let mut text = self.text.borrow_mut();
        let mut width = b.rect.2;
        let mut row = 0f32;
        for c in &choices {
            let p = text.paragraph(&crate::paint::text_spec(&style, &c.label), None);
            width = width.max(p.width + 32.0);
            row = row.max(p.height + 8.0);
        }
        let height = row * choices.len() as f32 + 2.0 * MENU_PAD;
        let (x, y, _, h) = b.rect;
        let below = y + h + 2.0;
        let top = if below + height > self.viewport.1 {
            (y - 2.0 - height).max(0.0)
        } else {
            below
        };
        Some((
            id,
            MenuPaint {
                rect: (x.min(self.viewport.0 - width).max(0.0), top, width, height),
                row,
                chosen: choices.iter().position(|c| Some(c.view) == chosen),
                rows: choices.into_iter().map(|c| (c.label, c.disabled)).collect(),
                accent: accent(&node, self.brush.dark),
                style,
            },
        ))
    }

    pub(crate) fn menu_paint(&self) -> Option<crate::paint::control::MenuPaint> {
        self.menu_geometry().map(|(_, m)| m)
    }

    /// A press while a menu is open: a row chooses it, anywhere else only
    /// closes the menu, as a light-dismiss popup does. `None` when no menu
    /// is open.
    pub(crate) fn menu_press(&mut self, x: f32, y: f32) -> Option<ViewId> {
        let (id, menu) = self.menu_geometry().or_else(|| {
            self.menu = None;
            None
        })?;
        self.menu = None;
        self.dirty = true;
        let (mx, my, mw, _) = menu.rect;
        let row = ((y - my - crate::paint::control::MENU_PAD) / menu.row).floor();
        if x >= mx && x < mx + mw && row >= 0.0 && (row as usize) < menu.rows.len() {
            let (_, disabled) = &menu.rows[row as usize];
            if !disabled {
                let value = self.host.kernel().select_choices(id)[row as usize]
                    .value
                    .clone();
                if let Err(e) = self.set_control_value(id, &value) {
                    self.host.log(format!("select: {e}"));
                }
            }
        }
        Some(id)
    }

    /// Each select's size, which Linux reports as the other hosts do (LLP
    /// 1069.001 D3): its widest option in its own font, with room for the
    /// chevron and Chrome's padding.
    pub(crate) fn size_controls(&mut self) {
        let mut sizes = Vec::new();
        {
            let kernel = self.host.kernel();
            let mut text = self.text.borrow_mut();
            for id in self.host.preorder() {
                let Some(node) = kernel.node(id) else {
                    continue;
                };
                if node.node_type != NodeType::Control {
                    continue;
                }
                // A select fits its widest option; a date control its
                // widest value (D3).
                let labels: Vec<String> = match node.props.str(PropId::Type) {
                    Some("select") => kernel
                        .select_choices(id)
                        .into_iter()
                        .map(|c| c.label)
                        .collect(),
                    Some("date") => vec!["0000-00-00".into()],
                    Some("time") => vec!["00:00:00".into()],
                    Some("datetime-local") => vec!["0000-00-00T00:00".into()],
                    _ => continue,
                };
                let chevron = if node.props.str(PropId::Type) == Some("select") {
                    30.0
                } else {
                    12.0
                };
                let style = node.computed_style(exact_kernel::StyleMask::INHERITED);
                let (mut w, mut h) = (0f32, 0f32);
                for label in &labels {
                    let p = text.paragraph(&crate::paint::text_spec(&style, label), None);
                    w = w.max(p.width);
                    h = h.max(p.height);
                }
                if h == 0.0 {
                    h = style.font_size * 1.2;
                }
                sizes.push((id, ((w + chevron).ceil(), (h + 6.0).ceil())));
            }
        }
        for (id, size) in sizes {
            if let Some(e) = self.host.set_intrinsic(id, Some(size)) {
                self.host.log(e);
            }
        }
    }
}
