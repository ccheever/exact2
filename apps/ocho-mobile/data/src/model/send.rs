//! The composer's messages: queued in order, each sent by its route (typed
//! input, a message, or Codex's queue: attach, send, detach).

use super::*;

impl Model {
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
        let points = points.clamp(44.0, 240.0);
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
        self.pending.push(Pending {
            key: key.clone(),
            text: text.clone(),
            after,
        });
        self.failed = None;
        self.outbox.push_back(Outgoing {
            key,
            text,
            route,
            request_id: format!("{:016x}{:016x}", self.now as u64, self.sent_count),
            thread,
            step: Step::Attach,
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
            _ => (
                "input",
                serde_json::json!({ "text": out.text, "enter": true }),
            ),
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
