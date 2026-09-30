//! Portable SQLite continuations. Layout queries never enter this module.

use crate::{dates, model, number, text, whole, Calendar, Schedule, Todo};
use exact_data::storage;
use exact_plan::Value;
use exact_runner::{Answer, Outcome};
use serde_json::{json, Value as Json};
use std::collections::BTreeMap;

const PATH: &str = "app:/data/calendar.db";
const PAGE: usize = 256;
const SUMMARY: &str = "SELECT id,title,color,all_day,start_day,end_day,start_minute,end_minute,kind FROM schedules WHERE id > ? ORDER BY id LIMIT 256";
const TODOS: &str =
    "SELECT id,title,color,created_at,completed_at FROM todos WHERE id > ? ORDER BY id LIMIT 256";
const STICKERS: &str = "SELECT day,sticker_id FROM stickers WHERE day > ? ORDER BY day LIMIT 256";
const REVISION: &str = "SELECT value FROM calendar_meta WHERE key='revision'";
const VERIFY: &str = "SELECT (SELECT value FROM calendar_meta WHERE key='revision'),COALESCE((SELECT value FROM calendar_meta WHERE key='theme'),0)";
const INSERT: &str = "INSERT INTO schedules(id,title,notes,color,all_day,start_day,end_day,start_minute,end_minute,kind) VALUES(?,?,?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET title=excluded.title,notes=excluded.notes,color=excluded.color,all_day=excluded.all_day,start_day=excluded.start_day,end_day=excluded.end_day,start_minute=excluded.start_minute,end_minute=excluded.end_minute,kind=excluded.kind";

pub(crate) enum AfterLoad {
    Library,
    ResumeMutation(Intent),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LoadStep {
    Schema,
    Initialized,
    Schedules,
    Todos,
    Stickers,
    Verify,
}

pub(crate) struct Load {
    pub source: String,
    after: AfterLoad,
    step: LoadStep,
    today: i32,
    revision: Option<u64>,
    events: BTreeMap<String, Schedule>,
    todos: BTreeMap<String, Todo>,
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
    SaveTodo {
        todo: Todo,
        create: bool,
    },
    CompleteTodo {
        id: String,
        completed: bool,
        at: i64,
    },
    DeleteTodo(String),
    ScheduleTodo {
        id: String,
        day: i32,
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
    Todo { id: String, todo: Option<Todo> },
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

fn schema(reply: &Json) -> Result<(bool, bool), String> {
    let columns = reply[0]["rows"]
        .as_array()
        .ok_or("Invalid calendar schema response.")?;
    let tables = rows(reply)?;
    let has_table = |name: &str| tables.iter().any(|row| row[0].as_str() == Some(name));
    let schedules = !columns.is_empty();
    if (schedules && (!has_table("calendar_meta") || !has_table("calendar_operations")))
        || (!schedules && !tables.is_empty())
    {
        return Err("The calendar database is incomplete.".into());
    }
    let kind = columns.iter().any(|row| row[1].as_str() == Some("kind"));
    Ok((
        schedules && kind && has_table("todos") && has_table("stickers"),
        schedules && !kind,
    ))
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
            events: BTreeMap::new(),
            todos: BTreeMap::new(),
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
            LoadStep::Todos => query(
                TODOS,
                json!([load
                    .todos
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

    fn initialize(&mut self, mut load: Load, add_kind: bool) -> Answer {
        let mut commands = vec![
            execute("CREATE TABLE IF NOT EXISTS schedules (id TEXT PRIMARY KEY NOT NULL,title TEXT NOT NULL,notes TEXT NOT NULL,color TEXT NOT NULL,all_day INTEGER NOT NULL,start_day INTEGER NOT NULL,end_day INTEGER NOT NULL,start_minute INTEGER NOT NULL,end_minute INTEGER NOT NULL,kind TEXT NOT NULL DEFAULT 'event')", json!([])),
            execute("CREATE TABLE IF NOT EXISTS calendar_meta (key TEXT PRIMARY KEY NOT NULL,value INTEGER NOT NULL)", json!([])),
            execute("CREATE TABLE IF NOT EXISTS calendar_operations (token TEXT PRIMARY KEY NOT NULL,fingerprint TEXT NOT NULL,revision INTEGER NOT NULL,event_id TEXT NOT NULL,day INTEGER NOT NULL)", json!([])),
            execute("CREATE TABLE IF NOT EXISTS todos (id TEXT PRIMARY KEY NOT NULL,title TEXT NOT NULL,color TEXT NOT NULL,created_at INTEGER NOT NULL,completed_at INTEGER NOT NULL DEFAULT 0)", json!([])),
            execute("CREATE TABLE IF NOT EXISTS stickers (day INTEGER PRIMARY KEY NOT NULL,sticker_id TEXT NOT NULL)", json!([])),
        ];
        if add_kind {
            commands.push(execute(
                "ALTER TABLE schedules ADD COLUMN kind TEXT NOT NULL DEFAULT 'event'",
                json!([]),
            ));
        }
        for (event, notes) in model::samples(load.today) {
            commands.push(execute("INSERT OR IGNORE INTO schedules(id,title,notes,color,all_day,start_day,end_day,start_minute,end_minute,kind) SELECT ?,?,?,?,?,?,?,?,?,? WHERE NOT EXISTS (SELECT 1 FROM calendar_meta WHERE key='seeded')", event.params(&notes)));
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
        load.todos.clear();
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
            Err(_) if load.step == LoadStep::Initialized && load.restarts < 2 => {
                load.restarts += 1;
                return self.probe_schema(load);
            }
            Err(message) => return self.failed_load(load.after, &message),
        };
        match load.step {
            LoadStep::Schema => match schema(&reply) {
                Ok((true, _)) => self.load_page(load, LoadStep::Schedules),
                Ok((false, add_kind)) => self.initialize(load, add_kind),
                Err(message) => self.failed_load(load.after, &message),
            },
            LoadStep::Initialized => self.load_page(load, LoadStep::Schedules),
            LoadStep::Schedules | LoadStep::Todos | LoadStep::Stickers => {
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
                        LoadStep::Todos => Todo::read(row).map(|todo| {
                            load.todos.insert(todo.id.clone(), todo);
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
                        LoadStep::Schedules => self.load_page(load, LoadStep::Todos),
                        LoadStep::Todos => self.load_page(load, LoadStep::Stickers),
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
                    Ok(value) if (0..=5).contains(&value) => value as u8,
                    _ => return self.failed_load(load.after, "Invalid calendar theme in storage."),
                };
                self.replace_library(load.events, load.todos, load.stickers, theme, current);
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
                if !create && !self.events.contains_key(&event.id) {
                    return Err("This schedule no longer exists.".into());
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
            Edit::SaveTodo { todo, create } => {
                let mut todo = todo.clone();
                if !create {
                    let current = self
                        .todos
                        .get(&todo.id)
                        .ok_or("This todo no longer exists.")?;
                    todo.created_at = current.created_at;
                    todo.completed_at = current.completed_at;
                }
                let commands = vec![execute("INSERT INTO todos(id,title,color,created_at,completed_at) VALUES(?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET title=excluded.title,color=excluded.color", json!([todo.id,todo.title,todo.color,todo.created_at,todo.completed_at]))];
                Ok((
                    Applied {
                        id: todo.id.clone(),
                        day: 0,
                        revision: next,
                        changes: vec![Change::Todo {
                            id: todo.id.clone(),
                            todo: Some(todo),
                        }],
                    },
                    commands,
                ))
            }
            Edit::CompleteTodo { id, completed, at } => {
                let mut todo = self
                    .todos
                    .get(id)
                    .ok_or("This todo no longer exists.")?
                    .clone();
                let completed_at = if *completed {
                    if todo.completed_at == 0 {
                        *at
                    } else {
                        todo.completed_at
                    }
                } else {
                    0
                };
                let changed = completed_at != todo.completed_at;
                todo.completed_at = completed_at;
                let commands = if changed {
                    vec![execute(
                        "UPDATE todos SET completed_at=? WHERE id=?",
                        json!([completed_at, id]),
                    )]
                } else {
                    vec![]
                };
                Ok((
                    Applied {
                        id: id.clone(),
                        day: 0,
                        revision: if changed { next } else { self.revision },
                        changes: if changed {
                            vec![Change::Todo {
                                id: id.clone(),
                                todo: Some(todo),
                            }]
                        } else {
                            vec![]
                        },
                    },
                    commands,
                ))
            }
            Edit::DeleteTodo(id) => {
                if !self.todos.contains_key(id) {
                    return Err("This todo no longer exists.".into());
                }
                Ok((
                    Applied {
                        id: id.clone(),
                        day: 0,
                        revision: next,
                        changes: vec![Change::Todo {
                            id: id.clone(),
                            todo: None,
                        }],
                    },
                    vec![execute("DELETE FROM todos WHERE id=?", json!([id]))],
                ))
            }
            Edit::ScheduleTodo { id, day } => {
                let todo = self.todos.get(id).ok_or("This todo no longer exists.")?;
                if self.events.contains_key(id) {
                    return Err("A schedule with this todo identifier already exists.".into());
                }
                let event = Schedule {
                    id: id.clone(),
                    title: todo.title.clone(),
                    color: todo.color.clone(),
                    all_day: true,
                    start: *day,
                    end: *day,
                    start_time: 0,
                    end_time: 0,
                    kind: model::Kind::Event,
                };
                event.validate()?;
                let commands = vec![
                    execute(INSERT, event.params("")),
                    execute("DELETE FROM todos WHERE id=?", json!([id])),
                ];
                Ok((
                    Applied {
                        id: event.id.clone(),
                        day: *day,
                        revision: next,
                        changes: vec![
                            Change::Schedule {
                                id: event.id.clone(),
                                event: Some(event),
                            },
                            Change::Todo {
                                id: id.clone(),
                                todo: None,
                            },
                        ],
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
                        Change::Todo {
                            id,
                            todo: Some(todo),
                        } => {
                            self.todos.insert(id, todo);
                        }
                        Change::Todo { id, todo: None } => {
                            self.todos.remove(&id);
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
        "saveSchedule" | "savePlan" => (10, 9),
        "deleteSchedule" | "deleteTodo" | "removeSticker" => (2, 1),
        "moveSchedule" | "scheduleTodo" | "setSticker" => (3, 2),
        "saveTodo" => (5, 4),
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
        "saveSchedule" | "savePlan" => {
            let create = id.is_empty();
            let kind = if source == "savePlan" {
                model::Kind::Plan
            } else {
                model::Kind::Event
            };
            let event = Schedule {
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
        "saveTodo" => {
            let create = id.is_empty();
            let todo = Todo {
                id: if create {
                    format!("todo-{token}")
                } else {
                    id.into()
                },
                title: text(args, 1)?.trim().into(),
                color: text(args, 2)?.into(),
                created_at: epoch(args, 3)?,
                completed_at: 0,
            };
            todo.validate()?;
            Edit::SaveTodo { todo, create }
        }
        "setTodoCompleted" => Edit::CompleteTodo {
            id: id.into(),
            completed: match args.get(1) {
                Some(Value::Bool(value)) => *value,
                _ => return Err("Choose whether this todo is completed.".into()),
            },
            at: epoch(args, 2)?,
        },
        "deleteTodo" => Edit::DeleteTodo(id.into()),
        "scheduleTodo" => Edit::ScheduleTodo {
            id: id.into(),
            day: whole(args, 1)?,
        },
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
            if !(0..=5).contains(&theme) {
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
