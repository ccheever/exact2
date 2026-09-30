//! Calendar business logic. Contract owns the UI; portable storage owns disk I/O.
//! Month paging reads an in-memory summary index and a nine-layout LRU cache.

#![forbid(unsafe_code)]

mod dates;
mod drag;
mod layout;
mod model;
mod storage;

use exact_plan::Value;
use exact_runner::{Answer, DataError, DataSource, Outcome, Store};
use model::Schedule;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Identity shared by the Contract bake, browser store and Apple app.
pub const APP: &str = "com.exact.calendar";
/// The app needs only its own SQLite file, never the device's calendar database.
pub const GRANTS: &str = "sqlite.open app:/data/calendar.db";

/// A local calendar. Wrap this in `exact_data_host::Storage` in native hosts.
/// Loading summaries is asynchronous; settled month and agenda queries do no I/O.
#[derive(Default)]
pub struct Calendar {
    events: BTreeMap<String, Schedule>,
    index: BTreeMap<i32, Vec<String>>,
    pages: VecDeque<(i32, Value)>,
    revision: u64,
    ready: bool,
    load: Option<storage::Load>,
    mutation: Option<storage::PendingMutation>,
}

impl Calendar {
    fn library(&self, message: &str) -> Value {
        Value::record(vec![
            Value::Bool(self.ready),
            Value::Number(self.revision as f64),
            Value::str(message),
        ])
    }

    fn index_event(&mut self, event: &Schedule) {
        for month in dates::month_of(event.start)..=dates::month_of(event.last_day()) {
            self.index.entry(month).or_default().push(event.id.clone());
        }
    }

    fn replace_events(&mut self, events: BTreeMap<String, Schedule>, revision: u64) {
        self.index.clear();
        for event in events.values() {
            self.index_event(event);
        }
        self.events = events;
        self.pages.clear();
        self.revision = revision;
        self.ready = true;
    }

    fn changed_event(&mut self, id: &str, replacement: Option<Schedule>, revision: u64) {
        let old = self.events.remove(id);
        if let Some(old) = &old {
            for month in dates::month_of(old.start)..=dates::month_of(old.last_day()) {
                if let Some(ids) = self.index.get_mut(&month) {
                    ids.retain(|candidate| candidate != id);
                }
            }
        }
        if let Some(event) = &replacement {
            self.index_event(event);
            self.events.insert(id.to_owned(), event.clone());
        }
        self.index.retain(|_, ids| !ids.is_empty());
        self.pages.retain(|(month, _)| {
            let first = dates::month_first(*month);
            let from = first - dates::weekday(first) as i32;
            let last = dates::month_first(*month + 1) - 1;
            let to = last + 6 - dates::weekday(last) as i32;
            !old.iter()
                .chain(replacement.iter())
                .any(|e| e.start <= to && e.last_day() >= from)
        });
        self.revision = revision;
    }

    fn month_events(&self, month: i32) -> Vec<&Schedule> {
        // Leading/trailing date cells may show the adjacent month's events.
        let mut ids = BTreeSet::new();
        for m in (month - 1)..=(month + 1) {
            if let Some(bucket) = self.index.get(&m) {
                ids.extend(bucket.iter());
            }
        }
        ids.into_iter()
            .filter_map(|id| self.events.get(id))
            .collect()
    }

    fn month(&mut self, month: i32) -> Value {
        if let Some(at) = self.pages.iter().position(|(key, _)| *key == month) {
            let entry = self.pages.remove(at).unwrap();
            let value = entry.1.clone();
            self.pages.push_back(entry);
            return value;
        }
        let page = layout::month(month, &self.month_events(month));
        self.pages.push_back((month, page.clone()));
        while self.pages.len() > 9 {
            self.pages.pop_front();
        }
        page
    }

    fn agenda(&self, day: i32) -> Value {
        let mut events: Vec<_> = self
            .index
            .get(&dates::month_of(day))
            .into_iter()
            .flatten()
            .filter_map(|id| self.events.get(id))
            .filter(|event| event.occurs(day))
            .collect();
        events.sort_by(|a, b| {
            (!a.all_day, a.start_time, a.start, &a.id).cmp(&(
                !b.all_day,
                b.start_time,
                b.start,
                &b.id,
            ))
        });
        Value::record(vec![
            Value::Number(day.into()),
            Value::str(&dates::iso(day)),
            Value::str(&dates::day_label(day)),
            Value::list(events.into_iter().map(|event| event.value("")).collect()),
        ])
    }

    fn opened(&self, id: &str, notes: &str, message: &str) -> Value {
        let event = self.events.get(id);
        Value::record(vec![
            Value::Bool(event.is_some()),
            event.unwrap_or(&model::empty_schedule()).value(notes),
            Value::str(message),
        ])
    }

    fn pure(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        let invalid = |message: String| DataError::Unavailable(message);
        match source {
            "calendarDragInput" => Ok(drag::decode(text(args, 0).map_err(invalid)?)),
            "calendarDragTarget" => {
                let month = whole(args, 0).map_err(invalid)?;
                if !(dates::FIRST_MONTH - 8..=dates::LAST_MONTH + 8).contains(&month) {
                    return Err(invalid("Month is outside the calendar range.".into()));
                }
                Ok(drag::target(&self.month(month), args))
            }
            "calendarClock" => {
                let epoch = number(args, 0).map_err(invalid)?;
                let offset = number(args, 1).map_err(invalid)?;
                if offset.abs() > 1080.0 {
                    return Err(invalid("Invalid UTC offset.".into()));
                }
                let day = ((epoch + offset * 60_000.0) / dates::DAY_MS).floor().clamp(
                    dates::month_first(dates::FIRST_MONTH).into(),
                    (dates::month_first(dates::LAST_MONTH + 1) - 1).into(),
                ) as i32;
                Ok(Value::record(vec![
                    Value::Number(day.into()),
                    Value::Number(dates::month_of(day).into()),
                    Value::str(&dates::iso(day)),
                    Value::Bool(epoch > 0.0),
                ]))
            }
            "calendarMonths" => {
                let center = whole(args, 0).map_err(invalid)?;
                if !(dates::FIRST_MONTH..=dates::LAST_MONTH).contains(&center) {
                    return Err(invalid("Month is outside the calendar range.".into()));
                }
                Ok(Value::list(
                    ((center - 3).max(dates::FIRST_MONTH)..=(center + 3).min(dates::LAST_MONTH))
                        .map(|month| self.month(month))
                        .collect(),
                ))
            }
            "calendarMonth" => {
                let month = whole(args, 0).map_err(invalid)?;
                if !(dates::FIRST_MONTH - 8..=dates::LAST_MONTH + 8).contains(&month) {
                    return Err(invalid("Month is outside the calendar range.".into()));
                }
                Ok(self.month(month))
            }
            "calendarDay" => Ok(self.agenda(whole(args, 0).map_err(invalid)?)),
            "loadCalendar" => Ok(self.library("")),
            "calendarEvent" => Ok(self.opened(text(args, 0).map_err(invalid)?, "", "")),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }
}

impl DataSource for Calendar {
    fn app_id(&self) -> &str {
        APP
    }
    fn grants(&self) -> &str {
        GRANTS
    }

    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        self.pure(source, args)
    }

    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        match source {
            "loadCalendar" => {
                // The bake and a host's first frame have no reported wall clock.
                // An explicit startup action loads once after exactTime reports
                // the launch date, avoiding dependent resource ready callbacks.
                let clock_ready = match args.get(1) {
                    Some(Value::Bool(ready)) => *ready,
                    _ => {
                        return Err(DataError::Unavailable(
                            "The calendar needs a clock-ready value.".into(),
                        ));
                    }
                };
                if !clock_ready {
                    return Ok(Answer::Now(self.library("")));
                }
                store.observe_external_read();
                if self.ready {
                    return Ok(Answer::Now(self.library("")));
                }
                let today = whole(args, 0).map_err(DataError::Unavailable)?;
                if !dates::in_range(today) {
                    return Ok(Answer::Now(
                        self.library("The current date must be between 1900 and 2100."),
                    ));
                }
                // A repeated startup action replaces the old request. Its
                // retired reply will never be parsed, so replace the app's
                // pending read as well instead of waiting for it.
                Ok(self.start_load(source, storage::AfterLoad::Library, today, true))
            }
            "calendarEvent" => {
                let id = text(args, 0).map_err(DataError::Unavailable)?;
                if !self.events.contains_key(id) {
                    return Ok(Answer::Now(self.opened(id, "", "")));
                }
                store.observe_external_read();
                Ok(storage::notes(id))
            }
            "saveSchedule" | "deleteSchedule" | "moveSchedule" => {
                store.observe_external_read();
                Ok(self.begin_mutation(source, args))
            }
            _ => self.pure(source, args).map(Answer::Now),
        }
    }

    fn parse(
        &mut self,
        _: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        if self.load.as_ref().is_some_and(|load| load.source == source) {
            return Ok(self.parse_load(outcome));
        }
        match source {
            "calendarEvent" => {
                let id = text(args, 0).map_err(DataError::Unavailable)?;
                let result = exact_data::storage::response(outcome).and_then(|reply| {
                    let rows = storage::rows(&reply)?;
                    match rows.first() {
                        Some(row) => row[0]
                            .as_str()
                            .map(str::to_owned)
                            .ok_or_else(|| "Invalid schedule notes in storage.".into()),
                        None => Err("This schedule no longer exists.".into()),
                    }
                });
                Ok(Answer::Now(match result {
                    Ok(notes) => self.opened(id, &notes, ""),
                    Err(message) => self.opened(id, "", &message),
                }))
            }
            "saveSchedule" | "deleteSchedule" | "moveSchedule" => Ok(self.parse_mutation(outcome)),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }
}

fn number(args: &[Value], i: usize) -> Result<f64, String> {
    args.get(i)
        .and_then(Value::as_number)
        .filter(|n| n.is_finite())
        .ok_or_else(|| format!("Invalid number at argument {}.", i + 1))
}

fn whole(args: &[Value], i: usize) -> Result<i32, String> {
    let value = number(args, i)?;
    if value.fract() != 0.0 || value < i32::MIN as f64 || value > i32::MAX as f64 {
        return Err(format!("Invalid whole number at argument {}.", i + 1));
    }
    Ok(value as i32)
}

fn text(args: &[Value], i: usize) -> Result<&str, String> {
    args.get(i)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("Invalid text at argument {}.", i + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pager_payloads_stay_bounded_at_both_supported_date_limits() {
        let mut app = Calendar::default();
        for (month, expected) in [
            (
                dates::FIRST_MONTH,
                vec!["1900-01", "1900-02", "1900-03", "1900-04"],
            ),
            (
                dates::LAST_MONTH,
                vec!["2100-09", "2100-10", "2100-11", "2100-12"],
            ),
            (
                dates::month_of(dates::ordinal(2026, 9, 1)),
                vec![
                    "2026-06", "2026-07", "2026-08", "2026-09", "2026-10", "2026-11", "2026-12",
                ],
            ),
        ] {
            let Value::List(pages) = app
                .pure(
                    "calendarMonths",
                    &[Value::Number(month.into()), Value::Number(0.0)],
                )
                .unwrap()
            else {
                panic!("expected month pages");
            };
            let actual: Vec<_> = pages
                .iter()
                .map(|page| {
                    let Value::Record(fields) = page else {
                        panic!("expected month payload")
                    };
                    let Value::List(weeks) = &fields[3] else {
                        panic!("expected complete weeks")
                    };
                    assert!((4..=6).contains(&weeks.len()));
                    fields[0].as_str().unwrap()
                })
                .collect();
            assert_eq!(actual, expected);
            assert!(app.pages.len() <= 9);
        }
    }

    #[test]
    fn month_cache_is_bounded_and_only_affected_pages_are_invalidated() {
        let today = dates::ordinal(2026, 9, 1);
        let mut app = Calendar::default();
        app.replace_events(
            model::samples(today)
                .into_iter()
                .map(|(e, _)| (e.id.clone(), e))
                .collect(),
            1,
        );
        let september = dates::month_of(today);
        for m in (september - 50)..=(september + 50) {
            app.pure(
                "calendarMonths",
                &[Value::Number(m.into()), Value::Number(1.0)],
            )
            .unwrap();
            assert!(app.pages.len() <= 9);
        }
        app.month(september);
        let event = app.events.values().next().unwrap().clone();
        let moved = event.moved(event.start + 45).unwrap();
        app.changed_event(&event.id, Some(moved), 2);
        assert!(!app.pages.iter().any(|(m, _)| *m == september));
        assert!(app.pages.iter().any(|(m, _)| *m > september + 40));
    }
}
