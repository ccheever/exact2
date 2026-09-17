//! Controller ownership/publication checks. Full app/painter barrier is a
//! separate integration proof; these tests do not substitute fake Ready metrics.
use super::*;

#[test]
fn singleton_admission_refuses_a_second_live_region_before_any_font_snapshot() {
    let _test = crate::content_region::test_service();
    let first = Controller::admit().unwrap();
    assert!(matches!(first.phase(), Phase::AwaitingFirstPaint));
    assert!(Controller::admit().is_err());
    assert_eq!(
        first.counts(),
        worker::Counts {
            running: 0,
            pending: 0,
            completed: 0
        }
    );
    drop(first);
    // Closure/factory and actual session-owner drops are bounded background work.
    let end = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while SERVICE.occupied() {
        assert!(std::time::Instant::now() < end);
        std::thread::yield_now();
    }
    let next = Controller::admit().unwrap();
    drop(next);
    while SERVICE.occupied() {
        assert!(std::time::Instant::now() < end);
        std::thread::yield_now();
    }
}

#[test]
fn explicit_refusal_retires_the_service_even_while_ui_keeps_old_controller() {
    let _test = crate::content_region::test_service();
    let mut old = Controller::admit().unwrap();
    assert!(old
        .record_refusal(Err("injected explicit refusal".into()))
        .is_err());
    assert!(
        old.port.is_closed(),
        "refused UI controller must release actual idle service"
    );
    let end = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while SERVICE.occupied() {
        assert!(std::time::Instant::now() < end);
        std::thread::yield_now();
    }
    let next = Controller::admit().unwrap();
    assert!(matches!(old.phase(), Phase::Refused(_)));
    drop(next);
    while SERVICE.occupied() {
        assert!(std::time::Instant::now() < end);
        std::thread::yield_now();
    }
}
