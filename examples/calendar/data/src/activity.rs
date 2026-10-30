//! The person's activity beside the calendar (LLP 1133.026 §12), read from
//! `@exact/health` through the generated binding while "Show my activity"
//! is on: the steps of each day the month shows, and the selected day's
//! workouts. Only what the answers say becomes state; nothing is kept (D9).
//!
//! A day Health shows nothing for is left out, never shown as 0: on Apple
//! an answer is never `complete`, so an empty day may be one the person did
//! not share (D2, D5).

use crate::dates::{self, DAY_MS};
use crate::modules::health;
use exact_plan::Value;
use exact_runner::{Answer, Outcome};
use std::collections::BTreeMap;

/// A day's workouts rarely fill one page.
const PAGE: f64 = 50.0;

/// A walk over the workouts' pages for one reader.
#[derive(Default)]
pub(crate) struct Walk {
    start: f64,
    end: f64,
    offset: f64,
    workouts: Vec<health::HealthWorkout>,
}

/// The walks in flight, by source and arguments.
pub(crate) type Walks = BTreeMap<String, Walk>;

fn key(source: &str, args: &[Value]) -> String {
    format!("{source}{args:?}")
}

/// `ActivityDays`: each day with a value, and what the answer saw, `since`
/// also as its local date at `offset`.
fn days(
    error: &str,
    days: Vec<(i32, f64)>,
    since: Option<f64>,
    offset: f64,
    complete: bool,
) -> Value {
    let days = (days.into_iter())
        .map(|(day, steps)| Value::record(vec![Value::Number(day.into()), Value::Number(steps)]))
        .collect();
    let from = since.map_or_else(String::new, |ms| {
        let (_, month, day) = dates::civil(local_day(ms, offset));
        format!("{day} {}", dates::MONTH_NAMES[month as usize - 1])
    });
    Value::record(vec![
        Value::str(error),
        Value::list(days),
        Value::Number(since.unwrap_or(0.0)),
        Value::str(&from),
        Value::Bool(complete),
    ])
}

/// `ActivityAgenda`.
fn agenda(error: &str, workouts: Vec<Value>, complete: bool) -> Value {
    Value::record(vec![
        Value::str(error),
        Value::list(workouts),
        Value::Bool(complete),
    ])
}

fn failed(source: &str, code: &str) -> Value {
    match source {
        "calendarActivityDays" => days(code, Vec::new(), None, 0.0, false),
        _ => agenda(code, Vec::new(), false),
    }
}

/// What the build and a first frame show: nothing read yet.
pub(crate) fn unread(source: &str) -> Value {
    failed(source, "unread")
}

fn page(walk: &Walk, after: Option<String>) -> Result<Answer, String> {
    let options = health::HealthWorkoutsOptions {
        start: walk.start,
        end: walk.end,
        limit: Some(PAGE),
        after,
        ..Default::default()
    };
    health::health_workouts(&options)
        .map(Answer::Later)
        .map_err(|e| e.code)
}

/// `calendarActivityDays(month, show, offset)` and
/// `calendarActivityAgenda(start, show, offset)`: nothing while `show` is
/// false; else the first request.
pub(crate) fn answer(walks: &mut Walks, source: &str, args: &[Value]) -> Answer {
    if !matches!(args.get(1), Some(Value::Bool(true))) {
        walks.remove(&key(source, args));
        return Answer::Now(unread(source));
    }
    let number = |i: usize| args.get(i).and_then(Value::as_number).unwrap_or(0.0);
    let offset = number(2);
    if source == "calendarActivityDays" {
        // The six weeks the month shows, from local midnight, one bucket a day.
        let month = number(0) as i32;
        let first = dates::month_first(month);
        let from = first - dates::weekday(first) as i32;
        let last = dates::month_first(month + 1) - 1;
        let to = last + 6 - dates::weekday(last) as i32;
        let shift = offset * 60_000.0;
        let options = health::HealthStatisticsOptions {
            r#type: Some("steps".into()),
            start: from as f64 * DAY_MS - shift,
            end: (to + 1) as f64 * DAY_MS - shift,
            interval: "P1D".into(),
            ..Default::default()
        };
        return health::health_statistics(&options)
            .map(Answer::Later)
            .unwrap_or_else(|e| Answer::Now(failed(source, &e.code)));
    }
    let walk = Walk {
        start: number(0),
        end: number(0) + DAY_MS,
        offset,
        ..Default::default()
    };
    let first = page(&walk, None);
    walks.insert(key(source, args), walk);
    first.unwrap_or_else(|code| Answer::Now(failed(source, &code)))
}

/// A reply: the answer, or the next page's request.
pub(crate) fn parse(walks: &mut Walks, source: &str, args: &[Value], outcome: Outcome) -> Answer {
    let offset = args.get(2).and_then(Value::as_number).unwrap_or(0.0);
    if source == "calendarActivityDays" {
        return Answer::Now(match health::health_statistics_reply(&outcome) {
            Ok(stats) => {
                let seen = (stats.buckets.iter())
                    .filter_map(|b| Some((local_day(b.start, offset), b.value?)))
                    .collect();
                days("", seen, stats.since, offset, stats.complete)
            }
            Err(e) => failed(source, &e.code),
        });
    }
    let key = key(source, args);
    let Some(walk) = walks.get_mut(&key) else {
        return Answer::Now(failed(source, "AbortError"));
    };
    let reply = match health::health_workouts_reply(&outcome) {
        Ok(reply) => reply,
        Err(e) => {
            walks.remove(&key);
            return Answer::Now(failed(source, &e.code));
        }
    };
    walk.workouts.extend(reply.workouts);
    if let Some(after) = reply.next {
        return page(walk, Some(after)).unwrap_or_else(|code| {
            walks.remove(&key);
            Answer::Now(failed(source, &code))
        });
    }
    let walk = walks.remove(&key).unwrap_or_default();
    Answer::Now(agenda("", rows(&walk), reply.complete))
}

/// The local day an instant falls on, at the reader's offset.
fn local_day(ms: f64, offset: f64) -> i32 {
    ((ms + offset * 60_000.0) / DAY_MS).floor() as i32
}

/// The day's workouts as the agenda shows them: the activity, the time and
/// the distance.
fn rows(walk: &Walk) -> Vec<Value> {
    let minute = |ms: f64| {
        let local = ms + walk.offset * 60_000.0;
        ((local - (local / DAY_MS).floor() * DAY_MS) / 60_000.0) as u16
    };
    (walk.workouts.iter())
        .map(|w| {
            let title = match w.activity.as_str() {
                "walking" => "Walk",
                "running" => "Run",
                "hiking" => "Hike",
                "cycling" => "Ride",
                "swimming" => "Swim",
                _ => "Workout",
            };
            let time = format!(
                "{} – {}",
                dates::display_time(minute(w.start)),
                dates::display_time(minute(w.end))
            );
            let distance = (w.distance)
                .map(|m| format!("{:.1} km", m / 1000.0))
                .unwrap_or_default();
            Value::record(vec![
                Value::str(&w.id),
                Value::str(title),
                Value::str(&time),
                Value::str(&distance),
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

    fn record(answer: Answer) -> Vec<Value> {
        match answer {
            Answer::Now(Value::Record(fields)) => fields.to_vec(),
            _ => panic!("expected an answer"),
        }
    }

    const SEP_16: f64 = 20_712.0 * DAY_MS;
    const OCTOBER: f64 = (2026 * 12 + 9) as f64;

    #[test]
    fn the_month_asks_daily_steps_and_leaves_out_the_days_with_none() {
        let mut walks = Walks::new();
        let args = [
            Value::Number(OCTOBER),
            Value::Bool(true),
            Value::Number(0.0),
        ];
        let body = asked(&answer(&mut walks, "calendarActivityDays", &args));
        assert_eq!(body["source"], "healthStatistics");
        assert_eq!(body["args"]["type"], "steps");
        assert_eq!(body["args"]["interval"], "P1D");
        let oct_4 = 20_730.0 * DAY_MS;
        let stats = json!({"type": "steps", "unit": "count", "since": oct_4, "complete": false, "buckets": [
            {"start": oct_4 - DAY_MS, "end": oct_4, "value": null},
            {"start": oct_4, "end": oct_4 + DAY_MS, "value": 0},
            {"start": oct_4 + DAY_MS, "end": oct_4 + 2.0 * DAY_MS, "value": 8412},
        ]});
        let f = record(parse(
            &mut walks,
            "calendarActivityDays",
            &args,
            reply(stats),
        ));
        assert_eq!(f[0], Value::str(""));
        let day = |d: f64, s: f64| Value::record(vec![Value::Number(d), Value::Number(s)]);
        assert_eq!(
            f[1],
            Value::list(vec![day(20_730.0, 0.0), day(20_731.0, 8412.0)])
        );
        assert_eq!(f[2], Value::Number(oct_4));
        assert_eq!(f[3], Value::str("4 October"));
        assert_eq!(f[4], Value::Bool(false));
    }

    #[test]
    fn the_day_walks_the_workout_pages() {
        let mut walks = Walks::new();
        let args = [Value::Number(SEP_16), Value::Bool(true), Value::Number(0.0)];
        let body = asked(&answer(&mut walks, "calendarActivityAgenda", &args));
        assert_eq!(body["source"], "healthWorkouts");
        assert_eq!(body["args"]["end"], SEP_16 + DAY_MS);
        let walk = json!({"id": "w10", "activity": "walking", "start": SEP_16 + 12.0 * 3_600_000.0,
            "end": SEP_16 + 12.5 * 3_600_000.0, "duration": 1800, "distance": 2400, "energy": 105});
        let next = parse(
            &mut walks,
            "calendarActivityAgenda",
            &args,
            reply(json!({"workouts": [walk], "next": "k", "complete": false})),
        );
        assert_eq!(asked(&next)["args"]["after"], "k");
        let swim = json!({"id": "w11", "activity": "swimming", "start": SEP_16 + 18.0 * 3_600_000.0,
            "end": SEP_16 + 19.0 * 3_600_000.0, "duration": 3600});
        let f = record(parse(
            &mut walks,
            "calendarActivityAgenda",
            &args,
            reply(json!({"workouts": [swim], "complete": false})),
        ));
        let row = |v: &Value| match v {
            Value::Record(f) => f
                .iter()
                .map(|x| x.as_str().unwrap().to_owned())
                .collect::<Vec<_>>(),
            _ => panic!("a row"),
        };
        let Value::List(rows) = &f[1] else {
            panic!("rows")
        };
        assert_eq!(
            row(&rows[0])[1..],
            ["Walk", "12:00 PM – 12:30 PM", "2.4 km"]
        );
        assert_eq!(row(&rows[1])[1..], ["Swim", "6:00 PM – 7:00 PM", ""]);
        assert!(walks.is_empty());
    }

    #[test]
    fn off_reads_nothing_and_an_error_answers_its_code() {
        let mut walks = Walks::new();
        let off = [
            Value::Number(SEP_16),
            Value::Bool(false),
            Value::Number(0.0),
        ];
        assert_eq!(
            record(answer(&mut walks, "calendarActivityAgenda", &off))[0],
            Value::str("unread")
        );
        let args = [
            Value::Number(OCTOBER),
            Value::Bool(true),
            Value::Number(0.0),
        ];
        answer(&mut walks, "calendarActivityDays", &args);
        let refused =
            reply(json!({"error": "NotAllowedError", "message": "requestHealthAuthorization"}));
        let f = record(parse(&mut walks, "calendarActivityDays", &args, refused));
        assert_eq!(f[0], Value::str("NotAllowedError"));
    }
}
