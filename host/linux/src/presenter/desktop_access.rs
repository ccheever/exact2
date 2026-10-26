//! Desktop accessibility actions use the same focus and editor as physical input.
use super::*;
use exact_runner::FieldSelection;

impl<D: DataSource> Presenter<D> {
    /// The live view for an enabled node in the last accessibility statement.
    pub fn access_view(&self, id: i32) -> Option<ViewId> {
        let node = self.access_told().iter().find(|n| n.id == id)?;
        (node.flags & access::flag::DISABLED == 0
            && node.view != 0
            && self.host.kernel().node(node.view).is_some()
            && self.focus_refusal(node.view).is_none())
        .then_some(node.view)
    }

    /// The accessible identity holding keyboard focus, including a menu item.
    pub fn access_focus(&self) -> Option<i32> {
        let view = if let Some((focused, _)) = self.desktop_surface_access() {
            focused
        } else {
            self.focus
        }?;
        self.access_told()
            .iter()
            .find(|n| n.view == view)
            .map(|n| n.id)
    }

    /// The top-layer surface's offset inside the desktop window, in logical pixels.
    pub fn access_origin(&self) -> (f32, f32) {
        self.desktop_surface_access()
            .map_or((0., 0.), |(_, origin)| origin)
    }

    /// Focus an accessible control without invoking it.
    pub fn access_focus_node(&mut self, id: i32, now_ms: f64) -> bool {
        let Some(node) = self.access_told().iter().find(|n| n.id == id) else {
            return false;
        };
        if node.flags & access::flag::DISABLED != 0 {
            return false;
        }
        let view = node.view;
        if self.surface_focus(view) {
            return true;
        }
        if self.access_view(id).is_none() || !self.focusable(view) {
            return false;
        }
        if let Some(error) = self.set_focus(Some(view), now_ms) {
            self.host.log(error);
        }
        self.focus == Some(view)
    }

    /// The editor's UTF-16 selection. Password values remain private to the editor.
    pub fn access_selection(&self, id: i32) -> Option<FieldSelection> {
        let view = self.access_view(id)?;
        let node = self.host.kernel().node(view)?;
        (node.node_type == NodeType::TextInput && node.props.str(PropId::Type) != Some("password"))
            .then(|| self.field_selection(view))
    }

    /// The current editor value, independent of the page statement's cache.
    pub fn access_text(&self, id: i32) -> Option<String> {
        self.access_selection(id)?;
        Some(self.field_text(self.access_view(id)?))
    }

    /// Select text requested by an accessibility service, clamped to the current value.
    pub fn access_select(&mut self, id: i32, selection: FieldSelection, now_ms: f64) -> bool {
        if self.access_selection(id).is_none() {
            return false;
        }
        let Some(view) = self.access_view(id) else {
            return false;
        };
        if !self.access_focus_node(id, now_ms) || self.access_view(id) != Some(view) {
            return false;
        }
        let value = self.field_text(view);
        let selection = FieldSelection::clamped(
            selection.start.into(),
            selection.end.into(),
            selection.direction,
            field::utf16_len(&value),
        );
        self.mark_field(view, &value, selection, true);
        self.field_select(view, &value, selection, now_ms);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_runner::SelectionDirection;

    #[test]
    fn accessibility_focus_and_selection_use_the_live_editor() {
        let plan = crate::test_plan(
            r#"component App
  view
    column
      textarea aria-label="Draft" value="a🦀z" width=220 height=60
      input aria-label="Read only" value="fixed" readonly=true
      input aria-label="Secret" type="password" value="secret"
      button aria-label="Disabled" disabled=true
        text "Disabled"
"#,
        );
        let (mut p, error) = Presenter::boot_with(
            &plan,
            (),
            (420., 300.),
            1.,
            PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(error.is_none(), "{error:?}");
        p.frame();
        p.reader_access(1., true);
        let find = |name| p.access_told().iter().find(|n| n.name == name).unwrap().id;
        let draft = find("Draft");
        let readonly = find("Read only");
        let secret = find("Secret");
        let disabled = find("Disabled");
        assert!(p.access_focus_node(draft, 1.));
        assert_eq!(p.access_focus(), Some(draft));
        let selection = FieldSelection {
            start: 1,
            end: 3,
            direction: SelectionDirection::Backward,
        };
        assert!(p.access_select(draft, selection, 2.));
        assert_eq!(p.access_selection(draft), Some(selection));
        assert_eq!(p.field_text(p.access_view(draft).unwrap()), "a🦀z");
        assert!(!p.access_focus_node(disabled, 3.));
        assert_eq!(p.access_focus(), Some(draft));
        assert_eq!(p.access_selection(secret), None);
        assert_eq!(p.access_text(secret), None);
        assert!(!p.access_select(secret, selection, 4.));
        assert!(p.access_select(readonly, selection, 5.));
        assert_eq!(p.field_text(p.access_view(readonly).unwrap()), "fixed");
        p.type_text(p.access_view(draft).unwrap(), "new 🦀 draft")
            .unwrap();
        assert_eq!(p.access_text(draft).as_deref(), Some("new 🦀 draft"));
        p.access_reset();
        assert!(!p.access_focus_node(draft, 6.));
        assert!(!p.access_select(draft, selection, 6.));
    }
}
