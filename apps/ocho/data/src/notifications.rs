//! macOS notifications (notifications.rs): a completed turn (Running →
//! Idle) and input required (→ Blocked), one per session, removed when the
//! session runs again. The module posts them (`notify` jobs); this keeps
//! the phases and decides. Without timers or a pane read, a completed turn
//! posts at once (the desktop waits 400 ms for a generated title) and a
//! blocked one says its status text (the desktop reads the prompt's
//! question and choices from the pane).

use crate::settings::DesktopSettings;
use crate::types::{Machine, Session};
use std::collections::HashMap;

/// What a session's state means for notifications.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// working, running, starting.
    Running,
    /// idle.
    Idle,
    /// blocked, awaiting input, awaiting approval.
    Blocked,
    /// closed, exited.
    Gone,
    /// Anything else.
    Unknown,
}

/// The phase a state names.
pub fn phase(state: &str) -> Phase {
    match state {
        "working" | "running" | "starting" => Phase::Running,
        "idle" => Phase::Idle,
        "blocked" | "awaiting input" | "awaiting approval" => Phase::Blocked,
        "closed" | "exited" => Phase::Gone,
        _ => Phase::Unknown,
    }
}

fn title_rank(source: &str) -> u8 {
    match source {
        "label" => 5,
        "native-custom" => 4,
        "generated" => 3,
        "native" => 2,
        "prompt" => 1,
        _ => 0,
    }
}

/// A session as last observed.
#[derive(Clone, Debug)]
pub struct KnownSession {
    /// The phase.
    pub phase: Phase,
    /// The machine's id.
    pub machine_id: String,
    /// The machine's name.
    pub machine_name: String,
    /// The session's id.
    pub session_id: String,
    /// The best title seen.
    pub title: String,
    title_source: String,
    /// The provider.
    pub provider: String,
    /// The status text.
    pub status_text: String,
    /// The last message.
    pub last_message: String,
    last_turn_interrupted: bool,
    observed_at: i64,
    /// A notification is up for it.
    pub posted: bool,
}

impl KnownSession {
    fn from(machine: &Machine, session: &Session) -> Self {
        KnownSession {
            phase: phase(&session.state),
            machine_id: machine.id.clone(),
            machine_name: machine.name.clone(),
            session_id: session.id.clone(),
            title: session.title.clone(),
            title_source: session.title_source.clone(),
            provider: session.provider.clone(),
            status_text: session.status_text.clone(),
            last_message: session.last_message.clone(),
            last_turn_interrupted: session.last_turn_interrupted,
            observed_at: session.status_observed_at,
            posted: false,
        }
    }

    fn update(&mut self, machine: &Machine, session: &Session) {
        self.phase = phase(&session.state);
        self.machine_id = machine.id.clone();
        self.machine_name = machine.name.clone();
        self.session_id = session.id.clone();
        if !session.title.is_empty()
            && title_rank(&session.title_source) >= title_rank(&self.title_source)
        {
            self.title = session.title.clone();
            self.title_source = session.title_source.clone();
        }
        if !session.provider.is_empty() {
            self.provider = session.provider.clone();
        }
        self.status_text = session.status_text.clone();
        if !session.last_message.is_empty() {
            self.last_message = session.last_message.clone();
        }
        self.observed_at = session.status_observed_at;
        self.last_turn_interrupted = session.last_turn_interrupted;
    }

    /// The notification's thread id: `machine:session`.
    pub fn thread(&self) -> String {
        format!("{}:{}", self.machine_id, self.session_id)
    }

    /// The title: the session's, else "{Provider} session", else "Ocho
    /// session" (notifications.rs `display_title`).
    pub fn display_title(&self) -> String {
        if !self.title.trim().is_empty() {
            self.title.clone()
        } else if !self.provider.is_empty() {
            format!("{} session", crate::session::provider_label(&self.provider))
        } else {
            "Ocho session".into()
        }
    }

    /// The note for this session as notifications.rs `post` writes it: the
    /// title and body as plain text, the body clipped to 320 characters and
    /// "Ready for your next message" when nothing is left.
    fn post(&self, kind: &'static str, body: &str) -> Note {
        let body = clipped(&notification_text(body), 320);
        Note::Post {
            kind,
            title: notification_text(&self.display_title()),
            body: if body.is_empty() {
                "Ready for your next message".into()
            } else {
                body
            },
            thread: self.thread(),
        }
    }
}

/// A notification to post: a `notify` job's arguments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Note {
    /// Post: kind (`completed` / `blocked`), title, body, thread.
    Post {
        /// `completed` or `blocked`.
        kind: &'static str,
        /// The title.
        title: String,
        /// The body.
        body: String,
        /// The thread id.
        thread: String,
    },
    /// Remove the thread's notification.
    Remove {
        /// The thread id.
        thread: String,
    },
}

/// The sessions watched.
#[derive(Debug, Default)]
pub struct Notifications {
    sessions: HashMap<(String, String), KnownSession>,
}

impl Notifications {
    /// A session as an event showed it; what to post, if anything.
    pub fn observe(
        &mut self,
        machine: &Machine,
        session: &Session,
        settings: &DesktopSettings,
    ) -> Vec<Note> {
        if session.id.is_empty()
            || session.historical
            || session.pid == 0
            || (!session.managed && !session.tracked)
        {
            return Vec::new();
        }
        let key = (machine.id.clone(), session.id.clone());
        let next = phase(&session.state);
        let Some(known) = self.sessions.get_mut(&key) else {
            self.sessions
                .insert(key, KnownSession::from(machine, session));
            return Vec::new();
        };
        if session.status_observed_at > 0
            && known.observed_at > 0
            && session.status_observed_at < known.observed_at
        {
            return Vec::new();
        }
        let previous = known.phase;
        known.update(machine, session);
        let mut out = Vec::new();
        if next == Phase::Running {
            if known.posted {
                known.posted = false;
                out.push(Note::Remove {
                    thread: known.thread(),
                });
            }
            return out;
        }
        match (previous, next) {
            (Phase::Running, Phase::Idle) if settings.notify_turn_complete() => {
                if known.last_turn_interrupted {
                    return out;
                }
                if known.posted {
                    out.push(Note::Remove {
                        thread: known.thread(),
                    });
                }
                known.posted = true;
                out.push(known.post("completed", &known.last_message));
            }
            (old, Phase::Blocked) if old != Phase::Blocked && settings.notify_input_required() => {
                if known.posted {
                    out.push(Note::Remove {
                        thread: known.thread(),
                    });
                }
                known.posted = true;
                let question = if known.status_text.is_empty() {
                    "The session is waiting for you"
                } else {
                    known.status_text.as_str()
                };
                out.push(known.post("blocked", question));
            }
            _ => {}
        }
        out
    }
}

/// `text` trimmed, at most `limit` characters: a longer one keeps
/// `limit - 1` and ends in an ellipsis.
pub fn clipped(text: &str, limit: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let mut value: String = text.chars().take(limit.saturating_sub(1)).collect();
    value.push('…');
    value
}

/// Markdown reduced to its words, whitespace collapsed, for a
/// notification's title and body.
pub fn notification_text(markdown: &str) -> String {
    crate::markdown_doc::plain_text(&crate::markdown_doc::parse(markdown))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(state: &str, at: i64) -> Session {
        Session {
            id: "s".into(),
            title: "Fix the build".into(),
            provider: "claude".into(),
            state: state.into(),
            pid: 42,
            managed: true,
            status_observed_at: at,
            last_message: "Done: **all green**".into(),
            ..Default::default()
        }
    }

    #[test]
    fn a_completed_turn_posts_once_and_running_removes_it() {
        let m = Machine {
            id: "m".into(),
            name: "mac".into(),
            ..Default::default()
        };
        let settings = DesktopSettings::default();
        let mut n = Notifications::default();
        assert!(n.observe(&m, &session("working", 1), &settings).is_empty());
        let notes = n.observe(&m, &session("idle", 2), &settings);
        assert_eq!(
            notes,
            vec![Note::Post {
                kind: "completed",
                title: "Fix the build".into(),
                body: "Done: all green".into(),
                thread: "m:s".into()
            }]
        );
        assert!(n.observe(&m, &session("idle", 3), &settings).is_empty());
        assert_eq!(
            n.observe(&m, &session("working", 4), &settings),
            vec![Note::Remove {
                thread: "m:s".into()
            }]
        );
        let notes = n.observe(&m, &session("blocked", 5), &settings);
        assert!(matches!(
            &notes[0],
            Note::Post {
                kind: "blocked",
                ..
            }
        ));
        // An older observation is ignored.
        assert!(n.observe(&m, &session("idle", 1), &settings).is_empty());
    }

    #[test]
    fn blocked_and_untitled_sessions_read_plainly() {
        let m = Machine {
            id: "m".into(),
            ..Default::default()
        };
        let settings = DesktopSettings::default();
        let mut n = Notifications::default();
        let mut s = session("working", 1);
        s.title = "  ".into();
        s.provider = "grok".into();
        s.last_message = "```\n```".into();
        n.observe(&m, &s, &settings);
        s.state = "idle".into();
        s.status_observed_at = 2;
        assert_eq!(
            n.observe(&m, &s, &settings),
            vec![Note::Post {
                kind: "completed",
                title: "Grok session".into(),
                body: "Ready for your next message".into(),
                thread: "m:s".into()
            }]
        );
        s.state = "awaiting approval".into();
        s.status_observed_at = 3;
        let notes = n.observe(&m, &s, &settings);
        assert_eq!(
            notes[0],
            Note::Remove {
                thread: "m:s".into()
            }
        );
        assert!(matches!(
            &notes[1],
            Note::Post { kind: "blocked", body, .. } if body == "The session is waiting for you"
        ));
    }

    #[test]
    fn notification_copy_is_plain_text() {
        assert_eq!(
            notification_text("## Done\n**Fixed** [the bug](https://example.com) in `app.rs`."),
            "Done Fixed the bug in app.rs."
        );
        assert_eq!(
            notification_text("- First item\n- Second *item*\n\n```rust\nlet ok = true;\n```"),
            "First item Second item let ok = true;"
        );
        assert_eq!(clipped("  short  ", 320), "short");
        assert_eq!(clipped("abcdef", 4), "abc…");
        assert_eq!(clipped("abcd", 4), "abcd");
    }

    #[test]
    fn higher_priority_titles_cannot_be_replaced_by_native_fallbacks() {
        let machine = Machine {
            id: "machine".into(),
            ..Default::default()
        };
        let mk = |title: &str, source: &str| Session {
            id: "session".into(),
            title: title.into(),
            title_source: source.into(),
            ..Default::default()
        };
        let mut known = KnownSession::from(&machine, &mk("Secure Session Sync", "generated"));
        known.update(&machine, &mk("old first prompt", "native"));
        assert_eq!(known.title, "Secure Session Sync");
        known.update(&machine, &mk("My label", "label"));
        assert_eq!(known.title, "My label");
        // An unknown source ranks with none; a first prompt above both.
        known.title_source = String::new();
        known.update(&machine, &mk("odd", "something"));
        assert_eq!(known.title, "odd");
        known.update(&machine, &mk("first prompt", "prompt"));
        assert_eq!(known.title, "first prompt");
        known.update(&machine, &mk("odd again", "something"));
        assert_eq!(known.title, "first prompt");
    }
}
