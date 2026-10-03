//! The EXGAME v4 save: the entity table and every component storage as columnar
//! payloads (`data::columns`), resources as their values. The free list is the
//! table's dead slots; entity generations are stored once, in the table.
use crate::data::columns::{Columns, Rows};
use crate::data::Bulk;
use crate::data::{limits, BulkKind, MAX_LOAD_ENTITIES};
use crate::{Data, DataError, Reader, Writer};
use std::collections::BTreeSet;

#[derive(Default)]
pub(super) struct Free(pub BTreeSet<u32>);

impl Data for super::State {
    fn write(&self, w: &mut dyn Writer) {
        w.begin_struct();
        w.field("tick");
        self.tick.write(w);
        w.field("hz");
        self.hz.write(w);
        w.field("seed");
        self.seed.write(w);
        w.field("slots");
        let mut slots = Columns::default();
        for slot in &self.slots {
            slots.row(|w| slot.write(w));
        }
        w.bytes(Bulk::U8(&slots.finish()));
        w.field("busy");
        self.busy.borrow().write(w);
        w.end_struct();
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        r.begin_struct()?;
        while let Some(field) = r.field()? {
            match field.as_str() {
                "tick" => self.tick.read(r),
                "hz" => self.hz.read(r),
                "seed" => self.seed.read(r),
                "slots" => read_slots(r, self),
                "busy" => self.busy.get_mut().read(r),
                _ => r.skip(),
            }
            .map_err(|e| e.at(field))?;
        }
        Ok(())
    }
}
fn read_slots(r: &mut dyn Reader, state: &mut super::State) -> Result<(), DataError> {
    let bytes = r
        .bytes(BulkKind::U8)?
        .ok_or_else(|| DataError::new("expected a columnar entity table"))?
        .to_vec();
    let (_, mut rows) = Rows::decode(&bytes, MAX_LOAD_ENTITIES, false, r)?;
    state.slots.clear();
    state.free.0.clear();
    limits::reserve(r, &mut state.slots, rows.len())?;
    for i in 0..rows.len() {
        let slot = rows
            .read(r, |r| {
                let mut slot = super::Slot::default();
                slot.read(r).map(|()| slot)
            })
            .map_err(|e| e.at(i))?;
        if !slot.alive {
            r.claim(64)?;
            state.free.0.insert(i as u32);
        }
        state.slots.push(slot);
    }
    Ok(())
}
