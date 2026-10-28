//! The control arm retains Vello scene fragments. @ref LLP 1149 D4, D15.
use super::*;
use std::{collections::HashMap, rc::Rc};
use vello::kurbo::Shape as _;

pub(super) enum Op {
    Scene(vello::Scene),
    Push(backdrop::Layer),
    Pop,
    Mask(Shape, Result<GradientPaint, [u8; 4]>, Transform),
    Backdrop(Shape, exact_kernel::style::BackdropFilter, Transform),
    Slot(u32),
    Layer(Rc<Drawing>, Affine, f32),
}

#[derive(Default)]
pub(super) struct Drawing {
    ops: Vec<Op>,
    _images: Vec<Arc<Bitmap>>,
}

enum Kind {
    Row(u32),
    Slot(u32, Option<u32>),
    Layer(Affine, f32),
}

pub(super) struct Capture {
    kind: Kind,
    origin: (f32, f32),
    parent: vello::Scene,
    layers: Vec<backdrop::Layer>,
    ops: Vec<Op>,
    pub(super) images: Vec<Arc<Bitmap>>,
    slots: HashMap<u32, Slot>,
}

struct Slot {
    drawing: Rc<Drawing>,
    origin: (f32, f32),
}

pub(super) struct Row {
    drawing: Rc<Drawing>,
    slots: HashMap<u32, Slot>,
}

#[derive(Default)]
pub(super) struct Rows {
    pub(super) kept: HashMap<u32, Rc<Row>>,
    pub(super) capture: Vec<Capture>,
    pub(super) groups: Vec<(u32, (f32, f32))>,
}

impl Rows {
    pub(super) fn origin(&self) -> (f32, f32) {
        self.capture.last().map_or((0., 0.), |c| c.origin)
    }
}

impl Gpu {
    fn fragment(&mut self) {
        self.rows
            .capture
            .last_mut()
            .expect("recording")
            .ops
            .push(Op::Scene(std::mem::take(&mut self.scene)));
    }

    pub(super) fn record(&mut self, op: Op) -> bool {
        if self.rows.capture.is_empty() {
            return false;
        }
        self.fragment();
        self.rows.capture.last_mut().unwrap().ops.push(op);
        true
    }

    pub(super) fn record_transform(&self, ts: Transform) -> Transform {
        let origin = self.rows.origin();
        ts.post_translate(-origin.0, -origin.1)
    }

    fn capture(&mut self, kind: Kind, origin: (f32, f32)) {
        self.rows.capture.push(Capture {
            kind,
            origin,
            parent: std::mem::take(&mut self.scene),
            layers: std::mem::take(&mut self.layers),
            ops: Vec::new(),
            images: Vec::new(),
            slots: HashMap::new(),
        });
    }

    fn end_capture(&mut self) -> Capture {
        self.fragment();
        let mut c = self.rows.capture.pop().expect("recording");
        self.scene = std::mem::take(&mut c.parent);
        self.layers = std::mem::take(&mut c.layers);
        c
    }

    pub(super) fn record_row(&mut self, id: u32, origin: (f32, f32)) {
        assert!(self.rows.capture.is_empty(), "rows do not nest");
        self.capture(Kind::Row(id), origin);
    }

    pub(super) fn keep_row(&mut self) -> u32 {
        let c = self.end_capture();
        let Kind::Row(id) = c.kind else {
            panic!("row balanced")
        };
        self.rows.kept.insert(
            id,
            Rc::new(Row {
                drawing: Rc::new(Drawing {
                    ops: c.ops,
                    _images: c.images,
                }),
                slots: c.slots,
            }),
        );
        id
    }

    pub(super) fn record_slot(&mut self, id: u32) {
        if !self
            .rows
            .capture
            .iter()
            .any(|c| matches!(c.kind, Kind::Row(_)))
        {
            return;
        }
        if self.rows.capture.is_empty() {
            return;
        }
        let origin = self.rows.origin();
        self.record(Op::Slot(id));
        self.capture(Kind::Slot(id, None), origin);
    }

    pub(super) fn keep_slot(&mut self) {
        if !matches!(
            self.rows.capture.last().map(|c| &c.kind),
            Some(Kind::Slot(..))
        ) {
            return;
        }
        let c = self.end_capture();
        let Kind::Slot(id, row) = c.kind else {
            unreachable!()
        };
        let slot = Slot {
            drawing: Rc::new(Drawing {
                ops: c.ops,
                _images: c.images,
            }),
            origin: c.origin,
        };
        if let Some(row) = row {
            Rc::get_mut(self.rows.kept.get_mut(&row).expect("kept row"))
                .expect("not replaying while replacing")
                .slots
                .insert(id, slot);
        } else {
            self.rows
                .capture
                .iter_mut()
                .find(|c| matches!(c.kind, Kind::Row(_)))
                .expect("slot belongs to row")
                .slots
                .insert(id, slot);
        }
    }

    pub(super) fn replace_slot(&mut self, row: u32, id: u32) -> bool {
        if !self.rows.capture.is_empty() {
            return false;
        }
        let Some(slot) = self.rows.kept.get(&row).and_then(|r| r.slots.get(&id)) else {
            return false;
        };
        let origin = slot.origin;
        self.capture(Kind::Slot(id, Some(row)), origin);
        true
    }

    pub(super) fn culled(&self, (x, y, w, h): Rect4) -> bool {
        let scale = self.scale as f64;
        let mut bounds = Rect::new(
            0.,
            0.,
            self.width as f64 * scale,
            self.height as f64 * scale,
        );
        for layer in &self.layers {
            if let backdrop::Layer::Clip(_, ts, path) | backdrop::Layer::Group(ts, path) = layer {
                bounds = bounds.intersect(ts.transform_rect_bbox(path.bounding_box()));
            }
        }
        let row = Rect::new(
            x as f64 * scale,
            y as f64 * scale,
            (x + w) as f64 * scale,
            (y + h) as f64 * scale,
        );
        let intersection = bounds.intersect(row);
        intersection.width() <= 0. || intersection.height() <= 0.
    }

    pub(super) fn draw_row(&mut self, id: u32, origin: (f32, f32)) {
        let row = self.rows.kept.get(&id).cloned().expect("kept row");
        let offset = Affine::translate((
            origin.0 as f64 * self.scale as f64,
            origin.1 as f64 * self.scale as f64,
        ));
        self.replay(&row.drawing, &row.slots, offset);
    }

    fn replay(&mut self, drawing: &Drawing, slots: &HashMap<u32, Slot>, offset: Affine) {
        let point_ts =
            Affine::scale(1. / self.scale as f64) * offset * Affine::scale(self.scale as f64);
        let a = point_ts.as_coeffs();
        let ts = Transform::from_row(
            a[0] as f32,
            a[1] as f32,
            a[2] as f32,
            a[3] as f32,
            a[4] as f32,
            a[5] as f32,
        );
        for op in &drawing.ops {
            match op {
                Op::Scene(scene) => self.scene.append(scene, Some(offset)),
                Op::Push(layer) => self.push_recorded(match layer {
                    backdrop::Layer::Clip(rule, t, p) => {
                        backdrop::Layer::Clip(*rule, offset * *t, p.clone())
                    }
                    backdrop::Layer::Group(t, p) => backdrop::Layer::Group(offset * *t, p.clone()),
                    backdrop::Layer::Opacity(alpha) => backdrop::Layer::Opacity(*alpha),
                }),
                Op::Pop => self.pop_recorded(),
                Op::Mask(shape, mask, own) => self.pop_masked(shape, mask, ts.pre_concat(*own)),
                Op::Backdrop(shape, filter, own) => {
                    self.filter_backdrop(shape, filter, ts.pre_concat(*own))
                }
                Op::Slot(id) => {
                    if let Some(slot) = slots.get(id) {
                        self.replay(&slot.drawing, slots, offset);
                    }
                }
                Op::Layer(drawing, transform, alpha) => {
                    self.push_recorded(backdrop::Layer::Opacity(*alpha));
                    self.replay(drawing, slots, offset * *transform);
                    self.pop_recorded();
                }
            }
        }
    }

    pub(super) fn record_layer(
        &mut self,
        _key: u64,
        ts: Transform,
        pivot: (f32, f32),
        base: [f32; 7],
    ) {
        let origin = self.rows.origin();
        let space = self.affine(ts);
        let own = Affine::translate(((pivot.0 + base[0]) as f64, (pivot.1 + base[1]) as f64))
            * Affine::rotate((base[3] as f64).to_radians())
            * Affine::scale(base[2] as f64)
            * Affine::translate((-pivot.0 as f64, -pivot.1 as f64));
        if !self.rows.capture.is_empty() {
            self.fragment();
        }
        self.capture(Kind::Layer(space * own * space.inverse(), base[4]), origin);
    }

    pub(super) fn keep_layer(&mut self) {
        let c = self.end_capture();
        let Kind::Layer(ts, alpha) = c.kind else {
            panic!("layer balanced")
        };
        let drawing = Rc::new(Drawing {
            ops: c.ops,
            _images: c.images,
        });
        if !self.record(Op::Layer(drawing.clone(), ts, alpha)) {
            self.push_recorded(backdrop::Layer::Opacity(alpha));
            self.replay(&drawing, &c.slots, ts);
            self.pop_recorded();
        }
    }
}
