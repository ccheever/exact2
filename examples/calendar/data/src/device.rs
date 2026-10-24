//! The person's calendars beside the app's own (LLP 1133.018 §11), read from
//! `@exact/calendar` through the generated binding while the switch is on:
//! the selected day's events, each in its calendar's colour, and the days of
//! the six weeks a month shows that have an event. A read walks the module's
//! pages (`next`), so only the answer computed from them becomes state (D6).
//!
//! Neither read is made while the switch is off, so the person is asked for
//! access only when they turn it on.

use crate::dates::{self, DAY_MS};
use crate::modules::calendar;
use exact_plan::Value;
use exact_runner::{Answer, Outcome};
use std::collections::{BTreeMap, BTreeSet};

/// One page at a time; a month's six weeks rarely need a second.
const PAGE: f64 = 500.0;

/// A walk over the module's pages for one reader, by its arguments.
#[derive(Default)]
pub(crate) struct Walk {
    /// The range asked, ms since the epoch, and the reader's UTC offset in minutes.
    start: f64,
    end: f64,
    offset: f64,
    /// Each calendar's colour, once `calendars()` answered (the day's walk only).
    colors: Option<BTreeMap<String, String>>,
    events: Vec<calendar::CalendarEvent>,
}

/// The walks in flight, by source and arguments.
pub(crate) type Walks = BTreeMap<String, Walk>;

fn key(source: &str, args: &[Value]) -> String {
    format!("{source}{args:?}")
}

fn days(error: &str, days: Vec<i32>) -> Value {
    Value::record(vec![
        Value::str(error),
        Value::list(days.into_iter().map(|d| Value::Number(d.into())).collect()),
    ])
}

fn agenda(error: &str, events: Vec<Value>) -> Value {
    Value::record(vec![Value::str(error), Value::list(events)])
}

fn page(walk: &Walk, after: Option<String>) -> Result<Answer, String> {
    let options = calendar::CalendarEventsOptions {
        start: walk.start,
        end: walk.end,
        limit: Some(PAGE),
        after,
        ..Default::default()
    };
    calendar::calendar_events(&options)
        .map(Answer::Later)
        .map_err(|e| e.code)
}

/// `calendarDeviceDays(month, show, offset)` and `calendarDeviceAgenda(start,
/// show, offset)`: nothing while `show` is false; else the first request.
pub(crate) fn answer(walks: &mut Walks, source: &str, args: &[Value]) -> Answer {
    let show = matches!(args.get(1), Some(Value::Bool(true)));
    let number = |i: usize| args.get(i).and_then(Value::as_number).unwrap_or(0.0);
    if !show {
        walks.remove(&key(source, args));
        return Answer::Now(failed(source, "unread"));
    }
    let offset = number(2);
    let (start, end) = match source {
        "calendarDeviceDays" => {
            let month = number(0) as i32;
            let first = dates::month_first(month);
            let from = first - dates::weekday(first) as i32;
            let last = dates::month_first(month + 1) - 1;
            let to = last + 6 - dates::weekday(last) as i32;
            let shift = offset * 60_000.0;
            (
                from as f64 * DAY_MS - shift,
                (to + 1) as f64 * DAY_MS - shift,
            )
        }
        _ => (number(0), number(0) + DAY_MS),
    };
    let walk = Walk {
        start,
        end,
        offset,
        ..Default::default()
    };
    let first = match source {
        "calendarDeviceAgenda" => Ok(Answer::Later(calendar::calendars())),
        _ => page(&walk, None),
    };
    walks.insert(key(source, args), walk);
    first.unwrap_or_else(|code| Answer::Now(failed(source, &code)))
}

fn failed(source: &str, code: &str) -> Value {
    match source {
        "calendarDeviceDays" => days(code, Vec::new()),
        _ => agenda(code, Vec::new()),
    }
}

/// A reply in a walk: the next request, or the answer.
pub(crate) fn parse(walks: &mut Walks, source: &str, args: &[Value], outcome: Outcome) -> Answer {
    let key = key(source, args);
    let Some(walk) = walks.get_mut(&key) else {
        return Answer::Now(failed(source, "AbortError"));
    };
    if walk.colors.is_none() && source == "calendarDeviceAgenda" {
        let next = calendar::calendars_reply(&outcome).map(|list| {
            walk.colors = Some(
                (list.calendars.into_iter())
                    .map(|c| (c.id, c.color))
                    .collect(),
            );
        });
        let next = next.map_err(|e| e.code).and_then(|()| page(walk, None));
        return next.unwrap_or_else(|code| {
            walks.remove(&key);
            Answer::Now(failed(source, &code))
        });
    }
    let reply = match calendar::calendar_events_reply(&outcome) {
        Ok(reply) => reply,
        Err(e) => {
            walks.remove(&key);
            return Answer::Now(failed(source, &e.code));
        }
    };
    walk.events.extend(reply.events);
    if let Some(after) = reply.next {
        return page(walk, Some(after)).unwrap_or_else(|code| {
            walks.remove(&key);
            Answer::Now(failed(source, &code))
        });
    }
    let walk = walks.remove(&key).unwrap_or_default();
    Answer::Now(match source {
        "calendarDeviceDays" => days("", marked(&walk)),
        _ => agenda("", rows(&walk)),
    })
}

/// The local day an instant falls on, at the reader's offset.
fn local_day(ms: f64, offset: f64) -> i32 {
    ((ms + offset * 60_000.0) / DAY_MS).floor() as i32
}

/// Each day of the range an event touches.
fn marked(walk: &Walk) -> Vec<i32> {
    let first = local_day(walk.start, walk.offset);
    let last = local_day(walk.end - 1.0, walk.offset);
    let mut out = BTreeSet::new();
    for e in &walk.events {
        let (Some(start), Some(end)) = (e.utc_start, e.utc_end) else {
            continue;
        };
        let from = local_day(start, walk.offset).max(first);
        let to = local_day((end - 1.0).max(start), walk.offset).min(last);
        out.extend(from..=to);
    }
    out.into_iter().collect()
}

/// The day's events as the agenda section shows them, in the module's order.
fn rows(walk: &Walk) -> Vec<Value> {
    let colors = walk.colors.clone().unwrap_or_default();
    let minute = |ms: f64| {
        let local = ms + walk.offset * 60_000.0;
        (local - (local / DAY_MS).floor() * DAY_MS) / 60_000.0
    };
    (walk.events.iter())
        .map(|e| {
            let start = e.utc_start.unwrap_or(walk.start);
            let end = e.utc_end.unwrap_or(start);
            let time = if e.show_without_time || (start <= walk.start && end >= walk.end) {
                "All day".to_owned()
            } else {
                format!(
                    "{} – {}",
                    dates::display_time(minute(start.max(walk.start)) as u16),
                    dates::display_time(minute(end.min(walk.end - 60_000.0)) as u16)
                )
            };
            let id = format!("{}@{}", e.id.as_deref().unwrap_or(""), start);
            let color = (e.calendar_id.as_ref())
                .and_then(|c| colors.get(c))
                .cloned()
                .unwrap_or_else(|| "#8e8e93".into());
            Value::record(vec![
                Value::str(&id),
                Value::str(&e.title),
                Value::str(&time),
                Value::str(&color),
            ])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_runner::Response;
    use serde_json::{json, Value as Json};

    fn reply(value: Json) -> Outcome {
        Outcome::Response(Response {
            status: 200,
            headers: Vec::new(),
            body: value.to_string().into_bytes(),
        })
    }

    fn asked(answer: &Answer) -> Json {
        let Answer::Later(request) = answer else {
            panic!("expected a request")
        };
        serde_json::from_slice(&request.body).unwrap()
    }

    fn event(id: &str, calendar: &str, start: f64, end: f64, all_day: bool) -> Json {
        json!({"id": id, "calendarId": calendar, "title": id, "start": "", "duration": "PT1H",
            "showWithoutTime": all_day, "status": "confirmed", "freeBusyStatus": "busy",
            "utcStart": start, "utcEnd": end})
    }

    fn record(answer: Answer) -> Vec<Value> {
        match answer {
            Answer::Now(Value::Record(fields)) => fields.to_vec(),
            _ => panic!("expected an answer"),
        }
    }

    const SEP_16: f64 = 20_712.0 * DAY_MS;
    const HOUR: f64 = 3_600_000.0;
    const SEPTEMBER: f64 = (2026 * 12 + 8) as f64;

    #[test]
    fn the_day_walks_the_calendars_then_every_page() {
        let mut walks = Walks::new();
        let args = [Value::Number(SEP_16), Value::Bool(true), Value::Number(0.0)];
        let source = "calendarDeviceAgenda";
        assert_eq!(
            asked(&answer(&mut walks, source, &args))["source"],
            "calendars"
        );
        let home = json!({"id": "home", "title": "Home", "color": "#3b82f6", "writable": true, "isDefault": true, "source": "On My Mac"});
        let calendars = json!({"calendars": [home]});
        let body = asked(&parse(&mut walks, source, &args, reply(calendars)));
        assert_eq!(body["source"], "calendarEvents");
        assert_eq!(body["args"]["start"], SEP_16);
        assert_eq!(body["args"]["end"], SEP_16 + DAY_MS);
        let timed = event(
            "a",
            "home",
            SEP_16 + 9.0 * HOUR,
            SEP_16 + 10.5 * HOUR,
            false,
        );
        let one = json!({"events": [timed], "next": "k1"});
        let next = parse(&mut walks, source, &args, reply(one));
        assert_eq!(asked(&next)["args"]["after"], "k1");
        let spanning = event("b", "gone", SEP_16 - DAY_MS, SEP_16 + DAY_MS, true);
        let done = record(parse(
            &mut walks,
            source,
            &args,
            reply(json!({"events": [spanning]})),
        ));
        assert_eq!(done[0], Value::str(""));
        let Value::List(rows) = &done[1] else {
            panic!("rows")
        };
        let row = |i: usize| match &rows[i] {
            Value::Record(f) => (f.iter())
                .map(|v| v.as_str().unwrap().to_owned())
                .collect::<Vec<_>>(),
            _ => panic!("a row"),
        };
        assert_eq!(row(0)[2..], ["9:00 AM – 10:30 AM", "#3b82f6"]);
        assert_eq!(row(1)[2..], ["All day", "#8e8e93"]);
        assert!(walks.is_empty());
    }

    #[test]
    fn a_module_error_answers_its_code_and_off_reads_nothing() {
        let mut walks = Walks::new();
        let off = [
            Value::Number(SEP_16),
            Value::Bool(false),
            Value::Number(0.0),
        ];
        let f = record(answer(&mut walks, "calendarDeviceAgenda", &off));
        assert_eq!(f[0], Value::str("unread"));
        let args = [
            Value::Number(SEPTEMBER),
            Value::Bool(true),
            Value::Number(-420.0),
        ];
        let first = answer(&mut walks, "calendarDeviceDays", &args);
        // The six weeks September 2026 shows begin Sunday 30 August, at UTC−7.
        let start = 20_695.0 * DAY_MS + 420.0 * 60_000.0;
        assert_eq!(asked(&first)["args"]["start"], start);
        let refused = reply(json!({"error": "NotAllowedError"}));
        let f = record(parse(&mut walks, "calendarDeviceDays", &args, refused));
        assert_eq!(f[0], Value::str("NotAllowedError"));
        assert!(walks.is_empty());
    }

    #[test]
    fn a_month_marks_each_local_day_an_event_touches() {
        let mut walks = Walks::new();
        let args = [
            Value::Number(SEPTEMBER),
            Value::Bool(true),
            Value::Number(0.0),
        ];
        answer(&mut walks, "calendarDeviceDays", &args);
        // One ending at midnight marks one day; one over two days marks both.
        let page = json!({"events": [
            event("a", "home", SEP_16 + 23.0 * HOUR, SEP_16 + DAY_MS, false),
            event("b", "home", SEP_16 + 2.0 * DAY_MS, SEP_16 + 4.0 * DAY_MS, true),
        ]});
        let f = record(parse(&mut walks, "calendarDeviceDays", &args, reply(page)));
        let days = [20_712.0, 20_714.0, 20_715.0].map(Value::Number).to_vec();
        assert_eq!(f[1], Value::list(days));
    }
}
