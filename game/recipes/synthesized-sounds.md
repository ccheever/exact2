# Recipe: synthesized sounds

Name each sound once in setup, as a `Synth`; play it from the tick by name. A
synthesized sound needs no file: it is rendered from these numbers on every host.

```rust
use exact_game::*;
use exact_game::audio::{self, AudioSource, Synth};
struct Reef;
impl Game for Reef {
    const ID: &'static str = "recipe-synth";
    type Args = ();
    fn actions() -> Actions { Actions::new().button("eat", &["KeyE"]).button("dive", &["Space"]) }
    fn setup(w: &mut World, _: &()) {
        w.sounds([
            // A bubble's pop: a short sine gliding up an octave and a half.
            ("pop", Synth::sine(420.).slide(18.).decay(0.08).seconds(0.12)),
            // A splash: filtered noise with a soft attack and a long tail.
            ("splash", Synth::noise().lowpass_hz(1800.).attack(0.01).decay(0.4).seconds(0.6).gain(0.4)),
            // A chime: two voices a fifth apart, each enveloped on its own.
            ("chime", Synth::sine(880.).decay(0.6).seconds(0.8)
                .layer(Synth::triangle(1320.).decay(0.4).seconds(0.6).gain(0.2))),
            // A motor: a looped low saw, sustained and filtered.
            ("motor", Synth::saw(55.).sustain(1.).lowpass_hz(400.).seconds(2.).gain(0.2).looped()),
        ]);
        w.spawn_named("sub", (Transform::default(), AudioSource::new("motor")));
    }
    fn tick(w: &mut World, input: &Input, _: &()) {
        if input.pressed("eat") {
            // A little pitch variety from the world's own random stream.
            let pitch = w.rand(0.9..1.1);
            w.play("pop").at("sub").pitch(pitch).start();
            w.play("chime").ui().start();
        }
        if input.pressed("dive") { w.play("splash").at("sub").start(); }
        audio::step(w);
    }
}
let mut sim = Sim::<Reef>::new(()).unwrap();
sim.tap("KeyE");
sim.run(20.);
let played = |sim: &Sim<Reef>, name: &str| sim.world().journal().iter()
    .filter(|e| e.line.contains(&format!("sfx {name}"))).count();
assert_eq!((played(&sim, "pop"), played(&sim, "chime"), played(&sim, "splash")), (1, 1, 0));
assert!(audio::state(sim.world()).contains("\"motor\""));
sim.tap("Space");
sim.run(20.);
assert_eq!(played(&sim, "splash"), 1);
assert_eq!(Synth::sine(880.).decay(0.6).seconds(0.8)
    .layer(Synth::triangle(1320.).seconds(0.9)).duration(), 0.9);
```

- `app.json` `"game": {"audio": true}` links the audio executor; without it the
  sounds save and journal, and nothing is heard.
- `seconds` includes `release`; `sustain` is 0 by default, so a sound decays to
  silence unless it sets one. `slide` is semitones per second; `hz` the start.
- A play does nothing until `.start()`. `at(entity)` places it in the world for
  spatial audio, `ui()` plays it without spatial attenuation. Voices and definitions save;
  call `audio::step(w)` after gameplay. Sampled sounds come from `art/` as
  `.sound` assets: [audio](../audio/README.md). This fragment is a doc-test.
