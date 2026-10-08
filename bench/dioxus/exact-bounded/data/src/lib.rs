//! Heavy list data, bounded: the 10,000 messages of `data/messages.json` (the copy of
//! `bench/heavy-list/data/messages.json` that `../prepare.sh` writes)
//! (bundled into the binary, never regenerated), the reaction counts the app
//! has changed, and live mode (`BENCH_LIVE=1` in the launch environment).
//!
//! - `feed(cursor)` answers a window (LLP 1027.004 D1): at most `K` messages
//!   starting `K/2` before the message `cursor` names (the top when empty),
//!   with the ids of its first and last rows as the neighbouring cursors. The
//!   empty cursor's window is baked into the plan at build;
//! - `react(id, emoji)` adds one to that message's reaction and answers `{ ok }`;
//!   its mutation refreshes the window;
//! - `config()` answers `{ live }` from the launch environment;
//! - `tick()` (live mode) inserts message `live-k` at the top — the content of
//!   message `(k*37) % 10000`, `minutesAgo` 0 — bumps one reaction on message
//!   `(k*101) % 10000` (the first at or after it that has reactions), and
//!   answers `{ ok }`.
//!
//! Record sharing (LLP 1053 §0, G8): every answer is a fresh list, but its
//! items are the previous answer's `Rc` records for every message that did
//! not change, so the runner's identity checks (`compare::same`) hold for
//! them. A tick builds exactly two records: the inserted `live-k` (which
//! shares its copied content's fields) and the bumped message (which shares
//! every field but its reactions).
#![forbid(unsafe_code)]



use exact_plan::Value;
use exact_runner::{DataError, DataSource};

/// The benchmark's data, as the build found it.
pub const JSON: &[u8] = include_bytes!("../messages.json");

/// One message's content, parsed once.
struct Message {
    id: String,
    author: String,
    avatar: String,
    minutes_ago: f64,
    paragraphs: Value,
    photos: Value,
    link: Value,
    quote: Value,
    reactions: Vec<(String, f64)>,
}

// The JSON parses straight into these typed rows on a background thread, so
// the source never holds a generic `serde_json::Value` tree (~86 MB here).
#[derive(serde::Deserialize)]
struct Doc {
    messages: Vec<Raw>,
}
#[derive(serde::Deserialize)]
struct Raw {
    id: String,
    author: String,
    avatar: String,
    #[serde(rename = "minutesAgo")]
    minutes_ago: f64,
    paragraphs: Vec<Vec<RawRun>>,
    #[serde(default)]
    photos: Vec<RawPhoto>,
    link: Option<RawLink>,
    quote: Option<RawQuote>,
    #[serde(default)]
    reactions: Vec<RawReaction>,
}
#[derive(serde::Deserialize)]
struct RawRun {
    t: String,
    #[serde(default)]
    s: String,
}
#[derive(serde::Deserialize)]
struct RawPhoto {
    src: String,
    w: f64,
    h: f64,
}
#[derive(serde::Deserialize)]
struct RawLink {
    thumb: String,
    title: String,
    description: String,
    site: String,
}
#[derive(serde::Deserialize)]
struct RawQuote {
    author: String,
    excerpt: String,
}
#[derive(serde::Deserialize)]
struct RawReaction {
    emoji: String,
    count: f64,
}

/// `shape Ack`.
fn ack(ok: bool) -> Value {
    Value::record(vec![Value::Bool(ok)])
}

fn some(v: Value) -> Value {
    Value::Option(Some(std::rc::Rc::new(v)))
}

impl Message {
    fn parse(m: Raw) -> Message {
        // Field order is `shape Para` / `shape Run` in app.contract.
        let paragraphs = m
            .paragraphs
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let runs = p
                    .iter()
                    .enumerate()
                    .map(|(j, r)| Value::record(vec![Value::str(&j.to_string()), Value::str(&r.t), Value::str(&r.s)]))
                    .collect();
                Value::record(vec![Value::str(&i.to_string()), Value::list(runs)])
            })
            .collect();
        let photos = match m.photos.first() {
            Some(first) => {
                let at = |i: usize| m.photos.get(i).map(|p| p.src.clone()).unwrap_or_default();
                // `aspect-ratio` is width / height.
                some(Value::record(vec![
                    Value::Number(m.photos.len() as f64),
                    Value::str(&at(0)),
                    Value::str(&at(1)),
                    Value::str(&at(2)),
                    Value::str(&at(3)),
                    Value::Number(first.w / first.h),
                ]))
            }
            None => Value::Option(None),
        };
        // Contract has no `text-transform`; the site line is upper-cased here.
        let link = match &m.link {
            Some(l) => some(Value::record(vec![
                Value::str(&l.thumb),
                Value::str(&l.title),
                Value::str(&l.description),
                Value::str(&l.site.to_uppercase()),
            ])),
            None => Value::Option(None),
        };
        let quote = match &m.quote {
            Some(q) => some(Value::record(vec![Value::str(&q.author), Value::str(&q.excerpt)])),
            None => Value::Option(None),
        };
        let reactions = m.reactions.into_iter().map(|r| (r.emoji, r.count)).collect();
        Message {
            id: m.id,
            author: m.author,
            avatar: m.avatar,
            minutes_ago: m.minutes_ago,
            paragraphs: Value::list(paragraphs),
            photos,
            link,
            quote,
            reactions,
        }
    }

    /// One row. Field order is `shape Message` in app.contract.
    fn value(&self) -> Value {
        Value::record(vec![
            Value::str(&self.id),
            Value::str(&self.author),
            Value::str(&self.avatar),
            Value::Number(self.minutes_ago),
            self.paragraphs.clone(),
            self.photos.clone(),
            self.link.clone(),
            self.quote.clone(),
            self.reactions_value(),
        ])
    }

    fn reactions_value(&self) -> Value {
        Value::list(
            self.reactions
                .iter()
                .map(|(e, c)| Value::record(vec![Value::str(e), Value::Number(*c)]))
                .collect(),
        )
    }
}

/// `row`'s fields (a `shape Message` record), shared.
fn fields(row: &Value) -> &[Value] {
    match row {
        Value::Record(f) => f,
        _ => unreachable!("rows are this source's own records"),
    }
}

/// `row` with field `at` replaced; every other field keeps its allocation.
fn with(row: &Value, changes: &[(usize, Value)]) -> Value {
    let mut f = fields(row).to_vec();
    for (at, v) in changes {
        f[*at] = v.clone();
    }
    Value::record(f)
}

/// The most rows a window holds (LLP 1027.004 D1).
const K: usize = 200;

/// `shape Message` field indices.
const ID: usize = 0;
const MINUTES_AGO: usize = 3;
const REACTIONS: usize = 8;

/// The feed: live messages (newest first) above the 10,000, each with its row value.
struct Feed {
    messages: Vec<Message>,
    rows: Vec<Value>,
    live: Vec<(Message, Value)>,
    ticks: usize,
}

impl Feed {
    fn load(doc: Doc) -> Feed {
        let messages: Vec<Message> = doc.messages.into_iter().map(Message::parse).collect();
        let rows = messages.iter().map(Message::value).collect();
        Feed { messages, rows, live: Vec::new(), ticks: 0 }
    }

    /// The row at display position `d`: live messages (newest first) above the 10,000.
    fn row(&self, d: usize) -> &Value {
        let live = self.live.len();
        if d < live { &self.live[live - 1 - d].1 } else { &self.rows[d - live] }
    }

    /// Where the message `id` shows, if it is in the feed.
    fn position(&self, id: &str) -> Option<usize> {
        let live = self.live.len();
        if let Some(k) = id.strip_prefix("live-").and_then(|k| k.parse::<usize>().ok()) {
            return (1..=live).contains(&k).then(|| live - k);
        }
        let i = id.strip_prefix('m')?.parse::<usize>().ok()?;
        (self.messages.get(i)?.id == id).then_some(live + i)
    }

    /// `shape Window`: at most `K` rows starting `K/2` before `cursor`'s row.
    fn window(&self, cursor: &str) -> Option<Value> {
        let n = self.live.len() + self.rows.len();
        let at = if cursor.is_empty() { 0 } else { self.position(cursor)? };
        let start = at.saturating_sub(K / 2);
        let end = (start + K).min(n);
        let id = |d: usize| fields(self.row(d))[ID].clone();
        Some(Value::record(vec![
            Value::list((start..end).map(|d| self.row(d).clone()).collect()),
            id(start),
            id(end - 1),
            Value::Bool(start > 0),
            Value::Bool(end < n),
        ]))
    }

    fn react(&mut self, id: &str, emoji: &str) -> bool {
        let bump = |m: &mut Message| match m.reactions.iter_mut().find(|(e, _)| e == emoji) {
            Some((_, c)) => {
                *c += 1.0;
                true
            }
            None => false,
        };
        if let Some(i) = id.strip_prefix('m').and_then(|n| n.parse::<usize>().ok()).filter(|&i| i < self.messages.len()) {
            if self.messages[i].id == id && bump(&mut self.messages[i]) {
                self.rows[i] = with(&self.rows[i], &[(REACTIONS, self.messages[i].reactions_value())]);
                return true;
            }
            return false;
        }
        if let Some((m, v)) = self.live.iter_mut().find(|(m, _)| m.id == id) {
            if bump(m) {
                *v = with(v, &[(REACTIONS, m.reactions_value())]);
                return true;
            }
        }
        false
    }

    fn tick(&mut self) {
        let k = self.ticks + 1;
        self.ticks = k;
        let n = self.messages.len();
        let from_at = (k * 37) % n;
        let from = &self.messages[from_at];
        let copy = Message {
            id: format!("live-{k}"),
            author: from.author.clone(),
            avatar: from.avatar.clone(),
            minutes_ago: 0.0,
            paragraphs: from.paragraphs.clone(),
            photos: from.photos.clone(),
            link: from.link.clone(),
            quote: from.quote.clone(),
            reactions: from.reactions.clone(),
        };
        // The copy shares its source row's fields but the id and the time.
        let v = with(&self.rows[from_at], &[(ID, Value::str(&copy.id)), (MINUTES_AGO, Value::Number(0.0))]);
        self.live.push((copy, v));
        let start = (k * 101) % n;
        if let Some(i) = (0..n).map(|d| (start + d) % n).find(|&i| !self.messages[i].reactions.is_empty()) {
            let m = &mut self.messages[i];
            let at = k % m.reactions.len();
            m.reactions[at].1 += 1.0;
            self.rows[i] = with(&self.rows[i], &[(REACTIONS, m.reactions_value())]);
        }
    }
}

/// The source. On a device the feed the list first shows is the plan's
/// baked value; the source's own copy (what a reaction or a live tick edits)
/// is parsed on a background thread started at launch and turned into row
/// values on first use.
pub struct Heavy {
    parsed: Option<std::thread::JoinHandle<Doc>>,
    feed: Option<Feed>,
}

impl Default for Heavy {
    fn default() -> Self {
        // wasm32 has no threads: the web module parses on first use instead.
        #[cfg(target_arch = "wasm32")]
        let parsed = None;
        #[cfg(not(target_arch = "wasm32"))]
        let parsed = Some(std::thread::spawn(|| serde_json::from_slice::<Doc>(JSON).expect("messages.json")));
        Heavy { parsed, feed: None }
    }
}

impl Heavy {
    fn feed(&mut self) -> &mut Feed {
        if self.feed.is_none() {
            let doc = match self.parsed.take() {
                Some(thread) => thread.join().expect("parse thread"),
                None => serde_json::from_slice::<Doc>(JSON).expect("messages.json"),
            };
            self.feed = Some(Feed::load(doc));
        }
        self.feed.as_mut().expect("loaded")
    }
}

impl DataSource for Heavy {
    fn app_id(&self) -> &str {
        "dev.exact.heavybench.bounded"
    }

    /// After first pixel: the web module parses its copy now, not on the
    /// first window shift or tap (native parses on its launch thread).
    fn activate(&mut self) -> Result<(), DataError> {
        #[cfg(target_arch = "wasm32")]
        self.feed();
        Ok(())
    }

    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match (source, args) {
            ("feed", [cursor]) => {
                let Some(cursor) = cursor.as_str() else {
                    return Err(DataError::BadArguments("feed(cursor: string)".into()));
                };
                self.feed().window(cursor).ok_or_else(|| DataError::BadArguments(format!("feed: no message {cursor}")))
            }
            ("react", [id, emoji]) => {
                let (Some(id), Some(emoji)) = (id.as_str(), emoji.as_str()) else {
                    return Err(DataError::BadArguments("react(id: string, emoji: string)".into()));
                };
                Ok(ack(self.feed().react(id, emoji)))
            }
            ("config", []) => {
                let live = std::env::var("BENCH_LIVE").is_ok_and(|v| v == "1");
                if live {
                    // Parse now, not on the first live tick.
                    self.feed();
                }
                Ok(Value::record(vec![Value::Bool(live)]))
            }
            ("tick", []) => {
                self.feed().tick();
                Ok(ack(true))
            }
            ("feed", _) => Err(DataError::BadArguments("feed(cursor)".into())),
            ("config" | "tick", _) => Err(DataError::BadArguments(format!("{source}()"))),
            ("react", _) => Err(DataError::BadArguments("react(id, emoji)".into())),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(v: &Value) -> (Vec<String>, String, String, bool, bool) {
        let f = fields(v);
        let Value::List(rows) = &f[0] else { unreachable!() };
        let ids = rows.iter().map(|r| fields(r)[ID].as_str().unwrap().to_string()).collect();
        let s = |i: usize| f[i].as_str().unwrap().to_string();
        let b = |i: usize| matches!(f[i], Value::Bool(true));
        (ids, s(1), s(2), b(3), b(4))
    }

    #[test]
    fn windows_move_by_cursor_and_edits_stay_bounded() {
        let mut h = Heavy::default();
        let (ids, earlier, later, has_earlier, has_later) = window(&h.query("feed", &[Value::str("")]).unwrap());
        assert_eq!((ids.len(), earlier.as_str(), later.as_str(), has_earlier, has_later), (K, "m0", "m199", false, true));
        // Moving to the last row recentres on it.
        let (ids, earlier, later, has_earlier, _) = window(&h.query("feed", &[Value::str(&later)]).unwrap());
        assert_eq!((ids.len(), earlier.as_str(), later.as_str(), has_earlier), (K, "m99", "m298", true));
        let (ids, _, later, _, has_later) = window(&h.query("feed", &[Value::str("m9999")]).unwrap());
        assert_eq!((ids.len(), later.as_str(), has_later), (101, "m9999", false));
        assert!(h.query("feed", &[Value::str("m10000")]).is_err());
        // A tap answers an acknowledgement; the window shows the new count.
        let m = h.feed().messages.iter().find(|m| !m.reactions.is_empty()).unwrap();
        let (id, (emoji, before)) = (m.id.clone(), m.reactions[0].clone());
        assert_eq!(h.query("react", &[Value::str(&id), Value::str(&emoji)]).unwrap(), ack(true));
        let row = h.feed().position(&id).map(|d| h.feed().row(d).clone()).unwrap();
        let Value::List(r) = &fields(&row)[REACTIONS] else { unreachable!() };
        assert_eq!(fields(&r[0])[1], Value::Number(before + 1.0));
        // A live tick puts `live-1` at the top; a cursor still names its row.
        h.query("tick", &[]).unwrap();
        let (ids, ..) = window(&h.query("feed", &[Value::str("")]).unwrap());
        assert_eq!((ids[0].as_str(), ids[1].as_str()), ("live-1", "m0"));
        let (ids, ..) = window(&h.query("feed", &[Value::str("live-1")]).unwrap());
        assert_eq!(ids[0], "live-1");
    }
}
