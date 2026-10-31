//! Painted editors scroll their value independently of the surrounding page.
use super::*;

impl<D: DataSource> Presenter<D> {
    /// The focused insertion point in viewport coordinates, clipped to its field.
    pub fn caret_frame(&self) -> Option<Rect4> {
        let id = self.focus?;
        let selection = self.field_selection(id);
        if selection.start != selection.end || self.shape_surface_open() {
            return None;
        }
        #[cfg(windows)]
        if let Some([x, y, w, h]) = exact_ink::device::d3d11::fields::caret(id) {
            let s = self.brush.scale;
            return Some((x / s, y / s, w / s, h / s));
        }
        if self.ime_paint().is_some_and(|c| !c.cursor) {
            return None;
        }
        self.insertion_frame(self.field_cursor(id))
    }

    /// Candidate windows follow the active composition clause, even when
    /// that clause is selected and the ordinary insertion caret is hidden.
    pub fn ime_cursor_area(&self) -> Option<Rect4> {
        self.ime_target()?;
        let id = self.focus?;
        let cursor = if self.ime_owns(id) {
            crate::text::field::Cursor {
                offset: self.field_selection(id).start,
                upstream: false,
            }
        } else {
            self.field_cursor(id)
        };
        self.insertion_frame(cursor)
    }

    fn insertion_frame(&self, cursor: crate::text::field::Cursor) -> Option<Rect4> {
        let id = self.focus?;
        let field = self.brush.field_text.get(&id)?;
        let box_ = self.boxes.iter().find(|b| b.id == id)?;
        if !box_.flat() {
            return None;
        }
        let rect = field.caret_frame(cursor);
        let clip = box_
            .clip
            .unwrap_or((0., 0., self.viewport.0, self.viewport.1));
        let left = rect.0.max(clip.0).max(0.);
        let top = rect.1.max(clip.1).max(0.);
        let right = (rect.0 + rect.2).min(clip.0 + clip.2).min(self.viewport.0);
        let bottom = (rect.1 + rect.3).min(clip.1 + clip.3).min(self.viewport.1);
        (right > left && bottom > top).then_some((left, top, right - left, bottom - top))
    }

    /// Set the next paint's caret visibility without changing selection or focus.
    pub fn set_caret_visible(&mut self, visible: bool) {
        self.brush.caret_hidden = !visible;
    }

    pub(crate) fn reveal_field_cursor(&mut self) -> bool {
        if !std::mem::take(&mut self.field_reveal) {
            return false;
        }
        let Some(id) = self.focus else { return false };
        let Some(field) = self.brush.field_text.get(&id) else {
            return false;
        };
        let next = field.reveal_cursor(self.field_cursor(id));
        if self.scroll_of(id) == next {
            return false;
        }
        self.scroll.insert(id, next);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_runner::FieldSelection;

    fn fixture(tag: &str, value: &str) -> (Presenter<()>, ViewId) {
        let source = format!("component App\n  view\n    {tag} testId=\"editor\" value={} width=200 height=64 padding=4 appearance=\"none\" outline=\"none\" font-size=16 line-height=\"24px\" caret-color=\"#ff0000\"\n", serde_json::to_string(value).unwrap());
        let (mut p, error) = Presenter::boot_with(
            &crate::test_plan(&source),
            (),
            (300., 200.),
            1.,
            PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(error.is_none(), "{error:?}");
        let k = p.host.kernel();
        let id = k.node_by_key(k.find_by_test_id("editor")[0]).unwrap().id;
        p.set_focus(Some(id), 0.);
        (p, id)
    }

    fn assert_caret_in_field(p: &mut Presenter<()>, id: ViewId) {
        let pixels = p.frame();
        let r = p.rect_of(id).unwrap();
        let mut count = 0;
        for y in 0..pixels.height() {
            for x in 0..pixels.width() {
                let pixel = pixels.pixel(x, y).unwrap();
                let coverage = pixel.red().saturating_sub(pixel.green());
                if coverage > 0 {
                    count += u32::from(coverage);
                    assert!(x as f32 >= r.0 && (x as f32) < r.0 + r.2);
                    assert!(y as f32 >= r.1 && (y as f32) < r.1 + r.3);
                }
            }
        }
        assert!(
            count >= 16 * 255,
            "caret coverage={count}, scroll={:?}",
            p.scroll_of(id)
        );
    }

    #[test]
    fn multiline_caret_reveals_scrolls_and_hits_the_visible_text() {
        let value = (1..=12)
            .map(|n| format!("line {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        let (mut p, id) = fixture("textarea", &value);
        assert_caret_in_field(&mut p, id);
        let end = p.scroll_of(id).1;
        assert!(end > 100.);
        p.wheel_at(20., 20., 0., -48.);
        p.frame();
        assert_eq!(
            p.scroll_of(id).1,
            end - 48.,
            "a wheel must not snap back to the caret"
        );
        p.press_field(id, 8., 8.);
        assert!(
            p.field_selection(id).start > 0,
            "hit testing must account for the internal scroll"
        );
        p.hold_modifier("ControlLeft", true);
        p.field_key(id, "Home", 1.);
        p.hold_modifier("ControlLeft", false);
        assert_caret_in_field(&mut p, id);
        assert_eq!(p.scroll_of(id), (0., 0.));
        p.hold_modifier("ControlLeft", true);
        p.field_key(id, "End", 2.);
        p.hold_modifier("ControlLeft", false);
        p.field_key(id, "!", 3.);
        assert_caret_in_field(&mut p, id);
        assert!(p.field_text(id).ends_with('!'));
        assert_eq!(p.scroll_of(id).1, end);
    }

    #[test]
    fn single_line_caret_scrolls_horizontally_and_returns_home() {
        let (mut p, id) = fixture("input", &"abcdefghij ".repeat(12));
        assert_caret_in_field(&mut p, id);
        assert!(p.scroll_of(id).0 > 500.);
        p.field_key(id, "Home", 1.);
        assert_caret_in_field(&mut p, id);
        assert_eq!(p.scroll_of(id).0, 0.);
    }

    #[test]
    fn blinking_changes_only_caret_pixels_and_preserves_the_editor() {
        let (mut p, id) = fixture("textarea", &"wrapped text ".repeat(30));
        let on = p.frame();
        let rect = p.caret_frame().unwrap();
        let selection = p.field_selection(id);
        let scroll = p.scroll_of(id);
        assert!(scroll.1 > 0.);
        p.set_caret_visible(false);
        let off = p.frame();
        let mut changed = 0;
        for (i, (a, b)) in on.pixels().iter().zip(off.pixels()).enumerate() {
            if a != b {
                let x = i as u32 % on.width();
                let y = i as u32 / on.width();
                assert!((x as f32) >= rect.0.floor() && (x as f32) < (rect.0 + rect.2).ceil());
                assert!((y as f32) >= rect.1.floor() && (y as f32) < (rect.1 + rect.3).ceil());
                changed += 1;
            }
        }
        assert!(changed > 0);
        assert_eq!(p.field_selection(id), selection);
        assert_eq!(p.scroll_of(id), scroll);
        assert_eq!(p.caret_frame(), Some(rect));
        p.set_caret_visible(true);
        assert_eq!(p.frame().data(), on.data());
        p.mark_field(
            id,
            &p.field_text(id),
            FieldSelection {
                start: 0,
                end: 4,
                ..FieldSelection::caret(0)
            },
            false,
        );
        assert_eq!(p.caret_frame(), None);
        p.set_focus(None, 1.);
        assert_eq!(p.caret_frame(), None);
    }
}
