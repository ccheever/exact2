//! Real portable SQLite execution: restarts, transactions, retries and paging.

use calendar_data::{Calendar, GRANTS};
use exact_data_host::Storage;
use exact_plan::Value;
use exact_runner::{Answer, DataSource, Dispatch, InFlight, Outcome, Reply, Store, Target, Work};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

const TODAY: i32 = 20_697; // 2026-09-01, days since 1970-01-01.

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

#[test]
fn unknown_boot_clock_creates_no_storage_and_real_clock_seeds_the_current_month() {
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
    let epoch_day = app.call("calendarDay", vec![num(0), num(1)]);
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
    let august = app.call("calendarDay", vec![num(TODAY - 29), num(1)]);
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
fn committed_write_with_lost_reply_is_reconciled_before_retry() {
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
    assert_eq!(
        fields(&app.finish("saveSchedule", &args, failure))[1],
        Value::Bool(false)
    );
    let retried = app.call("saveSchedule", args);
    let id = success(&retried);
    assert_eq!(fields(&retried)[0], num(2));
    assert_eq!(fields(&app.event(id))[1].as_str(), Some("Exactly once"));
    let day = app.call("calendarDay", vec![num(20_721), num(2)]);
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
    for i in 0..7 {
        success(&app.call(
            "deleteSchedule",
            vec![
                Value::str(&format!("sample-2026-09-{i}")),
                Value::str(&format!("delete-sample-{i}")),
            ],
        ));
    }
    drop(app);
    let mut app = App::open(&root);
    assert_eq!(app.load(), 8);
    for day in TODAY..TODAY + 31 {
        let agenda = app.call("calendarDay", vec![num(day), num(8)]);
        assert!(list(&fields(&agenda)[3]).is_empty());
    }
}

#[test]
fn paginated_summary_reload_keeps_large_notes_out_of_month_scrolls() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.load();
    for i in 0..260 {
        let mut args = save_args("", &format!("Schedule {i}"), &format!("bulk-{i:03}"));
        args[2] = Value::str(&"Long notes. ".repeat(1000));
        success(&app.call("saveSchedule", args));
    }
    drop(app);
    let mut app = App::open(&root);
    assert_eq!(app.load(), 261);
    assert_eq!(
        app.requests, 4,
        "schema check, two summary pages, snapshot verification"
    );
    let before = app.requests;
    for month in (2026 * 12 - 48)..(2026 * 12 + 48) {
        let pages = app.call("calendarMonths", vec![num(month), num(261)]);
        assert_eq!(list(&pages).len(), 7);
        for page in list(&pages) {
            assert!((4..=6).contains(&list(&fields(page)[3]).len()));
        }
    }
    let day = app.call("calendarDay", vec![num(20_721), num(261)]);
    assert_eq!(
        app.requests, before,
        "paging and agenda summaries perform no storage work"
    );
    assert_eq!(list(&fields(&day)[3]).len(), 261);
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
