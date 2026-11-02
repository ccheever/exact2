//! Engine-owned deterministic state card; no host or GPU needed.
mod fixture;
use exact_game::{Game, Sim};
fn card<G: Game>(args: G::Args) {
    let mut s = Sim::<G>::new(args).unwrap();
    println!(
        "{} setup=0x{:016x} save_bytes={}",
        G::ID,
        s.world().hash(),
        s.save().unwrap().len()
    );
    for (name, request) in [
        ("tree", r#"{"op":"tree","world":true}"#),
        ("state", r#"{"op":"state"}"#),
        ("player", r#"{"op":"state","entity":"player"}"#),
        (
            "layout",
            r#"{"op":"layout","entity":"player","width":800,"height":600,"scale":2}"#,
        ),
    ] {
        println!("{} {name}={}", G::ID, s.agent(request));
    }
    s.key_down("KeyW");
    s.run(1500.0);
    println!("{} W1500=0x{:016x}", G::ID, s.world().hash());
}
fn main() {
    println!("{} {}", std::env::consts::ARCH, std::env::consts::OS);
    card::<fixture::Probe>(());
}
