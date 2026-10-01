//! Portable SQLite continuations. Layout queries never enter this module.

use crate::{dates, model, number, text, whole, Calendar, Schedule};
use exact_data::storage;
use exact_plan::Value;
use exact_runner::{Answer, Outcome};
use serde_json::{json, Value as Json};
use std::collections::BTreeMap;

const PATH: &str = "app:/data/calendar.db";
const PAGE: usize = 256;
const SUMMARY: &str = "SELECT id,title,color,all_day,start_day,end_day,start_minute,end_minute,kind,completed FROM schedules WHERE id > ? ORDER BY id LIMIT 256";
const STICKERS: &str = "SELECT day,sticker_id FROM stickers WHERE day > ? ORDER BY day LIMIT 256";
const REVISION: &str = "SELECT value FROM calendar_meta WHERE key='revision'";
const VERIFY: &str = "SELECT (SELECT value FROM calendar_meta WHERE key='revision'),COALESCE((SELECT value FROM calendar_meta WHERE key='theme'),0)";
const INSERT: &str = "INSERT INTO schedules(id,title,notes,color,all_day,start_day,end_day,start_minute,end_minute,kind,completed) VALUES(?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET title=excluded.title,notes=excluded.notes,color=excluded.color,all_day=excluded.all_day,start_day=excluded.start_day,end_day=excluded.end_day,start_minute=excluded.start_minute,end_minute=excluded.end_minute,kind=excluded.kind,completed=excluded.completed";

pub(crate) enum AfterLoad {
    Library,
    ResumeMutation(Intent),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LoadStep {
    Schema,
    Initialized,
    ReviewSamples,
    Upgraded,
    Schedules,
    Stickers,
    Verify,
}

pub(crate) struct Load {
    pub source: String,
    after: AfterLoad,
    step: LoadStep,
    today: i32,
    revision: Option<u64>,
    sample_version: i64,
    events: BTreeMap<String, Schedule>,
    stickers: BTreeMap<i32, String>,
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
    CompleteTodo {
        id: String,
        completed: bool,
    },
    Sticker {
        day: i32,
        sticker: Option<String>,
    },
    MoveSticker {
        from: i32,
        to: i32,
        expected: String,
    },
    Theme(u8),
}

pub(crate) struct Intent {
    source: String,
    token: String,
    fingerprint: String,
    edit: Edit,
    recovery: Option<Box<Recovery>>,
}

struct Recovery {
    message: String,
    reloaded: bool,
}

enum Change {
    Schedule { id: String, event: Option<Schedule> },
    Sticker { day: i32, sticker: Option<String> },
    Theme(u8),
}

pub(crate) struct Applied {
    changes: Vec<Change>,
    id: String,
    day: i32,
    revision: u64,
}

pub(crate) enum PendingMutation {
    Lookup(Intent),
    Write { intent: Intent, applied: Applied },
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

struct Schema {
    exists: bool,
    kind: bool,
    completed: bool,
    todos: bool,
    current: bool,
    version: i64,
}

fn schema(reply: &Json) -> Result<Schema, String> {
    let columns = reply[0]["rows"]
        .as_array()
        .ok_or("Invalid calendar schema response.")?;
    let tables = rows(reply)?;
    let has_table = |name: &str| tables.iter().any(|row| row[0].as_str() == Some(name));
    let exists = !columns.is_empty();
    if (exists && (!has_table("calendar_meta") || !has_table("calendar_operations")))
        || (!exists && !tables.is_empty())
    {
        return Err("The calendar database is incomplete.".into());
    }
    let kind = columns.iter().any(|row| row[1].as_str() == Some("kind"));
    let completed = columns
        .iter()
        .any(|row| row[1].as_str() == Some("completed"));
    let todos = has_table("todos");
    let version = model::integer(&reply[1]["rows"][0][0])?;
    Ok(Schema {
        exists,
        kind,
        completed,
        todos,
        current: exists && kind && completed && !todos && has_table("stickers") && version == 5,
        version,
    })
}

fn sticker(row: &Json) -> Result<(i32, String), String> {
    let day =
        i32::try_from(model::integer(&row[0])?).map_err(|_| "Invalid sticker date in storage.")?;
    let id = row[1].as_str().ok_or("Invalid sticker in storage.")?;
    if !dates::in_range(day) || !model::STICKERS.contains(&id) {
        return Err("Invalid sticker in storage.".into());
    }
    Ok((day, id.to_owned()))
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
            sample_version: 0,
            events: BTreeMap::new(),
            stickers: BTreeMap::new(),
            restarts: 0,
        };
        if initialize {
            self.probe_schema(load)
        } else {
            self.load_page(load, LoadStep::Schedules)
        }
    }

    fn probe_schema(&mut self, mut load: Load) -> Answer {
        load.step = LoadStep::Schema;
        self.load = Some(load);
        sql(
            false,
            vec![
                query("PRAGMA table_info(schedules)", json!([])),
                query("PRAGMA user_version", json!([])),
                query("SELECT name FROM sqlite_schema WHERE type='table' AND name IN ('calendar_meta','calendar_operations','todos','stickers')", json!([])),
            ],
        )
    }

    fn load_page(&mut self, mut load: Load, step: LoadStep) -> Answer {
        let command = match step {
            LoadStep::Schedules => query(
                SUMMARY,
                json!([load
                    .events
                    .last_key_value()
                    .map_or("", |(id, _)| id.as_str())]),
            ),
            LoadStep::Stickers => query(
                STICKERS,
                json!([load
                    .stickers
                    .last_key_value()
                    .map_or(dates::month_first(dates::FIRST_MONTH) - 1, |(day, _)| *day)]),
            ),
            _ => unreachable!(),
        };
        load.step = step;
        self.load = Some(load);
        sql(false, vec![query(REVISION, json!([])), command])
    }

    fn initialize(&mut self, mut load: Load, schema: Schema) -> Answer {
        load.sample_version = schema.version;
        let mut commands = vec![
            execute("CREATE TABLE IF NOT EXISTS schedules (id TEXT PRIMARY KEY NOT NULL,title TEXT NOT NULL,notes TEXT NOT NULL,color TEXT NOT NULL,all_day INTEGER NOT NULL,start_day INTEGER NOT NULL,end_day INTEGER NOT NULL,start_minute INTEGER NOT NULL,end_minute INTEGER NOT NULL,kind TEXT NOT NULL DEFAULT 'event',completed INTEGER NOT NULL DEFAULT 0)", json!([])),
            execute("CREATE TABLE IF NOT EXISTS calendar_meta (key TEXT PRIMARY KEY NOT NULL,value INTEGER NOT NULL)", json!([])),
            execute("CREATE TABLE IF NOT EXISTS calendar_operations (token TEXT PRIMARY KEY NOT NULL,fingerprint TEXT NOT NULL,revision INTEGER NOT NULL,event_id TEXT NOT NULL,day INTEGER NOT NULL)", json!([])),
            execute("CREATE TABLE IF NOT EXISTS stickers (day INTEGER PRIMARY KEY NOT NULL,sticker_id TEXT NOT NULL)", json!([])),
        ];
        if schema.exists && !schema.kind {
            commands.push(execute(
                "ALTER TABLE schedules ADD COLUMN kind TEXT NOT NULL DEFAULT 'event'",
                json!([]),
            ));
        }
        if schema.exists && !schema.completed {
            commands.push(execute(
                "ALTER TABLE schedules ADD COLUMN completed INTEGER NOT NULL DEFAULT 0",
                json!([]),
            ));
        }
        if schema.todos {
            // An undated item's creation time supplies its local date. Invalid or
            // out-of-range timestamps fall back to the reported launch date.
            let first = dates::month_first(dates::FIRST_MONTH);
            let last = dates::month_first(dates::LAST_MONTH + 1) - 1;
            commands.push(execute("WITH dated AS (SELECT *,CAST((created_at+?*60000-CASE WHEN created_at+?*60000<0 THEN 86399999 ELSE 0 END)/86400000 AS INTEGER) AS local_day FROM todos),converted AS (SELECT *,CASE WHEN created_at>0 AND local_day BETWEEN ? AND ? THEN local_day ELSE ? END AS day FROM dated) INSERT INTO schedules(id,title,notes,color,all_day,start_day,end_day,start_minute,end_minute,kind,completed) SELECT id,title,'',color,1,day,day,0,0,'todo',CASE WHEN completed_at>0 THEN 1 ELSE 0 END FROM converted", json!([self.utc_offset,self.utc_offset,first,last,load.today])));
            commands.push(execute("UPDATE calendar_meta SET value=value+1 WHERE key='revision' AND EXISTS(SELECT 1 FROM todos)", json!([])));
            commands.push(execute("DROP TABLE todos", json!([])));
        }
        for (event, notes) in model::samples(load.today) {
            commands.push(execute("INSERT OR IGNORE INTO schedules(id,title,notes,color,all_day,start_day,end_day,start_minute,end_minute,kind,completed) SELECT ?,?,?,?,?,?,?,?,?,?,? WHERE NOT EXISTS (SELECT 1 FROM calendar_meta WHERE key='seeded')", event.params(&notes)));
        }
        for (day, sticker) in model::sample_stickers(load.today) {
            commands.push(execute("INSERT OR IGNORE INTO stickers(day,sticker_id) SELECT ?,? WHERE NOT EXISTS (SELECT 1 FROM calendar_meta WHERE key='seeded')", json!([day,sticker])));
        }
        commands.push(execute(
            "INSERT OR IGNORE INTO calendar_meta VALUES('seeded',1),('revision',1)",
            json!([]),
        ));
        if schema.exists {
            load.step = LoadStep::Initialized;
        } else {
            commands.push(execute("PRAGMA user_version=5", json!([])));
            load.step = LoadStep::Upgraded;
        }
        self.load = Some(load);
        sql(true, commands)
    }

    fn refresh_samples(&mut self, mut load: Load, reply: &Json) -> Result<Answer, String> {
        let replies = reply
            .as_array()
            .ok_or("Invalid calendar initialization response.")?;
        let current = model::integer(&replies[replies.len().saturating_sub(2)]["rows"][0][0])?;
        let mut unchanged = BTreeMap::new();
        let mut original_ids = std::collections::BTreeSet::new();
        for row in rows(reply)? {
            let Some(id) = row[0].as_str() else { continue };
            let Some(date) = id.strip_prefix("sample-").and_then(|tail| tail.get(..7)) else {
                continue;
            };
            let Ok(day) = dates::parse_date(&format!("{date}-01")) else {
                continue;
            };
            let originals = model::original_samples(day);
            let previous = model::previous_samples(day);
            let previous_v4 = model::previous_samples_v4(day);
            if load.sample_version >= 2 {
                original_ids.extend(previous.iter().map(|(event, _)| event.id.clone()));
            } else if originals.iter().any(|(event, _)| event.id == id) {
                original_ids.extend(originals.iter().map(|(event, _)| event.id.clone()));
            }
            if originals
                .iter()
                .chain(previous.iter())
                .chain(previous_v4.iter())
                .any(|(original, notes)| {
                    let expected = original.params(notes);
                    expected
                        .as_array()
                        .unwrap()
                        .iter()
                        .enumerate()
                        .all(|(index, expected)| {
                            if let Some(text) = expected.as_str() {
                                row[index].as_str() == Some(text)
                            } else {
                                model::integer(&row[index]).ok() == expected.as_i64()
                            }
                        })
                })
            {
                unchanged.insert(id.to_owned(), ());
            }
        }
        let ranged_todos = model::integer(&replies[0]["rows"][0][1])? != 0;
        let revision = current + i64::from(!unchanged.is_empty() || ranged_todos);
        let mut commands = vec![execute("UPDATE calendar_meta SET value=CASE WHEN value=? THEN ? ELSE NULL END WHERE key='revision'", json!([current,revision]))];
        if !unchanged.is_empty() {
            for id in unchanged.keys() {
                commands.push(execute("DELETE FROM schedules WHERE id=?", json!([id])));
            }
            for (event, notes) in model::samples(load.today) {
                // A deleted original stays deleted; a modified original wins its
                // existing ID. New trip examples fill only previously unused IDs.
                if original_ids.contains(&event.id) && !unchanged.contains_key(&event.id) {
                    continue;
                }
                commands.push(execute("INSERT OR IGNORE INTO schedules(id,title,notes,color,all_day,start_day,end_day,start_minute,end_minute,kind,completed) VALUES(?,?,?,?,?,?,?,?,?,?,?)", event.params(&notes)));
            }
        }
        if !unchanged.is_empty() {
            for (day, sticker) in model::sample_stickers(load.today) {
                commands.push(execute(
                    "INSERT OR IGNORE INTO stickers(day,sticker_id) VALUES(?,?)",
                    json!([day, sticker]),
                ));
            }
        }
        commands.push(execute(
            "UPDATE schedules SET end_day=start_day WHERE kind='todo' AND end_day<>start_day",
            json!([]),
        ));
        commands.push(execute("PRAGMA user_version=5", json!([])));
        load.step = LoadStep::Upgraded;
        self.load = Some(load);
        Ok(sql(true, commands))
    }

    fn failed_load(&self, after: AfterLoad, message: &str) -> Answer {
        match after {
            AfterLoad::Library => Answer::Now(self.library(message)),
            AfterLoad::ResumeMutation(intent) => result(
                self.revision,
                false,
                intent.recovery.as_ref().map_or(message, |r| &r.message),
                "",
                0,
            ),
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
        load.stickers.clear();
        load.revision = None;
        load.restarts += 1;
        self.load_page(load, LoadStep::Schedules)
    }

    pub(crate) fn parse_load(&mut self, outcome: Outcome) -> Answer {
        let Some(mut load) = self.load.take() else {
            return Answer::Now(self.library("There is no pending calendar load."));
        };
        let reply = match storage::response(outcome) {
            Ok(reply) => reply,
            // Another app window may have added the column after our probe.
            // Re-read the schema after a rolled-back transaction; never reset it.
            Err(_)
                if matches!(load.step, LoadStep::Initialized | LoadStep::Upgraded)
                    && load.restarts < 2 =>
            {
                load.restarts += 1;
                return self.probe_schema(load);
            }
            Err(message) => return self.failed_load(load.after, &message),
        };
        match load.step {
            LoadStep::Schema => match schema(&reply) {
                Ok(schema) if schema.current => self.load_page(load, LoadStep::Schedules),
                Ok(schema) => self.initialize(load, schema),
                Err(message) => self.failed_load(load.after, &message),
            },
            LoadStep::Initialized => {
                load.step = LoadStep::ReviewSamples;
                self.load = Some(load);
                sql(false, vec![query("SELECT value,EXISTS(SELECT 1 FROM schedules WHERE kind='todo' AND end_day<>start_day) FROM calendar_meta WHERE key='revision'", json!([])), query("SELECT id,title,notes,color,all_day,start_day,end_day,start_minute,end_minute,kind,completed FROM schedules WHERE id LIKE 'sample-%'", json!([]))])
            }
            LoadStep::ReviewSamples => match self.refresh_samples(load, &reply) {
                Ok(answer) => answer,
                Err(message) => Answer::Now(self.library(&message)),
            },
            LoadStep::Upgraded => self.load_page(load, LoadStep::Schedules),
            LoadStep::Schedules | LoadStep::Stickers => {
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
                    let read = match load.step {
                        LoadStep::Schedules => Schedule::read(row).map(|event| {
                            load.events.insert(event.id.clone(), event);
                        }),
                        LoadStep::Stickers => sticker(row).map(|(day, id)| {
                            load.stickers.insert(day, id);
                        }),
                        _ => unreachable!(),
                    };
                    if let Err(message) = read {
                        return self.failed_load(load.after, &message);
                    }
                }
                if rows.len() == PAGE {
                    let step = load.step;
                    self.load_page(load, step)
                } else {
                    match load.step {
                        LoadStep::Schedules => self.load_page(load, LoadStep::Stickers),
                        LoadStep::Stickers => {
                            load.step = LoadStep::Verify;
                            self.load = Some(load);
                            sql(false, vec![query(VERIFY, json!([]))])
                        }
                        _ => unreachable!(),
                    }
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
                let theme = match model::integer(&reply[0]["rows"][0][1]) {
                    Ok(value) if (0..=6).contains(&value) => value as u8,
                    _ => return self.failed_load(load.after, "Invalid calendar theme in storage."),
                };
                self.replace_library(load.events, load.stickers, theme, current);
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
        self.mutation = Some(PendingMutation::Write { intent, applied });
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
                let mut event = event.clone();
                if !create {
                    let current = self
                        .events
                        .get(&event.id)
                        .ok_or("This schedule no longer exists.")?;
                    event.completed = event.kind == model::Kind::Todo && current.completed;
                }
                let applied = Applied {
                    changes: vec![Change::Schedule {
                        id: event.id.clone(),
                        event: Some(event.clone()),
                    }],
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
                    changes: vec![Change::Schedule {
                        id: id.clone(),
                        event: None,
                    }],
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
                        changes: if changed {
                            vec![Change::Schedule {
                                id: id.clone(),
                                event: Some(moved),
                            }]
                        } else {
                            vec![]
                        },
                        id: id.clone(),
                        day: *day,
                        revision: if changed { next } else { self.revision },
                    },
                    commands,
                ))
            }
            Edit::CompleteTodo { id, completed } => {
                let mut event = self
                    .events
                    .get(id)
                    .filter(|event| event.kind == model::Kind::Todo)
                    .ok_or("This todo no longer exists.")?
                    .clone();
                let changed = event.completed != *completed;
                event.completed = *completed;
                let commands = if changed {
                    vec![execute(
                        "UPDATE schedules SET completed=? WHERE id=?",
                        json!([*completed as u8, id]),
                    )]
                } else {
                    vec![]
                };
                Ok((
                    Applied {
                        id: id.clone(),
                        day: event.start,
                        revision: if changed { next } else { self.revision },
                        changes: if changed {
                            vec![Change::Schedule {
                                id: id.clone(),
                                event: Some(event),
                            }]
                        } else {
                            vec![]
                        },
                    },
                    commands,
                ))
            }
            Edit::Sticker { day, sticker } => {
                let changed = self.stickers.get(day) != sticker.as_ref();
                let commands = if !changed {
                    vec![]
                } else if let Some(sticker) = sticker {
                    vec![execute("INSERT INTO stickers(day,sticker_id) VALUES(?,?) ON CONFLICT(day) DO UPDATE SET sticker_id=excluded.sticker_id", json!([day,sticker]))]
                } else {
                    vec![execute("DELETE FROM stickers WHERE day=?", json!([day]))]
                };
                Ok((
                    Applied {
                        id: dates::iso(*day),
                        day: *day,
                        revision: if changed { next } else { self.revision },
                        changes: if changed {
                            vec![Change::Sticker {
                                day: *day,
                                sticker: sticker.clone(),
                            }]
                        } else {
                            vec![]
                        },
                    },
                    commands,
                ))
            }
            Edit::MoveSticker { from, to, expected } => {
                if self.stickers.get(from) != Some(expected) {
                    return Err("This sticker changed before it could be moved.".into());
                }
                let changed = from != to;
                let commands = if changed {
                    vec![execute("DELETE FROM stickers WHERE day=?", json!([from])), execute("INSERT INTO stickers(day,sticker_id) VALUES(?,?) ON CONFLICT(day) DO UPDATE SET sticker_id=excluded.sticker_id", json!([to,expected]))]
                } else {
                    vec![]
                };
                Ok((
                    Applied {
                        id: dates::iso(*to),
                        day: *to,
                        revision: if changed { next } else { self.revision },
                        changes: if changed {
                            vec![
                                Change::Sticker {
                                    day: *from,
                                    sticker: None,
                                },
                                Change::Sticker {
                                    day: *to,
                                    sticker: Some(expected.clone()),
                                },
                            ]
                        } else {
                            vec![]
                        },
                    },
                    commands,
                ))
            }
            Edit::Theme(theme) => {
                let changed = self.theme != *theme;
                Ok((
                    Applied {
                        id: format!("theme-{theme}"),
                        day: 0,
                        revision: if changed { next } else { self.revision },
                        changes: if changed {
                            vec![Change::Theme(*theme)]
                        } else {
                            vec![]
                        },
                    },
                    if changed {
                        vec![execute("INSERT INTO calendar_meta(key,value) VALUES('theme',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value", json!([theme]))]
                    } else {
                        vec![]
                    },
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
            Err(message) => {
                return match pending {
                    PendingMutation::Write { mut intent, .. } => {
                        // A failed reply may follow a committed transaction. Resolve
                        // its exact token with reads only; never issue a second write.
                        intent.recovery = Some(Box::new(Recovery {
                            message,
                            reloaded: false,
                        }));
                        self.lookup(intent)
                    }
                    PendingMutation::Lookup(intent) => result(
                        self.revision,
                        false,
                        intent.recovery.as_ref().map_or(&message, |r| &r.message),
                        "",
                        0,
                    ),
                };
            }
        };
        match pending {
            PendingMutation::Lookup(mut intent) => {
                let current = match revision(&reply) {
                    Ok(revision) => revision,
                    Err(message) => return result(self.revision, false, &message, "", 0),
                };
                if current != self.revision {
                    if let Some(recovery) = &mut intent.recovery {
                        if recovery.reloaded {
                            return result(self.revision, false, &recovery.message, "", 0);
                        }
                        recovery.reloaded = true;
                    }
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
                            intent.recovery.as_ref().map_or(
                                "This operation was already used for another change.",
                                |r| &r.message,
                            ),
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
                if let Some(recovery) = intent.recovery {
                    return result(self.revision, false, &recovery.message, "", 0);
                }
                self.write_mutation(intent)
            }
            PendingMutation::Write { applied, .. } => {
                for change in applied.changes {
                    match change {
                        Change::Schedule { id, event } => {
                            self.changed_event(&id, event, applied.revision)
                        }
                        Change::Sticker { day, sticker } => self.changed_sticker(day, sticker),
                        Change::Theme(theme) => self.theme = theme,
                    }
                }
                self.revision = applied.revision;
                result(applied.revision, true, "", &applied.id, applied.day)
            }
        }
    }
}

fn intent(source: &str, args: &[Value]) -> Result<Intent, String> {
    let (count, token_at) = match source {
        "saveSchedule" | "savePlan" | "saveTodo" => (10, 9),
        "deleteSchedule" | "removeSticker" => (2, 1),
        "moveSchedule" | "setSticker" => (3, 2),
        "setTodoCompleted" | "moveSticker" => (4, 3),
        "setTheme" => (2, 1),
        _ => return Err("Unknown calendar operation.".into()),
    };
    if args.len() != count {
        return Err("The calendar operation has invalid arguments.".into());
    }
    let token = text(args, token_at)?;
    if token.is_empty() || token.len() > 160 || token.chars().any(char::is_control) {
        return Err("The calendar operation identifier is invalid.".into());
    }
    let id = if matches!(
        source,
        "setSticker" | "removeSticker" | "moveSticker" | "setTheme"
    ) {
        ""
    } else {
        text(args, 0)?
    };
    let edit = match source {
        "saveSchedule" | "savePlan" | "saveTodo" => {
            let create = id.is_empty();
            let kind = match source {
                "savePlan" => model::Kind::Plan,
                "saveTodo" => model::Kind::Todo,
                _ => model::Kind::Event,
            };
            let mut event = Schedule {
                id: if create {
                    format!("{}-{token}", kind.name())
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
                kind,
                completed: false,
            };
            if kind == model::Kind::Todo {
                event.end = event.start;
            }
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
        "setTodoCompleted" => {
            epoch(args, 2)?;
            Edit::CompleteTodo {
                id: id.into(),
                completed: match args.get(1) {
                    Some(Value::Bool(value)) => *value,
                    _ => return Err("Choose whether this todo is completed.".into()),
                },
            }
        }
        "setSticker" | "removeSticker" => {
            let day = whole(args, 0)?;
            if !dates::in_range(day) {
                return Err("Choose a sticker date between 1900 and 2100.".into());
            }
            let sticker = if source == "setSticker" {
                let sticker = text(args, 1)?;
                if !model::STICKERS.contains(&sticker) {
                    return Err("Choose one of the available stickers.".into());
                }
                Some(sticker.to_owned())
            } else {
                None
            };
            Edit::Sticker { day, sticker }
        }
        "moveSticker" => {
            let from = whole(args, 0)?;
            let to = whole(args, 1)?;
            let expected = text(args, 2)?;
            if !dates::in_range(from) || !dates::in_range(to) {
                return Err("Choose a sticker date between 1900 and 2100.".into());
            }
            if !model::STICKERS.contains(&expected) {
                return Err("Choose one of the available stickers.".into());
            }
            Edit::MoveSticker {
                from,
                to,
                expected: expected.into(),
            }
        }
        "setTheme" => {
            let theme = whole(args, 0)?;
            if !(0..=6).contains(&theme) {
                return Err("Choose an available calendar theme.".into());
            }
            Edit::Theme(theme as u8)
        }
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
        recovery: None,
    })
}

fn epoch(args: &[Value], at: usize) -> Result<i64, String> {
    // exactTime + now() may carry fractional milliseconds on native hosts.
    let value = number(args, at)?.floor();
    if value <= 0.0 || value > 9_007_199_254_740_991.0 {
        return Err("The todo needs a valid creation or completion time.".into());
    }
    Ok(value as i64)
}
