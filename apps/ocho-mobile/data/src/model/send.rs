//! The composer's messages: queued in order, each sent by its route (typed
//! input, a message, or the queue Codex and Claude's gateway share: attach,
//! send, detach).

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
        let Some((thread, leaf)) = self
            .live_session(&key)
            .map(|s| (s.native_id.clone(), s.queue_leaf()))
        else {
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
            leaf,
            retries: 0,
            checks: 0,
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
        let (route, thread, leaf) = match self.live_session(&key) {
            // Claude's gateway takes words only: a message with files goes to
            // the terminal, where they are pasted and become images.
            Some(s) if s.queues_claude() && s.can_type() && !split_files(&text).1.is_empty() => {
                (SendRoute::Input, String::new(), "")
            }
            Some(s) => (s.send_route(), s.native_id.clone(), s.queue_leaf()),
            None => (
                SendRoute::None("This session is offline."),
                String::new(),
                "",
            ),
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
            queued: false,
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
            leaf,
            retries: 0,
            checks: 0,
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
        // A step that waits its moment (a look at a receipt, a retry after a
        // blip) goes when `tick` finds it due.
        if self.now < self.send.next_at {
            return None;
        }
        let conn = self.conn.as_ref()?;
        let out = self.outbox.pop_front()?;
        let (leaf, body) = match out.route {
            SendRoute::Queue => {
                let mut body = serde_json::json!({
                    "operation": match out.step {
                        Step::Attach => "attach",
                        Step::Send if out.interrupt => "interrupt-send",
                        Step::Send => "send",
                        Step::Poll => "poll",
                        Step::Detach => "detach",
                    },
                    "client_id": self.client_id(),
                    "thread_id": out.thread,
                });
                if out.step == Step::Send {
                    body["request_id"] = out.request_id.clone().into();
                    body["text"] = out.text.clone().into();
                }
                if out.step == Step::Poll {
                    body["request_id"] = out.request_id.clone().into();
                }
                (out.leaf, body)
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
        // A network blip (a VPN reconnecting, a lost connection) is tried
        // again, a few times, where that cannot deliver the message twice.
        if let Err(why) = &result {
            if out.retries < SEND_RETRIES && retry_is_safe(&out, why) {
                out.retries += 1;
                self.send.next_at = self.now + 1_000.0 * f64::from(1u32 << out.retries);
                self.outbox.push_front(out);
                return;
            }
        }
        out.retries = 0;
        if out.route == SendRoute::Queue && out.step == Step::Poll {
            self.receipt_seen(out, result);
            return;
        }
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
        // The queue answers a snapshot whose receipt is the message's.
        let result = result.map(|reply| reply.get("receipt").cloned().unwrap_or(reply));
        let result_status = result.as_ref().ok().cloned();
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
            // Taken already, or held behind the turn? Look once delivery has
            // had its moment (an interrupting send waits for its answer).
            let next = if failure.is_none()
                && !out.interrupt
                && receipt_status(&result_status) != Some("submitted")
            {
                self.send.next_at = self.now + RECEIPT_LOOK_MS;
                Step::Poll
            } else {
                Step::Detach
            };
            self.outbox.push_front(Outgoing {
                step: next,
                checks: 0,
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

/// How many times a step is tried again after a network failure.
const SEND_RETRIES: u32 = 4;

/// Whether a failed step can be sent again without delivering it twice:
/// always when it never left the phone or never reached the machine; when
/// it may have arrived, only on a route that names it (`request_id`), where
/// Fleet answers a repeat with the first one's outcome.
fn retry_is_safe(out: &Outgoing, why: &str) -> bool {
    let lower = why.to_lowercase();
    let never_arrived =
        lower.contains("appears to be offline") || lower.contains("not connected to the relay");
    let maybe_arrived = lower.contains("connection was lost")
        || lower.contains("timed out")
        || lower.contains("did not answer in time")
        || lower.contains("machine disconnected")
        || lower.contains("no answer")
        || lower.contains("failed to fetch");
    never_arrived || (maybe_arrived && out.route != SendRoute::Input)
}

/// How long after a send its receipt is looked at, and how many looks a
/// message gets before the transcript is left to say.
const RECEIPT_LOOK_MS: f64 = 700.0;
const RECEIPT_LOOKS: u32 = 3;

fn receipt_status(receipt: &Option<serde_json::Value>) -> Option<&str> {
    receipt.as_ref()?.get("status")?.as_str()
}

impl Model {
    /// A look at a sent message's receipt: taken (a plain bubble), held
    /// behind the turn (the queue tray, with "Send now"), refused, or not
    /// yet known (look again, a few times).
    fn receipt_seen(&mut self, mut out: Outgoing, result: Result<serde_json::Value, String>) {
        let receipt = result
            .ok()
            .map(|reply| reply.get("receipt").cloned().unwrap_or(reply));
        let status = receipt_status(&receipt).map(str::to_string);
        let echoed = !self.pending.iter().any(|p| p.request_id == out.request_id);
        let mut detach = true;
        match status.as_deref() {
            _ if echoed => {}
            Some("submitted") => self.mark_queued(&out.request_id, false),
            Some("queued") => self.mark_queued(&out.request_id, true),
            Some("not_submitted") => {
                let why = receipt
                    .as_ref()
                    .and_then(|r| r.get("reason"))
                    .and_then(|r| r.as_str())
                    .filter(|r| !r.is_empty())
                    .unwrap_or("The session did not take the message.")
                    .to_string();
                self.outbox.push_front(Outgoing {
                    step: Step::Detach,
                    ..out.clone()
                });
                self.send_failed(out, why);
                return;
            }
            // A server whose poll names no receipt (Fleet before #475) can't
            // say: held while the session works, as before.
            None if receipt.as_ref().is_some_and(|r| r.get("status").is_none()) => {
                let working = self.live_session(&out.key).is_some_and(|s| s.working());
                self.mark_queued(&out.request_id, working);
            }
            _ if out.checks + 1 < RECEIPT_LOOKS => {
                out.checks += 1;
                self.send.next_at = self.now + 1_000.0;
                self.outbox.push_front(out.clone());
                detach = false;
            }
            _ => {}
        }
        if detach {
            self.outbox.push_front(Outgoing {
                step: Step::Detach,
                ..out
            });
            self.send.bump();
        }
    }

    fn mark_queued(&mut self, request_id: &str, queued: bool) {
        if let Some(p) = self.pending.iter_mut().find(|p| p.request_id == request_id) {
            if p.queued != queued {
                p.queued = queued;
                self.version += 1;
            }
        }
    }
}

/// The platform's description of a failed request, without the fetch
/// wrapper ("TypeError: Failed to fetch — ").
pub(super) fn system_error(why: &str) -> String {
    let line = why.lines().next().unwrap_or("").trim();
    let line = line.rsplit(" — ").next().unwrap_or(line).trim();
    line.chars().take(120).collect()
}
