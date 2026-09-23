//! Real authored app lifecycle tests; no hosts or network fixtures.
use exact_kernel::Kernel;
use exact_live_data::Live;
use exact_plan::Value;
use exact_runner::{Outcome, Response, Runner, Viewport};

fn boot(width: f64) -> Runner<Live> {
    let plan = contract::compile(include_str!("../../../app.contract")).unwrap();
    let baked = contract::bake(plan, Live::default()).unwrap();
    Runner::boot(
        baked,
        Live::default(),
        Kernel::with_monospace(),
        Viewport {
            width,
            height: 844.,
        },
        "/",
    )
    .unwrap()
}
fn present(r: &Runner<Live>, id: &str) -> bool {
    !r.kernel().find_by_test_id(id).is_empty()
}

#[test]
fn phone_and_desktop_mount_one_composer_and_real_content() {
    for width in [390., 1280.] {
        let r = boot(width);
        assert!(present(&r, "rundown"));
        assert_eq!(
            r.kernel().find_by_test_id("crew-input").len(),
            usize::from(width >= 900.)
        );
        assert!(!r.slot("running").unwrap().as_bool().unwrap());
    }
}

#[test]
fn photo_and_phone_sheet_use_existing_typed_bindings() {
    let mut r = boot(390.);
    r.act(
        "command",
        vec![
            Value::str("open"),
            Value::str("photo-00000"),
            Value::Number(0.),
        ],
    )
    .unwrap();
    let handle = r.kernel().find_by_test_id("viewer-handle")[0];
    let target = r.kernel().find_by_test_id("viewer-transform")[0];
    let clip = r.kernel().find_by_test_id("viewer-clip")[0];
    let binding = r.kernel().transform_drag_binding(handle).unwrap();
    assert_eq!(
        (binding.handle, binding.target, binding.clip),
        (handle, target, clip)
    );
    r.act("toggleSheet", vec![]).unwrap();
    let sheet = r.kernel().find_by_test_id("crew-sheet")[0];
    let grip = r.kernel().find_by_test_id("crew-sheet-grip")[0];
    assert_eq!(r.kernel().height_drag_target(grip), Some(sheet));
    assert_eq!(r.kernel().find_by_test_id("crew-input").len(), 1);
    r.act("showPane", vec![Value::str("crew")]).unwrap();
    assert!(!present(&r, "crew-sheet"));
    assert_eq!(r.kernel().find_by_test_id("crew-input").len(), 1);
    assert!(r.kernel().transform_drag_binding(handle).is_none());
}

#[test]
fn root_producer_and_draft_survive_pane_navigation() {
    let mut r = boot(390.);
    r.act("startLive", vec![]).unwrap();
    r.act("showPane", vec![Value::str("crew")]).unwrap();
    r.act("editDraft", vec![Value::str("keep the sound bed")])
        .unwrap();
    r.act("showPane", vec![Value::str("runbook")]).unwrap();
    r.advance(500.).unwrap();
    assert_eq!(r.slot("revision"), Some(&Value::Number(2.)));
    r.act("showPane", vec![Value::str("crew")]).unwrap();
    assert_eq!(r.slot("draft"), Some(&Value::str("keep the sound bed")));
    let transcript = r
        .collections()
        .into_iter()
        .find(|x| x.count == 10_000)
        .unwrap();
    assert!(transcript.rows.len() <= 32);
    r.act("pause", vec![]).unwrap();
    r.advance(1000.).unwrap();
    assert_eq!(r.slot("revision"), Some(&Value::Number(2.)));
}

#[test]
fn selected_load_runs_independently_and_guided_live_restores_ten_thousand_thirty_two() {
    let mut r = boot(390.);
    r.act("chooseLoad", vec![Value::Number(1000.)]).unwrap();
    r.act("chooseBatch", vec![Value::Number(1.)]).unwrap();
    r.act("startSelected", vec![]).unwrap();
    r.advance(250.).unwrap();
    assert_eq!(r.slot("count"), Some(&Value::Number(1000.)));
    assert_eq!(r.slot("batch"), Some(&Value::Number(1.)));
    assert_eq!(r.slot("revision"), Some(&Value::Number(1.)));
    r.act("startLive", vec![]).unwrap();
    assert_eq!(r.slot("count"), Some(&Value::Number(10_000.)));
    assert_eq!(r.slot("batch"), Some(&Value::Number(32.)));
    assert_eq!(r.slot("revision"), Some(&Value::Number(0.)));
    r.advance(500.).unwrap();
    assert_eq!(r.slot("revision"), Some(&Value::Number(1.)));
    r.act("resetScene", vec![]).unwrap();
    r.act("startSelected", vec![]).unwrap();
    assert_eq!(r.slot("running"), Some(&Value::Bool(false)));
}

#[test]
fn real_job_tickets_survive_navigation_resize_and_scene_reset() {
    let mut r = boot(390.);
    assert!(r.take_requests().is_empty());
    r.act("attachFixture", vec![]).unwrap();
    r.act("startJobs", vec![]).unwrap();
    let opened = r.take_requests();
    assert_eq!(opened.len(), 1);
    assert!(opened[0].request.url.contains("/api/open?count=128"));
    // This is a test transport response, never an app-generated completion.
    r.fulfill(
        opened[0].ticket,
        Outcome::Response(Response {
            status: 200,
            headers: vec![],
            body: br#"{"id":7,"count":128}"#.to_vec(),
        }),
    )
    .unwrap();
    let requests = r.take_requests();
    assert_eq!(requests.len(), 128);
    let mut tickets = requests.iter().map(|q| q.ticket).collect::<Vec<_>>();
    tickets.sort();
    tickets.dedup();
    assert_eq!(tickets.len(), 128);
    for pane in ["crew", "runbook", "rundown"] {
        r.act("showPane", vec![Value::str(pane)]).unwrap();
    }
    r.set_viewport(1280., 844.).unwrap();
    r.act("resetScene", vec![]).unwrap();
    assert_eq!(r.pending().len(), 128);
    assert!(r.take_requests().is_empty());
    assert_eq!(r.derive("waveId"), Some(&Value::Number(7.)));
    let lane = requests
        .iter()
        .find(|q| q.request.url.ends_with("&lane=0"))
        .unwrap();
    assert!(r
        .fulfill(
            lane.ticket,
            Outcome::Response(Response {
                status: 200,
                headers: vec![],
                body: br#"{"wave":7,"lane":0,"value":"wave 7 lane 0"}"#.to_vec(),
            })
        )
        .unwrap()
        .is_some());
    assert_eq!(r.derive("validCount"), Some(&Value::Number(1.)));
    assert_eq!(r.pending().len(), 127);
}

#[test]
fn resize_recomposes_without_resetting_workload_or_local_echo() {
    let mut r = boot(1280.);
    r.act("editDraft", vec![Value::str("Camera B ready")])
        .unwrap();
    r.act("sendDraft", vec![]).unwrap();
    r.act("startLive", vec![]).unwrap();
    r.set_viewport(390., 844.).unwrap();
    r.act("showPane", vec![Value::str("crew")]).unwrap();
    assert_eq!(r.slot("echo"), Some(&Value::str("Camera B ready")));
    assert_eq!(r.slot("count"), Some(&Value::Number(10_000.)));
    assert_eq!(r.kernel().find_by_test_id("crew-input").len(), 1);
}

#[test]
fn all_job_resources_belong_to_the_root_not_a_conditional_pane() {
    let source = include_str!("../../../app.contract");
    let root = source.split("\ncomponent ").nth(1).unwrap();
    let declarations = root.split("\n  view\n").next().unwrap();
    assert_eq!(
        declarations
            .lines()
            .filter(|l| l.trim_start().starts_with("resource r") && l.contains("completion("))
            .count(),
        128
    );
    for line in declarations.lines().filter(|l| l.contains("completion(")) {
        assert!(!line.contains("pane"));
    }
    let mut r = boot(390.);
    for pane in ["crew", "runbook", "rundown"] {
        r.act("showPane", vec![Value::str(pane)]).unwrap();
        assert_eq!(r.derive("waveId"), Some(&Value::Number(0.)));
    }
}
