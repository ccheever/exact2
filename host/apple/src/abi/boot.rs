//! One session's boots from the input buffer: prepare, then commit.

use super::{Bridge, Hooks};
use exact_runner::DataSource;

impl<D: DataSource> Bridge<D> {
    /// Prepare and commit a single session's plan replacement.
    pub fn boot_plan(&mut self, len: usize, data: D, hooks: Hooks, width: f32, height: f32) -> u32 {
        let len = self.prepare_plan(len, data, hooks, width, height);
        if self.prepared.is_some() {
            self.commit_plan()
        } else {
            len
        }
    }
    /// Gallery refresh uses a fresh data seam and an empty, disposable store.
    /// Host activation would reset the running app's picked-file roots.
    pub fn refresh_entry(&mut self, mut data: D, now: f64) -> u32 {
        use exact_runner::{Answer, Store, Value};
        if !now.is_finite() {
            return self.refuse("entry refresh date must be finite");
        }
        let Some(host) = &self.host else {
            return self.emit(super::not_booted());
        };
        let plan = host.runner().plan().clone();
        let Some(source) = plan.snapshot_refresh else {
            return self.refuse("not a snapshot entry");
        };
        let signature = plan.source(source);
        data.bind(&plan);
        let mut store = Store::new(data.grants(), []);
        let answer = exact_runner::time::launching_with(self.launch_facts.clone(), || {
            data.activate()?;
            data.answer(&mut store, plan.str(signature.name), &[Value::Number(now)])
        });
        let result = match answer {
            Ok(Answer::Now(value)) if value.conforms(&plan, signature.ty) => {
                let mut json = String::new();
                exact_runner::agent::typed_json(&plan, signature.ty, &value, &mut json);
                serde_json::json!({"entryRefresh":"timeline", "timeline":serde_json::from_str::<serde_json::Value>(&json).unwrap()})
            }
            Ok(Answer::Now(_)) => {
                return self.refuse("entry refresh does not conform to its timeline type")
            }
            Ok(Answer::Later(request)) => {
                if let Some(token) = request.continuation {
                    data.discard(token);
                }
                serde_json::json!({"entryRefresh":"pending"})
            }
            Err(error) => {
                serde_json::json!({"entryRefresh":"unavailable", "reason":error.to_string()})
            }
        };
        self.emit(result.to_string())
    }
}
