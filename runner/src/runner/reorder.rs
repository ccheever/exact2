//! Collection-certified Arrange. No host clock, motion executor or data preview.
//! A list with a `reorderGroup` adds a target and a hold (`reorder_group.rs`,
//! LLP 1094); every call here keeps the source's token.
use super::reorder_group::Session;
use super::*;
use crate::instance::collection::{
    Collection, ReorderBinding, ReorderFrame, ReorderGeometry, ReorderPhase, ReorderProgress,
    ReorderStart, ReorderToken,
};
use exact_kernel::{Display, NodeKey, NodeType, Op, PropId};
use std::sync::atomic::{AtomicU64, Ordering};
static SERIAL: AtomicU64 = AtomicU64::new(1);

impl<D: DataSource> Runner<D> {
    /// Resolve a strict authored ancestor IDREF, then join only mounted collection
    /// rows. String key, current positive measurement and List handler are required.
    pub fn reorder_binding(&self, handle: NodeKey) -> Option<ReorderBinding> {
        if self.poisoned {
            return None;
        }
        let node = self.kernel.node_by_key(handle)?;
        let name = node.props.str(PropId::ReorderFor)?;
        if name.is_empty()
            || node.props.str(PropId::HeightDragFor).is_some()
            || node.props.str(PropId::TransformDragFor).is_some()
        {
            return None;
        }
        let mut at = node;
        let mut list = None;
        loop {
            if at.style.display == Display::None
                || at.props.bool(PropId::Disabled) == Some(true)
                || at.props.bool(PropId::Inert) == Some(true)
            {
                return None;
            }
            if at.key != handle && at.props.str(PropId::Id) == Some(name) {
                if list.is_some() {
                    return None;
                }
                list = Some(at.key);
            }
            if at.is_root {
                break;
            }
            at = self.kernel.node(at.parent?)?;
        }
        let list = self.kernel.node_by_key(list?)?;
        if list.node_type != NodeType::List
            || !self.handlers_of(list.id).contains(&EventKind::Reorderdrop)
        {
            return None;
        }
        let binding = self
            .tree
            .as_ref()?
            .reorder_collection(list.id)?
            .reorder_binding(&self.kernel, handle)?;
        let mut at = node;
        loop {
            if at.style.rotate != 0.0
                || at.style.scale != 1.0
                || (at.key != binding.wrapper
                    && (at.style.translate.x != 0.0
                        || at.style.translate.y != 0.0
                        || at.style.translate_percent.x != 0.0
                        || at.style.translate_percent.y != 0.0))
            {
                return None;
            }
            if at.is_root {
                break;
            }
            at = self.kernel.node(at.parent?)?;
        }
        Some(binding)
    }
    /// Latest accepted scroll facts. Hosts also certify their own viewport mapping.
    pub fn reorder_geometry(&self, list: NodeKey) -> Option<ReorderGeometry> {
        let view = self.kernel.node_by_key(list)?.id;
        self.tree
            .as_ref()?
            .reorder_collection(view)?
            .reorder_geometry(list)
    }
    /// Admit one preview only after the host acquired the existing interaction pin.
    /// Refused identity/facts do not consume a serial or change any styles/pins.
    /// The session stays in its list, a grouped one too; a host that drags
    /// between lists begins with `begin_group_reorder` (LLP 1094 D4).
    pub fn begin_reorder(
        &mut self,
        binding: ReorderBinding,
        geometry: ReorderGeometry,
    ) -> Result<Option<ReorderStart>, RunnerError> {
        self.begin_session(binding, geometry, None, false)
    }
    pub(super) fn begin_session(
        &mut self,
        binding: ReorderBinding,
        geometry: ReorderGeometry,
        group: Option<String>,
        ghost: bool,
    ) -> Result<Option<ReorderStart>, RunnerError> {
        if self.reorder_owner.is_some()
            || self.reorder_binding(binding.handle) != Some(binding)
            || self.reorder_geometry(binding.list).as_ref() != Some(&geometry)
        {
            return Ok(None);
        }
        let serial = SERIAL
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
            .map_err(|_| reorder_error("reorder serial exhausted"))?;
        let token = ReorderToken {
            list: binding.list,
            serial,
        };
        let handle = self.kernel.node_by_key(binding.handle).unwrap().id;
        let grouped = group.is_some();
        let (accepted, ops) = self.edit_reorder(binding.list, |c, u, _| {
            if !c.begin_preview(u, binding, token, handle)? {
                return Ok(None);
            }
            let item = c.preview_item();
            if grouped {
                c.set_grouped();
                if ghost {
                    c.set_hidden(u, item.as_ref().map(|(ident, _)| ident.clone()))?;
                }
            }
            Ok(item)
        })?;
        let Some(Some((item, key))) = accepted else {
            return Ok(None);
        };
        self.reorder_owner = Some(binding.list);
        self.reorder_session = Some(Box::new(Session {
            token,
            target: binding.list,
            group,
            phase: ReorderPhase::Active,
            ending: None,
            deadline_ms: None,
            item,
            key,
            before: None,
            place: None,
            ghost,
            touched: vec![binding.list],
            warned: false,
        }));
        let receipt = self.apply_reorder_ops(ops)?;
        Ok(Some(ReorderStart { token, receipt }))
    }
    /// True only while the original source binding, pin and action capability live.
    pub fn has_reorder(&self, token: ReorderToken) -> bool {
        let Some(c) = self.reorder_collection(token.list) else {
            return false;
        };
        c.preview_active(token)
            && c.preview_binding()
                .is_some_and(|b| self.reorder_binding(b.handle) == Some(b))
    }
    /// Measured gap selection in unpreviewed content coordinates. No queries,
    /// all-key serialization or destination pin; stale precedes numeric validation.
    pub fn preview_reorder(
        &mut self,
        token: ReorderToken,
        geometry: ReorderGeometry,
        content_y: f64,
    ) -> Result<ReorderProgress, RunnerError> {
        if self.is_grouped(token) {
            return self.preview_reorder_into(token, token.list, geometry, content_y);
        }
        if !self.has_reorder(token) || self.reorder_geometry(token.list).as_ref() != Some(&geometry)
        {
            return Ok(ReorderProgress::Stale);
        }
        if !content_y.is_finite() || content_y.abs() > f32::MAX as f64 {
            return Err(reorder_error("invalid reorder coordinate"));
        }
        let (accepted, ops) = self.edit_reorder(token.list, |c, u, _| {
            c.move_preview(u, geometry.clone(), content_y)
        })?;
        if accepted != Some(true) {
            return Ok(ReorderProgress::NeedsMeasurement);
        }
        Ok(ReorderProgress::Accepted {
            receipt: self.apply_reorder_ops(ops)?,
        })
    }
    /// Consume terminal eligibility once and dispatch exact private keys while the
    /// source pin remains owned. Host rebase/end MUST precede `finish_reorder`.
    /// A grouped session's `geometry` is its target's; it drops there and
    /// may hold (`ReorderFrame::phase`, LLP 1094 D8).
    pub fn drop_reorder(
        &mut self,
        token: ReorderToken,
        geometry: ReorderGeometry,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        if self.is_grouped(token) {
            return self.drop_group(token, geometry);
        }
        if !self.has_reorder(token) || self.reorder_geometry(token.list).as_ref() != Some(&geometry)
        {
            return Ok(None);
        }
        let (event, ops) = self.edit_reorder(token.list, |c, u, _| c.preview_drop(u, &geometry))?;
        let Some(Some((item, before))) = event else {
            return Ok(None);
        };
        // Reset targets join the action's one instance commit. The descriptor is
        // already terminal, so reentry cannot dispatch twice or reacquire it.
        self.reorder_ops.extend(ops);
        self.set_phase(token, ReorderPhase::Settling);
        let view = self.kernel.node_by_key(token.list).unwrap().id;
        let result = self.dispatch(view, Event::ReorderDrop { item, before });
        if result.is_err() {
            // Action validation/data refusal may precede apply. Do not leak its
            // queued style operations into a later unrelated user action.
            let ops = std::mem::take(&mut self.reorder_ops);
            self.apply_reorder_ops(ops)?;
        }
        result.map(Some)
    }
    /// Enter the no-action terminal phase; retain the pin until host hold cleanup.
    /// A hold is past cancelling: its send is out (LLP 1094 D8).
    pub fn cancel_reorder(
        &mut self,
        token: ReorderToken,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        let Some(c) = self.reorder_collection(token.list) else {
            return Ok(None);
        };
        if !c.preview_active(token) {
            return Ok(None);
        }
        if self.is_grouped(token) {
            let ops = self.end_session(None, false)?;
            return self.apply_reorder_ops(ops);
        }
        let (_, ops) = self.edit_reorder(token.list, |c, u, _| c.end_preview(u))?;
        self.set_phase(token, ReorderPhase::Cancelling);
        self.apply_reorder_ops(ops)
    }
    /// Bounded live wrapper identities/targets for host before/after C0 rebasing.
    /// Current viewport presentation and mapping remain the host's responsibility.
    /// It says where the session stands, its target, and which mounted
    /// wrapper holds the dragged row now (LLP 1094 D4, D8).
    pub fn reorder_frame(&self, token: ReorderToken) -> Option<ReorderFrame> {
        let mut frame = self
            .reorder_collection(token.list)?
            .preview_frame(&self.kernel, token)?;
        let Some(s) = self.reorder_session.as_deref().filter(|s| s.token == token) else {
            return Some(frame);
        };
        frame.phase = s.phase;
        frame.ending = s.ending;
        frame.target = Some(s.target);
        let mut lists = vec![s.target, token.list];
        if let Some(group) = s.group.as_deref() {
            lists.extend(self.group_lists(group));
        }
        frame.row = lists.into_iter().find_map(|list| {
            self.reorder_collection(list)?
                .wrapper_of(&self.kernel, &s.item)
        });
        Some(frame)
    }
    /// Whether `token` is a live session on a grouped list.
    pub(super) fn is_grouped(&self, token: ReorderToken) -> bool {
        self.reorder_session
            .as_deref()
            .is_some_and(|s| s.token == token && s.group.is_some())
    }
    fn set_phase(&mut self, token: ReorderToken, phase: ReorderPhase) {
        if let Some(s) = self
            .reorder_session
            .as_deref_mut()
            .filter(|s| s.token == token)
        {
            s.phase = phase;
        }
    }
    /// Retire the terminal descriptor and only its still-owned interaction pin.
    /// Hosts may defer this through the source return spring; finish only when
    /// presentation no longer needs the pin, or before explicitly replacing it.
    /// A stale token cannot clear a successor; deletion needs no source to survive.
    pub fn finish_reorder(
        &mut self,
        token: ReorderToken,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        if self.reorder_owner != Some(token.list) {
            return Ok(None);
        }
        let Some(c) = self.reorder_collection(token.list) else {
            self.reorder_owner = None;
            return Ok(None);
        };
        if c.preview_token() != Some(token) {
            return Ok(None);
        }
        let (finished, mut ops) = self.edit_reorder(token.list, |c, u, frames| {
            c.finish_preview(u, frames, token)
        })?;
        if finished != Some(true) {
            return Ok(None);
        }
        self.reorder_owner = None;
        // Every row a grouped session hid shows again, and any gap left
        // open closes (LLP 1094 D6).
        if let Some(s) = self.reorder_session.take() {
            for list in s.touched {
                let (_, o) = self.edit_reorder(list, |c, u, _| {
                    c.close_incoming(u, false)?;
                    c.set_hidden(u, None)
                })?;
                ops.extend(o);
            }
        }
        self.apply(ops).map(Some)
    }
    pub(super) fn reorder_collection(&self, key: NodeKey) -> Option<&Collection> {
        let id = self.kernel.node_by_key(key)?.id;
        self.tree.as_ref()?.reorder_collection(id)
    }
    pub(super) fn edit_reorder<T>(
        &mut self,
        key: NodeKey,
        edit: impl FnMut(&mut Collection, &mut Update<'_>, &[Frame]) -> Result<T, InstanceError>,
    ) -> Result<(Option<T>, Vec<Op>), RunnerError> {
        if self.poisoned {
            return Err(RunnerError::Poisoned);
        }
        let Some(id) = self.kernel.node_by_key(key).map(|n| n.id) else {
            return Ok((None, Vec::new()));
        };
        let mut tree = self.tree.take().expect("booted");
        let mut ids = std::mem::take(&mut self.ids);
        let result = {
            let mut u = Update::new(self.env(&[], &[]), &self.sites, &mut ids);
            tree.edit_reorder(id, &mut u, edit)
                .map(|r| (r, u.ops, u.notes))
        };
        self.tree = Some(tree);
        self.ids = ids;
        let result = result.map(|(r, ops, notes)| {
            self.notes.extend(notes);
            (r, ops)
        });
        result.map_err(Into::into)
    }
    pub(super) fn apply_reorder_ops(
        &mut self,
        ops: Vec<Op>,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        if ops.is_empty() {
            Ok(None)
        } else {
            self.apply(ops).map(Some)
        }
    }
    pub(super) fn reconcile_reorder(&mut self) -> Result<Vec<Op>, RunnerError> {
        let Some(owner) = self.reorder_owner else {
            return Ok(Vec::new());
        };
        let Some(c) = self.reorder_collection(owner) else {
            self.reorder_owner = None;
            self.reorder_session = None;
            return Ok(Vec::new());
        };
        let Some(token) = c.preview_token() else {
            self.reorder_owner = None;
            self.reorder_session = None;
            return Ok(Vec::new());
        };
        if self.is_grouped(token) {
            return self.reconcile_group();
        }
        if !c.preview_active(token) || self.has_reorder(token) {
            return Ok(Vec::new());
        }
        self.edit_reorder(owner, |c, u, _| c.end_preview(u))
            .map(|(_, ops)| ops)
    }
}
fn reorder_error(message: &str) -> RunnerError {
    InstanceError::Collection(message.into()).into()
}
