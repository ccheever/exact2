//! `rangeFrame` (LLP 1114.007): a text field's UTF-16 range as the field
//! was last painted, read by the runner while an action runs. The presenter
//! publishes each painted field whenever its boxes change; the runner's
//! reader holds the same cell, so a read never reaches into the presenter.
use super::*;
use crate::paint::field::PaintedField;
use exact_runner::geometry::{GeometryAnswer, InlineGeometry};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

/// The fields last painted, by view, with whether a perspective placed
/// each (its box is no rectangle's bounds, so it is not read); and the
/// runner they belong to: `session` counts runners (`replaced`), `shown`
/// is the one whose paint the presenter shows now. A new runner reuses
/// view ids, so a picture painted for an older one is never read.
#[derive(Debug, Default)]
pub(crate) struct Painted {
    fields: RefCell<HashMap<ViewId, (Rc<PaintedField>, bool)>>,
    session: Cell<u64>,
    pub(crate) shown: Cell<u64>,
}

impl Painted {
    /// The runner painting now.
    pub(crate) fn session(&self) -> u64 {
        self.session.get()
    }
}

impl std::fmt::Debug for PaintedField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PaintedField")
    }
}

impl InlineGeometry for Painted {
    // Linux reads no shaped inline run's fragments: `frame` stays the kernel's.
    fn frame(&self, _view: ViewId) -> GeometryAnswer {
        GeometryAnswer::UNAVAILABLE
    }

    fn range_frame(&self, view: ViewId, start: u32, end: u32) -> GeometryAnswer {
        let fields = self.fields.borrow();
        let Some((x, y, width, height)) = fields
            .get(&view)
            .filter(|(_, projected)| !projected)
            .and_then(|(field, _)| field.range_frame(start, end))
        else {
            return GeometryAnswer::UNAVAILABLE;
        };
        GeometryAnswer {
            x: f64::from(x),
            y: f64::from(y),
            width: f64::from(width),
            height: f64::from(height),
            provisional: false,
            unavailable: false,
        }
    }
}

impl<D: DataSource> Presenter<D> {
    /// Hand a new runner the reader, which reads nothing it has not
    /// painted. Android's fields are the platform's own, so it has none.
    pub(crate) fn read_ranges(&mut self) {
        let ranges = &self.ranges;
        ranges.session.set(ranges.session.get() + 1);
        ranges.fields.borrow_mut().clear();
        let read: Option<Rc<dyn InlineGeometry>> = if cfg!(target_os = "android") {
            None
        } else {
            Some(self.ranges.clone())
        };
        self.host.runner_mut().set_inline_geometry(read);
    }

    /// The painted fields beside the boxes now shown, when they are this
    /// runner's: a hosted field (the shell's own) is never painted here.
    pub(crate) fn publish_ranges(&self) {
        let mut fields = self.ranges.fields.borrow_mut();
        fields.clear();
        if self.ranges.shown.get() != self.ranges.session.get() {
            return;
        }
        // A field with no box beside it (a failed paint that dropped the
        // boxes) is not read: whether a perspective placed it is unknown.
        for (id, field) in &self.brush.field_text {
            if let Some(b) = self.boxes.iter().find(|b| b.id == *id) {
                fields.insert(*id, (field.clone(), b.projected()));
            }
        }
    }
}

#[cfg(test)]
#[path = "range_frame_tests.rs"]
mod range_frame_tests;
