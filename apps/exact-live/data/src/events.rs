//! The job wave's progress as it happens (LLP 1016.000; LLP 1069.004 slice
//! 2): one server-sent event stream per wave, replacing a poll. Each event
//! carries the wave's counts and the log entries since this reader's cursor.
//! The host keeps only the newest undelivered event (coalescing, D4); a gap
//! in the log re-asks from the cursor with `Last-Event-ID` (D6), and the
//! server answers every entry after it in one event. "On trial" (1069.004
//! Decided 3): the counters here are what the smoke measures.
use completion_storm_data::CONTROL;
use exact_plan::Value;
use exact_runner::{Answer, DataError, Message, Outcome, Request};

/// One wave's reading: the server's counts, and the log as far as it is
/// whole. `cursor` is the last entry taken; `server` the newest the server
/// said it had.
#[derive(Default)]
struct Log {
    wave: u64,
    status: String,
    counts: [f64; 4],
    cursor: u64,
    server: u64,
    entries: u64,
    messages: u64,
    coalesced: u64,
    gaps: u64,
    reasks: u64,
    last: String,
}

/// `jobEvents(wave)`: the open stream's reading, or `idle` with no wave.
#[derive(Default)]
pub struct Events {
    log: Log,
}

fn unavailable(message: impl Into<String>) -> DataError {
    DataError::Unavailable(message.into())
}

impl Events {
    fn wave(args: &[Value]) -> Result<u64, DataError> {
        match args.first() {
            Some(Value::Number(n)) if *n >= 0. && n.fract() == 0. => Ok(*n as u64),
            _ => Err(DataError::BadArguments("jobEvents(wave)".into())),
        }
    }

    /// The reading for `wave`, starting over when the wave is new.
    fn log(&mut self, wave: u64) -> &mut Log {
        if self.log.wave != wave {
            self.log = Log {
                wave,
                ..Log::default()
            };
        }
        &mut self.log
    }

    /// Open (or reopen, from the cursor) the stream for a wave.
    fn open(log: &mut Log, status: &str) -> Answer {
        log.status = status.into();
        let mut request = Request::get(&format!("{CONTROL}/api/events?wave={}", log.wave))
            .independent_http(64 << 10);
        if log.cursor > 0 {
            request = request.header("last-event-id", &log.cursor.to_string());
        }
        Answer::stream(request)
    }

    /// Ask: the stream, or `idle` for no wave.
    pub fn answer(&mut self, args: &[Value]) -> Result<Answer, DataError> {
        let wave = Self::wave(args)?;
        if wave == 0 {
            return Ok(Answer::Now(self.log(0).value("idle")));
        }
        let log = self.log(wave);
        let status = if log.cursor > 0 {
            "resuming"
        } else {
            "connecting"
        };
        Ok(Self::open(log, status))
    }

    /// One message, or what ended the stream.
    pub fn parse(&mut self, args: &[Value], outcome: Outcome) -> Result<Answer, DataError> {
        let log = self.log(Self::wave(args)?);
        let status = match outcome {
            Outcome::Message(m) => return log.message(m),
            Outcome::Failed { message, .. } if message == "the event stream ended" => {
                "ended".to_string()
            }
            Outcome::Failed { message, .. } => format!("disconnected: {message}"),
            Outcome::Response(r) => format!("HTTP {}", r.status),
            _ => return Err(unavailable("jobEvents: not an HTTP answer")),
        };
        Ok(Answer::Now(log.value(&status)))
    }
}

impl Log {
    fn message(&mut self, m: Message) -> Result<Answer, DataError> {
        self.messages += 1;
        self.coalesced += u64::from(m.coalesced);
        let json = Json::parse(&m.data).ok_or_else(|| unavailable("jobEvents: malformed event"))?;
        self.server = self.server.max(m.id.parse().unwrap_or(0));
        let entries = json.entries();
        // A gap: what came is past the cursor. Re-ask from it; the server
        // sends everything after it in one event (LLP 1016.000 D6).
        if entries.first().is_some_and(|e| e.0 > self.cursor + 1) {
            self.gaps += 1;
            self.reasks += 1;
            return Ok(Events::open(self, "re-asking from cursor"));
        }
        for (seq, lane, what) in entries {
            if seq <= self.cursor {
                continue;
            }
            self.cursor = seq;
            self.entries += 1;
            self.last = if lane < 0. {
                format!("#{seq} {what}")
            } else {
                format!("#{seq} lane {lane} {what}")
            };
        }
        for (i, key) in ["count", "received", "held", "finished"].iter().enumerate() {
            self.counts[i] = json.number(key);
        }
        Ok(Answer::Now(self.value("live")))
    }

    fn value(&mut self, status: &str) -> Value {
        self.status = status.into();
        let n = |v: u64| Value::Number(v as f64);
        Value::record(vec![
            n(self.wave),
            Value::str(&self.status),
            Value::Number(self.counts[0]),
            Value::Number(self.counts[1]),
            Value::Number(self.counts[2]),
            Value::Number(self.counts[3]),
            n(self.entries),
            n(self.server),
            n(self.messages),
            n(self.coalesced),
            n(self.gaps),
            n(self.reasks),
            Value::str(&self.last),
        ])
    }
}

/// The few JSON forms this fixture sends: numbers by key, and
/// `"entries":[{"seq":n,"lane":n,"what":"…"}, …]`. The app has no JSON
/// dependency and needs none for these.
struct Json<'a>(&'a str);

impl<'a> Json<'a> {
    fn parse(text: &'a str) -> Option<Self> {
        (text.starts_with('{') && text.ends_with('}')).then_some(Json(text))
    }

    fn number_in(text: &str, key: &str) -> f64 {
        let needle = format!("\"{key}\":");
        text.find(&needle)
            .map(|at| &text[at + needle.len()..])
            .and_then(|rest| {
                let end = rest
                    .find(|c: char| !(c.is_ascii_digit() || c == '-' || c == '.'))
                    .unwrap_or(rest.len());
                rest[..end].parse().ok()
            })
            .unwrap_or(0.)
    }

    fn number(&self, key: &str) -> f64 {
        let head = self.0.split("\"entries\"").next().unwrap_or("");
        Self::number_in(head, key)
    }

    fn entries(&self) -> Vec<(u64, f64, String)> {
        let Some(list) = self.0.split("\"entries\":[").nth(1) else {
            return vec![];
        };
        list.split('{')
            .skip(1)
            .map(|item| {
                let what = item
                    .split("\"what\":\"")
                    .nth(1)
                    .and_then(|w| w.split('"').next())
                    .unwrap_or("")
                    .to_string();
                (
                    Self::number_in(item, "seq") as u64,
                    Self::number_in(item, "lane"),
                    what,
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(id: u64, data: &str, coalesced: u32) -> Outcome {
        Outcome::Message(Message {
            id: id.to_string(),
            data: data.into(),
            coalesced,
            ..Message::default()
        })
    }

    fn event(seqs: std::ops::RangeInclusive<u64>) -> String {
        let entries: Vec<String> = seqs
            .map(|s| format!(r#"{{"seq":{s},"lane":{},"what":"done"}}"#, s % 7))
            .collect();
        format!(
            r#"{{"wave":3,"count":8,"received":8,"held":2,"finished":6,"released":true,"entries":[{}]}}"#,
            entries.join(",")
        )
    }

    fn field(v: &Answer, i: usize) -> Value {
        let Answer::Now(Value::Record(fields)) = v else {
            panic!("a reading: {v:?}")
        };
        fields[i].clone()
    }

    fn header(a: &Answer) -> Option<String> {
        let Answer::Later(r) = a else { return None };
        assert!(r.stream);
        r.headers
            .iter()
            .find(|(k, _)| k == "last-event-id")
            .map(|(_, v)| v.clone())
    }

    #[test]
    fn a_gap_re_asks_from_the_cursor_and_the_replay_makes_it_whole() {
        let mut e = Events::default();
        let args = [Value::Number(3.)];
        let first = e.answer(&args).unwrap();
        assert_eq!(header(&first), None, "a first ask has no cursor");
        let a = e.parse(&args, message(2, &event(1..=2), 0)).unwrap();
        assert_eq!(field(&a, 6), Value::Number(2.), "two entries");
        // Entries 3 and 4 were coalesced away: 5 arrives past the cursor.
        let gap = e.parse(&args, message(5, &event(5..=5), 2)).unwrap();
        assert_eq!(header(&gap).as_deref(), Some("2"), "re-ask from the cursor");
        let whole = e.parse(&args, message(5, &event(3..=5), 0)).unwrap();
        assert_eq!(field(&whole, 6), Value::Number(5.), "3, 4 and 5 once each");
        assert_eq!(field(&whole, 7), Value::Number(5.), "the server's newest");
        assert_eq!(field(&whole, 10), Value::Number(1.), "one gap");
        assert_eq!(field(&whole, 9), Value::Number(2.), "two coalesced");
        assert_eq!(field(&whole, 3), Value::Number(8.), "received");
        assert_eq!(field(&whole, 12), Value::str("#5 lane 5 done"));
        // A dropped connection keeps the reading; a reconnect resumes.
        let ended = e
            .parse(
                &args,
                Outcome::Failed {
                    kind: exact_runner::FailureKind::Network,
                    message: "the event stream ended".into(),
                },
            )
            .unwrap();
        assert_eq!(field(&ended, 1), Value::str("ended"));
        assert_eq!(header(&e.answer(&args).unwrap()).as_deref(), Some("5"));
        assert_eq!(
            field(&e.answer(&[Value::Number(0.)]).unwrap(), 1),
            Value::str("idle")
        );
    }
}
