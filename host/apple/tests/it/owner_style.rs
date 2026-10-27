//! A content owner's container units (LLP 1123 D12) in a native style: the
//! presenter is sent them resolved by the owner, as layout resolves them,
//! not by the window, on every path that sends a style.

use crate::host::{view, NoData};
use exact_apple::{Claims, Host};
use exact_kernel::{MonospaceMeasurer, OwnerValues};
use exact_runner::Event;

const PAGE: &str = r##"component App
  view
    column testId="owner" width="100%" height="100%"
      box testId="box" width=40 min-height="10cqh"
"##;

/// The style the batch last sends `id`.
fn style_of(batch: &str, id: u32) -> serde_json::Value {
    let batch: serde_json::Value = serde_json::from_str(batch).unwrap();
    let ops = batch["ops"].as_array().unwrap();
    let op = ops
        .iter()
        .rev()
        .find(|o| o["op"] == "style" && o["id"] == id);
    op.map_or(serde_json::Value::Null, |o| o["style"].clone())
}

/// `src` booted in an 800 × 600 window, and the batch that gives the view
/// `owner` a 400 × 300 content root.
fn boot_owned(src: &str) -> (Host<NoData>, String) {
    let plan = contract::compile(src).unwrap();
    let measurer = Box::new(MonospaceMeasurer::default());
    let (mut host, _) = Host::boot(&plan.encode(), NoData, measurer, 800.0, 600.0).unwrap();
    let values = OwnerValues {
        safe: [0.0; 4],
        regions: None,
        cols: 1,
        rows: 1,
        segments: vec![],
        size: (400.0, 300.0),
    };
    let batch = host.set_claims(&Claims {
        owners: &[(view(&host, "owner"), Some(values))],
        ..Claims::default()
    });
    (host, batch)
}

#[test]
fn an_owners_container_unit_reaches_the_presenter_as_the_owner_resolves_it() {
    let (host, batch) = boot_owned(PAGE);
    let boxed = view(&host, "box");
    assert_eq!(
        style_of(&batch, boxed)["min_height"],
        30.0,
        "the owner's 300, not the window's 600"
    );
}

const MOVING: &str = r##"component App
  state on = false
  action toggle
    on = not on
  view
    column testId="owner" width="100%" height="100%"
      button press=toggle testId="toggle"
        text "Go"
      box testId="card" width=40 height=40 border-radius="10cqh" color=(on ? "#ffffff" : "#000000") background-color=(on ? "#0000ff" : "#ff0000") transition="background-color 1s linear, color 1s linear"
        box testId="inner" width=20 height=20 border-radius="10cqh"
      box testId="face" width="10cqh" height="10cqh" background-color="#00ff00"
        text "AB"
      text testId="para"
        text "plain "
        text "run" testId="run" border-radius="10cqh"
"##;

/// A face's size and an inline run's radius cross as the owner resolves
/// them, and so do a box's radius and its inheriting child's on a frame of
/// paint motion and when the motion arrives: 10% of 300, not of 600.
#[test]
fn paint_motion_faces_and_runs_keep_the_owners_container_units() {
    let (mut host, first) = boot_owned(MOVING);
    let (toggle, card, inner) = (
        view(&host, "toggle"),
        view(&host, "card"),
        view(&host, "inner"),
    );
    let (para, run) = (view(&host, "para"), view(&host, "run"));
    let radius = |batch: &str, id| style_of(batch, id)["border_radius_top_left"].clone();
    assert_eq!(radius(&first, card), 30.0, "{first}");
    assert!(first.contains("\"faceBoxSize\":\"30x30\""), "{first}");
    let paragraph = |batch: &str| {
        let batch: serde_json::Value = serde_json::from_str(batch).unwrap();
        let ops = batch["ops"].as_array().unwrap().clone();
        let op = ops
            .into_iter()
            .rev()
            .find(|o| o["op"] == "paragraph" && o["id"] == para);
        let runs = op.map(|o| o["runs"].as_array().unwrap().clone());
        let run = runs.and_then(|r| r.into_iter().find(|r| r["id"] == run));
        run.map_or(serde_json::Value::Null, |r| {
            r["style"]["border_radius_top_left"].clone()
        })
    };
    assert_eq!(paragraph(&first), 30.0, "{first}");
    host.dispatch_at(toggle, Event::Press, 0.0);
    let mid = host.tick(500.0);
    assert!(style_of(&mid, card)["background_color"].is_array(), "{mid}");
    assert_eq!(radius(&mid, card), 30.0, "{mid}");
    assert_eq!(radius(&mid, inner), 30.0, "{mid}");
    let done = host.tick(1000.0);
    assert_eq!(radius(&done, card), 30.0, "{done}");
    assert_eq!(radius(&done, inner), 30.0, "{done}");
}
