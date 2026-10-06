//! The composer's messages: queued in order, each sent by its route (typed
//! input, a message, or Codex's queue: attach, send, detach).

use super::*;

impl Model {
    /// "Send now" on a message waiting behind a running Codex turn: the same
    /// request again as `interrupt-send`, which stops the turn and sends it.
    pub fn send_now(&mut self, request_id: &str) {
        let Some(at) = self
            .pending
            .iter()
            .position(|p| p.request_id == request_id && p.queued && !p.interrupting)
        else {
            return;
        };
        let key = self.pending[at].key.clone();
        let Some(thread) = self.live_session(&key).map(|s| s.native_id.clone()) else {
            return;
        };
        self.pending[at].interrupting = true;
        self.outbox.push_back(Outgoing {
            key,
            text: self.pending[at].text.clone(),
            route: SendRoute::Queue,
            request_id: request_id.to_string(),
            thread,
            step: Step::Attach,
            interrupt: true,
        });
        self.send.bump();
        self.feel("medium");
        self.version += 1;
    }

    /// The call's state from the voice page: felt as it connects and ends.
    pub fn voice_state(&mut self, state: &str) {
        match state {
            "connected" => self.feel("success"),
            "ended" => self.feel("light"),
            _ => {}
        }
    }

    /// Where the open session's attachments upload (fleet serve's
    /// `…/sessions/{id}/upload`, through the relay) and the bearer.
    pub fn upload(&self) -> Option<(String, String)> {
        let conn = self.conn.as_ref()?;
        let key = self.open.as_ref()?;
        self.live_session(key)?;
        let url = conn.session_url(&self.route_of(&self.via), &key.0, &key.1, "upload");
        Some((url, conn.bearer()))
    }

    /// Fleet's voice page for the open session (`…/voice?thread=<native id>`
    /// through the relay) and the bearer for its first request's header;
    /// `None` unless Codex's voice can join it.
    pub fn voice(&self) -> Option<(String, String)> {
        let conn = self.conn.as_ref()?;
        let key = self.open.as_ref()?;
        let session = self.live_session(key).filter(|s| s.can_talk())?;
        let url = conn.session_url(&self.route_of(&self.via), &key.0, &key.1, "voice");
        Some((
            format!("{url}?thread={}", crate::api::encode(&session.native_id)),
            conn.bearer(),
        ))
    }

    /// Draw a page more of the open conversation's history.
    pub fn show_earlier(&mut self) {
        let Some(key) = self.open.clone() else { return };
        let conversation = self.conversations.entry(key).or_default();
        conversation.shown =
            conversation.shown.max(crate::view::PAGE_ENTRIES) + crate::view::PAGE_ENTRIES;
        self.version += 1;
    }

    /// The native composer asked for `points` of height (its text wrapped).
    pub fn composer_sized(&mut self, points: f64) {
        // Six lines of text and a row of attachments.
        let points = points.clamp(44.0, 320.0);
        if (points - self.composer_height).abs() >= 0.5 {
            self.composer_height = points;
            self.version += 1;
        }
    }

    /// Send `text` to the open session.
    pub fn send_text(&mut self, text: &str) {
        let text = text.trim_end().to_string();
        let Some(key) = self.open.clone() else { return };
        if text.trim().is_empty() {
            return;
        }
        let (route, thread) = match self.live_session(&key) {
            Some(s) => (s.send_route(), s.native_id.clone()),
            None => (SendRoute::None("This session is offline."), String::new()),
        };
        if let SendRoute::None(why) = route {
            self.failed = Some((key, text, why.to_string()));
            self.feel("error");
            self.version += 1;
            return;
        }
        self.sent_count += 1;
        let after = self
            .conversations
            .get(&key)
            .map(|c| c.transcript.entries.len())
            .unwrap_or(0);
        let request_id = format!("{:016x}{:016x}", self.now as u64, self.sent_count);
        self.pending.push(Pending {
            key: key.clone(),
            text: text.clone(),
            after,
            request_id: request_id.clone(),
            queued: route == SendRoute::Queue,
            interrupting: false,
        });
        self.failed = None;
        self.outbox.push_back(Outgoing {
            key,
            text,
            route,
            request_id,
            thread,
            step: Step::Attach,
            interrupt: false,
        });
        self.send.bump();
        self.feel("light");
        self.scroll_revision += 1;
        self.version += 1;
    }

    /// Send the failed message again.
    pub fn retry(&mut self) {
        if let Some((key, text, _)) = self.failed.take() {
            if self.open.as_ref() == Some(&key) {
                self.send_text(&text);
                return;
            }
        }
        self.version += 1;
    }

    /// Dismiss a send failure.
    pub fn dismiss(&mut self) {
        self.failed = None;
        self.version += 1;
    }

    /// The outgoing message's request: URL, bearer, JSON body.
    pub fn send_request(&mut self) -> Option<(String, String, String)> {
        let conn = self.conn.as_ref()?;
        let out = self.outbox.pop_front()?;
        let (leaf, body) = match out.route {
            SendRoute::Queue => {
                let mut body = serde_json::json!({
                    "operation": match out.step {
                        Step::Attach => "attach",
                        Step::Send if out.interrupt => "interrupt-send",
                        Step::Send => "send",
                        Step::Detach => "detach",
                    },
                    "client_id": self.client_id(),
                    "thread_id": out.thread,
                });
                if out.step == Step::Send {
                    body["request_id"] = out.request_id.clone().into();
                    body["text"] = out.text.clone().into();
                }
                ("codex-client", body)
            }
            SendRoute::Message => (
                "message",
                serde_json::json!({ "request_id": out.request_id, "text": out.text }),
            ),
            // Attached files are pasted first and alone, as the desktop pastes
            // them: an agent's terminal (Claude Code's) makes a paste of image
            // paths an attached image, but not one with words around it. The
            // words follow, with Enter.
            _ => {
                let (words, files) = split_files(&out.text);
                let body = if out.step == Step::Attach && !files.is_empty() {
                    serde_json::json!({ "text": format!("{files} "), "enter": false })
                } else {
                    serde_json::json!({ "text": words, "enter": true })
                };
                ("input", body)
            }
        };
        let url = conn.session_url(&self.route_of(&self.via), &out.key.0, &out.key.1, leaf);
        let bearer = conn.bearer();
        self.send.start(self.now);
        self.sending = Some(out);
        Some((url, bearer, body.to_string()))
    }

    /// A finished send: the reply's JSON on a 2xx, else why.
    pub fn send_done(&mut self, result: Result<serde_json::Value, String>) {
        self.send.inflight = false;
        let Some(mut out) = self.sending.take() else {
            return;
        };
        if out.route == SendRoute::Queue && out.step != Step::Send {
            let attached = out.step == Step::Attach;
            match result {
                Ok(_) if attached => {
                    out.step = Step::Send;
                    self.outbox.push_front(out);
                    self.send.bump();
                }
                // Detaching is tidying; the message is already Fleet's.
                _ if !attached => {}
                Err(why) => {
                    self.track(
                        "send.result",
                        true,
                        vec![("ok", false.into()), ("route", "queue".into())],
                    );
                    self.send_failed(out, why);
                }
                Ok(_) => {}
            }
            if !attached && !self.outbox.is_empty() {
                self.send.bump();
            }
            return;
        }
        if out.route == SendRoute::Input
            && out.step == Step::Attach
            && !split_files(&out.text).1.is_empty()
        {
            match result {
                Ok(_) => {
                    out.step = Step::Send;
                    self.outbox.push_front(out);
                    self.send.bump();
                }
                Err(why) => self.send_failed(out, why),
            }
            return;
        }
        // `codex-client` answers a snapshot whose receipt is the message's.
        let result = result.map(|reply| reply.get("receipt").cloned().unwrap_or(reply));
        let failure = match result {
            Ok(reply) => match reply.get("status").and_then(|s| s.as_str()) {
                Some("not_submitted") => Some(
                    reply
                        .get("reason")
                        .and_then(|r| r.as_str())
                        .filter(|r| !r.is_empty())
                        .unwrap_or("The session did not take the message.")
                        .to_string(),
                ),
                _ => None,
            },
            Err(e) => Some(e),
        };
        let route = match out.route {
            SendRoute::Queue => "queue",
            SendRoute::Message => "message",
            _ => "input",
        };
        let ok = failure.is_none();
        self.track(
            "send.result",
            !ok,
            vec![("ok", ok.into()), ("route", route.into())],
        );
        if out.route == SendRoute::Queue {
            self.outbox.push_front(Outgoing {
                step: Step::Detach,
                ..out.clone()
            });
        }
        match failure {
            None => {
                if self.open.as_ref() == Some(&out.key) && !self.transcript.inflight {
                    self.transcript.next_at = self.now + AFTER_SEND_MS;
                }
                self.version += 1;
            }
            Some(why) => self.send_failed(out, why),
        }
        if !self.outbox.is_empty() {
            self.send.bump();
        }
    }

    /// `out` did not go: take back its bubble and say why.
    fn send_failed(&mut self, out: Outgoing, why: String) {
        if let Some(i) = self
            .pending
            .iter()
            .rposition(|p| p.key == out.key && p.text == out.text)
        {
            self.pending.remove(i);
        }
        self.failed = Some((out.key, out.text, why));
        self.feel("error");
        self.version += 1;
    }

    /// This phone, to a Codex thread's subscribers.
    fn client_id(&self) -> String {
        format!("ocho-phone-{}", self.telemetry.install)
    }
}

/// A message's words and, apart, the attached files' paths the composer put
/// after them: the last paragraph when every word of it is a path in Fleet's
/// paste folder.
pub fn split_files(text: &str) -> (String, String) {
    let (words, last) = match text.rfind("\n\n") {
        Some(at) => (&text[..at], &text[at + 2..]),
        None => ("", text),
    };
    let paths: Vec<&str> = last.split_whitespace().collect();
    if paths.is_empty()
        || !paths
            .iter()
            .all(|p| p.starts_with('/') && p.contains("/fleet/paste/"))
    {
        return (text.to_string(), String::new());
    }
    (words.trim_end().to_string(), paths.join(" "))
}

/// Whether a transcript's user entry is `sent` read back. Attached files come
/// back as the agent shows them (`[Image #1]`, its own path), so a message
/// with files matches on its words.
pub fn echoes(entry: &str, sent: &str) -> bool {
    let (words, files) = split_files(sent);
    if files.is_empty() {
        entry.trim() == sent.trim()
    } else {
        entry.contains(words.trim())
    }
}
