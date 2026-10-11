//! Web admission of the fixed photo pair. @ref LLP 1041 §8.5 / LLP 1002 D4.
//!
//! Frozen wire v2 is EXACTLY 120 LE bytes: version/op u32 at 0/4;
//! runtime/handle/target/clip/geometry-sequence u64 at 8/16/24/32/40;
//! original Translate/Scale serials u64 at 48/56; six f64 values at 64..104;
//! clock-ms f64 at 112. All JSON u64s are decimal strings, never JS Numbers.
//! Ops 10 geometry=[bw,bh,pw,ph,0,0], 11 begin=[x,y,s,0,0,0],
//! 12 move=[x,y,s,0,0,0], 13 action=[x,y,s,0,0,0], 14 invalidate=[0;6].
//! Op 13's velocities are the engine's own, measured over every value the
//! pair was given (LLP 1057.001 §3); the reply carries them as `velocity`.
//! Ops 10/11/14 require zero token fields. Every unused value must equal zero
//! (+0/-0 accepted; NaN/nonzero refused). Single-property v1 ends remain ends.
//! Stale identity/sequence/tokens refuse before incoming values/time; malformed
//! live input cannot partly mutate a pair. Accepted geometry synchronizes current
//! authoring/time and cancels the old pair BEFORE feedback, then lowers once.

use super::{Batch, Host, HostError};
use exact_kernel::{motion::motion_node, NodeKey, TransformDragBinding, ViewId};
use exact_motion::{HoldEnd, TransformHold, Value};
use exact_runner::{DataSource, Event, Timed};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_RUNTIME: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Default)]
struct Geometry {
    sequence: u64,
    dimensions: Option<[f64; 4]>,
    ready: bool,
}
struct Handle {
    key: NodeKey,
    geometry_handler: bool,
    release_handler: bool,
    published: Option<Option<TransformDragBinding>>,
    geometry: Geometry,
}
#[derive(Clone, Copy)]
struct Active {
    binding: TransformDragBinding,
    target_view: ViewId,
    sequence: u64,
    held: TransformHold,
    action_fired: bool,
    ending: bool,
}
pub(super) struct TransformDrags {
    runtime: u64,
    owner: Option<NodeKey>,
    handles: BTreeMap<ViewId, Handle>,
    // Existing Translate serial is the key, not a second pair serial/registry.
    // During independent terminal cleanup one original survivor may remain.
    pairs: BTreeMap<u64, Active>,
}
impl TransformDrags {
    pub fn new() -> Result<Self, HostError> {
        let runtime = NEXT_RUNTIME
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| HostError::RuntimeIdExhausted)?;
        Ok(Self {
            runtime,
            owner: None,
            handles: BTreeMap::new(),
            pairs: BTreeMap::new(),
        })
    }
    pub fn insert(
        &mut self,
        view: ViewId,
        key: NodeKey,
        geometry_handler: bool,
        release_handler: bool,
    ) {
        self.handles.insert(
            view,
            Handle {
                key,
                geometry_handler,
                release_handler,
                published: None,
                geometry: Geometry::default(),
            },
        );
    }
    pub fn remove(&mut self, view: ViewId) {
        self.handles.remove(&view);
    }
}

struct Input {
    op: u32,
    runtime: u64,
    binding: TransformDragBinding,
    sequence: u64,
    tokens: [u64; 2],
    values: [f64; 6],
    now_ms: f64,
}
impl Input {
    fn decode(bytes: &[u8]) -> Result<Self, &'static str> {
        use exact_plan::bytes::Reader;
        if bytes.len() != 120 {
            return Err("malformed transform length");
        }
        let decode = || -> Result<Self, exact_plan::PlanError> {
            let mut r = Reader::new(bytes);
            if r.u32()? != 2 {
                return Err(exact_plan::PlanError::BadCount(0));
            }
            let op = r.u32()?;
            let runtime = r.u64()?;
            let key = |n: u64| NodeKey {
                index: n as u32,
                generation: (n >> 32) as u32,
            };
            let binding = TransformDragBinding {
                handle: key(r.u64()?),
                target: key(r.u64()?),
                clip: key(r.u64()?),
            };
            let sequence = r.u64()?;
            let tokens = [r.u64()?, r.u64()?];
            let mut values = [0.0; 6];
            for v in &mut values {
                *v = r.f64()?;
            }
            let now_ms = r.f64()?;
            Ok(Self {
                op,
                runtime,
                binding,
                sequence,
                tokens,
                values,
                now_ms,
            })
        };
        let input = decode().map_err(|_| "malformed transform input")?;
        if !(10..=14).contains(&input.op) {
            return Err("invalid transform operation");
        }
        Ok(input)
    }
    fn event(&self) -> Event {
        let [x, y, scale, vx, vy, vscale] = self.values;
        if self.op == 10 {
            Event::TransformGeometry {
                box_width: x,
                box_height: y,
                port_width: scale,
                port_height: vx,
            }
        } else {
            Event::TransformRelease {
                x,
                y,
                scale,
                vx,
                vy,
                vscale,
            }
        }
    }
    fn validate(&self, floor: f64) -> Result<(), &'static str> {
        if !self.now_ms.is_finite() || self.now_ms < 0.0 || self.now_ms / 1000.0 < floor {
            return Err("invalid transform clock");
        }
        if [10, 11, 14].contains(&self.op) && self.tokens != [0; 2] {
            return Err("unused transform tokens must be zero");
        }
        let unused = match self.op {
            10 => 4,
            11..=13 => 3,
            _ => 0,
        };
        if self.values[unused..].iter().any(|v| *v != 0.0) {
            return Err("unused transform values must be zero");
        }
        if self.op != 14 && !valid_event(&self.event()) {
            return Err("invalid transform values");
        }
        Ok(())
    }
    fn samples(&self) -> [Value; 2] {
        [
            Value::new(self.values[0], self.values[1]),
            Value::scalar(self.values[2]),
        ]
    }
}

pub(super) fn valid_event(event: &Event) -> bool {
    let pixel = |v: f64| v.is_finite() && v.abs() <= f32::MAX as f64;
    match *event {
        Event::TransformGeometry {
            box_width,
            box_height,
            port_width,
            port_height,
        } => [box_width, box_height, port_width, port_height]
            .into_iter()
            .all(|v| pixel(v) && v >= 0.0),
        Event::TransformRelease {
            x,
            y,
            scale,
            vx,
            vy,
            vscale,
        } => {
            pixel(x)
                && pixel(y)
                && pixel(scale)
                && scale > 0.0
                && (scale as f32) > 0.0
                && [vx, vy, vscale].into_iter().all(f64::is_finite)
        }
        _ => true,
    }
}

impl<D: DataSource> Host<D> {
    /// Before an event the host delivers itself, mid-gesture (a drag's
    /// release, its handle's new geometry): the runner's clock moves to the
    /// input's time `now_ms`, as [`Host::dispatch_at`] moves it, firing the
    /// timers due by then at their own times. Idle, the runner's clock can be
    /// seconds old, and the event's action would read that `performanceNow()` and arm
    /// its `after`s from it. Their commits, in order, and a refusal, which
    /// stops the clock at the refusing timer (the runner's rule): the event
    /// then runs there, as through `dispatch_at`.
    pub(crate) fn advance_for_input(&mut self, now_ms: f64) -> (Vec<Timed>, Option<String>) {
        let a = self.runner.advance_timed(now_ms);
        (a.receipts, a.error.map(|e| format!("{e:?}")))
    }

    /// The event after [`Host::advance_for_input`], its commit last, unless
    /// the timers just fired ended its gesture (`still` false: they changed
    /// or removed the binding the input was validated against). The commits,
    /// the first refusal, and whether the event committed.
    pub(crate) fn deliver_after(
        &mut self,
        view: ViewId,
        event: Event,
        mut receipts: Vec<Timed>,
        mut error: Option<String>,
        still: bool,
    ) -> (Vec<Timed>, Option<String>, bool) {
        if !still {
            error.get_or_insert_with(|| "the gesture's binding changed before its event".into());
            return (receipts, error, false);
        }
        let committed = match self.runner.dispatch(view, event) {
            Ok(receipt) => {
                receipts.push(Timed {
                    at_ms: self.runner.now_ms(),
                    receipt,
                });
                true
            }
            Err(e) => {
                error.get_or_insert(format!("{e:?}"));
                false
            }
        };
        (receipts, error, committed)
    }

    /// Consume the fixed v2 photo packet. Geometry does not prove a worker result
    /// or change Kernel layout: it is a bound browser observation. Refused stale
    /// inputs never seek their incoming clock. A stale reply may include a batch
    /// retiring old surviving overlays at the existing Engine clock.
    pub fn transform_motion(&mut self, bytes: &[u8]) -> String {
        self.transform_input(bytes)
            .unwrap_or_else(exact_runner::agent::error)
    }

    fn transform_input(&mut self, bytes: &[u8]) -> Result<String, &'static str> {
        let mut input = Input::decode(bytes)?;
        if input.runtime != self.transform_drags.runtime {
            return Ok(stale());
        }
        let Some(handle) = self.runner.kernel().node_by_key(input.binding.handle) else {
            return Ok(stale());
        };
        let view = handle.id;
        let Some(record) = self.transform_drags.handles.get(&view) else {
            return Ok(stale());
        };
        if record.key != input.binding.handle
            || self.transform_drag_binding(view) != Some(input.binding)
        {
            return Ok(self.transform_stale());
        }
        let geometry = record.geometry;
        if input.sequence == 0 || input.sequence < geometry.sequence {
            return Ok(stale());
        }
        if [11, 12, 13].contains(&input.op)
            && (input.sequence != geometry.sequence || !geometry.ready || !record.release_handler)
        {
            return Ok(stale());
        }
        let active = if input.op == 12 || input.op == 13 {
            let Some(active) = self.transform_drags.pairs.get(&input.tokens[0]).copied() else {
                return Ok(stale());
            };
            if active.binding != input.binding
                || active.sequence != input.sequence
                || active.held.scale().token.serial() != input.tokens[1]
                || active.action_fired
                || active.ending
            {
                return Ok(stale());
            }
            if !self.transform_active_valid(active) {
                return Ok(self.transform_stale());
            }
            Some(active)
        } else {
            None
        };
        input.validate(self.springs.now())?;
        if input.op == 10 || input.op == 14 {
            return self.transform_geometry(view, &input, geometry);
        }
        if input.op == 11 {
            let held = self
                .springs
                .begin_transform_hold(
                    motion_node(input.binding.target),
                    input.samples(),
                    input.now_ms / 1000.0,
                )
                .map_err(|_| "transform begin refused")?;
            let Some(held) = held else {
                return Ok(stale());
            };
            let target_view = self
                .runner
                .kernel()
                .node_by_key(input.binding.target)
                .expect("binding target")
                .id;
            self.transform_drags.pairs.insert(
                held.translate().token.serial(),
                Active {
                    binding: input.binding,
                    target_view,
                    sequence: input.sequence,
                    held,
                    action_fired: false,
                    ending: false,
                },
            );
            self.now_ms = input.now_ms;
            let mut batch = Batch::new();
            self.reconcile_transform_drags(&mut batch);
            self.emit_springs(&mut batch, &[], input.now_ms / 1000.0);
            for start in [held.translate(), held.scale()] {
                batch.animate(
                    target_view,
                    start.token.property().name(),
                    0.0,
                    0.0,
                    &[],
                    false,
                );
            }
            return Ok(format!("{{\"accepted\":true,\"runtime\":\"{}\",\"geometrySequence\":\"{}\",\"translateToken\":\"{}\",\"scaleToken\":\"{}\",\"value\":[{},{},{}],\"batch\":{}}}",
                input.runtime,input.sequence,held.translate().token.serial(),held.scale().token.serial(),held.translate().value.x,held.translate().value.y,held.scale().value.x,
                self.finish(batch, None)));
        }
        let active = active.expect("paired operation");
        if !self
            .springs
            .update_transform_hold(active.held, input.samples(), input.now_ms / 1000.0)
            .map_err(|_| "transform move refused")?
        {
            return Ok(self.transform_stale());
        }
        if input.op == 12 {
            return Ok(accepted(self.hold_batch(input.now_ms)));
        }
        self.transform_drags
            .pairs
            .get_mut(&input.tokens[0])
            .expect("active pair")
            .action_fired = true;
        self.now_ms = input.now_ms;
        // The release velocity is the engine's, over every value the pair was
        // given (LLP 1057.001 §3); finite samples give a finite slope.
        let now_s = input.now_ms / 1000.0;
        let [translate, scale] = [active.held.translate(), active.held.scale()].map(|s| {
            self.springs
                .hold_velocity(s.token.serial(), now_s)
                .unwrap_or(Value::ZERO)
        });
        let measured = [translate.x, translate.y, scale.x];
        input.values[3..].copy_from_slice(&if measured.iter().all(|v| v.is_finite()) {
            measured
        } else {
            [0.0; 3]
        });
        // All six values/time passed preflight, both old holds are live, and the
        // action executes while both still own presentation. Do not lower first.
        let (early, early_error) = self.advance_for_input(input.now_ms);
        let still = early.is_empty()
            || self
                .runner
                .kernel()
                .transform_drag_binding(input.binding.handle)
                == Some(input.binding);
        let (receipts, error, committed) =
            self.deliver_after(view, input.event(), early, early_error, still);
        let batch = self.batch_for(&receipts, error.as_deref());
        Ok(format!(
            "{{\"accepted\":true,\"dispatched\":true,\"committed\":{committed},\"velocity\":[{},{},{}],\"batch\":{batch}}}",
            input.values[3], input.values[4], input.values[5]
        ))
    }

    fn transform_geometry(
        &mut self,
        view: ViewId,
        input: &Input,
        previous: Geometry,
    ) -> Result<String, &'static str> {
        let dimensions = [
            input.values[0],
            input.values[1],
            input.values[2],
            input.values[3],
        ];
        if input.sequence == previous.sequence {
            if input.op == 10
                && previous.dimensions == Some(dimensions)
                && previous.ready == dimensions.into_iter().all(|v| v > 0.0)
                || input.op == 14 && !previous.ready
            {
                return Ok(accepted(self.finish(Batch::new(), None)));
            }
            return Err("geometry sequence cannot change its facts");
        }
        let handle = self
            .transform_drags
            .handles
            .get_mut(&view)
            .expect("registered handle");
        handle.geometry.sequence = input.sequence;
        handle.geometry.ready = input.op == 10 && dimensions.into_iter().all(|v| v > 0.0);
        let changed = input.op == 10 && previous.dimensions != Some(dimensions);
        if input.op == 10 {
            handle.geometry.dimensions = Some(dimensions);
        }
        let dispatch = changed && handle.geometry_handler;
        // Mapping changes invalidate the Engine pair BEFORE feedback, not only
        // the JS contact. First adopt current authoring/time while still held,
        // then cancel originals. A geometry action can retarget at the same
        // clock; its receipt performs the sole dirty-frame lowering.
        self.now_ms = input.now_ms;
        let mut batch = Batch::new();
        // Timers due by the observation fire first, so the authoring adopted
        // below while held, and the cancellation, are what they left.
        let (early, early_error) = self.advance_for_input(input.now_ms);
        let synced = self.springs.synchronize_transform(
            self.runner.kernel(),
            input.binding.target,
            input.now_ms / 1000.0,
        );
        Self::emit_lowered(&mut batch, synced);
        let active: Vec<_> = self
            .transform_drags
            .pairs
            .iter()
            .filter_map(|(&serial, a)| (a.binding.target == input.binding.target).then_some(serial))
            .collect();
        for serial in active {
            self.retire_transform_pair(serial, &mut batch);
        }
        if dispatch {
            let still = early.is_empty()
                || self
                    .runner
                    .kernel()
                    .transform_drag_binding(input.binding.handle)
                    == Some(input.binding);
            let (receipts, error, committed) =
                self.deliver_after(view, input.event(), early, early_error, still);
            if !committed {
                Self::emit_lowered(&mut batch, self.springs.lower_current(self.runner.kernel()));
            }
            Ok(admitted(
                self.batch_from(batch, &receipts, error.as_deref()),
                committed,
            ))
        } else {
            Self::emit_lowered(&mut batch, self.springs.lower_current(self.runner.kernel()));
            if early.is_empty() && early_error.is_none() {
                Ok(accepted(self.finish(batch, None)))
            } else {
                Ok(admitted(
                    self.batch_from(batch, &early, early_error.as_deref()),
                    true,
                ))
            }
        }
    }

    fn transform_drag_binding(&self, view: ViewId) -> Option<TransformDragBinding> {
        let handle = self.transform_drags.handles.get(&view)?;
        if !handle.geometry_handler || !handle.release_handler {
            return None;
        }
        self.runner
            .kernel()
            .transform_drag_binding(handle.key)
            .filter(|b| Some(b.target) == self.transform_drags.owner)
    }

    fn reconcile_transform_owner(&mut self) {
        let kernel = self.runner.kernel();
        self.transform_drags
            .handles
            .retain(|_, h| kernel.node_by_key(h.key).is_some());
        let mut first = None;
        let mut current_valid = false;
        for h in self.transform_drags.handles.values() {
            if !h.geometry_handler || !h.release_handler {
                continue;
            }
            if let Some(b) = kernel.transform_drag_binding(h.key) {
                first.get_or_insert(b.target);
                current_valid |= self.transform_drags.owner == Some(b.target);
            }
        }
        // Keep one eligible owner across sibling handle removal; never steal a
        // live owner for a second target. No retained history or per-frame scan.
        if !current_valid {
            self.transform_drags.owner = first;
        }
    }

    fn transform_active_valid(&self, a: Active) -> bool {
        let Some(node) = self.runner.kernel().node_by_key(a.binding.handle) else {
            return false;
        };
        let Some(handle) = self.transform_drags.handles.get(&node.id) else {
            return false;
        };
        let live = [a.held.translate(), a.held.scale()]
            .map(|s| self.springs.token(s.token.serial()) == Some(s.token));
        handle.geometry.ready
            && handle.geometry.sequence == a.sequence
            && self.transform_drag_binding(node.id) == Some(a.binding)
            && if a.ending {
                live.into_iter().any(|v| v)
            } else {
                live.into_iter().all(|v| v)
            }
    }

    pub(super) fn reconcile_transform_drags(&mut self, batch: &mut Batch) {
        self.reconcile_transform_owner();
        let invalid: Vec<_> = self
            .transform_drags
            .pairs
            .iter()
            .filter_map(|(&serial, &a)| (!self.transform_active_valid(a)).then_some(serial))
            .collect();
        for serial in invalid {
            self.retire_transform_pair(serial, batch);
        }
    }

    fn retire_transform_pair(&mut self, serial: u64, batch: &mut Batch) {
        let a = self
            .transform_drags
            .pairs
            .remove(&serial)
            .expect("collected pair");
        for start in [a.held.translate(), a.held.scale()] {
            // Independent original-token cleanup: a replacement is never
            // ended, and stale packet time cannot be used for cancellation.
            let _ =
                self.springs
                    .end_hold(start.token.serial(), HoldEnd::Cancel, self.springs.now());
            batch.retire_transform_token(a.target_view, self.transform_drags.runtime, start.token);
        }
    }

    pub(super) fn transform_member_ended(&mut self, serial: u64) {
        let pair = self.transform_drags.pairs.iter().find_map(|(&key, a)| {
            [a.held.translate(), a.held.scale()]
                .iter()
                .any(|s| s.token.serial() == serial)
                .then_some(key)
        });
        if let Some(key) = pair {
            let a = self.transform_drags.pairs.get_mut(&key).expect("found");
            a.ending = true;
            if [a.held.translate(), a.held.scale()]
                .into_iter()
                .all(|s| self.springs.token(s.token.serial()).is_none())
            {
                self.transform_drags.pairs.remove(&key);
            }
        }
    }

    fn transform_stale(&mut self) -> String {
        let mut batch = Batch::new();
        self.reconcile_transform_drags(&mut batch);
        let lowered = self.springs.lower_current(self.runner.kernel());
        Self::emit_lowered(&mut batch, lowered);
        format!(
            "{{\"accepted\":false,\"batch\":{}}}",
            self.finish(batch, None)
        )
    }

    pub(super) fn emit_transform_drags(&mut self, batch: &mut Batch) {
        self.reconcile_transform_owner();
        let kernel = self.runner.kernel();
        let owner = self.transform_drags.owner;
        for (&view, handle) in &mut self.transform_drags.handles {
            let binding = kernel.transform_drag_binding(handle.key).filter(|b| {
                handle.geometry_handler && handle.release_handler && Some(b.target) == owner
            });
            if handle.published == Some(binding) {
                continue;
            }
            handle.geometry.ready = false;
            handle.geometry.dimensions = None;
            handle.published = Some(binding);
            let targets = binding.and_then(|b| {
                Some([
                    (b.target, kernel.node_by_key(b.target)?.id),
                    (b.clip, kernel.node_by_key(b.clip)?.id),
                ])
            });
            batch.transform_drag(view, self.transform_drags.runtime, handle.key, targets);
        }
    }
}
fn stale() -> String {
    "{\"accepted\":false}".into()
}
fn accepted(batch: String) -> String {
    format!("{{\"accepted\":true,\"batch\":{batch}}}")
}

/// A geometry reply that says whether its event committed: a timer's
/// refusal in the batch is not the observation's (the glue admits from it).
fn admitted(batch: String, committed: bool) -> String {
    format!("{{\"accepted\":true,\"committed\":{committed},\"batch\":{batch}}}")
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use exact_runner::{DataError, Value as DataValue};

    struct NoData;
    impl DataSource for NoData {
        fn query(&mut self, name: &str, _: &[DataValue]) -> Result<DataValue, DataError> {
            Err(DataError::UnknownSource(name.into()))
        }
    }

    #[test]
    fn repeated_pair_takeover_keeps_only_current_metadata_and_independent_end_survivor() {
        let source = r#"component App
  state calls = 0
  action geometry(w: number, h: number, pw: number, ph: number)
    calls = calls + 1
  action finish(x: number, y: number, s: number, vx: number, vy: number, vs: number)
    calls = calls + 1
  view
    column width=320 height=200 overflow="hidden" border-width=0 padding=0
      column id="photo" width="100%" height="100%" box-sizing="border-box" border-width=0 padding=0
        column testId="handle" transformDragFor="photo" transformgeometry=geometry transformrelease=finish
"#;
        crate::link::link_for_tests();
        let (mut host, _) = Host::boot(
            &contract::compile(source).unwrap().encode(),
            NoData,
            Default::default(),
            "/",
        )
        .unwrap();
        let handle = host.runner.kernel().find_by_test_id("handle")[0];
        let binding = host.runner.kernel().transform_drag_binding(handle).unwrap();
        let packet = |op: u32, runtime: u64, values: [f64; 6]| {
            let mut bytes = Vec::new();
            for n in [2u32, op] {
                bytes.extend(n.to_le_bytes());
            }
            for n in [
                runtime,
                motion_node(handle),
                motion_node(binding.target),
                motion_node(binding.clip),
                1,
                0,
                0,
            ] {
                bytes.extend(n.to_le_bytes());
            }
            for n in values.into_iter().chain([0.0]) {
                bytes.extend(n.to_le_bytes());
            }
            bytes
        };
        let runtime = host.transform_drags.runtime;
        let geometry = packet(10, runtime, [320.0, 200.0, 320.0, 200.0, 0.0, 0.0]);
        assert!(host
            .transform_motion(&geometry)
            .contains("\"accepted\":true"));
        let begin = packet(11, runtime, [12.0, 3.0, 1.5, 0.0, 0.0, 0.0]);
        for _ in 0..128 {
            let old: Vec<_> = host
                .transform_drags
                .pairs
                .values()
                .map(|a| a.held)
                .collect();
            assert!(host.transform_motion(&begin).contains("\"accepted\":true"));
            assert_eq!(host.transform_drags.pairs.len(), 1);
            assert_eq!(host.transform_drags.handles.len(), 1);
            assert!(old
                .into_iter()
                .all(|a| !host.has_hold(a.translate().token.serial())
                    && !host.has_hold(a.scale().token.serial())));
        }
        let pair = host.transform_drags.pairs.values().next().unwrap().held;
        host.end_hold(pair.translate().token.serial(), HoldEnd::Cancel, 0.0)
            .unwrap()
            .unwrap();
        assert_eq!(host.transform_drags.pairs.len(), 1);
        assert!(host.has_hold(pair.scale().token.serial()));
        host.end_hold(pair.scale().token.serial(), HoldEnd::Cancel, 0.0)
            .unwrap()
            .unwrap();
        assert!(host.transform_drags.pairs.is_empty());
    }
}
