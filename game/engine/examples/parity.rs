//! Native parity cards for the two browser proofs; no host or GPU needed.
#[path = "../../games/beacons/logic/src/lib.rs"]
mod beacons;
#[path = "../../games/greybox/logic/src/lib.rs"]
mod greybox;
use exact_game::{Clock, Game, InputEvent, Sim, Value};
fn card<G: Game>(values: &[Value]) {
    let mut s = Sim::<G>::new(values).unwrap();
    println!("{} setup=0x{:016x}", G::ID, s.world().hash());
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
    s.advance(0.0, Clock::Seekable);
    s.input(InputEvent::Key {
        code: "KeyW".into(),
        down: true,
        at_ms: 0.0,
    });
    s.advance(1500.0, Clock::Seekable);
    println!("{} W1500=0x{:016x}", G::ID, s.world().hash());
}
fn main() {
    println!("{} {}", std::env::consts::ARCH, std::env::consts::OS);
    card::<greybox::Greybox>(&[Value::Number(7.0), Value::Bool(false)]);
    card::<beacons::Beacons>(&[Value::Number(7.0), Value::Number(0.0), Value::Bool(false)]);
}
