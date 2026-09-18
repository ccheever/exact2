// Shared by consumer tests: the same script must survive every-tick reconstruction.
use exact_game::{Game, Paranoid, Sim};
use std::time::Instant;

pub fn compare<G: Game>(create: impl Fn() -> Sim<G>, script: impl Fn(&mut Sim<G>)) {
    let mut expected: Option<(u64, u64, Vec<u8>)> = None;
    let mut normal = 0.0;
    let mut failures = Vec::new();
    for mode in [Paranoid::Off, Paranoid::Save, Paranoid::FreshGame] {
        let mut sim = create().paranoid(mode);
        let start = Instant::now();
        script(&mut sim);
        let elapsed = start.elapsed().as_secs_f64();
        let state = (sim.world().hash(), sim.world().tick(), sim.save());
        if let Some(expected) = &expected {
            // EXSIM includes the complete published record, journal/cursor,
            // physics snapshot, held input and future queue, beyond the hash.
            if expected != &state {
                failures.push(format!("{mode:?}: expected hash 0x{:016x} tick {}, got 0x{:016x} tick {}; first save difference {:?}",
                    expected.0, expected.1, state.0, state.1,
                    expected.2.iter().zip(&state.2).position(|(a, b)| a != b)));
            }
        } else {
            expected = Some(state);
            normal = elapsed;
        }
        println!(
            "PARANOID {} {mode:?}: {:.3} ms, {:.2}x normal",
            G::ID,
            elapsed * 1000.0,
            elapsed / normal
        );
    }
    assert!(failures.is_empty(), "{}: {}", G::ID, failures.join("; "));
}
