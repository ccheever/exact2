//! Row-local instances; clips and composition reopen against this frame. @ref LLP 1149 D4.
use super::*;
use exact_ink::rows::{Command, Drawing};
use std::{collections::HashMap, rc::Rc};

#[cfg(test)]
mod tests;

#[derive(Clone)]
pub(super) enum Marker {
    Clip(Shape, Transform),
    Coverage(exact_ink::path::Coverage, bool),
    PopClip,
    Opacity(f32),
    PopOpacity,
    Mask(
        Shape,
        std::result::Result<GradientPaint, [u8; 4]>,
        Transform,
    ),
    Backdrop(Shape, exact_kernel::style::BackdropFilter, Transform),
    Slot(u32),
    Layer(Rc<Drawing<Marker>>, Transform, f32),
}

enum CaptureKind {
    Row(u32),
    Slot(u32, Option<u32>),
    Layer(Transform, f32),
}

pub(super) struct Capture {
    kind: CaptureKind,
    origin: (f32, f32),
    pub(super) drawing: Drawing<Marker>,
    slots: HashMap<u32, Slot>,
}

impl Capture {
    pub(super) fn offset(&self, scale: f32) -> [f32; 2] {
        [-self.origin.0 * scale, -self.origin.1 * scale]
    }
}

#[derive(Clone)]
struct Slot {
    drawing: Rc<Drawing<Marker>>,
    textures: Vec<TextureId>,
    origin: (f32, f32),
}

struct Row {
    drawing: Rc<Drawing<Marker>>,
    textures: Vec<TextureId>,
    slots: HashMap<u32, Slot>,
}

#[derive(Default)]
pub(super) struct Rows {
    kept: HashMap<u32, Rc<Row>>,
    pub(super) capture: Vec<Capture>,
    pub(super) groups: Vec<(u32, (f32, f32))>,
}

fn textures(drawing: &Drawing<Marker>) -> Vec<TextureId> {
    let mut textures = drawing.textures();
    for command in &drawing.commands {
        match command {
            Command::Marker(Marker::Coverage(c, _)) => textures.push(c.entry.texture),
            Command::Marker(Marker::Layer(drawing, ..)) => textures.extend(self::textures(drawing)),
            _ => {}
        }
    }
    textures.sort_unstable_by_key(|t| t.0);
    textures.dedup();
    textures
}

impl Ink {
    pub(super) fn record(&mut self, marker: Marker) -> bool {
        if let Some(c) = self.rows.capture.last_mut() {
            c.drawing.marker(marker);
            true
        } else {
            false
        }
    }

    pub(super) fn record_transform(&self, ts: Transform) -> Transform {
        self.rows
            .capture
            .last()
            .map_or(ts, |c| ts.post_translate(-c.origin.0, -c.origin.1))
    }

    fn pin(&mut self, textures: &[TextureId]) {
        for &t in textures {
            self.paths.pin(t);
            self.ramps.pin(t);
            self.glyphs.pin(t);
            self.images.pin(t);
        }
    }

    fn unpin(&mut self, textures: &[TextureId]) {
        for &t in textures {
            self.paths.unpin(t);
            self.ramps.unpin(t);
            self.glyphs.unpin(t);
            self.images.unpin(t);
        }
    }

    fn touch(&mut self, textures: &[TextureId]) {
        for &t in textures {
            self.paths.touch(t);
            self.ramps.touch(t);
            self.glyphs.touch(t);
        }
    }

    pub(super) fn record_row(&mut self, id: u32, origin: (f32, f32)) {
        assert!(self.rows.capture.is_empty(), "rows do not nest");
        self.rows.capture.push(Capture {
            kind: CaptureKind::Row(id),
            origin,
            drawing: Drawing::default(),
            slots: HashMap::new(),
        });
    }

    pub(super) fn keep_row(&mut self) -> u32 {
        let c = self.rows.capture.pop().expect("row_begin");
        let CaptureKind::Row(id) = c.kind else {
            panic!("row balanced")
        };
        self.free_row(id);
        let textures = textures(&c.drawing);
        self.pin(&textures);
        self.rows.kept.insert(
            id,
            Rc::new(Row {
                drawing: Rc::new(c.drawing),
                textures,
                slots: c.slots,
            }),
        );
        id
    }

    pub(super) fn free_row(&mut self, id: u32) {
        if let Some(row) = self.rows.kept.remove(&id) {
            self.unpin(&row.textures);
            for slot in row.slots.values() {
                self.unpin(&slot.textures);
            }
        }
    }

    pub(super) fn record_slot(&mut self, id: u32) {
        if !self
            .rows
            .capture
            .iter()
            .any(|c| matches!(c.kind, CaptureKind::Row(_)))
        {
            return;
        }
        let Some(parent) = self.rows.capture.last_mut() else {
            return;
        };
        let origin = parent.origin;
        parent.drawing.marker(Marker::Slot(id));
        self.rows.capture.push(Capture {
            kind: CaptureKind::Slot(id, None),
            origin,
            drawing: Drawing::default(),
            slots: HashMap::new(),
        });
    }

    pub(super) fn keep_slot(&mut self) {
        if !matches!(
            self.rows.capture.last().map(|c| &c.kind),
            Some(CaptureKind::Slot(..))
        ) {
            return;
        }
        let c = self.rows.capture.pop().unwrap();
        let CaptureKind::Slot(id, row) = c.kind else {
            unreachable!()
        };
        let textures = textures(&c.drawing);
        self.pin(&textures);
        let slot = Slot {
            drawing: Rc::new(c.drawing),
            textures,
            origin: c.origin,
        };
        let old = if let Some(row) = row {
            Rc::get_mut(self.rows.kept.get_mut(&row).expect("kept row"))
                .expect("not replaying while replacing a slot")
                .slots
                .insert(id, slot)
        } else {
            self.rows
                .capture
                .iter_mut()
                .find(|c| matches!(c.kind, CaptureKind::Row(_)))
                .expect("slot belongs to row")
                .slots
                .insert(id, slot)
        };
        if let Some(old) = old {
            self.unpin(&old.textures);
        }
    }

    pub(super) fn replace_slot(&mut self, row: u32, id: u32) -> bool {
        if !self.rows.capture.is_empty() {
            return false;
        }
        let Some(slot) = self.rows.kept.get(&row).and_then(|r| r.slots.get(&id)) else {
            return false;
        };
        self.rows.capture.push(Capture {
            kind: CaptureKind::Slot(id, Some(row)),
            origin: slot.origin,
            drawing: Drawing::default(),
            slots: HashMap::new(),
        });
        true
    }

    pub(super) fn culled(&self, (x, y, w, h): Rect4) -> bool {
        let s = self.encoder.scissor();
        let scale = self.scale;
        w <= 0.
            || h <= 0.
            || s.width == 0
            || s.height == 0
            || (x + w) * scale <= s.x as f32
            || (y + h) * scale <= s.y as f32
            || x * scale >= (s.x + s.width) as f32
            || y * scale >= (s.y + s.height) as f32
    }

    pub(super) fn draw_row(&mut self, id: u32, origin: (f32, f32)) -> Result<()> {
        let row = self
            .rows
            .kept
            .get(&id)
            .cloned()
            .ok_or_else(|| Error::Invalid("missing row".into()))?;
        self.touch(&row.textures);
        self.replay(&row.drawing, &row.slots, origin)
    }

    fn replay(
        &mut self,
        drawing: &Drawing<Marker>,
        slots: &HashMap<u32, Slot>,
        origin: (f32, f32),
    ) -> Result<()> {
        let offset = [origin.0 * self.scale, origin.1 * self.scale];
        for command in &drawing.commands {
            match command {
                Command::Instances(range) => {
                    for &(i, mut b) in &drawing.instances[range.clone()] {
                        let blended = i.flags & flags::BLEND_MASK != 0;
                        if blended {
                            b.destination = self.destination()?;
                        }
                        self.emit(i.translated(offset), b)?;
                        if blended {
                            self.flush()?;
                        }
                    }
                }
                Command::Marker(marker) => match marker {
                    Marker::Clip(shape, ts) => {
                        self.clip(shape, ts.post_translate(origin.0, origin.1))?
                    }
                    Marker::Coverage(c, transformed) => {
                        let mut c = *c;
                        c.rect[0] += offset[0];
                        c.rect[1] += offset[1];
                        self.coverage_clip(c, *transformed)?;
                    }
                    Marker::PopClip => self.pop_clip(),
                    Marker::Opacity(alpha) => self.open_group(*alpha, None),
                    Marker::PopOpacity => self.close_group()?,
                    Marker::Mask(shape, mask, ts) => {
                        self.masked(shape, mask, ts.post_translate(origin.0, origin.1))?
                    }
                    Marker::Backdrop(shape, filter, ts) => {
                        self.backdrop(shape, filter, ts.post_translate(origin.0, origin.1))?
                    }
                    Marker::Slot(id) => {
                        if let Some(slot) = slots.get(id) {
                            self.touch(&slot.textures);
                            self.replay(&slot.drawing, slots, origin)?;
                        }
                    }
                    Marker::Layer(drawing, transform, alpha) => {
                        self.open_group(*alpha, None);
                        let ts = Transform::from_translate(offset[0], offset[1])
                            .pre_concat(*transform)
                            .pre_translate(-offset[0], -offset[1]);
                        self.groups.last_mut().unwrap().transform = Some(ts);
                        self.replay(drawing, slots, origin)?;
                        self.close_group()?;
                    }
                },
            }
        }
        Ok(())
    }

    pub(super) fn record_layer(
        &mut self,
        _key: u64,
        ts: Transform,
        pivot: (f32, f32),
        base: [f32; 7],
    ) {
        let origin = self.rows.capture.last().map_or((0., 0.), |c| c.origin);
        let space = self.device_transform(self.record_transform(ts));
        let own = Transform::from_translate(pivot.0 + base[0], pivot.1 + base[1])
            .pre_rotate(base[3])
            .pre_scale(base[2], base[2])
            .pre_translate(-pivot.0, -pivot.1);
        let transform = space
            .pre_concat(own)
            .pre_concat(space.invert().unwrap_or_default());
        self.rows.capture.push(Capture {
            kind: CaptureKind::Layer(transform, base[4]),
            origin,
            drawing: Drawing::default(),
            slots: HashMap::new(),
        });
    }

    pub(super) fn keep_layer(&mut self) -> Result<()> {
        let c = self.rows.capture.pop().expect("layer_begin");
        let CaptureKind::Layer(ts, alpha) = c.kind else {
            panic!("layer balanced")
        };
        let marker = Marker::Layer(Rc::new(c.drawing), ts, alpha);
        if !self.record(marker.clone()) {
            let mut drawing = Drawing::default();
            drawing.marker(marker);
            self.replay(&drawing, &c.slots, (0., 0.))?;
        }
        Ok(())
    }
}
