//! Actual presenter failure fallback and physical dispatch over retained pixels.
use super::*;
use crate::content_region::ContentRegionRegistration;
use exact_runner::{DataError, Value};
use std::time::Instant;
#[path = "fail_backend.rs"]
mod fail_backend;
struct Empty;
impl DataSource for Empty {
    fn query(&mut self, n: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(n.into()))
    }
}
const APP: &str = r#"component App
  state title = "old picture"
  state link = "https://old.example/"
  state count = 0
  state draft = ""
  state showing = true
  action replace writes title, link
    title = "new source with changed action semantics"
    link = "https://new.example/"
  action replaceAgain writes title, link
    title = "C newest source"
    link = "https://third.example/"
  action hide writes showing
    showing = false
  action activate writes count
    count = title == "old picture" ? 1 : 2
  action edit(value) writes draft
    draft = value
  view
    column width=400 height=500
      button press=replace testId="replace" height=32
        text "replace"
      button press=replaceAgain testId="replace-again" height=20
        text "replace again"
      button press=hide testId="hide" height=20
        text "hide"
      input value=draft change=edit testId="input" height=32
      text `${count}` testId="count" height=24
      when showing
        view id="owner" width=400 height=200 overflow-x="hidden" overflow-y="hidden"
          scroll id="content" testId="scroll" width=400 height=200
            view height=600
              text title href=link press=activate testId="paragraph" font-size=20 color="light-dark(#ff0000,#0000ff)"
          text "Preparing content" id="pending" position="absolute"
"#;
fn id(p: &Presenter<Empty>, name: &str) -> ViewId {
    p.host
        .kernel()
        .node_by_key(p.host.kernel().find_by_test_id(name)[0])
        .unwrap()
        .id
}
fn ready(p: &mut Presenter<Empty>) {
    let end = Instant::now() + Duration::from_secs(90);
    while !p.host.content_region().unwrap().receipt().unwrap().current {
        assert!(Instant::now() < end, "font/publication watchdog");
        assert!(p.host.content_region().unwrap().refusal().is_none());
        assert!(p.content_region_fd().is_some());
        // Test-only wait. Production watches the fd alongside physical input.
        std::thread::sleep(Duration::from_millis(1));
        assert!(p.poll_content_region().is_none());
    }
}
#[test]
fn non_cpu_region_refuses_before_plan_font_or_device_work() {
    for choice in [PainterChoice::Auto, PainterChoice::Gpu] {
        let result = Presenter::boot_with_content_region(
            &[],
            Empty,
            (400., 500.),
            1.,
            PathBuf::from("/nonexistent"),
            choice,
            ContentRegionRegistration {
                activate: None,
                owner: "o",
                content: "c",
                pending: "p",
            },
        );
        assert!(
            matches!(result, Err(HostError::Painter(ref e)) if e.contains("CPU")),
            "trial must refuse this painter before attempting even plan decoding"
        );
    }
}
#[test]
fn failed_frame_retains_source_hits_and_rejects_live_replacement_actions() {
    let _service = crate::content_region::test_service();
    let (mut p, error) = Presenter::boot_with_content_region(
        &contract::compile(APP).unwrap().encode(),
        Empty,
        (400., 500.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
        ContentRegionRegistration {
            activate: None,
            owner: "owner",
            content: "content",
            pending: "pending",
        },
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.frame();
    let input = id(&p, "input");
    p.type_text(input, "external control remains live").unwrap();
    ready(&mut p);
    let first = p.frame();
    let paragraph = id(&p, "paragraph");
    let key = p.host.kernel().node(paragraph).unwrap().key;
    let b = p.box_of(paragraph).unwrap();
    let point = (b.rect.0 + 2., b.rect.1 + 2.);
    assert_eq!(p.hit(point.0, point.1), Some(paragraph));
    let old_stamp = p
        .host
        .content_region()
        .unwrap()
        .text_snapshot(key)
        .unwrap()
        .request
        .stamp()
        .clone();
    let replace = id(&p, "replace");
    assert!(p.host.dispatch_at(replace, Event::Press, 1.).is_none());
    assert!(p.after_commit().is_none());
    ready(&mut p); // B is layout-ready, but has never succeeded in the backend.
    p.brush.replace_backend(Box::new(fail_backend::Failure));
    let failed = p.frame();
    assert!(!p.last_frame_succeeded);
    assert_eq!(first.data(), failed.data());
    let snapshot = p.host.content_region().unwrap().text_snapshot(key).unwrap();
    assert_eq!(snapshot.request.stamp(), &old_stamp);
    assert_eq!(
        snapshot.runs[0].link.as_ref().unwrap().1.as_ref(),
        "https://old.example/"
    );
    assert!(!snapshot.current);
    assert_eq!(p.handler_target(paragraph, EventKind::Press), None);
    assert_eq!(p.press_at(point.0, point.1, 2.), None);
    assert_eq!(
        p.host
            .kernel()
            .node(id(&p, "count"))
            .unwrap()
            .props
            .str(PropId::Text),
        Some("0")
    );
    assert!(
        !p.pointer_down(point.0, point.1, 2.).unwrap(),
        "old hit must not acquire a new content gesture"
    );
    assert!(p.collection_interaction().is_none());
    let port = id(&p, "scroll");
    p.wheel_at(point.0, point.1, 0., 30.);
    assert_eq!(p.scroll_of(port).1, 30., "read-only content still scrolls");
    assert!(p.resize(480., 520.).is_none());
    let resized_failure = p.frame();
    assert_eq!(
        (resized_failure.width(), resized_failure.height()),
        (480, 520)
    );
    for y in 0..first.height() as usize {
        assert_eq!(
            &resized_failure.data()[y * 480 * 4..][..400 * 4],
            &first.data()[y * 400 * 4..][..400 * 4]
        );
    }
    assert!(resized_failure.data()[500 * 480 * 4..]
        .iter()
        .all(|v| *v == 255));
    p.brush.replace_backend(Box::new(Raster::new()));
    p.frame();
    assert!(p.last_frame_succeeded);
    let snapshot = p.host.content_region().unwrap().text_snapshot(key).unwrap();
    assert_eq!(
        snapshot.runs[0].link.as_ref().unwrap().1.as_ref(),
        "https://new.example/"
    );
    assert!(snapshot.current);
    p.scroll.insert(port, (0., 0.));
    let light = p.frame();
    let shape_calls = p.text.borrow().shape_calls;
    p.brush.dark = true;
    assert!(p.host.content_region_appearance(true).is_none());
    let dark = p.frame();
    assert!(
        p.last_frame_succeeded,
        "appearance-only receipt must not strand reused artifacts"
    );
    assert_ne!(light.data(), dark.data());
    assert_eq!(
        p.text.borrow().shape_calls,
        shape_calls,
        "palette resolution cannot shape text"
    );
    assert!(p.host.content_region().unwrap().publication_painted());
    drop(p);
}

#[test]
fn held_old_source_latest_demand_and_destroy_retire_without_publishing_stale_pixels() {
    let _service = crate::content_region::test_service();
    let (mut p, error) = Presenter::boot_with_content_region(
        &contract::compile(APP).unwrap().encode(),
        Empty,
        (400., 500.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
        ContentRegionRegistration {
            activate: None,
            owner: "owner",
            content: "content",
            pending: "pending",
        },
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.frame();
    ready(&mut p);
    let original = p.frame();
    let key = p.host.kernel().node(id(&p, "paragraph")).unwrap().key;
    let original_stamp = p
        .host
        .content_region()
        .unwrap()
        .text_snapshot(key)
        .unwrap()
        .request
        .stamp()
        .clone();
    let gate = crate::content_region::test_hooks::next_text();
    assert!(p
        .host
        .dispatch_at(id(&p, "replace"), Event::Press, 1.)
        .is_none());
    assert!(p.after_commit().is_none());
    gate.entered();
    assert_eq!(p.host.content_region().unwrap().work_counts().0, 1);
    assert!(p
        .host
        .dispatch_at(id(&p, "replace-again"), Event::Press, 2.)
        .is_none());
    assert!(p.after_commit().is_none());
    let held = p.frame();
    assert_eq!(
        original.data(),
        held.data(),
        "B/C pending must retain successful A"
    );
    let snapshot = p.host.content_region().unwrap().text_snapshot(key).unwrap();
    assert_eq!(snapshot.request.stamp(), &original_stamp);
    assert!(!snapshot.current);
    let counts = p.host.content_region().unwrap().work_counts();
    assert_eq!(counts.0, 1);
    assert_eq!(counts.1, 1);
    assert_eq!(counts.2, 0);
    drop(gate);
    ready(&mut p);
    p.frame();
    let snapshot = p.host.content_region().unwrap().text_snapshot(key).unwrap();
    assert_eq!(
        snapshot.runs[0].link.as_ref().unwrap().1.as_ref(),
        "https://third.example/"
    );
    assert!(snapshot.current);
    // Now destroy the owner while a real old job is indivisible/parked.
    let gate = crate::content_region::test_hooks::next_text();
    assert!(p
        .host
        .dispatch_at(id(&p, "replace"), Event::Press, 3.)
        .is_none());
    assert!(p.after_commit().is_none());
    gate.entered();
    assert!(p
        .host
        .dispatch_at(id(&p, "hide"), Event::Press, 4.)
        .is_some());
    p.after_commit();
    assert!(
        p.host.content_region().unwrap().refusal().is_some(),
        "destroyed registration must explicitly retire"
    );
    assert!(p.content_region_fd().is_none());
    assert_eq!(
        p.host.content_region().unwrap().work_counts().0,
        1,
        "cancellation cannot refund running native allocations"
    );
    drop(gate);
    let end = Instant::now() + Duration::from_secs(10);
    while p.host.content_region().unwrap().work_counts().0 != 0 {
        assert!(Instant::now() < end);
        std::thread::yield_now();
    }
    assert!(!p.host.poll_content_region().unwrap());
    assert_eq!(
        p.host
            .content_region()
            .unwrap()
            .text_snapshot(key)
            .unwrap()
            .runs[0]
            .link
            .as_ref()
            .unwrap()
            .1
            .as_ref(),
        "https://third.example/"
    );
}

#[test]
fn exact_scale_prepared_index_refuses_changed_dpr_before_stale_adoption_or_replay() {
    let _service = crate::content_region::test_service();
    let (mut p, error) = Presenter::boot_with_content_region(
        &contract::compile(APP).unwrap().encode(),
        Empty,
        (400., 500.),
        1.25,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
        ContentRegionRegistration {
            activate: None,
            owner: "owner",
            content: "content",
            pending: "pending",
        },
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.frame();
    ready(&mut p);
    let exact_kernel::RegionSelection::Accepted(a) = &p
        .host
        .content_region()
        .unwrap()
        .receipt()
        .unwrap()
        .selection
    else {
        panic!("current")
    };
    for artifact in a.artifacts() {
        let n = artifact
            .payload::<crate::content_region::NativeText>()
            .unwrap();
        assert_eq!(n.paint_context().scale().to_bits(), 1.25f32.to_bits());
        if let Some(paragraph) = n.paragraph() {
            assert!(
                paragraph.ink_capacity_bytes() > 0,
                "index exists BEFORE first paint"
            );
        }
    }
    let first = p.frame();
    let key = p.host.kernel().node(id(&p, "paragraph")).unwrap().key;
    let stamp = p
        .host
        .content_region()
        .unwrap()
        .text_snapshot(key)
        .unwrap()
        .request
        .stamp()
        .clone();
    let gate = crate::content_region::test_hooks::next_text();
    assert!(p
        .host
        .dispatch_at(id(&p, "replace"), Event::Press, 1.)
        .is_none());
    p.after_commit();
    gate.entered();
    // Actual native DPI is fixed at boot today. Emulate a carrier reporting a
    // different scale to exercise the explicit retirement boundary.
    p.brush.scale = 2.;
    let retained = p.frame();
    assert!(
        p.host.content_region().unwrap().refusal().is_some(),
        "DPR must not replay/build a differently scaled index on UI"
    );
    assert_eq!((retained.width(), retained.height()), (800, 1000));
    for y in 0..first.height() as usize {
        assert_eq!(
            &retained.data()[y * 800 * 4..][..500 * 4],
            &first.data()[y * 500 * 4..][..500 * 4]
        );
    }
    assert!(p.content_region_fd().is_none());
    let old = p.host.content_region().unwrap().text_snapshot(key).unwrap();
    assert_eq!(old.request.stamp(), &stamp);
    assert!(!old.current);
    assert!(
        p.boxes.is_empty(),
        "old-DPR hits cannot route at new coordinates"
    );
    drop(gate);
    let end = Instant::now() + Duration::from_secs(10);
    while p.host.content_region().unwrap().work_counts().0 != 0 {
        assert!(Instant::now() < end);
        std::thread::yield_now();
    }
    assert!(!p.host.poll_content_region().unwrap());
    assert_eq!(
        p.host
            .content_region()
            .unwrap()
            .text_snapshot(key)
            .unwrap()
            .request
            .stamp(),
        &stamp
    );
}

#[test]
fn malformed_region_scale_refuses_before_plan_fonts_or_worker_admission() {
    for scale in [0., -1., f32::NAN, f32::INFINITY] {
        let result = Presenter::boot_with_content_region(
            &[],
            Empty,
            (400., 500.),
            scale,
            PathBuf::from("/nonexistent"),
            PainterChoice::Cpu,
            ContentRegionRegistration {
                activate: None,
                owner: "owner",
                content: "content",
                pending: "pending",
            },
        );
        assert!(
            matches!(result, Err(HostError::Painter(ref e)) if e.contains("content raster context"))
        );
    }
}

#[test]
fn unsupported_region_query_preserves_previous_pixels_and_recovers() {
    let _service = crate::content_region::test_service();
    let app = APP.replace("state showing = true", "state showing = true\n  state ownerScale = 1\n  action collapse writes ownerScale\n    ownerScale = 0\n  action restore writes ownerScale\n    ownerScale = 1")
        .replace("view id=\"owner\"", "view id=\"owner\" scale=ownerScale")
        .replace("button press=hide", "button press=collapse testId=\"collapse\" height=20\n        text \"collapse\"\n      button press=restore testId=\"restore\" height=20\n        text \"restore\"\n      button press=hide");
    let (mut p, error) = Presenter::boot_with_content_region(
        &contract::compile(&app).unwrap().encode(),
        Empty,
        (400., 500.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
        ContentRegionRegistration {
            activate: None,
            owner: "owner",
            content: "content",
            pending: "pending",
        },
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.frame();
    ready(&mut p);
    let first = p.frame();
    assert!(p
        .host
        .dispatch_at(id(&p, "collapse"), Event::Press, 1.)
        .is_none());
    p.after_commit();
    let refused = p.frame();
    assert!(
        !p.last_frame_succeeded,
        "singular region query must refuse, not visit every glyph"
    );
    assert_eq!(refused.data(), first.data());
    assert!(p
        .host
        .dispatch_at(id(&p, "restore"), Event::Press, 2.)
        .is_none());
    p.after_commit();
    assert_eq!(p.frame().data(), first.data());
    assert!(p.last_frame_succeeded);
}

#[test]
fn uncertain_ink_coordinates_refuse_full_glyph_fallback_and_recover() {
    let _service = crate::content_region::test_service();
    let app = APP.replace("state showing = true", "state showing = true\n  state inset = 0\n  action move writes inset\n    inset = 20000000\n  action restore writes inset\n    inset = 0")
        .replace("view height=600", "view height=600 padding-top=inset")
        .replace("button press=hide", "button press=move testId=\"move\" height=20\n        text \"move\"\n      button press=restore testId=\"restore\" height=20\n        text \"restore\"\n      button press=hide");
    let (mut p, error) = Presenter::boot_with_content_region(
        &contract::compile(&app).unwrap().encode(),
        Empty,
        (400., 500.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
        ContentRegionRegistration {
            activate: None,
            owner: "owner",
            content: "content",
            pending: "pending",
        },
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.frame();
    ready(&mut p);
    let first = p.frame();
    assert!(p
        .host
        .dispatch_at(id(&p, "move"), Event::Press, 1.)
        .is_none());
    p.after_commit();
    ready(&mut p);
    let refused = p.frame();
    assert!(
        !p.last_frame_succeeded,
        "uncertain ink viewport cannot enter full glyph paint"
    );
    assert_eq!(refused.data(), first.data());
    assert!(p
        .host
        .dispatch_at(id(&p, "restore"), Event::Press, 2.)
        .is_none());
    p.after_commit();
    ready(&mut p);
    assert_eq!(p.frame().data(), first.data());
    assert!(p.last_frame_succeeded);
}
