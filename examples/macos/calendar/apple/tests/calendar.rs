//! The real baked Contract in a Mac-sized window: runner actions, the input
//! module's packets and native SQLite across restarts.

use exact_data_host::Storage;
use exact_kernel::{Env, Frame, Kernel, NodeKey, Offer, PropId};
use exact_plan::{Plan, Value};
use exact_runner::{
    DataSource, Dispatch, Event, Outcome, Reply, RequestOut, Runner, Viewport, Work,
};
use macos_calendar_data::Calendar;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

const TODAY: i32 = 20_697; // 2026-09-01, a Tuesday.
const WIDTH: f32 = 1180.0;
const HEIGHT: f32 = 800.0;
const INSPECTOR: f32 = 300.0;
const SEPTEMBER: f64 = 2026.0 * 12.0 + 8.0;
type CalendarRunner = Runner<Storage<Calendar>>;

#[path = "calendar/dragging.rs"]
mod dragging;
#[path = "calendar/editing.rs"]
mod editing;

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        Self(std::env::temp_dir().join(format!(
            "macos-calendar-contract-{}-{}",
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
        // `viewport-fit=cover` on the Mac: the titlebar is the top inset.
        kernel.set_env(Env::new(28.0, 0.0, 0.0, 0.0)).unwrap();
        let mut runner = Runner::boot(
            Plan::decode(macos_calendar_apple::PLAN).unwrap(),
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
        app.finish_motion();
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
    fn key_press(&mut self, test_id: &str, name: &str) {
        self.event(test_id, Event::Key(name.into()));
    }
    fn finish_motion(&mut self) {
        for _ in 0..72 {
            self.presented_frame(1000.0 / 120.0);
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
    fn number(&self, name: &str) -> f64 {
        self.derived(name).as_number().unwrap()
    }
    fn agenda(&self) -> &[Value] {
        fields(self.runner.resource("agenda").unwrap())
    }
    fn agenda_ids(&self) -> Vec<String> {
        items(&self.agenda()[3])
            .iter()
            .map(|item| fields(item)[0].as_str().unwrap().to_owned())
            .collect()
    }
    fn agenda_id(&self, title: &str) -> String {
        items(&self.agenda()[3])
            .iter()
            .find(|item| fields(item)[1].as_str() == Some(title))
            .map(|item| fields(item)[0].as_str().unwrap().to_owned())
            .unwrap_or_else(|| panic!("agenda has no {title}"))
    }
    fn sticker(&self) -> &str {
        self.agenda()[4].as_str().unwrap()
    }
    fn mounted(&self, test_id: &str) -> bool {
        !self.runner.kernel().find_by_test_id(test_id).is_empty()
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
    fn text(&self, test_id: &str) -> &str {
        self.runner
            .kernel()
            .node_by_key(self.key(test_id))
            .unwrap()
            .props
            .str(PropId::Text)
            .unwrap()
    }
    /// A point in the input root's box: the root is the window's content.
    fn center(&self, test_id: &str) -> (f64, f64) {
        let frame = self.frame(test_id);
        let root = self.frame("calendar");
        (
            f64::from(frame.x - root.x + frame.width / 2.0),
            f64::from(frame.y - root.y + frame.height / 2.0),
        )
    }
}

/// What the input module posts for one contact: a stand-in for the AppKit
/// handle, in the root's coordinates.
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
    /// Begin, one move, and the end at `to`: the shape of every mouse drag.
    fn drag_to(&self, app: &mut App, sequence: u64, to: (f64, f64)) {
        app.event(
            "calendar-input",
            self.event(sequence, "begin", self.origin(), ""),
        );
        app.event("calendar-input", self.event(sequence + 1, "move", to, ""));
        app.event("calendar-input", self.event(sequence + 2, "end", to, ""));
    }
}

/// The handle on a schedule's first bar in one month page.
fn bar_handle(app: &App, month_id: &str, event_id: &str) -> String {
    let pages = items(app.runner.resource("pages").unwrap());
    let page = pages
        .iter()
        .find(|page| fields(page)[0].as_str() == Some(month_id))
        .unwrap_or_else(|| panic!("{month_id} is not paged in"));
    items(&fields(page)[3])
        .iter()
        .flat_map(|week| items(&fields(week)[2]))
        .find(|bar| fields(bar)[1].as_str() == Some(event_id))
        .map(|bar| format!("bar-handle-{month_id}-{}", fields(bar)[0].as_str().unwrap()))
        .unwrap_or_else(|| panic!("{month_id} has no bar for {event_id}"))
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
fn day(offset: i32) -> Value {
    Value::Number(f64::from(TODAY + offset))
}

#[test]
fn the_month_takes_the_window_beside_a_fixed_inspector_showing_today() {
    let root = Root::new();
    let app = App::open(&root);
    assert_eq!(app.derived("month"), &Value::Number(SEPTEMBER));
    assert_eq!(app.text("month-label"), "September");
    assert_eq!(app.text("year-label"), "2026");
    assert_eq!(app.derived("monthTitle").as_str(), Some("September 2026"));
    let inspector = app.frame("inspector");
    assert_eq!(inspector.width, INSPECTOR);
    assert_eq!(inspector.x + inspector.width, WIDTH);
    let stage = app.frame("calendar-stage");
    assert_eq!(stage.width, WIDTH - INSPECTOR);
    assert_eq!(app.number("pageWidth"), f64::from(WIDTH - INSPECTOR));
    assert_eq!(
        app.state("stageWidth"),
        &Value::Number(f64::from(WIDTH - INSPECTOR))
    );
    // The header sits under the titlebar inset.
    assert_eq!(app.frame("calendar-header").y, 28.0);
    assert_eq!(app.derived("day"), &day(0));
    assert_eq!(app.text("inspector-date"), "Tue, Sep 1");
    let cell = app.frame("date-2026-09-2026-09-01");
    assert!((cell.width - (WIDTH - INSPECTOR) / 7.0).abs() < 0.01);
    // Nothing in the window is a sheet: no handle, no backdrop, no Cancel zone.
    for gone in [
        "date-popup",
        "date-sheet-handle",
        "cancel-zone",
        "add-surface",
        "theme-backdrop",
        "editor-backdrop",
    ] {
        assert!(!app.mounted(gone), "{gone} belongs to the phone");
    }
}

#[test]
fn a_click_selects_a_date_and_arrows_move_the_selection_across_months() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.press("date-2026-09-2026-09-16");
    assert_eq!(app.derived("day"), &day(15));
    assert_eq!(app.agenda_id("San Francisco"), "sample-2026-09-6");
    app.press("date-2026-09-2026-09-16");
    assert_eq!(
        app.derived("day"),
        &day(15),
        "a second press keeps the selection"
    );
    app.key_press("date-2026-09-2026-09-16", "ArrowRight");
    assert_eq!(app.derived("day"), &day(16));
    app.key_press("date-2026-09-2026-09-17", "ArrowDown");
    assert_eq!(app.derived("day"), &day(23));
    app.key_press("date-2026-09-2026-09-24", "ArrowUp");
    app.key_press("date-2026-09-2026-09-17", "ArrowLeft");
    assert_eq!(app.derived("day"), &day(15));
    for ignored in ["Enter", " ", "a"] {
        app.key_press("date-2026-09-2026-09-16", ignored);
        assert_eq!(
            app.derived("day"),
            &day(15),
            "{ignored:?} leaves the selection"
        );
    }
    assert_eq!(app.state("editorOpen"), &Value::Bool(false));

    app.press("date-2026-09-2026-09-30");
    app.key_press("date-2026-09-2026-09-30", "ArrowRight");
    assert_eq!(app.derived("day"), &day(30));
    assert_eq!(app.derived("month"), &Value::Number(SEPTEMBER + 1.0));
    app.key_press("date-2026-10-2026-10-01", "ArrowLeft");
    assert_eq!(app.derived("day"), &day(29));
    assert_eq!(app.derived("month"), &Value::Number(SEPTEMBER));
    // A trailing October date on the September page moves within the page it is on.
    app.press("date-2026-09-2026-10-02");
    assert_eq!(app.derived("day"), &day(31));
    assert_eq!(app.derived("month"), &Value::Number(SEPTEMBER));
}

#[test]
fn month_buttons_switch_at_once_count_every_click_and_today_returns() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.press("month-next");
    assert_eq!(app.derived("month"), &Value::Number(SEPTEMBER + 1.0));
    assert_eq!(app.text("month-label"), "October");
    // Only the month on view is mounted: no page waits off to the side.
    assert!(app.mounted("month-2026-10"));
    assert!(!app.mounted("month-2026-09") && !app.mounted("month-2026-11"));
    app.press("date-2026-10-2026-10-14");
    app.press("month-previous");
    app.press("month-previous");
    assert_eq!(app.text("month-label"), "August");
    app.press("today");
    assert_eq!(app.derived("month"), &Value::Number(SEPTEMBER));
    assert_eq!(app.derived("day"), &day(0));
}

#[test]
fn hover_writes_only_a_change_and_leaving_clears_only_its_own_target() {
    let root = Root::new();
    let mut app = App::open(&root);
    app.event("date-2026-09-2026-09-10", Event::Hover(true));
    assert_eq!(
        app.state("hovered").as_str(),
        Some(&*format!("day-{}", TODAY + 9))
    );
    app.event("today", Event::Hover(true));
    assert_eq!(app.state("hovered").as_str(), Some("today"));
    app.event("date-2026-09-2026-09-10", Event::Hover(false));
    assert_eq!(
        app.state("hovered").as_str(),
        Some("today"),
        "a late leave keeps the newer target"
    );
    app.event("today", Event::Hover(false));
    assert_eq!(app.state("hovered").as_str(), Some(""));
}
