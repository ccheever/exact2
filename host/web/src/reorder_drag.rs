//! Arrange's bounded browser presentation owner. @ref LLP 1041 §8.5.
//! Wire v3: 176-byte header and <=4096 32-byte wrapper samples. All keys,
//! runtime and serials are LE u64 / decimal JSON strings. See `Input`.
use super::{Batch, Host, HostError};
use exact_kernel::{motion::motion_node, CommitReceipt, NodeKey, ViewId};
use exact_motion::{HoldEnd, HoldToken, Property, Value};
use exact_runner::{
    DataSource, ReorderBinding, ReorderGeometry, ReorderProgress, ReorderToken, Timed,
};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT_RUNTIME: AtomicU64 = AtomicU64::new(1);
const MAX_WRAPPERS: usize = 4096;

struct Handle {
    key: NodeKey,
    published: Option<Option<ReorderBinding>>,
}
struct Active {
    binding: ReorderBinding,
    token: ReorderToken,
    holds: BTreeMap<NodeKey, HoldToken>,
    terminal: bool,
    captured: bool,
    released: bool,
    velocity: Value,
}
pub(super) struct ReorderDrags {
    runtime: u64,
    handles: BTreeMap<ViewId, Handle>,
    active: Option<Active>,
}
impl ReorderDrags {
    pub fn new() -> Result<Self, HostError> {
        let runtime = NEXT_RUNTIME
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
            .map_err(|_| HostError::RuntimeIdExhausted)?;
        Ok(Self {
            runtime,
            handles: BTreeMap::new(),
            active: None,
        })
    }
    pub fn track(&mut self, view: ViewId, key: NodeKey) {
        self.handles.entry(view).or_insert(Handle {
            key,
            published: None,
        });
    }
    pub fn remove(&mut self, view: ViewId) {
        self.handles.remove(&view);
    }
}
#[derive(Clone, Copy)]
struct Sample {
    key: NodeKey,
    serial: u64,
    value: Value,
}
struct Input {
    op: u32,
    runtime: u64,
    binding: ReorderBinding,
    token: u64,
    geometry: ReorderGeometry,
    y: f64,
    value: Value,
    velocity: Value,
    now: f64,
    rows: Vec<Sample>,
}
fn key(v: u64) -> NodeKey {
    NodeKey {
        index: v as u32,
        generation: (v >> 32) as u32,
    }
}
impl Input {
    fn decode(bytes: &[u8]) -> Result<Self, &'static str> {
        use exact_plan::{bytes::Reader, PlanError};
        if bytes.len() < 176 {
            return Err("malformed reorder length");
        }
        let parse = || -> Result<Self, PlanError> {
            let mut r = Reader::new(bytes);
            if r.u32()? != 3 {
                return Err(PlanError::BadCount(0));
            }
            let op = r.u32()?;
            let runtime = r.u64()?;
            let binding = ReorderBinding {
                handle: key(r.u64()?),
                list: key(r.u64()?),
                wrapper: key(r.u64()?),
                root: key(r.u64()?),
                row_epoch: r.u64()?,
            };
            let token = r.u64()?;
            let revision = r.u64()?;
            let scroll_sequence = r.u64()?;
            let count = r.u32()? as usize;
            if r.u32()? != 0
                || count > MAX_WRAPPERS
                || bytes.len() != 176 + count * 32
                || !(15..=20).contains(&op)
            {
                return Err(PlanError::BadCount(count as u32));
            }
            let geometry = ReorderGeometry {
                list: binding.list,
                revision,
                scroll_sequence,
                scroll_top: r.f64()?,
                port_width: r.f64()?,
                port_height: r.f64()?,
                row_width: r.f64()?,
                total_extent: r.f64()?,
            };
            let y = r.f64()?;
            let value = Value::new(r.f64()?, r.f64()?);
            let velocity = Value::new(r.f64()?, r.f64()?);
            let now = r.f64()?;
            let mut rows = Vec::with_capacity(count);
            for _ in 0..count {
                rows.push(Sample {
                    key: key(r.u64()?),
                    serial: r.u64()?,
                    value: Value::new(r.f64()?, r.f64()?),
                });
            }
            Ok(Self {
                op,
                runtime,
                binding,
                token,
                geometry,
                y,
                value,
                velocity,
                now,
                rows,
            })
        };
        parse().map_err(|_| "malformed reorder input")
    }
    fn validate(&self, floor: f64) -> Result<(), &'static str> {
        if !self.now.is_finite() || self.now < 0. || self.now / 1000. < floor {
            return Err("invalid reorder clock");
        }
        if self.velocity != Value::ZERO {
            // Measured by the engine (LLP 1057.001 §3); the slots stay zero.
            return Err("reorder velocity is measured; its slots must be zero");
        }
        let pixel = |v: f64| v.is_finite() && v.abs() <= f32::MAX as f64;
        if ![self.y, self.value.x, self.value.y].into_iter().all(pixel)
            || ![self.velocity.x, self.velocity.y]
                .into_iter()
                .all(f64::is_finite)
            || self
                .rows
                .iter()
                .any(|s| !pixel(s.value.x) || !pixel(s.value.y))
        {
            return Err("invalid reorder sample");
        }
        if [15, 16, 20].contains(&self.op) && !self.rows.is_empty() {
            return Err("unexpected reorder samples");
        }
        let mut keys = std::collections::BTreeSet::new();
        if self.rows.iter().any(|s| !keys.insert(s.key)) {
            return Err("duplicate reorder sample");
        }
        Ok(())
    }
}
impl<D: DataSource> Host<D> {
    /// Version3 of the existing motion boundary. No logical item keys are
    /// accepted from the browser; the collection certifies the final gap.
    pub fn reorder_motion(&mut self, bytes: &[u8]) -> String {
        self.reorder_input(bytes)
            .unwrap_or_else(exact_runner::agent::error)
    }
    fn reorder_input(&mut self, bytes: &[u8]) -> Result<String, &'static str> {
        let i = Input::decode(bytes)?;
        if i.runtime != self.reorder_drags.runtime {
            return Ok(stale());
        }
        if i.op == 15 {
            if i.token != 0
                || self.reorder_drags.active.is_some()
                || self.runner.reorder_binding(i.binding.handle) != Some(i.binding)
                || self.runner.reorder_geometry(i.binding.list).as_ref() != Some(&i.geometry)
            {
                return Ok(stale());
            }
        } else {
            let Some(a) = self.reorder_drags.active.as_ref() else {
                return Ok(stale());
            };
            if a.binding != i.binding || a.token.serial() != i.token {
                return Ok(stale());
            }
            if (i.op == 16 || i.op == 17)
                && (a.terminal
                    || !self.runner.has_reorder(a.token)
                    || a.holds
                        .get(&a.binding.wrapper)
                        .is_none_or(|t| self.springs.token(t.serial()) != Some(*t))
                    || self.runner.reorder_geometry(i.binding.list).as_ref() != Some(&i.geometry))
            {
                return Ok(stale());
            }
            if i.op == 18 && (a.released || a.captured)
                || i.op == 19 && (!a.terminal || a.released)
                || i.op == 20 && !a.released
            {
                return Ok(stale());
            }
        }
        i.validate(self.springs.now())?;
        if i.op == 15 {
            return self.reorder_begin(&i);
        }
        if i.op == 20 {
            return self.reorder_finish(&i);
        }
        if i.op == 19 {
            return self.reorder_rebase(&i);
        }
        let token = self.reorder_drags.active.as_ref().unwrap().token;
        if i.op == 17 || i.op == 18 {
            self.reorder_samples(&i)?;
        }
        // Numeric/identity preflight is complete before any Engine update.
        let source = self
            .reorder_drags
            .active
            .as_ref()
            .unwrap()
            .holds
            .get(&i.binding.wrapper)
            .copied();
        if let Some(source) = source {
            self.springs
                .update_hold(source.serial(), i.value, i.now / 1000.)
                .map_err(|_| "reorder move refused")?;
        }
        self.now_ms = i.now;
        if i.op == 18 {
            return self.reorder_terminal(&i, false, None);
        }
        let progress = self
            .runner
            .preview_reorder(token, i.geometry.clone(), i.y)
            .map_err(|_| "reorder preview refused")?;
        let (certified, receipt) = match progress {
            ReorderProgress::Accepted { receipt } => (true, receipt),
            ReorderProgress::NeedsMeasurement => (false, None),
            ReorderProgress::Stale => return Ok(stale()),
        };
        if i.op == 17 {
            return self.reorder_terminal(&i, certified, receipt);
        }
        let batch = self.reorder_batch(receipt, i.now);
        Ok(self.reorder_reply(&batch, certified, false))
    }
    fn reorder_begin(&mut self, i: &Input) -> Result<String, &'static str> {
        let view = self
            .runner
            .kernel()
            .node_by_key(i.binding.wrapper)
            .unwrap()
            .id;
        let Some(admitted) = self
            .runner
            .begin_reorder(i.binding, i.geometry.clone())
            .map_err(|_| "reorder admission failed")?
        else {
            return Ok(stale());
        };
        let start = self.springs.begin_hold(
            self.runner.kernel(),
            view,
            Property::Translate,
            i.value,
            i.now / 1000.,
        );
        let start = match start {
            Ok(Some(start)) => start,
            _ => {
                // Return every reset operation even on engine admission failure.
                // No logical action was run; the original pin is retired once.
                let mut receipts = Vec::new();
                if let Some(receipt) = admitted.receipt {
                    receipts.push(receipt);
                }
                if let Some(receipt) = self
                    .runner
                    .cancel_reorder(admitted.token)
                    .map_err(|_| "reorder cancel failed")?
                {
                    receipts.push(receipt);
                }
                if let Some(receipt) = self
                    .runner
                    .finish_reorder(admitted.token)
                    .map_err(|_| "reorder finish failed")?
                {
                    receipts.push(receipt);
                }
                let at_ms = self.springs.now() * 1000.;
                let receipts: Vec<_> = receipts
                    .into_iter()
                    .map(|receipt| Timed { at_ms, receipt })
                    .collect();
                let batch = self.batch_for(&receipts, None);
                return Ok(format!("{{\"accepted\":false,\"batch\":{batch}}}"));
            }
        };
        self.reorder_drags.active = Some(Active {
            binding: i.binding,
            token: admitted.token,
            holds: BTreeMap::from([(i.binding.wrapper, start.token)]),
            terminal: false,
            captured: false,
            released: false,
            velocity: Value::ZERO,
        });
        self.now_ms = i.now;
        let mut b = Batch::new();
        b.animate(view, "translate", 0., 0., &[], true);
        let receipts = admitted.receipt.map(|receipt| Timed {
            at_ms: i.now,
            receipt,
        });
        let batch = self.batch_from(b, receipts.as_slice(), None);
        Ok(self.reorder_reply(&batch, true, false))
    }
    fn reorder_samples(&self, i: &Input) -> Result<(), &'static str> {
        let a = self.reorder_drags.active.as_ref().unwrap();
        let frame = self.runner.reorder_frame(a.token);
        let wrappers = frame.as_ref().map_or(&[][..], |f| f.wrappers.as_slice());
        let sampled: exact_kernel::id::IdSet<_> = i.rows.iter().map(|s| s.key).collect();
        if i.rows.len() != wrappers.len() || wrappers.iter().any(|w| !sampled.contains(&w.wrapper))
        {
            return Err("incomplete reorder capture");
        }
        for s in &i.rows {
            let expected = a
                .holds
                .get(&s.key)
                .filter(|t| self.springs.token(t.serial()) == Some(**t))
                .map_or(0, |t| t.serial());
            if expected != s.serial {
                return Err("stale reorder hold capture");
            }
        }
        Ok(())
    }
    fn reorder_terminal(
        &mut self,
        i: &Input,
        certified: bool,
        preview: Option<CommitReceipt>,
    ) -> Result<String, &'static str> {
        // Preview's authoring is synchronized while all old presentations remain
        // owned. It is lowered together with the action/reset receipt, never lost.
        let mut b = Batch::new();
        if let Some(ref receipt) = preview {
            Self::emit_lowered(
                &mut b,
                self.springs.synchronize(
                    self.runner.kernel(),
                    std::slice::from_ref(receipt),
                    i.now / 1000.,
                ),
            );
        }
        for sample in &i.rows {
            if self
                .reorder_drags
                .active
                .as_ref()
                .unwrap()
                .holds
                .contains_key(&sample.key)
            {
                continue;
            }
            let view = self.runner.kernel().node_by_key(sample.key).unwrap().id;
            let Some(start) = self
                .springs
                .begin_hold(
                    self.runner.kernel(),
                    view,
                    Property::Translate,
                    sample.value,
                    i.now / 1000.,
                )
                .map_err(|_| "reorder neighbor capture refused")?
            else {
                continue;
            };
            self.reorder_drags
                .active
                .as_mut()
                .unwrap()
                .holds
                .insert(sample.key, start.token);
            b.animate(view, "translate", 0., 0., &[], true);
        }
        let a = self.reorder_drags.active.as_mut().unwrap();
        a.terminal = true;
        a.captured = true;
        // The source row's release velocity is the engine's, over the values
        // its hold was given (LLP 1057.001 §3), not the packet's.
        let measured = a
            .holds
            .get(&i.binding.wrapper)
            .and_then(|t| self.springs.hold_velocity(t.serial(), i.now / 1000.))
            .filter(|v| v.x.is_finite() && v.y.is_finite());
        let a = self.reorder_drags.active.as_mut().unwrap();
        a.velocity = if certified {
            measured.unwrap_or(Value::ZERO)
        } else {
            Value::ZERO
        };
        let token = a.token;
        let result = if certified {
            self.runner.drop_reorder(token, i.geometry.clone())
        } else {
            self.runner.cancel_reorder(token)
        };
        let mut receipts = Vec::new();
        if let Some(receipt) = preview {
            receipts.push(Timed {
                at_ms: i.now,
                receipt,
            });
        }
        let (dispatched, error) = match result {
            Ok(r) => {
                let dispatched = certified && r.is_some();
                if let Some(receipt) = r {
                    receipts.push(Timed {
                        at_ms: i.now,
                        receipt,
                    });
                }
                (dispatched, None)
            }
            Err(e) => (false, Some(format!("{e:?}"))),
        };
        let batch = self.batch_from(b, &receipts, error.as_deref());
        Ok(self.reorder_reply(&batch, certified, dispatched))
    }
    fn reorder_rebase(&mut self, i: &Input) -> Result<String, &'static str> {
        let a = self.reorder_drags.active.as_ref().unwrap();
        // Only surviving ORIGINAL holds may be rebased; a property takeover
        // invalidates that member without permitting it to address a successor.
        let survivors: Vec<_> = a
            .holds
            .iter()
            .filter(|(key, t)| {
                self.runner.kernel().node_by_key(**key).is_some()
                    && self.springs.token(t.serial()) == Some(**t)
            })
            .map(|(k, t)| (*k, *t))
            .collect();
        let sampled: exact_kernel::id::IdMap<_, _> =
            i.rows.iter().map(|s| (s.key, s.serial)).collect();
        if i.rows.len() != survivors.len()
            || survivors
                .iter()
                .any(|(k, t)| sampled.get(k) != Some(&t.serial()))
        {
            return Err("incomplete reorder rebase");
        }
        let source = a.binding.wrapper;
        let velocity = a.velocity;
        for sample in &i.rows {
            self.springs
                .update_hold(sample.serial, sample.value, i.now / 1000.)
                .map_err(|_| "reorder rebase refused")?;
        }
        for (key, token) in survivors {
            self.springs
                .end_hold(
                    token.serial(),
                    HoldEnd::Release {
                        velocity: if key == source { velocity } else { Value::ZERO },
                    },
                    i.now / 1000.,
                )
                .map_err(|_| "reorder release refused")?;
        }
        self.reorder_drags.active.as_mut().unwrap().released = true;
        let batch = self.hold_batch(i.now);
        Ok(self.reorder_reply(&batch, false, false))
    }
    fn reorder_finish(&mut self, i: &Input) -> Result<String, &'static str> {
        let token = self.reorder_drags.active.take().unwrap().token;
        let receipt = self
            .runner
            .finish_reorder(token)
            .map_err(|_| "reorder finish refused")?;
        let batch = self.reorder_batch(receipt, i.now);
        Ok(format!("{{\"accepted\":true,\"batch\":{batch}}}"))
    }
    fn reorder_batch(&mut self, receipt: Option<CommitReceipt>, now: f64) -> String {
        self.now_ms = now;
        if let Some(receipt) = receipt {
            self.batch_for(
                &[Timed {
                    at_ms: now,
                    receipt,
                }],
                None,
            )
        } else {
            self.hold_batch(now)
        }
    }
    fn reorder_reply(&self, batch: &str, certified: bool, dispatched: bool) -> String {
        let a = self.reorder_drags.active.as_ref().unwrap();
        format!("{{\"accepted\":true,\"certified\":{certified},\"dispatched\":{dispatched},\"runtime\":\"{}\",\"token\":\"{}\",\"terminal\":{},\"released\":{},\"frame\":{},\"batch\":{batch}}}",self.reorder_drags.runtime,a.token.serial(),a.terminal,a.released,self.reorder_frame_json())
    }
    fn reorder_frame_json(&self) -> String {
        let Some(a) = self.reorder_drags.active.as_ref() else {
            return "[]".into();
        };
        let Some(frame) = self.runner.reorder_frame(a.token) else {
            return "[]".into();
        };
        let rows:Vec<_>=frame.wrappers.iter().filter_map(|w|{
            let view=self.runner.kernel().node_by_key(w.wrapper)?.id;
            let token=a.holds.get(&w.wrapper).filter(|t|self.springs.token(t.serial())==Some(**t)).map_or(0,|t|t.serial());
            Some(format!("{{\"view\":{view},\"key\":\"{}\",\"rootKey\":\"{}\",\"top\":{},\"offset\":{},\"hold\":\"{token}\"}}",motion_node(w.wrapper),motion_node(w.root),w.top,w.offset))
        }).collect();
        format!("[{}]", rows.join(","))
    }
    pub(super) fn emit_reorder_drags(&mut self, batch: &mut Batch) {
        let kernel = self.runner.kernel();
        self.reorder_drags
            .handles
            .retain(|_, h| kernel.node_by_key(h.key).is_some());
        for (&view, h) in &mut self.reorder_drags.handles {
            let binding = self.runner.reorder_binding(h.key);
            if h.published == Some(binding) {
                continue;
            }
            h.published = Some(binding);
            batch.reorder_drag(view, self.reorder_drags.runtime, h.key, binding, kernel);
        }
        if let Some(a) = self.reorder_drags.active.as_mut() {
            a.terminal |= self
                .runner
                .reorder_frame(a.token)
                .is_none_or(|f| f.terminal);
            batch.reorder_state(
                self.reorder_drags.runtime,
                a.token.serial(),
                a.terminal,
                a.released,
                &self.reorder_frame_json(),
            );
        }
    }
}
fn stale() -> String {
    "{\"accepted\":false}".into()
}
#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "reorder_tests.rs"]
mod tests;
