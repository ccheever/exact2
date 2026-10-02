//! LLP 1072 §8.5: where canvas draws are deferred, the turns main waits on
//! lay canvases out and say a draw is owed; `canvas_draw` runs it, and a
//! frame's tick draws in its own turn.
use exact_apple::Host;
use exact_kernel::MonospaceMeasurer;
use exact_runner::exact_canvas::{Context2d, DrawError, Frame};
use exact_runner::{DataError, DataSource, Event, Value};

/// One 2D surface, `dot(n)`: a square `n` points in, asking for the next
/// frame while `n` is odd.
struct Dots;

impl DataSource for Dots {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
    fn canvas_surfaces(&self) -> Vec<(String, usize)> {
        vec![("dot".into(), 1)]
    }
    fn draw_2d(
        &mut self,
        _: &str,
        args: &[Value],
        ctx: &Context2d,
        _: &Frame,
    ) -> Result<bool, DrawError> {
        let n = match args.first() {
            Some(Value::Number(n)) => *n,
            _ => 0.0,
        };
        ctx.set_fill_style_str("#336699");
        ctx.fill_rect(n, n, 4.0, 4.0);
        Ok(n % 2.0 == 1.0)
    }
}

const APP: &str = r#"component App
  state n = 2
  action step
    n = n + 1
  view
    column
      button press=step testId="step"
        text "Step"
      canvas surface=dot(n) width=40 height=40 testId="dot"
"#;

fn boot() -> Host<Dots> {
    let plan = contract::compile(APP).unwrap();
    let (host, first) = Host::boot(
        &plan.encode(),
        Dots,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    assert!(first.contains("\"op\":\"canvas2d\""), "boot draws: {first}");
    host
}

fn step<D: DataSource>(host: &mut Host<D>, now: f64) -> String {
    let k = host.runner().kernel();
    let key = k.find_by_test_id("step")[0];
    let id = k.node_by_key(key).unwrap().id;
    host.dispatch_at(id, Event::Press, now)
}

fn draws(batch: &str) -> usize {
    batch.matches("\"op\":\"canvas2d\"").count()
}

#[test]
fn a_deferred_draw_is_owed_by_the_turn_and_run_by_its_own() {
    let mut host = boot();
    host.set_canvas_deferred(true);
    let turn = step(&mut host, 10.0);
    assert_eq!(
        draws(&turn),
        0,
        "the turn main waits on draws nothing: {turn}"
    );
    assert!(turn.contains("\"canvasOwed\":true"), "{turn}");
    let drawn = host.canvas_draw();
    assert_eq!(draws(&drawn), 1, "{drawn}");
    assert!(
        !drawn.contains("canvasOwed"),
        "nothing is owed after it: {drawn}"
    );
    assert!(
        drawn.contains("\"canvas\":true"),
        "n = 3 asks for the next frame: {drawn}"
    );
    // Nothing owed: a draw turn draws nothing.
    assert_eq!(draws(&host.canvas_draw()), 0);
}

#[test]
fn a_tick_draws_a_frame_request_in_its_own_turn() {
    let mut host = boot();
    host.set_canvas_deferred(true);
    step(&mut host, 10.0);
    host.canvas_draw();
    let tick = host.tick(30.0);
    assert_eq!(
        draws(&tick),
        1,
        "the frame request is drawn by the tick: {tick}"
    );
    assert!(!tick.contains("canvasOwed"), "{tick}");
}

#[test]
fn undeferred_turns_draw_as_before() {
    let mut host = boot();
    let turn = step(&mut host, 10.0);
    assert_eq!(draws(&turn), 1, "{turn}");
    assert!(!turn.contains("canvasOwed"), "{turn}");
}
