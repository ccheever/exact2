use exact_game::{Clock, InputEvent, Sim, Value};
use greybox_logic::Greybox;
fn main() {
    let mut sim = Sim::<Greybox>::new(&[Value::Number(7.0), Value::Bool(false)]).unwrap();
    sim.advance(0.0, Clock::Seekable);
    sim.input(InputEvent::Key {
        code: "KeyW".into(),
        down: true,
        at_ms: 0.0,
    });
    let start = std::time::Instant::now();
    sim.advance(60000.0, Clock::Seekable);
    let seconds = start.elapsed().as_secs_f64();
    println!(
        "60 s / {seconds:.6} s = {:.1}x; hash=0x{:016x}",
        60.0 / seconds,
        sim.world().hash()
    );
    assert!(60.0 / seconds > 1000.0, "headless throughput below 1000x");
}
