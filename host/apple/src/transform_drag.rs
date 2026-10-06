//! Apple admission of the fixed photo pair. @ref LLP 1041 §8.5 / LLP 1002 D4.
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

use super::transform_drag_wire::Input;
use super::{Batch, Host, HostError};
use exact_kernel::{motion::motion_node, Kernel, NodeKey, TransformDragBinding, ViewId};
use exact_motion::{Engine, HoldEnd, Property, TransformHold};
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
    pub(super) mapping_pending: bool,
    handles: BTreeMap<ViewId, Handle>,
    // Existing Translate serial is the key, not a second pair serial/registry.
    // During independent terminal cleanup one original survivor may remain.
    pairs: BTreeMap<u64, Active>,
}
impl TransformDrags {
    #[cfg(test)]
    pub(super) fn pair_count(&self) -> usize {
        self.pairs.len()
    }
    pub fn new() -> Result<Self, HostError> {
        let runtime = NEXT_RUNTIME
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| HostError::RuntimeIdExhausted)?;
        Ok(Self {
            runtime,
            owner: None,
            mapping_pending: false,
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

impl<D: DataSource> Host<D> {
    /// An event the host delivers itself, mid-gesture (a drag's release, its
    /// handle's new geometry), at the input's time `now_ms`: the runner's
    /// clock moves there first, as [`Host::dispatch_at`] moves it, so a timer
    /// due by then fires at its own time and the action's `now()` — and an
    /// `after` it arms — is the input's, not the last advance's. Idle, with
    /// no frame or timer to move it, the runner's clock can be seconds old.
    /// The commits in order, the event's last; the refusal, the event's or
    /// a timer's; and whether the event committed.
    pub(super) fn deliver_at(
        &mut self,
        view: ViewId,
        event: Event,
        now_ms: f64,
    ) -> (Vec<Timed>, Option<String>, bool) {
        let mut a = self.runner.advance_timed(now_ms);
        let mut error = a.error.take().map(|e| format!("{e:?}"));
        let committed = match self.runner.dispatch(view, event) {
            Ok(receipt) => {
                a.receipts.push(Timed {
                    at_ms: now_ms.max(a.now_ms),
                    receipt,
                });
                true
            }
            Err(e) => {
                error.get_or_insert(format!("{e:?}"));
                false
            }
        };
        (a.receipts, error, committed)
    }

    /// Consume the fixed v2 photo packet. Geometry does not prove a worker result
    /// or change Kernel layout: it is a bound native observation. Refused stale
    /// inputs never seek their incoming clock. A stale reply may include a batch
    /// retiring old surviving overlays at the existing Engine clock.
    pub fn transform_motion(&mut self, bytes: &[u8]) -> String {
        self.transform_input(bytes).unwrap_or_else(|error| {
            format!(
                "{{\"accepted\":false,\"batch\":{}}}",
                self.hold_refusal(error)
            )
        })
    }

    fn transform_input(&mut self, bytes: &[u8]) -> Result<String, &'static str> {
        let mut input = Input::decode(bytes)?;
        if input.runtime != self.transform_drags.runtime {
            return Ok(self.transform_refused());
        }
        let Some(handle) = self.runner.kernel().node_by_key(input.binding.handle) else {
            return Ok(self.transform_refused());
        };
        let view = handle.id;
        let Some(record) = self.transform_drags.handles.get(&view) else {
            return Ok(self.transform_refused());
        };
        if record.key != input.binding.handle
            || self.transform_drag_binding(view) != Some(input.binding)
        {
            return Ok(self.transform_stale());
        }
        let geometry = record.geometry;
        if input.sequence == 0 || input.sequence < geometry.sequence {
            return Ok(self.transform_refused());
        }
        if [11, 12, 13].contains(&input.op)
            && (input.sequence != geometry.sequence || !geometry.ready || !record.release_handler)
        {
            return Ok(self.transform_refused());
        }
        let active = if input.op == 12 || input.op == 13 {
            let Some(active) = self.transform_drags.pairs.get(&input.tokens[0]).copied() else {
                return Ok(self.transform_refused());
            };
            if active.binding != input.binding
                || active.sequence != input.sequence
                || active.held.scale().token.serial() != input.tokens[1]
                || active.action_fired
                || active.ending
            {
                return Ok(self.transform_refused());
            }
            if !self.transform_active_valid(active) {
                return Ok(self.transform_stale());
            }
            Some(active)
        } else {
            None
        };
        input.validate(self.engine.now())?;
        if input.op == 10 || input.op == 14 {
            return self.transform_geometry(view, &input, geometry);
        }
        if input.op == 11 {
            let held = self
                .engine
                .begin_transform_hold(
                    motion_node(input.binding.target),
                    input.now_ms / 1000.0,
                    Some(input.samples()),
                )
                .map_err(|_| "transform begin refused")?;
            let Some(held) = held else {
                return Ok(self.transform_refused());
            };
            let target_view = self
                .runner
                .kernel()
                .node_by_key(input.binding.target)
                .expect("binding target")
                .id;
            for start in [held.translate(), held.scale()] {
                self.holds.insert(start.token.serial(), start.token);
            }
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
            self.present(&mut batch, false);
            return Ok(format!("{{\"accepted\":true,\"runtime\":\"{}\",\"geometrySequence\":\"{}\",\"translateToken\":\"{}\",\"scaleToken\":\"{}\",\"value\":[{},{},{}],\"batch\":{}}}",
                input.runtime,input.sequence,held.translate().token.serial(),held.scale().token.serial(),held.translate().value.x,held.translate().value.y,held.scale().value.x,
                self.finish(batch, None)));
        }
        let active = active.expect("paired operation");
        if !self
            .engine
            .update_transform_hold(active.held, input.now_ms / 1000.0, input.samples())
            .map_err(|_| "transform move refused")?
        {
            return Ok(self.transform_stale());
        }
        if input.op == 12 {
            self.now_ms = input.now_ms;
            let mut batch = Batch::new();
            self.present(&mut batch, false);
            return Ok(accepted(self.finish(batch, None)));
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
            self.engine
                .hold_velocity(s.token, now_s)
                .unwrap_or(exact_motion::Value::ZERO)
        });
        let measured = [translate.x, translate.y, scale.x];
        input.values[3..].copy_from_slice(&if measured.iter().all(|v| v.is_finite()) {
            measured
        } else {
            [0.0; 3]
        });
        // All six values/time passed preflight, both old holds are live, and the
        // action executes while both still own presentation. Do not lower first.
        let (receipts, error, committed) = self.deliver_at(view, input.event(), input.now_ms);
        let batch = self.commit(&receipts, error);
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
        self.engine
            .advance(input.now_ms / 1000.0)
            .map_err(|_| "transform clock refused")?;
        // Latest authored pair targets/declarations arrive while still held.
        if let Some(node) = self.runner.kernel().node_by_key(input.binding.target) {
            let mut sync = exact_kernel::motion::MotionSync::default();
            sync.transitions
                .push((motion_node(node.key), node.style.transition.clone()));
            for (property, value) in exact_kernel::motion::targets(node.style) {
                if matches!(property, Property::Translate | Property::Scale) {
                    sync.changes.push(exact_motion::Change {
                        node: motion_node(node.key),
                        property,
                        value,
                        velocity: None,
                    });
                }
            }
            sync.apply(&mut self.engine)
                .map_err(|_| "transform authoring refused")?;
        }
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
            let (receipts, error, _) = self.deliver_at(view, input.event(), input.now_ms);
            Ok(accepted(self.commit_into(&receipts, error, batch)))
        } else {
            self.present(&mut batch, false);
            Ok(accepted(self.finish(batch, None)))
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
            .filter(|b| {
                Some(b.target) == self.transform_drags.owner
                    && supported_mapping(self.runner.kernel(), &self.engine, *b)
            })
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
        let live = [a.held.translate(), a.held.scale()].map(|s| self.engine.has_hold(s.token));
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
            let _ = self
                .engine
                .end_hold(start.token, self.engine.now(), HoldEnd::Cancel);
            self.holds.remove(&start.token.serial());
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
                .all(|s| !self.engine.has_hold(s.token))
            {
                self.transform_drags.pairs.remove(&key);
            }
        }
    }

    fn transform_refused(&self) -> String {
        format!(
            "{{\"accepted\":false,\"batch\":{}}}",
            self.refused(Batch::new(), None)
        )
    }

    fn transform_stale(&mut self) -> String {
        let mut batch = Batch::new();
        self.reconcile_transform_drags(&mut batch);
        self.present(&mut batch, false);
        format!(
            "{{\"accepted\":false,\"batch\":{}}}",
            self.finish(batch, None)
        )
    }

    pub(super) fn emit_transform_drags(&mut self, batch: &mut Batch) {
        self.reconcile_transform_owner();
        let kernel = self.runner.kernel();
        let owner = self.transform_drags.owner;
        let mut pending = false;
        for (&view, handle) in &mut self.transform_drags.handles {
            let authored = kernel.transform_drag_binding(handle.key).filter(|b| {
                handle.geometry_handler && handle.release_handler && Some(b.target) == owner
            });
            let binding = authored.filter(|b| supported_mapping(kernel, &self.engine, *b));
            pending |= authored.is_some() && binding.is_none();
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
        self.transform_drags.mapping_pending = pending;
    }
}
fn accepted(batch: String) -> String {
    format!("{{\"accepted\":true,\"batch\":{batch}}}")
}

// Actual native samples are certified again by Swift. This preflight rejects
// ancestor motion even at an identity crossing, before trusting a packet clock.
fn supported_mapping(kernel: &Kernel, engine: &Engine, binding: TransformDragBinding) -> bool {
    let Some(mut node) = kernel.node_by_key(binding.handle) else {
        return false;
    };
    loop {
        for property in [Property::Translate, Property::Scale, Property::Rotate] {
            if node.key == binding.target && property != Property::Rotate {
                continue;
            }
            let key = motion_node(node.key);
            if engine.is_active(key, property)
                || engine
                    .value(key, property)
                    .is_some_and(|v| Some(v) != property.identity())
            {
                return false;
            }
        }
        let Some(parent) = node.parent else {
            return true;
        };
        let Some(next) = kernel.node(parent) else {
            return false;
        };
        node = next;
    }
}
