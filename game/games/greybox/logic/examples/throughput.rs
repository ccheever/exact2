use exact_game::Sim;
use greybox_logic::{Greybox, GreyboxArgs};
fn main() {
    let mut sim = Sim::<Greybox>::new(GreyboxArgs {
        seed: 7,
        paused: false,
        restart: false,
    })
    .unwrap();
    sim.key_down("KeyW");
    let start = std::time::Instant::now();
    sim.run(60000.0);
    let seconds = start.elapsed().as_secs_f64();
    println!(
        "60 s / {seconds:.6} s = {:.1}x; hash=0x{:016x}",
        60.0 / seconds,
        sim.world().hash()
    );
    assert!(60.0 / seconds > 1000.0, "headless throughput below 1000x");
}
