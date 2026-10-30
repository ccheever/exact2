//! What the runner keeps for desktop processes (LLP 1133.007): the
//! admission of what data code asks of a child by the origin of the commit
//! that asks it (D13), and the clock a host's process work keeps under the
//! agent (§7).
use super::super::{DataSource, Request, Runner, Target};
use super::admit::held_to_app;
use super::CallOrigin;
use crate::process::WorkClock;
use exact_plan::Value;
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
    /// The arguments of each resource a tool, a wake or the device asked
    /// while its child ran: asked again with them, by any origin, the
    /// resource keeps what it has (D13).
    refused: Vec<(Target, Vec<Value>)>,
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
    /// said; a host never sees a refused one. A `spawn` is a two-way
    /// stream and nothing else is; a person-required refusal in a tool's
    /// call ends the call `person-required` (LLP 1111 D4a).
    pub(in crate::runner) fn process_refusal(&mut self, request: &Request) -> Option<&'static str> {
        if !request.is_process() {
            return None;
        }
        let (kind, _) = crate::process::summary(&request.body);
        let streams = request.stream && request.socket;
        let why = match crate::process::declared_name(&kind) {
            Some(_) if (kind == "spawn") != streams || request.stream != request.socket => {
                format!("{kind}: a spawn is a two-way stream and nothing else is")
            }
            Some(name) => self.admit_process(name, Some(request.reach.origin)).err()?,
            None => format!("{kind}: not a call @exact/child-process declares"),
        };
        self.log(format!("process refused, {why}"));
        Some(match why.starts_with("person-required") {
            true => {
                self.call_person_required();
                "person-required: only a person's input or the app's own code starts a program (LLP 1133.007 D13)"
            }
            false => "process: a call @exact/child-process does not declare (LLP 1133.007 D12)",
        })
    }

    /// Whether resource `i`, which settlement would ask again at `args`
    /// (`forced` for a refresh), keeps what it has instead (LLP 1133.007
    /// D13). A tool's, a wake's or the device's ask would let go of the
    /// child a person's input or the app's own code started, ending its
    /// group, whatever the new answer is: the child keeps running under its
    /// ticket, said, and a tool's call ends `person-required` (LLP 1111
    /// D4a). The arguments it asked stay refused, by any origin and after
    /// the child ends, until the arguments change ([`Self::forget_refused`]).
    pub(in crate::runner) fn keeps_resource(
        &mut self,
        i: usize,
        args: &[Value],
        forced: bool,
    ) -> bool {
        let target = Target::Resource(i);
        let refused = &self.extensions.processes.refused;
        if refused.iter().any(|(t, a)| *t == target && a == args) {
            return true;
        }
        let Some((running, asked)) = self.child_of(target) else {
            return false;
        };
        let origin = self.origin_for(Some(target));
        if origin == CallOrigin::App {
            return false;
        }
        if asked != args {
            let refused = &mut self.extensions.processes.refused;
            refused.retain(|(t, _)| *t != target);
            refused.push((target, args.to_vec()));
        }
        self.call_person_required();
        let (resource, what) = (
            self.target_name(target),
            ["re-ask", "refresh"][usize::from(forced)],
        );
        self.log(format!(
            "process {running} ({resource}): kept running, its {what} refused, person-required ({} origin): letting it go would end its group (LLP 1133.007 D13)",
            origin.word()
        ));
        true
    }

    /// Settlement asks resource `i` at `args`: arguments refused earlier
    /// stay refused only while they are asked (D13).
    pub(in crate::runner) fn forget_refused(&mut self, i: usize, args: &[Value]) {
        let target = Target::Resource(i);
        (self.extensions.processes.refused).retain(|(t, a)| *t != target || a == args);
    }

    /// Whether `target`'s request in flight is a child this commit's origin
    /// may not let go (D13): a tool's, a wake's or the device's would end
    /// its group. Every path that lets a ticket go asks first; a kept child
    /// is said, and a tool's call ends `person-required`.
    pub(in crate::runner) fn holds_child(&mut self, target: Target) -> bool {
        let Some((running, _)) = self.child_of(target) else {
            return false;
        };
        let origin = self.origin_for(Some(target));
        if origin == CallOrigin::App {
            return false;
        }
        self.call_person_required();
        let resource = self.target_name(target);
        self.log(format!(
            "process {running} ({resource}): kept running, person-required ({} origin): letting it go would end its group (LLP 1133.007 D13)",
            origin.word()
        ));
        true
    }

    /// `target`'s child stream in flight: its ticket and the arguments it
    /// was asked with.
    fn child_of(&self, target: Target) -> Option<(u64, Vec<Value>)> {
        let children = &self.extensions.processes.children;
        (self.pending.iter())
            .find(|p| p.target == target && children.contains_key(&p.ticket))
            .map(|p| (p.ticket, p.args.clone()))
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
#[path = "processes_tests.rs"]
mod runner_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::{ProcessOp, KINDS};

    #[test]
    fn every_kind_and_operation_is_a_declared_name_and_app_names_are_held() {
        let ops = [
            ProcessOp::Write {
                data: Vec::new(),
                drain: false,
            },
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
