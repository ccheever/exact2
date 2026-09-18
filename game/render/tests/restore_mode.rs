use exact_game::{
    audio::{Sounds, Synth, Voices},
    Game, Input, Sim, World,
};
use exact_game_render::WorldSurface;
use exact_gpu::{Restore, Surface};
struct Tone;
impl Game for Tone {
    const ID: &'static str = "restore-mode";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.register_audio();
        w.resource_mut::<Sounds>().add("tone", Synth::sine(880.));
    }
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
exact_game_render::module!(Tone, audio);
#[test]
fn carry_overlays_fresh_definitions_open_preserves_saved_and_runtime_names() {
    let mut old = Sim::<Tone>::new(()).unwrap();
    old.world_mut()
        .resource_mut::<Sounds>()
        .add("tone", Synth::sine(220.));
    old.world_mut()
        .resource_mut::<Sounds>()
        .add("runtime", Synth::sine(330.));
    old.world().play("tone").start();
    let bytes = old.save().unwrap();
    for mode in [Restore::Open, Restore::Carry] {
        let mut surface = WorldSurface::<Tone, GameAudio>::default();
        surface.bind(&[], None).unwrap();
        surface.restore(&bytes, mode).unwrap();
        let world = surface.sim().unwrap().world();
        assert_eq!(
            world.resource::<Sounds>().0["tone"].hz,
            if mode == Restore::Carry { 880. } else { 220. }
        );
        assert_eq!(world.resource::<Sounds>().0["runtime"].hz, 330.);
        assert_eq!(world.resource::<Voices>().voices[0].synth.hz, 220.);
    }
}
