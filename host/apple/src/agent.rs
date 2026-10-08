//! Read operations over the Apple host's settled layout and runner.
use super::*;
use std::fmt::Write;

impl<D: DataSource> Host<D> {
    /// The agent API's read operations (LLP 1012): `tree`, `state`, and
    /// `logs` from the runner; `settle` — the clock at which the last
    /// transition in flight ends, milliseconds, `null` when quiescent — from
    /// the engine, which is what the presenter's `clock` advances to.
    pub fn agent(&self, request: &str) -> String {
        if exact_runner::agent::field_str(request, "op").as_deref() == Some("settle") {
            return match self.engine.settle_time() {
                Some(t) => format!("{{\"settle\":{}}}", exact_runner::agent::num(t * 1000.0)),
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
}
