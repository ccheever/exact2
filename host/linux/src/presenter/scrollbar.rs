use super::*;
use crate::paint::scrollbar::contains;

impl<D: DataSource> Presenter<D> {
    /// Refresh Windows' automatic scrollbar visibility after returning to the app.
    #[cfg(windows)]
    pub fn refresh_scrollbar_preferences(&mut self) {
        if crate::com::ensure_mta().is_err() {
            self.set_scrollbars(true);
            return;
        }
        self.set_scrollbars(
            windows::UI::ViewManagement::UISettings::new()
                .and_then(|settings| settings.AutoHideScrollBars())
                .unwrap_or(true),
        );
    }
    /// Desktop scrollbars, with the person's automatic visibility preference.
    pub fn set_scrollbars(&mut self, auto_hide: bool) {
        self.brush.scrollbars.enabled = true;
        self.brush.scrollbars.auto_hide = auto_hide;
        self.dirty = true;
    }

    pub(super) fn scrollbar_pointer(&mut self, phase: i32, x: f32, y: f32) -> bool {
        if self.shape_surface_open() || !self.brush.scrollbars.enabled {
            return false;
        }
        if let Some((key, vertical, grab)) = self.brush.scrollbars.drag {
            let bar = self
                .brush
                .scrollbars
                .bars
                .iter()
                .find(|b| (b.key, b.vertical) == (key, vertical))
                .copied();
            if phase == 1 {
                self.brush.scrollbars.drag = None;
                self.brush.scrollbars.pressed = false;
            }
            if let Some(bar) = bar.filter(|b| {
                self.host
                    .kernel()
                    .node_by_key(key)
                    .is_some_and(|n| n.id == b.id)
                    && self.alert_allows(b.id)
                    && !self.host.route_visibility(b.id).1
            }) {
                let mut to = self.scroll_of(bar.id);
                let value = bar.destination(bar.coordinate(x, y), grab);
                if vertical {
                    to.1 = value;
                } else {
                    to.0 = value;
                }
                self.scroll_to(bar.id, to);
            } else {
                self.brush.scrollbars.drag = None;
            }
            self.dirty = true;
            return true;
        }
        if phase == 1 {
            return std::mem::take(&mut self.brush.scrollbars.pressed);
        }
        if phase != 0 {
            return false;
        }
        // A press that lands on an open popover is the popover's: no page
        // scrollbar under it takes it.
        let on_popover = self.alert.is_some()
            && !self.alert_inert()
            && self.hit(x, y).is_some_and(|h| self.alert_holds(h));
        let bar = self
            .brush
            .scrollbars
            .bars
            .iter()
            .rev()
            .find(|b| {
                self.alert_allows(b.id)
                    && (!on_popover || self.alert_holds(b.id))
                    && contains(b.track, x, y)
            })
            .copied();
        let Some(bar) = bar else {
            return false;
        };
        self.brush.scrollbars.pressed = true;
        if contains(bar.thumb, x, y) {
            self.brush.scrollbars.drag = Some((
                bar.key,
                bar.vertical,
                bar.coordinate(x, y) - bar.start(bar.thumb),
            ));
        } else {
            let direction = if bar.coordinate(x, y) < bar.start(bar.thumb) {
                -1.
            } else {
                1.
            };
            let mut to = self.scroll_of(bar.id);
            let value = bar.offset + direction * bar.length(bar.port);
            if bar.vertical {
                to.1 = value;
            } else {
                to.0 = value;
            }
            self.scroll_to(bar.id, to);
        }
        self.dirty = true;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_scrollbars_drag_page_and_cancel_without_pressing_content() {
        let source = r#"component App
  state presses = 0
  action press
    presses = presses + 1
  view
    column width=240 height=200 overflow-y="auto" testId="list"
      button press=press width=240 height=1000 flex-shrink=0
        text "A long row"
"#;
        let (mut p, error) = Presenter::boot_with(
            &crate::test_plan(source),
            (),
            (240., 200.),
            1.,
            PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(error.is_none());
        p.set_scrollbars(false);
        p.frame();
        let bar = p.brush.scrollbars.bars[0];
        assert!(bar.vertical);
        assert!(bar.thumb.3 < bar.track.3);
        let (x, y) = (
            bar.thumb.0 + bar.thumb.2 / 2.,
            bar.thumb.1 + bar.thumb.3 / 2.,
        );
        p.pointer_down(x, y, 0.).unwrap();
        p.pointer_move(x, bar.track.1 + bar.track.3, 1.).unwrap();
        p.frame();
        p.pointer_up(x, bar.track.1 + bar.track.3, 2.).unwrap();
        assert_eq!(p.scroll_of(bar.id).1, bar.max);
        assert_eq!(
            p.host.runner().slot("presses"),
            Some(&exact_runner::Value::Number(0.))
        );
        p.frame();
        let bar = p.brush.scrollbars.bars[0];
        p.pointer_down(x, bar.track.1 + 1., 3.).unwrap();
        p.pointer_up(x, bar.track.1 + 1., 4.).unwrap();
        assert_eq!(p.scroll_of(bar.id).1, (bar.max - bar.port.3).max(0.));
        assert_eq!(
            p.host.runner().slot("presses"),
            Some(&exact_runner::Value::Number(0.))
        );
        p.frame();
        let bar = p.brush.scrollbars.bars[0];
        p.pointer_down(x, bar.thumb.1 + 2., 5.).unwrap();
        p.pointer_cancel(6.).unwrap();
        assert!(p.brush.scrollbars.drag.is_none());
    }
}
