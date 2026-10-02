//! Real portable SQLite execution: restarts, transactions, retries and paging.

use exact_data_host::Storage;
use exact_plan::Value;
use exact_runner::{
    Answer, DataError, DataSource, Dispatch, InFlight, Outcome, Reply, Store, Target, Work,
};
use macos_calendar_data::{Calendar, GRANTS};
use serde_json::{json, Value as Json};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

const TODAY: i32 = 20_697; // 2026-09-01, days since 1970-01-01.

#[path = "persistence/desktop.rs"]
mod desktop;
#[path = "persistence/stickers.rs"]
mod stickers;

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self(std::env::temp_dir().join(format!(
            "exact-calendar-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        if self.0.exists() {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
}

struct App {
    data: Storage<Calendar>,
    store: Store,
    requests: usize,
}
impl App {
    fn open(root: &Root) -> Self {
        let mut data = Storage::new(Calendar::default());
        data.configure_storage(
            root.0.join("data"),
            root.0.join("cache"),
            root.0.join("temporary"),
        )
        .unwrap();
        data.activate().unwrap();
        Self {
            data,
            store: Store::new(GRANTS, Vec::<(String, String)>::new()),
            requests: 0,
        }
    }

    fn work(&mut self, answer: Answer) -> Work {
        let Answer::Later(request) = answer else {
            panic!("expected storage request")
        };
        assert!(request.url.is_empty(), "the calendar works offline");
        self.requests += 1;
        match self
            .data
            .dispatch(request.continuation.unwrap(), &self.store)
        {
            Dispatch::Run(work) => work,
            _ => panic!("expected native storage work"),
        }
    }

    fn finish(&mut self, source: &str, args: &[Value], mut answer: Answer) -> Value {
        for _ in 0..200 {
            match answer {
                Answer::Now(value) => return value,
                Answer::Later(_) => {
                    let outcome = run(self.work(answer));
                    answer = self
                        .data
                        .parse(&mut self.store, source, args, outcome)
                        .unwrap();
                }
            }
        }
        panic!("{source} did not settle");
    }

    fn call(&mut self, source: &str, args: Vec<Value>) -> Value {
        let answer = self.data.answer(&mut self.store, source, &args).unwrap();
        self.finish(source, &args, answer)
    }

    fn load(&mut self) -> u64 {
        let value = self.call("loadCalendar", vec![num(TODAY), Value::Bool(true)]);
        assert_eq!(fields(&value)[0], Value::Bool(true), "{value:?}");
        fields(&value)[1].as_number().unwrap() as u64
    }

    fn save(&mut self, id: &str, title: &str, token: &str) -> Value {
        self.call("saveSchedule", save_args(id, title, token))
    }

    fn event(&mut self, id: &str) -> Value {
        let value = self.call("calendarEvent", vec![Value::str(id), num(0)]);
        assert_eq!(fields(&value)[0], Value::Bool(true), "{value:?}");
        assert_eq!(fields(&value)[2].as_str(), Some(""));
        fields(&value)[1].clone()
    }
}

fn run(work: Work) -> Outcome {
    match work {
        Work::Now(work) => std::thread::spawn(work).join().unwrap(),
        Work::Later(hand) => {
            let (tx, rx) = std::sync::mpsc::channel();
            hand(Reply::new(move |outcome| {
                let _ = tx.send(outcome);
            }));
            rx.recv_timeout(std::time::Duration::from_secs(30)).unwrap()
        }
    }
}

fn fields(value: &Value) -> &[Value] {
    let Value::Record(fields) = value else {
        panic!("expected record: {value:?}")
    };
    fields
}
fn list(value: &Value) -> &[Value] {
    let Value::List(items) = value else {
        panic!("expected list")
    };
    items
}
fn num(value: i32) -> Value {
    Value::Number(value.into())
}
fn success(value: &Value) -> &str {
    assert_eq!(fields(value)[1], Value::Bool(true), "{value:?}");
    fields(value)[3].as_str().unwrap()
}
fn save_args(id: &str, title: &str, token: &str) -> Vec<Value> {
    vec![
        Value::str(id),
        Value::str(title),
        Value::str("Bring the draft and café notes.\nSecond line."),
        Value::str("#747AFF"),
        Value::Bool(false),
        Value::str("2026-09-25"),
        Value::str("2026-09-30"),
        Value::str("09:30"),
        Value::str("17:15"),
        Value::str(token),
    ]
}
fn storage_error() -> Outcome {
    Outcome::Storage(br#"{"error":"Simulated storage interruption."}"#.to_vec())
}

struct Sql {
    transaction: bool,
    commands: Json,
}
impl DataSource for Sql {
    fn app_id(&self) -> &str {
        macos_calendar_data::APP
    }
    fn grants(&self) -> &str {
        GRANTS
    }
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        Ok(Value::Unit)
    }
    fn answer(&mut self, _: &mut Store, _: &str, _: &[Value]) -> Result<Answer, DataError> {
        Ok(Answer::Later(exact_data::storage::request(
            if self.transaction {
                "sqlite.transaction"
            } else {
                "sqlite"
            },
            json!({ "path": "app:/data/calendar.db", "commands": self.commands }),
        )))
    }
    fn parse(
        &mut self,
        _: &mut Store,
        _: &str,
        _: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        let result = exact_data::storage::response(outcome).map_err(DataError::Unavailable)?;
        Ok(Answer::Now(Value::str(&result.to_string())))
    }
}

fn sql(root: &Root, transaction: bool, commands: Json) -> Json {
    let mut data = Storage::new(Sql {
        transaction,
        commands,
    });
    data.configure_storage(
        root.0.join("data"),
        root.0.join("cache"),
        root.0.join("temporary"),
    )
    .unwrap();
    data.activate().unwrap();
    let mut store = Store::new(GRANTS, Vec::<(String, String)>::new());
    let Answer::Later(request) = data.answer(&mut store, "fixture", &[]).unwrap() else {
        panic!("fixture request")
    };
    let Dispatch::Run(work) = data.dispatch(request.continuation.unwrap(), &store) else {
        panic!("fixture work")
    };
    let Answer::Now(value) = data.parse(&mut store, "fixture", &[], run(work)).unwrap() else {
        panic!("fixture result")
    };
    serde_json::from_str(value.as_str().unwrap()).unwrap()
}

fn legacy_database(root: &Root) -> String {
    let args = save_args("", "Legacy schedule", "legacy-create");
    let values: Vec<Json> = args[..9]
        .iter()
        .map(|value| match value {
            Value::Bool(value) => json!(value),
            value => json!(value.as_str().unwrap()),
        })
        .collect();
    let fingerprint = json!(["saveSchedule", values]).to_string();
    sql(
        root,
        true,
        json!([
            {"kind":"execute","sql":"CREATE TABLE schedules (id TEXT PRIMARY KEY NOT NULL,title TEXT NOT NULL,notes TEXT NOT NULL,color TEXT NOT NULL,all_day INTEGER NOT NULL,start_day INTEGER NOT NULL,end_day INTEGER NOT NULL,start_minute INTEGER NOT NULL,end_minute INTEGER NOT NULL)","params":[]},
            {"kind":"execute","sql":"CREATE TABLE calendar_meta (key TEXT PRIMARY KEY NOT NULL,value INTEGER NOT NULL)","params":[]},
            {"kind":"execute","sql":"CREATE TABLE calendar_operations (token TEXT PRIMARY KEY NOT NULL,fingerprint TEXT NOT NULL,revision INTEGER NOT NULL,event_id TEXT NOT NULL,day INTEGER NOT NULL)","params":[]},
            {"kind":"execute","sql":"INSERT INTO calendar_meta VALUES('seeded',1),('revision',12)","params":[]},
            {"kind":"execute","sql":"INSERT INTO schedules VALUES(?,?,?,?,?,?,?,?,?)","params":["event-legacy-create","Legacy schedule",args[2].as_str().unwrap(),"#747AFF",0,TODAY+24,TODAY+29,570,1035]},
            {"kind":"execute","sql":"INSERT INTO calendar_operations VALUES(?,?,?,?,?)","params":["legacy-create",fingerprint,12,"event-legacy-create",TODAY+24]}
        ]),
    );
    fingerprint
}

fn todo_args(id: &str, title: &str, token: &str) -> Vec<Value> {
    let mut args = save_args(id, title, token);
    args[4] = Value::Bool(true);
    args[6] = args[5].clone();
    args
}

fn date_cell(page: &Value, day: i32) -> (&[Value], &[Value]) {
    for week in list(&fields(page)[3]) {
        for cell in list(&fields(week)[1]) {
            if fields(cell)[1] == num(day) {
                return (fields(week), fields(cell));
            }
        }
    }
    panic!("date was not in the month page");
}

#[test]
fn unknown_boot_clock_creates_no_storage_and_real_clock_seeds_the_trip_months() {
    let root = Root::new();
    let mut app = App::open(&root);
    let clock = app.call("calendarClock", vec![num(0), num(0)]);
    assert_eq!(fields(&clock)[3], Value::Bool(false));
    let waiting = app.call(
        "loadCalendar",
        vec![fields(&clock)[0].clone(), fields(&clock)[3].clone()],
    );
    assert_eq!(fields(&waiting)[0], Value::Bool(false));
    assert_eq!(app.requests, 0);
    assert!(
        !root.0.exists(),
        "an unreported clock must not create storage"
    );

    let clock = app.call(
        "calendarClock",
        vec![Value::Number(f64::from(TODAY) * 86_400_000.0), num(0)],
    );
    assert_eq!(fields(&clock)[0], num(TODAY));
    assert_eq!(fields(&clock)[3], Value::Bool(true));
    let loaded = app.call(
        "loadCalendar",
        vec![fields(&clock)[0].clone(), fields(&clock)[3].clone()],
    );
    assert_eq!(fields(&loaded)[0], Value::Bool(true));
    assert_eq!(fields(&loaded)[1], num(1));
    app.event("sample-2026-09-0");
    let epoch_day = app.call("calendarDay", vec![num(0), num(1), num(TODAY)]);
    assert!(list(&fields(&epoch_day)[3]).is_empty());
}

#[test]
fn changing_day_during_initial_load_replaces_the_retired_read() {
    let root = Root::new();
    let mut app = App::open(&root);
    let old_args = vec![num(TODAY - 1), Value::Bool(true)];
    let old = app
        .data
        .answer(&mut app.store, "loadCalendar", &old_args)
        .unwrap();
    let Answer::Later(old) = old else {
        panic!("the first load needs storage")
    };
    let args = vec![num(TODAY), Value::Bool(true)];
    let next = app
        .data
        .answer(&mut app.store, "loadCalendar", &args)
        .unwrap();
    let Answer::Later(request) = &next else {
        panic!("new arguments must restart the retired read")
    };
    app.data.forgotten(&[InFlight {
        target: Target::Mutation(0),
        source: "loadCalendar",
        args: &args,
        continuation: request.continuation,
    }]);
    assert!(matches!(
        app.data.dispatch(old.continuation.unwrap(), &app.store),
        Dispatch::Missing
    ));
    let loaded = app.finish("loadCalendar", &args, next);
    assert_eq!(fields(&loaded)[0], Value::Bool(true));
    app.event("sample-2026-09-0");
    let august = app.call("calendarDay", vec![num(TODAY - 29), num(1), num(TODAY)]);
    assert!(list(&fields(&august)[3]).is_empty());
}

#[test]
fn create_edit_delete_and_notes_survive_a_complete_restart() {
    let root = Root::new();
    let mut app = App::open(&root);
    assert_eq!(app.load(), 1);
    let saved = app.save("", "Team planning", "create-1");
    let id = success(&saved).to_owned();
    assert_eq!(fields(&saved)[0], num(2));
    drop(app);

    let mut app = App::open(&root);
    assert_eq!(app.load(), 2);
    let opened = app.event(&id);
    assert_eq!(fields(&opened)[1].as_str(), Some("Team planning"));
    assert_eq!(
        fields(&opened)[2].as_str(),
        Some("Bring the draft and café notes.\nSecond line.")
    );
    assert_eq!(success(&app.save(&id, "Updated title", "edit-1")), id);
    drop(app);

    let mut app = App::open(&root);
    assert_eq!(app.load(), 3);
    assert_eq!(fields(&app.event(&id))[1].as_str(), Some("Updated title"));
    let deleted = app.call(
        "deleteSchedule",
        vec![Value::str(&id), Value::str("delete-1")],
    );
    success(&deleted);
    drop(app);

    let mut app = App::open(&root);
    assert_eq!(app.load(), 4);
    let opened = app.call("calendarEvent", vec![Value::str(&id), num(4)]);
    assert_eq!(fields(&opened)[0], Value::Bool(false));
    let replay = app.call(
        "deleteSchedule",
        vec![Value::str(&id), Value::str("delete-1")],
    );
    success(&replay);
    assert_eq!(fields(&replay)[0], num(4));
}

#[test]
fn range_moves_are_absolute_and_replays_do_not_undo_later_moves() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.load();
    let mut args = save_args("", "Six days", "range-create");
    args[4] = Value::Bool(true);
    let id = success(&app.call("saveSchedule", args)).to_owned();
    let moved = app.call(
        "moveSchedule",
        vec![Value::str(&id), num(20_716), Value::str("move-1")],
    );
    success(&moved);
    let event = app.event(&id);
    assert_eq!(fields(&event)[7].as_str(), Some("2026-09-20"));
    assert_eq!(fields(&event)[8].as_str(), Some("2026-09-25"));
    assert_eq!(fields(&event)[9].as_str(), Some("09:30"));
    assert_eq!(fields(&event)[10].as_str(), Some("17:15"));
    success(&app.call(
        "moveSchedule",
        vec![Value::str(&id), num(20_757), Value::str("move-2")],
    ));
    let replay = app.call(
        "moveSchedule",
        vec![Value::str(&id), num(20_716), Value::str("move-1")],
    );
    success(&replay);
    assert_eq!(fields(&replay)[0], num(4));
    assert_eq!(fields(&app.event(&id))[7].as_str(), Some("2026-10-31"));
    drop(app);

    let mut app = App::open(&root);
    assert_eq!(app.load(), 4);
    success(&app.call(
        "moveSchedule",
        vec![Value::str(&id), num(20_716), Value::str("move-1")],
    ));
    assert_eq!(fields(&app.event(&id))[8].as_str(), Some("2026-11-05"));
    let conflict = app.call(
        "moveSchedule",
        vec![Value::str(&id), num(TODAY), Value::str("move-1")],
    );
    assert_eq!(fields(&conflict)[1], Value::Bool(false));
    assert_eq!(fields(&app.event(&id))[7].as_str(), Some("2026-10-31"));
}

#[test]
fn committed_write_with_lost_reply_is_reconciled_without_another_write() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.load();
    let args = save_args("", "Exactly once", "lost-create");
    let lookup = app
        .data
        .answer(&mut app.store, "saveSchedule", &args)
        .unwrap();
    let reply = run(app.work(lookup));
    let write = app
        .data
        .parse(&mut app.store, "saveSchedule", &args, reply)
        .unwrap();
    let committed = run(app.work(write));
    assert!(exact_data::storage::response(committed).is_ok());
    let failure = app
        .data
        .parse(&mut app.store, "saveSchedule", &args, storage_error())
        .unwrap();
    let recovered = app.finish("saveSchedule", &args, failure);
    assert_eq!(success(&recovered), "event-lost-create");
    assert_eq!(fields(&recovered)[0], num(2));
    let retried = app.call("saveSchedule", args);
    let id = success(&retried);
    assert_eq!(fields(&retried)[0], num(2));
    assert_eq!(fields(&app.event(id))[1].as_str(), Some("Exactly once"));
    let day = app.call("calendarDay", vec![num(20_721), num(2), num(TODAY)]);
    assert_eq!(
        list(&fields(&day)[3])
            .iter()
            .filter(|e| fields(e)[0].as_str() == Some(id))
            .count(),
        1
    );
}

#[test]
fn concurrent_revision_conflict_rolls_back_and_retry_refreshes_the_index() {
    let root = Root::new();
    let mut first = App::open(&root);
    let mut second = App::open(&root);
    first.load();
    second.load();
    let args = save_args("", "First window", "first-window");
    let lookup = first
        .data
        .answer(&mut first.store, "saveSchedule", &args)
        .unwrap();
    let reply = run(first.work(lookup));
    let write = first
        .data
        .parse(&mut first.store, "saveSchedule", &args, reply)
        .unwrap();
    success(&second.save("", "Second window", "second-window"));
    let reply = run(first.work(write));
    let rejected = first
        .data
        .parse(&mut first.store, "saveSchedule", &args, reply)
        .unwrap();
    let rejected = first.finish("saveSchedule", &args, rejected);
    assert_eq!(fields(&rejected)[1], Value::Bool(false));
    let retried = first.call("saveSchedule", args);
    success(&retried);
    assert_eq!(fields(&retried)[0], num(3));
    assert_eq!(
        fields(&first.event("event-second-window"))[1].as_str(),
        Some("Second window")
    );
    drop(first);
    drop(second);
    let mut reopened = App::open(&root);
    assert_eq!(reopened.load(), 3);
    reopened.event("event-first-window");
    reopened.event("event-second-window");
}

#[test]
fn deleting_every_sample_does_not_reseed_a_reopened_calendar() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.load();
    let rows = sql(
        &root,
        false,
        json!([{"kind":"query","sql":"SELECT id FROM schedules ORDER BY id","params":[]}]),
    );
    let ids = rows[0]["rows"].as_array().unwrap();
    assert!(ids.len() > 7, "both sample months must be populated");
    for (i, row) in ids.iter().enumerate() {
        success(&app.call(
            "deleteSchedule",
            vec![
                Value::str(row[0].as_str().unwrap()),
                Value::str(&format!("delete-sample-{i}")),
            ],
        ));
    }
    drop(app);
    let mut app = App::open(&root);
    let revision = ids.len() as i32 + 1;
    assert_eq!(app.load(), revision as u64);
    for day in TODAY..TODAY + 61 {
        let agenda = app.call("calendarDay", vec![num(day), num(revision), num(TODAY)]);
        assert!(list(&fields(&agenda)[3]).is_empty());
    }
}

#[test]
fn paginated_summary_reload_keeps_large_notes_out_of_month_scrolls() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.load();
    let seeded = app.call("calendarDay", vec![num(TODAY + 24), num(1), num(TODAY)]);
    let seeded_count = list(&fields(&seeded)[3]).len();
    for i in 0..260 {
        let mut args = save_args("", &format!("Schedule {i}"), &format!("bulk-{i:03}"));
        args[2] = Value::str(&"Long notes. ".repeat(1000));
        success(&app.call("saveSchedule", args));
    }
    drop(app);
    let mut app = App::open(&root);
    assert_eq!(app.load(), 261);
    assert_eq!(
        app.requests, 5,
        "schema check, two schedule pages, stickers, snapshot verification"
    );
    let before = app.requests;
    for month in (2026 * 12 - 48)..(2026 * 12 + 48) {
        let pages = app.call("calendarMonths", vec![num(month), num(261)]);
        assert_eq!(list(&pages).len(), 7);
        for page in list(&pages) {
            assert!((4..=6).contains(&list(&fields(page)[3]).len()));
        }
    }
    let day = app.call("calendarDay", vec![num(20_721), num(261), num(TODAY)]);
    assert_eq!(
        app.requests, before,
        "paging and agenda summaries perform no storage work"
    );
    assert_eq!(list(&fields(&day)[3]).len(), 260 + seeded_count);
    assert!(list(&fields(&day)[3])
        .iter()
        .all(|event| fields(event)[2].as_str() == Some("")));
    assert_eq!(
        fields(&app.event("event-bulk-259"))[2]
            .as_str()
            .unwrap()
            .len(),
        12_000
    );
}

#[test]
fn invalid_dates_and_failed_writes_leave_stored_events_unchanged() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.load();
    let id = success(&app.save("", "Original title", "valid-create")).to_owned();
    let mut invalid = save_args(&id, "Bad date", "invalid-date");
    invalid[5] = Value::str("2026-02-30");
    assert_eq!(
        fields(&app.call("saveSchedule", invalid))[1],
        Value::Bool(false)
    );
    let args = save_args(&id, "Should not land", "failed-edit");
    let lookup = app
        .data
        .answer(&mut app.store, "saveSchedule", &args)
        .unwrap();
    let reply = run(app.work(lookup));
    let write = app
        .data
        .parse(&mut app.store, "saveSchedule", &args, reply)
        .unwrap();
    drop(app.work(write)); // The executor could not run this transaction.
    let failed = app
        .data
        .parse(&mut app.store, "saveSchedule", &args, storage_error())
        .unwrap();
    assert_eq!(
        fields(&app.finish("saveSchedule", &args, failed))[1],
        Value::Bool(false)
    );
    assert_eq!(fields(&app.event(&id))[1].as_str(), Some("Original title"));
    drop(app);
    let mut app = App::open(&root);
    assert_eq!(app.load(), 2);
    assert_eq!(fields(&app.event(&id))[1].as_str(), Some("Original title"));
}

#[test]
fn original_database_upgrades_without_reseeding_or_changing_saved_operation_results() {
    let root = Root::new();
    let fingerprint = legacy_database(&root);
    let mut app = App::open(&root);
    assert_eq!(app.load(), 12);
    let event = app.event("event-legacy-create");
    assert_eq!(fields(&event)[1].as_str(), Some("Legacy schedule"));
    assert_eq!(
        fields(&event)[2].as_str(),
        Some("Bring the draft and café notes.\nSecond line.")
    );
    assert_eq!(fields(&event)[12].as_str(), Some("event"));
    let replay = app.save("", "Legacy schedule", "legacy-create");
    assert_eq!(success(&replay), "event-legacy-create");
    assert_eq!(fields(&replay)[0], num(12));
    let inspection = sql(
        &root,
        false,
        json!([
            {"kind":"query","sql":"SELECT COUNT(*) FROM schedules","params":[]},
            {"kind":"query","sql":"SELECT fingerprint FROM calendar_operations WHERE token='legacy-create'","params":[]},
            {"kind":"query","sql":"SELECT name FROM sqlite_schema WHERE type='table' ORDER BY name","params":[]}
        ]),
    );
    assert_eq!(inspection[0]["rows"][0][0]["integer"], "1");
    assert_eq!(inspection[1]["rows"][0][0], fingerprint);
    assert_eq!(inspection[2]["rows"].as_array().unwrap().len(), 4);
    assert_eq!(fields(&event)[13], Value::Bool(false));
}

#[test]
fn legacy_undated_todos_become_dated_items_once_without_losing_completion_or_journal() {
    let root = Root::new();
    let fingerprint = legacy_database(&root);
    sql(
        &root,
        true,
        json!([
            {"kind":"execute","sql":"ALTER TABLE schedules ADD COLUMN kind TEXT NOT NULL DEFAULT 'event'","params":[]},
            {"kind":"execute","sql":"CREATE TABLE todos (id TEXT PRIMARY KEY NOT NULL,title TEXT NOT NULL,color TEXT NOT NULL,created_at INTEGER NOT NULL,completed_at INTEGER NOT NULL)","params":[]},
            {"kind":"execute","sql":"CREATE TABLE stickers (day INTEGER PRIMARY KEY NOT NULL,sticker_id TEXT NOT NULL)","params":[]},
            {"kind":"execute","sql":"INSERT INTO todos VALUES(?,?,?,?,?),(?,?,?,?,?)","params":["todo-legacy-open","Buy a travel adapter","#747AFF",i64::from(TODAY)*86_400_000+1_800_000,0,"todo-legacy-done","Reserve a room","#70B8A2",0,1_790_737_200_000_i64]},
            {"kind":"execute","sql":"INSERT INTO stickers VALUES(?,?)","params":[TODAY,"coffee"]}
        ]),
    );
    let mut app = App::open(&root);
    app.call(
        "calendarClock",
        vec![Value::Number(f64::from(TODAY) * 86_400_000.0), num(-60)],
    );
    assert_eq!(app.load(), 13);
    let open = app.event("todo-legacy-open");
    let done = app.event("todo-legacy-done");
    for item in [&open, &done] {
        assert_eq!(fields(item)[12].as_str(), Some("todo"));
        assert_eq!(fields(item)[4], Value::Bool(true));
        assert_eq!(fields(item)[5], fields(item)[6]);
        assert_eq!(fields(item)[2].as_str(), Some(""));
    }
    assert_eq!(
        fields(&open)[5],
        num(TODAY - 1),
        "legacy timestamp uses the known local UTC offset"
    );
    assert_eq!(fields(&open)[13], Value::Bool(false));
    assert_eq!(
        fields(&done)[5],
        num(TODAY),
        "unknown creation timestamp falls back to load date"
    );
    assert_eq!(fields(&done)[13], Value::Bool(true));
    let inspection = sql(
        &root,
        false,
        json!([
            {"kind":"query","sql":"SELECT name FROM sqlite_schema WHERE name='todos'","params":[]},
            {"kind":"query","sql":"SELECT fingerprint,revision FROM calendar_operations WHERE token='legacy-create'","params":[]},
            {"kind":"query","sql":"SELECT day,sticker_id FROM stickers","params":[]}
        ]),
    );
    assert!(inspection[0]["rows"].as_array().unwrap().is_empty());
    assert_eq!(inspection[1]["rows"][0][0], fingerprint);
    assert_eq!(inspection[1]["rows"][0][1]["integer"], "12");
    assert_eq!(inspection[2]["rows"][0][1], "coffee");
    drop(app);
    let mut reopened = App::open(&root);
    assert_eq!(reopened.load(), 13);
    assert_eq!(reopened.event("todo-legacy-open"), open);
    assert_eq!(reopened.event("todo-legacy-done"), done);
    assert_eq!(
        success(&reopened.save("", "Legacy schedule", "legacy-create")),
        "event-legacy-create"
    );
}

fn original_samples() -> Vec<Json> {
    [
        ("Design review","#747AFF",0,2,2,600,660,"Review the new calendar flow with the team."),
        ("Coffee with Alex","#F39B65",0,5,5,540,585,"Meet at the neighborhood café."),
        ("Focus time","#70B8A2",0,8,8,840,960,"An afternoon for uninterrupted work."),
        ("Weekend getaway","#EE8192",1,11,13,0,0,"A few days away. Long-press this item to move the whole trip."),
        ("Product workshop","#747AFF",1,16,18,0,0,"Planning, ideas, and a working prototype."),
        ("Dinner reservation","#F39B65",0,20,20,1140,1230,"A table for two."),
        ("Summer in Seoul","#70B8A2",1,24,29,0,0,"The 25th through the 30th, inclusive. Move it to the 20th to keep the full range through the 25th.")
    ].into_iter().enumerate().map(|(i,(title,color,all_day,start,end,start_time,end_time,notes))| json!({"kind":"execute","sql":"INSERT INTO schedules VALUES(?,?,?,?,?,?,?,?,?)","params":[format!("sample-2026-09-{i}"),title,notes,color,all_day,TODAY+start,TODAY+end,start_time,end_time]})).collect()
}

#[test]
fn sample_refresh_preserves_modified_deleted_and_user_rows_and_runs_only_once() {
    let root = Root::new();
    let fingerprint = legacy_database(&root);
    let mut commands = original_samples();
    commands.extend([
        json!({"kind":"execute","sql":"UPDATE schedules SET title='My custom review' WHERE id='sample-2026-09-0'","params":[]}),
        json!({"kind":"execute","sql":"UPDATE schedules SET notes='My private notes' WHERE id='sample-2026-09-1'","params":[]}),
        json!({"kind":"execute","sql":"DELETE FROM schedules WHERE id='sample-2026-09-3'","params":[]})
    ]);
    sql(&root, true, Json::Array(commands));
    let mut app = App::open(&root);
    assert_eq!(app.load(), 13);
    assert_eq!(
        fields(&app.event("sample-2026-09-0"))[1].as_str(),
        Some("My custom review")
    );
    assert_eq!(
        fields(&app.event("sample-2026-09-1"))[2].as_str(),
        Some("My private notes")
    );
    assert_eq!(
        fields(&app.call(
            "calendarEvent",
            vec![Value::str("sample-2026-09-3"), num(13)]
        ))[0],
        Value::Bool(false)
    );
    assert_eq!(
        fields(&app.event("event-legacy-create"))[1].as_str(),
        Some("Legacy schedule")
    );
    let refreshed = app.event("sample-2026-09-6");
    assert_eq!(fields(&refreshed)[1].as_str(), Some("San Francisco"));
    assert_eq!(fields(&refreshed)[12].as_str(), Some("plan"));
    assert_eq!(fields(&refreshed)[7].as_str(), Some("2026-09-08"));
    assert_eq!(fields(&refreshed)[8].as_str(), Some("2026-09-19"));
    let before = sql(
        &root,
        false,
        json!([
            {"kind":"query","sql":"SELECT * FROM schedules ORDER BY id","params":[]},
            {"kind":"query","sql":"SELECT fingerprint FROM calendar_operations WHERE token='legacy-create'","params":[]}
        ]),
    );
    assert_eq!(before[1]["rows"][0][0], fingerprint);
    drop(app);
    let mut reopened = App::open(&root);
    assert_eq!(reopened.load(), 13);
    let after = sql(
        &root,
        false,
        json!([{"kind":"query","sql":"SELECT * FROM schedules ORDER BY id","params":[]} ]),
    );
    assert_eq!(after[0], before[0]);
    assert_eq!(reopened.event("sample-2026-09-6"), refreshed);
}

#[test]
fn fresh_samples_keep_todos_on_one_date_and_events_and_plans_across_multiple_days() {
    let root = Root::new();
    let mut app = App::open(&root);
    assert_eq!(app.load(), 1);
    let rows = sql(
        &root,
        false,
        json!([{"kind":"query","sql":"SELECT id,kind,completed,start_day,end_day FROM schedules ORDER BY id","params":[]} ]),
    );
    let rows = rows[0]["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 48);
    for row in rows {
        let start = row[3]["integer"].as_str().unwrap().parse::<i32>().unwrap();
        let end = row[4]["integer"].as_str().unwrap().parse::<i32>().unwrap();
        if row[1] == "todo" {
            assert_eq!(end, start, "starter Todo uses one date: {}", row[0]);
        } else {
            assert!(
                end > start,
                "Event/Plan starter spans multiple days: {}",
                row[0]
            );
        }
        if end == start + 1 {
            assert_ne!(
                (start + 4).rem_euclid(7),
                6,
                "two-day starter bar must not start on Saturday: {}",
                row[0]
            );
        }
    }
    for month in ["2026-09", "2026-10"] {
        let month_rows: Vec<_> = rows
            .iter()
            .filter(|row| {
                row[0]
                    .as_str()
                    .unwrap()
                    .starts_with(&format!("sample-{month}-"))
            })
            .collect();
        assert_eq!(month_rows.len(), 24);
        for kind in ["event", "plan", "todo"] {
            assert!(month_rows.iter().any(|row| row[1] == kind));
        }
        for complete in ["0", "1"] {
            assert!(month_rows
                .iter()
                .any(|row| row[1] == "todo" && row[2]["integer"] == complete));
        }
    }
    let trip = app.event("sample-2026-09-6");
    assert_eq!(fields(&trip)[12].as_str(), Some("plan"));
    assert_eq!(
        fields(&trip)[6].as_number().unwrap() - fields(&trip)[5].as_number().unwrap() + 1.0,
        12.0
    );
    let japan = app.event("sample-2026-10-0");
    assert_eq!(fields(&japan)[7].as_str(), Some("2026-10-09"));
    assert_eq!(fields(&japan)[8].as_str(), Some("2026-10-22"));
    let marks = sql(
        &root,
        false,
        json!([{"kind":"query","sql":"SELECT day,sticker_id FROM stickers ORDER BY day","params":[]} ]),
    );
    let marks = marks[0]["rows"].as_array().unwrap();
    assert_eq!(marks.len(), 12);
    assert_eq!(
        marks
            .iter()
            .filter(|row| row[0]["integer"].as_str().unwrap().parse::<i32>().unwrap() < TODAY + 30)
            .count(),
        6
    );
    success(&app.call(
        "removeSticker",
        vec![num(TODAY + 1), Value::str("remove-starter-art")],
    ));
    drop(app);
    let mut reopened = App::open(&root);
    assert_eq!(reopened.load(), 2);
    let day = reopened.call("calendarDay", vec![num(TODAY + 1), num(2), num(TODAY)]);
    assert_eq!(
        fields(&day)[4].as_str(),
        Some(""),
        "removed starter art must not return on restart"
    );
}

#[test]
fn two_windows_reprobe_when_the_other_window_has_already_added_the_kind_column() {
    let root = Root::new();
    legacy_database(&root);
    let mut first = App::open(&root);
    let mut second = App::open(&root);
    let args = vec![num(TODAY), Value::Bool(true)];
    let first_probe = first
        .data
        .answer(&mut first.store, "loadCalendar", &args)
        .unwrap();
    let first_reply = run(first.work(first_probe));
    let first_migration = first
        .data
        .parse(&mut first.store, "loadCalendar", &args, first_reply)
        .unwrap();
    let second_probe = second
        .data
        .answer(&mut second.store, "loadCalendar", &args)
        .unwrap();
    let second_reply = run(second.work(second_probe));
    let second_migration = second
        .data
        .parse(&mut second.store, "loadCalendar", &args, second_reply)
        .unwrap();
    assert_eq!(
        fields(&first.finish("loadCalendar", &args, first_migration))[0],
        Value::Bool(true)
    );
    assert_eq!(
        fields(&second.finish("loadCalendar", &args, second_migration))[0],
        Value::Bool(true)
    );
    assert_eq!(
        fields(&second.event("event-legacy-create"))[12].as_str(),
        Some("event")
    );
    assert_eq!(second.load(), 12);
}

#[test]
fn plans_keep_their_type_notes_and_time_range_through_edits_moves_and_restarts() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.load();
    let result = app.call("savePlan", save_args("", "Plan the launch", "new-plan"));
    let id = success(&result).to_owned();
    let moved = app.call(
        "moveSchedule",
        vec![Value::str(&id), num(TODAY + 19), Value::str("move-plan")],
    );
    success(&moved);
    drop(app);
    let mut app = App::open(&root);
    assert_eq!(app.load(), 3);
    let event = app.event(&id);
    assert_eq!(fields(&event)[12].as_str(), Some("plan"));
    assert_eq!(fields(&event)[7].as_str(), Some("2026-09-20"));
    assert_eq!(fields(&event)[8].as_str(), Some("2026-09-25"));
    assert_eq!(fields(&event)[9].as_str(), Some("09:30"));
    assert_eq!(fields(&event)[10].as_str(), Some("17:15"));
    assert_eq!(
        fields(&event)[2].as_str(),
        Some("Bring the draft and café notes.\nSecond line.")
    );
    success(&app.call("savePlan", save_args(&id, "Updated plan", "edit-plan")));
    let page = app.call("calendarMonth", vec![num(24320), num(4)]);
    let bars: Vec<_> = list(&fields(&page)[3])
        .iter()
        .flat_map(|week| list(&fields(week)[2]))
        .filter(|bar| fields(bar)[1].as_str() == Some(&id))
        .collect();
    assert!(!bars.is_empty());
    assert!(bars
        .iter()
        .all(|bar| fields(bar)[11].as_str() == Some("plan")));
    success(&app.call(
        "deleteSchedule",
        vec![Value::str(&id), Value::str("delete-plan")],
    ));
    drop(app);
    let mut app = App::open(&root);
    app.load();
    assert_eq!(
        fields(&app.call("calendarEvent", vec![Value::str(&id), num(5)]))[0],
        Value::Bool(false)
    );
}

#[test]
fn dated_todos_preserve_dates_notes_and_completion_through_edits_moves_and_restarts() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.load();
    let id =
        success(&app.call("saveTodo", save_args("", "Book the ferry", "dated-todo"))).to_owned();
    let created = app.event(&id);
    assert_eq!(fields(&created)[12].as_str(), Some("todo"));
    assert_eq!(fields(&created)[13], Value::Bool(false));
    assert_eq!(fields(&created)[7].as_str(), Some("2026-09-25"));
    assert_eq!(
        fields(&created)[8].as_str(),
        Some("2026-09-25"),
        "saveTodo normalizes a different end date to its start date"
    );
    let complete = vec![
        Value::str(&id),
        Value::Bool(true),
        Value::Number(1_790_737_200_000.875),
        Value::str("complete-todo"),
    ];
    success(&app.call("setTodoCompleted", complete.clone()));
    assert_eq!(fields(&app.call("setTodoCompleted", complete))[0], num(3));
    success(&app.call(
        "saveTodo",
        save_args(&id, "Book the morning ferry", "edit-todo"),
    ));
    drop(app);

    let mut app = App::open(&root);
    assert_eq!(app.load(), 4);
    let edited = app.event(&id);
    assert_eq!(fields(&edited)[1].as_str(), Some("Book the morning ferry"));
    assert_eq!(fields(&edited)[2], fields(&created)[2]);
    assert_eq!(&fields(&edited)[4..11], &fields(&created)[4..11]);
    assert_eq!(fields(&edited)[13], Value::Bool(true));
    let agenda = app.call("calendarDay", vec![num(TODAY + 24), num(4), num(TODAY)]);
    let summary = list(&fields(&agenda)[3])
        .iter()
        .find(|item| fields(item)[0].as_str() == Some(&id))
        .unwrap();
    assert_eq!(fields(summary)[12].as_str(), Some("todo"));
    assert_eq!(fields(summary)[13], Value::Bool(true));
    let page = app.call("calendarMonth", vec![num(24320), num(4)]);
    let bars: Vec<_> = list(&fields(&page)[3])
        .iter()
        .flat_map(|week| list(&fields(week)[2]))
        .filter(|bar| fields(bar)[1].as_str() == Some(&id))
        .collect();
    assert!(!bars.is_empty());
    assert!(bars
        .iter()
        .all(|bar| fields(bar)[11].as_str() == Some("todo")
            && fields(bar)[12] == Value::Bool(true)
            && fields(bar)[5] == num(1)));
    let next_day = app.call("calendarDay", vec![num(TODAY + 25), num(4), num(TODAY)]);
    assert!(!list(&fields(&next_day)[3])
        .iter()
        .any(|item| fields(item)[0].as_str() == Some(&id)));
    success(&app.call(
        "moveSchedule",
        vec![Value::str(&id), num(TODAY + 60), Value::str("move-todo")],
    ));
    let moved = app.event(&id);
    assert_eq!(fields(&moved)[7].as_str(), Some("2026-10-31"));
    assert_eq!(fields(&moved)[8].as_str(), Some("2026-10-31"));
    assert_eq!(fields(&moved)[13], Value::Bool(true));
    assert_eq!(fields(&moved)[2], fields(&created)[2]);
    success(&app.call(
        "setTodoCompleted",
        vec![
            Value::str(&id),
            Value::Bool(false),
            Value::Number(1_790_737_200_030.0),
            Value::str("reopen-todo"),
        ],
    ));
    assert_eq!(fields(&app.event(&id))[13], Value::Bool(false));
    drop(app);

    let mut app = App::open(&root);
    assert_eq!(app.load(), 6);
    assert_eq!(fields(&app.event(&id))[13], Value::Bool(false));
    let rejected = app.call(
        "setTodoCompleted",
        vec![
            Value::str("sample-2026-09-6"),
            Value::Bool(true),
            Value::Number(1_790_737_200_040.0),
            Value::str("complete-event"),
        ],
    );
    assert_eq!(fields(&rejected)[1], Value::Bool(false));
    assert_eq!(fields(&rejected)[0], num(6));
    success(&app.call(
        "deleteSchedule",
        vec![Value::str(&id), Value::str("delete-todo")],
    ));
    drop(app);
    let mut app = App::open(&root);
    assert_eq!(app.load(), 7);
    assert_eq!(
        fields(&app.call("calendarEvent", vec![Value::str(&id), num(7)]))[0],
        Value::Bool(false)
    );
    for source in ["calendarTodos", "deleteTodo", "scheduleTodo"] {
        assert!(
            matches!(
                app.data.answer(&mut app.store, source, &[]),
                Err(DataError::UnknownSource(_))
            ),
            "{source} must be removed"
        );
    }
}

#[test]
fn dated_todo_moves_recover_lost_replies_without_changing_kind_or_duplicating_rows() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.load();
    let id = success(&app.call("saveTodo", todo_args("", "Take a walk", "walk"))).to_owned();
    let args = vec![Value::str(&id), num(TODAY + 6), Value::str("schedule-walk")];
    let lookup = app
        .data
        .answer(&mut app.store, "moveSchedule", &args)
        .unwrap();
    let lookup = run(app.work(lookup));
    let write = app
        .data
        .parse(&mut app.store, "moveSchedule", &args, lookup)
        .unwrap();
    assert!(exact_data::storage::response(run(app.work(write))).is_ok());
    let failed = app
        .data
        .parse(&mut app.store, "moveSchedule", &args, storage_error())
        .unwrap();
    let recovered = app.finish("moveSchedule", &args, failed);
    assert_eq!(success(&recovered), id);
    assert_eq!(fields(&recovered)[0], num(3));
    let result = app.call("moveSchedule", args.clone());
    assert_eq!(success(&result), id);
    assert_eq!(fields(&result)[0], num(3));
    assert_eq!(success(&app.call("moveSchedule", args)), id);
    let event = app.event(&id);
    assert_eq!(fields(&event)[1].as_str(), Some("Take a walk"));
    assert_eq!(fields(&event)[3].as_str(), Some("#747AFF"));
    assert_eq!(fields(&event)[4], Value::Bool(true));
    assert_eq!(fields(&event)[5], num(TODAY + 6));
    assert_eq!(fields(&event)[6], num(TODAY + 6));
    assert_eq!(fields(&event)[12].as_str(), Some("todo"));
    drop(app);
    let mut app = App::open(&root);
    assert_eq!(app.load(), 3);
    let agenda = app.call("calendarDay", vec![num(TODAY + 6), num(3), num(TODAY)]);
    assert_eq!(
        list(&fields(&agenda)[3])
            .iter()
            .filter(|item| fields(item)[0].as_str() == Some(&id))
            .count(),
        1
    );
}

#[test]
fn dated_todo_moves_roll_back_if_the_final_journal_insert_conflicts() {
    let root = Root::new();
    let mut first = App::open(&root);
    first.load();
    let id = success(&first.call("saveTodo", todo_args("", "Keep me", "keep"))).to_owned();
    let args = vec![Value::str(&id), num(TODAY), Value::str("claimed-token")];
    let lookup = first
        .data
        .answer(&mut first.store, "moveSchedule", &args)
        .unwrap();
    let outcome = run(first.work(lookup));
    let write = first
        .data
        .parse(&mut first.store, "moveSchedule", &args, outcome)
        .unwrap();
    let mut second = App::open(&root);
    second.load();
    let no_op = second.call(
        "removeSticker",
        vec![num(TODAY), Value::str("claimed-token")],
    );
    success(&no_op);
    assert_eq!(fields(&no_op)[0], num(2));
    let outcome = run(first.work(write));
    let failed = first
        .data
        .parse(&mut first.store, "moveSchedule", &args, outcome)
        .unwrap();
    assert_eq!(
        fields(&first.finish("moveSchedule", &args, failed))[1],
        Value::Bool(false)
    );
    drop(first);
    drop(second);
    let mut app = App::open(&root);
    assert_eq!(app.load(), 2);
    assert_eq!(
        fields(&app.call("calendarEvent", vec![Value::str(&id), num(2)]))[0],
        Value::Bool(true)
    );
    assert_eq!(fields(&app.event(&id))[5], num(TODAY + 24));
    assert_eq!(
        success(&app.call(
            "moveSchedule",
            vec![Value::str(&id), num(TODAY), Value::str("fresh-conversion")]
        )),
        id
    );
}

#[test]
fn stickers_replace_remove_and_invalidate_spill_cells_without_changing_event_lanes() {
    let root = Root::new();
    let mut app = stickers::without_starter_stickers(&root);
    let day = TODAY + 29;
    let september = app.call("calendarMonth", vec![num(24320), num(1)]);
    let october = app.call("calendarMonth", vec![num(24321), num(1)]);
    success(&app.call(
        "setSticker",
        vec![num(day), Value::str("sunshine"), Value::str("sunshine-1")],
    ));
    for (month, before) in [(24320, &september), (24321, &october)] {
        let after = app.call("calendarMonth", vec![num(month), num(2)]);
        let (week, cell) = date_cell(&after, day);
        assert_eq!(cell[5].as_str(), Some("sunshine"));
        assert_eq!(week[4], num(40));
        assert_eq!(
            date_cell(before, day).0[2],
            week[2],
            "stickers never change event lanes"
        );
    }
    success(&app.call(
        "setSticker",
        vec![num(day), Value::str("coffee"), Value::str("coffee-1")],
    ));
    let rows = sql(
        &root,
        false,
        json!([{"kind":"query","sql":"SELECT day,sticker_id FROM stickers","params":[]}]),
    );
    assert_eq!(rows[0]["rows"].as_array().unwrap().len(), 1);
    assert_eq!(rows[0]["rows"][0][1], "coffee");
    drop(app);
    let mut app = App::open(&root);
    assert_eq!(app.load(), 3);
    let agenda = app.call("calendarDay", vec![num(day), num(3), num(TODAY)]);
    assert_eq!(fields(&agenda)[4].as_str(), Some("coffee"));
    let before = app.requests;
    assert_eq!(
        fields(&app.call(
            "setSticker",
            vec![
                num(day),
                Value::str("invalid"),
                Value::str("invalid-sticker")
            ]
        ))[1],
        Value::Bool(false)
    );
    assert_eq!(app.requests, before);
    let removed = app.call("removeSticker", vec![num(day), Value::str("remove-coffee")]);
    success(&removed);
    assert_eq!(fields(&removed)[0], num(4));
    let repeated = app.call("removeSticker", vec![num(day), Value::str("remove-coffee")]);
    assert_eq!(fields(&repeated)[0], num(4));
    let page = app.call("calendarMonth", vec![num(24321), num(4)]);
    assert_eq!(date_cell(&page, day).1[5].as_str(), Some(""));
    assert_eq!(date_cell(&page, day).0[4], num(14));
}

#[test]
fn snapshot_retry_clears_dated_todos_and_stickers_changed_by_another_window() {
    let root = Root::new();
    let mut writer = App::open(&root);
    writer.load();
    let id = success(&writer.call("saveTodo", todo_args("", "Transient", "transient"))).to_owned();
    success(&writer.call(
        "setSticker",
        vec![
            num(TODAY),
            Value::str("sunshine"),
            Value::str("transient-sticker"),
        ],
    ));
    let mut reader = App::open(&root);
    let args = vec![num(TODAY), Value::Bool(true)];
    let mut pending = reader
        .data
        .answer(&mut reader.store, "loadCalendar", &args)
        .unwrap();
    for _ in 0..3 {
        let outcome = run(reader.work(pending));
        pending = reader
            .data
            .parse(&mut reader.store, "loadCalendar", &args, outcome)
            .unwrap();
    }
    success(&writer.call(
        "deleteSchedule",
        vec![Value::str(&id), Value::str("remove-transient")],
    ));
    success(&writer.call(
        "removeSticker",
        vec![num(TODAY), Value::str("remove-transient-sticker")],
    ));
    let loaded = reader.finish("loadCalendar", &args, pending);
    assert_eq!(fields(&loaded)[0], Value::Bool(true));
    assert_eq!(fields(&loaded)[1], num(5));
    assert_eq!(
        fields(&reader.call("calendarEvent", vec![Value::str(&id), num(5)]))[0],
        Value::Bool(false)
    );
    let agenda = reader.call("calendarDay", vec![num(TODAY), num(5), num(TODAY)]);
    assert_eq!(fields(&agenda)[4].as_str(), Some(""));
}

fn assert_landing_matches_bar(preview: &Value, page: &Value, day: i32, id: &str) {
    let preview = fields(preview);
    assert_eq!(preview[0], Value::Bool(true));
    assert_eq!(preview[1].as_str(), Some(id));
    let mut top = 0.0;
    for week in list(&fields(page)[3]) {
        let week = fields(week);
        if list(&week[1])
            .iter()
            .any(|cell| fields(cell)[1] == num(day))
        {
            let bar = list(&week[2])
                .iter()
                .find(|bar| fields(bar)[1].as_str() == Some(id))
                .unwrap();
            let bar = fields(bar);
            assert_eq!(preview[2], bar[11]);
            assert_eq!(
                preview[3],
                Value::Number(bar[4].as_number().unwrap() * 50.0 + 2.0)
            );
            assert_eq!(
                preview[4],
                Value::Number(top + 34.0 + bar[6].as_number().unwrap() * 24.0)
            );
            assert_eq!(
                preview[5],
                Value::Number(bar[5].as_number().unwrap() * 50.0 - 4.0)
            );
            assert_eq!(preview[6], num(20));
            assert_eq!(preview[7], bar[7]);
            assert_eq!(preview[8], bar[8]);
            return;
        }
        top +=
            (34.0 + week[3].as_number().unwrap() * 24.0 + week[4].as_number().unwrap()).max(76.0);
    }
    panic!("landing week missing");
}

#[test]
fn dense_landing_previews_match_committed_todo_and_plan_bars_without_changing_the_live_page() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.load();
    for n in 0..10 {
        let mut args = save_args("", &format!("Tie {n}"), &format!("lane-{n:02}"));
        args[4] = Value::Bool(true);
        args[5] = Value::str("2026-09-07");
        args[6] = Value::str("2026-09-07");
        args[7] = Value::str("00:00");
        args[8] = Value::str("00:00");
        success(&app.call("saveSchedule", args));
    }
    let mut args = save_args("", "Earlier plan", "earlier-plan");
    args[4] = Value::Bool(true);
    args[5] = Value::str("2026-09-01");
    args[6] = Value::str("2026-09-07");
    let plan = success(&app.call("savePlan", args)).to_owned();
    let todo =
        success(&app.call("saveTodo", todo_args("", "One more tie", "dense-todo"))).to_owned();
    success(&app.call(
        "setSticker",
        vec![
            num(TODAY),
            Value::str("sunshine"),
            Value::str("preview-sticker"),
        ],
    ));
    let before = app.call("calendarMonth", vec![num(24320), num(14)]);
    let requests = app.requests;
    let preview = app.call(
        "calendarLanding",
        vec![
            Value::str(&todo),
            num(TODAY + 6),
            num(24320),
            num(14),
            num(350),
            num(76),
        ],
    );
    assert_eq!(
        app.requests, requests,
        "prospective layout performs no storage work"
    );
    assert_eq!(app.call("calendarMonth", vec![num(24320), num(14)]), before);
    assert_eq!(fields(&app.event(&todo))[5], num(TODAY + 24));
    let conversion = app.call(
        "moveSchedule",
        vec![
            Value::str(&todo),
            num(TODAY + 6),
            Value::str("a-conversion"),
        ],
    );
    assert_eq!(success(&conversion), todo);
    let after = app.call("calendarMonth", vec![num(24320), num(15)]);
    assert_landing_matches_bar(&preview, &after, TODAY + 6, &todo);
    let agenda = app.call("calendarDay", vec![num(TODAY + 6), num(15), num(TODAY)]);
    let items = list(&fields(&agenda)[3]);
    assert_eq!(
        fields(items.last().unwrap())[12].as_str(),
        Some("todo"),
        "Todos remain distinct dated items after Events and Plans"
    );

    let preview = app.call(
        "calendarLanding",
        vec![
            Value::str(&plan),
            num(TODAY + 10),
            num(24320),
            num(15),
            num(350),
            num(76),
        ],
    );
    success(&app.call(
        "moveSchedule",
        vec![
            Value::str(&plan),
            num(TODAY + 10),
            Value::str("move-preview-plan"),
        ],
    ));
    let after = app.call("calendarMonth", vec![num(24320), num(16)]);
    assert_landing_matches_bar(&preview, &after, TODAY + 10, &plan);
    assert_eq!(
        fields(&preview)[8],
        Value::Bool(true),
        "a seven-day Plan continues into the next week"
    );
}
