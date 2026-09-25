//! Arrange on the Apple hosts (LLP 1041 §8.5): the platform recognizes the
//! contact and Swift maps the pointer; this owns the lifted wrapper's hold and
//! asks the collection for a gap, in the web and Linux hosts' order: the final
//! sample, one `reorderdrop` while the source is still held, the rebase of
//! every surviving wrapper, release, and `finish` once the source has settled.
use super::*;
use exact_kernel::CommitReceipt;
use exact_motion::{HoldEnd, Value};
use exact_runner::{ReorderBinding, ReorderProgress, ReorderToken};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Phase {
    Active,
    /// The action or cancel is committing; the rebase follows its layout.
    Committing,
    /// Released; the pin is kept until the source's return ends.
    Settling,
}

pub(super) struct Arrange {
    pub(super) binding: ReorderBinding,
    pub(super) token: ReorderToken,
    pub(super) hold: HoldToken,
    /// The presentation caught at recognition and the List's scrollTop then.
    base: Value,
    scroll: f64,
    phase: Phase,
    /// Presented wrapper positions before the terminal commit (layout space).
    captures: Vec<(NodeKey, Value)>,
    velocity: Value,
    dispatched: bool,
}

fn finite(v: Value) -> bool {
    [v.x, v.y]
        .into_iter()
        .all(|n| n.is_finite() && n.abs() <= f32::MAX as f64)
}

fn timed(receipts: Vec<CommitReceipt>, at_ms: f64) -> Vec<Timed> {
    receipts
        .into_iter()
        .map(|receipt| Timed { at_ms, receipt })
        .collect()
}

impl<D: DataSource> Host<D> {
    /// Catch the handle's row where it is presented. `scroll_top` is the
    /// List's actual offset as its collection feedback reports it; a stale
    /// one, an ineligible handle or a live contact refuses without a hold.
    pub fn reorder_begin(&mut self, handle: ViewId, scroll_top: f64, now_ms: f64) -> String {
        if !now_ms.is_finite() || now_ms < 0. || !scroll_top.is_finite() {
            return self.hold_refusal("invalid reorder sample");
        }
        if self
            .arrange
            .as_ref()
            .is_some_and(|a| a.phase != Phase::Settling)
        {
            return self.reorder_refused(Batch::new(), None);
        }
        let binding = self
            .runner
            .kernel()
            .node(handle)
            .and_then(|n| self.runner.reorder_binding(n.key));
        let Some((binding, geometry)) = binding.and_then(|b| {
            let g = self.runner.reorder_geometry(b.list)?;
            (g.scroll_top == scroll_top).then_some((b, g))
        }) else {
            return self.reorder_refused(Batch::new(), None);
        };
        self.now_ms = self.arrange_now(now_ms);
        let mut batch = Batch::new();
        // A settling predecessor retires before its successor takes the owner.
        let mut receipts = match self.arrange_finish(&mut batch) {
            Ok(r) => r.into_iter().collect::<Vec<_>>(),
            Err(e) => return self.reorder_refused(batch, Some(e)),
        };
        let start = match self.runner.begin_reorder(binding, geometry) {
            Ok(Some(start)) => start,
            Ok(None) => return self.reorder_refused_after(batch, receipts, None),
            Err(e) => return self.reorder_refused_after(batch, receipts, Some(format!("{e:?}"))),
        };
        receipts.extend(start.receipt);
        // The admission receipt adopts the wrapper's translate first; the hold
        // then catches whatever it presents.
        let (mut batch, mut error) = self.commit_tree(&timed(receipts, self.now_ms), None, batch);
        let node = motion_node(binding.wrapper);
        match self
            .engine
            .begin_hold(node, Property::Translate, self.now_ms / 1000., None)
        {
            Ok(Some(hold)) => {
                self.arrange = Some(Arrange {
                    binding,
                    token: start.token,
                    hold: hold.token,
                    base: hold.value,
                    scroll: scroll_top,
                    phase: Phase::Active,
                    captures: Vec::new(),
                    velocity: Value::ZERO,
                    dispatched: false,
                });
                self.reorder_state(&mut batch, start.token.serial());
            }
            result => {
                // No action ran; the admitted preview ends and its pin retires once.
                error = error.or(result.err().map(|e| format!("reorder hold: {e:?}")));
                let mut receipts = Vec::new();
                for step in [
                    self.runner.cancel_reorder(start.token),
                    self.runner.finish_reorder(start.token),
                ] {
                    match step {
                        Ok(r) => receipts.extend(r),
                        Err(e) => error = error.or(Some(format!("{e:?}"))),
                    }
                }
                let (b, e) = self.commit_tree(&timed(receipts, self.now_ms), None, batch);
                batch = b;
                error = error.or(e);
                batch.reorder(0, (0, 0), "refused", false);
            }
        }
        self.commit_finish(batch, error)
    }

    /// Present the source at the pointer: `dy` is its downward travel since
    /// recognition, `scroll_top` the List's actual offset, `inside` whether the
    /// pointer is inside the List's port. A stale token answers its state.
    pub fn reorder_move(
        &mut self,
        serial: u64,
        dy: f64,
        scroll_top: f64,
        inside: bool,
        now_ms: f64,
    ) -> String {
        if !self.arrange_active(serial) {
            return self.reorder_reply(serial, Vec::new(), None);
        }
        if ![dy, scroll_top, now_ms].into_iter().all(f64::is_finite) || now_ms < 0. {
            return self.hold_refusal("invalid reorder sample");
        }
        let now_ms = self.arrange_now(now_ms);
        match self.arrange_sample(dy, scroll_top, inside, now_ms) {
            Ok((_, receipt)) => {
                self.now_ms = now_ms;
                self.reorder_reply(serial, receipt.into_iter().collect(), None)
            }
            Err(e) => self.hold_refusal(&e),
        }
    }

    /// The contact ended: with `drop`, the final sample and, when its gap is
    /// certified, the one `reorderdrop` while the source is held; otherwise a
    /// cancel. `velocity` is the source's downward speed, points per second.
    #[allow(clippy::too_many_arguments)]
    pub fn reorder_end(
        &mut self,
        serial: u64,
        drop: bool,
        dy: f64,
        scroll_top: f64,
        inside: bool,
        velocity: f64,
        now_ms: f64,
    ) -> String {
        if !self.arrange_active(serial) {
            return self.reorder_reply(serial, Vec::new(), None);
        }
        if !now_ms.is_finite()
            || now_ms < 0.
            || drop && ![dy, scroll_top, velocity].into_iter().all(f64::is_finite)
        {
            return self.hold_refusal("invalid reorder sample");
        }
        let now_ms = self.arrange_now(now_ms);
        let mut receipts = Vec::new();
        let mut certified = false;
        if drop {
            match self.arrange_sample(dy, scroll_top, inside, now_ms) {
                Ok((c, receipt)) => {
                    certified = c;
                    receipts.extend(receipt);
                }
                Err(e) => return self.hold_refusal(&e),
            }
        }
        self.now_ms = now_ms;
        let velocity = Value::new(0., if certified { velocity } else { 0. });
        let error = self.arrange_terminal(certified, velocity, &mut receipts);
        self.reorder_reply(serial, receipts, error)
    }

    fn arrange_now(&self, now_ms: f64) -> f64 {
        now_ms.max(self.now_ms).max(self.engine.now() * 1000.)
    }

    fn arrange_active(&self, serial: u64) -> bool {
        self.arrange
            .as_ref()
            .is_some_and(|a| a.phase == Phase::Active && a.token.serial() == serial)
    }

    /// The original binding, preview capability and hold all survive.
    fn arrange_live(&self) -> bool {
        self.arrange.as_ref().is_some_and(|a| {
            a.phase == Phase::Active
                && self.engine.has_hold(a.hold)
                && self.runner.has_reorder(a.token)
                && self.runner.reorder_binding(a.binding.handle) == Some(a.binding)
        })
    }

    /// One pointer sample: the held presentation, then the gap under the
    /// source's centre. A pointer outside the port, or a scroll the runner has
    /// not accepted, asks for an unproved gap, so no older one stays eligible.
    fn arrange_sample(
        &mut self,
        dy: f64,
        scroll_top: f64,
        inside: bool,
        now_ms: f64,
    ) -> Result<(bool, Option<CommitReceipt>), String> {
        let a = self.arrange.as_ref().expect("active arrange");
        let (token, hold, wrapper) = (a.token, a.hold, a.binding.wrapper);
        let value = Value::new(a.base.x, a.base.y + dy + (scroll_top - a.scroll));
        if !finite(value) {
            return Err("reorder position outside finite layout range".into());
        }
        if !self.arrange_live() {
            return Ok((false, None));
        }
        let (Some(g), Some(top), Some(node)) = (
            self.runner.reorder_geometry(token.list()),
            self.runner.reorder_frame(token).and_then(|f| {
                f.wrappers
                    .iter()
                    .find(|w| w.wrapper == wrapper)
                    .map(|w| w.top)
            }),
            self.runner.kernel().node_by_key(wrapper),
        ) else {
            return Ok((false, None));
        };
        let height = node.frame.height as f64;
        self.engine
            .update_hold(hold, now_ms / 1000., value)
            .map_err(|e| format!("reorder hold: {e:?}"))?;
        // The runner clamps to the content, so the unproved probe is its end
        // when that is below the port, else its start above a scrolled port
        // (a fully visible List has no unproved gap: any point is certified).
        let y = if inside && g.scroll_top == scroll_top {
            top + value.y + height / 2.
        } else if g.total_extent > g.scroll_top + g.port_height {
            g.total_extent
        } else {
            0.
        };
        match self
            .runner
            .preview_reorder(token, g, y)
            .map_err(|e| format!("{e:?}"))?
        {
            ReorderProgress::Accepted { receipt } => Ok((true, receipt)),
            ReorderProgress::NeedsMeasurement | ReorderProgress::Stale => Ok((false, None)),
        }
    }

    /// Capture every mounted wrapper's presented position, then drop or
    /// cancel. The rebase and release run after the commit's layout
    /// (`arrange_after_commit`).
    fn arrange_terminal(
        &mut self,
        drop: bool,
        velocity: Value,
        receipts: &mut Vec<CommitReceipt>,
    ) -> Option<String> {
        let token = self.arrange.as_ref().expect("arrange").token;
        let captures = self
            .runner
            .reorder_frame(token)
            .map(|f| f.wrappers)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|w| {
                let node = self.runner.kernel().node_by_key(w.wrapper)?;
                let p = self
                    .engine
                    .value(motion_node(w.wrapper), Property::Translate)?;
                let v = Value::new(node.frame.x as f64 + p.x, node.frame.y as f64 + p.y);
                finite(v).then_some((w.wrapper, v))
            })
            .collect();
        let a = self.arrange.as_mut().expect("arrange");
        a.captures = captures;
        a.velocity = velocity;
        a.phase = Phase::Committing;
        let mut error = None;
        let mut dispatched = false;
        if let Some(g) = drop
            .then(|| self.runner.reorder_geometry(token.list()))
            .flatten()
        {
            match self.runner.drop_reorder(token, g) {
                Ok(Some(receipt)) => {
                    dispatched = true;
                    receipts.push(receipt);
                }
                Ok(None) => {}
                Err(e) => error = Some(format!("{e:?}")),
            }
        }
        if !dispatched {
            match self.runner.cancel_reorder(token) {
                Ok(receipt) => receipts.extend(receipt),
                Err(e) => error = error.or(Some(format!("{e:?}"))),
            }
        }
        self.arrange.as_mut().expect("arrange").dispatched = dispatched;
        error
    }

    /// After a commit's layout: a contact the commit invalidated cancels at
    /// receipt time; a terminal commit rebases and releases; a settled source
    /// finishes, retiring its pin.
    pub(super) fn arrange_after_commit(&mut self, batch: &mut Batch) -> Option<String> {
        let phase = self.arrange.as_ref()?.phase;
        let mut error = None;
        if phase == Phase::Active && !self.arrange_live() {
            let mut receipts = Vec::new();
            error = self.arrange_terminal(false, Value::ZERO, &mut receipts);
            let (b, e) =
                self.commit_tree(&timed(receipts, self.now_ms), None, std::mem::take(batch));
            *batch = b;
            error = error.or(e);
        }
        if self.arrange.as_ref()?.phase == Phase::Committing {
            error = error.or(self.arrange_rebase());
            let serial = self.arrange.as_ref()?.token.serial();
            self.reorder_state(batch, serial);
        }
        if self.arrange_settled() {
            match self.arrange_finish(batch) {
                Ok(receipt) => {
                    let (b, e) = self.commit_tree(
                        &timed(receipt.into_iter().collect(), self.now_ms),
                        None,
                        std::mem::take(batch),
                    );
                    *batch = b;
                    error = error.or(e);
                }
                Err(e) => error = error.or(Some(e)),
            }
        }
        error
    }

    /// Keep each surviving wrapper where it was presented across the new
    /// layout, then release: the source with its velocity, neighbors at rest.
    fn arrange_rebase(&mut self) -> Option<String> {
        let a = self.arrange.as_mut().expect("arrange");
        let captures = std::mem::take(&mut a.captures);
        let (source, hold, velocity) = (a.binding.wrapper, a.hold, a.velocity);
        a.phase = Phase::Settling;
        let now = self.now_ms / 1000.;
        let mut error = None;
        for (key, visual) in captures {
            let Some(node) = self.runner.kernel().node_by_key(key) else {
                continue;
            };
            let value = Value::new(
                visual.x - node.frame.x as f64,
                visual.y - node.frame.y as f64,
            );
            if !finite(value) {
                error = error.or(Some("invalid reorder rebase".into()));
                continue;
            }
            let node = motion_node(key);
            let result = if key == source {
                self.engine.update_hold(hold, now, value).map(|_| ())
            } else if self.engine.is_held(node, Property::Translate) {
                Ok(())
            } else {
                // A neighbor is re-seated without taking any caller's token.
                match self
                    .engine
                    .begin_hold(node, Property::Translate, now, Some(value))
                {
                    Ok(Some(start)) => self
                        .engine
                        .end_hold(
                            start.token,
                            now,
                            HoldEnd::Release {
                                velocity: Value::ZERO,
                            },
                        )
                        .map(|_| ()),
                    Ok(None) => Ok(()),
                    Err(e) => Err(e),
                }
            };
            if let Err(e) = result {
                error = error.or(Some(format!("reorder rebase: {e:?}")));
            }
        }
        if let Err(e) = self
            .engine
            .end_hold(hold, now, HoldEnd::Release { velocity })
        {
            error = error.or(Some(format!("reorder release: {e:?}")));
        }
        error
    }

    /// The source's return has ended (or its row is gone).
    pub(super) fn arrange_settled(&self) -> bool {
        self.arrange.as_ref().is_some_and(|a| {
            a.phase == Phase::Settling
                && (!self
                    .engine
                    .is_active(motion_node(a.binding.wrapper), Property::Translate)
                    || self.runner.reorder_frame(a.token).is_none())
        })
    }

    /// A motion frame found the source settled: finish, and present.
    pub(super) fn arrange_settle(&mut self) -> String {
        let mut batch = Batch::new();
        let (batch, error) = match self.arrange_finish(&mut batch) {
            Ok(receipt) => self.commit_tree(
                &timed(receipt.into_iter().collect(), self.now_ms),
                None,
                batch,
            ),
            Err(e) => (batch, Some(e)),
        };
        self.commit_finish(batch, error)
    }

    /// Retire a settling contact: the runner's descriptor and the pin it owns.
    fn arrange_finish(&mut self, batch: &mut Batch) -> Result<Option<CommitReceipt>, String> {
        let Some(a) = self.arrange.take() else {
            return Ok(None);
        };
        let ids = self.arrange_ids(&a);
        batch.reorder(a.token.serial(), ids, "finished", a.dispatched);
        self.runner
            .finish_reorder(a.token)
            .map_err(|e| format!("{e:?}"))
    }

    fn arrange_ids(&self, a: &Arrange) -> (u32, u32) {
        let id = |key| self.runner.kernel().node_by_key(key).map_or(0, |n| n.id);
        (id(a.binding.list), id(a.binding.wrapper))
    }

    fn reorder_state(&self, batch: &mut Batch, serial: u64) {
        match self.arrange.as_ref().filter(|a| a.token.serial() == serial) {
            Some(a) => {
                let phase = if a.phase == Phase::Active {
                    "active"
                } else {
                    "settling"
                };
                batch.reorder(serial, self.arrange_ids(a), phase, a.dispatched);
            }
            None => batch.reorder(serial, (0, 0), "finished", false),
        }
    }

    fn reorder_reply(
        &mut self,
        serial: u64,
        receipts: Vec<CommitReceipt>,
        error: Option<String>,
    ) -> String {
        let (mut batch, e) = self.commit_tree(&timed(receipts, self.now_ms), error, Batch::new());
        let after = self.arrange_after_commit(&mut batch);
        self.reorder_state(&mut batch, serial);
        self.commit_finish(batch, e.or(after))
    }

    fn reorder_refused(&mut self, mut batch: Batch, error: Option<String>) -> String {
        batch.reorder(0, (0, 0), "refused", false);
        self.finish(batch, error)
    }

    fn reorder_refused_after(
        &mut self,
        batch: Batch,
        receipts: Vec<CommitReceipt>,
        error: Option<String>,
    ) -> String {
        let (mut batch, e) = self.commit_tree(&timed(receipts, self.now_ms), error, batch);
        batch.reorder(0, (0, 0), "refused", false);
        self.commit_finish(batch, e)
    }
}
