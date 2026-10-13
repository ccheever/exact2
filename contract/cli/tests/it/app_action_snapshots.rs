//! The apps' actions must use newly computed values at timer boundaries.
use exact_kernel::{Kernel, Offer};
use exact_runner::{Answer, DataError, DataSource, Request, Runner, Store, Value};
use std::path::PathBuf;

struct Replies;

impl DataSource for Replies {
    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        Ok(match self.query(source, args) {
            Ok(value) => Answer::Now(value),
            Err(_) => Answer::Later(Request::get(&format!("https://app.test/{source}"))),
        })
    }

    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        match source {
            "ask" => Ok(Value::record(vec![
                Value::Number(1.0),
                Value::Bool(true),
                Value::str(""),
            ])),
            "command" => Ok(Value::record(vec![
                Value::Number(1.0),
                Value::str("sent"),
                Value::str(""),
            ])),
            _ => Err(DataError::Unavailable(source.into())),
        }
    }
}

// Seed timing and an arriving reply without a network or a wall-clock wait.
// The action under test and the rest of the app remain the authored source.
fn boot(app: &str, setup: &str, bubble: bool) -> Runner<Replies> {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../apps/{app}/app.contract"));
    let source = std::fs::read_to_string(&path).unwrap();
    let mut source = source.replacen("  view\n", &format!("{setup}  view\n"), 1);
    if bubble {
        source = source.replacen(
            "      each entry in stack(nav)",
            "      box id=\"bubble-sent\" width=100 height=40\n      each entry in stack(nav)",
            1,
        );
    }
    let plan = contract::compile_path_source(&path, &source).unwrap();
    Runner::boot(
        plan,
        Replies,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn number(r: &Runner<Replies>, name: &str) -> f64 {
    r.slot(name).and_then(Value::as_number).unwrap()
}

#[test]
fn expose_expires_the_toast_on_the_current_tick() {
    let mut r = boot("expose", "", false);
    r.advance(1000.0).unwrap();
    r.act("nope", vec![Value::str("Camera")]).unwrap();
    r.advance(3000.0).unwrap();
    assert_eq!(number(&r, "toastAt"), 1000.0);
    r.advance(4000.0).unwrap();
    assert_eq!(number(&r, "toastAt"), 0.0);
}

#[test]
fn expose_dates_a_new_banner_now_even_when_the_previous_one_expired() {
    let mut r = boot(
        "expose",
        "  action seed\n    bannerAt = 1\n    send reply = ask(\"expose\", \"hi\", \"\", 0)\n",
        false,
    );
    r.advance(9000.0).unwrap();
    r.act("seed", vec![]).unwrap();
    r.advance(10000.0).unwrap();
    assert_eq!(number(&r, "bannerAt"), 10000.0);
    r.advance(17000.0).unwrap();
    assert_eq!(number(&r, "bannerAt"), 10000.0);
    r.advance(18000.0).unwrap();
    assert_eq!(number(&r, "bannerAt"), 0.0);
}

#[test]
fn video_seeks_on_the_sixth_quiet_tick() {
    let mut r = boot("video-player", "", false);
    r.act("measured", vec![Value::Number(120.0)]).unwrap();
    r.act("forward", vec![]).unwrap();
    r.advance(500.0).unwrap();
    assert_eq!(r.slot("pending"), Some(&Value::Bool(true)));
    assert_eq!(number(&r, "seekTo"), 0.0);
    r.advance(600.0).unwrap();
    assert_eq!(number(&r, "quiet"), 6.0);
    assert_eq!(r.slot("pending"), Some(&Value::Bool(false)));
    assert_eq!(number(&r, "seekTo"), 10.0);
}

#[test]
fn messages_starts_a_late_flight_instead_of_timing_it_out() {
    let mut r = boot(
        "messages",
        "  action seed\n    flightBody = \"hi\"\n    flightPhase = 1\n    flightAt = 1\n    send changed = command(\"send\", \"demo\", \"hi\", 0)\n",
        true,
    );
    r.advance(2100.0).unwrap();
    r.act("seed", vec![]).unwrap();
    let root = r.kernel().roots()[0];
    r.kernel_mut()
        .compute_layout(root, Offer::definite(402.0, 874.0))
        .unwrap();
    r.act("animate", vec![]).unwrap();
    assert_eq!(number(&r, "flightAt"), 2100.0);
    assert_eq!(number(&r, "flightPhase"), 2.0);
    r.advance(2120.0).unwrap();
    assert_eq!(number(&r, "flightPhase"), 3.0);
}
