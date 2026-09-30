//! The real baked Contract, runner actions and native SQLite across restarts.

use calendar_data::Calendar;
use exact_data_host::Storage;
use exact_kernel::{Env, Frame, Kernel, Offer};
use exact_plan::{Plan, Value};
use exact_runner::{
    DataSource, Dispatch, Event, Outcome, Reply, RequestOut, Runner, Viewport, Work,
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
                let mut outcome = run(item);
                if mutation_request {
                    if let Some(remaining) = &mut self.lose_write_reply_after {
                        *remaining -= 1;
                    }
                }
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
        let keys = self.runner.kernel().find_by_test_id(test_id);
        assert_eq!(keys.len(), 1, "expected one visible {test_id}");
        let view = self.runner.kernel().node_by_key(keys[0]).unwrap().id;
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
    fn frame(&self, test_id: &str) -> Frame {
        let keys = self.runner.kernel().find_by_test_id(test_id);
        assert_eq!(keys.len(), 1, "expected one laid-out {test_id}");
        self.runner.kernel().laid_out_frame(keys[0]).unwrap().0
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
        let mut source = app.frame(&format!("agenda-item-{id}"));
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
    assert_eq!(last_week.y + last_week.height, HEIGHT);
    app.press("date-2026-09-2026-09-25");
    assert_eq!(app.state("popupOpen"), &Value::Bool(true));
    assert_eq!(app.agenda_id("Summer in Seoul"), "sample-2026-09-6");
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
    app.press("add-schedule");
    app.input("schedule-title", "Planning");
    app.input("schedule-notes", "Bring café notes.\nSecond line.");
    app.input("start-date", "2026-09-25");
    app.input("end-date", "2026-09-30");
    app.press("save-schedule");
    assert_eq!(app.state("editorOpen"), &Value::Bool(false));
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
    assert_eq!(app.agenda_id("Updated planning"), id);
    app.press(&format!("agenda-item-{id}"));
    app.press("delete-schedule");
    app.press("confirm-delete");
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
    app.press("add-schedule");
    app.input("schedule-title", "Draft");
    app.input("schedule-notes", "Unsaved draft");
    app.input("start-date", "2026-09-30");
    app.input("end-date", "2026-09-25");
    app.press("save-schedule");
    assert_eq!(app.state("editorOpen"), &Value::Bool(true));
    assert_eq!(app.derived("notes").as_str(), Some("Unsaved draft"));
    assert!(!app.state("error").as_str().unwrap().is_empty());
    assert_eq!(app.derived("revision"), &revision);
    app.input("end-date", "2026-10-01");
    app.press("save-schedule");
    assert_eq!(app.state("editorOpen"), &Value::Bool(false));
    assert_eq!(app.derived("revision"), &Value::Number(2.0));
    assert!(!app.agenda_id("Draft").is_empty());
}

#[test]
fn retrying_a_committed_create_and_delete_after_a_lost_reply_is_exactly_once() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.press("add-schedule");
    app.input("schedule-title", "Reply lost");
    app.input("schedule-notes", "Keep this draft until success.");
    app.input("start-date", "2026-09-20");
    app.input("end-date", "2026-09-25");
    // Native storage consumes the opaque first request for the journal lookup;
    // its second request commits the write before we replace only the reply.
    app.lose_write_reply_after = Some(2);
    app.press("save-schedule");
    assert!(
        app.lose_write_reply_after.is_none(),
        "the committed write must be intercepted"
    );
    assert_eq!(app.state("editorOpen"), &Value::Bool(true));
    assert!(!app.state("error").as_str().unwrap().is_empty());
    assert_eq!(
        app.derived("notes").as_str(),
        Some("Keep this draft until success.")
    );

    app.press("save-schedule");
    assert_eq!(app.state("editorOpen"), &Value::Bool(false));
    assert_eq!(app.derived("revision"), &Value::Number(2.0));
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
    app.lose_write_reply_after = Some(2);
    app.press("confirm-delete");
    assert!(app.lose_write_reply_after.is_none());
    assert_eq!(app.state("editorOpen"), &Value::Bool(true));
    assert!(!app.state("error").as_str().unwrap().is_empty());
    app.press("confirm-delete");
    assert_eq!(app.state("editorOpen"), &Value::Bool(false));
    assert_eq!(app.derived("revision"), &Value::Number(3.0));
    drop(app);

    let mut app = App::open(&root);
    app.press("date-2026-09-2026-09-20");
    assert!(items(&fields(app.runner.resource("agenda").unwrap())[3])
        .iter()
        .all(|event| fields(event)[0].as_str() != Some(&id)));
}

#[test]
fn drag_messages_ack_after_completion_and_drop_uses_final_coordinates_once() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.press("date-2026-09-2026-09-25");
    let id = app.agenda_id("Summer in Seoul");
    app.press(&format!("agenda-item-{id}"));
    let metadata = ["title", "notes", "color", "allDay", "startTime", "endTime"]
        .map(|name| (name, app.derived(name).clone()));
    app.press("cancel-editor");
    let contact = Contact::new(&app, &id, 1);
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
    assert_eq!(app.state("dragSpan"), &Value::Number(5.0));
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
    assert_eq!(app.derived("revision"), &Value::Number(2.0));
    let moved = items(&fields(app.runner.resource("agenda").unwrap())[3])
        .iter()
        .find(|event| fields(event)[0].as_str() == Some(&id))
        .unwrap();
    assert_eq!(fields(moved)[7].as_str(), Some("2026-09-20"));
    assert_eq!(fields(moved)[8].as_str(), Some("2026-09-25"));

    app.event("calendar-input", contact.event(3, "end", final_point, ""));
    app.event("calendar-input", contact.event(4, "end", final_point, ""));
    assert_eq!(
        app.derived("revision"),
        &Value::Number(2.0),
        "a terminal packet cannot replay storage"
    );
    app.runner.advance(400.0).unwrap();
    app.settle();
    assert_eq!(app.state("dragId").as_str(), Some(""));
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
    assert_eq!(app.derived("endDate").as_str(), Some("2026-09-25"));
    for (name, expected) in metadata {
        assert_eq!(app.derived(name), &expected, "{name} must survive the move");
    }
}

#[test]
fn cancel_outside_and_same_date_drops_do_not_change_storage() {
    for ending in ["cancel", "outside", "same-date"] {
        let root = Root::new();
        let mut app = App::open(&root);
        app.press("date-2026-09-2026-09-25");
        let id = app.agenda_id("Summer in Seoul");
        let contact = Contact::new(&app, &id, 1);
        app.event(
            "calendar-input",
            contact.event(1, "begin", contact.origin(), ""),
        );
        let point = match ending {
            "outside" => (-10.0, 180.0),
            _ => app.center("date-2026-09-2026-09-25"),
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
        app.runner.advance(400.0).unwrap();
        app.settle();
        assert_eq!(app.state("dragId").as_str(), Some(""));
        assert_eq!(app.agenda_id("Summer in Seoul"), id);
    }
}

#[test]
fn stale_contact_identity_is_acknowledged_without_affecting_the_live_drag() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.press("date-2026-09-2026-09-25");
    let id = app.agenda_id("Summer in Seoul");
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
    app.press("date-2026-09-2026-09-25");
    let id = app.agenda_id("Summer in Seoul");
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
    app.press("date-2026-09-2026-09-25");
    let id = app.agenda_id("Summer in Seoul");
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
    assert_eq!(app.derived("revision"), &Value::Number(2.0));
    assert_eq!(app.derived("contactMonth"), &Value::Number(september + 1.0));
    let moved = items(&fields(app.runner.resource("agenda").unwrap())[3])
        .iter()
        .find(|event| fields(event)[0].as_str() == Some(&id))
        .unwrap();
    assert_eq!(fields(moved)[7].as_str(), Some("2026-10-04"));
    assert_eq!(fields(moved)[8].as_str(), Some("2026-10-09"));
    app.runner.advance(app.runner.now_ms() + 100.0).unwrap();
    app.settle();
    assert_eq!(app.state("dragPhase").as_str(), Some("landing"));
    assert_eq!(
        app.derived("pagerPosition"),
        &Value::Number(presented),
        "the destination page stays still while the drop image lands"
    );
    drop(app);

    let mut app = App::open(&root);
    app.press("next-month");
    for _ in 0..60 {
        app.runner.advance(app.runner.now_ms() + 16.0).unwrap();
        app.settle();
    }
    app.press("date-2026-10-2026-10-04");
    app.press(&format!("agenda-item-{id}"));
    assert_eq!(app.derived("startDate").as_str(), Some("2026-10-04"));
    assert_eq!(app.derived("endDate").as_str(), Some("2026-10-09"));
}

#[test]
fn horizontal_edges_take_priority_then_vertical_scroll_rehits_and_persists_the_drop() {
    let root = Root::new();
    let mut app = App::open(&root);
    // Natural-height dense weeks create actual overflow; do not manufacture
    // impossible native offsets in a month whose content already fits.
    for n in 0..20 {
        app.press("add-schedule");
        app.input("schedule-title", &format!("Dense schedule {n}"));
        app.input("start-date", "2026-09-15");
        app.input("end-date", "2026-09-15");
        app.press("save-schedule");
    }
    let id = fields(&items(&fields(app.runner.resource("agenda").unwrap())[3])[0])[0]
        .as_str()
        .unwrap()
        .to_owned();
    let contact = Contact::new(&app, &id, 1);
    app.event(
        "calendar-input",
        contact.event(1, "begin", contact.origin(), ""),
    );
    let left = app.state("portX").as_number().unwrap();
    let top = app.state("portY").as_number().unwrap();
    let width = app.state("portWidth").as_number().unwrap();
    let height = app.state("portHeight").as_number().unwrap();
    let corner = (left + width - 8.0, top + height - 8.0);
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
    let point = (left + width / 2.0, top + height - 8.0);
    app.event("calendar-input", contact.event(3, "move", point, ""));
    let initial_day = app.derived("hoverDay").clone();
    assert_ne!(initial_day, Value::Number(-1_000_000.0));
    let max_scroll = fields(app.runner.resource("hoverTarget").unwrap())[5]
        .as_number()
        .unwrap();
    assert!(max_scroll > 300.0);
    let last_week = app.frame("date-2026-09-2026-10-03");
    assert!(
        (max_scroll - (f64::from(last_week.y + last_week.height) - top - height)).abs() < 0.001,
        "drag scrolling must stop at the exact rendered final week"
    );
    let revision = app.derived("revision").clone();
    for _ in 0..20 {
        app.runner.advance(app.runner.now_ms() + 34.0).unwrap();
        app.settle();
        let requested = app.state("calendarScrollRequest").as_number().unwrap();
        assert!(requested > app.state("calendarScrollTop").as_number().unwrap());
        // The native scroll executor reports the authored offset back. No new
        // pointer message is sent while the calendar moves beneath the finger.
        app.event("month-scroll-2026-09", Event::Scroll(0.0, requested));
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
    assert_eq!(
        app.derived("revision").as_number().unwrap(),
        revision.as_number().unwrap() + 1.0
    );
    let moved = items(&fields(app.runner.resource("agenda").unwrap())[3])
        .iter()
        .find(|event| fields(event)[0].as_str() == Some(&id))
        .unwrap();
    assert_eq!(fields(moved)[5], expected_day);
    let saved_date = fields(moved)[7].as_str().unwrap().to_owned();
    drop(app);

    let mut app = App::open(&root);
    app.press(&format!("date-2026-09-{saved_date}"));
    app.press(&format!("agenda-item-{id}"));
    assert_eq!(app.derived("startDate").as_str(), Some(saved_date.as_str()));
    assert_eq!(app.derived("endDate").as_str(), Some(saved_date.as_str()));
}
