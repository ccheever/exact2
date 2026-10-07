//! Host-owned page answers; never baked or carried device data.
//! @ref LLP 1069.000 D2
use super::{DataError, DataSource, Runner, RunnerError};
use crate::page::{Page, SOURCE};
use exact_kernel::CommitReceipt;
use exact_plan::{TypeKind, Value};

impl<D: DataSource> Runner<D> {
    /// What the host last said about the page.
    pub fn page(&self) -> Page {
        self.page
    }

    /// The page's visibility, connectivity, share sheet, pickers or focus
    /// changed: re-answer, in one commit, every `exactPage` resource whose
    /// shape reads a field that changed; the same facts again, or no such
    /// reader, commit nothing. Focus moves each time the person switches
    /// windows, and asks nothing of a reader of `onLine` alone (#114). Size
    /// readers are not asked.
    pub fn set_page(&mut self, page: Page) -> Result<Option<CommitReceipt>, RunnerError> {
        if page == self.page {
            return Ok(None);
        }
        let previous = std::mem::replace(&mut self.page, page);
        let which = (0..self.plan.resources.len())
            .filter(|i| self.plan.str(self.plan.resources[*i].source) == SOURCE)
            .filter(|i| self.page_reads_changed(*i, previous))
            .collect();
        let result = self.recommit(which, "page");
        if result.is_err() {
            self.page = previous;
        }
        result
    }

    /// Whether resource `i`'s shape names a field `previous` answered
    /// otherwise; a shape that is no record is asked again, to be refused.
    fn page_reads_changed(&self, i: usize, previous: Page) -> bool {
        let ty = self.plan.type_(self.plan.resources[i].ty);
        ty.kind != TypeKind::Record
            || ty.fields.iter().any(|f| {
                let name = self.plan.str(self.plan.field(f).name);
                previous.field(name) != self.page.field(name)
            })
    }

    pub(super) fn page_answer(&self, i: usize) -> Result<Value, DataError> {
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
            match self.page.field(name) {
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
