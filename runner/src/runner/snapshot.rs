//! Snapshot refresh is separate from rendering (LLP 1126 D3/D8).
use super::{DataSource, Runner};
use crate::{Answer, Store, Value};

impl<D: DataSource> Runner<D> {
    /// Stage 1 never writes the store: refresh returns props only, so there is
    /// no second writer. A private store copy also prevents a failed refresh
    /// from changing the render's snapshot. Async refresh awaits a later chunk.
    pub fn snapshot_timeline(
        &mut self,
        now: f64,
        snapshot: Vec<(String, Vec<u8>)>,
    ) -> Result<String, String> {
        if !now.is_finite() {
            return Err("non-finite timeline date".into());
        }
        let source = self.plan.snapshot_refresh.ok_or("not a snapshot entry")?;
        let signature = self.plan.source(source);
        let name = self.plan.str(signature.name).to_owned();
        let ty = signature.ty;
        self.data
            .activate()
            .map_err(|e| format!("refresh activation: {e:?}"))?;
        let mut store = Store::new(self.data.grants(), snapshot);
        let answer = self
            .data
            .answer(&mut store, &name, &[Value::Number(now)])
            .map_err(|e| format!("refresh {name}: {e:?}"))?;
        if !store.take_writes().is_empty() {
            return Err("snapshot refresh cannot write the store in stage 1".into());
        }
        let value = match answer {
            Answer::Now(value) => value,
            Answer::Later(request) => {
                if let Some(token) = request.continuation {
                    self.data.discard(token);
                }
                return Err("asynchronous widget refresh is not implemented".into());
            }
        };
        if !value.conforms(&self.plan, ty) {
            return Err(format!(
                "refresh {name} returned a value outside its declared timeline type"
            ));
        }
        let mut json = String::new();
        crate::agent::typed_json(&self.plan, ty, &value, &mut json);
        Ok(json)
    }
}
