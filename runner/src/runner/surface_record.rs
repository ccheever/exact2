//! Host-owned surface facts; never baked or carried device data.
use super::{DataError, DataSource, Runner, RunnerError};
use crate::surface_record::{decode, surface_name, SOURCE};
use exact_kernel::CommitReceipt;
use exact_plan::Value;

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
        let previous = match json {
            Some(text) => self
                .surface_records
                .insert(name.to_owned(), text.to_owned()),
            None => self.surface_records.remove(name),
        };
        let which = self
            .plan
            .resources
            .iter()
            .enumerate()
            .filter_map(|(i, row)| {
                (self.plan.str(row.source) == SOURCE && surface_name(&self.plan, row) == Some(name))
                    .then_some(i)
            })
            .collect();
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
