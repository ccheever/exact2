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

    /// The language of the displayed strings. Without tables, language is
    /// unknown (HTML's empty `lang`), rather than the viewer's preferred locale.
    pub fn resolved_locale(&self) -> &str {
        self.plan
            .locale
            .and_then(|slot| self.slots.get(slot.0 as usize))
            .and_then(Value::as_str)
            .filter(|name| {
                self.plan
                    .locales
                    .iter()
                    .any(|row| self.plan.str(row.name) == *name)
            })
            .or_else(|| self.plan.locales.first().map(|row| self.plan.str(row.name)))
            .unwrap_or("")
    }

    /// CSS direction of the resolved table, compiled from CLDR likely subtags.
    pub fn direction(&self) -> &'static str {
        if self
            .plan
            .locales
            .iter()
            .any(|row| row.rtl && self.plan.str(row.name) == self.resolved_locale())
        {
            "rtl"
        } else {
            "ltr"
        }
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

    /// What the host last said about the viewer and this launch.
    pub fn place(&self) -> &Place {
        &self.place
    }

    /// Re-answer every `exactTime` resource with the viewer's locale and
    /// zone, and write the table that locale resolves to into the plan's
    /// locale slot, in one commit; the same fact again commits nothing.
    /// The launch seed arrives with them; `None` keeps the current seed.
    pub fn set_place(
        &mut self,
        locale: &str,
        time_zone: &str,
        seed: Option<f64>,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        let place = Place {
            locale: locale.into(),
            time_zone: time_zone.into(),
            seed: seed.unwrap_or(self.place.seed),
        };
        if let Err(error) = place.validate() {
            self.log(format!("place refused: {error:?}"));
            return Err(error);
        }
        let changed = place != self.place;
        let previous = std::mem::replace(&mut self.place, place);
        // @ref LLP 1060 D4 — an ordinary slot write, so dependency tracking
        // re-renders exactly the `t(...)` readers. The checkpoint the commit
        // takes already holds the new value: a refusal restores it here.
        let written = self.plan.locale.and_then(|slot| {
            let i = slot.0 as usize;
            let resolved = self.plan.resolve_locale(&self.place.locale)?;
            let value = Value::str(resolved);
            (self.slots[i] != value).then(|| (i, std::mem::replace(&mut self.slots[i], value)))
        });
        let which: Vec<usize> = (0..self.plan.resources.len())
            .filter(|i| {
                (changed || written.is_some())
                    && self.plan.str(self.plan.resources[*i].source) == SOURCE
            })
            .collect();
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
            match self
                .time
                .field(name)
                .or_else(|| self.place.field(name))
                .or_else(|| (name == "resolvedLocale").then(|| Value::str(self.resolved_locale())))
            {
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
