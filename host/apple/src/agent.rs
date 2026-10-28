//! Read operations over the Apple host's settled layout and runner, and
//! the `then`s an input settled.
use super::*;
use std::fmt::Write;

impl<D: DataSource> Host<D> {
    /// The `then`s an agent's input settled, the clock unmoved
    /// ([`exact_runner::Runner::land_then`]); `first`, only when one is the
    /// earliest work due ([`exact_runner::Runner::land_then_first`]).
    pub fn land_then(&mut self, first: bool) -> String {
        let a = if first {
            self.runner.land_then_first()
        } else {
            self.runner.land_then()
        };
        self.advanced(a)
    }

    /// The agent API's read operations (LLP 1012): `tree`, `state`, and
    /// `logs` from the runner; `settle` — the clock at which the last
    /// transition in flight ends, milliseconds, `null` when quiescent — from
    /// the engine (and a resource's re-ask), which is what the presenter's
    /// `clock` advances to.
    pub fn agent(&self, request: &str) -> String {
        if exact_runner::agent::field_str(request, "op").as_deref() == Some("settle") {
            let engine = self.engine.settle_time().map(|t| t * 1000.0);
            return match engine
                .into_iter()
                .chain(self.runner.text_axes_settle_ms())
                // A re-ask after a turn over its budget: no I/O to wait on, so
                // the clock goes to it (exact-runner's `overrun.rs`).
                .chain(self.runner.reask_due_ms())
                // An answer waiting on its own timer (`data_timers.rs`): the clock goes to it too.
                .chain(self.runner.data_timer_due_ms())
                .reduce(f64::max)
            {
                Some(t) => format!("{{\"settle\":{}}}", exact_runner::agent::num(t)),
                None => "{\"settle\":null}".to_string(),
            };
        }
        // A native content region's frames are the host's, not the kernel's
        // (LLP 1080.001 D2): `layout agree` must not compare them.
        if exact_runner::agent::field_str(request, "op").as_deref() == Some("frames") {
            return exact_runner::agent::frames(&self.runner, request, &|id| {
                self.native_protected_id(id)
            });
        }
        let mut reply = exact_runner::agent::handle(&self.runner, request);
        if exact_runner::agent::field_str(request, "op").as_deref() == Some("state")
            && reply.ends_with('}')
        {
            reply.pop();
            let _ = write!(
                reply,
                ",\"kernelLayout\":{{\"provisionalLayouts\":{}}}}}",
                self.runner.kernel().provisional_layouts()
            );
        }
        reply
    }

    /// The agent's `sensorEvents` (LLP 1133.013 D6): what the substitute's
    /// canvas inputs owe their surfaces up to `to`, which the Swift agent
    /// hands each surface before it steps the worlds a frame; `inputs` is
    /// false when no input runs or has an end still to deliver.
    pub(crate) fn sensor_events(&mut self, request: &str) -> String {
        let to = exact_runner::agent::field_num(request, "to").unwrap_or(0.0);
        let inputs = self.runner.surface_sensor_inputs();
        let events: Vec<String> = if inputs {
            (self.runner.surface_sensor_events(to).into_iter())
                .map(|(surface, event)| {
                    let surface = serde_json::to_string(&surface).unwrap_or_default();
                    format!("{{\"surface\":{surface},\"event\":{event}}}")
                })
                .collect()
        } else {
            Vec::new()
        };
        format!("{{\"inputs\":{inputs},\"events\":[{}]}}", events.join(","))
    }
}
