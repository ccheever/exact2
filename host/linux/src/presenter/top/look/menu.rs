//! Overflowing desktop menus keep the item geometry, hit tests and focus in one viewport.
use super::*;

pub(super) struct Scroll {
    offset: f32,
    max: f32,
    dark: bool,
    hovered: bool,
    grab: Option<f32>,
}

impl Scroll {
    pub(super) fn new(max: f32, dark: bool) -> Self {
        Self {
            offset: 0.,
            max,
            dark,
            hovered: false,
            grab: None,
        }
    }
}

pub(super) fn contains(r: Rect4, x: f32, y: f32) -> bool {
    x >= r.0 && y >= r.1 && x < r.0 + r.2 && y < r.1 + r.3
}

impl Layout {
    pub(in crate::presenter) fn menu_scroll(&self) -> Option<(f32, f32)> {
        self.menu.as_ref().map(|s| (s.offset, s.max))
    }

    pub(in crate::presenter) fn menu_clip(&self) -> Option<Rect4> {
        self.menu
            .as_ref()
            .map(|_| (1., 2., self.size.0 - 2., self.size.1 - 4.))
    }

    pub(super) fn scroll_menu(&mut self, delta: f32) -> bool {
        let Some(scroll) = &mut self.menu else {
            return false;
        };
        let next = (scroll.offset + delta).clamp(0., scroll.max);
        let shift = scroll.offset - next;
        if shift == 0. {
            return false;
        }
        scroll.offset = next;
        for target in &mut self.targets {
            target.rect.1 += shift;
            target.layer.0 = Shape::new(target.rect, [4.; 4]);
        }
        for item in &mut self.items {
            match item {
                Item::Text(_, at, _, _) => at.1 += shift,
                Item::Rule(rect, _) | Item::Symbol(_, rect, _) => rect.1 += shift,
            }
        }
        true
    }

    pub(in crate::presenter) fn reveal_menu_target(&mut self, index: usize) -> bool {
        let Some((_, top, _, height)) = self.menu_clip() else {
            return false;
        };
        let Some(target) = self.targets.get(index) else {
            return false;
        };
        let (_, y, _, h) = target.rect;
        let delta = if y < top {
            y - top
        } else {
            (y + h - top - height).max(0.)
        };
        self.scroll_menu(delta)
    }

    fn menu_bar(&self) -> Option<(Rect4, Rect4)> {
        let scroll = self.menu.as_ref()?;
        let height = (self.size.1 - 8.).max(0.);
        let track = (self.size.0 - 13., 4., 12., height);
        let thumb_height = (height * height / (height + scroll.max))
            .max(24.)
            .min(height);
        let top = track.1 + scroll.offset / scroll.max * (height - thumb_height);
        Some((track, (track.0, top, track.2, thumb_height)))
    }

    pub(super) fn paint_menu_scrollbar(&self, backend: &mut dyn Backend) {
        let Some((_, thumb)) = self.menu_bar() else {
            return;
        };
        let scroll = self.menu.as_ref().unwrap();
        let active = scroll.hovered || scroll.grab.is_some();
        let width = if active { 6. } else { 2. };
        let ink = self.contrast.map_or_else(
            || {
                if scroll.dark {
                    [255, 255, 255, 139]
                } else {
                    [0, 0, 0, 114]
                }
            },
            |palette| {
                if active {
                    palette.highlight
                } else {
                    palette.button_text
                }
            },
        );
        let rect = (thumb.0 + (thumb.2 - width) / 2., thumb.1, width, thumb.3);
        backend.fill(
            &Shape::new(rect, [width / 2.; 4]),
            ink,
            Transform::identity(),
        );
    }

    /// Returns whether the scrollbar consumed the pointer and whether its picture changed.
    pub(in crate::presenter) fn menu_pointer(
        &mut self,
        phase: i32,
        x: f32,
        y: f32,
    ) -> (bool, bool) {
        let Some((track, thumb)) = self.menu_bar() else {
            return (false, false);
        };
        let scroll = self.menu.as_mut().unwrap();
        let hovered = contains(track, x, y);
        let mut changed = hovered != scroll.hovered;
        scroll.hovered = hovered;
        let consumed = hovered || scroll.grab.is_some();
        let mut delta = 0.;
        match phase {
            0 if hovered => {
                if contains(thumb, x, y) {
                    scroll.grab = Some(y - thumb.1);
                } else {
                    delta = self.size.1 * if y < thumb.1 { -1. } else { 1. };
                }
                changed = true;
            }
            1 => {
                changed |= scroll.grab.take().is_some();
            }
            2 => {
                if let Some(grab) = scroll.grab {
                    let travel = track.3 - thumb.3;
                    if travel > 0. {
                        delta = ((y - grab - track.1) / travel).clamp(0., 1.) * scroll.max
                            - scroll.offset;
                    }
                }
            }
            _ => {}
        }
        changed |= self.scroll_menu(delta);
        (consumed, changed)
    }

    pub(in crate::presenter) fn cancel_menu_pointer(&mut self) -> bool {
        self.menu.as_mut().is_some_and(|scroll| {
            let changed = scroll.hovered || scroll.grab.is_some();
            scroll.hovered = false;
            scroll.grab = None;
            changed
        })
    }
}
