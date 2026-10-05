//! Ocho on the phone: the desktop's windows and tabs, each session's
//! conversation, and a composer, over Fleet's mobile API.
//!
//! The seam is Ocho's, Elm-shaped. Every event goes to `dispatch`, which
//! answers the counters: the view's version and one turn per I/O lane. Each
//! lane is a resource keyed by its turn (`poll`, `transcript`, `send`,
//! `probe`, `resync`, `markRead`, `haptic`, `report`); asked with a new turn it answers with that lane's
//! request, and its reply lands in the model. `picture` is the whole
//! view-model, answered now whenever a version moves.

#![deny(missing_docs)]

pub mod api;
pub mod fleet;
pub mod model;
mod shapes;
pub mod telemetry;
mod view;

use exact_plan::Value;
use exact_runner::{Answer, DataError, DataSource, Outcome, Request, Store};

pub use model::Model;

/// The app's data source.
#[derive(Default)]
pub struct OchoMobile {
    model: Model,
}

const GRANTS: &str = "net.fetch https://fleet-relay.fly.dev\n\
net.fetch https://fleet-service.eliot-4cd.workers.dev\n\
secret.keep ocho.install\n\
device.camera purpose.camera\n\
secret.keep ocho.connection\n\
secret.keep ocho.desktop";

fn text(args: &[Value], i: usize) -> String {
    match args.get(i) {
        Some(Value::Number(n)) => format!("{n}"),
        Some(v) => v.as_str().unwrap_or_default().to_string(),
        None => String::new(),
    }
}

fn number(args: &[Value], i: usize) -> f64 {
    match args.get(i) {
        Some(Value::Number(n)) => *n,
        Some(v) => v.as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0),
        None => 0.0,
    }
}

fn counters(m: &Model) -> Value {
    shapes::read(
        &shapes::UI,
        &serde_json::json!({
            "version": m.version,
            "poll": m.poll.turn,
            "transcript": m.transcript.turn,
            "send": m.send.turn,
            "probe": m.probe.turn,
            "pulls": m.refresh.turn,
            "reads": m.reads.turn,
            "buzz": m.buzz.turn,
            "reports": m.report.turn,
        }),
    )
}

fn version(m: &Model) -> Value {
    shapes::read(
        &shapes::VERSION,
        &serde_json::json!({ "version": m.version }),
    )
}

fn get(url: &str, bearer: &str, max: u32) -> Request {
    Request::get(url)
        .header("authorization", bearer)
        .header("accept", "application/json")
        .independent_http(max)
}

/// A response's JSON on a 2xx; else the status and the server's `error`.
fn body_json(outcome: Outcome) -> Result<serde_json::Value, (u16, String)> {
    match outcome {
        Outcome::Response(r) if (200..300).contains(&r.status) => {
            serde_json::from_slice(&r.body).map_err(|e| (r.status, e.to_string()))
        }
        Outcome::Response(r) => {
            let why = serde_json::from_slice::<serde_json::Value>(&r.body)
                .ok()
                .and_then(|j| j.get("error").and_then(|e| e.as_str()).map(str::to_string))
                .unwrap_or_else(|| format!("HTTP {}", r.status));
            Err((r.status, why))
        }
        Outcome::Failed { message, .. } => Err((0, message)),
        _ => Err((0, "no answer".into())),
    }
}

impl OchoMobile {
    fn load(&mut self, store: &Store) {
        if !self.model.loaded {
            self.model.load(
                store.get(model::KEY_CONNECTION),
                store.get(model::KEY_DESKTOP),
                store.get(model::KEY_INSTALL),
            );
        }
    }

    fn flush(&mut self, store: &mut Store) {
        for (name, value) in self.model.writes.drain(..) {
            // Refused only outside the grants, which name every key.
            let _ = match value {
                Some(v) => store.set(name, &v),
                None => store.forget(name),
            };
        }
    }

    fn dispatch(&mut self, args: &[Value]) {
        let (kind, a, b) = (text(args, 0), text(args, 1), text(args, 2));
        let now = number(args, 3);
        if now > 0.0 {
            self.model.clock(now);
        }
        let m = &mut self.model;
        match kind.as_str() {
            "tick" => m.tick(now),
            "pair-typed" => m.pair_typed(&a),
            "pair" => m.pair(),
            "pair-scanned" => m.pair_scanned(&a),
            "pair-link" => m.pair_link(&a),
            "unpair" => m.unpair(),
            "window" => m.pick_window(a.parse().unwrap_or(0)),
            "open" => m.open(&a, &b),
            "close" => m.close(),
            "nav" => m.navigated(&a),
            "send" => m.send_text(&a),
            "retry" => m.retry(),
            "dismiss" => m.dismiss(),
            "earlier" => m.show_earlier(),
            "composer" => {
                if let Some(text) = a.strip_prefix("s:") {
                    m.send_text(text);
                } else if let Some(points) = a.strip_prefix("h:") {
                    m.composer_sized(points.parse().unwrap_or(0.0));
                }
            }
            "pull" => m.pull(),
            _ => {}
        }
    }

    fn ask(&mut self, source: &str) -> Answer {
        let m = &mut self.model;
        let request = match source {
            "poll" if !m.poll.inflight => m
                .poll_request()
                .map(|(url, bearer)| get(&url, &bearer, api::POLL_BYTES)),
            "transcript" if !m.transcript.inflight => m
                .transcript_request()
                .map(|(url, bearer)| get(&url, &bearer, api::MAX_BYTES)),
            "send" if !m.send.inflight => m.send_request().map(|(url, bearer, body)| {
                Request::post_json(&url, &body)
                    .header("authorization", &bearer)
                    .independent_http(api::SEND_BYTES)
            }),
            "probe" if !m.probe.inflight => m
                .probe_request()
                .map(|url| Request::get(&url).independent_http(64 << 10)),
            "report" if !m.report.inflight => m.report_request().map(|body| {
                Request::post_json(telemetry::ENDPOINT, &body).independent_http(16 << 10)
            }),
            "haptic" if !m.buzz.inflight => m
                .haptic_request()
                .map(|body| Request::native(body.into_bytes())),
            "markRead" if !m.reads.inflight => m.reads_request().map(|(url, bearer, body)| {
                Request::post_json(&url, &body)
                    .header("authorization", &bearer)
                    .independent_http(64 << 10)
            }),
            "resync" if !m.refresh.inflight => m.refresh_request().map(|(url, bearer)| {
                Request::post_json(&url, "{}")
                    .header("authorization", &bearer)
                    .independent_http(64 << 10)
            }),
            _ => None,
        };
        match request {
            Some(request) => Answer::Later(request),
            None => Answer::Now(version(m)),
        }
    }

    fn reply(&mut self, source: &str, outcome: Outcome) {
        let m = &mut self.model;
        match source {
            "poll" => {
                let result = body_json(outcome);
                if matches!(result, Err((401, _))) {
                    m.poll_refused();
                }
                m.poll_done(result);
            }
            "transcript" => m.transcript_done(match outcome {
                Outcome::Response(r) if r.status == 200 => {
                    Ok(String::from_utf8_lossy(&r.body).into_owned())
                }
                other => Err(body_json(other)
                    .err()
                    .map(|(_, why)| why)
                    .unwrap_or_default()),
            }),
            "send" => m.send_done(body_json(outcome).map_err(|(_, why)| why)),
            "probe" => m.probe_done(
                body_json(outcome)
                    .is_ok_and(|j| j.get("ok").and_then(|ok| ok.as_bool()).unwrap_or(false)),
            ),
            "resync" => m.refresh_done(),
            "haptic" => m.haptic_done(),
            "report" => m.report_done(
                matches!(outcome, Outcome::Response(ref r) if (200..300).contains(&r.status)),
            ),
            "markRead" => m.reads_done(match outcome {
                Outcome::Response(r) if (200..300).contains(&r.status) => Ok(()),
                Outcome::Response(r) => Err(r.status),
                _ => Err(0),
            }),
            _ => {}
        }
    }
}

impl DataSource for OchoMobile {
    fn app_id(&self) -> &str {
        "dev.getfirewood.ocho.mobile"
    }

    fn grants(&self) -> &str {
        GRANTS
    }

    fn query(&mut self, source: &str, _args: &[Value]) -> Result<Value, DataError> {
        match source {
            "dispatch" => Ok(counters(&self.model)),
            "poll" | "transcript" | "send" | "probe" | "resync" | "markRead" | "haptic"
            | "report" => Ok(version(&self.model)),
            "picture" => Ok(view::render(&self.model)),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }

    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        self.load(store);
        let answer = match source {
            "dispatch" => {
                self.dispatch(args);
                Answer::Now(counters(&self.model))
            }
            "picture" => Answer::Now(view::render(&self.model)),
            "poll" | "transcript" | "send" | "probe" | "resync" | "markRead" | "haptic"
            | "report" => {
                if source == "poll" {
                    // Asked with a new visibility or connection, not a new turn.
                    let visible = text(args, 1) != "hidden";
                    let online = !matches!(args.get(2), Some(Value::Bool(false)));
                    self.model.page(visible, online);
                    // Exact lets go of a request whose arguments are replaced,
                    // and its reply never comes (LLP 1016 D5): a poll still
                    // marked in flight would wait for the watchdog, 40 s of
                    // "Connecting…". Ask again; an equal request keeps the
                    // one already out (LLP 1054.000.000 D3).
                    self.model.poll_replaced();
                }
                self.ask(source)
            }
            _ => return Err(DataError::UnknownSource(source.into())),
        };
        self.flush(store);
        Ok(answer)
    }

    fn parse(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        self.load(store);
        // A reply for a turn the lane gave up on (`Model::give_up_on_overdue`)
        // is stale: the lane has asked again.
        let asked = number(args, 0) as u64;
        if self
            .model
            .lane_turn(source)
            .is_some_and(|turn| turn != asked)
        {
            return Ok(Answer::Now(version(&self.model)));
        }
        self.reply(source, outcome);
        self.flush(store);
        Ok(Answer::Now(version(&self.model)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_composer_sends_its_text_and_reports_its_height() {
        let mut source = OchoMobile::default();
        source.model.open = Some(("mac".into(), "s1".into()));
        source.dispatch(&[
            Value::from("composer"),
            Value::from("h:88"),
            Value::from(""),
            Value::Number(1.0),
        ]);
        assert_eq!(source.model.composer_height, 88.0);
        // No live session here: the send fails, which proves it was tried.
        source.dispatch(&[
            Value::from("composer"),
            Value::from("s:hello there"),
            Value::from(""),
            Value::Number(2.0),
        ]);
        let failed = source
            .model
            .failed
            .clone()
            .expect("the text reached send_text");
        assert_eq!(failed.1, "hello there");
    }

    #[test]
    fn a_poll_asked_again_before_its_reply_asks_again_at_once() {
        let mut source = OchoMobile::default();
        let connection =
            r#"{"relay":"https://fleet-relay.fly.dev","machine":"mac","token":"t","name":"Mac"}"#;
        let mut store = Store::new(
            GRANTS,
            [(model::KEY_CONNECTION.to_string(), connection.to_string())],
        );
        let turn = source.model.poll.turn as f64;
        let ask = |online: Value| vec![Value::Number(turn), Value::from("visible"), online];
        // Launch: the page has not said whether it is online yet.
        let first = source
            .answer(&mut store, "poll", &ask(Value::Unit))
            .unwrap();
        assert!(matches!(first, Answer::Later(_)));
        // It says so before the first reply: Exact lets that request go.
        let again = source
            .answer(&mut store, "poll", &ask(Value::Bool(true)))
            .unwrap();
        assert!(matches!(again, Answer::Later(_)), "not 40 s of Connecting…");
    }
}
