//! Heavy list data: the 10,000 messages of `data/messages.json` (the copy of
//! `bench/heavy-list/data/messages.json` that `../prepare.sh` writes; bundled
//! into the binary), the reaction counts the app has changed, and live mode
//! (`BENCH_LIVE=1` in the launch environment).
//!
//! - `messages()` answers the feed (baked into the plan at build);
//! - `react(id, emoji)` adds one to that message's reaction and answers the feed;
//! - `config()` answers `{ live }` from the launch environment;
//! - `tick()` (live mode) inserts message `live-k` at the top — the content of
//!   message `(k*37) % 10000`, `minutesAgo` 0 — bumps one reaction on message
//!   `(k*101) % 10000` (the first at or after it that has reactions), and
//!   answers the feed.
//!
//! Record sharing (LLP 1053 §0, G8): every answer is a fresh list, but its
//! items are the previous answer's shared records for every message that did
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

// The fallback parser uses typed rows, never a generic serde_json::Value tree.
// Device boot adopts the already-decoded baked Value and does not parse again.
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
                    .map(|(j, r)| {
                        Value::record(vec![
                            Value::str(&j.to_string()),
                            Value::str(&r.t),
                            Value::str(&r.s),
                        ])
                    })
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
            Some(q) => some(Value::record(vec![
                Value::str(&q.author),
                Value::str(&q.excerpt),
            ])),
            None => Value::Option(None),
        };
        let reactions = m
            .reactions
            .into_iter()
            .map(|r| (r.emoji, r.count))
            .collect();
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

/// `shape Message` field indices.
const ID: usize = 0;
const MINUTES_AGO: usize = 3;
const REACTIONS: usize = 8;

/// Retain the runner's exact outer lease until the first mutation, so its
/// compiled-resource release check also sees the source's ownership.
enum Rows {
    Adopted(Value),
    Owned(Vec<Value>),
}

impl Rows {
    fn values(&self) -> &[Value] {
        match self {
            Rows::Adopted(value) => match &fields(value)[0] {
                Value::List(rows) => rows,
                _ => unreachable!("adopted feed was validated"),
            },
            Rows::Owned(rows) => rows,
        }
    }
    fn make_owned(&mut self) -> &mut Vec<Value> {
        if matches!(self, Rows::Adopted(_)) {
            *self = Rows::Owned(self.values().to_vec());
        }
        match self {
            Rows::Owned(rows) => rows,
            _ => unreachable!(),
        }
    }
}

/// The feed owns shared row values, including the baked answer the runner adopts.
struct Feed {
    rows: Rows,
    live: Vec<Value>,
    ticks: usize,
}

impl Feed {
    fn load(doc: Doc) -> Feed {
        let rows = doc
            .messages
            .into_iter()
            .map(|m| Message::parse(m).value())
            .collect();
        Feed {
            rows: Rows::Owned(rows),
            live: Vec::new(),
            ticks: 0,
        }
    }

    fn adopted(value: &Value) -> Option<Feed> {
        let Value::Record(fields) = value else {
            return None;
        };
        let [Value::List(_)] = &**fields else {
            return None;
        };
        Some(Feed {
            rows: Rows::Adopted(value.clone()),
            live: Vec::new(),
            ticks: 0,
        })
    }

    fn value(&self) -> Value {
        let mut rows = Vec::with_capacity(self.live.len() + self.rows.values().len());
        rows.extend(self.live.iter().rev().cloned());
        rows.extend(self.rows.values().iter().cloned());
        Value::record(vec![Value::list(rows)])
    }

    fn reactions(row: &Value) -> &[Value] {
        match &fields(row)[REACTIONS] {
            Value::List(items) => items,
            _ => unreachable!("rows are this source's own records"),
        }
    }

    fn bump(row: &Value, at: usize) -> Value {
        let mut reactions = Self::reactions(row).to_vec();
        let count = fields(&reactions[at])[1]
            .as_number()
            .expect("reaction count");
        reactions[at] = with(&reactions[at], &[(1, Value::Number(count + 1.0))]);
        with(row, &[(REACTIONS, Value::list(reactions))])
    }

    fn react(&mut self, id: &str, emoji: &str) -> bool {
        let regular = id
            .strip_prefix('m')
            .and_then(|n| n.parse::<usize>().ok())
            .filter(|&i| {
                i < self.rows.values().len()
                    && fields(&self.rows.values()[i])[ID].as_str() == Some(id)
            });
        let (live, index) = if let Some(i) = regular {
            (false, i)
        } else if let Some(i) = self
            .live
            .iter()
            .position(|row| fields(row)[ID].as_str() == Some(id))
        {
            (true, i)
        } else {
            return false;
        };
        let row = if live {
            &self.live[index]
        } else {
            &self.rows.values()[index]
        };
        let Some(at) = Self::reactions(row)
            .iter()
            .position(|r| fields(r)[0].as_str() == Some(emoji))
        else {
            return false;
        };
        let changed = Self::bump(row, at);
        if live {
            self.live[index] = changed
        } else {
            self.rows.make_owned()[index] = changed
        }
        true
    }

    fn tick(&mut self) {
        let k = self.ticks + 1;
        self.ticks = k;
        let n = self.rows.values().len();
        let from = &self.rows.values()[(k * 37) % n];
        self.live.push(with(
            from,
            &[
                (ID, Value::str(&format!("live-{k}"))),
                (MINUTES_AGO, Value::Number(0.0)),
            ],
        ));
        let start = (k * 101) % n;
        if let Some(i) = (0..n)
            .map(|d| (start + d) % n)
            .find(|&i| !Self::reactions(&self.rows.values()[i]).is_empty())
        {
            let at = k % Self::reactions(&self.rows.values()[i]).len();
            let changed = Self::bump(&self.rows.values()[i], at);
            self.rows.make_owned()[i] = changed;
        }
    }
}

/// Runtime boot adopts the runner's decoded baked resource. Baking and a source
/// without a compiled answer parse on its first messages request instead.
#[derive(Default)]
pub struct Heavy {
    feed: Option<Feed>,
}

impl Heavy {
    fn feed(&mut self) -> &mut Feed {
        self.feed.get_or_insert_with(|| {
            Feed::load(serde_json::from_slice::<Doc>(JSON).expect("messages.json"))
        })
    }
}

impl DataSource for Heavy {
    fn adopt(&mut self, source: &str, args: &[Value], value: &Value) {
        if self.feed.is_none() && source == "messages" && args.is_empty() {
            self.feed = Feed::adopted(value);
        }
    }

    fn app_id(&self) -> &str {
        "dev.exact.heavybench.exact"
    }

    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match (source, args) {
            ("messages", []) => Ok(self.feed().value()),
            ("react", [id, emoji]) => {
                let (Some(id), Some(emoji)) = (id.as_str(), emoji.as_str()) else {
                    return Err(DataError::BadArguments(
                        "react(id: string, emoji: string)".into(),
                    ));
                };
                let feed = self.feed();
                feed.react(id, emoji);
                Ok(feed.value())
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
                let feed = self.feed();
                feed.tick();
                Ok(feed.value())
            }
            ("messages" | "config" | "tick", _) => {
                Err(DataError::BadArguments(format!("{source}()")))
            }
            ("react", _) => Err(DataError::BadArguments("react(id, emoji)".into())),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }
}

#[cfg(test)]
mod tests;
