//! Desktop scroll thumbs use the same extent and offset as the painted content.
use super::*;

pub(crate) const WIDTH: f32 = 12.;

#[derive(Clone, Copy)]
pub(crate) struct Bar {
    pub id: ViewId,
    pub key: exact_kernel::NodeKey,
    pub vertical: bool,
    pub port: Rect4,
    pub track: Rect4,
    pub thumb: Rect4,
    pub max: f32,
    pub offset: f32,
}

pub(crate) fn contains(r: Rect4, x: f32, y: f32) -> bool {
    x >= r.0 && y >= r.1 && x < r.0 + r.2 && y < r.1 + r.3
}

impl Bar {
    pub fn coordinate(&self, x: f32, y: f32) -> f32 {
        if self.vertical {
            y
        } else {
            x
        }
    }
    pub fn start(&self, r: Rect4) -> f32 {
        if self.vertical {
            r.1
        } else {
            r.0
        }
    }
    pub fn length(&self, r: Rect4) -> f32 {
        if self.vertical {
            r.3
        } else {
            r.2
        }
    }
    pub fn destination(&self, point: f32, grab: f32) -> f32 {
        let travel = self.length(self.track) - self.length(self.thumb);
        if travel <= 0. {
            return 0.;
        }
        ((point - grab - self.start(self.track)) / travel).clamp(0., 1.) * self.max
    }
}

#[derive(Default)]
pub(crate) struct State {
    pub enabled: bool,
    pub auto_hide: bool,
    pub pointer: Option<(f32, f32)>,
    pub drag: Option<(exact_kernel::NodeKey, bool, f32)>,
    pub pressed: bool,
    pub bars: Vec<Bar>,
    pub limits: BTreeMap<ViewId, f32>,
}

impl Painter {
    pub(super) fn paint_scrollbars(&mut self, walk: &Walk<'_, '_>) {
        self.scrollbars.bars.clear();
        if !self.scrollbars.enabled {
            return;
        }
        for b in &walk.boxes {
            let Some(node) = walk.scene.kernel.node(b.id) else {
                continue;
            };
            if b.projected() || !b.pointer_hit {
                continue;
            }
            let mut bounds = self.scroll_bounds(
                walk.scene.kernel,
                None,
                &node,
                self.scrollbars.limits.get(&b.id).copied(),
            );
            if let Some(field) = walk.fields.get(&b.id) {
                bounds.max = field.scroll_max;
            }
            let axes = [bounds.axes.0, bounds.axes.1];
            let max = [bounds.max.0, bounds.max.1];
            let offset = b.scroll.unwrap_or_default();
            let port = self.split_port(b.id, b.rect);
            for vertical in [false, true] {
                let i = usize::from(vertical);
                if !matches!(axes[i], Overflow::Auto | Overflow::Scroll) || max[i] <= 0. {
                    continue;
                }
                let track = if vertical {
                    (port.0 + port.2 - WIDTH, port.1 + 2., WIDTH, port.3 - 4.)
                } else {
                    (
                        port.0 + 2.,
                        port.1 + port.3 - WIDTH,
                        port.2 - 4. - if max[1] > 0. { WIDTH } else { 0. },
                        WIDTH,
                    )
                };
                // A clipped edge is not a scrollbar in the middle of its ancestor.
                let clip = b.clip.unwrap_or((0., 0., self.viewport.0, self.viewport.1));
                if track.0 < clip.0
                    || track.1 < clip.1
                    || track.0 + track.2 > clip.0 + clip.2 + 0.1
                    || track.1 + track.3 > clip.1 + clip.3 + 0.1
                {
                    continue;
                }
                let visible = if vertical { port.3 } else { port.2 };
                let length = if vertical { track.3 } else { track.2 };
                if length <= 0. || visible <= 0. {
                    continue;
                }
                let size = (length * visible / (visible + max[i])).max(24.).min(length);
                let offset = if vertical { offset.1 } else { offset.0 };
                let start = offset.clamp(0., max[i]) / max[i] * (length - size);
                let thumb = if vertical {
                    (track.0, track.1 + start, WIDTH, size)
                } else {
                    (track.0 + start, track.1, size, WIDTH)
                };
                self.scrollbars.bars.push(Bar {
                    id: b.id,
                    key: node.key,
                    vertical,
                    port,
                    track,
                    thumb,
                    max: max[i],
                    offset,
                });
            }
        }
        for bar in &self.scrollbars.bars {
            let dragged = self
                .scrollbars
                .drag
                .is_some_and(|d| (d.0, d.1) == (bar.key, bar.vertical));
            let hovered = self
                .scrollbars
                .pointer
                .is_some_and(|(x, y)| contains(bar.track, x, y));
            let visible = !self.scrollbars.auto_hide
                || dragged
                || self
                    .scrollbars
                    .pointer
                    .is_some_and(|(x, y)| contains(bar.port, x, y));
            if !visible {
                continue;
            }
            let expanded = hovered || dragged || !self.scrollbars.auto_hide;
            let width = if expanded { 6. } else { 2. };
            let (x, y, w, h) = bar.thumb;
            let rect = if bar.vertical {
                (x + (w - width) / 2., y, width, h)
            } else {
                (x, y + (h - width) / 2., w, width)
            };
            let ink = if let Some(palette) = super::look::contrast() {
                if dragged || hovered {
                    palette.highlight
                } else {
                    palette.button_text
                }
            } else if self.dark {
                [255, 255, 255, 139]
            } else {
                [0, 0, 0, 114]
            };
            self.backend.fill(
                &Shape::new(rect, [width / 2.; 4]),
                ink,
                Transform::identity(),
            );
        }
    }
}
