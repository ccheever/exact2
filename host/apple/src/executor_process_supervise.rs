//! A spawned child's supervision (LLP 1133.007 D2, D4): one incarnation
//! after another while the resource holds it. Each incarnation's events go
//! to the stream in order — `spawn`, output chunks, `exit`, then `close`
//! once both pipes reach end of file or the drain timeout passes — with
//! `drain`, `ready` and `ready-timeout` among them. Its staged writes, ends
//! and kills arrive here addressed to it. Release signals the group and
//! escalates to `SIGKILL` after the grace, whether or not the leader still
//! lives. Every deadline is on the supervision clock, so under the agent
//! the drive's clock moves them.
use super::clock::{Clock, Leave};
use super::{deliver, event, ChildCtl, Input, Ran, Spec, CHUNK};
use super::{failed, Shared};
use exact_runner::{FailureKind, Message, Outcome, ProcessOp};
use ibex2::stdlib::abort::AbortController;
use serde_json::json;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::Arc;

/// What a supervisor sees of its child: output, its exit, a pipe at end of
/// file (`None` for both at once), or its stdin queue drained below the
/// high-water mark.
#[derive(Debug, PartialEq)]
pub(crate) enum Seen {
    Out(bool, Vec<u8>),
    Exit(Option<i64>, Option<String>),
    Eof(Option<bool>),
    #[allow(dead_code)] // the real transport's writer (stage 1)
    Drained,
}

/// One incarnation, scripted or real.
pub(crate) trait Child: Send {
    /// Its pid (a script's own number).
    fn pid(&self) -> u64;
    /// When its own next step is due, after its start: a script's.
    fn due(&self) -> Option<f64> {
        None
    }
    /// What it does by `elapsed` milliseconds after its start: a script's.
    fn run(&mut self, _elapsed: f64) -> Vec<Seen> {
        Vec::new()
    }
    /// Bytes for its stdin, in order.
    fn write(&mut self, bytes: Vec<u8>);
    /// Close its stdin once what is queued is written.
    fn end(&mut self);
    /// One signal to its process group.
    fn signal(&mut self, signal: &str) -> Vec<Seen>;
    /// Bytes waiting in its stdin.
    fn queued(&self) -> u64;
    /// Whether any member of its group lives.
    fn group_alive(&self) -> bool;
    /// Stop reading its pipes: the drain timeout passed.
    fn close_pipes(&mut self) {}
    /// A chunk it sent was delivered: a reader may read on.
    fn taken(&self) {}
    /// A readiness probe's answer when it gives it itself (a script's).
    fn ready(&self, _elapsed: f64) -> Option<bool> {
        None
    }
}

/// A script's incarnation as a child.
struct ScriptedChild(super::script::Scripted);

impl Child for ScriptedChild {
    fn pid(&self) -> u64 {
        self.0.pid
    }
    fn due(&self) -> Option<f64> {
        self.0.due()
    }
    fn run(&mut self, elapsed: f64) -> Vec<Seen> {
        (self.0.run(elapsed).into_iter()).map(seen).collect()
    }
    fn write(&mut self, bytes: Vec<u8>) {
        self.0.write(bytes.len() as u64);
    }
    fn end(&mut self) {
        self.0.end();
    }
    fn signal(&mut self, signal: &str) -> Vec<Seen> {
        (self.0.signal(signal).into_iter()).map(seen).collect()
    }
    fn queued(&self) -> u64 {
        self.0.stdin
    }
    fn group_alive(&self) -> bool {
        !self.0.exited()
    }
    fn ready(&self, elapsed: f64) -> Option<bool> {
        Some(self.0.ready_after().is_some_and(|at| elapsed >= at))
    }
}

fn seen(step: super::script::Step) -> Seen {
    match step {
        super::script::Step::Out(stderr, bytes) => Seen::Out(stderr, bytes),
        super::script::Step::Exit(code, signal) => Seen::Exit(code, signal),
        super::script::Step::Close => Seen::Eof(None),
    }
}

/// UTF-8 across chunk boundaries (`encoding: "utf8"`): a sequence split
/// between reads waits for its rest; what is left at the end is flushed as
/// U+FFFD, as Node's decoder ends.
#[derive(Default)]
struct Utf8(Vec<u8>);

impl Utf8 {
    fn decode(&mut self, bytes: &[u8]) -> String {
        self.0.extend_from_slice(bytes);
        let valid = match std::str::from_utf8(&self.0) {
            Ok(_) => self.0.len(),
            Err(e) if e.error_len().is_none() => e.valid_up_to(),
            Err(_) => self.0.len(),
        };
        let text = String::from_utf8_lossy(&self.0[..valid]).into_owned();
        self.0.drain(..valid);
        text
    }

    fn flush(&mut self) -> String {
        let text = String::from_utf8_lossy(&self.0).into_owned();
        self.0.clear();
        text
    }
}

/// How an incarnation ended.
struct Ended {
    code: Option<i64>,
    signal: Option<String>,
    drained: bool,
    /// The signals the app sent it with `kill`.
    sent: Vec<String>,
}

/// The shortest restart delay: systemd's `RestartSec=` default. With the
/// doubling of each consecutive restart it bounds a crash loop's rate.
const RESTART_FLOOR_MS: f64 = 100.0;
/// An incarnation that runs this long resets the doubling, as one that
/// becomes ready does: systemd's `StartLimitIntervalSec=` default.
const STEADY_MS: f64 = 10_000.0;

/// One spawn's supervision.
pub(crate) struct Supervisor<'a> {
    shared: &'a Arc<Shared>,
    ticket: u64,
    spec: Spec,
    ran: Ran,
    ctl: Arc<ChildCtl>,
    rx: Receiver<Input>,
    clock: Arc<Clock>,
    waiter: u64,
    /// Its place on the clock, given up once the stream's end is queued.
    leave: Option<Leave>,
    /// For readiness probes against a real child (D4).
    probe: Option<ibex2::host::Bindings>,
    released: bool,
    /// `exactReady`'s round passed with `onTimeout: "fail"` (D4).
    timed_out: bool,
}

impl<'a> Supervisor<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        shared: &'a Arc<Shared>,
        ticket: u64,
        spec: Spec,
        ran: Ran,
        ctl: Arc<ChildCtl>,
        rx: Receiver<Input>,
        host: ibex2::host::Host,
        set: &ibex2::grant::GrantSet,
    ) -> Self {
        let clock = shared.processes.clock.clone();
        let tick = ctl.clone();
        let waiter = clock.join(Box::new(move || tick.send(Input::Tick)));
        let probe = spec.ready.as_ref().map(|_| host.endow(set.clone()));
        let leave = Some(Leave(clock.clone(), waiter));
        Supervisor {
            shared,
            ticket,
            spec,
            ran,
            ctl,
            rx,
            clock,
            waiter,
            leave,
            probe,
            released: false,
            timed_out: false,
        }
    }

    /// Run to the stream's end: its last outcome, and the place on the
    /// clock to give up once that is queued.
    pub(crate) fn run(mut self) -> (Outcome, Option<Leave>) {
        let outcome = self.incarnations();
        (outcome, self.leave.take())
    }

    fn start(&self, n: u32) -> Result<Box<dyn Child>, Outcome> {
        match &self.ran {
            Ran::Script(script) => Ok(Box::new(ScriptedChild(script.incarnation(n)))),
            Ran::Real | Ran::File(_) => super::real::spawn(
                self.shared,
                self.ticket,
                n,
                &self.spec,
                &self.ran,
                &self.ctl,
            ),
        }
    }

    fn incarnations(&mut self) -> Outcome {
        let (mut n, mut consecutive, mut restarts) = (0u32, 0u32, 0u32);
        loop {
            n += 1;
            let child = match self.start(n) {
                Ok(child) => child,
                Err(outcome) => return outcome,
            };
            self.ctl.begin(n, self.spec.pipes[0]);
            let spawned = json!({ "pid": child.pid(), "restarts": restarts, "incarnation": n, "ran": self.ran.words() });
            // Let go before its first event landed: the incarnation starts
            // by releasing its group, which then runs to its close.
            let gone = !deliver(self.shared, self.ticket, event("spawn", spawned), false);
            let began = self.clock.now();
            let (ended, ready) = self.incarnation(n, child, gone);
            self.ctl.end();
            // A child that became ready, or ran long enough, failed alone:
            // its restart waits only the delay again.
            if ready || self.clock.now() - began >= STEADY_MS {
                consecutive = 0;
            }
            let close = json!({ "code": ended.code, "signal": ended.signal, "drained": ended.drained, "incarnation": n });
            if self.timed_out {
                return failed(
                    FailureKind::Timeout,
                    "exactReady: not ready within its timeout",
                );
            }
            if self.released {
                return failed(FailureKind::Aborted, "the stream was let go");
            }
            let restart = self.spec.restart.as_ref().filter(|r| {
                let prevented = ended.code.is_some_and(|c| r.prevent.contains(&c));
                // A signal the app sent with `kill` is no failure (D4).
                let theirs = (ended.signal.as_ref()).is_some_and(|s| !ended.sent.contains(s));
                let failed = ended.code.is_some_and(|c| c != 0) || theirs;
                !prevented && (r.always || failed)
            });
            let Some(restart) = restart else {
                return Outcome::Closed {
                    code: 1000,
                    reason: close.to_string(),
                    was_clean: ended.drained,
                };
            };
            let base = restart.delay.max(RESTART_FLOOR_MS);
            let delay =
                (base * 2f64.powi(consecutive.min(30) as i32)).min(restart.max_delay.max(base));
            if !deliver(self.shared, self.ticket, event("close", close), false) {
                return failed(FailureKind::Aborted, "the stream was let go");
            }
            consecutive += 1;
            restarts += 1;
            if !self.sleep(self.clock.now() + delay) {
                return failed(FailureKind::Aborted, "the stream was let go");
            }
        }
    }

    /// Wait until `until` on the clock, refusing operations (no incarnation
    /// runs); false if the child was let go meanwhile.
    fn sleep(&mut self, until: f64) -> bool {
        loop {
            if self.released {
                return false;
            }
            if self.clock.passed(Some(until)) {
                return true;
            }
            self.clock.wait_for(self.waiter, Some(until));
            match self.rx.recv_timeout(self.clock.timeout(Some(until))) {
                Ok(Input::Release) | Err(RecvTimeoutError::Disconnected) => self.released = true,
                _ => {}
            }
        }
    }

    /// One incarnation, to its close: how it ended, and whether it became ready.
    fn incarnation(&mut self, n: u32, mut child: Box<dyn Child>, gone: bool) -> (Ended, bool) {
        let started = self.clock.now();
        let utf8 = self.spec.utf8;
        let (mut out, mut err) = (Utf8::default(), Utf8::default());
        let mut exit: Option<(Option<i64>, Option<String>, f64)> = None;
        let mut eof = [!self.spec.pipes[1], !self.spec.pipes[2]];
        let mut drained = true;
        // A write that answered `false` owes a `drain` (D2).
        let mut owed = false;
        // Readiness: the next probe, and when the round times out (D4).
        let mut ready = false;
        let mut probe_at = self.spec.ready.as_ref().map(|_| started);
        let mut round_end = self.spec.ready.as_ref().map(|r| started + r.timeout);
        let mut probing = false;
        // Release, or a group still alive at close: when `SIGKILL` follows.
        let mut escalate: Option<f64> = None;
        let mut fail_ready = false;
        let mut pending: Vec<Seen> = Vec::new();
        let mut sent: Vec<String> = Vec::new();
        // What the child's own stdin queue held at the last look. The
        // handle's count (`ctl.queued`) also holds what was sent and not
        // yet taken here, so it falls only by what the child consumed.
        let mut held = 0u64;
        if gone {
            self.release(&mut child, &mut escalate, &mut pending);
        }
        loop {
            let now = self.clock.now();
            if !self.released && child.due().is_some() {
                pending.extend(child.run(now - started));
            }
            // What a release made of the child as these were taken.
            let mut released = Vec::new();
            for seen in pending.drain(..) {
                match seen {
                    Seen::Out(stderr, bytes) => {
                        child.taken();
                        if (if stderr { eof[1] } else { eof[0] }) || self.released {
                            continue;
                        }
                        let name = if stderr { "stderr" } else { "stdout" };
                        for chunk in bytes.chunks(CHUNK) {
                            let message = if utf8 {
                                let decoder = if stderr { &mut err } else { &mut out };
                                let data = decoder.decode(chunk);
                                if data.is_empty() {
                                    continue;
                                }
                                Message {
                                    event: name.into(),
                                    data,
                                    ..Message::default()
                                }
                            } else {
                                Message {
                                    event: name.into(),
                                    binary: Some(chunk.to_vec()),
                                    ..Message::default()
                                }
                            };
                            if !deliver(self.shared, self.ticket, message, true) {
                                self.release(&mut child, &mut escalate, &mut released);
                            }
                        }
                    }
                    Seen::Exit(code, signal) => {
                        if exit.is_none() {
                            exit = Some((code, signal.clone(), now));
                            let exited =
                                json!({ "code": code, "signal": signal, "incarnation": n });
                            if !self.released
                                && !deliver(self.shared, self.ticket, event("exit", exited), false)
                            {
                                self.release(&mut child, &mut escalate, &mut released);
                            }
                        }
                    }
                    Seen::Eof(None) => eof = [true, true],
                    Seen::Eof(Some(stderr)) => eof[usize::from(stderr)] = true,
                    Seen::Drained => {}
                }
            }
            pending.extend(released);
            // What the child took of its stdin; once nothing is queued, a
            // write that answered `false` hears `drain`, as Node's does (D2).
            let queued = child.queued();
            self.ctl.consumed(held.saturating_sub(queued));
            held = queued;
            if owed && self.ctl.queued.load(Ordering::Acquire) == 0 {
                owed = false;
                if !self.released {
                    deliver(
                        self.shared,
                        self.ticket,
                        event("drain", json!({ "incarnation": n })),
                        false,
                    );
                }
            }
            // Readiness (D4): a script answers its own probe; a real child's is a GET.
            if !ready && !probing && !self.released && self.clock.passed(probe_at) {
                match child.ready(now - started) {
                    Some(true) => {
                        ready = true;
                        deliver(
                            self.shared,
                            self.ticket,
                            event("ready", json!({ "incarnation": n })),
                            false,
                        );
                    }
                    Some(false) => probe_at = self.spec.ready.as_ref().map(|r| now + r.interval),
                    None => {
                        probing = true;
                        self.probe(n);
                    }
                }
            }
            if !ready && !self.released && self.clock.passed(round_end) {
                deliver(
                    self.shared,
                    self.ticket,
                    event("ready-timeout", json!({ "incarnation": n })),
                    false,
                );
                if self.spec.ready.as_ref().is_some_and(|r| r.fail) {
                    fail_ready = true;
                    self.release(&mut child, &mut escalate, &mut pending);
                } else {
                    round_end = self.spec.ready.as_ref().map(|r| now + r.timeout);
                }
            }
            // The drain (D2): output after exit has its timeout, then the pipes close.
            if let Some((_, _, at)) = &exit {
                if !(eof[0] && eof[1]) && self.clock.passed(Some(at + self.spec.drain_timeout)) {
                    child.close_pipes();
                    drained = false;
                    eof = [true, true];
                }
            }
            if escalate.is_some_and(|at| self.clock.passed(Some(at))) {
                escalate = None;
                if child.group_alive() {
                    pending.extend(child.signal("SIGKILL"));
                    continue;
                }
            }
            let closed = eof[0] && eof[1] && exit.is_some();
            // Its group is gone: nothing is left to escalate on.
            if closed && !child.group_alive() {
                break;
            }
            if closed && child.group_alive() && escalate.is_none() {
                // Its leader exited and its members held on past the close:
                // the group is released as the resource's release would.
                escalate = Some(self.clock.now() + self.spec.force_kill_after);
                pending.extend(child.signal(&self.spec.kill_signal));
                continue;
            }
            if !pending.is_empty() {
                continue;
            }
            let deadline = [
                child.due().map(|d| started + d).filter(|_| !self.released),
                probe_at.filter(|_| !ready && !probing),
                round_end.filter(|_| !ready),
                exit.as_ref()
                    .map(|(_, _, at)| at + self.spec.drain_timeout)
                    .filter(|_| !(eof[0] && eof[1])),
                escalate,
            ]
            .into_iter()
            .flatten()
            .reduce(f64::min);
            self.clock.wait_for(self.waiter, deadline);
            let input = match self.rx.recv_timeout(self.clock.timeout(deadline)) {
                Ok(input) => input,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => Input::Release,
            };
            match input {
                Input::Tick => {}
                Input::Release => self.release(&mut child, &mut escalate, &mut pending),
                Input::Op(m, _) if m != n => {}
                Input::Op(_, ProcessOp::Write { data, drain }) => {
                    self.ctl.consumed(held.saturating_sub(child.queued()));
                    child.write(data);
                    held = child.queued();
                    owed |= drain;
                }
                Input::Op(_, ProcessOp::End) => child.end(),
                Input::Op(_, ProcessOp::Kill(signal)) => {
                    pending.extend(child.signal(&signal));
                    sent.push(signal);
                }
                Input::Child(m, seen) if m == n => pending.push(seen),
                Input::Child(..) => {}
                Input::Probe(m, ok) if m == n => {
                    probing = false;
                    if ok && !ready && !self.released {
                        ready = true;
                        deliver(
                            self.shared,
                            self.ticket,
                            event("ready", json!({ "incarnation": n })),
                            false,
                        );
                    } else {
                        probe_at = self
                            .spec
                            .ready
                            .as_ref()
                            .map(|r| self.clock.now() + r.interval);
                    }
                }
                Input::Probe(..) => {}
            }
        }
        // The decoders' remainders go before `close`.
        for (decoder, name) in [(&mut out, "stdout"), (&mut err, "stderr")] {
            let rest = decoder.flush();
            if utf8 && !rest.is_empty() && !self.released {
                deliver(
                    self.shared,
                    self.ticket,
                    Message {
                        event: name.into(),
                        data: rest,
                        ..Message::default()
                    },
                    true,
                );
            }
        }
        if fail_ready {
            self.timed_out = true;
        }
        let (code, signal, _) = exit.unwrap_or((None, None, 0.0));
        (
            Ended {
                code,
                signal,
                drained,
                sent,
            },
            ready,
        )
    }

    /// Let the child go (D2): its kill signal to the group now, `SIGKILL`
    /// after the grace (at once when it is 0). What the signal does (a
    /// script's exit and close) goes to `pending`, so the loop ends with it.
    fn release(
        &mut self,
        child: &mut Box<dyn Child>,
        escalate: &mut Option<f64>,
        pending: &mut Vec<Seen>,
    ) {
        if self.released {
            return;
        }
        self.released = true;
        let grace = self.spec.force_kill_after;
        let signal = if grace == 0.0 {
            "SIGKILL"
        } else {
            self.spec.kill_signal.as_str()
        };
        pending.extend(child.signal(signal));
        if grace > 0.0 {
            *escalate = Some(self.clock.now() + grace);
        }
    }

    /// Probe the readiness URL once, off this thread, with a 1 s limit; its
    /// answer comes back as [`Input::Probe`].
    fn probe(&self, n: u32) {
        let (Some(ready), Some(bindings)) = (self.spec.ready.as_ref(), self.probe.clone()) else {
            return;
        };
        let (url, accept, ctl) = (ready.url.clone(), ready.accept, self.ctl.clone());
        let spawned = std::thread::Builder::new()
            .name(format!("exact-ready-{}", self.ticket))
            .spawn(move || {
                let abort = AbortController::new();
                let mut req = ibex2::stdlib::fetch::Request::get(&url);
                req.timeout = Some(std::time::Duration::from_secs(1));
                req.max_body = Some(64 << 10);
                let status = bindings
                    .fetch
                    .stream(req, &abort.signal())
                    .map(|r| r.status);
                let ok = status.is_ok_and(|s| (accept.0..=accept.1).contains(&s));
                ctl.send(Input::Probe(n, ok));
            });
        if spawned.is_err() {
            self.ctl.send(Input::Probe(n, false));
        }
    }
}

/// `execFile` (D3): run to `close` and answer `{stdout, stderr, code,
/// signal}`; the timeout kills the group, and output past `maxBuffer` is
/// an error, as Node's are.
pub(crate) fn exec_file(
    shared: &Arc<Shared>,
    spec: &Spec,
    ran: Ran,
    abort: &AbortController,
) -> Outcome {
    match ran {
        Ran::Script(script) => {
            let (stdout, stderr, code, signal, after) = script.exec();
            let clock = shared.processes.clock.clone();
            let (tx, rx) = std::sync::mpsc::channel::<()>();
            let waiter = clock.join(Box::new(move || {
                let _ = tx.send(());
            }));
            let start = clock.now();
            let limit = spec.timeout.map_or(after, |t| t.min(after));
            // Its signal's deadline, before it would have closed, ends it.
            let cut = spec.deadline.filter(|d| *d < limit);
            let until = Some(start + cut.unwrap_or(limit));
            while !clock.passed(until) && !abort.signal().aborted() {
                clock.wait_for(waiter, until);
                let _ = rx.recv_timeout(clock.timeout(until));
            }
            clock.leave(waiter);
            if abort.signal().aborted() {
                return failed(FailureKind::Aborted, "the execFile was let go");
            }
            if let Some(ms) = cut {
                return failed(
                    FailureKind::Timeout,
                    format!("the execFile timed out after {ms} ms"),
                );
            }
            let timed_out = spec.timeout.is_some_and(|t| t < after);
            let (code, signal) = if timed_out {
                (None, Some(spec.kill_signal.clone()))
            } else {
                (code, signal)
            };
            exec_reply(spec, &stdout, &stderr, code, signal, timed_out)
        }
        Ran::Real | Ran::File(_) => super::real::exec(shared, spec, &ran, abort),
    }
}

/// An `execFile`'s answer, as js/src/process.js reads it.
pub(crate) fn exec_reply(
    spec: &Spec,
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i64>,
    signal: Option<String>,
    timed_out: bool,
) -> Outcome {
    let overflow = [("stdout", stdout), ("stderr", stderr)]
        .into_iter()
        .find(|(_, bytes)| bytes.len() > spec.max_buffer)
        .map(|(name, _)| name);
    let cut = |bytes: &[u8]| bytes[..bytes.len().min(spec.max_buffer)].to_vec();
    let text = |bytes: Vec<u8>| {
        if spec.utf8 {
            json!(String::from_utf8_lossy(&bytes))
        } else {
            json!(exact_runner::agent::base64(&bytes))
        }
    };
    super::reply(json!({
        "stdout": text(cut(stdout)),
        "stderr": text(cut(stderr)),
        "code": code,
        "signal": signal,
        "timedOut": timed_out,
        "overflow": overflow,
    }))
}
