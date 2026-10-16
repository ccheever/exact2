//! The build channel's polled stream (LLP 1122 D4, LLP 1121 stage 4 commit
//! 5): this host holds no SSE dev stream, so the URL poller's channel worker asks
//! `/__dev/hello` for a nonce, registers with it (`POST /__dev/report`), and
//! on each tick sends what the presenter queued and takes the frames queued
//! for it (`GET /__dev/commands`). A 404 hello is a dev loop with no design
//! server: the channel is quiet a while, then asks again. A 403 is an
//! expired nonce (5 s unpolled, or a restarted loop): hello again and
//! register again, under the same id, so the loop replays this build's pins.
//! Only the worker touches the network; the presenter applies the frames
//! (`presenter/build_channel.rs`).

use serde_json::{json, Value};
use std::time::{Duration, Instant};

/// A hello, a report's answer and a frame list are small documents.
const MAX_CHANNEL_BYTES: usize = 64 * 1024;
/// How long a dev loop that answered hello 404 is left before it is asked again.
const QUIET: Duration = Duration::from_secs(30);
/// Reports kept while none can be sent; the oldest give way.
const MAX_OUTBOX: usize = 64;

/// This process's build id, the same for every registration it makes.
fn build_id() -> &'static str {
    static ID: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    ID.get_or_init(|| {
        let mut bytes = [0u8; 8];
        let _ = getrandom::fill(&mut bytes);
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        format!("linux-{hex}")
    })
}

/// What a pin replaced, handed back by a null pin.
#[derive(Clone)]
pub(crate) struct Pinned {
    pub dark: bool,
    pub contrast: exact_runner::Contrast,
    pub modes: Vec<(String, String)>,
    pub root_font_size: f64,
}

/// The presenter's side of the channel, held by the URL poller.
pub(crate) struct Link {
    id: String,
    nonce: Option<String>,
    quiet_until: Option<Instant>,
    /// Reports for the next exchange.
    outbox: Vec<Value>,
    /// Frames the last exchange took, for the presenter to apply.
    pub frames: Vec<Value>,
    /// The last exchange registered (again): the presenter reports status.
    pub registered: bool,
    /// What a registration names: the plan identity and design digest last reported.
    pub plan: Option<String>,
    pub design: Option<String>,
    /// The last pin revision applied.
    pub revision: Option<u64>,
    /// What the first pin replaced, while any is held.
    pub before: Option<Pinned>,
    /// The pin held, by revision: set again on a new generation's runner.
    pub held: Option<(u64, Pin)>,
    /// The scheme (dark), contrast and root font size the last status named.
    pub facts: Option<(bool, exact_runner::Contrast, f64)>,
    /// The window's modes the last status named, and the kernel epoch they
    /// were last compared at: read again only after a commit.
    pub modes: Option<(u64, Vec<(String, String)>)>,
    /// Fetch `/__dev/design`'s snapshot next tick: a batch was stale, or the
    /// build registered (again) and may have missed batches.
    pub snapshot: bool,
}

impl Link {
    pub fn new(plan: Option<String>) -> Self {
        Self {
            id: build_id().to_string(),
            nonce: None,
            quiet_until: None,
            outbox: Vec::new(),
            frames: Vec::new(),
            registered: false,
            plan,
            design: None,
            revision: None,
            before: None,
            held: None,
            facts: None,
            modes: None,
            snapshot: false,
        }
    }

    /// Queue a report (its nonce is the exchange's). A status supersedes
    /// any still queued: at most one goes out a tick.
    pub fn report(&mut self, report: Value) {
        if is_status(&report) {
            self.outbox.retain(|r| !is_status(r));
        }
        self.outbox.push(report);
        self.bound();
    }

    fn bound(&mut self) {
        let over = self.outbox.len().saturating_sub(MAX_OUTBOX);
        self.outbox.drain(..over);
    }

    /// Whether the next tick fetches `/__dev/design`: always while the
    /// channel is off (no nonce: a 404 hello, not yet registered), else
    /// only when asked — the live frames come down the commands poll.
    pub fn take_design(&mut self) -> bool {
        let wanted = self.nonce.is_none() || self.snapshot;
        self.snapshot = false;
        wanted
    }

    /// A snapshot [`Link::take_design`] asked for did not come (a failed
    /// request, a 5xx): the next tick asks again.
    pub fn missed_design(&mut self) {
        self.snapshot = true;
    }

    /// The live-table frames (`tamber-batch`, `tamber-snapshot`) the ticks
    /// took, in order; the rest stay for [`Link::frames`]' other readers.
    pub fn take_live(&mut self) -> Vec<Value> {
        let (live, rest) = std::mem::take(&mut self.frames)
            .into_iter()
            .partition(is_live);
        self.frames = rest;
        live
    }

    /// The worker's half of the next tick, or `None` while the loop is quiet.
    pub fn exchange(&mut self) -> Option<Exchange> {
        if self.quiet_until.is_some_and(|t| Instant::now() < t) {
            self.outbox.clear();
            return None;
        }
        Some(Exchange {
            id: self.id.clone(),
            nonce: self.nonce.clone(),
            reports: std::mem::take(&mut self.outbox),
            plan: self.plan.clone(),
            design: self.design.clone(),
        })
    }

    /// Take a tick's answer back.
    pub fn settle(&mut self, answer: Answer) {
        self.nonce = answer.nonce;
        self.quiet_until = answer.off.then(|| Instant::now() + QUIET);
        self.registered |= answer.registered;
        self.snapshot |= answer.registered;
        self.frames.extend(answer.frames);
        // What did not go out goes first next tick, unless a newer status
        // queued since stands for its status.
        let newer = self.outbox.iter().any(is_status);
        let mut unsent = answer.unsent;
        unsent.retain(|r| !(newer && is_status(r)));
        unsent.append(&mut self.outbox);
        self.outbox = unsent;
        if answer.off {
            self.outbox.clear();
        }
        self.bound();
    }
}

/// One tick's network work, moved to the worker thread.
pub(crate) struct Exchange {
    id: String,
    nonce: Option<String>,
    reports: Vec<Value>,
    plan: Option<String>,
    design: Option<String>,
}

/// What a tick brings back.
pub(crate) struct Answer {
    nonce: Option<String>,
    frames: Vec<Value>,
    registered: bool,
    off: bool,
    /// Reports kept for the next tick: a transport failure or a 5xx.
    unsent: Vec<Value>,
}

/// A live-table frame, the same JSON the web page's stream carries.
pub(crate) fn is_live(frame: &Value) -> bool {
    matches!(
        frame["kind"].as_str(),
        Some("tamber-batch" | "tamber-snapshot")
    )
}

fn is_status(report: &Value) -> bool {
    report["kind"] == "status"
}

/// A request's status and body; `None` when it did not complete.
fn send(url: &str, body: Option<&Value>) -> Option<(u16, Vec<u8>)> {
    let mut req = ibex2::stdlib::fetch::Request::get(url);
    req.headers.set("accept", "application/json");
    req.headers.set("cache-control", "no-cache");
    if let Some(body) = body {
        req.method = "POST".into();
        req.headers.set("content-type", "application/json");
        req.body = Some(body.to_string().into_bytes());
    }
    req.max_body = Some(MAX_CHANNEL_BYTES);
    req.timeout = Some(Duration::from_secs(5));
    let r = ibex2::transport::default_transport().send(&req).ok()?;
    Some((r.status, r.body))
}

impl Exchange {
    /// Hello and register when there is no nonce, send the reports, take the
    /// frames; an expired nonce is renewed once a tick. A report a failed
    /// transport or a 5xx did not take is kept; another 4xx refused it.
    pub fn run(mut self, page: &str) -> Answer {
        let mut answer = Answer {
            nonce: self.nonce.take(),
            frames: Vec::new(),
            registered: false,
            off: false,
            unsent: Vec::new(),
        };
        self.tick(page, &mut answer);
        answer.unsent = std::mem::take(&mut self.reports);
        answer
    }

    fn tick(&mut self, page: &str, answer: &mut Answer) {
        let (Ok(hello), Ok(report), Ok(commands)) = (
            crate::fetch::join(page, "/__dev/hello"),
            crate::fetch::join(page, "/__dev/report"),
            crate::fetch::join(page, "/__dev/commands"),
        ) else {
            return;
        };
        for _ in 0..2 {
            let nonce = match answer.nonce.clone() {
                Some(nonce) => nonce,
                None => {
                    let Some((status, body)) = send(&hello, None) else {
                        return;
                    };
                    if status == 404 {
                        answer.off = true;
                        return;
                    }
                    let opened: Value = serde_json::from_slice(&body).unwrap_or_default();
                    let Some(nonce) = opened["nonce"].as_str().filter(|_| status == 200) else {
                        return;
                    };
                    let register = json!({"nonce": nonce, "kind": "register", "id": self.id,
                        "host": "linux", "capabilities": ["pin"],
                        "plan": self.plan, "design": self.design});
                    if send(&report, Some(&register)).map(|r| r.0) != Some(200) {
                        return;
                    }
                    answer.registered = true;
                    answer.nonce = Some(nonce.to_string());
                    nonce.to_string()
                }
            };
            let mut expired = false;
            while !self.reports.is_empty() {
                let mut body = self.reports[0].clone();
                body["nonce"] = nonce.clone().into();
                match send(&report, Some(&body)).map(|r| r.0) {
                    Some(403) => {
                        expired = true;
                        break;
                    }
                    None | Some(500..) => return,
                    Some(_) => {
                        self.reports.remove(0);
                    }
                }
            }
            if !expired {
                let url = format!("{commands}?nonce={}", encode(&nonce));
                match send(&url, None) {
                    Some((200, body)) => {
                        if let Ok(Value::Array(frames)) = serde_json::from_slice(&body) {
                            answer.frames.extend(frames);
                        }
                        return;
                    }
                    Some((403, _)) => {}
                    _ => return,
                }
            }
            answer.nonce = None;
        }
    }
}

#[cfg(test)]
impl Exchange {
    /// The reports this tick would send.
    pub fn reports_for_test(self) -> Vec<Value> {
        self.reports
    }
}

/// A query value's percent-encoding (a nonce is base-36 today; this keeps
/// any other safe).
fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// A `pin` frame's facts; each absent one is what the first pin replaced.
#[derive(Debug, Default, PartialEq, Clone)]
pub(crate) struct Pin {
    pub dark: Option<bool>,
    pub contrast: Option<exact_runner::Contrast>,
    pub mode: Vec<(String, String)>,
    pub root_font_size: Option<f64>,
}

/// A command frame, as the presenter dispatches it.
#[derive(Debug, PartialEq)]
pub(crate) enum Frame {
    /// `pin` `{revision, pin}`: `None` hands every fact back.
    Pin { revision: u64, pin: Option<Pin> },
    /// `inspect`, `outline`, `hold`: reserved (T2b, stage 8), answered
    /// `unsupported`.
    Reserved(String),
}

impl Frame {
    /// Read a frame; `None` for one this host does not dispatch. A fact
    /// with a value the loop would have refused is left unpinned.
    pub fn parse(frame: &Value) -> Option<Frame> {
        match frame["kind"].as_str()? {
            "pin" => {
                let revision = frame["revision"].as_u64().unwrap_or(0);
                let Some(facts) = frame["pin"].as_object() else {
                    return Some(Frame::Pin {
                        revision,
                        pin: None,
                    });
                };
                let mut pin = Pin {
                    dark: match facts.get("scheme").and_then(Value::as_str) {
                        Some("dark") => Some(true),
                        Some("light") => Some(false),
                        _ => None,
                    },
                    contrast: match facts.get("contrast").and_then(Value::as_str) {
                        Some("more") => Some(exact_runner::Contrast::More),
                        Some("less") => Some(exact_runner::Contrast::Less),
                        Some("no-preference") => Some(exact_runner::Contrast::NoPreference),
                        _ => None,
                    },
                    mode: Vec::new(),
                    root_font_size: facts
                        .get("rootFontSize")
                        .and_then(Value::as_f64)
                        .filter(|px| px.is_finite() && *px > 0.0),
                };
                if let Some(mode) = facts.get("mode").and_then(Value::as_object) {
                    for (axis, value) in mode {
                        if let Some(value) = value.as_str() {
                            pin.mode.push((axis.clone(), value.to_string()));
                        }
                    }
                }
                Some(Frame::Pin {
                    revision,
                    pin: Some(pin),
                })
            }
            kind @ ("inspect" | "outline" | "hold") => Some(Frame::Reserved(kind.to_string())),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    #[test]
    fn frames_parse_to_pins_and_reserved_commands() {
        let pin = Frame::parse(&json!({"kind": "pin", "revision": 3, "pin": {
            "scheme": "dark", "contrast": "more", "mode": {"density": "compact"}, "rootFontSize": 20}}));
        assert_eq!(
            pin,
            Some(Frame::Pin {
                revision: 3,
                pin: Some(Pin {
                    dark: Some(true),
                    contrast: Some(exact_runner::Contrast::More),
                    mode: vec![("density".into(), "compact".into())],
                    root_font_size: Some(20.0),
                })
            })
        );
        assert_eq!(
            Frame::parse(&json!({"kind": "pin", "revision": 4, "pin": null})),
            Some(Frame::Pin {
                revision: 4,
                pin: None
            })
        );
        assert_eq!(
            Frame::parse(&json!({"kind": "hold"})),
            Some(Frame::Reserved("hold".into()))
        );
        assert_eq!(Frame::parse(&json!({"kind": "future"})), None);
        assert!(build_id().starts_with("linux-") && build_id().len() <= 64);
    }

    /// A dev loop's channel endpoints: hello hands out nonces, the first
    /// nonce expires after one commands poll, and every report is kept.
    #[test]
    fn an_expired_nonce_is_renewed_and_the_build_registers_again() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let page = format!("http://{}/app/", listener.local_addr().unwrap());
        let posts = Arc::new(Mutex::new(Vec::<Value>::new()));
        let kept = posts.clone();
        std::thread::spawn(move || {
            let mut hellos = 0;
            let mut polls = 0;
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                let mut head = Vec::new();
                let mut byte = [0u8; 1];
                while !head.ends_with(b"\r\n\r\n") && stream.read(&mut byte).unwrap() == 1 {
                    head.push(byte[0]);
                }
                let head = String::from_utf8_lossy(&head).to_string();
                let length: usize = head
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().parse().unwrap())
                    })
                    .unwrap_or(0);
                let mut body = vec![0u8; length];
                stream.read_exact(&mut body).unwrap();
                let path = head.split_whitespace().nth(1).unwrap_or("").to_string();
                let (status, reply) = if path == "/__dev/hello" {
                    hellos += 1;
                    (
                        200,
                        json!({"hello": true, "nonce": format!("n{hellos}")}).to_string(),
                    )
                } else if path == "/__dev/report" {
                    let r: Value = serde_json::from_slice(&body).unwrap();
                    // n1 expires once it has been polled.
                    let fresh = r["nonce"] == format!("n{hellos}") && !(hellos == 1 && polls > 0);
                    kept.lock().unwrap().push(r);
                    if fresh {
                        (200, "{}".into())
                    } else {
                        (403, "{}".into())
                    }
                } else if path.starts_with("/__dev/commands?nonce=n1") && polls == 0 {
                    polls += 1;
                    (
                        200,
                        json!([{"kind": "pin", "revision": 1, "pin": {"scheme": "dark"}}])
                            .to_string(),
                    )
                } else if path == format!("/__dev/commands?nonce=n{hellos}") && hellos > 1 {
                    (200, "[]".into())
                } else {
                    (403, "{}".into())
                };
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
                    reply.len()
                );
            }
        });
        let mut link = Link::new(Some("plan-a".into()));
        link.design = Some("design-a".into());
        let answer = link.exchange().unwrap().run(&page);
        link.settle(answer);
        assert!(link.registered);
        assert_eq!(link.frames.len(), 1);
        assert_eq!(
            Frame::parse(&link.frames[0]),
            Some(Frame::Pin {
                revision: 1,
                pin: Some(Pin {
                    dark: Some(true),
                    ..Pin::default()
                })
            })
        );
        link.registered = false;
        // The loop forgot n1: the status is refused, hello again, register
        // again under the same id, and the status goes on the new nonce.
        link.report(json!({"kind": "status", "scheme": "dark"}));
        let answer = link.exchange().unwrap().run(&page);
        link.settle(answer);
        assert!(link.registered);
        let posts = posts.lock().unwrap();
        let kinds: Vec<_> = posts
            .iter()
            .map(|p| {
                format!(
                    "{}@{}",
                    p["kind"].as_str().unwrap(),
                    p["nonce"].as_str().unwrap()
                )
            })
            .collect();
        assert_eq!(
            kinds,
            ["register@n1", "status@n1", "register@n2", "status@n2"]
        );
        assert_eq!(posts[0]["id"], posts[2]["id"]);
        assert_eq!(posts[0]["host"], "linux");
        assert_eq!(posts[0]["capabilities"], json!(["pin"]));
        assert_eq!(posts[0]["plan"], "plan-a");
        assert_eq!(posts[0]["design"], "design-a");
    }

    /// A report a 5xx or a failed transport did not take goes again next
    /// tick; one a 4xx refused does not; a newer status stands for an older.
    #[test]
    fn reports_survive_a_failed_post() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let page = format!("http://{}/", listener.local_addr().unwrap());
        let posts = Arc::new(Mutex::new(Vec::<String>::new()));
        let kept = posts.clone();
        std::thread::spawn(move || {
            let mut failing = true;
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                let mut head = Vec::new();
                let mut byte = [0u8; 1];
                while !head.ends_with(b"\r\n\r\n") && stream.read(&mut byte).unwrap() == 1 {
                    head.push(byte[0]);
                }
                let head = String::from_utf8_lossy(&head).to_string();
                let length: usize = head
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().parse().unwrap())
                    })
                    .unwrap_or(0);
                let mut body = vec![0u8; length];
                stream.read_exact(&mut body).unwrap();
                let path = head.split_whitespace().nth(1).unwrap_or("").to_string();
                let (status, reply) = match path.as_str() {
                    "/__dev/hello" => (200, json!({"nonce": "n"}).to_string()),
                    "/__dev/report" => {
                        let r: Value = serde_json::from_slice(&body).unwrap();
                        let what = format!(
                            "{}:{}",
                            r["kind"].as_str().unwrap(),
                            r["what"].as_str().unwrap_or("")
                        );
                        kept.lock().unwrap().push(what.clone());
                        if what == "unsupported:bad" {
                            (400, "{}".into())
                        } else if what.starts_with("unsupported:batch") && failing {
                            failing = false;
                            (503, "{}".into())
                        } else {
                            (200, "{}".into())
                        }
                    }
                    _ => (200, "[]".into()),
                };
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
                    reply.len()
                );
            }
        });
        let mut link = Link::new(None);
        link.report(json!({"kind": "unsupported", "what": "bad"}));
        link.report(json!({"kind": "unsupported", "what": "batch"}));
        link.report(json!({"kind": "status", "scheme": "light"}));
        let exchange = link.exchange().unwrap();
        // While the tick runs, a newer status is queued.
        link.report(json!({"kind": "status", "scheme": "dark"}));
        link.settle(exchange.run(&page));
        assert_eq!(
            link.outbox.len(),
            2,
            "the batch report and the newer status"
        );
        let answer = link.exchange().unwrap().run(&page);
        link.settle(answer);
        assert!(link.outbox.is_empty());
        assert_eq!(
            *posts.lock().unwrap(),
            [
                "register:",
                "unsupported:bad",
                "unsupported:batch",
                "unsupported:batch",
                "status:"
            ]
        );
        // A transport failure keeps them too.
        let mut link = Link::new(None);
        link.report(json!({"kind": "status"}));
        let answer = link.exchange().unwrap().run("http://127.0.0.1:9/");
        link.settle(answer);
        assert_eq!(link.outbox.len(), 1);
        for _ in 0..MAX_OUTBOX + 5 {
            link.report(json!({"kind": "unsupported", "what": "x"}));
        }
        assert_eq!(link.outbox.len(), MAX_OUTBOX);
    }

    /// The design snapshot is polled while the channel is off, and once it
    /// is on only when asked: after a registration or a stale batch.
    #[test]
    fn the_snapshot_is_fetched_on_demand_once_the_channel_is_on() {
        let mut link = Link::new(None);
        assert!(link.take_design(), "off: polled");
        link.settle(Answer {
            nonce: Some("n".into()),
            frames: vec![
                json!({"kind": "tamber-batch", "n": 1}),
                json!({"kind": "pin"}),
                json!({"kind": "tamber-snapshot", "n": 2}),
            ],
            registered: true,
            off: false,
            unsent: Vec::new(),
        });
        assert!(link.take_design(), "registered: one catch-up snapshot");
        assert!(!link.take_design());
        link.snapshot = true;
        assert!(link.take_design());
        let live = link.take_live();
        assert_eq!(
            (live[0]["n"].as_u64(), live[1]["n"].as_u64()),
            (Some(1), Some(2))
        );
        assert_eq!(link.frames, [json!({"kind": "pin"})]);
    }

    /// A dev loop answering each request on its own thread, by path; a
    /// `None` reply holds the request open.
    fn serve(reply: impl Fn(&str) -> Option<(u16, String)> + Send + Sync + 'static) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let page = format!("http://{}/", listener.local_addr().unwrap());
        let reply = Arc::new(reply);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                let reply = reply.clone();
                std::thread::spawn(move || {
                    let mut head = Vec::new();
                    let mut byte = [0u8; 1];
                    while !head.ends_with(b"\r\n\r\n") && stream.read(&mut byte).unwrap_or(0) == 1 {
                        head.push(byte[0]);
                    }
                    let head = String::from_utf8_lossy(&head).to_string();
                    let length: usize = head
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse().unwrap())
                        })
                        .unwrap_or(0);
                    let mut body = vec![0u8; length];
                    let _ = stream.read_exact(&mut body);
                    let path = head.split_whitespace().nth(1).unwrap_or("").to_string();
                    let Some((status, body)) = reply(&path) else {
                        std::thread::sleep(Duration::from_secs(30));
                        return;
                    };
                    let _ = write!(
                        stream,
                        "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                });
            }
        });
        page
    }

    /// Poll until `done`, or fail after `within`.
    fn poll_until(
        poller: &mut crate::fetch::Poller,
        within: Duration,
        mut done: impl FnMut(&mut crate::fetch::Poller, bool) -> bool,
    ) {
        let until = Instant::now() + within;
        loop {
            let finished = poller.poll().is_err();
            if done(poller, finished) {
                return;
            }
            assert!(Instant::now() < until, "timed out");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// The commands poll keeps its pace while a plan download stalls: the
    /// loop drops a stream left unpolled 5 s, and the pins sent meanwhile.
    #[test]
    fn a_stalled_plan_download_does_not_stop_the_commands_poll() {
        let polls = Arc::new(Mutex::new(0));
        let counted = polls.clone();
        let page = serve(move |path| match path {
            "/" => None,
            "/__dev/hello" => Some((200, json!({"nonce": "n"}).to_string())),
            "/__dev/report" => Some((200, "{}".into())),
            p if p.starts_with("/__dev/commands") => {
                *counted.lock().unwrap() += 1;
                Some((
                    200,
                    json!([{"kind": "pin", "revision": 1, "pin": null}]).to_string(),
                ))
            }
            _ => Some((503, "{}".into())),
        });
        let mut poller = crate::fetch::Poller::new(page, "{}".into(), None);
        poll_until(&mut poller, Duration::from_secs(4), |_, finished| {
            assert!(!finished, "the plan request is held open");
            *polls.lock().unwrap() >= 3
        });
        let link = poller.link.as_mut().unwrap();
        assert!(link.registered);
        assert!(link.frames.len() >= 2, "each poll's pin is taken");
    }

    /// A snapshot asked for and not answered (a 5xx) is asked for again
    /// next tick, and not again once one came.
    #[test]
    fn a_failed_snapshot_is_asked_for_again() {
        let designs = Arc::new(Mutex::new(0));
        let counted = designs.clone();
        let page = serve(move |path| match path {
            "/__dev/design" => {
                let mut n = counted.lock().unwrap();
                *n += 1;
                Some(if *n == 1 {
                    (503, "{}".into())
                } else {
                    (200, "{\"s\":1}".into())
                })
            }
            p if p.starts_with("/__dev/commands") => Some((200, "[]".into())),
            _ => Some((404, String::new())),
        });
        let mut poller = crate::fetch::Poller::new(page, "{}".into(), None);
        let link = poller.link.as_mut().unwrap();
        link.nonce = Some("n".into());
        link.snapshot = true;
        // The first tick's snapshot fails: still asked for.
        poll_until(&mut poller, Duration::from_secs(4), |_, finished| finished);
        assert_eq!(*designs.lock().unwrap(), 1);
        assert!(poller.link.as_ref().unwrap().snapshot);
        assert_eq!(poller.design, None);
        // The next comes.
        poll_until(&mut poller, Duration::from_secs(4), |_, finished| finished);
        assert_eq!(*designs.lock().unwrap(), 2);
        assert_eq!(poller.design.as_deref(), Some("{\"s\":1}"));
        assert!(!poller.link.as_ref().unwrap().snapshot);
        // And no third.
        poll_until(&mut poller, Duration::from_secs(4), |_, finished| finished);
        assert_eq!(*designs.lock().unwrap(), 2);
    }

    #[test]
    fn a_production_build_holds_no_channel() {
        let production = r#"{"inputs":{"trust":"production"}}"#;
        let poller =
            crate::fetch::Poller::new("http://127.0.0.1:9/".into(), production.into(), None);
        assert!(poller.link.is_none());
        let poller = crate::fetch::Poller::new("http://127.0.0.1:9/".into(), "{}".into(), None);
        assert!(poller.link.is_some());
    }

    #[test]
    fn a_loop_with_no_design_server_leaves_the_channel_quiet() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let page = format!("http://{}/", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(
                b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
        });
        let mut link = Link::new(None);
        let answer = link.exchange().unwrap().run(&page);
        link.settle(answer);
        assert!(!link.registered);
        link.report(json!({"kind": "status"}));
        assert!(link.exchange().is_none(), "quiet after a 404 hello");
        assert!(link.outbox.is_empty());
    }
}
