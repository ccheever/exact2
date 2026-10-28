//! Linux's first scroll completion step (LLP 1005, Scroll completion).
//! The Host keeps time; this queue is serviced only after its ordinary work.
use super::*;
use exact_kernel::NodeKey;
use exact_runner::ReorderToken;
use std::collections::VecDeque;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Completion {
    Instant,
    Reader,
    Edge(ReorderToken),
    Collection,
}

#[derive(Clone, Copy)]
struct Pending {
    key: NodeKey,
    due: f64,
    kind: Completion,
}

#[derive(Default)]
pub(super) struct State {
    pending: BTreeMap<ViewId, Pending>,
    // A shell sample may finish a scroll before the Host reaches its deadline.
    completed: VecDeque<(ViewId, NodeKey, f64)>,
    delivering: VecDeque<(ViewId, NodeKey)>,
    model: BTreeMap<ViewId, (collection::ModelScroll, Completion)>,
    reader_previous: BTreeMap<ViewId, Pending>,
    command_work: bool,
}

impl State {
    pub(super) fn forget(&mut self, id: ViewId) {
        self.pending.remove(&id);
        self.completed.retain(|(view, _, _)| *view != id);
        self.delivering.retain(|(view, _)| *view != id);
        self.model.remove(&id);
        self.reader_previous.remove(&id);
    }
}

impl<D: DataSource> Presenter<D> {
    pub(crate) fn tracks_scroll_end(&self, view: ViewId) -> bool {
        !cfg!(windows)
            && self
                .host
                .runner()
                .handlers_of(view)
                .contains(&EventKind::Scrollend)
    }

    fn scroll_end_live(&self, view: ViewId, key: NodeKey) -> bool {
        if self.host.kernel().node(view).is_none_or(|n| n.key != key)
            || !self.tracks_scroll_end(view)
            || self.host.route_visibility(view).0
        {
            return false;
        }
        let mut at = Some(view);
        while let Some(id) = at {
            let Some(node) = self.host.kernel().node(id) else {
                return false;
            };
            if node.style.display == exact_kernel::Display::None {
                return false;
            }
            if node.is_root {
                return true;
            }
            at = node.parent;
        }
        false
    }

    pub(super) fn scroll_end_input(
        &mut self,
        view: ViewId,
        before: (f32, f32),
        completion: Completion,
    ) {
        if self.scroll_of(view) == before {
            self.scroll_end_requested(view, before, completion);
        } else {
            self.scroll_end_moved(view, completion);
        }
    }

    /// Called only for an actual user/API move, before its scroll handler.
    pub(super) fn scroll_end_moved(&mut self, view: ViewId, mut completion: Completion) {
        if self.booting || !self.tracks_scroll_end(view) {
            return;
        }
        self.scroll_end.model.remove(&view);
        let Some(key) = self.host.kernel().node(view).map(|n| n.key) else {
            return;
        };
        if !self.scroll_end_live(view, key) {
            return;
        }
        self.scroll_end.reader_previous.remove(&view);
        if let Some(previous) = self.scroll_end.pending.get(&view).copied() {
            // Corrections made while an owned edge scroll is active belong
            // to that owner, including the sample which reaches its boundary.
            if completion == Completion::Collection
                && matches!(previous.kind, Completion::Edge(token) if self.scroll_edge_owned(token))
            {
                completion = previous.kind;
            }
            // Deferred wheel input has one scroll event at the final offset.
            let continuing = self.scroll_end_frame_blocked(view)
                || (completion == previous.kind
                    && matches!(completion, Completion::Edge(_) | Completion::Collection));
            if !continuing && self.scroll_end_finished(view, &previous) {
                self.scroll_end.completed.push_back((
                    view,
                    previous.key,
                    previous.due.min(self.host.now()),
                ));
            } else if !continuing
                && completion == Completion::Reader
                && previous.kind == Completion::Reader
            {
                self.scroll_end.reader_previous.insert(view, previous);
            }
        }
        let due = match completion {
            Completion::Instant => self.host.now(),
            Completion::Reader => self.host.now() + 100.,
            Completion::Edge(_) | Completion::Collection => f64::INFINITY,
        };
        self.scroll_end.pending.insert(
            view,
            Pending {
                key,
                due,
                kind: completion,
            },
        );
    }

    /// A model write may wait for its picture's acknowledgement. Only that
    /// acknowledgement moves the offset and starts completion's clock.
    pub(super) fn scroll_end_requested(
        &mut self,
        view: ViewId,
        before: (f32, f32),
        completion: Completion,
    ) {
        if self.booting || !self.tracks_scroll_end(view) {
            return;
        }
        if let Some(model) = self.pending_model_scroll(view) {
            self.scroll_end.model.insert(view, (model, completion));
        } else {
            self.scroll_end.model.remove(&view);
            if self.scroll_of(view) != before {
                self.scroll_end_moved(view, completion);
            }
        }
    }

    /// A pending position's bookkeeping changed (`advance_pending_base`): the copy kept to
    /// recognize its acknowledgement follows it, so its `scrollend` still comes.
    pub(super) fn scroll_end_model_moved(
        &mut self,
        view: ViewId,
        was: collection::ModelScroll,
        now: collection::ModelScroll,
    ) {
        if let Some((model, _)) = self.scroll_end.model.get_mut(&view) {
            if model.same_target(&was) {
                *model = now;
            }
        }
    }

    // Layout may clamp an unacknowledged request without starting another scroll.
    pub(super) fn scroll_end_retargeted(
        &mut self,
        view: ViewId,
        before: Option<collection::ModelScroll>,
    ) {
        let Some(current) = self.pending_model_scroll(view) else {
            self.scroll_end.model.remove(&view);
            return;
        };
        if let Some((model, _)) = self.scroll_end.model.get_mut(&view) {
            if before.is_some_and(|b| model.same_target(&b))
                && (model.key, model.sequence) == (current.key, current.sequence)
            {
                *model = current;
            }
            if !model.same_target(&current) {
                self.scroll_end.model.remove(&view);
            }
        }
    }

    fn retire_model_scroll_ends(&mut self) {
        // Cursor advances and sequence changes can invalidate an unpainted request.
        let mut models = std::mem::take(&mut self.scroll_end.model);
        models.retain(|view, (model, _)| {
            self.pending_model_scroll(*view)
                .is_some_and(|p| p.same_target(model))
        });
        self.scroll_end.model = models;
    }

    #[cfg(any(target_os = "linux", target_os = "android", test))]
    pub(super) fn scroll_end_acknowledged(
        &mut self,
        view: ViewId,
        accepted: collection::ModelScroll,
        moved: bool,
    ) {
        if let Some((_, completion)) = self
            .scroll_end
            .model
            .get(&view)
            .copied()
            .filter(|(model, _)| model.same_target(&accepted))
        {
            self.scroll_end.model.remove(&view);
            if moved {
                self.scroll_end_moved(view, completion);
            }
        }
    }

    /// The reader shell already stamps its accepted sample for prefetching.
    /// Keep that stamp without advancing the Host or sampling its clock again.
    #[cfg(any(target_os = "android", all(test, not(windows))))]
    pub(crate) fn scroll_end_reader_sampled(&mut self, view: ViewId, at: f64) {
        if let Some(previous) = self.scroll_end.reader_previous.remove(&view) {
            if previous.due <= at {
                self.scroll_end
                    .completed
                    .push_back((view, previous.key, previous.due));
            }
        }
        if let Some(pending) = self
            .scroll_end
            .pending
            .get_mut(&view)
            .filter(|p| p.kind == Completion::Reader)
        {
            pending.due = at + 100.;
        }
    }

    pub(super) fn retire_scroll_ends(&mut self) {
        let gone: Vec<_> = self
            .scroll_end
            .pending
            .iter()
            .map(|(id, p)| (*id, p.key))
            .chain(
                self.scroll_end
                    .completed
                    .iter()
                    .map(|(id, key, _)| (*id, *key)),
            )
            .chain(self.scroll_end.delivering.iter().copied())
            .chain(
                self.scroll_end
                    .model
                    .iter()
                    .map(|(id, (model, _))| (*id, model.key)),
            )
            .filter_map(|(id, key)| (!self.scroll_end_live(id, key)).then_some(id))
            .collect();
        for id in gone {
            self.scroll_end.forget(id);
        }
        self.retire_model_scroll_ends();
    }

    pub(crate) fn timer_or_scroll_due_ms(&self) -> Option<f64> {
        self.host
            .timer_due_ms()
            .into_iter()
            .chain(self.scroll_end_due())
            .reduce(f64::min)
    }

    pub(crate) fn scroll_end_due(&self) -> Option<f64> {
        self.scroll_end
            .pending
            .iter()
            .filter(|(id, _)| !self.scroll_end_frame_blocked(**id))
            .filter_map(|(id, p)| {
                if matches!(p.kind, Completion::Instant | Completion::Reader) {
                    Some(p.due)
                } else if self.scroll_end_finished(*id, p) {
                    Some(self.host.now())
                } else {
                    None
                }
            })
            .chain(
                self.scroll_end
                    .completed
                    .iter()
                    .filter(|(view, _, _)| !self.scroll_end_frame_blocked(*view))
                    .map(|(_, _, due)| *due),
            )
            .reduce(f64::min)
    }

    fn scroll_end_finished(&self, view: ViewId, pending: &Pending) -> bool {
        match pending.kind {
            Completion::Edge(token) => !self.scroll_edge_active(view, token),
            Completion::Collection => false,
            _ => pending.due <= self.host.now(),
        }
    }

    pub(super) fn finish_collection_scroll_ends(&mut self) {
        let finished: Vec<_> = self
            .scroll_end
            .pending
            .iter()
            .filter_map(|(view, p)| {
                (p.kind == Completion::Collection && !self.collection_scroll_pending(*view))
                    .then_some(*view)
            })
            .collect();
        for view in finished {
            let pending = self.scroll_end.pending.get_mut(&view).unwrap();
            pending.kind = Completion::Instant;
            pending.due = self.host.now();
        }
    }

    fn scroll_end_frame_blocked(&self, view: ViewId) -> bool {
        self.deferred_collections
            .as_ref()
            .is_some_and(|ids| ids.iter().any(|(id, _)| *id == view))
    }

    pub(super) fn deliver_scroll_ends(&mut self) -> Option<String> {
        if self.scroll_end.pending.is_empty() && self.scroll_end.completed.is_empty() {
            return None;
        }
        self.retire_scroll_ends();
        self.finish_collection_scroll_ends();
        // A motion-only tick has not caught runner timers up. Let the shell's
        // existing timer path service them before taking a completion snapshot.
        if self
            .host
            .runner()
            .timer_due_ms()
            .is_some_and(|due| due <= self.host.now())
        {
            return None;
        }
        let mut due: Vec<_> = self
            .scroll_end
            .pending
            .iter()
            .filter_map(|(id, p)| {
                (self.scroll_end_finished(*id, p) && !self.scroll_end_frame_blocked(*id))
                    .then_some((*id, *p))
            })
            .collect();
        due.sort_by(|a, b| a.1.due.total_cmp(&b.1.due).then(a.0.cmp(&b.0)));
        for (id, pending) in due {
            self.scroll_end.pending.remove(&id);
            self.scroll_end.reader_previous.remove(&id);
            self.scroll_end.completed.push_back((
                id,
                pending.key,
                pending.due.min(self.host.now()),
            ));
        }
        for _ in 0..self.scroll_end.completed.len() {
            let end = self.scroll_end.completed.pop_front().unwrap();
            if self.scroll_end_frame_blocked(end.0) || end.2 > self.host.now() {
                self.scroll_end.completed.push_back(end);
            } else {
                self.scroll_end.delivering.push_back((end.0, end.1));
            }
        }
        if self.scroll_end.delivering.is_empty() {
            return None;
        }
        // Take the due snapshot first: a scroll handler can owe a new end,
        // or retire any remaining target, including a recycled row.
        let mut error = self.dispatch_authored_scroll();
        while let Some(&(view, key)) = self.scroll_end.delivering.front() {
            if self.has_pending_scroll(view) {
                let scroll = self.dispatch_authored_scroll();
                error = error.or(scroll);
                // That scroll handler may have retired the snapshot's target.
                if self.scroll_end.delivering.front() != Some(&(view, key)) {
                    continue;
                }
            }
            self.scroll_end.delivering.pop_front();
            if !self.scroll_end_live(view, key) {
                continue;
            }
            let sent = self
                .host
                .dispatch_at(view, Event::Scrollend, self.host.now());
            let after = self.after_commit();
            self.scroll_end.command_work |= !self.commands.is_empty();
            error = error.or(sent).or(after);
        }
        error
    }
}

impl<D: DataSource + Default> Presenter<D> {
    pub(crate) fn settle_scroll_commands(&mut self, line: &str, mut reply: String) -> String {
        use exact_runner::agent::{field_bool, field_str};
        self.retire_model_scroll_ends();
        if (self.scroll_end.pending.is_empty()
            && self.scroll_end.completed.is_empty()
            && self.scroll_end.model.is_empty()
            && !self.scroll_end.command_work)
            || !field_bool(line, "settle")
            || field_str(line, "op").as_deref() != Some("clock")
        {
            return reply;
        }
        // `handle` ran the first command snapshot. Commands caused by an end
        // can themselves issue commands before another scroll exists.
        for _ in 0..16 {
            if !field_bool(&reply, "settled") {
                return reply;
            }
            if self.scroll_end_due().is_none() && !self.scroll_end.command_work {
                break;
            }
            let command_work = std::mem::take(&mut self.scroll_end.command_work);
            reply = crate::agent::answer(self, line);
            // Recompute settlement after the last command too: it can start
            // motion or a request without leaving another command queued.
            self.scroll_end.command_work |= command_work && !self.commands.is_empty();
            self.run_commands(D::default);
        }
        // A frame must acknowledge model writes and release deferred scrolls.
        self.retire_model_scroll_ends();
        if (!self.scroll_end.pending.is_empty()
            || !self.scroll_end.completed.is_empty()
            || !self.scroll_end.model.is_empty()
            || self.scroll_end.command_work)
            && field_bool(&reply, "settled")
        {
            format!(
                "{{\"clock\":{},\"settled\":false,\"reason\":\"scrollend\"}}",
                num(self.host.now())
            )
        } else {
            reply
        }
    }
}

#[cfg(all(test, windows))]
#[test]
fn windows_keeps_scroll_completion_disabled() {
    struct NoData;
    impl DataSource for NoData {
        fn query(
            &mut self,
            name: &str,
            _: &[exact_runner::Value],
        ) -> Result<exact_runner::Value, exact_runner::DataError> {
            Err(exact_runner::DataError::UnknownSource(name.into()))
        }
    }
    let plan = contract::compile(
        r#"component App
  state ends = 0
  action ended
    ends = ends + 1
  view
    scroll testId="port" width=200 height=100 scrollend=ended
      box height=1000
"#,
    )
    .unwrap()
    .encode();
    let (mut p, error) = Presenter::boot_with(
        &plan,
        NoData,
        (400., 500.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert_eq!(error, None);
    let key = p.host.kernel().find_by_test_id("port")[0];
    let view = p.host.kernel().node_by_key(key).unwrap().id;
    assert!(!p.tracks_scroll_end(view));
    p.wheel(view, 0., 20.).unwrap();
    p.advance(200.);
    assert_eq!(p.scroll_end_due(), None);
    assert_eq!(
        p.host.runner().slot("ends"),
        Some(&exact_runner::Value::Number(0.))
    );
}
