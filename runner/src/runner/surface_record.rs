//! Host-owned surface facts; never baked or carried device data.
use super::{DataError, DataSource, Runner, RunnerError};
use crate::surface_record::{surface_name, MAX_BYTES, SOURCE};
use exact_kernel::CommitReceipt;

impl<D: DataSource> Runner<D> {
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
}
