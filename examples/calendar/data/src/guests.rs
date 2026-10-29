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
/// A guest's phone number, by the guest's position: a table of its own, so a
/// `guests` table made before phones were picked keeps its schema.
const PHONES: &str = "CREATE TABLE IF NOT EXISTS guest_phones (schedule_id TEXT NOT NULL,position INTEGER NOT NULL,tel TEXT NOT NULL,PRIMARY KEY(schedule_id,position))";

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

/// `addGuests(id, names, emails, tels)`: each name, email and phone number,
/// one per line, after the item's guests.
pub(crate) fn add(calendar: &Calendar, id: &str, names: &str, emails: &str, tels: &str) -> Answer {
    if !calendar.has_event(id) {
        return Answer::Now(added(0, "This item no longer exists."));
    }
    let emails: Vec<&str> = emails.split('\n').collect();
    let tels: Vec<&str> = tels.split('\n').collect();
    let mut commands = vec![execute(TABLE, json!([])), execute(PHONES, json!([]))];
    for (i, name) in names.split('\n').enumerate() {
        let (email, tel) = (
            emails.get(i).copied().unwrap_or_default(),
            tels.get(i).copied().unwrap_or_default(),
        );
        if name.is_empty() && email.is_empty() && tel.is_empty() {
            continue;
        }
        commands.push(execute(
            "INSERT INTO guests(schedule_id,position,name,email) VALUES(?,(SELECT COALESCE(MAX(position),-1)+1 FROM guests WHERE schedule_id = ?),?,?)",
            json!([id, id, name, email]),
        ));
        if !tel.is_empty() {
            commands.push(execute(
                "INSERT INTO guest_phones(schedule_id,position,tel) VALUES(?,(SELECT MAX(position) FROM guests WHERE schedule_id = ?),?)",
                json!([id, id, tel]),
            ));
        }
    }
    sql(true, commands)
}

/// `calendarGuestPhones(id, revision)`: the guests' phone numbers, in the
/// order added. Before the first phone there is no table, which reads as none.
pub(crate) fn read_phones(id: &str) -> Answer {
    if id.is_empty() {
        return Answer::Now(phones(Vec::new()));
    }
    sql(
        false,
        vec![query(
            "SELECT tel FROM guest_phones WHERE schedule_id = ? ORDER BY position",
            json!([id]),
        )],
    )
}

fn phones(tels: Vec<String>) -> Value {
    Value::record(vec![Value::list(
        tels.iter().map(|t| Value::str(t)).collect(),
    )])
}

pub(crate) fn parse_phones(outcome: Outcome) -> Answer {
    let tels = data_storage::response(outcome).and_then(|reply| {
        Ok(rows(&reply)?
            .iter()
            .map(|r| r[0].as_str().unwrap_or_default().to_owned())
            .collect())
    });
    Answer::Now(phones(tels.unwrap_or_default()))
}

/// Where the invitation to item `id` is written: the app's temporary
/// directory, which the next launch empties.
fn invitation_path(id: &str) -> String {
    let safe: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("app:/tmp/invite-{safe}.ics")
}

/// `writeInvitation(id, title, notes, now)`: the item as an iCalendar event
/// (RFC 5545 §3.6.1) in a file a mail can attach; `now`, epoch ms, is its
/// `DTSTAMP`.
pub(crate) fn write_invitation(
    calendar: &Calendar,
    id: &str,
    title: &str,
    notes: &str,
    now: f64,
) -> Answer {
    let Some((all_day, start, end, start_time, end_time)) = calendar.span(id) else {
        return Answer::Now(invitation("", "This item no longer exists."));
    };
    let date = |day: i32| dates::iso(day).replace('-', "");
    let time = |minutes: u16| dates::time(minutes).replace(':', "");
    let now = now.max(0.0) as i64;
    let stamp = format!(
        "{}T{}{:02}Z",
        date((now / 86_400_000) as i32),
        time(((now % 86_400_000) / 60_000) as u16),
        (now % 60_000) / 1000
    );
    let (from, to) = if all_day {
        (
            format!("DTSTART;VALUE=DATE:{}", date(start)),
            format!("DTEND;VALUE=DATE:{}", date(end + 1)),
        )
    } else {
        (
            format!("DTSTART:{}T{}00", date(start), time(start_time)),
            format!("DTEND:{}T{}00", date(end), time(end_time)),
        )
    };
    let lines = [
        "BEGIN:VCALENDAR".to_owned(),
        "VERSION:2.0".into(),
        "PRODID:-//exact2//Calendar example//EN".into(),
        "METHOD:PUBLISH".into(),
        "BEGIN:VEVENT".into(),
        format!("UID:{id}@{}", crate::APP),
        format!("DTSTAMP:{stamp}"),
        from,
        to,
        format!("SUMMARY:{}", ics_text(title)),
    ];
    let mut lines = lines.to_vec();
    if !notes.is_empty() {
        lines.push(format!("DESCRIPTION:{}", ics_text(notes)));
    }
    lines.extend(["END:VEVENT".into(), "END:VCALENDAR".into()]);
    let path = invitation_path(id);
    Answer::Later(data_storage::request(
        "fs.writeFile",
        json!({"path": path, "text": lines.join("\r\n") + "\r\n"}),
    ))
}

/// RFC 5545 §3.3.11's TEXT escapes.
fn ics_text(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace('\n', "\\n")
}

fn invitation(path: &str, message: &str) -> Value {
    Value::record(vec![Value::str(path), Value::str(message)])
}

pub(crate) fn parse_invitation(id: &str, outcome: Outcome) -> Answer {
    Answer::Now(match data_storage::response(outcome) {
        Ok(_) => invitation(&invitation_path(id), ""),
        Err(message) => invitation("", &message),
    })
}

/// How many guests `addGuests` adds: each line with a name, an email or a phone.
fn adding(names: &str, emails: &str, tels: &str) -> usize {
    let (emails, tels): (Vec<&str>, Vec<&str>) =
        (emails.split('\n').collect(), tels.split('\n').collect());
    (names.split('\n').enumerate())
        .filter(|(i, name)| {
            !name.is_empty()
                || emails.get(*i).is_some_and(|e| !e.is_empty())
                || tels.get(*i).is_some_and(|t| !t.is_empty())
        })
        .count()
}

pub(crate) fn parse_add(names: &str, emails: &str, tels: &str, outcome: Outcome) -> Answer {
    Answer::Now(match data_storage::response(outcome) {
        Ok(_) => added(adding(names, emails, tels), ""),
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
