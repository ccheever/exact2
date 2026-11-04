//! The sidebar under 768 CSS px: out of the row, and a sheet the toggle opens (ui/sidebar.tsx's isMobile arm).

use super::composer_accessibility_tests::{measure, Replies};
use super::PLAN;
use exact_apple::measure::CallbackMeasurer;
use exact_plan::Plan;
use exact_runner::Value;

fn host(width: f64) -> exact_apple::Host<Replies> {
    let plan = Plan::decode(PLAN).unwrap();
    let mut replies = Replies::default();
    replies.fixture(&plan, "data", &[(&["revision"], Value::Number(1.0))]);
    let measurer = CallbackMeasurer::new(measure, std::ptr::null_mut(), None);
    let (mut host, _) =
        exact_apple::Host::boot(PLAN, replies, Box::new(measurer), width, 840.0).unwrap();
    host.runner_mut()
        .set_time(1_791_446_400_000.0, 0.0)
        .unwrap();
    host.runner_mut().advance(1250.0).unwrap();
    host.runner_mut().take_commands();
    host
}

fn count(host: &mut exact_apple::Host<Replies>, test_id: &str) -> usize {
    host.runner_mut().kernel().find_by_test_id(test_id).len()
}

#[test]
fn a_narrow_window_moves_the_sidebar_into_a_sheet_the_toggle_opens() {
    let mut host = host(760.0);
    assert_eq!(count(&mut host, "sidebar"), 0, "no sidebar in the row");
    assert_eq!(count(&mut host, "sidebar-sheet"), 0, "the sheet starts closed");

    let runner = host.runner_mut();
    runner
        .act("inlineDocumentShortcut", vec![Value::str("sidebar.toggle")])
        .unwrap();
    assert!(runner.land_then().error.is_none());
    assert_eq!(runner.slot("sidebarSheetOpen"), Some(&Value::Bool(true)));
    runner.advance(1600.0).unwrap();
    assert_eq!(count(&mut host, "sidebar-sheet"), 1);
    assert_eq!(count(&mut host, "sidebar"), 1);
    assert_eq!(count(&mut host, "sidebar-sheet-trigger"), 1);
    assert_eq!(count(&mut host, "sidebar-resize"), 0, "the sheet's sidebar cannot resize");

    let runner = host.runner_mut();
    runner
        .act("inlineDocumentShortcut", vec![Value::str("sidebar.toggle")])
        .unwrap();
    assert!(runner.land_then().error.is_none());
    runner.advance(2000.0).unwrap();
    assert_eq!(count(&mut host, "sidebar-sheet"), 0);
    assert_eq!(count(&mut host, "sidebar"), 0);
}

#[test]
fn a_wide_window_keeps_the_sidebar_in_the_row_and_no_sheet() {
    let mut host = host(1280.0);
    assert_eq!(count(&mut host, "sidebar"), 1);
    let runner = host.runner_mut();
    runner.act("sidebarSheetSet", vec![Value::Bool(true)]).unwrap();
    assert!(runner.land_then().error.is_none());
    runner.advance(1600.0).unwrap();
    assert_eq!(count(&mut host, "sidebar-sheet"), 0);
    assert_eq!(count(&mut host, "sidebar"), 1);
}
