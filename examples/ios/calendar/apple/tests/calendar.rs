//! The real baked Contract, runner actions and native SQLite across restarts.

use calendar_data::Calendar;
use exact_data_host::Storage;
use exact_kernel::{Env, Frame, Kernel, NodeKey, Offer, PropId};
use exact_plan::{Plan, Value};
use exact_runner::{
    DataSource, Dispatch, Event, Outcome, Reply, RequestOut, Runner, RunnerError, ScrollEvent,
    Viewport, Work,
};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

const TODAY: i32 = 20_697; // 2026-09-01.
const WIDTH: f32 = 402.0;
const HEIGHT: f32 = 874.0;
type CalendarRunner = Runner<Storage<Calendar>>;

#[path = "calendar/features.rs"]
mod features;

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        Self(std::env::temp_dir().join(format!(
            "calendar-contract-{}-{}",
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
    runner: CalendarRunner,
    lose_write_reply_after: Option<usize>,
    fail_write_reply_after: Option<usize>,
}
impl App {
    fn open(root: &Root) -> Self {
        let existed = root.0.exists();
        let mut data = Storage::new(Calendar::default());
        data.configure_storage(
            root.0.join("data"),
            root.0.join("cache"),
            root.0.join("temporary"),
        )
        .unwrap();
        let mut kernel = Kernel::with_monospace();
        kernel.set_env(Env::new(62.0, 0.0, 34.0, 0.0)).unwrap();
        let mut runner = Runner::boot(
            Plan::decode(calendar_apple::PLAN).unwrap(),
            data,
            kernel,
            Viewport {
                width: f64::from(WIDTH),
                height: f64::from(HEIGHT),
                ..Viewport::default()
            },
            "/",
        )
        .unwrap();
        assert_eq!(
            root.0.exists(),
            existed,
            "the unreported bake clock must not seed storage"
        );
        runner.data().activate().unwrap();
        runner.data_ready().unwrap();
        runner
            .set_time(f64::from(TODAY) * 86_400_000.0, 0.0)
            .unwrap();
        assert!(runner.frame(0.0).error.is_none());
        let mut app = Self {
            runner,
            lose_write_reply_after: None,
            fail_write_reply_after: None,
        };
        app.settle();
        assert!(app.runner.frame(1.0).error.is_none());
        app.settle();
        assert_eq!(app.state("libraryReady"), &Value::Bool(true));
        app
    }

    fn layout(&mut self) {
        for root in self.runner.roots().to_vec() {
            self.runner
                .kernel_mut()
                .compute_layout(root, Offer::definite(WIDTH, HEIGHT))
                .unwrap();
        }
    }

    fn settle(&mut self) {
        let mut parked: BTreeMap<u64, RequestOut> = BTreeMap::new();
        for _ in 0..400 {
            self.runner.advance(self.runner.now_ms()).unwrap();
            let mut work = Vec::new();
            {
                let mut take = |request: RequestOut, dispatch: Dispatch| match dispatch {
                    Dispatch::Run(item) => {
                        work.push((request.ticket, item, request.target == "changed"))
                    }
                    Dispatch::Held => {
                        parked.insert(request.request.continuation.unwrap(), request);
                    }
                    Dispatch::Host(_) | Dispatch::Missing => {
                        panic!("calendar requested non-storage host work")
                    }
                };
                for request in self.runner.take_requests() {
                    assert!(
                        request.request.url.is_empty(),
                        "calendar must remain offline"
                    );
                    let dispatch = self
                        .runner
                        .dispatch_work(request.request.continuation.unwrap());
                    take(request, dispatch);
                }
            }
            for (token, dispatch) in self.runner.release_work() {
                if let Some(request) = parked.remove(&token) {
                    match dispatch {
                        Dispatch::Run(item) => {
                            work.push((request.ticket, item, request.target == "changed"))
                        }
                        Dispatch::Held => {
                            parked.insert(token, request);
                        }
                        _ => panic!("calendar lost native storage work"),
                    }
                }
            }
            if work.is_empty()
                && parked.is_empty()
                && !self.runner.has_pending()
                && self
                    .runner
                    .timer_due_ms()
                    .is_none_or(|due| due > self.runner.now_ms())
            {
                self.layout();
                return;
            }
            assert!(
                !work.is_empty(),
                "pending calendar work cannot make progress"
            );
            for (ticket, item, mutation_request) in work {
                if mutation_request {
                    if let Some(remaining) = &mut self.lose_write_reply_after {
                        *remaining -= 1;
                    }
                    if let Some(remaining) = &mut self.fail_write_reply_after {
                        *remaining -= 1;
                    }
                }
                let mut outcome = if self.fail_write_reply_after == Some(0) {
                    self.fail_write_reply_after = None;
                    Outcome::Storage(br#"{"error":"Simulated failed storage write."}"#.to_vec())
                } else {
                    run(item)
                };
                if self.lose_write_reply_after == Some(0) {
                    let Outcome::Storage(bytes) = &outcome else {
                        panic!("expected a completed SQLite transaction");
                    };
                    let response: serde_json::Value = serde_json::from_slice(bytes).unwrap();
                    assert!(response.get("error").is_none(), "{response}");
                    self.lose_write_reply_after = None;
                    outcome = Outcome::Storage(
                        br#"{"error":"Simulated storage interruption."}"#.to_vec(),
                    );
                }
                self.runner.fulfill(ticket, outcome).unwrap();
            }
        }
        panic!("calendar did not settle");
    }

    fn dispatch(&mut self, test_id: &str, event: Event) {
        let view = self
            .runner
            .kernel()
            .node_by_key(self.key(test_id))
            .unwrap()
            .id;
        self.runner.dispatch(view, event).unwrap();
    }
    fn event(&mut self, test_id: &str, event: Event) {
        self.dispatch(test_id, event);
        self.settle();
    }
    fn press(&mut self, test_id: &str) {
        self.event(test_id, Event::Press);
    }
    fn input(&mut self, test_id: &str, value: &str) {
        self.event(test_id, Event::Input(value.into()));
    }
    fn create(&mut self, kind: &str) {
        self.press(if self.state("popupOpen") == &Value::Bool(true) {
            "popup-add"
        } else {
            "add-schedule"
        });
        self.press(&format!("picker-{kind}"));
        self.finish_motion();
    }
    fn finish_motion(&mut self) {
        for _ in 0..72 {
            self.presented_frame(1000.0 / 120.0);
        }
    }
    fn assert_creation_closed(&self) {
        for id in [
            "date-popup",
            "type-picker",
            "schedule-editor",
            "todo-editor",
            "sticker-editor",
        ] {
            assert!(
                self.runner.kernel().find_by_test_id(id).is_empty(),
                "global creation unexpectedly opens {id}"
            );
        }
    }
    fn presented_frame(&mut self, elapsed_ms: f64) {
        assert!(self
            .runner
            .frame(self.runner.now_ms() + elapsed_ms)
            .error
            .is_none());
        self.settle();
    }
    fn state(&self, name: &str) -> &Value {
        self.runner.slot(name).unwrap()
    }
    fn derived(&self, name: &str) -> &Value {
        self.runner.derive(name).unwrap()
    }
    fn agenda_id(&self, title: &str) -> String {
        items(&fields(self.runner.resource("agenda").unwrap())[3])
            .iter()
            .find(|item| fields(item)[1].as_str() == Some(title))
            .map(|item| fields(item)[0].as_str().unwrap().to_owned())
            .unwrap_or_else(|| panic!("agenda has no {title}"))
    }
    fn date_for_day(&mut self, day: f64) -> String {
        let revision = self.derived("revision").clone();
        let value = self
            .runner
            .data()
            .query(
                "calendarDay",
                &[
                    Value::Number(day),
                    revision,
                    Value::Number(f64::from(TODAY)),
                ],
            )
            .unwrap();
        fields(&value)[1].as_str().unwrap().to_owned()
    }
    fn key(&self, test_id: &str) -> NodeKey {
        let keys = self.runner.kernel().find_by_test_id(test_id);
        assert_eq!(keys.len(), 1, "expected one mounted {test_id}");
        keys[0]
    }
    fn frame(&self, test_id: &str) -> Frame {
        self.runner
            .kernel()
            .laid_out_frame(self.key(test_id))
            .unwrap()
            .0
    }
    fn translation_y(&self, test_id: &str) -> f32 {
        self.runner
            .kernel()
            .node_by_key(self.key(test_id))
            .unwrap()
            .style
            .translate
            .y
    }
    fn content_height(&self, test_id: &str) -> f32 {
        self.runner
            .kernel()
            .node_by_key(self.key(test_id))
            .unwrap()
            .content
            .1
    }
    fn text(&self, test_id: &str) -> &str {
        self.runner
            .kernel()
            .node_by_key(self.key(test_id))
            .unwrap()
            .props
            .str(PropId::Text)
            .unwrap()
    }
    fn center(&self, test_id: &str) -> (f64, f64) {
        let frame = self.frame(test_id);
        let root = self.frame("calendar");
        let page_translation = if test_id.starts_with("date-") {
            (self.derived("month").as_number().unwrap()
                - self.derived("pagerPosition").as_number().unwrap())
                * f64::from(WIDTH)
        } else {
            0.0
        };
        (
            f64::from(frame.x - root.x + frame.width / 2.0) + page_translation,
            f64::from(frame.y - root.y + frame.height / 2.0),
        )
    }
}

struct Contact {
    id: String,
    serial: u64,
    source: Frame,
}
impl Contact {
    fn new(app: &App, id: &str, serial: u64) -> Self {
        Self::from_source(app, id, serial, &format!("agenda-item-{id}"))
    }
    fn from_source(app: &App, id: &str, serial: u64, test_id: &str) -> Self {
        let mut source = app.frame(test_id);
        let root = app.frame("calendar");
        source.x -= root.x;
        source.y -= root.y;
        Self {
            id: id.into(),
            serial,
            source,
        }
    }
    fn origin(&self) -> (f64, f64) {
        (
            f64::from(self.source.x + self.source.width / 2.0),
            f64::from(self.source.y + self.source.height / 2.0),
        )
    }
    fn event(&self, sequence: u64, phase: &str, point: (f64, f64), reason: &str) -> Event {
        Event::Message(
            serde_json::json!({
                "v": 1, "phase": phase, "id": self.id, "serial": self.serial, "seq": sequence,
                "x": point.0, "y": point.1, "reason": reason,
                "rx": self.source.x, "ry": self.source.y,
                "rw": self.source.width, "rh": self.source.height,
            })
            .to_string(),
        )
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
    match value {
        Value::Record(fields) => fields,
        _ => panic!("expected record: {value:?}"),
    }
}
fn items(value: &Value) -> &[Value] {
    match value {
        Value::List(items) => items,
        _ => panic!("expected list: {value:?}"),
    }
}

#[test]
fn launch_uses_the_reported_month_and_opens_the_selected_dates_agenda() {
    let root = Root::new();
    let mut app = App::open(&root);
    assert_eq!(app.derived("month"), &Value::Number(2026.0 * 12.0 + 8.0));
    let last_week = app.frame("date-2026-09-2026-10-03");
    // Dense sample weeks may overflow the viewport; their content remains scrollable.
    assert!(last_week.y + last_week.height >= HEIGHT);
    assert_eq!(
        last_week.y + last_week.height,
        app.frame("month-scroll-2026-09").y + app.content_height("month-scroll-2026-09")
    );
    app.press("date-2026-09-2026-09-16");
    assert_eq!(app.state("popupOpen"), &Value::Bool(true));
    assert_eq!(app.agenda_id("San Francisco"), "sample-2026-09-6");
}

#[test]
fn the_sheet_backdrop_covers_the_calendar_and_dismisses_without_selecting_another_date() {
    let root = Root::new();
    let mut app = App::open(&root);
    assert!(app
        .runner
        .kernel()
        .find_by_test_id("date-popup-backdrop")
        .is_empty());
    app.press("date-2026-09-2026-09-25");
    let selected = app.derived("day").clone();
    let month = app.derived("month").clone();
    let position = app.derived("pagerPosition").clone();
    assert_eq!(
        app.frame("date-popup-backdrop"),
        Frame {
            x: 0.0,
            y: 0.0,
            width: WIDTH,
            height: HEIGHT,
        }
    );
    assert!(
        app.runner
            .kernel()
            .node_by_key(app.key("date-popup-backdrop"))
            .unwrap()
            .style
            .z_index
            > app
                .runner
                .kernel()
                .node_by_key(app.key("calendar-pages"))
                .unwrap()
                .style
                .z_index
    );
    let backdrop = app
        .runner
        .kernel()
        .node_by_key(app.key("date-popup-backdrop"))
        .unwrap()
        .id;
    assert!(matches!(
        app.runner.dispatch(backdrop, Event::Pan(-140.0, 0.0)),
        Err(RunnerError::NoHandler { event: "pan", .. })
    ));
    assert_eq!(app.derived("pagerPosition"), &position);
    app.press("date-popup-backdrop");
    assert_eq!(app.state("popupClosing"), &Value::Bool(true));
    assert!(app.frame("date-popup").y + app.translation_y("date-popup") >= HEIGHT);
    app.finish_motion();
    assert_eq!(app.state("popupOpen"), &Value::Bool(false));
    assert_eq!(app.derived("day"), &selected);
    assert_eq!(app.derived("month"), &month);
    assert_eq!(app.derived("pagerPosition"), &position);
    assert!(app
        .runner
        .kernel()
        .find_by_test_id("date-popup-backdrop")
        .is_empty());

    app.press("date-2026-09-2026-09-25");
    app.create("event");
    assert!(app
        .runner
        .kernel()
        .find_by_test_id("date-popup-backdrop")
        .is_empty());
    app.press("cancel-editor");
    app.finish_motion();
    assert_eq!(
        app.runner
            .kernel()
            .find_by_test_id("date-popup-backdrop")
            .len(),
        1
    );
}

#[test]
fn the_date_sheet_ignores_horizontal_handle_motion_and_snaps_vertical_pull() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.press("date-2026-09-2026-09-25");
    let revision = app.derived("revision").clone();
    let popup = app.key("date-popup");

    for (dx, dy, vx, vy) in [(80.0, 2.0, 1000.0, 0.0), (0.0, 20.0, 0.0, 0.0)] {
        app.event("date-sheet-handle", Event::Pan(dx, dy));
        assert!(app.translation_y("date-popup") >= 0.0);
        app.event("date-sheet-handle", Event::PanRelease(vx, vy));
        assert_eq!(app.state("popupOpen"), &Value::Bool(true));
        assert_eq!(app.state("popupSnap"), &Value::Number(1.0));
        assert_eq!(app.key("date-popup"), popup);
        assert_eq!(app.translation_y("date-popup"), 0.0);
        assert_eq!(app.derived("revision"), &revision);
    }
    app.event("date-sheet-handle", Event::Pan(0.0, -80.0));
    app.event("date-sheet-handle", Event::PanRelease(0.0, -1000.0));
    assert_eq!(app.state("popupSnap"), &Value::Number(2.0));
    assert_eq!(app.key("date-popup"), popup);
    assert_eq!(app.derived("revision"), &revision);
}

#[test]
fn a_second_swipe_catches_the_presented_page_and_keeps_seven_months_mounted() {
    let root = Root::new();
    let mut app = App::open(&root);
    let september = 2026.0 * 12.0 + 8.0;
    assert_eq!(app.frame("calendar-stage").height, 710.0);
    assert_eq!(app.frame("calendar-stage").y, 164.0);
    assert_eq!(
        app.frame("calendar-stage").y + app.frame("calendar-stage").height,
        HEIGHT,
        "the calendar fills the viewport through the bottom edge"
    );
    assert_eq!(items(app.runner.resource("pages").unwrap()).len(), 7);
    let september_node = app.runner.kernel().find_by_test_id("month-2026-09")[0];

    app.event("date-2026-09-2026-09-15", Event::Pan(1.0, 30.0));
    assert_eq!(app.derived("pagerPosition"), &Value::Number(september));
    app.event("date-2026-09-2026-09-15", Event::Pan(-140.0, 3.0));
    let released = app.derived("pagerPosition").as_number().unwrap();
    assert!((released - september - 140.0 / f64::from(WIDTH)).abs() < 0.00001);
    app.event("date-2026-09-2026-09-15", Event::PanRelease(-900.0, 0.0));
    assert_eq!(app.derived("month"), &Value::Number(september + 1.0));
    assert_eq!(app.derived("pagerPosition"), &Value::Number(released));
    assert_eq!(
        app.runner.kernel().find_by_test_id("month-2026-09"),
        vec![september_node],
        "a month keeps its mounted identity when the seven-page window moves"
    );
    app.presented_frame(1000.0 / 120.0);
    let presented = app.derived("pagerPosition").as_number().unwrap();
    assert!(
        presented > released && presented < september + 1.0,
        "released={released} presented={presented}"
    );
    app.event("date-2026-10-2026-10-15", Event::Pan(20.0, 1.0));
    assert!(
        (app.derived("pagerPosition").as_number().unwrap() - (presented - 20.0 / f64::from(WIDTH)))
            .abs()
            < 0.00001,
        "interrupting a spring starts from its visible position"
    );
    app.event("date-2026-10-2026-10-15", Event::PanRelease(0.0, 0.0));
    for _ in 0..120 {
        app.presented_frame(1000.0 / 120.0);
    }
    assert_eq!(app.derived("pagerPosition"), app.derived("month"));
    assert_eq!(items(app.runner.resource("pages").unwrap()).len(), 7);
}

#[test]
fn editor_create_edit_delete_runs_through_contract_and_survives_restart() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.create("event");
    app.input("schedule-title", "Planning");
    app.input("schedule-notes", "Bring café notes.\nSecond line.");
    app.input("start-date", "2026-09-25");
    app.input("end-date", "2026-09-30");
    app.press("save-schedule");
    app.finish_motion();
    assert_eq!(app.state("editorOpen"), &Value::Bool(false));
    assert_eq!(app.state("popupOpen"), &Value::Bool(false));
    app.assert_creation_closed();
    app.press("date-2026-09-2026-09-25");
    let id = app.agenda_id("Planning");
    drop(app);

    let mut app = App::open(&root);
    app.press("date-2026-09-2026-09-25");
    app.press(&format!("agenda-item-{id}"));
    assert_eq!(
        app.derived("notes").as_str(),
        Some("Bring café notes.\nSecond line.")
    );
    assert_eq!(app.derived("startDate").as_str(), Some("2026-09-25"));
    assert_eq!(app.derived("endDate").as_str(), Some("2026-09-30"));
    app.input("schedule-title", "Updated planning");
    app.press("save-schedule");
    app.finish_motion();
    assert_eq!(app.agenda_id("Updated planning"), id);
    app.press(&format!("agenda-item-{id}"));
    app.press("delete-schedule");
    app.press("confirm-delete");
    app.finish_motion();
    assert_eq!(app.state("editorOpen"), &Value::Bool(false));
    drop(app);

    let mut app = App::open(&root);
    app.press("date-2026-09-2026-09-25");
    assert!(items(&fields(app.runner.resource("agenda").unwrap())[3])
        .iter()
        .all(|item| fields(item)[0].as_str() != Some(&id)));
}

#[test]
fn invalid_editor_save_keeps_the_draft_open_and_commits_no_schedule() {
    let root = Root::new();
    let mut app = App::open(&root);
    let revision = app.derived("revision").clone();
    app.create("event");
    app.input("schedule-title", "Draft");
    app.input("schedule-notes", "Unsaved draft");
    app.input("start-date", "2026-09-30");
    app.input("end-date", "2026-09-25");
    app.press("save-schedule");
    app.finish_motion();
    assert_eq!(app.state("editorOpen"), &Value::Bool(true));
    assert_eq!(app.derived("notes").as_str(), Some("Unsaved draft"));
    assert!(!app.state("error").as_str().unwrap().is_empty());
    assert_eq!(app.derived("revision"), &revision);
    app.input("end-date", "2026-10-01");
    app.press("save-schedule");
    app.finish_motion();
    assert_eq!(app.state("editorOpen"), &Value::Bool(false));
    assert_eq!(app.derived("revision"), &Value::Number(2.0));
    app.press("date-2026-09-2026-09-30");
    assert!(!app.agenda_id("Draft").is_empty());
}

#[test]
fn create_and_delete_recover_lost_replies_and_retry_uncommitted_failures_once() {
    for committed in [false, true] {
        let root = Root::new();
        let mut app = App::open(&root);
        app.create("event");
        app.input("schedule-title", "Reply lost");
        app.input("schedule-notes", "Keep this draft until success.");
        app.input("start-date", "2026-09-20");
        app.input("end-date", "2026-09-25");
        // The first native request reads the journal. The second either never
        // executes, or commits before its reply is lost. Recovery must distinguish
        // these cases without writing a committed mutation a second time.
        if committed {
            app.lose_write_reply_after = Some(2);
        } else {
            app.fail_write_reply_after = Some(2);
        }
        app.press("save-schedule");
        app.finish_motion();
        assert!(
            app.lose_write_reply_after.is_none() && app.fail_write_reply_after.is_none(),
            "the write must be intercepted"
        );
        if !committed {
            assert_eq!(app.state("editorOpen"), &Value::Bool(true));
            assert!(!app.state("error").as_str().unwrap().is_empty());
            assert_eq!(
                app.derived("notes").as_str(),
                Some("Keep this draft until success.")
            );

            app.press("save-schedule");
            app.finish_motion();
        }
        assert_eq!(app.state("editorOpen"), &Value::Bool(false));
        assert!(app.state("error").as_str().unwrap().is_empty());
        assert_eq!(app.derived("revision"), &Value::Number(2.0));
        assert_eq!(app.state("popupOpen"), &Value::Bool(false));
        app.press("date-2026-09-2026-09-20");
        let id = app.agenda_id("Reply lost");
        assert_eq!(
            items(&fields(app.runner.resource("agenda").unwrap())[3])
                .iter()
                .filter(|event| fields(event)[1].as_str() == Some("Reply lost"))
                .count(),
            1
        );
        drop(app);

        let mut app = App::open(&root);
        app.press("date-2026-09-2026-09-20");
        app.press(&format!("agenda-item-{id}"));
        assert_eq!(
            app.derived("notes").as_str(),
            Some("Keep this draft until success.")
        );
        app.press("delete-schedule");
        if committed {
            app.lose_write_reply_after = Some(2);
        } else {
            app.fail_write_reply_after = Some(2);
        }
        app.press("confirm-delete");
        app.finish_motion();
        assert!(app.lose_write_reply_after.is_none() && app.fail_write_reply_after.is_none());
        if !committed {
            assert_eq!(app.state("editorOpen"), &Value::Bool(true));
            assert!(!app.state("error").as_str().unwrap().is_empty());
            app.press("confirm-delete");
            app.finish_motion();
        }
        assert_eq!(app.state("editorOpen"), &Value::Bool(false));
        assert!(app.state("error").as_str().unwrap().is_empty());
        assert_eq!(app.derived("revision"), &Value::Number(3.0));
        drop(app);

        let mut app = App::open(&root);
        app.press("date-2026-09-2026-09-20");
        assert!(items(&fields(app.runner.resource("agenda").unwrap())[3])
            .iter()
            .all(|event| fields(event)[0].as_str() != Some(&id)));
    }
}

#[test]
fn drag_messages_ack_after_completion_and_drop_uses_final_coordinates_once() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.press("date-2026-09-2026-09-16");
    let id = app.agenda_id("San Francisco");
    app.press(&format!("agenda-item-{id}"));
    let metadata = ["title", "notes", "color", "allDay", "startTime", "endTime"]
        .map(|name| (name, app.derived(name).clone()));
    app.press("cancel-editor");
    app.finish_motion();
    let contact = Contact::new(&app, &id, 1);
    let popup = app.key("date-popup");
    let input = app.key("calendar-input");
    assert_eq!(
        app.runner
            .kernel()
            .find_by_test_id("date-popup-backdrop")
            .len(),
        1
    );
    let source = app
        .runner
        .kernel()
        .find_by_test_id(&format!("agenda-item-{id}"))[0];
    app.dispatch(
        "calendar-input",
        contact.event(1, "begin", contact.origin(), ""),
    );
    assert_eq!(
        app.state("contactAck"),
        &Value::Number(0.0),
        "the module must wait for then"
    );
    app.settle();
    assert_eq!(app.state("contactAck"), &Value::Number(1.0));
    assert_eq!(app.state("dragPhase").as_str(), Some("held"));
    assert_eq!(app.state("dragSpan"), &Value::Number(11.0));
    assert_eq!(app.key("date-popup"), popup);
    assert_eq!(app.key("calendar-input"), input);
    assert!(app
        .runner
        .kernel()
        .find_by_test_id("date-popup-backdrop")
        .is_empty());
    assert!(
        app.frame("date-popup").y + app.translation_y("date-popup") >= HEIGHT,
        "the entire sheet moves below the viewport while its source stays mounted"
    );
    assert_eq!(
        app.runner
            .kernel()
            .find_by_test_id(&format!("agenda-item-{id}")),
        vec![source]
    );
    assert_eq!(
        app.runner
            .kernel()
            .find_by_test_id("calendar-drag-image")
            .len(),
        1
    );

    let previous = app.center("date-2026-09-2026-09-19");
    app.event("calendar-input", contact.event(2, "move", previous, ""));
    assert_eq!(
        app.derived("hoverDay"),
        &Value::Number(f64::from(TODAY + 18))
    );
    let final_point = app.center("date-2026-09-2026-09-20");
    app.event("calendar-input", contact.event(3, "end", final_point, ""));
    assert_eq!(app.state("contactAck"), &Value::Number(3.0));
    assert_eq!(app.state("dragPhase").as_str(), Some("landing"));
    assert!(app.frame("date-popup").y + app.translation_y("date-popup") >= HEIGHT);
    assert!(app
        .runner
        .kernel()
        .find_by_test_id("date-popup-backdrop")
        .is_empty());
    app.event("calendar-input", contact.event(3, "end", final_point, ""));
    app.event("calendar-input", contact.event(4, "end", final_point, ""));
    app.finish_motion();
    assert_eq!(
        app.derived("revision"),
        &Value::Number(2.0),
        "terminal packets cannot replay storage"
    );
    assert_eq!(app.state("dragId").as_str(), Some(""));
    assert_eq!(app.state("popupOpen"), &Value::Bool(false));
    assert!(app.runner.kernel().find_by_test_id("date-popup").is_empty());
    assert_eq!(app.key("calendar-input"), input);
    assert!(app
        .runner
        .kernel()
        .find_by_test_id("date-popup-backdrop")
        .is_empty());
    assert!(app
        .runner
        .kernel()
        .find_by_test_id("calendar-notice")
        .is_empty());
    assert!(app
        .runner
        .kernel()
        .find_by_test_id("calendar-drag-image")
        .is_empty());
    drop(app);

    let mut app = App::open(&root);
    app.press("date-2026-09-2026-09-20");
    app.press(&format!("agenda-item-{id}"));
    assert_eq!(app.derived("startDate").as_str(), Some("2026-09-20"));
    assert_eq!(app.derived("endDate").as_str(), Some("2026-10-01"));
    for (name, expected) in metadata {
        assert_eq!(app.derived(name), &expected, "{name} must survive the move");
    }
}

#[test]
fn cancel_zone_outside_and_same_date_drops_restore_the_original_sheet_without_writes() {
    for ending in ["cancel", "cancel-zone", "outside", "same-date"] {
        let root = Root::new();
        let mut app = App::open(&root);
        app.press("date-2026-09-2026-09-16");
        let id = app.agenda_id("San Francisco");
        let contact = Contact::new(&app, &id, 1);
        let popup = app.key("date-popup");
        let source = app.key(&format!("agenda-item-{id}"));
        let original_day = app.derived("day").clone();
        app.event(
            "calendar-input",
            contact.event(1, "begin", contact.origin(), ""),
        );
        let point = match ending {
            "cancel-zone" => app.center("cancel-zone"),
            "outside" => (-10.0, 180.0),
            "same-date" => app.center("date-2026-09-2026-09-08"),
            _ => app.center("date-2026-09-2026-09-16"),
        };
        let (phase, reason) = if ending == "cancel" {
            ("cancel", "interrupted")
        } else {
            ("end", "")
        };
        app.event("calendar-input", contact.event(2, phase, point, reason));
        assert_eq!(app.state("contactAck"), &Value::Number(2.0));
        assert_eq!(
            app.state("dragPhase").as_str(),
            Some("returning"),
            "{ending}"
        );
        assert_eq!(app.derived("revision"), &Value::Number(1.0), "{ending}");
        assert_eq!(app.state("returnPrepared"), &Value::Bool(false));
        assert_eq!(app.derived("returnOpacity"), &Value::Number(0.0));
        assert_eq!(app.state("ghostX"), app.state("sourceX"));
        assert_eq!(app.state("ghostY"), app.state("sourceY"));
        assert_eq!(app.derived("ghostWidth"), app.state("sourceWidth"));
        assert_eq!(app.derived("ghostHeight"), app.state("sourceHeight"));
        assert_eq!(app.key(&format!("agenda-item-{id}")), source);
        assert_eq!(app.translation_y("date-popup"), 0.0);
        app.presented_frame(17.0);
        assert_eq!(app.state("returnPrepared"), &Value::Bool(true));
        assert_eq!(app.derived("returnOpacity"), &Value::Number(1.0));
        app.presented_frame(200.0);
        assert_eq!(app.state("dragPhase").as_str(), Some("returning"));
        assert_eq!(app.key("date-popup"), popup);
        app.finish_motion();
        assert_eq!(app.state("dragId").as_str(), Some(""));
        assert_eq!(app.key("date-popup"), popup);
        assert_eq!(app.key(&format!("agenda-item-{id}")), source);
        assert_eq!(app.derived("day"), &original_day);
        assert_eq!(app.translation_y("date-popup"), 0.0);
        assert_eq!(
            app.runner
                .kernel()
                .find_by_test_id("date-popup-backdrop")
                .len(),
            1
        );
        assert_eq!(app.agenda_id("San Francisco"), id);
        drop(app);

        let mut reopened = App::open(&root);
        reopened.press("date-2026-09-2026-09-16");
        assert_eq!(reopened.derived("revision"), &Value::Number(1.0));
        assert_eq!(reopened.agenda_id("San Francisco"), id);
    }
}

#[test]
fn a_drag_from_each_date_sheet_height_restores_that_height_on_cancel() {
    for (snap, pan) in [(0.0, 210.0), (1.0, 0.0), (2.0, -300.0)] {
        let root = Root::new();
        let mut app = App::open(&root);
        app.press("date-2026-09-2026-09-16");
        if pan != 0.0 {
            app.event("date-sheet-handle", Event::Pan(0.0, pan));
            app.event("date-sheet-handle", Event::PanRelease(0.0, 0.0));
        }
        assert_eq!(app.state("popupSnap"), &Value::Number(snap));
        let original_height = app.frame("date-popup").height;
        let popup = app.key("date-popup");
        let id = app.agenda_id("San Francisco");
        let contact = Contact::new(&app, &id, 1);
        app.event(
            "calendar-input",
            contact.event(1, "begin", contact.origin(), ""),
        );
        assert!(app.translation_y("date-popup") > HEIGHT);
        let cancel = app.center("cancel-zone");
        app.event("calendar-input", contact.event(2, "end", cancel, ""));
        assert_eq!(app.state("dragPhase").as_str(), Some("returning"));
        assert_eq!(app.key("date-popup"), popup);
        assert_eq!(app.translation_y("date-popup"), 0.0);
        assert_eq!(app.state("popupSnap"), &Value::Number(snap));
        assert!((app.frame("date-popup").height - original_height).abs() < 0.1);
        app.finish_motion();
        assert_eq!(app.state("dragId").as_str(), Some(""));
        assert_eq!(app.key("date-popup"), popup);
        assert_eq!(app.derived("revision"), &Value::Number(1.0));
        assert_eq!(app.agenda_id("San Francisco"), id);
    }
}

#[test]
fn the_sheet_stays_offscreen_until_a_pending_drop_finishes() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.press("date-2026-09-2026-09-16");
    let id = app.agenda_id("San Francisco");
    let contact = Contact::new(&app, &id, 1);
    let input = app.key("calendar-input");
    let popup = app.key("date-popup");
    let source = app.key(&format!("agenda-item-{id}"));
    app.event(
        "calendar-input",
        contact.event(1, "begin", contact.origin(), ""),
    );
    let destination = app.center("date-2026-09-2026-09-20");
    app.dispatch("calendar-input", contact.event(2, "end", destination, ""));
    app.runner.advance(app.runner.now_ms()).unwrap();
    app.layout();
    assert_eq!(app.state("contactAck"), &Value::Number(2.0));
    assert_eq!(app.derived("busy"), &Value::Bool(true));

    // Present frames while deliberately withholding the real storage request.
    // Neither the finishing animation nor a stale source can restore the sheet.
    for _ in 0..90 {
        assert!(app
            .runner
            .frame(app.runner.now_ms() + 1000.0 / 120.0)
            .error
            .is_none());
        app.layout();
        assert_eq!(app.key("calendar-input"), input);
        assert_eq!(app.key("date-popup"), popup);
        assert_eq!(app.key(&format!("agenda-item-{id}")), source);
        assert!(app.frame("date-popup").y + app.translation_y("date-popup") >= HEIGHT);
    }
    assert_eq!(app.derived("revision"), &Value::Number(1.0));
    assert_eq!(app.derived("busy"), &Value::Bool(true));

    app.settle();
    assert_eq!(app.derived("busy"), &Value::Bool(false));
    for _ in 0..90 {
        app.presented_frame(1000.0 / 120.0);
    }
    assert_eq!(app.derived("revision"), &Value::Number(2.0));
    assert_eq!(app.key("calendar-input"), input);
    assert_eq!(app.state("popupOpen"), &Value::Bool(false));
    assert!(app.runner.kernel().find_by_test_id("date-popup").is_empty());
    assert_eq!(app.state("dragId").as_str(), Some(""));
    app.press("date-2026-09-2026-09-20");
    assert_eq!(app.agenda_id("San Francisco"), id);
}

#[test]
fn stale_contact_identity_is_acknowledged_without_affecting_the_live_drag() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.press("date-2026-09-2026-09-16");
    let id = app.agenda_id("San Francisco");
    let contact = Contact::new(&app, &id, 2);
    app.event(
        "calendar-input",
        contact.event(1, "begin", contact.origin(), ""),
    );
    let stale = Contact::new(&app, &id, 1);
    let target = app.center("date-2026-09-2026-09-20");
    app.event("calendar-input", stale.event(2, "end", target, ""));
    assert_eq!(app.state("contactAck"), &Value::Number(2.0));
    assert_eq!(app.state("dragPhase").as_str(), Some("held"));
    assert_eq!(app.state("dragSerial"), &Value::Number(2.0));
    assert_eq!(app.derived("revision"), &Value::Number(1.0));
    app.event(
        "calendar-input",
        contact.event(3, "cancel", target, "cancelled"),
    );
    assert_eq!(app.state("dragPhase").as_str(), Some("returning"));
}

#[test]
fn a_stationary_long_press_cannot_drop_onto_a_date_behind_the_popup() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.press("date-2026-09-2026-09-16");
    let id = app.agenda_id("San Francisco");
    let contact = Contact::new(&app, &id, 1);
    app.event(
        "calendar-input",
        contact.event(1, "begin", contact.origin(), ""),
    );
    app.event(
        "calendar-input",
        contact.event(2, "end", contact.origin(), ""),
    );
    assert_eq!(app.state("dragPhase").as_str(), Some("returning"));
    assert_eq!(app.derived("revision"), &Value::Number(1.0));
}

#[test]
fn releasing_over_a_partly_presented_month_uses_the_cell_under_the_finger() {
    let root = Root::new();
    let mut app = App::open(&root);
    let september = 2026.0 * 12.0 + 8.0;
    app.press("date-2026-09-2026-09-16");
    let id = app.agenda_id("San Francisco");
    let contact = Contact::new(&app, &id, 1);
    app.event(
        "calendar-input",
        contact.event(1, "begin", contact.origin(), ""),
    );
    let edge_point = (
        f64::from(WIDTH) - 8.0,
        app.state("portY").as_number().unwrap() + 200.0,
    );
    app.event("calendar-input", contact.event(2, "move", edge_point, ""));
    for _ in 0..120 {
        app.presented_frame(1000.0 / 120.0);
        if app.derived("pagerPosition").as_number().unwrap() > september + 0.45 {
            break;
        }
    }
    let presented = app.derived("pagerPosition").as_number().unwrap();
    assert!(
        presented > september + 0.45 && presented < september + 0.7,
        "presented={presented}, target={:?}",
        app.derived("month")
    );
    assert_eq!(app.derived("month"), &Value::Number(september + 1.0));
    let final_point = app.center("date-2026-10-2026-10-04");
    assert!(final_point.0 > 0.0 && final_point.0 < f64::from(WIDTH));
    app.event("calendar-input", contact.event(3, "end", final_point, ""));
    assert_eq!(app.state("dragPhase").as_str(), Some("landing"));
    assert_eq!(app.derived("contactMonth"), &Value::Number(september + 1.0));
    app.runner.advance(app.runner.now_ms() + 100.0).unwrap();
    app.settle();
    assert_eq!(app.state("dragPhase").as_str(), Some("landing"));
    assert_eq!(
        app.derived("pagerPosition"),
        &Value::Number(presented),
        "the destination page stays still while the drop image lands"
    );
    app.finish_motion();
    assert_eq!(app.derived("revision"), &Value::Number(2.0));
    assert_eq!(app.state("popupOpen"), &Value::Bool(false));
    drop(app);

    let mut app = App::open(&root);
    app.event("month-2026-09", Event::Pan(-220.0, 0.0));
    app.event("month-2026-09", Event::PanRelease(0.0, 0.0));
    for _ in 0..60 {
        app.runner.advance(app.runner.now_ms() + 16.0).unwrap();
        app.settle();
    }
    app.press("date-2026-10-2026-10-04");
    app.press(&format!("agenda-item-{id}"));
    assert_eq!(app.derived("startDate").as_str(), Some("2026-10-04"));
    assert_eq!(app.derived("endDate").as_str(), Some("2026-10-15"));
}

#[test]
fn horizontal_edges_take_priority_then_vertical_scroll_rehits_and_persists_the_drop() {
    let root = Root::new();
    let mut app = App::open(&root);
    // Natural-height dense weeks create actual overflow; do not manufacture
    // impossible native offsets in a month whose content already fits.
    for n in 0..20 {
        app.create("event");
        app.input("schedule-title", &format!("Dense schedule {n}"));
        app.input("start-date", "2026-09-15");
        app.input("end-date", "2026-09-15");
        app.press("save-schedule");
        app.finish_motion();
    }
    app.press("date-2026-09-2026-09-15");
    let popup = app.frame("date-popup");
    assert!(
        (popup.height - app.derived("sheetHalfHeight").as_number().unwrap() as f32).abs() < 0.1,
        "long agenda opens at the half-height snap"
    );
    assert_eq!(popup.y + popup.height, HEIGHT);
    assert!(
        app.content_height("agenda-list") > app.frame("agenda-list").height + 600.0,
        "the long agenda scrolls inside the bounded sheet"
    );
    let id = app.agenda_id("Dense schedule 0");
    let contact = Contact::new(&app, &id, 1);
    app.event(
        "calendar-input",
        contact.event(1, "begin", contact.origin(), ""),
    );
    let left = app.state("portX").as_number().unwrap();
    let top = app.state("portY").as_number().unwrap();
    let width = app.state("portWidth").as_number().unwrap();
    let height = app.state("portHeight").as_number().unwrap();
    let scroll_edge = f64::from(app.frame("cancel-zone").y) - 8.0;
    let corner = (left + width - 8.0, scroll_edge);
    app.event("calendar-input", contact.event(2, "move", corner, ""));
    assert_eq!(app.derived("edgeWanted"), &Value::Number(1.0));
    let initial_request = app.state("calendarScrollRequest").clone();
    for _ in 0..6 {
        app.runner.advance(app.runner.now_ms() + 34.0).unwrap();
        app.settle();
        assert_eq!(
            app.state("calendarScrollRequest"),
            &initial_request,
            "a corner prepares horizontal paging without vertical scrolling"
        );
    }
    let point = (left + width / 2.0, scroll_edge);
    app.event("calendar-input", contact.event(3, "move", point, ""));
    let initial_day = app.derived("hoverDay").clone();
    assert_ne!(initial_day, Value::Number(-1_000_000.0));
    let max_scroll = fields(app.runner.resource("hoverTarget").unwrap())[5]
        .as_number()
        .unwrap();
    assert!(max_scroll > 300.0);
    let last_week = app.frame("date-2026-09-2026-10-03");
    let drag_padding = app.derived("dragPadding").as_number().unwrap();
    assert!(
        (max_scroll - (f64::from(last_week.y + last_week.height) - top - height + drag_padding))
            .abs()
            < 0.001,
        "held scroll room must end at the final week plus the cancel overlap"
    );
    let revision = app.derived("revision").clone();
    for _ in 0..120 {
        app.presented_frame(1000.0 / 120.0);
        let requested = app.state("calendarScrollRequest").as_number().unwrap();
        assert!(requested > app.state("calendarScrollTop").as_number().unwrap());
        // The native scroll executor reports the authored offset back. No new
        // pointer message is sent while the calendar moves beneath the finger.
        app.event(
            "month-scroll-2026-09",
            Event::Scroll(ScrollEvent {
                top: requested,
                ..Default::default()
            }),
        );
        if app.derived("hoverDay") != &initial_day {
            break;
        }
    }
    assert_ne!(app.derived("hoverDay"), &initial_day);
    assert_ne!(app.derived("hoverDay"), &Value::Number(-1_000_000.0));
    assert_eq!(app.state("dragX"), &Value::Number(point.0));
    assert_eq!(app.state("dragY"), &Value::Number(point.1));
    assert_eq!(app.derived("revision"), &revision);
    let expected_day = app.derived("hoverDay").clone();
    app.event("calendar-input", contact.event(4, "end", point, ""));
    assert_eq!(app.state("dragPhase").as_str(), Some("landing"));
    app.finish_motion();
    assert_eq!(app.state("popupOpen"), &Value::Bool(false));
    assert_eq!(
        app.derived("revision").as_number().unwrap(),
        revision.as_number().unwrap() + 1.0
    );
    let saved_date = app.date_for_day(expected_day.as_number().unwrap());
    app.press(&format!("date-2026-09-{saved_date}"));
    let moved = items(&fields(app.runner.resource("agenda").unwrap())[3])
        .iter()
        .find(|event| fields(event)[0].as_str() == Some(&id))
        .unwrap();
    assert_eq!(fields(moved)[5], expected_day);
    assert_eq!(fields(moved)[7].as_str(), Some(saved_date.as_str()));
    drop(app);

    let mut app = App::open(&root);
    app.press(&format!("date-2026-09-{saved_date}"));
    app.press(&format!("agenda-item-{id}"));
    assert_eq!(app.derived("startDate").as_str(), Some(saved_date.as_str()));
    assert_eq!(app.derived("endDate").as_str(), Some(saved_date.as_str()));
}
