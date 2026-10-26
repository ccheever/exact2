//! Mouse selection uses the field's painted glyph positions and UTF-16 offsets.
use super::*;
use crate::text::field::Cursor;
use exact_kernel::NodeKey;
use exact_runner::{FieldSelection, SelectionDirection};
use field::{byte_at, utf16_len};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy)]
enum Unit {
    Character,
    Word,
    Paragraph,
    Password,
}

pub(super) struct Drag {
    key: NodeKey,
    value: String,
    anchor: (u32, u32),
    unit: Unit,
    origin: (f32, f32),
    point: (f32, f32),
    moved: bool,
}

impl Drag {
    pub(super) fn moved(&self, point: (f32, f32)) -> bool {
        self.moved || point != self.origin
    }
}

fn range(value: &str, at: u32, unit: Unit) -> (u32, u32) {
    let at = at.min(utf16_len(value));
    let byte = byte_at(value, at);
    match unit {
        Unit::Character => (at, at),
        Unit::Password => (0, utf16_len(value)),
        Unit::Paragraph => {
            let start = value[..byte].rfind('\n').map_or(0, |i| i + 1);
            let end = value[byte..]
                .find('\n')
                .map_or(value.len(), |i| byte + i + 1);
            (utf16_len(&value[..start]), utf16_len(&value[..end]))
        }
        Unit::Word => {
            #[cfg(target_os = "windows")]
            if let Some(range) = super::word::range(value, at) {
                return range;
            }
            value
                .split_word_bound_indices()
                .find(|(start, text)| byte < start + text.len())
                .map_or((at, at), |(start, text)| {
                    (
                        utf16_len(&value[..start]),
                        utf16_len(&value[..start + text.len()]),
                    )
                })
        }
    }
}

fn selection(anchor: (u32, u32), current: (u32, u32)) -> FieldSelection {
    let (from, to) = if current.0 < anchor.0 {
        (anchor.1, current.0)
    } else {
        (anchor.0, current.1.max(anchor.1))
    };
    FieldSelection {
        start: from.min(to),
        end: from.max(to),
        direction: match to.cmp(&from) {
            std::cmp::Ordering::Less => SelectionDirection::Backward,
            std::cmp::Ordering::Equal => SelectionDirection::None,
            std::cmp::Ordering::Greater => SelectionDirection::Forward,
        },
    }
}

impl<D: DataSource> Presenter<D> {
    pub(super) fn begin_field_drag(
        &mut self,
        key: NodeKey,
        point: (f32, f32),
        clicks: u32,
        before: Option<(NodeKey, String, FieldSelection)>,
    ) {
        let Some(node) = self.host.kernel().node_by_key(key).filter(|n| {
            n.node_type == NodeType::TextInput
                && !crate::fields::hosts(n)
                && self.focus == Some(n.id)
                && self.input_live(key)
                && self.focus_refusal(n.id).is_none()
                && !self.pointerdown_prevents_focus(n.id)
        }) else {
            return;
        };
        let id = node.id;
        let unit = if clicks > 1 && node.props.str(PropId::Type) == Some("password") {
            Unit::Password
        } else {
            match clicks {
                0 | 1 => Unit::Character,
                2 => Unit::Word,
                _ => Unit::Paragraph,
            }
        };
        let value = self.field_text(id);
        let Some(cursor) = self.field_pointer_hit(id, &value, point, unit) else {
            return;
        };
        let current = range(&value, cursor.offset, unit);
        let anchor = before
            .filter(|(old_key, old_value, _)| {
                *old_key == key && *old_value == value && self.modifiers().shift
            })
            .map_or(current, |(_, _, mark)| {
                let at = if mark.direction == SelectionDirection::Backward {
                    mark.end
                } else {
                    mark.start
                };
                (at, at)
            });
        self.mark_pointer_selection(id, &value, selection(anchor, current), cursor.upstream);
        self.field_drag = Some(Drag {
            key,
            value,
            anchor,
            unit,
            origin: point,
            point,
            moved: false,
        });
    }

    fn field_pointer_hit(
        &mut self,
        id: ViewId,
        value: &str,
        point: (f32, f32),
        unit: Unit,
    ) -> Option<Cursor> {
        self.boxes();
        let point = self
            .boxes
            .iter()
            .find(|b| b.id == id)?
            .field_point(point.0, point.1);
        let field = self.current_field(id, value)?;
        match unit {
            Unit::Character => field.hit(point.0, point.1),
            _ => field.hit_character(point.0, point.1).map(|offset| Cursor {
                offset,
                upstream: false,
            }),
        }
    }

    pub(super) fn drag_field(&mut self, x: f32, y: f32) -> bool {
        let Some(mut drag) = self.field_drag.take() else {
            return false;
        };
        let Some(node) = self.host.kernel().node_by_key(drag.key).filter(|n| {
            self.focus == Some(n.id)
                && self.input_live(drag.key)
                && self.focus_refusal(n.id).is_none()
        }) else {
            return false;
        };
        let id = node.id;
        if self.field_text(id) != drag.value {
            return false;
        }
        if drag.point != (x, y) {
            let Some(cursor) = self.field_pointer_hit(id, &drag.value, (x, y), drag.unit) else {
                return false;
            };
            let current = range(&drag.value, cursor.offset, drag.unit);
            self.mark_pointer_selection(
                id,
                &drag.value,
                selection(drag.anchor, current),
                cursor.upstream,
            );
            drag.moved = true;
            drag.point = (x, y);
        }
        self.field_drag = Some(drag);
        true
    }
}

#[cfg(test)]
mod tests;
