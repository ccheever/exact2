//! What the runner keeps for desktop processes (LLP 1133.007): the
//! admission of what data code asks of a child by the origin of the commit
//! that asks it (D13), and the clock a host's process work keeps under the
//! agent (§7).
use super::super::{DataSource, Request, Runner, Target};
use super::admit::held_to_app;
use super::CallOrigin;
use crate::process::WorkClock;
use std::sync::Arc;

/// `@exact/child-process`'s declaration: the `origin` of each name in its
/// `data` list is held as a module name's is (`admit.rs`).
const DECLARATION: &str = include_str!("../../../../packages/child-process/exact.json");

/// The runner's process state for this session.
#[derive(Default)]
pub(in crate::runner) struct Processes {
    /// The host's process work under the agent ([`Runner::set_work_clock`]).
    pub(super) clock: Option<Arc<dyn WorkClock>>,
    /// Each child stream's facts as its messages told them, by ticket.
    children: std::collections::BTreeMap<u64, ChildState>,
}

/// What `state.streams` shows of a child process (LLP 1133.007 D2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ChildState {
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

impl<D: DataSource> Runner<D> {
    /// Hand the host's process work its clock under the agent (LLP
    /// 1133.007 §7): each advance runs what is due on it, its next deadline
    /// is a due time `clock settle` reaches, and outcomes it owes keep a
    /// settle waiting. `None` (the default) leaves that work on the wall.
    pub fn set_work_clock(&mut self, clock: Option<Arc<dyn WorkClock>>) {
        if let Some(clock) = &clock {
            clock.advance(self.now_ms);
        }
        self.extensions.processes.clock = clock;
    }

    /// Why data code may not call `name` of `@exact/child-process` now
    /// ([`refusal`]). `asked` is the origin of the request that calls it,
    /// else the commit's.
    pub(in crate::runner) fn admit_process(
        &self,
        name: &str,
        asked: Option<CallOrigin>,
    ) -> Result<(), String> {
        refusal(name, asked.unwrap_or_else(|| self.origin_for(None)))
    }

    /// The refusal of process request `request` by its kind and origin,
    /// said; a host never sees a refused one.
    pub(in crate::runner) fn process_refusal(&mut self, request: &Request) -> Option<&'static str> {
        if !request.is_process() {
            return None;
        }
        let (kind, _) = crate::process::summary(&request.body);
        let name = crate::process::declared_name(&kind).unwrap_or(&kind);
        let why = self.admit_process(name, Some(request.reach.origin)).err()?;
        self.log(format!("process refused, {why}"));
        Some(match why.starts_with("person-required") {
            true => "person-required: only a person's input or the app's own code starts a program (LLP 1133.007 D13)",
            false => "process: a call @exact/child-process does not declare (LLP 1133.007 D12)",
        })
    }

    /// A re-ask of a resource whose child is running, refused for its
    /// origin: the child keeps running under its old ticket, since letting
    /// that go would end its group, which a tool, a wake or the device may
    /// not do (LLP 1133.007 D13). Said; `true` when kept.
    pub(in crate::runner) fn keeps_child(&mut self, target: Target, request: &Request) -> bool {
        if !(request.is_process() && request.stream) {
            return false;
        }
        let children = &self.extensions.processes.children;
        let Some(running) = (self.pending.iter())
            .find(|p| p.target == target && children.contains_key(&p.ticket))
            .map(|p| p.ticket)
        else {
            return false;
        };
        let (kind, _) = crate::process::summary(&request.body);
        let name = crate::process::declared_name(&kind).unwrap_or(&kind);
        let Err(why) = self.admit_process(name, Some(self.origin_for(Some(target)))) else {
            return false;
        };
        let resource = self.target_name(target);
        self.log(format!(
            "process {running} ({resource}): kept running, its replacement refused, {why}"
        ));
        true
    }

    /// When the host's process work is next due on the agent's clock.
    pub(in crate::runner) fn work_due(&self) -> Option<f64> {
        self.extensions.processes.clock.as_ref()?.due()
    }

    /// Whether that work has outcomes the runner has not taken.
    pub(in crate::runner) fn work_owed(&self) -> bool {
        (self.extensions.processes.clock.as_ref()).is_some_and(|clock| clock.owed())
    }

    /// The agent's clock reads `now_ms`: the host's process work due by
    /// then runs.
    pub(in crate::runner) fn advance_work(&self, now_ms: f64) {
        if let Some(clock) = &self.extensions.processes.clock {
            clock.advance(now_ms);
        }
    }

    /// A child stream asked for: its program, for `state.streams`.
    pub(in crate::runner) fn note_child(&mut self, ticket: u64, request: &Request) {
        if !(request.is_process() && request.stream) {
            return;
        }
        let program = crate::process::summary(&request.body).1.unwrap_or_default();
        let held: Vec<u64> = self.pending.iter().map(|p| p.ticket).collect();
        let children = &mut self.extensions.processes.children;
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
    pub(in crate::runner) fn note_child_message(&mut self, ticket: u64, message: &crate::Message) {
        let Some(child) = self.extensions.processes.children.get_mut(&ticket) else {
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
    pub(in crate::runner) fn note_child_sent(&mut self, ticket: u64, bytes: u64) {
        if let Some(child) = self.extensions.processes.children.get_mut(&ticket) {
            child.bytes_in += bytes;
        }
    }

    /// What `state.streams` shows of the child stream `ticket`.
    pub(crate) fn child_state(&self, ticket: u64) -> Option<&ChildState> {
        self.extensions.processes.children.get(&ticket)
    }
}

/// Why `name` of `@exact/child-process` is refused from `origin`: a name
/// its declaration does not list, or one whose `origin` is `app` called
/// from a tool, a wake or the device (`person-required`, LLP 1133.007 D13).
fn refusal(name: &str, origin: CallOrigin) -> Result<(), String> {
    static NAMES: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();
    let declaration = NAMES.get_or_init(|| serde_json::from_str(DECLARATION).unwrap_or_default());
    let declared =
        (declaration["data"].as_array().into_iter().flatten()).find(|d| d["data"] == name);
    match declared {
        None => Err(format!("{name}: not a call @exact/child-process declares")),
        Some(d) if held_to_app(d, &serde_json::json!({})) && origin != CallOrigin::App => {
            Err(format!(
                "person-required ({} origin): {name} (LLP 1133.007 D13)",
                origin.word()
            ))
        }
        Some(_) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::{ProcessOp, KINDS};

    #[test]
    fn every_kind_and_operation_is_a_declared_name_and_app_names_are_held() {
        let ops = [
            ProcessOp::Write(Vec::new()),
            ProcessOp::End,
            ProcessOp::Kill("SIGTERM".into()),
        ];
        let names: Vec<&str> = (KINDS.iter().map(|(_, n)| *n))
            .chain(ops.iter().map(ProcessOp::declared))
            .collect();
        for name in &names {
            assert_eq!(refusal(name, CallOrigin::App), Ok(()), "{name} is declared");
        }
        let declaration: serde_json::Value = serde_json::from_str(DECLARATION).unwrap();
        for d in declaration["data"].as_array().unwrap() {
            let name = d["data"].as_str().unwrap();
            // `processStatus` is a fact the binding answers; it asks the host nothing.
            assert!(
                names.contains(&name) || name == "processStatus",
                "{name} reaches the runner"
            );
            for origin in [CallOrigin::Tool, CallOrigin::Wake, CallOrigin::Device] {
                let refused = refusal(name, origin).is_err();
                assert_eq!(
                    refused,
                    held_to_app(d, &serde_json::json!({})),
                    "{name} from {origin:?}"
                );
            }
        }
        assert!(
            refusal("shell", CallOrigin::App).is_err(),
            "an undeclared kind"
        );
    }
}
