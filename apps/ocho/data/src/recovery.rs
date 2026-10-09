//! Terminal-tab recovery, the paused card and the LIMITED banner
//! (workspace.rs `RecoveryState`, `RecoveryLaunchMode`,
//! `tab_supports_recovery`, `recover_active_exited_session`,
//! `assess_recovery`, `launch_active_recovery`, `recovery_launch_args`,
//! `automatic_terminal_recovery_allowed`, `automatic_recovery_ready`,
//! `machine_unreachable`, the `Command::Recovery*` and `UnpauseTab` arms,
//! `resuming_tabs`; backend.rs `LaunchRecovery`, `RecoveryLaunch`; ui.rs
//! `render_tab_body`, `recovery_overlay`, `paused_overlay`,
//! `disconnected_overlay`, `limited_banner`), without the PTYs and the
//! background executor. Time is host milliseconds (`now_ms`).
//!
//! Each terminal tab owns one [`Recovery`]. The model keeps it beside the
//! tab (keyed by the tab key, which recovery itself can change: see
//! [`Assessed::Reattach`] and [`Relaunch`]), asks it what to run, and
//! answers it with the reply.
//!
//! # When to assess
//!
//! [`Recovery::assess`] returns the argv and moves to `Checking`. The model
//! calls it, for tab `pos` with `tab.session = (machine, session, read_only)`:
//! - when the tab's terminal process exits and [`should_assess_on_exit`]
//!   holds (workspace.rs `observe_terminal`, `recover_on_exit` is always
//!   true in Ocho);
//! - after the active tab changes (Next/Prev/Select tab, opening an
//!   existing tab, a refreshed inventory) when [`should_check_active`]
//!   holds (`recover_active_exited_session`);
//! - from Reconnect when the transport cannot retry in place and
//!   [`reconnect_assesses`] holds (`reconnect_tab`); otherwise Reconnect
//!   respawns the tab's frozen argv;
//! - from `recovery-continue` ([`Command::RecoveryContinue`]) with
//!   `continue_search = true`.
//!
//! # The `fleet` commands
//!
//! - `recovery M S [--continue]` → a `LaunchRecovery` JSON
//!   (`{status, reason, session?, launch}`), answered by
//!   [`Recovery::set_assessment`]. Status `live`/`running` with a session
//!   reattaches; `terminal-uncertain`, `agent-uncertain`, `restorable`,
//!   `ended` become the matching state; any other status is agent
//!   uncertainty. An error whose text names a transport failure
//!   ([`machine_unreachable`]) is `Unreachable`; any other error (and an
//!   unparsable reply, "Invalid recovery response: …") is
//!   `TerminalUncertain`. The reply is routed by the tab key the request
//!   was made for; a tab that has gone drops it.
//! - The relaunch is not a job but the tab's new terminal argv
//!   ([`recovery_launch_args`]): `run M --request-id R --provider P
//!   --account A --cwd C --model X --effort E [--resume ID] [--fork]
//!   [--claude-remote] [--permissions MODE] --attach`. Resume and fork pass
//!   `--resume`; replace starts fresh; the original prompt is never replayed.
//! - A live session is reattached with `attach M S [--read-only]`.
//! - `unpause M S` ([`unpause_argv`]) runs as a CLI job with the done
//!   message [`UNPAUSE_DONE`].
//!
//! # Presses
//!
//! The overlay buttons press `tab:<id>`; [`press_command`] maps an id to
//! the command the model executes, and the commands do:
//! - `recovery-retry` → `ReconnectTab` (retry the transport, else assess).
//! - `recovery-continue` → `RecoveryContinue`: `assess(…, true)`.
//! - `recovery-restore` → `RecoveryRestore`: `launch(Resume, …)`.
//! - `recovery-fork` → `RecoveryFork`: open the fork confirmation
//!   ([`fork_confirm_texts`]; the target is the tab's session, or one built
//!   from the tab's ids and title when it is not observed); its yes runs
//!   `launch(Fork, …)`.
//! - `recovery-replace` → `RecoveryReplace`: `launch(Replace, …)`.
//! - `recovery-view` → `ToggleTranscript`.
//! - `recovery-details` → `RecoveryDetails`: [`Recovery::toggle_details`].
//! - `recovery-close` → `CloseTab`.
//! - `unpause-button` → `UnpauseTab`: [`unpause_argv`]; on `Ok` the model
//!   adds the tab key to its resuming set and runs the argv with
//!   [`UNPAUSE_DONE`]; on `Err` it shows the message. Each machine
//!   observation keeps a key only while [`still_resuming`] holds.
//!
//! # The view
//!
//! [`tab_chrome`] returns these `TabView` fields for the active tab (the
//! connection strip and the transcript are the model's):
//!
//! ```text
//! overlay: string       // "" | "recovery" | "paused" | "disconnected" | "reconnecting"
//! overlayTitle: string
//! overlayDetail: string
//! overlayButtons: list<TabButton>   // {id, label, hint, primary, disabled}; press `tab:<id>`
//! overlayNote: string   // 11 px muted line under the buttons, "" when none
//! overlayReason: string // the Details box (recovery only), "" when hidden
//! exitBanner: string    // "Connection ended." | "Connection ended (exit status N)." | ""
//! limited: bool
//! limitedStatus: string // the session's summary ("Usage limit reached" by default)
//! limitedProvider: string // the provider's label ("Codex"), for "Start a fresh agent here with another {} account."
//! ```
//!
//! Which overlay shows, first match wins:
//! 1. `recovery` when the tab's recovery is not `None`.
//! 2. `reconnecting` / `disconnected` when the connection conceals the
//!    terminal (transport `ssh` in state `disconnected` or `reconnecting`),
//!    or the tab is remote (it has a connection, or its machine is not
//!    local), has a session, and its process exited with a nonzero code.
//!    It is `reconnecting` when the connection's state is `reconnecting`.
//! 3. `paused` when the process has not exited and the observed session's
//!    raw state is `paused`.
//!
//! The exit banner shows when the process exited, recovery is `None` and
//! the tab is not remote (a remote tab shows the connection strip instead).
//! LIMITED shows while the process runs and the session is LIMITED.
//!
//! # Drawing (ui.rs, 1:1; rem 16 px, TEXT_LG 18, TEXT_SM 12, TEXT_XS 11)
//!
//! - Recovery card: a scrim (absolute inset 0, `bg` at 78 %), centered a
//!   card 512 wide (`rems(32)`), column centered `gap_3` (12), `px_6 py_5`
//!   (24 × 20), `rounded_lg` (8), bg `elevated`, `border_1` `border`,
//!   `shadow_lg`: the title (18 px semibold `text`), the detail (12 px
//!   `muted`, centered), the buttons (wrapping row, centered, `gap_2` 8;
//!   hidden while busy), the note (11 px `muted`), then the reason box
//!   (`w_full`, `px_3 py_2` 12 × 8, `rounded_md` 6, bg `bg`, 11 px `muted`).
//! - Paused card: scrim `bg` at 72 %; the card as above without the fixed
//!   width: "Paused to free memory", the status (12 px `muted`), the
//!   Unpause button (primary), the note.
//! - Disconnected card: scrim `bg` at 55 %; `px_6 py_4` (24 × 16),
//!   `rounded_lg`, bg `elevated` at 94 %, `border_1`, `shadow_lg`; only the
//!   title (18 px semibold).
//! - Buttons (ui.rs `button`): row `gap_1p5` (6), `px_2 py_0p5` (8 × 2),
//!   `rounded_md`, bg `accent` at 15 % if primary else `element`, hover
//!   `selected`; the label, then the hint (11 px `placeholder`) if any.
//! - LIMITED banner: row `gap_3` (12), `px_4 py_2` (16 × 8), bg `warn` at
//!   8 %, `border_t_1` `warn` at 35 %: "LIMITED" (11 px bold `warn`); a
//!   column of the status (`text`) and the hint (11 px `muted`); a spacer;
//!   "Hand off…" (primary, `Command::Handoff`).

use crate::palette::Command;
use crate::session::{clean, provider_label, session_state, session_summary};
use crate::types::{LaunchRecovery, Machine, RecoveryLaunch, Session};
use serde_json::{json, Value as Json};

/// An automatic restart waits this long after the last one, so a provider
/// that exits repeatedly is not relaunched in a loop.
pub const AUTOMATIC_RECOVERY_BACKOFF_MS: f64 = 60_000.0;
/// The done message of `unpause`.
pub const UNPAUSE_DONE: &str = "Session resumed";
/// UnpauseTab on a tab whose session is not paused.
pub const NOT_PAUSED: &str = "This session is not paused";
/// The message after a live session was reattached.
pub const REATTACHED: &str = "Reconnected to the existing session";

/// Where a tab's recovery stands (workspace.rs `RecoveryState`).
#[derive(Clone, Debug, Default, PartialEq)]
pub enum RecoveryState {
    /// No recovery in progress; the terminal shows.
    #[default]
    None,
    /// `fleet recovery` is running.
    Checking,
    /// The machine could not be reached; the error.
    Unreachable(String),
    /// The saved terminal could not be verified; the reason.
    TerminalUncertain(String),
    /// Another copy of the agent could still be running; the reason.
    AgentUncertain(String, RecoveryLaunch),
    /// The agent is gone and its conversation can be resumed.
    Restorable(RecoveryLaunch),
    /// The agent is gone with no conversation to resume.
    Ended(RecoveryLaunch),
    /// A relaunch is being opened.
    Restoring,
}

impl RecoveryState {
    /// The card's title and detail (ui.rs `recovery_overlay`); `None` for
    /// [`RecoveryState::None`].
    pub fn texts(&self) -> Option<(&'static str, &'static str)> {
        Some(match self {
            RecoveryState::None => return None,
            RecoveryState::Checking => (
                "Checking session",
                "Fleet is looking for the saved terminal and its agent.",
            ),
            RecoveryState::Unreachable(_) => (
                "Machine unreachable",
                "Fleet couldn’t reach this machine. Nothing has been started or stopped.",
            ),
            RecoveryState::TerminalUncertain(_) => (
                "Session state uncertain",
                "Fleet couldn’t verify the saved terminal. It can continue with a read-only agent search.",
            ),
            RecoveryState::AgentUncertain(_, _) => (
                "Another execution could still be running",
                "Fleet can’t rule out another copy of this conversation running.",
            ),
            RecoveryState::Restorable(_) => (
                "Terminal ended",
                "Fleet confirmed that the old agent is gone and found its saved conversation.",
            ),
            RecoveryState::Ended(_) => (
                "Session ended",
                "The terminal and agent are gone, and no saved conversation is available to restore.",
            ),
            RecoveryState::Restoring => (
                "Restoring conversation",
                "Opening the saved conversation in a new terminal…",
            ),
        })
    }

    /// Checking or restoring: the card shows no buttons.
    pub fn busy(&self) -> bool {
        matches!(self, RecoveryState::Checking | RecoveryState::Restoring)
    }

    /// The reason the Details box shows, "" when the state has none.
    pub fn reason(&self) -> &str {
        match self {
            RecoveryState::Unreachable(reason)
            | RecoveryState::TerminalUncertain(reason)
            | RecoveryState::AgentUncertain(reason, _) => reason,
            _ => "",
        }
    }

    /// A conversation id is known, so a fork can be offered.
    pub fn can_fork(&self) -> bool {
        match self {
            RecoveryState::Restorable(launch) | RecoveryState::AgentUncertain(_, launch) => {
                !launch.resume.is_empty()
            }
            _ => false,
        }
    }
}

/// How a relaunch starts (workspace.rs `RecoveryLaunchMode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaunchMode {
    /// Resume the saved conversation.
    Resume,
    /// Fork the saved conversation into a new one.
    Fork,
    /// Start a fresh agent with the same settings.
    Replace,
}

impl LaunchMode {
    /// The message once the new terminal opens.
    pub fn message(self) -> &'static str {
        match self {
            LaunchMode::Resume => "Restoring conversation…",
            LaunchMode::Fork => "Forking conversation…",
            LaunchMode::Replace => "Starting replacement…",
        }
    }
}

/// One tab's recovery (the `recovery`, `recovery_details` and
/// `automatic_recovery_at` fields of workspace.rs `TerminalTab`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Recovery {
    /// Where it stands.
    pub state: RecoveryState,
    /// The Details box is open.
    pub details: bool,
    /// When the last automatic relaunch started, host ms.
    pub automatic_at: Option<f64>,
    /// The state before a relaunch, restored if it fails.
    previous: Option<RecoveryState>,
}

/// What the model does after [`Recovery::set_assessment`].
#[derive(Clone, Debug, PartialEq)]
pub enum Assessed {
    /// Only the card changed.
    Shown,
    /// The agent is alive under `session`: re-key the tab to `key`, set
    /// its session to `(machine, session, read_only)`, its reconnect argv
    /// and terminal to `argv` (keeping the last healthy frame), persist the
    /// tabs and say [`REATTACHED`]. If the terminal cannot start, call
    /// [`Recovery::attach_failed`].
    Reattach {
        /// The live session's id.
        session: String,
        /// The tab's new key, `{machine}:{session}:{read_only}`.
        key: String,
        /// `attach M S [--read-only]`.
        argv: Vec<String>,
    },
    /// Relaunch automatically: call [`Recovery::launch`] with this mode
    /// (the tab is the active one).
    Launch(LaunchMode),
}

/// A relaunch the model opens in the tab (workspace.rs
/// `launch_active_recovery`).
#[derive(Clone, Debug, PartialEq)]
pub struct Relaunch {
    /// The tab's new terminal argv, also its reconnect argv.
    pub argv: Vec<String>,
    /// The tab's new key, `{machine}:{request}:{read_only}`.
    pub key: String,
    /// The tab's new session tuple.
    pub session: (String, String, bool),
    /// The message once it opens.
    pub message: &'static str,
}

impl Recovery {
    /// Start an assessment of `machine`/`session` (the tab's own ids):
    /// the `fleet` argv, or `None` while one is checking or restoring.
    pub fn assess(
        &mut self,
        machine: &str,
        session: &str,
        continue_search: bool,
    ) -> Option<Vec<String>> {
        if self.state.busy() {
            return None;
        }
        self.state = RecoveryState::Checking;
        self.details = false;
        let mut args = vec!["recovery".to_string(), machine.into(), session.into()];
        if continue_search {
            args.push("--continue".into());
        }
        Some(args)
    }

    /// The reply to `recovery`. `machine` and `read_only` are the tab's;
    /// `active` says the tab is the active one.
    pub fn set_assessment(
        &mut self,
        reply: Result<String, String>,
        machine: &str,
        read_only: bool,
        active: bool,
        now_ms: f64,
    ) -> Assessed {
        let parsed = reply.and_then(|output| {
            serde_json::from_str::<LaunchRecovery>(&output)
                .map_err(|e| format!("Invalid recovery response: {e}"))
        });
        let recovery = match parsed {
            Ok(recovery) => recovery,
            Err(error) => {
                self.state = if machine_unreachable(&error) {
                    RecoveryState::Unreachable(error)
                } else {
                    RecoveryState::TerminalUncertain(error)
                };
                return Assessed::Shown;
            }
        };
        let automatic = |at: &mut Option<f64>| {
            let ready = automatic_recovery_ready(active, read_only, *at, now_ms);
            if ready {
                *at = Some(now_ms);
            }
            ready
        };
        match recovery.status.as_str() {
            "live" | "running" => {
                let Some(found) = recovery.session else {
                    self.state = RecoveryState::AgentUncertain(
                        "Fleet found the agent but not its terminal identity".into(),
                        recovery.launch,
                    );
                    return Assessed::Shown;
                };
                self.state = RecoveryState::None;
                Assessed::Reattach {
                    key: format!("{machine}:{}:{read_only}", found.id),
                    argv: crate::tab_tree::session_attach_command(machine, &found.id, read_only),
                    session: found.id,
                }
            }
            "terminal-uncertain" => {
                self.state = RecoveryState::TerminalUncertain(recovery.reason);
                Assessed::Shown
            }
            "agent-uncertain" => {
                self.state = RecoveryState::AgentUncertain(recovery.reason, recovery.launch);
                Assessed::Shown
            }
            "restorable" => {
                self.state = RecoveryState::Restorable(recovery.launch);
                if automatic(&mut self.automatic_at) {
                    Assessed::Launch(LaunchMode::Resume)
                } else {
                    Assessed::Shown
                }
            }
            "ended" => {
                self.state = RecoveryState::Ended(recovery.launch);
                if automatic(&mut self.automatic_at) {
                    Assessed::Launch(LaunchMode::Replace)
                } else {
                    Assessed::Shown
                }
            }
            _ => {
                self.state = RecoveryState::AgentUncertain(recovery.reason, recovery.launch);
                Assessed::Shown
            }
        }
    }

    /// The reattach terminal of [`Assessed::Reattach`] could not start.
    pub fn attach_failed(&mut self, error: &str) {
        self.state = RecoveryState::TerminalUncertain(error.to_string());
    }

    /// Relaunch on `machine` (the tab's) under the fresh `request` id, when
    /// the state offers `mode`: resume or fork a restorable conversation,
    /// fork an uncertain one, replace an ended one. Moves to `Restoring`;
    /// the model opens the terminal, then calls [`Recovery::launched`] or
    /// [`Recovery::launch_failed`].
    pub fn launch(
        &mut self,
        mode: LaunchMode,
        machine: &str,
        request: &str,
        read_only: bool,
    ) -> Option<Relaunch> {
        let launch = match (&self.state, mode) {
            (RecoveryState::Restorable(launch), LaunchMode::Resume | LaunchMode::Fork)
            | (RecoveryState::AgentUncertain(_, launch), LaunchMode::Fork)
            | (RecoveryState::Ended(launch), LaunchMode::Replace) => launch.clone(),
            _ => return None,
        };
        let argv = recovery_launch_args(machine, request, &launch, mode);
        self.previous = Some(std::mem::replace(&mut self.state, RecoveryState::Restoring));
        Some(Relaunch {
            argv,
            key: format!("{machine}:{request}:{read_only}"),
            session: (machine.to_string(), request.to_string(), read_only),
            message: mode.message(),
        })
    }

    /// The relaunch terminal opened: the model also re-keys the tab, sets
    /// its session and reconnect argv, and clears its transcript.
    pub fn launched(&mut self) {
        self.state = RecoveryState::None;
        self.details = false;
        self.previous = None;
    }

    /// The relaunch terminal could not open: the state goes back and the
    /// returned error is shown.
    pub fn launch_failed(&mut self, error: &str) -> String {
        if let Some(previous) = self.previous.take() {
            self.state = previous;
        }
        format!("Could not open recovery terminal: {error}")
    }

    /// `RecoveryDetails`.
    pub fn toggle_details(&mut self) {
        self.details = !self.details;
    }

    /// A worker terminal was attached in place (workspace.rs
    /// `attach_worker_terminal`): nothing to recover.
    pub fn clear(&mut self) {
        self.state = RecoveryState::None;
    }
}

/// Only managed sessions can be recovered (workspace.rs
/// `tab_supports_recovery`); `session` is the tab's observed session.
pub fn tab_supports_recovery(session: Option<&Session>) -> bool {
    session.is_some_and(|session| session.managed)
}

/// Native provider clients own their connections. If one exits, leave its
/// terminal visible until the user explicitly reconnects.
pub fn automatic_terminal_recovery_allowed(session: Option<&Session>) -> bool {
    session.is_some_and(|session| {
        session.codex_socket.is_empty() && !session.claude_remote && !session.eas
    })
}

/// An automatic relaunch needs the tab active and writable, and a minute
/// since the last one.
pub fn automatic_recovery_ready(
    active: bool,
    read_only: bool,
    previous: Option<f64>,
    now_ms: f64,
) -> bool {
    active
        && !read_only
        && previous.is_none_or(|at| (now_ms - at).max(0.0) >= AUTOMATIC_RECOVERY_BACKOFF_MS)
}

/// The tab's terminal process exited: assess when the observed session
/// supports recovery, allows it automatically, and none is under way.
pub fn should_assess_on_exit(session: Option<&Session>, recovery: &Recovery) -> bool {
    tab_supports_recovery(session)
        && automatic_terminal_recovery_allowed(session)
        && recovery.state == RecoveryState::None
}

/// A provider can exit while its terminal stays connected. Watcher state
/// catches that case and retries an unreachable tab once the machine is
/// observed again (workspace.rs `recover_active_exited_session`), for the
/// active tab: `tab_session` is its `(machine, session, read_only)`,
/// `observed` its machine and session in the inventory.
pub fn should_check_active(
    tab_session: Option<&(String, String, bool)>,
    observed: Option<(&Machine, &Session)>,
    recovery: &Recovery,
    now_ms: f64,
) -> bool {
    tab_session.is_some_and(|(_, _, read_only)| !read_only)
        && observed.is_some_and(|(machine, session)| {
            session.managed
                && !session.historical
                && machine.error.is_empty()
                && crate::session::session_observation_available(machine, session, now_ms)
                && (matches!(recovery.state, RecoveryState::Unreachable(_))
                    || (recovery.state == RecoveryState::None
                        && session.state == "exited"
                        && session.pid == 0))
        })
}

/// Reconnect assesses instead of respawning when the session supports
/// recovery and is not a Codex app-server: that reconnection reclaims the
/// thread through the gateway, and discovery could mistake a displaced
/// viewer for a stopped agent (workspace.rs `reconnect_tab`).
pub fn reconnect_assesses(session: Option<&Session>) -> bool {
    tab_supports_recovery(session) && session.is_some_and(|s| s.codex_socket.is_empty())
}

/// Transport failures, not session uncertainty (workspace.rs
/// `machine_unreachable`).
pub fn machine_unreachable(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    [
        "ssh",
        "timed out",
        "connection refused",
        "connection closed",
        "no route to host",
        "network is unreachable",
        "could not resolve hostname",
        "name or service not known",
        "permission denied",
        "host key verification failed",
    ]
    .iter()
    .any(|marker| error.contains(marker))
}

/// The relaunch argv (workspace.rs `recovery_launch_args`).
pub fn recovery_launch_args(
    machine: &str,
    request: &str,
    launch: &RecoveryLaunch,
    mode: LaunchMode,
) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "run".into(),
        machine.into(),
        "--request-id".into(),
        request.into(),
        "--provider".into(),
        launch.provider.clone(),
        "--account".into(),
        launch.account.clone(),
        "--cwd".into(),
        launch.cwd.clone(),
        "--model".into(),
        launch.model.clone(),
        "--effort".into(),
        launch.effort.clone(),
    ];
    if mode != LaunchMode::Replace {
        args.extend(["--resume".into(), launch.resume.clone()]);
    }
    if mode == LaunchMode::Fork {
        args.push("--fork".into());
    }
    if launch.claude_remote {
        args.push("--claude-remote".into());
    }
    args.push("--attach".into());
    crate::launch::with_permissions(args, &launch.permissions)
}

/// The fork confirmation (ui.rs `ConfirmAction::ForkRecovery`): title,
/// body and the yes label, for the session's (cleaned) title and its
/// machine's name.
pub fn fork_confirm_texts(name: &str, machine: &str) -> (String, String, &'static str) {
    (
        format!("Fork {name}?"),
        format!("{machine} · This starts a separate conversation in the same project folder. The uncertain execution is not stopped or changed, so both agents may edit the same files."),
        "Fork conversation",
    )
}

/// `UnpauseTab` for the active tab's observed session: `unpause M S`, or
/// the message when it is not paused.
pub fn unpause_argv(observed: Option<(&Machine, &Session)>) -> Result<Vec<String>, &'static str> {
    observed
        .filter(|(_, s)| s.state == "paused")
        .map(|(m, s)| vec!["unpause".to_string(), m.id.clone(), s.id.clone()])
        .ok_or(NOT_PAUSED)
}

/// A resume is over once `machine` no longer reports the tab's session
/// paused (workspace.rs `merge_machine`): keep the tab's key in the
/// resuming set while this holds. A tab on another machine keeps it.
pub fn still_resuming(tab_session: Option<&(String, String, bool)>, machine: &Machine) -> bool {
    tab_session.is_some_and(|(m, s, _)| {
        m != &machine.id
            || machine.last.as_ref().is_some_and(|snapshot| {
                snapshot
                    .sessions
                    .iter()
                    .any(|item| &item.id == s && item.state == "paused")
            })
    })
}

/// The command a `tab:<id>` press of an overlay button runs.
pub fn press_command(id: &str) -> Option<Command> {
    Some(match id {
        "recovery-retry" => Command::ReconnectTab,
        "recovery-continue" => Command::RecoveryContinue,
        "recovery-restore" => Command::RecoveryRestore,
        "recovery-replace" => Command::RecoveryReplace,
        "recovery-view" => Command::ToggleTranscript,
        "recovery-fork" => Command::RecoveryFork,
        "recovery-details" => Command::RecoveryDetails,
        "recovery-close" => Command::CloseTab,
        "unpause-button" => Command::UnpauseTab,
        _ => return None,
    })
}

/// What [`tab_chrome`] reads for the active tab.
pub struct Chrome<'a> {
    /// The tab's recovery.
    pub recovery: &'a Recovery,
    /// The tab's observed machine and session, if any.
    pub observed: Option<(&'a Machine, &'a Session)>,
    /// The tab follows a session (`tab.session.is_some()`), observed or not.
    pub has_session: bool,
    /// The tab's machine is known and not local.
    pub remote_machine: bool,
    /// The terminal's transport report: (state, transport, retryable,
    /// detail), state `connecting | connected | reconnecting |
    /// disconnected | ended`, transport `ssh | mosh`.
    pub connection: Option<(&'a str, &'a str, bool, String)>,
    /// The terminal process exited.
    pub exited: bool,
    /// Its exit code, when the host reported one.
    pub exit_code: Option<i32>,
    /// UnpauseTab was pressed and the session is still reported paused.
    pub resuming: bool,
}

fn button(id: &str, label: &str, primary: bool, hint: &dyn Fn(Command) -> String) -> Json {
    json!({
        "id": id,
        "label": label,
        "hint": press_command(id).map(hint).unwrap_or_default(),
        "primary": primary,
        "disabled": false,
    })
}

/// The recovery card's buttons, in ui.rs order.
fn recovery_buttons(state: &RecoveryState, hint: &dyn Fn(Command) -> String) -> Vec<Json> {
    if state.busy() {
        return Vec::new();
    }
    let mut out = Vec::new();
    match state {
        RecoveryState::Unreachable(_) | RecoveryState::AgentUncertain(_, _) => {
            out.push(button("recovery-retry", "Try again", true, hint));
        }
        RecoveryState::TerminalUncertain(_) => {
            out.push(button("recovery-retry", "Try again", true, hint));
            out.push(button(
                "recovery-continue",
                "Continue with agent search…",
                false,
                hint,
            ));
        }
        RecoveryState::Restorable(_) => {
            out.push(button(
                "recovery-restore",
                "Restore conversation",
                true,
                hint,
            ));
        }
        RecoveryState::Ended(_) => {
            out.push(button("recovery-replace", "Start replacement", true, hint));
        }
        _ => {}
    }
    if !matches!(state, RecoveryState::None) {
        out.push(button("recovery-view", "View conversation", false, hint));
    }
    if state.can_fork() {
        let label = if matches!(state, RecoveryState::AgentUncertain(_, _)) {
            "Fork anyway…"
        } else {
            "Fork instead…"
        };
        out.push(button("recovery-fork", label, false, hint));
    }
    if matches!(
        state,
        RecoveryState::Unreachable(_)
            | RecoveryState::TerminalUncertain(_)
            | RecoveryState::AgentUncertain(_, _)
    ) {
        out.push(button("recovery-details", "Details", false, hint));
    }
    out.push(button("recovery-close", "Close tab", false, hint));
    out
}

/// The active tab's overlay, exit banner and LIMITED banner fields (see
/// the module doc); `hint` is the keymap hint of a command.
pub fn tab_chrome(c: &Chrome, hint: &dyn Fn(Command) -> String) -> Json {
    let state = &c.recovery.state;
    let connection = c.connection.as_ref();
    let remote = connection.is_some() || c.remote_machine;
    let conceals = connection.is_some_and(|(state, transport, _, _)| {
        *transport == "ssh" && matches!(*state, "disconnected" | "reconnecting")
    });
    let failed = c.exited && c.exit_code.is_some_and(|code| code != 0);
    let session = c.observed.map(|(_, s)| s);
    let (overlay, title, detail, buttons, note, reason) =
        if let Some((title, detail)) = state.texts() {
            let note = if matches!(state, RecoveryState::AgentUncertain(_, _)) {
                "Forking keeps the uncertain execution unchanged."
            } else {
                ""
            };
            let reason = if c.recovery.details && !state.reason().is_empty() {
                clean(state.reason())
            } else {
                String::new()
            };
            (
                "recovery",
                title.to_string(),
                detail.to_string(),
                recovery_buttons(state, hint),
                note,
                reason,
            )
        } else if conceals || (remote && c.has_session && failed) {
            let reconnecting = connection.is_some_and(|(state, _, _, _)| *state == "reconnecting");
            let (overlay, title) = if reconnecting {
                ("reconnecting", "Reconnecting…")
            } else {
                ("disconnected", "Connection interrupted")
            };
            (
                overlay,
                title.to_string(),
                String::new(),
                Vec::new(),
                "",
                String::new(),
            )
        } else if let Some(s) = session.filter(|s| !c.exited && s.state == "paused") {
            let status = if s.status_text.is_empty() {
                "Opens where it left off".to_string()
            } else {
                s.status_text.clone()
            };
            let label = if c.resuming { "Resuming…" } else { "Unpause" };
            (
                "paused",
                "Paused to free memory".to_string(),
                status,
                vec![button("unpause-button", label, true, hint)],
                "Any key in this tab resumes it too",
                String::new(),
            )
        } else {
            (
                "",
                String::new(),
                String::new(),
                Vec::new(),
                "",
                String::new(),
            )
        };
    let exit_banner = if c.exited && *state == RecoveryState::None && !remote {
        match c.exit_code {
            Some(code) if code != 0 => format!("Connection ended (exit status {code})."),
            _ => "Connection ended.".to_string(),
        }
    } else {
        String::new()
    };
    let limited = session.filter(|s| !c.exited && session_state(s) == "LIMITED");
    json!({
        "overlay": overlay,
        "overlayTitle": title,
        "overlayDetail": detail,
        "overlayButtons": buttons,
        "overlayNote": note,
        "overlayReason": reason,
        "exitBanner": exit_banner,
        "limited": limited.is_some(),
        "limitedStatus": limited.map(session_summary).unwrap_or_default(),
        "limitedProvider": limited.map(|s| provider_label(&s.provider).to_string()).unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests;
