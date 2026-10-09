//! A picture's drawing kept apart from its row (`SLOT` in the row,
//! `SLOT_SET` beside it), so a picture arriving changes the slot and not the
//! row; and a kept row's slot drawn again without the row, in what the row's
//! recording had around it, so the row is not walked for a picture.
use super::*;

/// What a slot's drawing is recorded in: the row's origin, and the clips
/// around the picture with which of them the row had written by then.
#[derive(Clone)]
pub(super) struct Context {
    clips: Vec<clip::Clip>,
    origin: (f32, f32),
}

/// The frame's recording, set aside while a kept row's slot is drawn again.
pub(super) struct Again {
    ops: Vec<u32>,
    clips: Vec<clip::Clip>,
    matrix: Option<[f32; 6]>,
    origin: (f32, f32),
    row: u32,
}

impl Recorder {
    pub(super) fn slot_open(&mut self, id: ViewId) {
        if self.slot.is_some() || !*SLOTS {
            return;
        }
        let Some(row) = &mut self.row else { return };
        row.slots.push((
            id,
            Context {
                clips: self.clips.clone(),
                origin: self.origin,
            },
        ));
        self.ops.extend([SLOT, id]);
        let row = std::mem::take(&mut self.ops);
        let emitted = self.clips.iter().map(|c| c.emitted()).collect();
        self.slot = Some((id, row, self.matrix.take(), emitted));
    }

    pub(super) fn slot_close(&mut self) {
        let Some((id, row, matrix, emitted)) = self.slot.take() else {
            return;
        };
        // Clips the picture wrote close inside its slot: outside, pending again.
        for i in (0..self.clips.len()).rev() {
            if self.clips[i].emitted() && !emitted.get(i).copied().unwrap_or(false) {
                self.ops.push(RESTORE);
                self.clips[i].reopen();
            }
        }
        let drawing = std::mem::replace(&mut self.ops, row);
        self.matrix = matrix;
        if self.slots.get(&id) != Some(&drawing) {
            self.slot_sets.extend([SLOT_SET, id, drawing.len() as u32]);
            self.slot_sets.extend(&drawing);
            self.slots.insert(id, drawing);
        }
        // Drawn again outside its row: the frame's recording comes back,
        // and the slot's new drawing goes into it here (no row ends).
        if let Some(again) = self.again.take() {
            self.ops = again.ops;
            self.clips = again.clips;
            self.matrix = again.matrix;
            self.origin = again.origin;
            let sets = std::mem::take(&mut self.slot_sets);
            self.ops.extend(sets);
        }
    }

    /// Begin image node `id`'s slot again, as kept row `row` recorded it:
    /// false when the row has no such slot (it is recorded whole instead).
    pub(super) fn slot_reopen(&mut self, row: u32, id: ViewId) -> bool {
        if self.row.is_some() || self.slot.is_some() || self.again.is_some() || !*SLOTS {
            return false;
        }
        let Some(context) = self
            .slot_contexts
            .get(&row)
            .and_then(|slots| slots.iter().find(|(slot, _)| *slot == id))
            .map(|(_, context)| context.clone())
        else {
            return false;
        };
        let emitted = context.clips.iter().map(|c| c.emitted()).collect();
        self.again = Some(Again {
            ops: std::mem::take(&mut self.ops),
            clips: std::mem::replace(&mut self.clips, context.clips),
            matrix: self.matrix.take(),
            origin: std::mem::replace(&mut self.origin, context.origin),
            row,
        });
        self.slot = Some((id, Vec::new(), None, emitted));
        true
    }

    /// A picture drawn in a slot outside its row: the row keeps it.
    pub(super) fn again_image(&mut self, key: usize) {
        if let Some(again) = &self.again {
            if let Some((_, images)) = self.kept.get_mut(&again.row) {
                images.push(key);
            }
        }
    }

    /// The slots row `id`'s recording holds, as it ends.
    pub(super) fn keep_slots(&mut self, id: u32, slots: Vec<(ViewId, Context)>) {
        if slots.is_empty() {
            self.slot_contexts.remove(&id);
        } else {
            self.slot_contexts.insert(id, slots);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::Backend;

    fn picture() -> Arc<Bitmap> {
        let gate = exact_raster::Gate::new();
        let charge = gate.session().reserve_allocation(400).unwrap().charge();
        Arc::new(Bitmap::new(Pixmap::new(10, 10).unwrap(), (10, 10), charge))
    }

    /// A row as the walk records one: a background, then a picture's slot
    /// inside a rounded card's clip, the picture in it when it has arrived.
    fn row(r: &mut Recorder, id: u32, previous: Option<u32>, image: Option<&Arc<Bitmap>>) -> u32 {
        let ts = Transform::identity();
        let card = Shape {
            rect: (16.0, 8.0, 300.0, 200.0),
            radii: [(12.0, 12.0); 4],
            corners: None,
        };
        r.row_begin(id, (0.0, 0.0), previous);
        r.fill(&Shape::rect((0.0, 0.0, 400.0, 220.0)), [255; 4], ts);
        r.push_clip(&card, ts);
        r.slot_begin(9);
        if let Some(image) = image {
            // Reaching past the card, whose clip is pending around it.
            let content = (16.0, 8.0, 320.0, 200.0);
            r.image(image, content, &[Shape::rect(content)], ts, None);
        }
        r.slot_end();
        r.pop_clip();
        r.row_end((0.0, 0.0, 400.0, 220.0))
    }

    /// The drawing the stream sets slot 9 to, if it sets it.
    fn set(ops: &[u32]) -> Option<&[u32]> {
        let at =
            (0..ops.len().saturating_sub(2)).rfind(|&i| ops[i] == SLOT_SET && ops[i + 1] == 9)?;
        ops.get(at + 3..at + 3 + ops[at + 2] as usize)
    }

    #[test]
    fn a_slot_drawn_again_without_its_row_is_the_slot_the_row_would_record() {
        let bitmap = picture();
        // The row recorded before its picture arrived, in two recorders.
        let (mut again, mut whole) = (Recorder::new(), Recorder::new());
        for r in [&mut again, &mut whole] {
            r.begin(400.0, 800.0, 2.0);
            assert_eq!(row(r, 1, None, None), 1);
        }
        assert_eq!(set(&again.ops), Some(&[][..]), "an empty slot");
        // The picture arrives. One draws the slot again, the row untouched;
        // the other records the row again and finds its body the same.
        again.begin(400.0, 800.0, 2.0);
        assert!(again.slot_again(1, 9));
        let content = (16.0, 8.0, 320.0, 200.0);
        let ts = Transform::identity();
        again.image(&bitmap, content, &[Shape::rect(content)], ts, None);
        again.slot_end();
        whole.begin(400.0, 800.0, 2.0);
        assert_eq!(
            row(&mut whole, 2, Some(1), Some(&bitmap)),
            1,
            "the row stands"
        );
        let drawn = set(&again.ops).expect("the slot is set");
        assert!(!drawn.is_empty(), "the picture, within the card");
        assert_eq!(Some(drawn), set(&whole.ops));
        // Nothing of a row was written, and the frame's own state is back.
        assert!(!again.ops.contains(&ROW_BEGIN));
        assert!(again.clips.is_empty() && again.origin == (0.0, 0.0));
        // The row keeps the picture it now draws.
        assert_eq!(again.kept[&1].1, whole.kept[&1].1);
    }

    #[test]
    fn a_slot_no_kept_row_holds_is_not_drawn_again() {
        let mut r = Recorder::new();
        r.begin(400.0, 800.0, 2.0);
        row(&mut r, 1, None, None);
        r.begin(400.0, 800.0, 2.0);
        assert!(!r.slot_again(1, 8), "no such slot in the row");
        assert!(!r.slot_again(2, 9), "no such row");
        r.row_free(1);
        assert!(!r.slot_again(1, 9), "the row was freed");
    }
}
