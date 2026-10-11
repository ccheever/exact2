//! Host-owned surface facts; never baked or carried device data.
use super::{DataError, DataSource, Runner, RunnerError};
use crate::surface_record::{oversize, surface_name, MAX_BYTES, SOURCE};
use exact_kernel::CommitReceipt;

impl<D: DataSource> Runner<D> {
    /// Replace a named surface's current record, or clear it on disposal.
    /// Only resources reading this name are asked again; a refusal rolls back,
    /// is logged, and stands in [`Runner::surface_refusals`] until a record
    /// for that surface is accepted or the surface goes away.
    pub fn set_surface_record(
        &mut self,
        name: &str,
        json: Option<&str>,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        let result = self.replace_surface_record(name, json);
        match &result {
            Err(RunnerError::Data {
                resource,
                error:
                    DataError::Unavailable(why)
                    | DataError::BadArguments(why)
                    | DataError::UnknownSource(why)
                    | DataError::Interface(why)
                    | DataError::Failed(_, why),
            }) => self
                .surface_refusals
                .insert(name.to_owned(), format!("{resource}: {why}")),
            Err(other) => self
                .surface_refusals
                .insert(name.to_owned(), format!("{other:?}")),
            Ok(_) => self.surface_refusals.remove(name),
        };
        result
    }

    /// Each surface whose latest record was refused, with why (agent `state`).
    pub fn surface_refusals(&self) -> &exact_kernel::SortedMap<String, String> {
        &self.surface_refusals
    }

    fn replace_surface_record(
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
        if let Some(bytes) = json.map(str::len).filter(|n| *n > MAX_BYTES) {
            let resource: String = self.plan.str(self.plan.resources[which[0]].name).into();
            let why = oversize(bytes);
            self.log(format!("surface {name} refused: {resource}: {why}"));
            return Err(RunnerError::Data {
                resource,
                error: DataError::Unavailable(why),
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
