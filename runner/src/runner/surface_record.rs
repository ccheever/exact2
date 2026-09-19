//! Host-owned surface facts; never baked or carried device data.
use super::{DataError, DataSource, Runner, RunnerError};
use crate::surface_record::{decode, surface_name, MAX_BYTES, SOURCE};
use exact_kernel::CommitReceipt;
use exact_plan::Value;

impl<D: DataSource> Runner<D> {
    /// Isolate current state for a synchronous host-fact publication transaction.
    /// No boot, resource query, identity allocation or row initializer runs. The
    /// data source is inert here; the host retains its original instance on commit.
    pub fn fork_surface_records(
        &self,
        data: D,
        measurer: Box<dyn exact_kernel::TextMeasurer>,
    ) -> Result<Self, &'static str> {
        if self.has_pending()
            || !self.plan.mutations.is_empty()
            || self
                .plan
                .resources
                .iter()
                .any(|row| !exact_plan::runner_owned_source(self.plan.str(row.source)))
        {
            return Err(
                "surface staging cannot execute external resources or mutations; restart required",
            );
        }
        Ok(Self {
            plan: self.plan.clone(),
            data,
            kernel: self.kernel.rehydrate(measurer),
            slots: self.slots.clone(),
            derives: self.derives.clone(),
            resources: self.resources.clone(),
            resource_values: self.resource_values.clone(),
            tree: self.tree.clone(),
            ids: self.ids.clone(),
            now_ms: self.now_ms,
            timers: self.timers.clone(),
            batch: self.batch,
            commands: self.commands.clone(),
            surfaces: self.surfaces.clone(),
            pending: self.pending.clone(),
            pending_res: self.pending_res.clone(),
            pending_mut: self.pending_mut.clone(),
            next_ticket: self.next_ticket,
            requests: self.requests.clone(),
            refresh_next: self.refresh_next.clone(),
            store: self.store.clone(),
            store_readers: self.store_readers.clone(),
            stale: self.stale.clone(),
            keeps_answers: self.keeps_answers,
            poisoned: self.poisoned,
            delivery: self.delivery.clone(),
            viewport: self.viewport,
            surface_records: self.surface_records.clone(),
            router: self.router.clone(),
            journal: self.journal.clone(),
            journal_start: self.journal_start,
        })
    }

    /// Replace a named surface's current record, or clear it on disposal.
    /// Only resources reading this name are asked again; a refusal rolls back.
    pub fn set_surface_record(
        &mut self,
        name: &str,
        json: Option<&str>,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        if self.surface_records.get(name).map(String::as_str) == json {
            return Ok(None);
        }
        let which: Vec<_> = self
            .plan
            .resources
            .iter()
            .enumerate()
            .filter_map(|(i, row)| {
                (self.plan.str(row.source) == SOURCE && surface_name(&self.plan, row) == Some(name))
                    .then_some(i)
            })
            .collect();
        if which.is_empty() {
            return Ok(None);
        }
        if json.is_some_and(|text| text.len() > MAX_BYTES) {
            return Err(RunnerError::Data {
                resource: self.plan.str(self.plan.resources[which[0]].name).into(),
                error: DataError::Unavailable("record exceeds 64 KiB".into()),
            });
        }
        let previous = match json {
            Some(text) => self
                .surface_records
                .insert(name.to_owned(), text.to_owned()),
            None => self.surface_records.remove(name),
        };
        let result = self.recommit(which, &format!("surface {name}"));
        if result.is_err() {
            match previous {
                Some(text) => {
                    self.surface_records.insert(name.to_owned(), text);
                }
                None => {
                    self.surface_records.remove(name);
                }
            }
        }
        result
    }

    pub(super) fn surface_answer(&self, i: usize) -> Result<Value, DataError> {
        let row = &self.plan.resources[i];
        let name = surface_name(&self.plan, row).ok_or_else(|| {
            DataError::BadArguments(format!(
                "{SOURCE} takes exactly one string-literal surface name"
            ))
        })?;
        decode(
            &self.plan,
            row.ty,
            self.surface_records.get(name).map(String::as_str),
        )
        .map_err(DataError::Unavailable)
    }
}
