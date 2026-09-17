//! Fresh-process save proof, using the same game as the GPU module.
use beacons_logic::Beacons;
use exact_game::{Clock, InputEvent, Sim, Value};
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = std::path::Path::new(&args[2]);
    let mut sim =
        Sim::<Beacons>::new(&[Value::Number(7.0), Value::Bool(false), Value::Number(0.0)]).unwrap();
    sim.advance(0.0, Clock::Seekable);
    if args[1] == "original" {
        for (code, down, at_ms) in [
            ("KeyW", true, 0.0),
            ("Space", true, 1011.0),
            ("Space", false, 1012.0),
        ] {
            sim.input(InputEvent::Key {
                code: code.into(),
                down,
                at_ms,
            });
        }
        sim.advance(713.123, Clock::Seekable);
        std::fs::write(dir.join("midrun.world"), sim.save()).unwrap();
        sim.advance(2000.0, Clock::Seekable);
    } else {
        sim.restore(&std::fs::read(dir.join("midrun.world")).unwrap())
            .unwrap();
        sim.advance(0.0, Clock::Seekable);
        sim.advance(2000.0 - 713.123, Clock::Seekable);
    }
    std::fs::write(dir.join(format!("{}.world", args[1])), sim.save()).unwrap();
    println!(
        "{} pid={} hash=0x{:016x}",
        args[1],
        std::process::id(),
        sim.world().hash()
    );
}
