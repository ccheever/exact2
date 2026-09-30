//! Every user action is a `Command`. Keys, buttons, menus, and the command
//! palette all funnel through the same list, so nothing is keyboard-only. The
//! keys themselves live in `keymap`; this module only names the commands and
//! groups them for the palette.
//!
//! Ported from `fleet/desktop/src/palette.rs` (origin/main e577272). Added
//! for the contract: [`Command::id`] / [`Command::from_id`], a stable text
//! name for each command so a press id can carry one, and [`rank`], the
//! palette's filter-and-sort step lifted out of `Workspace::palette_items`.

use serde::Serialize;

use crate::model::Page;

/// One user action; the label and the key come from the tables here and in
/// `keymap`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[allow(missing_docs)]
pub enum Command {
    Palette,
    Help,
    Refresh,
    NextPage,
    PrevPage,
    Page(usize),
    Down,
    Up,
    First,
    Last,
    HalfDown,
    HalfUp,
    Search,
    PullRequests,
    Conversations,
    IndexConversations,
    ClearFilter,
    Open,
    /// Launch a session, from anywhere.
    New,
    QuickLaunch,
    AddAccount,
    AddProfile,
    NewWindow,
    CloseWindow,
    Edit,
    Message,
    ViewReadOnly,
    Track,
    AddMachine,
    ConnectFly,
    UpdateMachine,
    UpdateCodex,
    UpdateClaude,
    UpdateOpenCode,
    ToggleOthers,
    ToggleHistory,
    ToggleNonRunning,
    Pin,
    Archive,
    Interrupt,
    Pause,
    UnpauseTab,
    Handoff,
    /// Continue a Codex session under another account. Never offered for
    /// Claude, whose terms do not allow moving past a usage limit this way.
    SwitchAccount,
    AttachIMessage,
    Resume,
    Move,
    Stop,
    Delete,
    Shell,
    LaunchProfile(usize),
    NextTab,
    PrevTab,
    SelectTab(usize),
    /// The rail's next / previous row while it has focus.
    RailDown,
    RailUp,
    NewFolder,
    CloseTab,
    ReopenClosedTab,
    ClearScrollback,
    ReconnectTab,
    RecoveryContinue,
    RecoveryRestore,
    RecoveryFork,
    RecoveryReplace,
    RecoveryDetails,
    ToggleTerminalPanel,
    FocusTerminalPanel,
    RefreshSessionTheme,
    SwitchClaudeWorkerTerminal,
    ToggleTranscript,
    ToggleRequests,
    EnterNav,
    ExitNav,
    Themes,
    PairPhone,
    Settings,
    Quit,
    CheckForUpdates,
    UpdateDesktop,
    Undo,
}

/// The text id of every command without a number, in declaration order.
/// `Page(n)`, `SelectTab(n)` and `LaunchProfile(n)` are `page-n`, `tab-n`
/// and `profile-n`.
const IDS: &[(Command, &str)] = &[
    (Command::Palette, "palette"),
    (Command::Help, "help"),
    (Command::Refresh, "refresh"),
    (Command::NextPage, "next-page"),
    (Command::PrevPage, "prev-page"),
    (Command::Down, "down"),
    (Command::Up, "up"),
    (Command::First, "first"),
    (Command::Last, "last"),
    (Command::HalfDown, "half-down"),
    (Command::HalfUp, "half-up"),
    (Command::Search, "search"),
    (Command::PullRequests, "pull-requests"),
    (Command::Conversations, "conversations"),
    (Command::IndexConversations, "index-conversations"),
    (Command::ClearFilter, "clear-filter"),
    (Command::Open, "open"),
    (Command::New, "launch"),
    (Command::QuickLaunch, "quick-launch"),
    (Command::AddAccount, "add-account"),
    (Command::AddProfile, "add-profile"),
    (Command::NewWindow, "new-window"),
    (Command::CloseWindow, "close-window"),
    (Command::Edit, "edit"),
    (Command::Message, "message"),
    (Command::ViewReadOnly, "view-read-only"),
    (Command::Track, "track"),
    (Command::AddMachine, "add-machine"),
    (Command::ConnectFly, "connect-fly"),
    (Command::UpdateMachine, "update-machine"),
    (Command::UpdateCodex, "update-codex"),
    (Command::UpdateClaude, "update-claude"),
    (Command::UpdateOpenCode, "update-opencode"),
    (Command::ToggleOthers, "toggle-others"),
    (Command::ToggleHistory, "toggle-history"),
    (Command::ToggleNonRunning, "toggle-non-running"),
    (Command::Pin, "pin"),
    (Command::Archive, "archive"),
    (Command::Interrupt, "interrupt"),
    (Command::Pause, "pause"),
    (Command::UnpauseTab, "unpause-tab"),
    (Command::Handoff, "handoff"),
    (Command::SwitchAccount, "switch-account"),
    (Command::AttachIMessage, "attach-imessage"),
    (Command::Resume, "resume"),
    (Command::Move, "move"),
    (Command::Stop, "stop"),
    (Command::Delete, "delete"),
    (Command::Shell, "shell"),
    (Command::NextTab, "next-tab"),
    (Command::PrevTab, "prev-tab"),
    (Command::RailDown, "rail-down"),
    (Command::RailUp, "rail-up"),
    (Command::NewFolder, "new-folder"),
    (Command::CloseTab, "close-tab"),
    (Command::ReopenClosedTab, "reopen-closed-tab"),
    (Command::ClearScrollback, "clear-scrollback"),
    (Command::ReconnectTab, "reconnect-tab"),
    (Command::RecoveryContinue, "recovery-continue"),
    (Command::RecoveryRestore, "recovery-restore"),
    (Command::RecoveryFork, "recovery-fork"),
    (Command::RecoveryReplace, "recovery-replace"),
    (Command::RecoveryDetails, "recovery-details"),
    (Command::ToggleTerminalPanel, "toggle-terminal-panel"),
    (Command::FocusTerminalPanel, "focus-terminal-panel"),
    (Command::RefreshSessionTheme, "refresh-session-theme"),
    (
        Command::SwitchClaudeWorkerTerminal,
        "switch-claude-worker-terminal",
    ),
    (Command::ToggleTranscript, "toggle-transcript"),
    (Command::ToggleRequests, "toggle-requests"),
    (Command::EnterNav, "enter-nav"),
    (Command::ExitNav, "exit-nav"),
    (Command::Themes, "themes"),
    (Command::PairPhone, "pair-phone"),
    (Command::Settings, "settings"),
    (Command::Quit, "quit"),
    (Command::CheckForUpdates, "check-for-updates"),
    (Command::UpdateDesktop, "update-desktop"),
    (Command::Undo, "undo"),
];

impl Command {
    /// A stable text id for press ids and the contract: lower-case words
    /// joined by `-` (`launch`, `quick-launch`), with a number suffix for the
    /// numbered commands (`page-2`, `tab-1`, `profile-3`).
    pub fn id(&self) -> String {
        match self {
            Command::Page(n) => format!("page-{n}"),
            Command::SelectTab(n) => format!("tab-{n}"),
            Command::LaunchProfile(n) => format!("profile-{n}"),
            other => IDS
                .iter()
                .find(|(c, _)| c == other)
                .map(|(_, id)| (*id).to_string())
                .unwrap_or_default(),
        }
    }

    /// The command an [`id`](Command::id) names, if any.
    pub fn from_id(id: &str) -> Option<Command> {
        let numbered = |prefix: &str| -> Option<usize> {
            id.strip_prefix(prefix).and_then(|n| n.parse().ok())
        };
        if let Some(n) = numbered("page-") {
            return Some(Command::Page(n));
        }
        if let Some(n) = numbered("tab-") {
            return Some(Command::SelectTab(n));
        }
        if let Some(n) = numbered("profile-") {
            return Some(Command::LaunchProfile(n));
        }
        IDS.iter().find(|(_, i)| *i == id).map(|(c, _)| *c)
    }

    /// Every command without a number, in declaration order.
    pub fn unnumbered() -> impl Iterator<Item = Command> {
        IDS.iter().map(|(c, _)| *c)
    }
}

/// A command and the label the palette, menus, tooltips and help show for it.
#[derive(Clone, Copy, Debug)]
pub struct CommandInfo {
    /// The command.
    pub command: Command,
    /// Its label.
    pub label: &'static str,
}

const fn info(command: Command, label: &'static str) -> CommandInfo {
    CommandInfo { command, label }
}

/// Commands offered everywhere; the palette lists them last.
pub const GLOBAL: &[CommandInfo] = &[
    info(Command::ConnectFly, "Connect Fly.io…"),
    info(Command::CheckForUpdates, "Check for Ocho updates"),
    info(Command::Undo, "Undo last action"),
    info(Command::ReopenClosedTab, "Reopen closed tab"),
    info(Command::ClearScrollback, "Clear terminal scrollback"),
    info(Command::Palette, "Command palette"),
    info(Command::ToggleRequests, "Toggle Requests sidebar"),
    info(Command::Help, "Keyboard help"),
    info(Command::Themes, "Change theme…"),
    info(Command::PairPhone, "Pair phone…"),
    info(Command::Settings, "Settings…"),
    info(Command::Refresh, "Refresh Ocho"),
    info(Command::New, "Launch session"),
    info(Command::QuickLaunch, "Quick launch session"),
    info(Command::NewWindow, "New window"),
    info(Command::CloseWindow, "Close window (agents keep running)"),
    info(Command::Search, "Search sessions"),
    info(Command::PullRequests, "Open session for PR…"),
    info(Command::Conversations, "Find session by topic…"),
    info(
        Command::IndexConversations,
        "Index conversations for topic search",
    ),
    info(Command::Page(0), "Go to Machines"),
    info(Command::Page(1), "Go to Sessions"),
    info(Command::Page(2), "Go to Accounts"),
    info(Command::Page(3), "Go to Profiles"),
    info(Command::NextPage, "Next page"),
    info(Command::PrevPage, "Previous page"),
    info(Command::NextTab, "Next tab"),
    info(Command::PrevTab, "Previous tab"),
    info(Command::SelectTab(0), "Show Ocho manager"),
    info(
        Command::CloseTab,
        "Close tab / manager window (agents keep running)",
    ),
    info(Command::ReconnectTab, "Reconnect terminal tab"),
    info(Command::NewFolder, "New sidebar folder…"),
    info(Command::EnterNav, "Focus the rail (NAV)"),
    info(Command::Quit, "Quit Ocho"),
];

/// Actions for any terminal tab: a shell panel docked under the tab, on the
/// same machine and in the same directory as the agent.
pub const TAB_TERMINAL: &[CommandInfo] = &[
    info(
        Command::ToggleTerminalPanel,
        "Toggle terminal panel (shell in the agent's directory)",
    ),
    info(
        Command::FocusTerminalPanel,
        "Switch focus between agent and terminal panel",
    ),
];

/// Actions on a managed Codex session's terminal tab.
pub const TERMINAL: &[CommandInfo] = &[
    info(Command::ToggleTranscript, "Toggle transcript / terminal"),
    info(
        Command::RefreshSessionTheme,
        "Refresh current session theme",
    ),
];

/// Actions on a Claude Remote Control session's terminal tab.
pub const CLAUDE_REMOTE: &[CommandInfo] = &[
    info(
        Command::RefreshSessionTheme,
        "Refresh current session theme",
    ),
    info(
        Command::SwitchClaudeWorkerTerminal,
        "Switch Claude Remote Control to worker terminal",
    ),
];

/// The Sessions page's commands, on the selected row.
pub const SESSIONS: &[CommandInfo] = &[
    info(Command::Open, "Attach to session"),
    info(Command::ViewReadOnly, "Attach read-only"),
    info(Command::Message, "Show last assistant message"),
    info(Command::Edit, "Label session"),
    info(Command::Track, "Pick up (track) session"),
    info(
        Command::ToggleOthers,
        "Show / hide untracked and archived sessions",
    ),
    info(Command::ToggleHistory, "Toggle history"),
    info(
        Command::ToggleNonRunning,
        "Hide / show non-running sessions",
    ),
    info(Command::Pin, "Pin / unpin"),
    info(Command::Archive, "Archive / unarchive session"),
    info(Command::Interrupt, "Interrupt current turn…"),
    info(Command::Pause, "Pause / unpause session (frees memory)"),
    info(
        Command::Handoff,
        "Hand off limited session to another account…",
    ),
    info(
        Command::SwitchAccount,
        "Resume session with a different account…",
    ),
    info(Command::Resume, "Resume native conversation"),
    info(Command::Move, "Move session to another machine…"),
    info(Command::Stop, "Stop session…"),
    info(Command::Delete, "Stop and remove session…"),
    info(Command::ClearFilter, "Clear search filter"),
];

/// Actions that make sense while attached to a session's terminal, where the
/// row-based keys of the Sessions page are not reachable.
pub const TAB_SESSION: &[CommandInfo] = &[
    info(Command::AttachIMessage, "Attach iMessage here"),
    info(
        Command::Handoff,
        "Hand off limited session to another account…",
    ),
    info(
        Command::SwitchAccount,
        "Resume session with a different account…",
    ),
    info(Command::Move, "Move session to another machine…"),
    info(Command::Interrupt, "Interrupt current turn…"),
    info(Command::UnpauseTab, "Unpause this session"),
];

/// The Machines page's commands, on the selected row.
pub const MACHINES: &[CommandInfo] = &[
    info(Command::Open, "Show this machine's sessions"),
    info(Command::Shell, "Open shell on machine"),
    info(Command::AddMachine, "Add machine…"),
    info(
        Command::Edit,
        "Edit machine (name, notes, tags, default permissions)…",
    ),
    info(Command::UpdateMachine, "Update Ocho helper on machine"),
    info(Command::UpdateCodex, "Update Codex on machine"),
    info(Command::UpdateClaude, "Update Claude on machine"),
    info(Command::UpdateOpenCode, "Update OpenCode on machine"),
    info(Command::Delete, "Remove machine from Ocho…"),
];

/// The Accounts page's commands.
pub const ACCOUNTS: &[CommandInfo] = &[
    info(Command::AddAccount, "Add account…"),
    info(Command::Open, "Open account (sign in or shell)"),
    info(Command::Edit, "Reconnect account"),
];

/// The Profiles page's commands.
pub const PROFILES: &[CommandInfo] = &[
    info(Command::Open, "Launch profile"),
    info(Command::AddProfile, "Add profile…"),
    info(Command::Edit, "Edit profile…"),
    info(Command::Delete, "Remove profile…"),
];

/// Row movement on every manager page.
pub const MOTION: &[CommandInfo] = &[
    info(Command::Down, "Next row"),
    info(Command::Up, "Previous row"),
    info(Command::First, "First row"),
    info(Command::Last, "Last row"),
    info(Command::HalfDown, "Half page down"),
    info(Command::HalfUp, "Half page up"),
];

/// Commands that have keys or buttons but no palette entry of their own.
const UNLISTED: &[CommandInfo] = &[
    info(Command::RailDown, "Next rail row"),
    info(Command::RailUp, "Previous rail row"),
    info(Command::ExitNav, "Use the selected row"),
    info(Command::UpdateDesktop, "Install the Ocho update"),
    info(Command::SelectTab(1), "Terminal tab 1"),
    info(Command::SelectTab(2), "Terminal tab 2"),
    info(Command::SelectTab(3), "Terminal tab 3"),
    info(Command::SelectTab(4), "Terminal tab 4"),
    info(Command::SelectTab(5), "Terminal tab 5"),
    info(Command::SelectTab(6), "Terminal tab 6"),
    info(Command::SelectTab(7), "Terminal tab 7"),
    info(Command::SelectTab(8), "Terminal tab 8"),
    info(Command::SelectTab(9), "Terminal tab 9"),
];

/// The page's own palette commands (`Page::commands` in the GPUI workspace).
pub fn page_commands(page: Page) -> &'static [CommandInfo] {
    match page {
        Page::Machines => MACHINES,
        Page::Sessions => SESSIONS,
        Page::Accounts => ACCOUNTS,
        Page::Profiles => PROFILES,
    }
}

/// The label a command shows in menus, tooltips and help.
pub fn label(command: Command) -> &'static str {
    [
        GLOBAL,
        TAB_TERMINAL,
        TERMINAL,
        CLAUDE_REMOTE,
        SESSIONS,
        TAB_SESSION,
        MACHINES,
        ACCOUNTS,
        PROFILES,
        MOTION,
        UNLISTED,
    ]
    .into_iter()
    .flatten()
    .find(|item| item.command == command)
    .map(|item| item.label)
    .unwrap_or("")
}

/// Subsequence match with a small bonus for word starts; lower is better.
pub fn fuzzy_score(query: &str, text: &str) -> Option<i32> {
    let q: Vec<char> = query
        .chars()
        .flat_map(|c| c.to_lowercase())
        .filter(|c| !c.is_whitespace())
        .collect();
    if q.is_empty() {
        return Some(0);
    }
    let t: Vec<char> = text.chars().flat_map(|c| c.to_lowercase()).collect();
    let mut score = 0;
    let mut ti = 0;
    let mut last: Option<usize> = None;
    for qc in q {
        let mut found = None;
        while ti < t.len() {
            if t[ti] == qc {
                found = Some(ti);
                break;
            }
            ti += 1;
        }
        let at = found?;
        let word_start = at == 0 || !t[at - 1].is_alphanumeric();
        score += match last {
            Some(prev) if at == prev + 1 => 0,
            _ if word_start => 1,
            Some(prev) => (at - prev) as i32 + 2,
            None => at as i32 + 2,
        };
        last = Some(at);
        ti = at + 1;
    }
    Some(score)
}

/// The palette's filter and order, as `Workspace::palette_items` ends: the
/// query is trimmed; empty, every item stays in its place with score 0;
/// otherwise the items the query matches, best (lowest) score first, ties in
/// their original order.
pub fn rank<'a>(items: &[&'a CommandInfo], query: &str) -> Vec<(&'a CommandInfo, i32)> {
    let query = query.trim();
    let mut scored: Vec<(&'a CommandInfo, i32)> = items
        .iter()
        .filter_map(|c| {
            if query.is_empty() {
                Some((*c, 0))
            } else {
                fuzzy_score(query, c.label).map(|s| (*c, s))
            }
        })
        .collect();
    if !query.is_empty() {
        scored.sort_by_key(|(_, s)| *s);
    }
    scored
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_prefers_contiguous_and_word_starts() {
        assert_eq!(fuzzy_score("", "anything"), Some(0));
        assert!(fuzzy_score("xyz", "attach").is_none());
        let exact = fuzzy_score("attach", "Attach to session").unwrap();
        let spread = fuzzy_score("ats", "Attach to session").unwrap();
        assert!(exact <= spread);
        assert!(
            fuzzy_score("ls", "Launch session").unwrap()
                < fuzzy_score("ls", "Toggle history").unwrap_or(i32::MAX)
        );
    }

    #[test]
    fn machine_removal_is_available_from_the_command_palette() {
        let remove = MACHINES
            .iter()
            .find(|item| item.command == Command::Delete)
            .unwrap();
        assert_eq!(remove.label, "Remove machine from Ocho…");
    }

    #[test]
    fn machine_provider_updates_are_available_from_the_command_palette() {
        for command in [
            Command::UpdateCodex,
            Command::UpdateClaude,
            Command::UpdateOpenCode,
        ] {
            assert!(MACHINES.iter().any(|item| item.command == command));
        }
    }

    #[test]
    fn terminal_palette_offers_an_explicit_theme_refresh() {
        assert_eq!(
            label(Command::RefreshSessionTheme),
            "Refresh current session theme"
        );
        assert!(TERMINAL
            .iter()
            .any(|item| item.command == Command::ToggleTranscript));
    }

    #[test]
    fn claude_remote_palette_offers_worker_terminal_and_theme_refresh() {
        assert!(CLAUDE_REMOTE
            .iter()
            .any(|item| item.command == Command::SwitchClaudeWorkerTerminal));
        assert!(CLAUDE_REMOTE
            .iter()
            .any(|item| item.command == Command::RefreshSessionTheme));
    }

    #[test]
    fn terminal_tabs_offer_a_docked_shell_panel() {
        assert_eq!(
            label(Command::ToggleTerminalPanel),
            "Toggle terminal panel (shell in the agent's directory)"
        );
        assert!(TAB_TERMINAL
            .iter()
            .any(|item| item.command == Command::FocusTerminalPanel));
    }

    #[test]
    fn every_page_can_add_its_own_kind_of_thing() {
        assert!(ACCOUNTS.iter().any(|i| i.command == Command::AddAccount));
        assert!(PROFILES.iter().any(|i| i.command == Command::AddProfile));
        assert!(MACHINES.iter().any(|i| i.command == Command::AddMachine));
        // Launching a session is global and never means "add account".
        assert_eq!(label(Command::New), "Launch session");
    }

    #[test]
    fn interrupt_asks_first_and_is_reachable_from_a_tab() {
        assert!(label(Command::Interrupt).ends_with('…'));
        assert!(TAB_SESSION.iter().any(|i| i.command == Command::Interrupt));
    }

    #[test]
    fn unlisted_commands_still_have_labels() {
        assert_eq!(label(Command::RailDown), "Next rail row");
        assert_eq!(label(Command::SelectTab(3)), "Terminal tab 3");
        assert_eq!(label(Command::Undo), "Undo last action");
    }

    #[test]
    fn ids_round_trip() {
        for command in Command::unnumbered() {
            let id = command.id();
            assert!(!id.is_empty(), "{command:?} has no id");
            assert!(
                id.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
                "{id} is not a plain id"
            );
            assert_eq!(Command::from_id(&id), Some(command), "{id}");
        }
        for n in 0..10 {
            for command in [
                Command::Page(n),
                Command::SelectTab(n),
                Command::LaunchProfile(n),
            ] {
                assert_eq!(Command::from_id(&command.id()), Some(command));
            }
        }
        assert_eq!(Command::New.id(), "launch");
        assert_eq!(Command::Page(2).id(), "page-2");
        assert_eq!(Command::LaunchProfile(3).id(), "profile-3");
        assert_eq!(Command::SelectTab(1).id(), "tab-1");
        assert_eq!(Command::from_id("tab-x"), None);
        assert_eq!(Command::from_id(""), None);
        assert_eq!(Command::from_id("nope"), None);
    }

    #[test]
    fn ids_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for (_, id) in IDS {
            assert!(seen.insert(*id), "duplicate id {id}");
        }
        assert_eq!(IDS.len(), Command::unnumbered().count());
    }

    #[test]
    fn rank_keeps_order_without_a_query_and_sorts_with_one() {
        let items: Vec<&CommandInfo> = GLOBAL.iter().collect();
        let all = rank(&items, "  ");
        assert_eq!(all.len(), GLOBAL.len());
        assert!(all.iter().all(|(_, s)| *s == 0));
        assert_eq!(all[0].0.command, Command::ConnectFly);
        let found = rank(&items, "launch");
        assert!(!found.is_empty());
        assert_eq!(found[0].0.command, Command::New);
        assert!(found.windows(2).all(|w| w[0].1 <= w[1].1));
        assert!(rank(&items, "zzzzzz").is_empty());
    }

    #[test]
    fn pages_have_their_own_command_lists() {
        let commands = |items: &[CommandInfo]| items.iter().map(|i| i.command).collect::<Vec<_>>();
        assert_eq!(commands(page_commands(Page::Sessions)), commands(SESSIONS));
        assert_eq!(commands(page_commands(Page::Machines)), commands(MACHINES));
        assert_eq!(commands(page_commands(Page::Accounts)), commands(ACCOUNTS));
        assert_eq!(commands(page_commands(Page::Profiles)), commands(PROFILES));
    }
}
