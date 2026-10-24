//! An item's guests, picked from the person's contacts (LLP 1133.018 §11).
//! A pick is the app's copy (D4): names and emails kept in a `guests` table
//! of the app's own SQLite file, made when first used, so existing data keeps
//! its schema. And an item as the system's calendar editor takes it.

use crate::storage::{execute, query, rows, sql};
use crate::{dates, Calendar};
use exact_data::storage as data_storage;
use exact_plan::Value;
use exact_runner::{Answer, Outcome};
use serde_json::json;

const TABLE: &str = "CREATE TABLE IF NOT EXISTS guests (schedule_id TEXT NOT NULL,position INTEGER NOT NULL,name TEXT NOT NULL,email TEXT NOT NULL,PRIMARY KEY(schedule_id,position))";

fn list(guests: Vec<(String, String)>) -> Value {
    Value::record(vec![Value::list(
        (guests.into_iter())
            .map(|(name, email)| Value::record(vec![Value::str(&name), Value::str(&email)]))
            .collect(),
    )])
}

fn added(count: usize, message: &str) -> Value {
    Value::record(vec![Value::Number(count as f64), Value::str(message)])
}

/// `calendarGuests(id, revision)`: the item's guests, in the order added.
pub(crate) fn read(id: &str) -> Answer {
    if id.is_empty() {
        return Answer::Now(list(Vec::new()));
    }
    // A read writes nothing: before the first add there is no table, and the
    // failed query reads as no guests.
    sql(
        false,
        vec![query(
            "SELECT name,email FROM guests WHERE schedule_id = ? ORDER BY position",
            json!([id]),
        )],
    )
}

pub(crate) fn parse_read(outcome: Outcome) -> Answer {
    let guests = data_storage::response(outcome).and_then(|reply| {
        let rows = rows(&reply)?;
        Ok(rows
            .iter()
            .map(|r| {
                let at = |i: usize| r[i].as_str().unwrap_or_default().to_owned();
                (at(0), at(1))
            })
            .collect())
    });
    Answer::Now(list(guests.unwrap_or_default()))
}

/// `addGuests(id, names, emails)`: each name and email, one per line, after
/// the item's guests.
pub(crate) fn add(calendar: &Calendar, id: &str, names: &str, emails: &str) -> Answer {
    if !calendar.has_event(id) {
        return Answer::Now(added(0, "This item no longer exists."));
    }
    let emails: Vec<&str> = emails.split('\n').collect();
    let mut commands = vec![execute(TABLE, json!([]))];
    for (i, name) in names.split('\n').enumerate() {
        let email = emails.get(i).copied().unwrap_or_default();
        if name.is_empty() && email.is_empty() {
            continue;
        }
        commands.push(execute(
            "INSERT INTO guests(schedule_id,position,name,email) VALUES(?,(SELECT COALESCE(MAX(position),-1)+1 FROM guests WHERE schedule_id = ?),?,?)",
            json!([id, id, name, email]),
        ));
    }
    sql(true, commands)
}

pub(crate) fn parse_add(outcome: Outcome) -> Answer {
    Answer::Now(match data_storage::response(outcome) {
        Ok(reply) => added(reply.as_array().map_or(1, Vec::len) - 1, ""),
        Err(message) => added(0, &message),
    })
}

/// `calendarExport(id, revision)`: the stored item as the editor takes it, a
/// LocalDateTime start and an ISO 8601 duration (RFC 8984 §1.4.5, §1.4.6); an
/// all-day item spans its days, both ends included.
pub(crate) fn export(calendar: &Calendar, id: &str) -> Value {
    let Some((all_day, start, end, start_time, end_time)) = calendar.span(id) else {
        return Value::record(vec![Value::str(""), Value::str(""), Value::Bool(false)]);
    };
    let (start, duration) = if all_day {
        (
            format!("{}T00:00:00", dates::iso(start)),
            format!("P{}D", end - start + 1),
        )
    } else {
        let minutes = (end - start) * 1440 + i32::from(end_time) - i32::from(start_time);
        let minutes = minutes.max(0);
        (
            format!("{}T{}:00", dates::iso(start), dates::time(start_time)),
            match (minutes / 60, minutes % 60) {
                (0, m) => format!("PT{m}M"),
                (h, 0) => format!("PT{h}H"),
                (h, m) => format!("PT{h}H{m}M"),
            },
        )
    };
    Value::record(vec![
        Value::str(&start),
        Value::str(&duration),
        Value::Bool(all_day),
    ])
}
