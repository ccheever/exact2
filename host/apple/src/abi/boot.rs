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

    /// `exact_boot_entry`: boot a snapshot entry's plan (LLP 1126 D2) from the
    /// input buffer, the plan's `len` bytes followed by its kind's `kind_len`
    /// bytes. The headless session's boot: a running host refuses the plan.
    pub fn boot_entry(
        &mut self,
        len: usize,
        kind_len: usize,
        data: D,
        hooks: Hooks,
        width: f32,
        height: f32,
    ) -> u32 {
        let end = (len + kind_len).min(self.input.len());
        let kind = match std::str::from_utf8(&self.input[len.min(end)..end]) {
            Ok(kind) => kind.to_string(),
            Err(_) => return self.refuse_preparation("entry kind is not UTF-8"),
        };
        let len =
            self.prepare_plan_carried(len, data, hooks, width, height, None, None, Some(&kind));
        if self.prepared.is_some() {
            self.commit_plan()
        } else {
            len
        }
    }
    /// Refresh a snapshot entry through its declared data source. Only an
    /// immediate, typed first timeline value replaces the preview props.
    pub fn refresh_entry(
        &mut self,
        data: D,
        hooks: Hooks,
        now: f64,
        width: f32,
        height: f32,
    ) -> u32 {
        if !now.is_finite() {
            return self.refuse_preparation("entry refresh date must be finite");
        }
        use exact_plan::Value;
        use exact_runner::{Answer, Store};
        let Some(host) = self.host.as_mut() else {
            return self.emit(super::not_booted());
        };
        let plan = host.runner().plan().clone();
        let Some(source) = plan.snapshot_refresh else {
            return self.refuse_preparation("not a snapshot entry");
        };
        let signature = plan.source(source);
        let grants = host.runner_mut().data().grants().to_string();
        let mut store = Store::new(&grants, host.runner().store().snapshot());
        let answer =
            crate::host::Host::<D>::activate_source(host.runner_mut().data()).and_then(|()| {
                exact_runner::time::launching_with(self.launch_facts.clone(), || {
                    host.runner_mut().data().answer(
                        &mut store,
                        plan.str(signature.name),
                        &[Value::Number(now)],
                    )
                })
            });
        let value = match answer {
            Ok(Answer::Now(value)) if value.conforms(&plan, signature.ty) => value,
            Ok(Answer::Now(_)) => {
                return self
                    .refuse_preparation("entry refresh does not conform to its timeline type")
            }
            Ok(Answer::Later(request)) => {
                if let Some(token) = request.continuation {
                    host.runner_mut().data().discard(token);
                }
                self.output = br#"{"entryRefresh":"pending"}"#.to_vec();
                return self.output.len() as u32;
            }
            Err(error) => {
                self.output = format!(
                    "{{\"entryRefresh\":\"unavailable\",\"reason\":\"{}\"}}",
                    super::escape(&format!("{error:?}"))
                )
                .into_bytes();
                return self.output.len() as u32;
            }
        };
        let Value::List(entries) = &value else {
            return self.refuse_preparation("entry refresh is not a timeline");
        };
        let Some(Value::Record(first)) = entries.first() else {
            self.output = br#"{"entryRefresh":"empty"}"#.to_vec();
            return self.output.len() as u32;
        };
        let [Value::Number(date), props] = first.as_ref() else {
            return self.refuse_preparation("invalid timeline entry");
        };
        let mut carried = host.carry();
        carried.slots.retain(|(name, _)| name != "snapshot_props");
        carried.slots.push(("snapshot_props".into(), props.clone()));
        let bytes = plan.encode();
        self.input = bytes;
        let n = self.prepare_plan_carried(
            self.input.len(),
            data,
            hooks,
            width,
            height,
            None,
            Some(carried),
            Some(&plan.snapshot_entry),
        );
        if self.prepared.is_none() {
            return n;
        }
        self.commit_plan();
        let mut timeline = String::new();
        exact_runner::agent::typed_json(&plan, signature.ty, &value, &mut timeline);
        self.output.pop();
        self.output.extend_from_slice(
            format!(
                ",\"entryRefresh\":\"timeline\",\"entryDate\":{date},\"timeline\":{timeline}}}"
            )
            .as_bytes(),
        );
        self.output.len() as u32
    }
}
