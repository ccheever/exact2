use super::*;
use crate::data::{limits, MAX_LOAD_ENTITIES};

#[derive(Default)]
pub(super) struct Free(pub BTreeSet<u32>);
impl Data for Free {
    fn write(&self, w: &mut dyn Writer) {
        w.begin_seq(self.0.len());
        for i in &self.0 {
            w.item();
            i.write(w);
        }
        w.end_seq();
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        r.begin_seq()?;
        if r.sequence_len().is_some_and(|n| n > MAX_LOAD_ENTITIES) {
            return Err(DataError::new("free count exceeds entity limit"));
        }
        if let Some(count) = r.sequence_len() {
            r.check_allocation(
                count
                    .checked_mul(64)
                    .ok_or_else(|| DataError::new("allocation size overflow"))?,
            )?;
        }
        self.0.clear();
        let mut last = None;
        while r.item()? {
            if self.0.len() == MAX_LOAD_ENTITIES {
                return Err(DataError::new("free count exceeds entity limit"));
            }
            let mut i = 0u32;
            i.read(r)?;
            if last.is_some_and(|old| old >= i) {
                return Err(DataError::new("free list is not strictly ordered"));
            }
            r.claim(64)?;
            self.0.insert(i);
            last = Some(i);
        }
        Ok(())
    }
}
impl Data for State {
    fn write(&self, w: &mut dyn Writer) {
        w.begin_struct();
        w.field("tick");
        self.tick.write(w);
        w.field("hz");
        self.hz.write(w);
        w.field("seed");
        self.seed.write(w);
        w.field("slots");
        self.slots.write(w);
        w.field("free");
        self.free.write(w);
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
                "slots" => limits::read_vec(r, &mut self.slots, MAX_LOAD_ENTITIES),
                "free" => self.free.read(r),
                "busy" => self.busy.get_mut().read(r),
                _ => r.skip(),
            }
            .map_err(|e| e.at(field))?;
        }
        Ok(())
    }
}
