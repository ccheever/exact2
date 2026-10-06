//! A reorder across the lists that share a `reorderGroup` (LLP 1094): one
//! session keeps the source's token and owner for the whole gesture and
//! adds a target and a phase (D4). Samples move the gap in whichever list
//! the host names (`preview_reorder_into`), a drop fires once on the
//! target, and a hold waits for the move to show (D8); keys and custom
//! actions step the gap (D9). The collections hold their halves
//! (`instance/collection/reorder_group.rs`).
use super::*;
use crate::instance::collection::{
    IntoView, ReorderBinding, ReorderEnding, ReorderGeometry, ReorderPhase, ReorderProgress,
    ReorderStart, ReorderStep, ReorderToken,
};
use exact_kernel::{NodeKey, NodeType, Op, PropId};

/// How long a drop's hold waits for its move, on the session clock (LLP
/// 1094 D8; §9 Q1).
pub(super) const HOLD_MS: f64 = 1000.0;

/// One gesture's session; `group` is `None` for a list without one, whose
/// reorder stays within it (LLP 1041 §8.5).
#[derive(Debug)]
pub(crate) struct Session {
    pub(super) token: ReorderToken,
    /// The list a drop lands in, the source's own at first.
    pub(super) target: NodeKey,
    pub(super) group: Option<String>,
    pub(super) phase: ReorderPhase,
    pub(super) ending: Option<ReorderEnding>,
    pub(super) deadline_ms: Option<f64>,
    /// The dragged row's identity in an index, and its key.
    pub(super) item: String,
    pub(super) key: String,
    /// The row it was dropped before (an identity), and where it was then:
    /// its list and its neighbours. A hold watches for a change of either.
    pub(super) before: Option<String>,
    pub(super) place: Option<(NodeKey, Option<String>, Option<String>)>,
    /// The host draws a ghost (D6): the row stays hidden until `finish`.
    /// Without one it hides only while another list is the target.
    pub(super) ghost: bool,
    /// Every list this session offset or hid a row in.
    pub(super) touched: Vec<NodeKey>,
    /// The journal said once that a list holding the row is no target.
    pub(super) warned: bool,
}

impl Session {
    fn touch(&mut self, list: NodeKey) {
        if !self.touched.contains(&list) {
            self.touched.push(list);
        }
    }
}

impl<D: DataSource> Runner<D> {
    /// A list's non-empty `reorderGroup` (D1).
    pub(super) fn reorder_group(&self, list: NodeKey) -> Option<String> {
        let node = self.kernel.node_by_key(list)?;
        if node.node_type != NodeType::List {
            return None;
        }
        node.props
            .str(PropId::ReorderGroup)
            .filter(|g| !g.is_empty())
            .map(str::to_owned)
    }

    /// The mounted lists of `group` that have a collection, in tree order
    /// (D9's left and right).
    pub(super) fn group_lists(&self, group: &str) -> Vec<NodeKey> {
        let arena = self.kernel.arena();
        let Some(tree) = self.tree.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut stack: Vec<u32> = arena.roots().iter().rev().copied().collect();
        while let Some(slot) = stack.pop() {
            if arena.node_type(slot) == NodeType::List
                && arena.props(slot).str(PropId::ReorderGroup) == Some(group)
                && tree.reorder_collection(arena.local_id(slot)).is_some()
            {
                out.push(arena.key(slot));
            }
            stack.extend(arena.children(slot).iter().rev());
        }
        out
    }

    /// Begin a session on a grouped list (D4, D6): `begin_reorder`'s
    /// checks, with `ghost` when the host draws the row in its top layer,
    /// so the row itself hides until `finish_reorder`. A key's or a custom
    /// action's session (D9) has no ghost. `None` for a list without a
    /// group, or whatever `begin_reorder` refuses.
    pub fn begin_group_reorder(
        &mut self,
        binding: ReorderBinding,
        geometry: ReorderGeometry,
        ghost: bool,
    ) -> Result<Option<ReorderStart>, RunnerError> {
        let Some(group) = self.reorder_group(binding.list) else {
            return Ok(None);
        };
        self.begin_session(binding, geometry, Some(group), ghost)
    }

    /// The active grouped session `token` names.
    fn grouped(&self, token: ReorderToken) -> Option<&Session> {
        self.reorder_session
            .as_deref()
            .filter(|s| s.token == token && s.group.is_some())
    }

    /// Move the gap to `y` in `target`'s unpreviewed content coordinates
    /// (D4, D5): `geometry` is the target's, certified as the source's is
    /// for `preview_reorder`. A target that is not the session's yet takes
    /// over in one commit (`Outgoing` on the source, `Incoming` on it). A
    /// list that already holds the row is no target: the sticky target
    /// stays and the journal says so once (D1).
    pub fn preview_reorder_into(
        &mut self,
        token: ReorderToken,
        target: NodeKey,
        geometry: ReorderGeometry,
        y: f64,
    ) -> Result<ReorderProgress, RunnerError> {
        let Some(s) = self.grouped(token) else {
            return Ok(ReorderProgress::Stale);
        };
        let (group, item, current) = (s.group.clone(), s.item.clone(), s.target);
        if s.phase != ReorderPhase::Active
            || !self.has_reorder(token)
            || self.reorder_geometry(target).as_ref() != Some(&geometry)
        {
            return Ok(ReorderProgress::Stale);
        }
        if !y.is_finite() || y.abs() > f32::MAX as f64 {
            return Err(group_error("invalid reorder coordinate"));
        }
        let source = token.list;
        if target != source {
            if self.reorder_group(target) != group {
                return Ok(ReorderProgress::Stale);
            }
            if self
                .reorder_collection(target)
                .is_some_and(|c| c.holds(&item))
            {
                self.refuse_holder(target);
                return Ok(ReorderProgress::Accepted { receipt: None });
            }
        }
        let mut ops = Vec::new();
        let accepted = if target == current {
            if target == source {
                let (ok, o) =
                    self.edit_reorder(source, |c, u, _| c.move_preview(u, geometry.clone(), y))?;
                ops.extend(o);
                ok == Some(true)
            } else {
                let (ok, o) =
                    self.edit_reorder(target, |c, u, _| c.move_incoming(u, geometry.clone(), y))?;
                ops.extend(o);
                ok == Some(true)
            }
        } else if target == source {
            // Home again: today's preview, and the old target's gap closes.
            let (ok, o) =
                self.edit_reorder(source, |c, u, _| c.retarget_home(u, geometry.clone(), y))?;
            ops.extend(o);
            if ok == Some(true) {
                let (_, o) = self.edit_reorder(current, |c, u, _| c.close_incoming(u, false))?;
                ops.extend(o);
            }
            ok == Some(true)
        } else {
            let extent = self
                .reorder_collection(source)
                .and_then(|c| c.preview_extent())
                .unwrap_or(0.0);
            let (ok, o) = self.edit_reorder(target, |c, u, _| {
                c.open_incoming(item.clone(), extent);
                let ok = c.move_incoming(u, geometry.clone(), y)?;
                if !ok {
                    c.abandon_incoming();
                }
                Ok(ok)
            })?;
            ops.extend(o);
            if ok == Some(true) {
                ops.extend(self.leave_target(current, source)?);
            }
            ok == Some(true)
        };
        if accepted {
            if let Some(s) = self.reorder_session.as_deref_mut() {
                s.target = target;
                s.touch(target);
            }
        } else if target != current {
            // An unproved sample over a new target leaves no older gap
            // eligible for the drop, as an unproved sample does in one list.
            ops.extend(self.uncertify(current)?);
        }
        let receipt = self.apply_reorder_ops(ops)?;
        Ok(if accepted {
            ReorderProgress::Accepted { receipt }
        } else {
            ReorderProgress::NeedsMeasurement
        })
    }

    /// The old target lets go as a new foreign one takes over: the source
    /// closes the gap behind its row (`Outgoing`); a foreign one closes its
    /// gap. Without a ghost the row stays shown: it is the focused grip's,
    /// and a hidden one would lose the keys (D9).
    fn leave_target(&mut self, current: NodeKey, source: NodeKey) -> Result<Vec<Op>, RunnerError> {
        let mut ops = Vec::new();
        if current == source {
            let (_, o) = self.edit_reorder(source, |c, u, _| c.set_outgoing(u, true))?;
            ops.extend(o);
        } else {
            let (_, o) = self.edit_reorder(current, |c, u, _| c.close_incoming(u, false))?;
            ops.extend(o);
        }
        Ok(ops)
    }

    fn uncertify(&mut self, list: NodeKey) -> Result<Vec<Op>, RunnerError> {
        let (_, ops) = self.edit_reorder(list, |c, _, _| {
            c.uncertify();
            Ok(())
        })?;
        Ok(ops)
    }

    /// The journal's one line a drag about a list that holds the row (D1).
    fn refuse_holder(&mut self, list: NodeKey) {
        let Some(s) = self.reorder_session.as_deref_mut() else {
            return;
        };
        if std::mem::replace(&mut s.warned, true) {
            return;
        }
        let id = self.list_id(list);
        self.log(format!(
            "reorderdrop: {id} already holds the dragged row, so it is not a target"
        ));
    }

    /// A list's `id`, as `ReorderEvent` names it.
    pub(super) fn list_id(&self, list: NodeKey) -> String {
        self.kernel
            .node_by_key(list)
            .and_then(|n| n.props.str(PropId::Id).map(str::to_owned))
            .unwrap_or_default()
    }

    /// A grouped drop (D2, D8): the target's eligibility consumed once,
    /// the hold armed, then the target's `reorderdrop` with the keys and
    /// `ReorderEvent { from, to }`. The commit that shows the move ends the
    /// hold (`reconcile_reorder`), the drop's own when the action is
    /// synchronous.
    pub(super) fn drop_group(
        &mut self,
        token: ReorderToken,
        geometry: ReorderGeometry,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        let Some(s) = self.grouped(token) else {
            return Ok(None);
        };
        let (target, key) = (s.target, s.key.clone());
        if s.phase != ReorderPhase::Active
            || !self.has_reorder(token)
            || self.reorder_geometry(target).as_ref() != Some(&geometry)
        {
            return Ok(None);
        }
        let source = token.list;
        let (item, before) = if target == source {
            let (drop, _) = self.edit_reorder(source, |c, _, _| c.hold_drop(&geometry))?;
            let Some(Some((item, before))) = drop else {
                return Ok(None);
            };
            (item, before)
        } else {
            let (drop, _) = self.edit_reorder(target, |c, _, _| Ok(c.incoming_drop(&geometry)))?;
            let Some(Some(before)) = drop else {
                return Ok(None);
            };
            self.edit_reorder(source, |c, _, _| {
                c.hold_outgoing();
                Ok(())
            })?;
            (key, before)
        };
        let ident = |k: &Option<String>| k.as_deref().map(|k| format!("s:{k}"));
        let place = self
            .reorder_collection(source)
            .and_then(|c| c.place_of(&format!("s:{item}")))
            .map(|(a, b)| (source, a, b));
        let deadline = self.now_ms + HOLD_MS;
        if let Some(s) = self.reorder_session.as_deref_mut() {
            s.phase = ReorderPhase::Holding;
            s.deadline_ms = Some(deadline);
            s.before = ident(&before);
            s.place = place;
        }
        let view = self.kernel.node_by_key(target).unwrap().id;
        self.dropping_from = Some(source);
        let result = self.dispatch(view, Event::ReorderDrop { item, before });
        self.dropping_from = None;
        if result.is_err() {
            // A refused action moves nothing: the hold ends at once and
            // the rows go back, as a cancel's do.
            let ops = self.end_session(None, false)?;
            self.apply_reorder_ops(ops)?;
        }
        result.map(Some)
    }

    /// `reorderdrop`'s `ReorderEvent` (D2): the list the row came from and
    /// the one it was dropped on, by `id`; the same list for a drop within
    /// one and for a host's own `reorderdrop`.
    pub(super) fn reorder_record(&self, view: exact_kernel::ViewId) -> exact_plan::Value {
        let to = self
            .kernel
            .node(view)
            .and_then(|n| n.props.str(PropId::Id).map(str::to_owned))
            .unwrap_or_default();
        let from = self
            .dropping_from
            .map_or_else(|| to.clone(), |l| self.list_id(l));
        exact_plan::Value::record(vec![
            exact_plan::Value::str(&from),
            exact_plan::Value::str(&to),
        ])
    }

    /// Where a hold stands (D8): landed (where it was dropped, or where
    /// the action put it), gone from every grouped list, or still waiting
    /// where it was at the drop.
    fn hold_outcome(&self, s: &Session) -> Option<ReorderEnding> {
        if let Some((_, after)) = self
            .reorder_collection(s.target)
            .and_then(|c| c.place_of(&s.item))
        {
            if after == s.before {
                return Some(ReorderEnding::Landed);
            }
        }
        let group = s.group.as_deref()?;
        let found = self.group_lists(group).into_iter().find_map(|list| {
            let (a, b) = self.reorder_collection(list)?.place_of(&s.item)?;
            Some((list, a, b))
        });
        match found {
            None => Some(ReorderEnding::Gone),
            Some(place) if Some(&place) == s.place.as_ref() => None,
            Some(_) => Some(ReorderEnding::Landed),
        }
    }

    /// Close a grouped session's previews and enter its last phase: a
    /// drop's ending (`Settling`) or a cancel (`Cancelling`, `ending` none).
    /// `instant` when the rows moved in this commit. A ghost's row stays
    /// hidden wherever it now is, until `finish_reorder`; without a ghost
    /// every row shows again.
    pub(super) fn end_session(
        &mut self,
        ending: Option<ReorderEnding>,
        instant: bool,
    ) -> Result<Vec<Op>, RunnerError> {
        let Some(s) = self.reorder_session.as_deref() else {
            return Ok(Vec::new());
        };
        let (source, target, touched, item, ghost, dropped) = (
            s.token.list,
            s.target,
            s.touched.clone(),
            s.item.clone(),
            s.ghost,
            s.phase == ReorderPhase::Holding,
        );
        let mut ops = Vec::new();
        let (_, o) = self.edit_reorder(source, |c, u, _| c.end_hold(u, instant))?;
        ops.extend(o);
        if target != source {
            let (_, o) = self.edit_reorder(target, |c, u, _| c.close_incoming(u, instant))?;
            ops.extend(o);
        }
        for list in touched {
            let hide = (ghost
                && self
                    .reorder_collection(list)
                    .is_some_and(|c| c.holds(&item)))
            .then(|| item.clone());
            let (_, o) = self.edit_reorder(list, |c, u, _| c.set_hidden(u, hide.clone()))?;
            ops.extend(o);
        }
        if let Some(s) = self.reorder_session.as_deref_mut() {
            s.phase = if dropped || ending.is_some() {
                ReorderPhase::Settling
            } else {
                ReorderPhase::Cancelling
            };
            s.ending = ending;
            s.deadline_ms = None;
        }
        Ok(ops)
    }

    /// The session's commit-end check (D8), from `reconcile_reorder`: an
    /// active session whose grip died is cancelled, one whose foreign
    /// target went goes home, and a hold ends at the first commit that
    /// shows the move or loses the row.
    pub(super) fn reconcile_group(&mut self) -> Result<Vec<Op>, RunnerError> {
        let Some(s) = self
            .reorder_session
            .as_deref()
            .filter(|s| s.group.is_some())
        else {
            return Ok(Vec::new());
        };
        let (token, target, phase) = (s.token, s.target, s.phase);
        match phase {
            ReorderPhase::Holding => match self.hold_outcome(s) {
                Some(ending) => self.end_session(Some(ending), true),
                None => Ok(Vec::new()),
            },
            ReorderPhase::Active => {
                if self.reorder_collection(token.list).is_none() {
                    return Ok(Vec::new());
                }
                if !self.has_reorder(token) {
                    return self.end_session(None, false);
                }
                if target != token.list && self.reorder_collection(target).is_none() {
                    let (_, ops) =
                        self.edit_reorder(token.list, |c, u, _| c.set_outgoing(u, false))?;
                    if let Some(s) = self.reorder_session.as_deref_mut() {
                        s.target = token.list;
                    }
                    return Ok(ops);
                }
                Ok(Vec::new())
            }
            _ => Ok(Vec::new()),
        }
    }

    /// The hold's deadline, which `timer_due_ms` reports beside the plan's
    /// timers so a host wakes for it (D8).
    pub(super) fn reorder_deadline(&self) -> Option<f64> {
        self.reorder_session
            .as_deref()
            .filter(|s| s.phase == ReorderPhase::Holding)
            .and_then(|s| s.deadline_ms)
    }

    /// The deadline passed (D8): the previews close, the row springs onto
    /// wherever it is, and the journal says so. One commit, `ending:
    /// timeout`.
    pub(super) fn reorder_timeout(&mut self) -> Result<CommitReceipt, RunnerError> {
        self.log("reorderdrop: the move did not show within 1 s");
        let ops = self.end_session(Some(ReorderEnding::Timeout), false)?;
        self.apply(ops)
    }

    /// One key's or custom action's move (D9): the gap a row up or down in
    /// the target, clamped, or the same index in the previous or next
    /// grouped list in tree order that does not hold the row, clamped to
    /// its end; then the gap is kept in view (`scrollIntoView`'s
    /// `nearest`). Its drop needs no measured sample.
    pub fn reorder_step(
        &mut self,
        token: ReorderToken,
        step: ReorderStep,
    ) -> Result<Vec<CommitReceipt>, RunnerError> {
        let Some(s) = self.grouped(token) else {
            return Ok(Vec::new());
        };
        let (group, item, current) = (s.group.clone().unwrap(), s.item.clone(), s.target);
        if s.phase != ReorderPhase::Active || !self.has_reorder(token) {
            return Ok(Vec::new());
        }
        let source = token.list;
        let Some((slot, _)) = self.reorder_collection(current).and_then(|c| c.step_slot()) else {
            return Ok(Vec::new());
        };
        let mut ops = Vec::new();
        let target = match step.along() {
            Some(by) => {
                let next = slot.saturating_add_signed(by);
                let (_, o) = self.edit_reorder(current, |c, u, _| c.step_to(u, next))?;
                ops.extend(o);
                current
            }
            None => {
                let lists = self.group_lists(&group);
                let Some(at) = lists.iter().position(|l| *l == current) else {
                    return Ok(Vec::new());
                };
                let ahead: Vec<NodeKey> = if step == ReorderStep::NextList {
                    lists[at + 1..].to_vec()
                } else {
                    lists[..at].iter().rev().copied().collect()
                };
                let Some(target) = ahead.into_iter().find(|l| {
                    *l == source || !self.reorder_collection(*l).is_some_and(|c| c.holds(&item))
                }) else {
                    return Ok(Vec::new());
                };
                if target == source {
                    let (_, o) = self.edit_reorder(source, |c, u, _| {
                        c.set_outgoing(u, false)?;
                        c.step_to(u, slot)
                    })?;
                    ops.extend(o);
                    let (_, o) =
                        self.edit_reorder(current, |c, u, _| c.close_incoming(u, false))?;
                    ops.extend(o);
                } else {
                    let extent = self
                        .reorder_collection(source)
                        .and_then(|c| c.preview_extent())
                        .unwrap_or(0.0);
                    let (_, o) = self.edit_reorder(target, |c, u, _| {
                        c.open_incoming(item.clone(), extent);
                        c.step_to(u, slot)
                    })?;
                    ops.extend(o);
                    ops.extend(self.leave_target(current, source)?);
                }
                if let Some(s) = self.reorder_session.as_deref_mut() {
                    s.target = target;
                    s.touch(target);
                }
                target
            }
        };
        let mut receipts: Vec<CommitReceipt> = self.apply_reorder_ops(ops)?.into_iter().collect();
        let neighbour = self
            .reorder_collection(target)
            .and_then(|c| c.step_neighbour_key());
        if let (Some(key), Some(view)) = (neighbour, self.kernel.node_by_key(target).map(|n| n.id))
        {
            receipts.push(self.scroll_into_view(IntoView {
                list: format!("#{view}"),
                key: exact_plan::Value::str(&key),
                block: crate::instance::collection::Align::Nearest,
                inline: crate::instance::collection::Align::Nearest,
                row: None,
                view: Some(view),
                smooth: false,
            })?);
        }
        Ok(receipts)
    }

    /// `state.reorder` (D12): the session's row, its two lists, the key the
    /// gap is before and its phase; `null` with none.
    pub fn reorder_json(&self) -> String {
        let Some(s) = self.reorder_session.as_deref() else {
            return "null".into();
        };
        let before = match s.phase {
            ReorderPhase::Active => self
                .reorder_collection(s.target)
                .and_then(|c| c.gap_before(s.target == s.token.list)),
            _ => s.before.clone(),
        };
        let mut out = String::from("{\"item\":");
        crate::agent::quote(&s.key, &mut out);
        out.push_str(",\"from\":");
        crate::agent::quote(&self.list_id(s.token.list), &mut out);
        out.push_str(",\"to\":");
        crate::agent::quote(&self.list_id(s.target), &mut out);
        out.push_str(",\"before\":");
        match before.as_deref().and_then(|b| b.strip_prefix("s:")) {
            Some(key) => crate::agent::quote(key, &mut out),
            None => out.push_str("null"),
        }
        out.push_str(",\"phase\":");
        crate::agent::quote(s.phase.name(), &mut out);
        out.push_str(",\"ending\":");
        match s.ending {
            Some(e) => crate::agent::quote(e.name(), &mut out),
            None => out.push_str("null"),
        }
        out.push('}');
        out
    }
}

fn group_error(message: &str) -> RunnerError {
    crate::instance::InstanceError::Collection(message.into()).into()
}
