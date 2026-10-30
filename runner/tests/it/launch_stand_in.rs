//! A build-time answer shown for other arguments is a stand-in (LLP 1048.003
//! D6): a launch whose persisted state changed a resource's arguments shows
//! the bake's answer for the bake's arguments until its source is ready, and
//! `pending(x)` is true until then. A task gated on `not pending(x)` acts on
//! the answer for the persisted arguments, not on the stand-in: the
//! timetable's open-at-now jump landed on the bake's station's row.
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{Answer, DataError, DataSource, Runner, Store};

const SOURCE: &str = r##"shape Departure
  id: string
component App
  state station = "Harbor" persist
  state jumped = ""
  resource board = board(station) as shape Departure[]
  task reveal when not pending(board) key=station
    after(1, jump)
  action jump
    match first(board)
      case some(d)
        jumped = d.id
      case none
        jumped = "none"
  view
    text jumped
"##;

struct Boards {
    ready: bool,
}

impl DataSource for Boards {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }

    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        if source != "board" {
            return Err(DataError::UnknownSource(source.into()));
        }
        let Some(station) = args.first().and_then(Value::as_str) else {
            return Err(DataError::Unavailable(format!("{args:?}")));
        };
        let id = Value::str(&format!("{station} 05:10"));
        Ok(Answer::Now(Value::list(vec![Value::record(vec![id])])))
    }

    fn ready(&self) -> bool {
        self.ready
    }
}

fn launch(stored: Option<&str>) -> Runner<Boards> {
    let plan = contract::bake(contract::compile(SOURCE).unwrap(), Boards { ready: true }).unwrap();
    let snapshot = stored
        .map(|s| {
            vec![(
                "exact.state.station".into(),
                format!("\"{s}\"").into_bytes(),
            )]
        })
        .unwrap_or_default();
    Runner::boot_stored(
        plan,
        Boards { ready: false },
        Kernel::with_monospace(),
        snapshot,
        Default::default(),
        "/",
    )
    .unwrap()
}

#[test]
fn a_task_waiting_on_pending_acts_on_the_persisted_arguments_answer() {
    let mut r = launch(Some("Mission"));
    assert_eq!(r.slot("station"), Some(&Value::str("Mission")));
    // The first frame is the bake's answer, for Harbor, pending: the gate
    // stays shut past the task's delay while the module is not ready.
    assert_eq!(
        r.resource("board"),
        Some(&Value::list(vec![Value::record(vec![Value::str(
            "Harbor 05:10"
        )])]))
    );
    let now = r.now_ms();
    r.advance(now + 50.).unwrap();
    assert_eq!(r.slot("jumped"), Some(&Value::str("")));
    r.data().ready = true;
    r.data_ready().unwrap();
    r.advance(now + 100.).unwrap();
    assert_eq!(r.slot("jumped"), Some(&Value::str("Mission 05:10")));
}

#[test]
fn a_build_time_answer_for_the_launch_arguments_is_not_pending() {
    // The fast first paint: a fresh install's arguments are the bake's, so
    // the task acts on the build-time answer without waiting for the module.
    let mut r = launch(None);
    let now = r.now_ms();
    r.advance(now + 50.).unwrap();
    assert_eq!(r.slot("jumped"), Some(&Value::str("Harbor 05:10")));
}
