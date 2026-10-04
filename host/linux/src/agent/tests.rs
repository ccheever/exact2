//! The Linux agent protocol's tests, driven through `handle`.
use super::*;
use crate::presenter::PainterChoice;
use exact_runner::{DataError, Value};

#[derive(Default)]
struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

/// A source whose one request is handed to work that never replies: the
/// reply is leaked, so neither an outcome nor the drop's abort arrives.
#[derive(Default)]
struct Hung;
impl DataSource for Hung {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
    fn answer(
        &mut self,
        _: &mut exact_runner::Store,
        source: &str,
        _: &[Value],
    ) -> Result<exact_runner::Answer, DataError> {
        Ok(match source {
            "fallback" => exact_runner::Answer::Now(Value::Bool(false)),
            _ => exact_runner::Answer::Later(exact_runner::Request::continuation(1)),
        })
    }
    fn dispatch(&mut self, _: u64, _: &exact_runner::Store) -> exact_runner::Dispatch {
        exact_runner::Dispatch::Run(exact_runner::Work::Later(Box::new(std::mem::forget)))
    }
}

/// Answers `save` later, on the I/O worker, with 1.
#[derive(Default)]
struct Echo;
impl DataSource for Echo {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
    fn answer(
        &mut self,
        _: &mut exact_runner::Store,
        _: &str,
        _: &[Value],
    ) -> Result<exact_runner::Answer, DataError> {
        Ok(exact_runner::Answer::Later(
            exact_runner::Request::continuation(1),
        ))
    }
    fn dispatch(&mut self, _: u64, _: &exact_runner::Store) -> exact_runner::Dispatch {
        exact_runner::Dispatch::Run(exact_runner::Work::Now(Box::new(|| {
            exact_runner::Outcome::Storage(b"1".to_vec())
        })))
    }
    fn parse(
        &mut self,
        _: &mut exact_runner::Store,
        _: &str,
        _: &[Value],
        _: exact_runner::Outcome,
    ) -> Result<exact_runner::Answer, DataError> {
        Ok(exact_runner::Answer::Now(Value::Number(1.0)))
    }
}

/// A jump stops at each timer that sends and lands its reply before
/// the next fires (LLP 1016 D5); a stop at the target still fires the
/// other timers due there.
#[test]
fn a_jump_lands_every_reply_and_fires_every_timer_due() {
    let plan = contract::compile(
        "component App\n  state count = 0\n  mutation result as shape number\n  action ping\n    send result = save()\n  action tock\n    count = count + 1\n  task pings mount\n    every(300, ping)\n  task tocks mount\n    every(300, tock)\n  view\n    text toString(count)\n",
    )
    .unwrap();
    let (mut p, boot_error) = Presenter::boot_with(
        &plan.encode(),
        Echo,
        (300.0, 300.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(boot_error.is_none(), "{boot_error:?}");
    let count = |p: &mut Presenter<Echo>| {
        let state: serde_json::Value =
            serde_json::from_str(&handle(p, r#"{"op":"state"}"#)).unwrap();
        state["slots"]["count"].clone()
    };
    let reply = handle(&mut p, r#"{"op":"clock","to":300}"#);
    assert_eq!(count(&mut p), serde_json::json!(1), "{reply}");
    let reply = handle(&mut p, r#"{"op":"clock","to":1500}"#);
    assert_eq!(count(&mut p), serde_json::json!(5), "{reply}");
    let logs = handle(&mut p, r#"{"op":"logs"}"#);
    assert_eq!(logs.matches("fulfil ").count(), 5, "{logs}");
    assert!(!logs.contains("dropped"), "{logs}");
}

/// LLP 1012 §1: a targeted `tree` is the target and its descendants, and
/// `shallow` the target alone, as the runner answers on every host.
#[test]
fn tree_answers_its_target() {
    let plan = contract::compile(
        "component App\n  view\n    column testId=\"outer\"\n      column testId=\"inner\"\n        text \"a\" testId=\"a\"\n      text \"b\" testId=\"b\"\n",
    )
    .unwrap();
    let bytes = contract::bake(plan, NoData).unwrap().encode();
    let (mut p, _) = Presenter::boot_with(
        &bytes,
        NoData,
        (300.0, 300.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    let ids = |p: &mut Presenter<NoData>, request: &str| -> Vec<String> {
        let reply: serde_json::Value = serde_json::from_str(&handle(p, request)).unwrap();
        reply["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|n| n["props"]["testId"].as_str().map(str::to_owned))
            .collect()
    };
    assert_eq!(
        ids(&mut p, r#"{"op":"tree","target":"inner"}"#),
        ["inner", "a"]
    );
    assert_eq!(
        ids(&mut p, r#"{"op":"tree","target":"inner","shallow":true}"#),
        ["inner"]
    );
    assert_eq!(
        ids(&mut p, r#"{"op":"tree"}"#),
        ["outer", "inner", "a", "b"]
    );
    for refused in [
        r#"{"op":"tree","target":"missing"}"#,
        r#"{"op":"tree","shallow":true}"#,
        r#"{"op":"tree","target":"inner","shallow":1}"#,
    ] {
        assert!(handle(&mut p, refused).contains("\"error\""), "{refused}");
    }
}

/// LLP 1061 D5: `prefer` sets what `exactViewport()` answers and the
/// system appearance; an unknown feature is refused and nothing applies.
#[test]
fn prefer_sets_the_display_preferences_by_their_media_names() {
    let plan = contract::compile(
        "shape M\n  prefersReducedMotion: bool\ncomponent App\n  resource m = exactViewport() as shape M\n  view\n    text (m.prefersReducedMotion ? \"still\" : \"moving\") testId=\"t\"\n",
    )
    .unwrap();
    let bytes = contract::bake(plan, NoData).unwrap().encode();
    let (mut p, _) = Presenter::boot_with(
        &bytes,
        NoData,
        (300.0, 300.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    let text = |p: &mut Presenter<NoData>| handle(p, r#"{"op":"tree"}"#);
    assert!(text(&mut p).contains("moving"));
    let refused = handle(
        &mut p,
        r#"{"op":"prefer","media":{"prefers-reduced-motion":"reduce","prefers-contrast":"loud"}}"#,
    );
    assert!(refused.contains("\"error\""), "{refused}");
    assert!(text(&mut p).contains("moving"), "nothing applied");
    let reply: serde_json::Value = serde_json::from_str(&handle(
        &mut p,
        r#"{"op":"prefer","media":{"prefers-reduced-motion":"reduce","prefers-color-scheme":"dark"}}"#,
    ))
    .unwrap();
    assert_eq!(reply["media"]["prefers-reduced-motion"], "reduce");
    assert_eq!(
        reply["media"]["prefers-reduced-transparency"],
        "no-preference"
    );
    assert_eq!(reply["media"]["prefers-color-scheme"], "dark");
    assert!(text(&mut p).contains("still"));
    assert!(p.dark(), "no app override: the system's dark");
    p.app_scheme(Some(false));
    assert!(!p.dark(), "the app's own choice wins");
}

/// LLP 1069.000 D1, D2, D6: `prefer` sets contrast, the system's scheme
/// beneath an app's own, and what `exactPage()` answers; `state.device`
/// shows them without an app declaring either source.
#[test]
fn prefer_sets_contrast_scheme_and_the_page_facts() {
    let plan = contract::compile(
        "shape M\n  prefersContrast: string\n  prefersColorScheme: string\nshape P\n  visibilityState: string\n  onLine: bool\n  canShare: bool\n  canOpenFiles: bool\ncomponent App\n  resource m = exactViewport() as shape M\n  resource g = exactPage() as shape P\n  view\n    text `${m.prefersContrast} ${m.prefersColorScheme} ${g.visibilityState} ${g.onLine ? \"online\" : \"offline\"} ${g.canShare ? \"share\" : \"no-share\"} ${g.canOpenFiles ? \"pickers\" : \"no-pickers\"}` testId=\"t\"\n",
    )
    .unwrap();
    let bytes = contract::bake(plan, NoData).unwrap().encode();
    let (mut p, _) = Presenter::boot_with(
        &bytes,
        NoData,
        (300.0, 300.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    let text = |p: &mut Presenter<NoData>| handle(p, r#"{"op":"tree"}"#);
    assert!(
        text(&mut p).contains("no-preference light visible online no-share no-pickers"),
        "{}",
        text(&mut p)
    );
    p.app_scheme(Some(false));
    let reply: serde_json::Value = serde_json::from_str(&handle(
        &mut p,
        r#"{"op":"prefer","media":{"prefers-contrast":"more","prefers-color-scheme":"dark"},"page":{"visibility-state":"hidden","online":false,"can-share":"true","can-open-files":true}}"#,
    ))
    .unwrap();
    assert_eq!(reply["media"]["prefers-contrast"], "more");
    assert_eq!(reply["page"]["online"], false);
    assert!(
        text(&mut p).contains("more dark hidden offline share pickers"),
        "{}",
        text(&mut p)
    );
    assert!(!p.dark(), "the app's own scheme still paints");
    let state: serde_json::Value =
        serde_json::from_str(&handle(&mut p, r#"{"op":"state"}"#)).unwrap();
    assert_eq!(state["device"]["prefersColorScheme"], "dark");
    assert_eq!(state["device"]["visibilityState"], "hidden");
    assert_eq!(state["device"]["canOpenFiles"], true);
    let refused = handle(&mut p, r#"{"op":"prefer","page":{"online":"maybe"}}"#);
    assert!(refused.contains("\"error\""), "{refused}");
    // LLP 1069.000 D3: the root font size is layout, not a resource.
    let reply: serde_json::Value = serde_json::from_str(&handle(
        &mut p,
        r#"{"op":"prefer","page":{"root-font-size":24}}"#,
    ))
    .unwrap();
    assert_eq!(reply["page"]["root-font-size"], 24.0);
    let state: serde_json::Value =
        serde_json::from_str(&handle(&mut p, r#"{"op":"state"}"#)).unwrap();
    assert_eq!(state["device"]["rootFontSize"], 24);
    let refused = handle(&mut p, r#"{"op":"prefer","page":{"root-font-size":0}}"#);
    assert!(refused.contains("\"error\""), "{refused}");
}

/// Answers `item` with 1 a moment later, from another thread: a fetch
/// still in flight when the clock is asked to move.
#[derive(Default)]
struct Slow;
impl DataSource for Slow {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
    fn answer(
        &mut self,
        _: &mut exact_runner::Store,
        source: &str,
        _: &[Value],
    ) -> Result<exact_runner::Answer, DataError> {
        Ok(match source {
            "fallback" => exact_runner::Answer::Now(Value::Number(0.0)),
            _ => exact_runner::Answer::Later(exact_runner::Request::continuation(1)),
        })
    }
    fn dispatch(&mut self, _: u64, _: &exact_runner::Store) -> exact_runner::Dispatch {
        exact_runner::Dispatch::Run(exact_runner::Work::Later(Box::new(|reply| {
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(60));
                reply.send(exact_runner::Outcome::Storage(b"1".to_vec()));
            });
        })))
    }
    fn parse(
        &mut self,
        _: &mut exact_runner::Store,
        _: &str,
        _: &[Value],
        _: exact_runner::Outcome,
    ) -> Result<exact_runner::Answer, DataError> {
        Ok(exact_runner::Answer::Now(Value::Number(1.0)))
    }
}

/// habits, pomodoro, kanban: `clock data` lands what launch started (a
/// store's open, a fetch) before a test's first step, the clock unmoved and
/// no timer fired, where `clock settle` would have moved the clock.
#[test]
fn clock_data_lands_the_replies_without_moving_the_clock() {
    let plan = contract::compile(
        "component App\n  state count = 0\n  resource item = item() as shape number else fallback()\n  action tock\n    count = count + 1\n  task tocks mount\n    every(300, tock)\n  view\n    text `${count} ${item}` testId=\"log\" height=20\n",
    )
    .unwrap();
    let (mut p, boot_error) = Presenter::boot_with(
        &plan.encode(),
        Slow,
        (300.0, 300.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(boot_error.is_none(), "{boot_error:?}");
    let json = |s: String| -> serde_json::Value { serde_json::from_str(&s).unwrap() };
    let state = json(handle(&mut p, r#"{"op":"state"}"#));
    assert_eq!(state["resources"]["item"], 0, "in flight: {state}");
    let reply = json(handle(&mut p, r#"{"op":"clock","data":true}"#));
    assert_eq!(reply["settled"], true, "{reply}");
    assert_eq!(reply["clock"], 0.0, "{reply}");
    let state = json(handle(&mut p, r#"{"op":"state"}"#));
    assert_eq!(state["resources"]["item"], 1, "the reply landed: {state}");
    assert_eq!(state["slots"]["count"], 0, "no timer fired: {state}");
}

/// LLP 1069.007 §5 item 4, with a synthetic capability standing in for
/// the first real one: a held device request, a due app timer and an
/// unfinished fetch together. `clock +N` fires the timer without waiting
/// on the hold; `clock settle` waits for the fetch, never for the hold,
/// and says `device` with the tickets; `tap @t` / `type @t` answer it
/// once, `substituted`; a hold whose node goes is retired.
#[test]
fn a_held_device_request_is_answered_by_ticket_and_never_waited_on() {
    let plan = contract::compile(
        "component App\n  state count = 0\n  state show = true\n  resource item = item() as shape number else fallback()\n  action tock\n    count = count + 1\n  action hide\n    show = false\n  task tocks mount\n    every(300, tock)\n  view\n    column width=300 height=300\n      box testId=\"picker\" width=100 height=40\n      when show\n        box testId=\"doc\" width=100 height=40\n      button press=hide testId=\"hide\" width=100 height=40\n        text \"Hide\"\n      text `${count} ${item}` testId=\"log\" height=20\n",
    )
    .unwrap();
    let (mut p, boot_error) = Presenter::boot_with(
        &plan.encode(),
        Slow,
        (300.0, 300.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(boot_error.is_none(), "{boot_error:?}");
    let id = |p: &Presenter<Slow>, test_id: &str| {
        let k = p.host().kernel();
        k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
    };
    let json = |s: String| -> serde_json::Value { serde_json::from_str(&s).unwrap() };
    let (picker, doc, hide) = (id(&p, "picker"), id(&p, "doc"), id(&p, "hide"));
    let args = r#"{"id":"picker","accept":["image/*"],"multiple":false}"#;
    let t = p
        .host_mut()
        .runner_mut()
        .hold("sample", Some(picker), args, &[], true);
    let state = json(handle(&mut p, r#"{"op":"state"}"#));
    let held = state["pending"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["ticket"] == t)
        .cloned()
        .unwrap_or_default();
    assert_eq!(held["name"], "picker", "{state}");
    assert_eq!(held["device"]["capability"], "sample");
    assert_eq!(held["device"]["args"]["accept"][0], "image/*");
    assert!(
        state["pending"].as_array().unwrap().len() >= 2,
        "the fetch is pending beside the hold: {state}"
    );

    let bound = std::time::Duration::from_secs(3);
    let started = std::time::Instant::now();
    let reply = clock_within(&mut p, r#"{"op":"clock","to":600}"#, bound);
    assert!(started.elapsed() < bound, "clock +N waited: {reply}");
    let state = json(handle(&mut p, r#"{"op":"state"}"#));
    assert_eq!(state["slots"]["count"], 2, "both timers fired: {reply}");

    let started = std::time::Instant::now();
    let reply = json(clock_within(
        &mut p,
        r#"{"op":"clock","settle":true}"#,
        bound,
    ));
    assert!(started.elapsed() < bound, "settle waited on the hold");
    assert_eq!(reply["settled"], false, "{reply}");
    assert_eq!(reply["reason"], "device", "{reply}");
    assert_eq!(reply["tickets"], serde_json::json!([t]), "{reply}");
    assert_eq!(
        json(handle(&mut p, r#"{"op":"state"}"#))["resources"]["item"],
        1,
        "settle waited for the fetch"
    );

    let wrong = handle(
        &mut p,
        &format!(r#"{{"op":"tap","ticket":{t},"choice":"allow"}}"#),
    );
    assert!(wrong.contains("tap takes cancel"), "{wrong}");
    let answered = json(handle(
        &mut p,
        &format!(r#"{{"op":"type","ticket":{t},"text":"fixtures/cat.jpg"}}"#),
    ));
    assert_eq!(answered["delivery"], "substituted", "{answered}");
    assert_eq!(answered["answered"], "value");
    let again = handle(
        &mut p,
        &format!(r#"{{"op":"tap","ticket":{t},"choice":"cancel"}}"#),
    );
    assert!(again.contains(&format!("not pending: @{t}")), "{again}");
    let logs = handle(&mut p, r#"{"op":"logs"}"#);
    assert!(
        logs.contains(&format!("device sample {t} held (agent)")),
        "{logs}"
    );
    assert!(logs.contains(&format!("device sample {t} answered: a value")));
    assert!(
        !logs.contains("cat.jpg"),
        "a typed value is never journalled"
    );
    let reply = json(clock_within(
        &mut p,
        r#"{"op":"clock","settle":true}"#,
        bound,
    ));
    assert_eq!(reply["settled"], true, "{reply}");

    let u = p
        .host_mut()
        .runner_mut()
        .hold("sample", Some(doc), "{}", &[], true);
    handle(&mut p, &format!(r#"{{"op":"tap","id":{hide}}}"#));
    let state = json(handle(&mut p, r#"{"op":"state"}"#));
    assert!(
        !state["pending"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["ticket"] == u),
        "the node went, and its hold with it: {state}"
    );
    let logs = handle(&mut p, r#"{"op":"logs"}"#);
    assert!(
        logs.contains(&format!("device sample {u} retired")),
        "{logs}"
    );
    let late = handle(
        &mut p,
        &format!(r#"{{"op":"tap","ticket":{u},"choice":"cancel"}}"#),
    );
    assert!(late.contains(&format!("not pending: @{u}")), "{late}");
}

/// LLP 1069.007 D2: the offset follows the virtual date across a DST
/// change — Los Angeles, an hour before 2026's spring-forward, then two
/// hours on.
#[test]
fn a_clock_move_across_a_dst_change_recomputes_the_offset() {
    let plan = contract::compile("component App\n  view\n    text \"x\" height=20\n").unwrap();
    let (mut p, _) = Presenter::boot_with(
        &plan.encode(),
        NoData,
        (300.0, 300.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    let place = exact_runner::time::Place {
        locale: "en-US".into(),
        time_zone: "America/Los_Angeles".into(),
        seed: 1.0,
    };
    assert!(p.set_place(&place).is_none());
    assert!(p.set_time(1_772_960_400_000.0, -480.0).is_none());
    let offset = |p: &mut Presenter<NoData>| {
        let state: serde_json::Value =
            serde_json::from_str(&handle(p, r#"{"op":"state"}"#)).unwrap();
        state["time"]["utcOffset"].clone()
    };
    handle(&mut p, r#"{"op":"clock","to":1800000}"#);
    assert_eq!(offset(&mut p), -480, "still standard time at 09:30Z");
    handle(&mut p, r#"{"op":"clock","to":7200000}"#);
    assert_eq!(offset(&mut p), -420, "daylight time from 10:00Z");
}

/// LLP 1069.002 D9 on Linux: `showPicker` under the agent is a held
/// `pick` with its input's summary; settle stops at it; `type @t` with
/// a file copies it into `app:/tmp/picked/` and fires `change` with the
/// record; a refused answer leaves the hold; `tap @t cancel` fires
/// `cancel`.
#[test]
fn a_picker_is_held_and_answered_by_ticket() {
    let plan = contract::compile(
        "component App\n  state picked = \"none\"\n  state cancels = 0\n  action choose\n    showPicker(\"attach\")\n  action attach(files: list<Picked>)\n    picked = match first(files) { case some(f) => match f.width { case some(w) => `${length(files)} ${f.name} ${f.type} ${f.size} ${w} ${f.path}`, case none => \"no width\" }, case none => \"empty\" }\n  action cancelled\n    cancels = cancels + 1\n  view\n    column width=300 height=300\n      input type=\"file\" accept=\"image/png\" id=\"attach\" testId=\"attach\" display=\"none\" change=attach cancel=cancelled\n      button press=choose testId=\"choose\" width=100 height=40\n        text \"Add\"\n      text picked testId=\"picked\" height=20\n",
    )
    .unwrap();
    let (mut p, _) = Presenter::boot_with(
        &plan.encode(),
        NoData,
        (300.0, 300.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    let json = |s: String| -> serde_json::Value { serde_json::from_str(&s).unwrap() };
    let dir = std::env::temp_dir().join(format!("exact-picker-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    png.extend(64u32.to_be_bytes());
    png.extend(48u32.to_be_bytes());
    let fixture = dir.join("cat.png");
    std::fs::write(&fixture, &png).unwrap();
    let choose = {
        let k = p.host().kernel();
        k.node_by_key(k.find_by_test_id("choose")[0]).unwrap().id
    };
    handle(&mut p, &format!(r#"{{"op":"tap","id":{choose}}}"#));
    let state = json(handle(&mut p, r#"{"op":"state"}"#));
    let held = state["pending"][0].clone();
    assert_eq!(held["name"], "attach", "{state}");
    assert_eq!(held["device"]["capability"], "pick");
    assert_eq!(held["device"]["args"]["accept"][0], "image/png");
    assert_eq!(held["device"]["args"]["multiple"], false);
    let t = held["ticket"].as_u64().unwrap();
    let settle = json(handle(&mut p, r#"{"op":"clock","settle":true}"#));
    assert_eq!(settle["reason"], "device", "{settle}");
    assert_eq!(settle["tickets"], serde_json::json!([t]));

    let wrong = handle(
        &mut p,
        &format!(r#"{{"op":"type","ticket":{t},"text":"/x/a.jpg"}}"#),
    );
    assert!(wrong.contains("not among accept"), "{wrong}");
    let text = serde_json::Value::from(fixture.to_string_lossy().into_owned());
    let answered = json(handle(
        &mut p,
        &format!(r#"{{"op":"type","ticket":{t},"text":{text}}}"#),
    ));
    assert_eq!(answered["delivery"], "substituted", "{answered}");
    let state = json(handle(&mut p, r#"{"op":"state"}"#));
    let picked = state["slots"]["picked"].as_str().unwrap().to_owned();
    assert!(
        picked.starts_with(&format!(
            "1 cat.png image/png {} 64 app:/tmp/picked/",
            png.len()
        )),
        "{picked}"
    );
    let copied = crate::picker::resolve(picked.rsplit(' ').next().unwrap()).unwrap();
    assert_eq!(std::fs::read(copied).unwrap(), png);
    let logs = handle(&mut p, r#"{"op":"logs"}"#);
    assert!(
        logs.contains(&format!("device pick {t} answered: 1 item")),
        "{logs}"
    );
    assert!(!logs.contains("cat.png\""), "the value is never journalled");

    handle(&mut p, &format!(r#"{{"op":"tap","id":{choose}}}"#));
    let state = json(handle(&mut p, r#"{"op":"state"}"#));
    let u = state["pending"][0]["ticket"].as_u64().unwrap();
    handle(
        &mut p,
        &format!(r#"{{"op":"tap","ticket":{u},"choice":"cancel"}}"#),
    );
    let state = json(handle(&mut p, r#"{"op":"state"}"#));
    assert_eq!(state["slots"]["cancels"], 1, "{state}");
    let settle = json(handle(&mut p, r#"{"op":"clock","settle":true}"#));
    assert_eq!(settle["settled"], true, "{settle}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn settle_takes_one_bound_for_a_request_that_never_answers() {
    let plan = contract::compile(
        "component App\n  resource item = item() as shape bool else fallback()\n  view\n    text \"x\" height=20\n",
    )
    .unwrap();
    let (mut p, boot_error) = Presenter::boot_with(
        &plan.encode(),
        Hung,
        (300.0, 300.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(boot_error.is_none(), "{boot_error:?}");
    let bound = std::time::Duration::from_millis(200);
    let started = std::time::Instant::now();
    let reply = clock_within(&mut p, r#"{"op":"clock","settle":true}"#, bound);
    let took = started.elapsed();
    assert!(p.pending(), "the request is still out: {reply}");
    assert!(reply.contains("\"settled\":false"), "{reply}");
    assert!(reply.contains("\"reason\":\"requests\""), "{reply}");
    assert!(
        took < bound * 2,
        "settle took {took:?} for a {bound:?} bound"
    );
}

#[test]
fn resize_input_uses_presenter_and_paints_before_ack() {
    let plan = contract::compile("component App\n  view\n    view width=\"100%\" height=\"100%\" background-color=\"#f00\"\n").unwrap();
    let (mut p, boot_error) = Presenter::boot_with(
        &plan.encode(),
        NoData,
        (390.0, 844.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(boot_error.is_none(), "{boot_error:?}");
    let reply: serde_json::Value =
        serde_json::from_str(&handle(&mut p, r#"{"op":"tap","resize":[640,480]}"#)).unwrap();
    assert_eq!(reply["resized"], serde_json::json!([640.0, 480.0]));
    assert_eq!(p.viewport(), (640.0, 480.0));
    assert!(!p.dirty(), "a resize must paint before acknowledgment");
    assert_eq!(reply["painted"], serde_json::json!([640, 480]));
    let layout: serde_json::Value =
        serde_json::from_str(&handle(&mut p, r#"{"op":"layout"}"#)).unwrap();
    assert_eq!(layout["viewport"]["w"], 640);
    assert_eq!(layout["viewport"]["h"], 480);
    for request in [
        r#"{"op":"tap","resize":[0,480]}"#,
        r#"{"op":"tap","resize":[-1,480]}"#,
        r#"{"op":"tap","resize":[true,480]}"#,
        r#"{"op":"tap","resize":["640",480]}"#,
        r#"{"op":"tap","resize":[null,480]}"#,
        r#"{"op":"tap","resize":[NaN,480]}"#,
        r#"{"op":"tap","resize":[1e300,480]}"#,
        r#"{"op":"tap","resize":[4096,4096]}"#,
        r#"{"op":"tap","resize":[640.5,480]}"#,
        r#"{"op":"tap","resize":[640,480,1]}"#,
        r#"{"op":"tap","resize":[640,480],"wheel":[0,1]}"#,
    ] {
        let reply = handle(&mut p, request);
        assert!(reply.starts_with("{\"error\""), "{request}: {reply}");
        assert_eq!(
            p.viewport(),
            (640.0, 480.0),
            "invalid input mutated size: {request}"
        );
    }
}

#[test]
fn a_hover_never_presses_and_a_key_is_never_text() {
    let plan = contract::compile("component App\n  state hot = false\n  state presses = 0\n  state text = \"kept\"\n  state lastKey = \"\"\n  action hovered(value)\n    hot = value\n  action pressed\n    presses = presses + 1\n  action edit(value)\n    text = value\n  action keyed(value)\n    lastKey = value\n  view\n    column width=300 height=300\n      box hover=hovered press=pressed testId=\"hot\" width=200 height=60\n      box testId=\"away\" width=200 height=60\n      input value=text change=edit key=keyed testId=\"field\" height=32\n      text `${hot} ${presses} ${text} ${lastKey}` testId=\"log\" height=20\n").unwrap();
    let (mut p, boot_error) = Presenter::boot_with(
        &plan.encode(),
        NoData,
        (300.0, 300.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(boot_error.is_none(), "{boot_error:?}");
    let id = |p: &Presenter<NoData>, test_id: &str| {
        let k = p.host().kernel();
        k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
    };
    let log = |p: &Presenter<NoData>| {
        let k = p.host().kernel();
        let node = k.node_by_key(k.find_by_test_id("log")[0]).unwrap();
        node.props
            .str(exact_kernel::PropId::Text)
            .unwrap()
            .to_string()
    };
    let (hot, away, field) = (id(&p, "hot"), id(&p, "away"), id(&p, "field"));
    let reply = handle(
        &mut p,
        &format!(r#"{{"op":"tap","id":{hot},"hover":true}}"#),
    );
    assert!(reply.contains("\"hover\":true"), "{reply}");
    assert_eq!(log(&p), "true 0 kept ", "a hover enters and never presses");
    handle(
        &mut p,
        &format!(r#"{{"op":"tap","id":{away},"hover":true}}"#),
    );
    assert_eq!(log(&p), "false 0 kept ", "the pointer left");
    handle(
        &mut p,
        &format!(r#"{{"op":"type","id":{field},"key":"Escape"}}"#),
    );
    assert_eq!(
        log(&p),
        "false 0 kept Escape",
        "a key is heard by name and types nothing"
    );
}

#[test]
fn a_target_out_of_view_is_revealed_and_a_control_takes_a_value() {
    // ledger F7, shop F11: a scroller's row and a row below the fold
    // scroll into view; kanban F17: a checkbox and a select by label.
    let plan = contract::compile("component App\n  state on = false\n  state pick = \"a\"\n  action set(value: bool)\n    on = value\n  action choose(value: string)\n    pick = value\n  view\n    column width=300\n      input type=\"checkbox\" checked=on change=set testId=\"agree\"\n      select value=pick change=choose testId=\"pick\"\n        option \"Alpha\" value=\"a\"\n        option \"Beta\" value=\"b\"\n      text `${on} ${pick}` testId=\"log\" height=20\n      scroll testId=\"inner\" height=100\n        box height=400\n        box testId=\"deep\" width=50 height=20\n      box testId=\"auto\" height=100 overflow-y=\"auto\"\n        box height=400\n        box testId=\"autodeep\" width=50 height=20\n      box height=900\n      box testId=\"far\" width=50 height=20\n").unwrap();
    let (mut p, boot_error) = Presenter::boot_with(
        &plan.encode(),
        NoData,
        (300.0, 300.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(boot_error.is_none(), "{boot_error:?}");
    let id = |p: &Presenter<NoData>, test_id: &str| {
        let k = p.host().kernel();
        k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
    };
    let log = |p: &Presenter<NoData>| {
        let k = p.host().kernel();
        let node = k.node_by_key(k.find_by_test_id("log")[0]).unwrap();
        node.props
            .str(exact_kernel::PropId::Text)
            .unwrap()
            .to_string()
    };
    let (agree, pick, deep, far) = (
        id(&p, "agree"),
        id(&p, "pick"),
        id(&p, "deep"),
        id(&p, "far"),
    );
    // An `overflow: auto` container scrolls to reveal as a `scroll` does
    // (Astra's batch 2 review, finding 5).
    let autodeep = id(&p, "autodeep");
    let reply = handle(
        &mut p,
        &format!(r#"{{"op":"type","id":{agree},"text":"true"}}"#),
    );
    assert!(reply.contains("\"checked\":true"), "{reply}");
    handle(
        &mut p,
        &format!(r#"{{"op":"type","id":{agree},"text":"true"}}"#),
    );
    let reply = handle(
        &mut p,
        &format!(r#"{{"op":"type","id":{pick},"text":"Beta"}}"#),
    );
    assert!(reply.contains("\"value\":\"b\""), "{reply}");
    assert_eq!(
        log(&p),
        "true b",
        "on stays on, and the label chose its value"
    );
    let reply = handle(
        &mut p,
        &format!(r#"{{"op":"type","id":{agree},"text":"yes"}}"#),
    );
    assert!(reply.contains("takes true or false"), "{reply}");
    for target in [deep, autodeep, far] {
        let reply = handle(&mut p, &format!(r#"{{"op":"reveal","id":{target}}}"#));
        assert!(reply.contains("\"scrolled\":true"), "{reply}");
        let to: serde_json::Value = serde_json::from_str(&reply).unwrap();
        let y = to["to"][1].as_f64().unwrap();
        assert!((0.0..300.0).contains(&y), "#{target} is in view: {reply}");
        let again = handle(&mut p, &format!(r#"{{"op":"reveal","id":{target}}}"#));
        assert!(again.contains("\"scrolled\":false"), "{again}");
    }
}

/// A `video` or `audio` here plays nothing (no decoder, no audio output):
/// `state.media` says so per node, and the tree marks it unavailable
/// (LLP 1042 §5, §8).
#[test]
fn media_is_reported_unavailable() {
    let plan = contract::compile(
        "component App\n  view\n    column\n      audio \"assets/ding.wav\" testId=\"sound\" paused=false\n",
    )
    .unwrap();
    let (mut p, _) = Presenter::boot_with(
        &plan.encode(),
        NoData,
        (300.0, 300.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    let json = |s: String| -> serde_json::Value { serde_json::from_str(&s).unwrap() };
    let state = json(handle(&mut p, r#"{"op":"state"}"#));
    let media = state["media"].as_array().expect("state.media");
    assert_eq!(media.len(), 1, "{state}");
    assert_eq!(media[0]["state"]["paused"], true);
    assert!(media[0]["state"]["unavailable"].is_string(), "{state}");
    // No scratch store named: the drive has no app storage, and says so (trivia F7).
    assert_eq!(state["storage"]["available"], false, "{state}");
    assert_eq!(state["storage"]["code"], "agent", "{state}");
    let tree = json(handle(&mut p, r#"{"op":"tree"}"#));
    let sound = tree["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["type"] == "Video")
        .unwrap();
    assert_eq!(sound["unavailable"], true, "{tree}");
}

/// An input's end lands the `then` of the answer it settled (LLP 1012 §2;
/// trivia F3): `clock land` runs it, the clock unmoved and no timer fired.
#[test]
fn land_runs_an_answers_then_and_no_timer() {
    let plan = contract::compile(
        "component App\n  state screen = \"start\"\n  state ticks = 0\n  mutation m as shape bool then opened\n  action go\n    send m = fallback()\n  action opened\n    screen = \"play\"\n  action tick\n    ticks = ticks + 1\n  task ticking mount\n    every(100, tick)\n  view\n    column\n      button press=go testId=\"go\" aria-label=\"Go\"\n        text \"Go\"\n      text `${screen} ${ticks}`\n",
    )
    .unwrap();
    let (mut p, _) = Presenter::boot_with(
        &plan.encode(),
        Hung,
        (300.0, 300.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    let json = |s: String| -> serde_json::Value { serde_json::from_str(&s).unwrap() };
    let slots = |p: &mut Presenter<Hung>| json(handle(p, r#"{"op":"state"}"#))["slots"].clone();
    handle(&mut p, r#"{"op":"clock","to":100}"#);
    let go = p
        .host()
        .runner()
        .kernel()
        .find_first_by_test_id("go")
        .unwrap();
    let go = p.host().runner().kernel().node_by_key(go).unwrap().id;
    let tapped = handle(&mut p, &format!(r#"{{"op":"tap","id":{go}}}"#));
    assert!(!tapped.contains("\"error\""), "{tapped}");
    assert_eq!(slots(&mut p)["screen"], "start", "armed, not yet run");
    let landed = json(handle(&mut p, r#"{"op":"clock","land":true}"#));
    assert_eq!(landed["clock"], 100.0, "{landed}");
    let s = slots(&mut p);
    assert_eq!(
        (s["screen"].clone(), s["ticks"].clone()),
        ("play".into(), 1.into()),
        "{s}"
    );
}

/// Spreadsheet F6: `type <id> paste|copy|cut` delivers the clipboard's
/// event at the target, the nearest node with a handler hearing it.
#[test]
fn the_clipboard_events_reach_the_nearest_handler() {
    let plan = contract::compile("component App\n  state log = \"\"\n  action pasted(at: string, e: ClipboardEvent)\n    log = `${log}${at}:${e.text};`\n  action copied\n    log = `${log}copy;`\n  action seen\n    log = log\n  view\n    column width=300 paste=pasted(\"grid\") copy=copied testId=\"grid\"\n      box testId=\"cell\" width=50 height=20 focus=seen\n      text log testId=\"log\" height=20\n").unwrap();
    let (mut p, boot_error) = Presenter::boot_with(
        &plan.encode(),
        NoData,
        (300.0, 300.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(boot_error.is_none(), "{boot_error:?}");
    let id = |p: &Presenter<NoData>, test_id: &str| {
        let k = p.host().kernel();
        k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
    };
    let (cell, log) = (id(&p, "cell"), id(&p, "log"));
    let reply = handle(
        &mut p,
        &format!(r#"{{"op":"type","id":{cell},"clipboard":"paste","text":"a\tb"}}"#),
    );
    assert!(reply.contains("\"clipboard\":\"paste\""), "{reply}");
    handle(
        &mut p,
        &format!(r#"{{"op":"type","id":{cell},"clipboard":"copy"}}"#),
    );
    let k = p.host().kernel();
    assert_eq!(
        k.node(log).unwrap().props.str(exact_kernel::PropId::Text),
        Some("grid:a\tb;copy;")
    );
    let reply = handle(
        &mut p,
        &format!(r#"{{"op":"type","id":{log},"clipboard":"cut"}}"#),
    );
    assert!(reply.contains("no cut handler"), "{reply}");
}

/// Gallery F20: `tap <id> modifiers Shift+Meta` presses with the keys held,
/// which the action's `MouseEvent` reports; the keys are released after.
#[test]
fn a_tap_holds_its_modifiers_for_the_press() {
    let plan = contract::compile("component App\n  state log = \"\"\n  action pick(e: MouseEvent)\n    log = `${log}${e.shiftKey}${e.metaKey}${e.altKey};`\n  view\n    column width=300\n      button press=pick testId=\"b\" width=50 height=20\n      text log testId=\"log\" height=20\n").unwrap();
    let (mut p, boot_error) = Presenter::boot_with(
        &plan.encode(),
        NoData,
        (300.0, 300.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(boot_error.is_none(), "{boot_error:?}");
    let id = |p: &Presenter<NoData>, test_id: &str| {
        let k = p.host().kernel();
        k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
    };
    let (b, log) = (id(&p, "b"), id(&p, "log"));
    handle(
        &mut p,
        &format!(r#"{{"op":"tap","id":{b},"modifiers":"Shift+Meta"}}"#),
    );
    handle(&mut p, &format!(r#"{{"op":"tap","id":{b}}}"#));
    let k = p.host().kernel();
    assert_eq!(
        k.node(log).unwrap().props.str(exact_kernel::PropId::Text),
        Some("truetruefalse;falsefalsefalse;")
    );
    let reply = handle(
        &mut p,
        &format!(r#"{{"op":"tap","id":{b},"modifiers":"Hyper"}}"#),
    );
    assert!(reply.contains("error"), "{reply}");
}

/// A request that answers long after the drive moves on.
#[derive(Default)]
struct Stalled;
impl DataSource for Stalled {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
    fn answer(
        &mut self,
        _: &mut exact_runner::Store,
        _: &str,
        _: &[Value],
    ) -> Result<exact_runner::Answer, DataError> {
        Ok(exact_runner::Answer::Later(
            exact_runner::Request::continuation(1),
        ))
    }
    fn dispatch(&mut self, _: u64, _: &exact_runner::Store) -> exact_runner::Dispatch {
        exact_runner::Dispatch::Run(exact_runner::Work::Later(Box::new(|reply| {
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(30));
                reply.send(exact_runner::Outcome::Storage(b"1".to_vec()));
            });
        })))
    }
}

/// A jump waits for no request with no timer due before it, and its reply
/// names what it left in flight, as the web hosts' do (calendar F10, workout
/// F6); `clock settle` waits for it and says `requests` past its bound.
#[test]
fn a_jump_names_the_requests_it_left_in_flight() {
    let plan = contract::compile(
        "component App\n  resource item = item() as shape number\n  view\n    text `${item}` testId=\"log\"\n",
    )
    .unwrap();
    let (mut p, boot_error) = Presenter::boot_with(
        &plan.encode(),
        Stalled,
        (300.0, 300.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(boot_error.is_none(), "{boot_error:?}");
    let json = |s: String| -> serde_json::Value { serde_json::from_str(&s).unwrap() };
    let reply = json(handle(&mut p, r#"{"op":"clock","to":100}"#));
    assert_eq!(reply["clock"], 100, "{reply}");
    assert_eq!(reply["inflight"], 1, "{reply}");
    let reply = json(clock_within(
        &mut p,
        r#"{"op":"clock","settle":true}"#,
        std::time::Duration::from_millis(100),
    ));
    assert_eq!(reply["settled"], false, "{reply}");
    assert_eq!(reply["reason"], "requests", "{reply}");
}

/// Review A1: a drag's contact with `mouse` holds the left button through its
/// phases. Every phase says `mouse`, which a contact takes (a click's `mouse`
/// form stays the click's), and the box hears the press and the release.
#[test]
fn a_mouse_contact_goes_down_holds_and_lifts() {
    let plan = contract::compile("component App\n  state log = \"\"\n  action at(kind: string, e: PointerEvent)\n    log = `${log}${kind}:${e.pointerType};`\n  view\n    column width=300\n      box testId=\"pad\" width=200 height=100 touch-action=\"none\" pointerdown=at(\"down\") pointerup=at(\"up\")\n      text log testId=\"log\" height=20\n").unwrap();
    let (mut p, boot_error) = Presenter::boot_with(
        &plan.encode(),
        NoData,
        (300.0, 300.0),
        1.0,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(boot_error.is_none(), "{boot_error:?}");
    let id = |p: &Presenter<NoData>, test_id: &str| {
        let k = p.host().kernel();
        k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
    };
    let (pad, log) = (id(&p, "pad"), id(&p, "log"));
    let down = handle(
        &mut p,
        &format!(r#"{{"op":"tap","phase":"down","id":{pad},"x":20,"y":20,"mouse":true}}"#),
    );
    assert!(!down.contains("\"error\""), "{down}");
    // A hold seeks the presenter clock and reports it, so the driver does not seek again (platformer R7).
    let before = p.host().now();
    let held = handle(
        &mut p,
        r#"{"op":"tap","phase":"hold","ms":32,"mouse":true,"virtual":true}"#,
    );
    assert!(!held.contains("\"error\""), "{held}");
    let held: serde_json::Value = serde_json::from_str(&held).unwrap();
    assert_eq!(held["clock"].as_f64(), Some(before + 32.0), "{held}");
    let up = handle(&mut p, r#"{"op":"tap","phase":"up","mouse":true}"#);
    assert!(!up.contains("\"error\""), "{up}");
    let k = p.host().kernel();
    assert_eq!(
        k.node(log).unwrap().props.str(exact_kernel::PropId::Text),
        Some("down:mouse;up:mouse;")
    );
}

#[test]
fn driver_key_uses_the_web_vocabulary() {
    assert_eq!(driver_key("p"), Some(("KeyP", "p")));
    assert_eq!(driver_key("P"), Some(("KeyP", "P")));
    assert_eq!(driver_key("KeyP"), Some(("KeyP", "p")));
    assert_eq!(driver_key("7"), Some(("Digit7", "7")));
    assert_eq!(driver_key("Digit7"), Some(("Digit7", "7")));
    assert_eq!(driver_key("End"), Some(("End", "End")));
    assert_eq!(driver_key("Home"), Some(("Home", "Home")));
    assert_eq!(driver_key("Delete"), Some(("Delete", "Delete")));
    assert_eq!(driver_key(" "), Some(("Space", " ")));
    assert_eq!(driver_key("Space"), Some(("Space", " ")));
    assert_eq!(driver_key("-"), Some(("Minus", "-")));
    assert_eq!(driver_key("+"), Some(("Equal", "+")));
    assert_eq!(driver_key("!"), Some(("Digit1", "!")));
    assert_eq!(driver_key("F1"), Some(("F1", "F1")));
    assert_eq!(driver_key("F12"), Some(("F12", "F12")));
    assert_eq!(driver_key("F24"), Some(("F24", "F24")));
    assert_eq!(driver_key("Shift"), Some(("ShiftLeft", "Shift")));
    assert_eq!(driver_key("ArrowLeft"), Some(("ArrowLeft", "ArrowLeft")));
    assert_eq!(driver_key("Nope"), None);
    assert_eq!(driver_key("Endd"), None);
}
