//! What the runner keeps for desktop processes (LLP 1133.007): the origin
//! of the turns it runs now, stamped on what they ask of a child (D13), and
//! the clock a host's process work keeps under the agent (§7).
use super::*;
use crate::process::{TurnOrigin, WorkClock, ORIGIN_HEADER};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

/// The runner's process state for this session.
#[derive(Default)]
pub(super) struct Processes {
    /// The origin of the inputs it runs now: a person's, unless a turn of
    /// another origin is running. Nothing in the runner makes a tool, wake
    /// or headless turn yet (LLP 1111, 1114.001); the agent's `prefer
    /// origin` stands in for one so the rule can be driven.
    origin: AtomicU8,
    /// The host's process work under the agent ([`Runner::set_work_clock`]).
    pub(super) clock: Option<Arc<dyn WorkClock>>,
    /// Each child stream's facts as its messages told them, by ticket.
    children: std::collections::BTreeMap<u64, ChildState>,
}

/// What `state.streams` shows of a child process (LLP 1133.007 D2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChildState {
    /// The program as its grant line names it.
    pub program: String,
    /// What ran for it: `real`, or under the agent the drive's substitute.
    pub ran: String,
    /// The current incarnation's pid.
    pub pid: Option<u64>,
    /// The current incarnation, from 1.
    pub incarnation: u32,
    /// Restarts so far (D4).
    pub restarts: u32,
    /// Bytes read from its stdout and stderr.
    pub bytes_out: u64,
    /// Bytes the host accepted for its stdin.
    pub bytes_in: u64,
    /// Its last exit: the code, or the signal that ended it.
    pub exit: Option<(Option<i64>, Option<String>)>,
    /// Whether its last incarnation's pipes drained before `close`.
    pub drained: Option<bool>,
}

const ORIGINS: [TurnOrigin; 4] = [
    TurnOrigin::Person,
    TurnOrigin::Tool,
    TurnOrigin::Wake,
    TurnOrigin::Headless,
];

impl<D: DataSource> Runner<D> {
    /// Hand the host's process work its clock under the agent (LLP
    /// 1133.007 §7): each advance runs what is due on it, its next deadline
    /// is a due time `clock settle` reaches, and outcomes it owes keep a
    /// settle waiting. `None` (the default) leaves that work on the wall.
    pub fn set_work_clock(&mut self, clock: Option<Arc<dyn WorkClock>>) {
        if let Some(clock) = &clock {
            clock.advance(self.now_ms);
        }
        self.processes.clock = clock;
    }

    /// The origin of the turns the runner runs now.
    pub fn turn_origin(&self) -> TurnOrigin {
        ORIGINS[usize::from(self.processes.origin.load(Ordering::Relaxed)) % ORIGINS.len()]
    }

    /// Run later turns as `origin`'s, until another is set.
    pub fn set_turn_origin(&self, origin: TurnOrigin) {
        let index = ORIGINS.iter().position(|o| *o == origin).unwrap_or(0);
        self.processes.origin.store(index as u8, Ordering::Relaxed);
    }

    /// Stamp a process request with the origin of the turn that made it,
    /// for the host's check (D13).
    pub(super) fn stamp_process(&self, request: &mut Request) {
        let origin = self.turn_origin();
        if request.is_process() && origin != TurnOrigin::Person {
            request.headers.retain(|(name, _)| name != ORIGIN_HEADER);
            request
                .headers
                .push((ORIGIN_HEADER.into(), origin.name().into()));
        }
    }

    /// When the host's process work is next due on the agent's clock.
    pub(super) fn work_due(&self) -> Option<f64> {
        self.processes.clock.as_ref()?.due()
    }

    /// Whether that work has outcomes the runner has not taken.
    pub(super) fn work_owed(&self) -> bool {
        (self.processes.clock.as_ref()).is_some_and(|clock| clock.owed())
    }

    /// The agent's clock reads `now_ms`: the host's process work due by
    /// then runs.
    pub(super) fn advance_work(&self, now_ms: f64) {
        if let Some(clock) = &self.processes.clock {
            clock.advance(now_ms);
        }
    }

    /// A child stream asked for: its program, for `state.streams`.
    pub(super) fn note_child(&mut self, ticket: u64, request: &Request) {
        if !(request.is_process() && request.stream) {
            return;
        }
        let program = crate::process::summary(&request.body).1.unwrap_or_default();
        let held: Vec<u64> = self.pending.iter().map(|p| p.ticket).collect();
        let children = &mut self.processes.children;
        children.retain(|t, _| held.contains(t));
        children.insert(
            ticket,
            ChildState {
                program,
                ..ChildState::default()
            },
        );
    }

    /// What one message of a child stream tells of it.
    pub(super) fn note_child_message(&mut self, ticket: u64, message: &crate::Message) {
        let Some(child) = self.processes.children.get_mut(&ticket) else {
            return;
        };
        let data: serde_json::Value = serde_json::from_str(&message.data).unwrap_or_default();
        match message.event.as_str() {
            "spawn" => {
                child.pid = data["pid"].as_u64();
                child.incarnation = data["incarnation"].as_u64().unwrap_or(1) as u32;
                child.restarts = data["restarts"].as_u64().unwrap_or(0) as u32;
                child.ran = data["ran"].as_str().unwrap_or("real").to_string();
                child.exit = None;
                child.drained = None;
            }
            "stdout" | "stderr" => {
                let n = message.binary.as_ref().map_or(message.data.len(), Vec::len);
                child.bytes_out += n as u64;
            }
            "exit" => {
                child.exit = Some((
                    data["code"].as_i64(),
                    data["signal"].as_str().map(str::to_string),
                ));
            }
            "close" => child.drained = data["drained"].as_bool(),
            _ => {}
        }
    }

    /// Bytes a host accepted for a child's stdin.
    pub(super) fn note_child_sent(&mut self, ticket: u64, bytes: u64) {
        if let Some(child) = self.processes.children.get_mut(&ticket) {
            child.bytes_in += bytes;
        }
    }

    /// What `state.streams` shows of the child stream `ticket`.
    pub fn child_state(&self, ticket: u64) -> Option<&ChildState> {
        self.processes.children.get(&ticket)
    }
}
