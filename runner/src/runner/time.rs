//! Host-owned date answers; never baked or carried.
//! @ref LLP 1027.000.000
use super::{DataError, DataSource, Runner, RunnerError};
use crate::time::{Place, WallTime, SOURCE};
use exact_kernel::CommitReceipt;
use exact_plan::{TypeKind, Value};

impl<D: DataSource> Runner<D> {
    /// What the host last said about the date.
    pub fn wall_time(&self) -> WallTime {
        self.time
    }

    /// Re-answer every `exactTime` resource in one commit. An invalid fact
    /// is journaled without changing anything.
    pub fn set_time(
        &mut self,
        epoch_at_zero: f64,
        utc_offset: f64,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        let time = WallTime {
            epoch_at_zero,
            utc_offset,
        };
        if let Err(error) = time.validate() {
            self.log(format!("time refused: {error:?}"));
            return Err(error);
        }
        // A clock that drifts by less than a second is the same fact.
        if (time.epoch_at_zero - self.time.epoch_at_zero).abs() < 1000.0
            && time.utc_offset == self.time.utc_offset
        {
            return Ok(None);
        }
        let previous = self.time;
        self.time = time;
        let which = (0..self.plan.resources.len())
            .filter(|i| self.plan.str(self.plan.resources[*i].source) == SOURCE)
            .collect();
        let result = self.recommit(which, "time");
        if result.is_err() {
            self.time = previous;
        }
        result
    }

    /// Re-answer every `exactTime` resource with the viewer's locale and
    /// zone, and write the table that locale resolves to into the plan's
    /// locale slot, in one commit; the same fact again commits nothing.
    pub fn set_place(
        &mut self,
        locale: &str,
        time_zone: &str,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        let place = Place {
            locale: locale.into(),
            time_zone: time_zone.into(),
        };
        if let Err(error) = place.validate() {
            self.log(format!("place refused: {error:?}"));
            return Err(error);
        }
        if place == self.place {
            return Ok(None);
        }
        let previous = std::mem::replace(&mut self.place, place);
        let which: Vec<usize> = (0..self.plan.resources.len())
            .filter(|i| self.plan.str(self.plan.resources[*i].source) == SOURCE)
            .collect();
        // @ref LLP 1060 D4 — an ordinary slot write, so dependency tracking
        // re-renders exactly the `t(...)` readers. The checkpoint the commit
        // takes already holds the new value: a refusal restores it here.
        let written = self.plan.locale.and_then(|slot| {
            let i = slot.0 as usize;
            let resolved = self.plan.resolve_locale(&self.place.locale)?;
            let value = Value::str(resolved);
            (self.slots[i] != value).then(|| (i, std::mem::replace(&mut self.slots[i], value)))
        });
        if which.is_empty() && written.is_none() {
            return Ok(None);
        }
        let result = self.commit_again(which, "place");
        if result.is_err() {
            self.place = previous;
            if let Some((i, old)) = written {
                self.slots[i] = old;
            }
        }
        result.map(Some)
    }

    pub(super) fn time_answer(&self, i: usize) -> Result<Value, DataError> {
        let row = &self.plan.resources[i];
        if row.args.len > 0 {
            return Err(DataError::BadArguments(format!(
                "{SOURCE} takes no arguments"
            )));
        }
        let ty = self.plan.type_(row.ty);
        if ty.kind != TypeKind::Record {
            return Err(DataError::Unavailable(format!(
                "{SOURCE} answers a record; this one is declared `{}`",
                self.plan.str(ty.name)
            )));
        }
        let mut fields = Vec::with_capacity(ty.fields.len as usize);
        for f in ty.fields.iter() {
            let name = self.plan.str(self.plan.field(f).name);
            match self.time.field(name).or_else(|| self.place.field(name)) {
                Some(v) => fields.push(v),
                None => {
                    return Err(DataError::Unavailable(format!(
                        "{SOURCE} has no field `{name}`"
                    )))
                }
            }
        }
        Ok(Value::record(fields))
    }
}
