//! Dropping across lists on the Apple hosts (LLP 1094 D5–D9): a grip whose
//! list shares a `reorderGroup` lifts as a ghost Swift draws in its top
//! layer, and the runner's session hides the row itself. Swift picks the
//! target list under the ghost's centre and hands its content y here; the
//! drop fires on the target and may hold until its move shows. Every reply,
//! and every commit while the session lives, carries one `reorder` op with
//! `"group":true`, the phase, its ending, the target list and the wrapper
//! that holds the row now (where a ghost lands).
use super::*;
use exact_kernel::CommitReceipt;
use exact_runner::{ReorderBinding, ReorderPhase, ReorderStep, ReorderToken};

pub(super) struct GroupArrange {
    pub(super) binding: ReorderBinding,
    pub(super) token: ReorderToken,
    /// The op this session last published, so a commit says only a change.
    published: Option<String>,
    /// Its drop fired.
    dropped: bool,
}

impl<D: DataSource> Host<D> {
    /// Lift a grouped grip (D6): with `ghost` Swift draws the row and the
    /// runner hides it until `reorder_group_finish`; a key's session (D9)
    /// has none. Refused while any reorder holds the owner, a hold or a
    /// return included (D8).
    pub fn reorder_group_begin(
        &mut self,
        handle: ViewId,
        scroll_top: f64,
        ghost: bool,
        now_ms: f64,
    ) -> String {
        if !now_ms.is_finite() || now_ms < 0. || !scroll_top.is_finite() {
            return self.hold_refusal("invalid reorder sample");
        }
        if self.group.is_some() || self.arrange.is_some() {
            return self.group_refused(None);
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
            return self.group_refused(None);
        };
        self.now_ms = now_ms.max(self.now_ms);
        match self.runner.begin_group_reorder(binding, geometry, ghost) {
            Ok(Some(start)) => {
                self.group = Some(GroupArrange {
                    binding,
                    token: start.token,
                    published: None,
                    dropped: false,
                });
                self.group_reply(start.receipt.into_iter().collect(), None)
            }
            Ok(None) => self.group_refused(None),
            Err(e) => self.group_refused(Some(format!("{e:?}"))),
        }
    }

    /// The ghost's centre is in `target`'s port at `content_y` (D5): the gap
    /// moves there. Outside every port Swift sends `inside` false and the
    /// certified gap stands; a scroll the runner has not accepted is stale.
    pub fn reorder_move_into(
        &mut self,
        serial: u64,
        target: ViewId,
        content_y: f64,
        target_scroll_top: f64,
        inside: bool,
        now_ms: f64,
    ) -> String {
        let Some(token) = self.group_token(serial) else {
            return self.group_reply(Vec::new(), None);
        };
        if ![content_y, target_scroll_top, now_ms]
            .into_iter()
            .all(f64::is_finite)
        {
            return self.hold_refusal("invalid reorder sample");
        }
        self.now_ms = now_ms.max(self.now_ms);
        let target = self.runner.kernel().node(target).map(|n| n.key);
        let geometry = target.and_then(|t| self.runner.reorder_geometry(t));
        let (Some(target), Some(geometry), true) = (target, geometry, inside) else {
            return self.group_reply(Vec::new(), None);
        };
        if geometry.scroll_top != target_scroll_top {
            return self.group_reply(Vec::new(), None);
        }
        match self
            .runner
            .preview_reorder_into(token, target, geometry, content_y)
        {
            Ok(exact_runner::ReorderProgress::Accepted { receipt }) => {
                self.group_reply(receipt.into_iter().collect(), None)
            }
            Ok(_) => self.group_reply(Vec::new(), None),
            Err(e) => self.group_reply(Vec::new(), Some(format!("{e:?}"))),
        }
    }

    /// A key's or custom action's step (D9): 1 earlier, 2 later, 3 the
    /// previous list, 4 the next.
    pub fn reorder_group_step(&mut self, serial: u64, step: u32, now_ms: f64) -> String {
        let step = match step {
            1 => ReorderStep::Earlier,
            2 => ReorderStep::Later,
            3 => ReorderStep::PreviousList,
            4 => ReorderStep::NextList,
            _ => return self.hold_refusal("invalid reorder step"),
        };
        let Some(token) = self.group_token(serial) else {
            return self.group_reply(Vec::new(), None);
        };
        self.now_ms = now_ms.max(self.now_ms);
        match self.runner.reorder_step(token, step) {
            Ok(receipts) => self.group_reply(receipts, None),
            Err(e) => self.group_reply(Vec::new(), Some(format!("{e:?}"))),
        }
    }

    /// The contact ended (D8): `drop` into the session's target, which may
    /// hold; otherwise a cancel, which a hold ignores (its send is out).
    pub fn reorder_group_end(&mut self, serial: u64, drop: bool, now_ms: f64) -> String {
        let Some(token) = self
            .group
            .as_ref()
            .filter(|g| g.token.serial() == serial)
            .map(|g| g.token)
        else {
            return self.group_reply(Vec::new(), None);
        };
        self.now_ms = now_ms.max(self.now_ms);
        let mut receipts = Vec::new();
        let mut error = None;
        let mut dropped = false;
        let target = self.runner.reorder_frame(token).and_then(|f| f.target);
        if let Some(g) = drop
            .then(|| target.and_then(|t| self.runner.reorder_geometry(t)))
            .flatten()
        {
            match self.runner.drop_reorder(token, g) {
                Ok(Some(receipt)) => {
                    dropped = true;
                    receipts.push(receipt);
                    if let Some(g) = self.group.as_mut() {
                        g.dropped = true;
                    }
                }
                Ok(None) => {}
                Err(e) => error = Some(format!("{e:?}")),
            }
        }
        if !dropped {
            match self.runner.cancel_reorder(token) {
                Ok(receipt) => receipts.extend(receipt),
                Err(e) => error = error.or(Some(format!("{e:?}"))),
            }
        }
        self.group_reply(receipts, error)
    }

    /// The ghost has landed or faded: the session ends, its pin retires and
    /// the row shows (D6). Refused while the drop holds.
    pub fn reorder_group_finish(&mut self, serial: u64, now_ms: f64) -> String {
        let Some(token) = self
            .group
            .as_ref()
            .filter(|g| g.token.serial() == serial)
            .map(|g| g.token)
        else {
            return self.group_reply(Vec::new(), None);
        };
        self.now_ms = now_ms.max(self.now_ms);
        // Once the runner has no frame for it, the reply says `finished`
        // and the session is gone here too (`group_op`).
        match self.runner.finish_reorder(token) {
            Ok(receipt) => self.group_reply(receipt.into_iter().collect(), None),
            Err(e) => self.group_reply(Vec::new(), Some(format!("{e:?}"))),
        }
    }

    /// An active session's token for `serial`.
    fn group_token(&self, serial: u64) -> Option<ReorderToken> {
        let g = self.group.as_ref().filter(|g| g.token.serial() == serial)?;
        let phase = self.runner.reorder_frame(g.token)?.phase;
        (phase == ReorderPhase::Active).then_some(g.token)
    }

    /// The session's op, from the runner's frame; `finished` once it is
    /// gone. A session the runner retired (its list went) ends here.
    fn group_op(&mut self) -> Option<String> {
        let g = self.group.as_ref()?;
        let kernel = self.runner.kernel();
        let id = |key: Option<NodeKey>| key.and_then(|k| kernel.node_by_key(k)).map_or(0, |n| n.id);
        let (list, wrapper) = (id(Some(g.binding.list)), id(Some(g.binding.wrapper)));
        let (serial, dropped) = (g.token.serial(), g.dropped);
        let Some(frame) = self.runner.reorder_frame(g.token) else {
            self.group = None;
            return Some(format!("{{\"op\":\"reorder\",\"group\":true,\"token\":\"{serial}\",\"list\":{list},\"wrapper\":{wrapper},\"phase\":\"finished\",\"dispatched\":{dropped},\"ending\":null,\"target\":0,\"row\":0}}"));
        };
        let ending = frame
            .ending
            .map_or("null".to_owned(), |e| format!("\"{}\"", e.name()));
        Some(format!(
            "{{\"op\":\"reorder\",\"group\":true,\"token\":\"{serial}\",\"list\":{list},\"wrapper\":{wrapper},\"phase\":\"{}\",\"dispatched\":{},\"ending\":{ending},\"target\":{},\"row\":{}}}",
            frame.phase.name(),
            dropped,
            id(frame.target),
            id(frame.row),
        ))
    }

    /// After any commit: a changed session is said (a hold that ended on an
    /// answer or its deadline, a list that went).
    pub(super) fn group_after_commit(&mut self, batch: &mut Batch) {
        let Some(op) = self.group_op() else {
            return;
        };
        if let Some(g) = self.group.as_mut() {
            if g.published.as_deref() == Some(op.as_str()) {
                return;
            }
            g.published = Some(op.clone());
        }
        batch.push_op(op);
    }

    fn group_reply(&mut self, receipts: Vec<CommitReceipt>, error: Option<String>) -> String {
        let at_ms = self.now_ms;
        let timed: Vec<Timed> = receipts
            .into_iter()
            .map(|receipt| Timed { at_ms, receipt })
            .collect();
        let (mut batch, e) = self.commit_tree(&timed, error, Batch::new());
        let arrange = self.arrange_after_commit(&mut batch);
        // Every reply says the session, changed or not.
        if let Some(op) = self.group_op() {
            if let Some(g) = self.group.as_mut() {
                g.published = Some(op.clone());
            }
            batch.push_op(op);
        }
        self.commit_finish(batch, e.or(arrange))
    }

    fn group_refused(&mut self, error: Option<String>) -> String {
        let mut batch = Batch::new();
        batch.push_op("{\"op\":\"reorder\",\"group\":true,\"token\":\"0\",\"list\":0,\"wrapper\":0,\"phase\":\"refused\",\"dispatched\":false,\"ending\":null,\"target\":0,\"row\":0}".into());
        self.finish(batch, error)
    }
}
