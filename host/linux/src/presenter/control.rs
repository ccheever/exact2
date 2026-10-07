//! Form controls, activated (LLP 1069.001 D4, D7, D9): a press on a
//! checkbox toggles it, as a click does on the web, and reports the new
//! state as HTML's `input` then `change`.
use super::*;

/// Fixed geometry used by Linux's own painters. Text-bearing controls are
/// measured from their painted labels in `size_controls` instead.
fn fixed_painted_size(kind: exact_kernel::ControlKind) -> Option<(f32, f32)> {
    match kind {
        exact_kernel::ControlKind::Checkbox | exact_kernel::ControlKind::Radio => {
            Some((13.0, 13.0))
        }
        exact_kernel::ControlKind::Switch => Some((38.0, 22.0)),
        // Linux's file picker activation is native, but its visible control
        // is the same painted square as a checkbox until the picker opens.
        exact_kernel::ControlKind::File => Some((13.0, 13.0)),
        exact_kernel::ControlKind::Range => Some((129.0, 16.0)),
        _ => None,
    }
}

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
        // A native button presses as any button does (LLP 1069.011 D3); a
        // progress takes no press, as a box does (LLP 1069.001, amended
        // 2026-10-07).
        if matches!(node.props.str(PropId::Type), Some("button" | "progress")) {
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
        // A radio checks, and only checks (x2apps survey #2).
        if exact_kernel::ControlKind::of(node.node_type, node.props)
            == Some(exact_kernel::ControlKind::Radio)
        {
            self.check_radio(id, now_ms);
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
        // A radio takes `true`, checked as a click checks it; HTML has no
        // way to uncheck one but checking another (x2apps survey #2).
        if self.is_radio(id) {
            match value {
                "true" => {
                    let now = self.host.now();
                    self.check_radio(id, now);
                }
                "false" => {
                    return Err(format!(
                        "radio {id}: a radio is unchecked by checking another of its group"
                    ))
                }
                _ => return Err(format!("radio {id} takes true, not {value:?}")),
            }
            return Ok(format!(
                "{{\"typed\":{id},\"checked\":{},\"delivery\":\"recognized\"}}",
                self.radio_checked(id)
            ));
        }
        // A checkbox (or `switch`) takes `true` or `false`, and is toggled
        // when that differs, as a click does (the web host's typeControl).
        if matches!(
            exact_kernel::ControlKind::of(node.node_type, node.props),
            Some(exact_kernel::ControlKind::Checkbox | exact_kernel::ControlKind::Switch)
        ) {
            let on = match value {
                "true" => true,
                "false" => false,
                _ => return Err(format!("checkbox {id} takes true or false, not {value:?}")),
            };
            let checked = |p: &mut Self| {
                let bound = p
                    .host
                    .kernel()
                    .node(id)
                    .and_then(|n| n.props.bool(PropId::Checked));
                bound
                    .or_else(|| p.controls.get(&id).copied())
                    .unwrap_or(false)
            };
            if checked(self) != on {
                let now = self.host.now();
                self.toggle_control(id, now);
            }
            return Ok(format!(
                "{{\"typed\":{id},\"checked\":{},\"delivery\":\"recognized\"}}",
                checked(self)
            ));
        }
        // A select takes an enabled option's value, else its one label
        // (Playwright's `selectOption`; kanban F17, shop F10).
        let mut value = value.to_owned();
        if node.props.str(PropId::Type) == Some("select") {
            let choices = self.host.kernel().select_choices(id);
            let enabled: Vec<_> = choices.iter().filter(|c| !c.disabled).collect();
            let labelled: Vec<_> = enabled.iter().filter(|c| c.label == value.trim()).collect();
            if !enabled.iter().any(|c| c.value == value) && labelled.len() == 1 {
                value = labelled[0].value.clone();
            } else if !enabled.iter().any(|c| c.value == value) {
                return Err(format!(
                    "select {id} has no enabled option {value:?}{} (options: {})",
                    if labelled.len() > 1 {
                        " (that label is on more than one option: choose by value)"
                    } else {
                        ""
                    },
                    enabled
                        .iter()
                        .map(|c| format!("{:?} {:?}", c.value, c.label))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
        let value = value.as_str();
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
        let mut shown = self
            .host
            .kernel()
            .node(id)
            .and_then(|n| n.props.str(PropId::Value).map(str::to_owned))
            .unwrap_or_default();
        // A date, range or select keeps the choice until its bound value
        // changes, as the web build's does (`paint::control::choice`; LLP
        // 1069.001 D4, amended 2026-10-04; kanban2 #5).
        self.forget_replaced_choices();
        if shown != value {
            self.host.values.watch(id, Some(shown.clone()));
            self.chosen.insert(id, (value.to_owned(), shown.clone()));
            shown = value.to_owned();
        } else {
            self.host.values.watch(id, None);
            self.chosen.remove(&id);
        }
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
        let picked = crate::paint::control::choice(&node, self.chosen.get(&id));
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
                chosen: match picked {
                    Some(v) => choices.iter().position(|c| c.value == v),
                    None => choices.iter().position(|c| Some(c.view) == chosen),
                },
                rows: choices.into_iter().map(|c| (c.label, c.disabled)).collect(),
                // In the select's scheme (LLP 1034 §8), else the app's.
                accent: accent(&node, node.color_scheme_dark().unwrap_or(self.brush.dark)),
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

    /// Each control's painted size, which Linux reports as the other hosts
    /// do (LLP 1069.001 D3). Fixed painted widgets use the geometry their
    /// painter was designed for; fields measure the text they paint.
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
                let Some(kind) = exact_kernel::ControlKind::of(node.node_type, node.props) else {
                    continue;
                };
                if let Some(size) = fixed_painted_size(kind) {
                    sizes.push((id, size));
                    continue;
                }
                // A native button: its title in its look's font, padded
                // (LLP 1069.011 D6); Linux draws no symbol, which keeps the
                // room the other hosts give it.
                if node.props.str(PropId::Type) == Some("button") {
                    let face = kernel.press_face(id).unwrap_or_default();
                    let (px, py) = crate::paint::control::button_padding(
                        crate::paint::control::button_look(&node),
                    );
                    let p = text.paragraph(
                        &crate::paint::text_spec(
                            &crate::paint::control::button_text_style(),
                            face.title.as_deref().unwrap_or(" "),
                        ),
                        None,
                    );
                    let symbol = if face.symbol.is_some() {
                        p.height + if face.title.is_some() { 4.0 } else { 0.0 }
                    } else {
                        0.0
                    };
                    let w = if face.title.is_some() { p.width } else { 0.0 };
                    sizes.push((
                        id,
                        ((w + symbol + 2.0 * px).ceil(), (p.height + 2.0 * py).ceil()),
                    ));
                    continue;
                }
                // A select fits its widest option; a date control its widest
                // painted value (LLP 1069.001 D3).
                let labels: Vec<String> = match kind {
                    exact_kernel::ControlKind::Select => kernel
                        .select_choices(id)
                        .into_iter()
                        .map(|c| c.label)
                        .collect(),
                    exact_kernel::ControlKind::Date => vec!["0000-00-00".into()],
                    exact_kernel::ControlKind::Time => vec!["00:00:00".into()],
                    exact_kernel::ControlKind::DateTimeLocal => {
                        vec!["0000-00-00T00:00".into()]
                    }
                    _ => continue,
                };
                let chevron = if kind == exact_kernel::ControlKind::Select {
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

#[cfg(test)]
mod tests {
    use super::fixed_painted_size;
    use exact_kernel::ControlKind;

    #[test]
    fn fixed_controls_report_the_geometry_linux_paints() {
        assert_eq!(
            fixed_painted_size(ControlKind::Checkbox),
            Some((13.0, 13.0))
        );
        assert_eq!(fixed_painted_size(ControlKind::Radio), Some((13.0, 13.0)));
        assert_eq!(fixed_painted_size(ControlKind::Switch), Some((38.0, 22.0)));
        assert_eq!(fixed_painted_size(ControlKind::File), Some((13.0, 13.0)));
        assert_eq!(fixed_painted_size(ControlKind::Range), Some((129.0, 16.0)));
        assert_eq!(fixed_painted_size(ControlKind::Select), None);
        assert_eq!(fixed_painted_size(ControlKind::Date), None);
        assert_eq!(fixed_painted_size(ControlKind::Time), None);
        assert_eq!(fixed_painted_size(ControlKind::DateTimeLocal), None);
        assert_eq!(
            fixed_painted_size(ControlKind::Progress),
            None,
            "the kernel's 20 × 20"
        );
    }

    struct Empty;
    impl exact_runner::DataSource for Empty {
        fn query(
            &mut self,
            name: &str,
            _: &[exact_runner::Value],
        ) -> Result<exact_runner::Value, exact_runner::DataError> {
            Err(exact_runner::DataError::UnknownSource(name.into()))
        }
    }

    /// LLP 1069.001, amended 2026-10-07: a progress is painted as one still
    /// frame of UIKit's spokes in its `color`, centred in its box, and a
    /// press on it is its ancestor's, as on a box.
    #[test]
    fn a_progress_paints_still_spokes_in_its_color_and_takes_no_press() {
        let app = "component App\n  view\n    column width=100 height=100 background-color=\"#ffffff\" align-items=\"flex-start\"\n      progress testId=\"spin\" width=40 height=40 color=\"#ff0000\"\n";
        let (mut p, error) = super::Presenter::boot_with(
            &contract::compile(app).unwrap().encode(),
            Empty,
            (100., 100.),
            1.,
            std::path::PathBuf::new(),
            super::PainterChoice::Cpu,
        )
        .unwrap();
        assert!(error.is_none(), "{error:?}");
        let spin = {
            let k = p.host().kernel();
            k.node_by_key(k.find_by_test_id("spin")[0]).unwrap().id
        };
        let (x, y, w, h) = p.rect_of(spin).unwrap();
        assert_eq!((w, h), (40.0, 40.0));
        let mut at = |dx: f32, dy: f32| {
            let c = p
                .frame()
                .pixel((x + dx) as u32, (y + dy) as u32)
                .unwrap()
                .demultiply();
            [c.red(), c.green(), c.blue()]
        };
        assert_eq!(at(20.0, 4.0), [0xff, 0, 0], "the top spoke, at full ink");
        assert_eq!(at(20.0, 20.0), [0xff, 0xff, 0xff], "the centre is clear");
        assert_eq!(at(2.0, 2.0), [0xff, 0xff, 0xff], "the corners are clear");
        let now = p.host().now();
        assert!(
            !p.toggle_control(spin, now),
            "a press passes to its ancestor"
        );
        assert_eq!(
            p.type_text(spin, "50"),
            Err(format!("view {spin} is not an input"))
        );
    }
}
