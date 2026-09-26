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
    /// zone, in one commit; the same fact again commits nothing.
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
        let which = (0..self.plan.resources.len())
            .filter(|i| self.plan.str(self.plan.resources[*i].source) == SOURCE)
            .collect();
        let result = self.recommit(which, "place");
        if result.is_err() {
            self.place = previous;
        }
        result
    }

    /// Take the topics the source's native module announces (LLP 1016.002),
    /// calling `wake` from the announcing thread; the host then calls
    /// [`Runner::apply_announced`] on its own. A source with no native
    /// handle announces nothing.
    pub fn listen(&mut self, wake: std::sync::Arc<dyn Fn() + Send + Sync>) {
        let Some(native) = self.data.native() else {
            return;
        };
        let queue = self.announced.clone();
        native.on_changed(Some(std::sync::Arc::new(move |topic: &str| {
            let mut queue = queue.lock().unwrap_or_else(|e| e.into_inner());
            if !queue.iter().any(|t| t == topic) {
                queue.push(topic.to_owned());
            }
            drop(queue);
            wake();
        })));
    }

    /// Whether a topic was announced since the last [`Runner::apply_announced`].
    pub fn has_announced(&self) -> bool {
        !self
            .announced
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_empty()
    }

    /// Ask again every resource watching a topic announced since the last
    /// call: one commit per topic that anything watches. Announcements
    /// between two calls coalesce, so a fast producer costs one re-ask.
    pub fn apply_announced(&mut self) -> (Vec<CommitReceipt>, Option<RunnerError>) {
        let topics = std::mem::take(&mut *self.announced.lock().unwrap_or_else(|e| e.into_inner()));
        let mut receipts = Vec::new();
        for topic in topics {
            match self.changed(&topic) {
                Ok(Some(receipt)) => receipts.push(receipt),
                Ok(None) => {}
                Err(error) => return (receipts, Some(error)),
            }
        }
        (receipts, None)
    }

    /// The device says `topic` changed (LLP 1016.002): every resource whose
    /// answer watches it is asked again, in one commit; none, no commit.
    pub fn changed(&mut self, topic: &str) -> Result<Option<CommitReceipt>, RunnerError> {
        let which = (0..self.watching.len())
            .filter(|i| self.watching[*i].iter().any(|t| t == topic))
            .collect();
        self.recommit(which, "changed")
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
