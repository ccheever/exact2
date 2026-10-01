//! Transcript mode wired to the model (workspace.rs `refresh_transcript`,
//! `send_transcript`, the transcript keys, `ToggleTranscript`; ui.rs
//! `render_transcript`'s inputs). Each session tab keeps a
//! [`TranscriptView`]; the active one is prefetched on the tick so turning
//! the mode on never waits.
//!
//! Scrolling: the contract reports the list's offset (`Event::Scrolled`,
//! which changes nothing visible) and the model answers a key with an
//! absolute `scrollTop` target: offset ± 60 for j/k/↑/↓, 0 for Home, the end
//! for End. The list follows the newest message on its own
//! (`scrollFollowEnd`), as upstream's bottom-anchored list does.

use super::*;
use crate::picker::Mods;
use crate::transcript::{Send, TranscriptKey, TranscriptView};

/// A target past any transcript's end: the list clamps it to the bottom.
const END: f64 = 1.0e9;

/// The list's scroll for one tab: where it is, and where the model last
/// asked it to be.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TranscriptScroll {
    /// The offset the contract last reported, px.
    pub offset: f64,
    /// The authored `scrollTop`; a change moves the list.
    pub target: f64,
}

impl Workspace {
    /// The active tab's key and (machine, session), when it follows a session.
    fn active_session_tab(&self) -> Option<(String, String, String)> {
        let tab = self.tabs.active_tab()?;
        let (machine, session, _) = tab.session.as_ref()?;
        Some((tab.key.clone(), machine.clone(), session.clone()))
    }

    /// The tab's transcript, made on first use.
    fn transcript_entry(&mut self, key: &str, machine: &str, session: &str) -> &mut TranscriptView {
        self.transcripts
            .entry(key.to_string())
            .or_insert_with(|| TranscriptView::new(machine, session))
    }

    /// Transcript mode is on in the active tab.
    pub fn transcript_visible(&self) -> bool {
        self.tabs
            .active_tab()
            .and_then(|tab| self.transcripts.get(&tab.key))
            .is_some_and(|view| view.visible)
    }

    /// The active tab shows its transcript and its session is RUNNING.
    fn live_transcript_active(&self) -> bool {
        self.transcript_visible()
            && self
                .tab_target()
                .is_some_and(|item| crate::session::session_state(&item.session) == "RUNNING")
    }

    /// Read the active session's transcript when due, or now when `force`.
    pub fn refresh_transcript(&mut self, force: bool) {
        let Some((key, machine, session)) = self.active_session_tab() else {
            return;
        };
        let live = self.live_transcript_active();
        let now = self.now;
        let view = self.transcript_entry(&key, &machine, &session);
        if force {
            view.refresh_now();
        }
        let Some(argv) = view.wanted(now, live) else {
            return;
        };
        let request = view.begin();
        self.queue(
            "transcript",
            argv,
            String::new(),
            Reply::Transcript { key, request },
        );
    }

    /// A `fleet transcript` reply.
    pub(super) fn transcript_arrived(
        &mut self,
        key: &str,
        request: u64,
        result: Result<String, String>,
    ) {
        let active = self.tabs.active_tab().is_some_and(|tab| tab.key == key);
        let now = self.now;
        if let Some(view) = self.transcripts.get_mut(key) {
            view.set(request, now, result, active);
        }
        if active {
            self.refresh_transcript(false);
        }
    }

    /// ToggleTranscript: a session tab switches between its terminal and its
    /// conversation.
    pub fn toggle_transcript(&mut self) {
        let Some((key, machine, session)) = self.active_session_tab() else {
            return;
        };
        let view = self.transcript_entry(&key, &machine, &session);
        view.visible = !view.visible;
        view.editing = false;
        if view.visible {
            self.transcript_scroll.insert(
                key,
                TranscriptScroll {
                    offset: 0.0,
                    target: END,
                },
            );
        }
        self.nav = false;
        self.refresh_transcript(false);
    }

    /// Show the conversation of the session just attached, at `turn`
    /// (workspace.rs `open_conversation_selection`).
    pub fn reveal_transcript_turn(&mut self, turn: usize) {
        let Some((key, machine, session)) = self.active_session_tab() else {
            return;
        };
        let view = self.transcript_entry(&key, &machine, &session);
        view.visible = true;
        view.reveal = Some(turn);
        self.transcript_scroll.insert(
            key,
            TranscriptScroll {
                offset: 0.0,
                target: END,
            },
        );
        self.nav = false;
        self.refresh_transcript(true);
    }

    /// The tab's terminal is up.
    fn tab_connected(&self, key: &str) -> bool {
        !self.exited.contains(key)
    }

    /// A key while the active tab shows its transcript; `true` when taken.
    pub(super) fn transcript_key(&mut self, name: &str, mods: &Mods) -> bool {
        let Some(key) = self.tabs.active_tab().map(|tab| tab.key.clone()) else {
            return false;
        };
        let Some(view) = self.transcripts.get_mut(&key).filter(|view| view.visible) else {
            return false;
        };
        let result = view.key(name, mods);
        match result {
            TranscriptKey::Close => self.toggle_transcript(),
            TranscriptKey::Compose => self.focus_id = "transcript-input".into(),
            TranscriptKey::StopEditing => self.focus_id = "transcript".into(),
            TranscriptKey::Send => self.send_transcript(),
            TranscriptKey::Scrolled => {
                let canon = crate::picker::canon(name);
                let scroll = self.transcript_scroll.entry(key).or_default();
                scroll.target = match canon.as_str() {
                    "home" => 0.0,
                    "end" => END,
                    "up" | "k" => (scroll.offset - crate::transcript::SCROLL_STEP).max(0.0),
                    _ => scroll.offset + crate::transcript::SCROLL_STEP,
                };
            }
            TranscriptKey::None => return false,
        }
        true
    }

    /// Send the composer's draft (workspace.rs `send_transcript`): the host
    /// pastes it, presses Enter 150 ms later and says whether it went.
    pub fn send_transcript(&mut self) {
        let Some(key) = self.tabs.active_tab().map(|tab| tab.key.clone()) else {
            return;
        };
        let connected = self.tab_connected(&key);
        let Some(view) = self.transcripts.get_mut(&key) else {
            return;
        };
        match view.send(connected) {
            Send::Nothing => {}
            Send::NotConnected => self.set_error(crate::transcript::NOT_CONNECTED),
            Send::Paste(draft) => {
                self.queue(
                    "host",
                    vec!["submit-terminal".into(), key.clone()],
                    draft.clone(),
                    Reply::TranscriptSent { key, draft },
                );
            }
        }
    }

    /// The host's answer to `submit-terminal`: "sent", or an error when the
    /// terminal went away or Enter did not go through.
    pub(super) fn transcript_sent(
        &mut self,
        key: &str,
        draft: &str,
        result: Result<String, String>,
    ) {
        let connected =
            self.tab_connected(key) && !matches!(&result, Err(e) if e.contains("not connected"));
        let sent = matches!(&result, Ok(out) if out.trim() == "sent");
        let now = self.now;
        let error = self
            .transcripts
            .get_mut(key)
            .and_then(|view| view.sent(draft, connected, sent, now));
        if let Some(error) = error {
            self.set_error(error);
        }
    }

    /// Transcript presses; `true` when `id` was one.
    pub(super) fn transcript_press(&mut self, id: &str) -> bool {
        let Some(key) = self.tabs.active_tab().map(|tab| tab.key.clone()) else {
            return false;
        };
        if id == "transcript-send" {
            self.send_transcript();
        } else if id == "transcript-input" {
            if let Some(view) = self.transcripts.get_mut(&key) {
                view.editing = true;
            }
        } else if id == "transcript-blur" {
            if let Some(view) = self.transcripts.get_mut(&key) {
                view.editing = false;
            }
        } else if let Some(index) = id.strip_prefix("transcript-toggle-") {
            if let (Ok(index), Some(view)) = (index.parse(), self.transcripts.get_mut(&key)) {
                view.toggle_instructions(index);
            }
        } else if let Some(url) = id.strip_prefix("transcript-link:") {
            self.host(vec!["open-url".into(), url.into()], String::new());
        } else if let Some(text) = id.strip_prefix("transcript-copy:") {
            self.host(vec!["clipboard-write".into()], text.into());
            self.set_message("Copied");
        } else {
            return false;
        }
        true
    }

    /// The composer's text.
    pub(super) fn transcript_input(&mut self, value: &str) {
        let Some(key) = self.tabs.active_tab().map(|tab| tab.key.clone()) else {
            return;
        };
        if let Some(view) = self.transcripts.get_mut(&key) {
            view.input(value);
        }
    }

    /// The list scrolled to `top` (no re-render).
    pub(super) fn transcript_scrolled(&mut self, top: f64) {
        if let Some(key) = self.tabs.active_tab().map(|tab| tab.key.clone()) {
            self.transcript_scroll.entry(key).or_default().offset = top;
        }
    }

    /// The `Transcript` shape for the active tab, with the working
    /// indicator painted and the scroll target.
    pub fn transcript_view(&self) -> serde_json::Value {
        let Some(tab) = self.tabs.active_tab() else {
            return serde_json::json!({ "visible": false });
        };
        let Some(view) = self.transcripts.get(&tab.key) else {
            return serde_json::json!({ "visible": false });
        };
        let live = self.tab_target().filter(|item| {
            crate::session::session_observation_available(&item.machine, &item.session, self.now)
                && crate::session::session_state(&item.session) == "RUNNING"
        });
        let live_pair = live.as_ref().map(|item| {
            (
                item.session.provider.as_str(),
                item.session.status_text.as_str(),
            )
        });
        let mut json = view.view(live_pair, self.tab_connected(&tab.key));
        let provider = live_pair.map(|(p, _)| p).unwrap_or("");
        let paint = |text: &str| {
            let ind = crate::indicator::indicator(provider, text, self.now, &self.theme);
            serde_json::json!({
                "glyph": ind.glyph,
                "head": ind.head,
                "headColor": ind.head_color,
                "rest": ind.rest,
                "restColor": ind.rest_color,
            })
        };
        let working = json["working"].as_str().unwrap_or("").to_string();
        json["workingIndicator"] = paint(&working);
        if let Some(entries) = json["entries"].as_array_mut() {
            for entry in entries {
                let live = entry["live"].as_str().unwrap_or("").to_string();
                entry["liveIndicator"] = paint(&live);
            }
        }
        let scroll = self
            .transcript_scroll
            .get(&tab.key)
            .copied()
            .unwrap_or_default();
        json["scrollTop"] = serde_json::json!(scroll.target);
        json
    }
}
