//! macOS notifications (notifications.rs): a completed turn (Running →
//! Idle) and input required (→ Blocked), one per session, removed when the
//! session runs again. The module posts them (`notify` jobs); this keeps
//! the phases and decides.

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
        "" => 0,
        _ => 1,
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

    /// The title: the session's, else "{Provider} session", else "Ocho session".
    pub fn display_title(&self) -> String {
        let title = crate::session::clean(&self.title);
        if !title.is_empty() {
            return title;
        }
        if !self.provider.is_empty() {
            return format!("{} session", crate::session::provider_label(&self.provider));
        }
        "Ocho session".into()
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
                let body = if known.last_message.trim().is_empty() {
                    "Ready for your next message".to_string()
                } else {
                    clipped(&notification_text(&known.last_message), 320)
                };
                out.push(Note::Post {
                    kind: "completed",
                    title: known.display_title(),
                    body,
                    thread: known.thread(),
                });
            }
            (old, Phase::Blocked) if old != Phase::Blocked && settings.notify_input_required() => {
                if known.posted {
                    out.push(Note::Remove {
                        thread: known.thread(),
                    });
                }
                known.posted = true;
                let body = if known.status_text.trim().is_empty() {
                    "The session is waiting for you".to_string()
                } else {
                    clipped(&known.status_text, 320)
                };
                out.push(Note::Post {
                    kind: "blocked",
                    title: known.display_title(),
                    body,
                    thread: known.thread(),
                });
            }
            _ => {}
        }
        out
    }
}

/// The first `limit` characters, with an ellipsis when cut.
pub fn clipped(text: &str, limit: usize) -> String {
    let mut out: String = text.chars().take(limit).collect();
    if text.chars().count() > limit {
        out.push('…');
    }
    out
}

/// Markdown reduced to words for a notification's body.
pub fn notification_text(markdown: &str) -> String {
    crate::markdown::inline_text(markdown)
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
}
