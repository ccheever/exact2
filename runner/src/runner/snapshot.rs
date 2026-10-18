//! Snapshot refresh is separate from rendering (LLP 1126 D3/D8).
use super::{DataSource, Runner};
use crate::{Answer, Dispatch, Outcome, Request, Store, Value, Work};
use std::time::Instant;

impl<D: DataSource> Runner<D> {
    /// Stage 1 never writes the store: refresh returns props only, so there is
    /// no second writer. Each awaited request uses the host's existing carrier.
    pub fn snapshot_timeline(
        &mut self,
        now: f64,
        snapshot: Vec<(String, Vec<u8>)>,
        deadline: Instant,
        mut execute: impl FnMut(Request, Option<Work>) -> Result<Outcome, String>,
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
        let mut store = Store::new_linked(self.data.grants(), snapshot, self.links.grants);
        let args = [Value::Number(now)];
        let mut answer = self.data.answer(&mut store, &name, &args);
        let result = (|| {
            // Bound chains too: an immediate continuation must not spin forever.
            for _ in 0..128 {
                if Instant::now() >= deadline {
                    return Err("widget refresh timed out".into());
                }
                let step = answer.map_err(|e| format!("refresh {name}: {e:?}"))?;
                if !store.take_writes().is_empty() {
                    if let Answer::Later(request) = step {
                        if let Some(token) = request.continuation {
                            self.data.discard(token);
                        }
                    }
                    return Err("snapshot refresh cannot write the store in stage 1".into());
                }
                match step {
                    Answer::Now(value) => {
                        if !value.conforms(&self.plan, ty) {
                            return Err(format!("refresh {name} returned a value outside its declared timeline type"));
                        }
                        let mut json = String::new();
                        crate::agent::typed_json(&self.plan, ty, &value, &mut json);
                        return Ok(json);
                    }
                    Answer::Later(request) => {
                        // Native UI, streams and portable write requests have no
                        // stage-1 snapshot owner. JS file reads use continuations
                        // whose storage context was endowed read-only before load.
                        if request.stream
                            || request.surface.is_some()
                            || request.storage.is_some()
                            || (request.continuation.is_none()
                                && !request.url.starts_with("https://")
                                && !request.url.starts_with("http://"))
                        {
                            if let Some(token) = request.continuation {
                                self.data.discard(token);
                            }
                            return Err("snapshot refresh supports fetch and read-only storage continuations".into());
                        }
                        let work = if let Some(token) = request.continuation {
                            match self.data.dispatch(token, &store) {
                                Dispatch::Run(work) => Some(work),
                                Dispatch::Again => {
                                    answer = self.data.parse(
                                        &mut store,
                                        &name,
                                        &args,
                                        Dispatch::again_outcome(),
                                    );
                                    continue;
                                }
                                _ => {
                                    self.data.discard(token);
                                    return Err("snapshot continuation is unavailable".into());
                                }
                            }
                        } else {
                            None
                        };
                        let token = request.continuation;
                        let outcome = match execute(request, work) {
                            Ok(outcome) => outcome,
                            Err(error) => {
                                if let Some(token) = token {
                                    self.data.discard(token);
                                }
                                return Err(error);
                            }
                        };
                        answer = self.data.parse(&mut store, &name, &args, outcome);
                    }
                }
            }
            Err("snapshot refresh exceeded 128 requests".into())
        })();
        self.data.forgotten(&store, &[]);
        result
    }
}
