//! The compiled plan asks for its snapshot with
//! `exactViewport().displayMode`, so the title rows' inset and the sidebar minimum
//! (presentation.ts, sidebar-min-width.ts) follow the window into full screen and
//! back, with no native full-screen fact (T3FullScreen.swift is gone). The data
//! module is a stand-in that records each `snapshot` request and leaves it pending.

use super::composer_accessibility_tests::measure;
use super::PLAN;
use exact_apple::measure::CallbackMeasurer;
use exact_runner::{Answer, DataError, DataSource, Preferences, Request, Store, Value};

#[derive(Default)]
struct Snapshots {
    asked: Vec<Vec<Value>>,
}

impl DataSource for Snapshots {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.to_string()))
    }

    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        if source == "snapshot" {
            self.asked.push(args.to_vec());
        }
        Ok(Answer::Later(Request::get("https://fixture.invalid/")))
    }
}

/// The display mode (the fourth argument) of the newest snapshot request.
fn mode(host: &mut exact_apple::Host<Snapshots>) -> Value {
    let asked = &host.runner_mut().data().asked;
    asked.last().expect("a snapshot request")[3].clone()
}

#[test]
fn the_snapshot_request_follows_the_viewport_display_mode() {
    let measurer = CallbackMeasurer::new(measure, std::ptr::null_mut(), None);
    let (mut host, _) = exact_apple::Host::boot(
        PLAN,
        Snapshots::default(),
        Box::new(measurer),
        1280.0,
        840.0,
    )
    .unwrap();
    host.runner_mut()
        .set_time(1_791_446_400_000.0, 0.0)
        .unwrap();
    host.runner_mut().advance(1250.0).unwrap();
    assert_eq!(mode(&mut host), Value::str("standalone"));
    // LLP 1039: display mode 2 is `fullscreen` (the host's NSWindow full-screen state).
    host.set_preferences(Preferences::from_bits(2 << 11));
    assert_eq!(mode(&mut host), Value::str("fullscreen"));
    host.set_preferences(Preferences::from_bits(0));
    assert_eq!(mode(&mut host), Value::str("standalone"));
}
