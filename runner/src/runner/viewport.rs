//! Host-owned viewport answers; never baked or carried device data.
//! @ref LLP 1039 D1–D3
use super::{DataError, DataSource, Runner, RunnerError};
use crate::viewport::{Viewport, SOURCE};
use exact_kernel::CommitReceipt;
use exact_plan::{TypeKind, Value};

impl<D: DataSource> Runner<D> {
    /// The current validated layout viewport.
    pub fn viewport(&self) -> Viewport {
        self.viewport
    }

    /// Re-answer every viewport resource in one commit. Invalid sizes are
    /// journaled without changing the fact or poisoning the runner.
    pub fn set_viewport(
        &mut self,
        width: f64,
        height: f64,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        let viewport = Viewport { width, height };
        if let Err(error) = viewport.validate() {
            let (width, height) = (exact_num::Shortest(width), exact_num::Shortest(height));
            self.log(format!("viewport {width} × {height} refused: {error:?}"));
            return Err(error);
        }
        if viewport == self.viewport {
            return Ok(None);
        }
        let previous = self.viewport;
        self.viewport = viewport;
        let which = (0..self.plan.resources.len())
            .filter(|i| self.plan.str(self.plan.resources[*i].source) == SOURCE)
            .collect();
        let result = self.recommit(which, "viewport");
        if result.is_err() {
            self.viewport = previous;
        }
        result
    }

    pub(super) fn viewport_answer(&self, i: usize) -> Result<Value, DataError> {
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
            match self.viewport.field(name) {
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
