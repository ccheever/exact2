//! File-upload sizing, admission and lifetime in the async executor.
//! @ref LLP 1120#d5-byte-budgets-and-counts-the-caps-stated-as-admit-and-next_job-compute-them — one whole reservation before reading.
use super::*;

#[cfg(test)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in super::super) enum ReaderPhase {
    Spawn,
    Size,
    Read,
    Finished(bool),
}
#[cfg(test)]
pub(in super::super) type ReaderHook =
    Arc<dyn Fn(ReaderPhase, Run) -> Result<(), Outcome> + Send + Sync>;

impl Machine {
    #[cfg(test)]
    pub(in super::super) fn queued_uploads(&self) -> (usize, usize) {
        let state = self.shared.state.lock().unwrap();
        (
            state.book.files.len(),
            state.book.hops.iter().map(VecDeque::len).sum(),
        )
    }
    #[cfg(test)]
    fn reader_phase(&self, phase: ReaderPhase, run: Run) -> Result<(), Outcome> {
        let hook = self.reader_hook.lock().unwrap().clone();
        hook.map_or(Ok(()), |hook| hook(phase, run))
    }

    /// An `exactBodyFrom` job (D5): its deadline armed, a reader sizes the
    /// file for the finisher to decide, then, its whole charge taken, reads
    /// at most the charged size; a parked one, sized before, is read at once.
    pub(super) fn file(self: &Arc<Self>, lane: usize, run: Run, file: FileJob, sized: bool) {
        let request = &file.request;
        if let Some(why) = request.body_from_refusal() {
            return self.answer(lane, run, body_refused(why.to_string()));
        }
        if let Some(ms) = request.timeout_ms {
            self.timer.arm(ms, lane, run, file.abort.clone());
        }
        let path = request.body_from.clone().expect("a file job");
        let (scope, app) = (request.grants.clone(), self.grants.clone());
        let probe = request.clone();
        let (decide, decision) = mpsc::channel();
        // The read stops between chunks once the request has ended.
        let cancel = Arc::new(AtomicBool::new(false));
        let told = cancel.clone();
        let stop = file
            .abort
            .signal()
            .register(move || told.store(true, Ordering::Release));
        // The reader owns this registration until it physically exits. The
        // disposer may destroy FileJob before Core's shared abort fires.
        let told = cancel.clone();
        let retiring = self.shared.abort.signal().register(move || {
            told.store(true, Ordering::Release);
        });
        let size = {
            let mut state = self.shared.state.lock().unwrap();
            // Registered only for a run still open on a live machine, in one
            // step with its reader's place in `live`.
            let open = !state.retired
                && find(&mut state.running, run)
                    .is_some_and(|r| !r.terminal && r.claim == Claim::None);
            if !open {
                return;
            }
            let copies = if state.book.portable[lane] { 2 } else { 1 };
            let size = sized.then(|| {
                state
                    .book
                    .uploads
                    .get(&run)
                    .map(|u| (u.bytes / copies) as u64)
            });
            if size.is_none() {
                state.book.deciders.insert(run, decide);
            }
            state.book.files.insert(run, file);
            find(&mut state.running, run).expect("open").live += 1;
            size.flatten()
        };
        let m = self.clone();
        let ended = Ended(m.clone(), lane, run);
        let job = move || {
            let (_stop, _retiring, _ended) = (stop, retiring, ended);
            let read = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                // The app's directories open here, on the reader, never on the
                // starter: a stalled disk holds a reader, not a lane.
                let roots = m.shared.roots_for(&probe);
                let size = match size {
                    Some(size) => Some(size),
                    None => {
                        #[cfg(test)]
                        m.reader_phase(ReaderPhase::Size, run)?;
                        let answer = body::size(roots, &app, scope.as_deref(), &path);
                        let ok = answer.as_ref().ok().copied();
                        // A failure claims the run when it happens, as any outcome.
                        if ok.is_some()
                            || claim(&mut m.shared.state.lock().unwrap(), run, Claim::Answered)
                        {
                            m.post(lane, Event::Sized(run, answer));
                        }
                        match (ok, decision.recv()) {
                            (Some(size), Ok(Decision::Read)) => Some(size),
                            _ => None,
                        }
                    }
                };
                if let Some(size) = size {
                    #[cfg(test)]
                    m.reader_phase(ReaderPhase::Read, run)?;
                    let read = body::read_at_most(
                        roots,
                        &app,
                        scope.as_deref(),
                        &path,
                        size,
                        true,
                        &cancel,
                    );
                    #[cfg(test)]
                    m.reader_phase(ReaderPhase::Finished(cancel.load(Ordering::Acquire)), run)?;
                    if read.is_ok()
                        || claim(&mut m.shared.state.lock().unwrap(), run, Claim::Answered)
                    {
                        m.post(lane, Event::Read(run, read));
                    }
                }
                Ok::<(), Outcome>(())
            }));
            match read {
                Err(_) => m.answer(
                    lane,
                    run,
                    failed(FailureKind::Aborted, "native reader panicked"),
                ),
                Ok(Err(outcome)) => m.answer(lane, run, outcome),
                Ok(Ok(())) => {}
            }
        };
        #[cfg(test)]
        let spawn = self
            .reader_phase(ReaderPhase::Spawn, run)
            .and_then(|_| body::reader(job));
        #[cfg(not(test))]
        let spawn = body::reader(job);
        if let Err(outcome) = spawn {
            self.shared.state.lock().unwrap().book.deciders.remove(&run);
            self.answer(lane, run, outcome);
        }
    }

    /// The finisher's decision for a sized upload (D5; §6 question 8).
    pub(super) fn sized(
        &self,
        state: &mut State,
        lane: usize,
        run: Run,
        size: Result<u64, Outcome>,
    ) -> Option<After> {
        if state.book.sizing[lane] == Some(run) {
            state.book.sizing[lane] = None;
        }
        let decider = state.book.deciders.remove(&run);
        let tell = |decision| {
            if let Some(decider) = &decider {
                let _ = decider.send(decision);
            }
        };
        if let Err(outcome) = size {
            // The reader claimed it.
            return Some(self.settled(state, run, outcome));
        }
        let open = find(&mut state.running, run)
            .filter(|r| !r.terminal && r.claim == Claim::None)
            .map(|r| (r.limit, r.charge));
        let Some((limit, charge)) = open else {
            tell(Decision::Exit);
            return None;
        };
        let size = size.expect("checked") as usize;
        let Some(upload) = state.book.upload_bytes(lane, size) else {
            tell(Decision::Exit);
            return Some(self.settle(
                state,
                run,
                failed(FailureKind::Refused, "upload size overflow"),
            ));
        };
        let behind: usize = state.jobs[lane]
            .iter()
            .filter(|j| j.ticket > run.0)
            .map(|j| j.charge)
            .sum();
        let refusal = if limit.saturating_add(upload) > BYTES[lane] {
            Some("the file and response ceiling exceed the lane's byte budget")
        } else if limit.saturating_add(upload).saturating_add(behind) > BYTES[lane] {
            Some("the file and the requests queued behind it exceed the lane's byte budget")
        } else {
            None
        };
        if let Some(why) = refusal {
            tell(Decision::Exit);
            return Some(self.settle(state, run, failed(FailureKind::Refused, why)));
        }
        let need = limit - charge + upload;
        if need <= BYTES[lane].saturating_sub(state.bytes[lane]) {
            let r = find(&mut state.running, run).expect("found");
            r.charge = limit;
            state.bytes[lane] += need;
            state.book.uploads.insert(run, Charged::new(lane, upload));
            tell(Decision::Read);
            return None;
        }
        // Back to the head of its lane, disarmed, its size known, holding
        // its waiting charge only: in this one step, so nothing passes it.
        tell(Decision::Exit);
        let at = state
            .running
            .iter()
            .position(|r| (r.ticket, r.generation) == run)
            .expect("found");
        let r = state.running.swap_remove(at);
        if r.blocking {
            state.book.blocking = false;
        }
        let FileJob {
            request,
            forced,
            abort,
            retiring,
            transport,
            fetch,
            ..
        } = state.book.files.remove(&run).expect("a file job");
        state
            .book
            .trash
            .push(Box::new((abort, retiring, transport, fetch)));
        state.book.sized.insert(run.0, size as u64);
        // In admission order: behind any job still queued ahead of it.
        let at = state.jobs[lane]
            .iter()
            .position(|j| j.ticket > run.0)
            .unwrap_or(state.jobs[lane].len());
        state.jobs[lane].insert(
            at,
            Job {
                ticket: run.0,
                request,
                forced,
                work: None,
                charge: r.charge,
                limit: r.limit,
                forgotten: r.forgotten,
            },
        );
        Some(Box::new(move |m: &Arc<Machine>| m.timer.remove(run)))
    }

    /// A file read for `run`: its upload made and its first exchange queued
    /// for the starter, or its failure settled.
    pub(super) fn read(
        self: &Arc<Self>,
        state: &mut State,
        lane: usize,
        run: Run,
        read: Result<Vec<u8>, Option<Outcome>>,
    ) -> Option<After> {
        let bytes = match read {
            Ok(bytes) => bytes,
            Err(e) => {
                // The reader claimed it.
                let path = state
                    .book
                    .files
                    .get(&run)
                    .and_then(|f| f.request.body_from.clone());
                let outcome = e.unwrap_or_else(|| {
                    body_refused(format!(
                        "exactBodyFrom {}: the file changed while it was read",
                        path.unwrap_or_default()
                    ))
                });
                return Some(self.settled(state, run, outcome));
            }
        };
        let open =
            find(&mut state.running, run).is_some_and(|r| !r.terminal && r.claim == Claim::None);
        let file = state.book.files.remove(&run);
        let releaser = self.clone();
        let upload = Upload::new(
            bytes,
            Some(Box::new(move || releaser.post(lane, Event::Released(run)))),
        );
        if let Some(u) = state.book.uploads.get_mut(&run) {
            u.made = true;
            u.buffer = true;
        }
        let file = match (open, file) {
            (true, Some(file)) => file,
            (_, file) => {
                state.book.trash.push(Box::new((upload, file)));
                return None;
            }
        };
        let url = file.request.url.clone();
        // A queued or discarded hop owns the same charged buffer as the
        // eventual transport. Only its last release refunds the upload.
        let start: Start = Box::new(move |m: &Arc<Machine>| {
            m.fetch(
                lane,
                run,
                file.request,
                Some(upload),
                file.forced,
                file.abort,
                file.retiring,
                file.limit,
                file.transport,
                file.fetch,
                None,
            )
        });
        let origin = Some(origin(&url));
        state.book.hops[lane].push_back(Hop { run, origin, start });
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::core::async_tests::{core, Script};

    #[test]
    fn one_sizing_decision_keeps_every_successor_in_the_park_refusal() {
        let script = Arc::new(Script::default());
        let (core, _) = core(
            &script,
            "net.fetch https://*.example.test\nfs.read app:/tmp",
        );
        let machine = core.machine.as_ref().unwrap();
        let mut state = core.shared.state.lock().unwrap();
        for ticket in 1..=2 {
            let mut request = Request::get("https://a.example.test/")
                .body_from("app:/tmp/file")
                .independent_http(1 << 20);
            request.method = "POST".into();
            if ticket == 2 {
                request
                    .headers
                    .push(("x-waiting".into(), "x".repeat(3 << 20)));
            }
            let (lane, charge, limit) = super::super::super::reservation(&request).unwrap();
            state.jobs[lane].push_back(Job {
                ticket,
                request,
                forced: false,
                work: None,
                charge,
                limit,
                forgotten: false,
            });
            state.bytes[lane] += charge;
            state.counts[lane] += 1;
        }
        let starts = machine.select(&mut state, 1);
        assert_eq!(
            starts.len(),
            1,
            "the second file must not leave the queue to size"
        );
        assert_eq!(state.jobs[1].front().unwrap().ticket, 2);
        let run = starts[0].0;
        let after = machine.sized(&mut state, 1, run, Ok(61 << 20));
        assert!(after.is_some());
        assert!(matches!(&state.completed[1][0].outcome,
            Outcome::Failed { kind: FailureKind::Refused, message }
                if message.contains("queued behind it")));
        drop(state);
        drop(starts);
    }

    struct DropProbe(Arc<Shared>, mpsc::Sender<bool>);
    impl Drop for DropProbe {
        fn drop(&mut self) {
            let _ = self.1.send(self.0.state.try_lock().is_ok());
        }
    }

    #[test]
    fn a_stale_hop_and_duplicate_outcome_are_disposed_outside_the_state_lock() {
        let script = Arc::new(Script::default());
        let (core, _) = core(&script, "net.fetch https://*.example.test");
        let machine = core.machine.as_ref().unwrap();
        let (dropped, receipt) = mpsc::channel();
        let probe = DropProbe(core.shared.clone(), dropped);
        let mut state = core.shared.state.lock().unwrap();
        machine.apply(
            &mut state,
            1,
            Event::Hop((77, 1), Box::new(move || drop(probe))),
        );
        machine.apply(
            &mut state,
            1,
            Event::Outcome(
                (77, 1),
                Done::Now(Outcome::Response(Response {
                    status: 200,
                    headers: vec![],
                    body: vec![0; 1 << 20],
                })),
            ),
        );
        assert!(
            receipt.try_recv().is_err(),
            "opaque capture was destroyed under the lock"
        );
        assert_eq!(state.book.trash.len(), 2);
        let trash = std::mem::take(&mut state.book.trash);
        drop(state);
        drop(trash);
        assert!(receipt.recv().unwrap());
    }
}
