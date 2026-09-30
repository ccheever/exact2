//! Portable SQLite continuations. Layout queries never enter this module.

use crate::{dates, model, text, whole, Calendar, Schedule};
use exact_data::storage;
use exact_plan::Value;
use exact_runner::{Answer, Outcome};
use serde_json::{json, Value as Json};
use std::collections::BTreeMap;

const PATH: &str = "app:/data/calendar.db";
const PAGE: usize = 256;
const SUMMARY: &str = "SELECT id,title,color,all_day,start_day,end_day,start_minute,end_minute FROM schedules WHERE id > ? ORDER BY id LIMIT 256";
const REVISION: &str = "SELECT value FROM calendar_meta WHERE key='revision'";
const INSERT: &str = "INSERT INTO schedules(id,title,notes,color,all_day,start_day,end_day,start_minute,end_minute) VALUES(?,?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET title=excluded.title,notes=excluded.notes,color=excluded.color,all_day=excluded.all_day,start_day=excluded.start_day,end_day=excluded.end_day,start_minute=excluded.start_minute,end_minute=excluded.end_minute";

pub(crate) enum AfterLoad {
    Library,
    ResumeMutation(Intent),
}

enum LoadStep {
    Schema,
    Initialized,
    Page,
    Verify,
}

pub(crate) struct Load {
    pub source: String,
    after: AfterLoad,
    step: LoadStep,
    today: i32,
    revision: Option<u64>,
    events: BTreeMap<String, Schedule>,
    restarts: u8,
}

#[derive(Clone)]
enum Edit {
    Save {
        event: Schedule,
        notes: String,
        create: bool,
    },
    Delete(String),
    Move {
        id: String,
        day: i32,
    },
}

pub(crate) struct Intent {
    source: String,
    token: String,
    fingerprint: String,
    edit: Edit,
}

pub(crate) struct Applied {
    event: Option<Schedule>,
    remove: bool,
    id: String,
    day: i32,
    revision: u64,
}

pub(crate) enum PendingMutation {
    Lookup(Intent),
    Write(Applied),
}

fn sql(transaction: bool, commands: Vec<Json>) -> Answer {
    Answer::Later(storage::request(
        if transaction {
            "sqlite.transaction"
        } else {
            "sqlite"
        },
        json!({"path":PATH,"commands":commands}),
    ))
}

fn query(statement: &str, params: Json) -> Json {
    json!({"kind":"query","sql":statement,"params":params})
}

fn execute(statement: &str, params: Json) -> Json {
    json!({"kind":"execute","sql":statement,"params":params})
}

pub(crate) fn rows(reply: &Json) -> Result<&[Json], String> {
    reply
        .as_array()
        .and_then(|results| results.last())
        .and_then(|r| r["rows"].as_array())
        .map(Vec::as_slice)
        .ok_or_else(|| "Invalid calendar storage response.".into())
}

fn revision(reply: &Json) -> Result<u64, String> {
    let value = model::integer(&reply[0]["rows"][0][0])?;
    u64::try_from(value)
        .ok()
        .filter(|n| *n < 9_007_199_254_740_990)
        .ok_or_else(|| "Invalid calendar revision in storage.".into())
}

pub(crate) fn notes(id: &str) -> Answer {
    sql(
        false,
        vec![query(
            "SELECT notes FROM schedules WHERE id = ?",
            json!([id]),
        )],
    )
}

fn result(revision: u64, success: bool, message: &str, id: &str, day: i32) -> Answer {
    Answer::Now(Value::record(vec![
        Value::Number(revision as f64),
        Value::Bool(success),
        Value::str(message),
        Value::str(id),
        Value::Number(day.into()),
    ]))
}

impl Calendar {
    pub(crate) fn start_load(
        &mut self,
        source: &str,
        after: AfterLoad,
        today: i32,
        initialize: bool,
    ) -> Answer {
        let load = Load {
            source: source.into(),
            after,
            step: LoadStep::Schema,
            today,
            revision: None,
            events: BTreeMap::new(),
            restarts: 0,
        };
        if initialize {
            self.load = Some(load);
            sql(
                false,
                vec![query(
                    "SELECT 1 FROM sqlite_schema WHERE type='table' AND name='calendar_meta'",
                    json!([]),
                )],
            )
        } else {
            self.load_page(load)
        }
    }

    fn load_page(&mut self, mut load: Load) -> Answer {
        let cursor = load
            .events
            .last_key_value()
            .map_or("", |(id, _)| id.as_str())
            .to_owned();
        load.step = LoadStep::Page;
        self.load = Some(load);
        sql(
            false,
            vec![query(REVISION, json!([])), query(SUMMARY, json!([cursor]))],
        )
    }

    fn initialize(&mut self, mut load: Load) -> Answer {
        let mut commands = vec![
            execute("CREATE TABLE IF NOT EXISTS schedules (id TEXT PRIMARY KEY NOT NULL,title TEXT NOT NULL,notes TEXT NOT NULL,color TEXT NOT NULL,all_day INTEGER NOT NULL,start_day INTEGER NOT NULL,end_day INTEGER NOT NULL,start_minute INTEGER NOT NULL,end_minute INTEGER NOT NULL)", json!([])),
            execute("CREATE TABLE IF NOT EXISTS calendar_meta (key TEXT PRIMARY KEY NOT NULL,value INTEGER NOT NULL)", json!([])),
            execute("CREATE TABLE IF NOT EXISTS calendar_operations (token TEXT PRIMARY KEY NOT NULL,fingerprint TEXT NOT NULL,revision INTEGER NOT NULL,event_id TEXT NOT NULL,day INTEGER NOT NULL)", json!([])),
        ];
        for (event, notes) in model::samples(load.today) {
            commands.push(execute("INSERT OR IGNORE INTO schedules SELECT ?,?,?,?,?,?,?,?,? WHERE NOT EXISTS (SELECT 1 FROM calendar_meta WHERE key='seeded')", event.params(&notes)));
        }
        commands.push(execute(
            "INSERT OR IGNORE INTO calendar_meta VALUES('seeded',1),('revision',1)",
            json!([]),
        ));
        load.step = LoadStep::Initialized;
        self.load = Some(load);
        sql(true, commands)
    }

    fn failed_load(&self, after: AfterLoad, message: &str) -> Answer {
        match after {
            AfterLoad::Library => Answer::Now(self.library(message)),
            AfterLoad::ResumeMutation(_) => result(self.revision, false, message, "", 0),
        }
    }

    fn restart_load(&mut self, mut load: Load) -> Answer {
        if load.restarts >= 2 {
            return self.failed_load(
                load.after,
                "Schedules changed while loading. Please try again.",
            );
        }
        load.events.clear();
        load.revision = None;
        load.restarts += 1;
        self.load_page(load)
    }

    pub(crate) fn parse_load(&mut self, outcome: Outcome) -> Answer {
        let Some(mut load) = self.load.take() else {
            return Answer::Now(self.library("There is no pending calendar load."));
        };
        let reply = match storage::response(outcome) {
            Ok(reply) => reply,
            Err(message) => return self.failed_load(load.after, &message),
        };
        match load.step {
            LoadStep::Schema => match rows(&reply) {
                Ok([]) => self.initialize(load),
                Ok(_) => self.load_page(load),
                Err(message) => self.failed_load(load.after, &message),
            },
            LoadStep::Initialized => self.load_page(load),
            LoadStep::Page => {
                let page =
                    revision(&reply).and_then(|revision| rows(&reply).map(|rows| (revision, rows)));
                let (revision, rows) = match page {
                    Ok(page) => page,
                    Err(message) => return self.failed_load(load.after, &message),
                };
                if load.revision.is_some_and(|previous| previous != revision) {
                    return self.restart_load(load);
                }
                load.revision = Some(revision);
                for row in rows {
                    match Schedule::read(row) {
                        Ok(event) => {
                            load.events.insert(event.id.clone(), event);
                        }
                        Err(message) => return self.failed_load(load.after, &message),
                    }
                }
                if rows.len() == PAGE {
                    self.load_page(load)
                } else {
                    load.step = LoadStep::Verify;
                    self.load = Some(load);
                    sql(false, vec![query(REVISION, json!([]))])
                }
            }
            LoadStep::Verify => {
                let current = match revision(&reply) {
                    Ok(revision) => revision,
                    Err(message) => return self.failed_load(load.after, &message),
                };
                if load.revision != Some(current) {
                    return self.restart_load(load);
                }
                self.replace_events(load.events, current);
                match load.after {
                    AfterLoad::Library => Answer::Now(self.library("")),
                    AfterLoad::ResumeMutation(intent) => self.lookup(intent),
                }
            }
        }
    }

    pub(crate) fn begin_mutation(&mut self, source: &str, args: &[Value]) -> Answer {
        if !self.ready || self.load.is_some() || self.mutation.is_some() {
            return result(
                self.revision,
                false,
                "Wait for the current calendar operation to finish.",
                "",
                0,
            );
        }
        match intent(source, args) {
            Ok(intent) => self.lookup(intent),
            Err(message) => result(self.revision, false, &message, "", 0),
        }
    }

    fn lookup(&mut self, intent: Intent) -> Answer {
        let commands = vec![
            query(REVISION, json!([])),
            query(
                "SELECT fingerprint,revision,event_id,day FROM calendar_operations WHERE token = ?",
                json!([intent.token]),
            ),
        ];
        self.mutation = Some(PendingMutation::Lookup(intent));
        sql(false, commands)
    }

    fn write_mutation(&mut self, intent: Intent) -> Answer {
        let prepared = self.prepare(&intent);
        let (applied, mut commands) = match prepared {
            Ok(prepared) => prepared,
            Err(message) => return result(self.revision, false, &message, "", 0),
        };
        // A concurrent writer cannot overwrite an unseen revision. NULL violates
        // the schema, rolling this complete transaction back before any change.
        commands.insert(0, execute("UPDATE calendar_meta SET value=CASE WHEN value=? THEN ? ELSE NULL END WHERE key='revision'",
            json!([self.revision, applied.revision])));
        commands.push(execute(
            "INSERT INTO calendar_operations VALUES(?,?,?,?,?)",
            json!([
                intent.token,
                intent.fingerprint,
                applied.revision,
                applied.id,
                applied.day
            ]),
        ));
        self.mutation = Some(PendingMutation::Write(applied));
        sql(true, commands)
    }

    fn prepare(&self, intent: &Intent) -> Result<(Applied, Vec<Json>), String> {
        let next = self.revision + 1;
        match &intent.edit {
            Edit::Save {
                event,
                notes,
                create,
            } => {
                if !create && !self.events.contains_key(&event.id) {
                    return Err("This schedule no longer exists.".into());
                }
                let applied = Applied {
                    event: Some(event.clone()),
                    remove: false,
                    id: event.id.clone(),
                    day: event.start,
                    revision: next,
                };
                Ok((applied, vec![execute(INSERT, event.params(notes))]))
            }
            Edit::Delete(id) => {
                let event = self
                    .events
                    .get(id)
                    .ok_or("This schedule no longer exists.")?;
                let applied = Applied {
                    event: None,
                    remove: true,
                    id: id.clone(),
                    day: event.start,
                    revision: next,
                };
                Ok((
                    applied,
                    vec![execute("DELETE FROM schedules WHERE id = ?", json!([id]))],
                ))
            }
            Edit::Move { id, day } => {
                let event = self
                    .events
                    .get(id)
                    .ok_or("This schedule no longer exists.")?;
                let moved = event.moved(*day)?;
                let changed = event != &moved;
                let commands = if changed {
                    vec![execute(
                        "UPDATE schedules SET start_day=?,end_day=? WHERE id=?",
                        json!([moved.start, moved.end, id]),
                    )]
                } else {
                    vec![]
                };
                Ok((
                    Applied {
                        event: changed.then_some(moved),
                        remove: false,
                        id: id.clone(),
                        day: *day,
                        revision: if changed { next } else { self.revision },
                    },
                    commands,
                ))
            }
        }
    }

    pub(crate) fn parse_mutation(&mut self, outcome: Outcome) -> Answer {
        let Some(pending) = self.mutation.take() else {
            return result(
                self.revision,
                false,
                "There is no pending calendar edit.",
                "",
                0,
            );
        };
        let reply = match storage::response(outcome) {
            Ok(reply) => reply,
            Err(message) => return result(self.revision, false, &message, "", 0),
        };
        match pending {
            PendingMutation::Lookup(intent) => {
                let current = match revision(&reply) {
                    Ok(revision) => revision,
                    Err(message) => return result(self.revision, false, &message, "", 0),
                };
                if current != self.revision {
                    let source = intent.source.clone();
                    return self.start_load(&source, AfterLoad::ResumeMutation(intent), 0, false);
                }
                let rows = match rows(&reply) {
                    Ok(rows) => rows,
                    Err(message) => return result(self.revision, false, &message, "", 0),
                };
                if let Some(row) = rows.first() {
                    if row[0].as_str() != Some(&intent.fingerprint) {
                        return result(
                            self.revision,
                            false,
                            "This operation was already used for another change.",
                            "",
                            0,
                        );
                    }
                    let stored = model::integer(&row[1]).and_then(|revision| {
                        let day = i32::try_from(model::integer(&row[3])?)
                            .map_err(|_| "Invalid stored result date.")?;
                        let id = row[2].as_str().ok_or("Invalid stored result identifier.")?;
                        Ok((revision, id, day))
                    });
                    return match stored {
                        Ok((_, id, day)) => result(self.revision, true, "", id, day),
                        Err(message) => result(self.revision, false, &message, "", 0),
                    };
                }
                self.write_mutation(intent)
            }
            PendingMutation::Write(applied) => {
                if applied.remove || applied.event.is_some() {
                    self.changed_event(&applied.id, applied.event, applied.revision);
                }
                result(applied.revision, true, "", &applied.id, applied.day)
            }
        }
    }
}

fn intent(source: &str, args: &[Value]) -> Result<Intent, String> {
    let (count, token_at) = match source {
        "saveSchedule" => (10, 9),
        "deleteSchedule" => (2, 1),
        "moveSchedule" => (3, 2),
        _ => return Err("Unknown calendar operation.".into()),
    };
    if args.len() != count {
        return Err("The calendar operation has invalid arguments.".into());
    }
    let token = text(args, token_at)?;
    if token.is_empty() || token.len() > 160 || token.chars().any(char::is_control) {
        return Err("The calendar operation identifier is invalid.".into());
    }
    let id = text(args, 0)?;
    let edit = match source {
        "saveSchedule" => {
            let create = id.is_empty();
            let event = Schedule {
                id: if create {
                    format!("event-{token}")
                } else {
                    id.into()
                },
                title: text(args, 1)?.trim().into(),
                color: text(args, 3)?.into(),
                all_day: match args.get(4) {
                    Some(Value::Bool(value)) => *value,
                    _ => return Err("Choose whether this is an all-day schedule.".into()),
                },
                start: dates::parse_date(text(args, 5)?)?,
                end: dates::parse_date(text(args, 6)?)?,
                start_time: dates::parse_time(text(args, 7)?)?,
                end_time: dates::parse_time(text(args, 8)?)?,
            };
            event.validate()?;
            let notes = text(args, 2)?;
            if notes.chars().count() > 20_000 {
                return Err("Notes can contain up to 20,000 characters.".into());
            }
            Edit::Save {
                event,
                notes: notes.into(),
                create,
            }
        }
        "deleteSchedule" => Edit::Delete(id.into()),
        "moveSchedule" => Edit::Move {
            id: id.into(),
            day: whole(args, 1)?,
        },
        _ => unreachable!(),
    };
    let canonical: Result<Vec<Json>, String> = args[..token_at]
        .iter()
        .map(|arg| match arg {
            Value::Bool(value) => Ok(json!(value)),
            Value::Number(value) if value.is_finite() => Ok(json!(value)),
            value if value.is_str() => Ok(json!(value.as_str().unwrap())),
            _ => Err("The calendar operation has an invalid value.".into()),
        })
        .collect();
    Ok(Intent {
        source: source.into(),
        token: token.into(),
        fingerprint: json!([source, canonical?]).to_string(),
        edit,
    })
}
