//! Size probe: cargo build -p exact-game-audio --example null_probe --profile web --target wasm32-unknown-unknown
#[cfg(target_arch = "wasm32")]
mod probe {
    use exact_game::{
        audio::{Sounds, Synth},
        World,
    };
    use exact_game_audio::{NullOutput, Player};
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen]
    pub struct Probe {
        player: Player<NullOutput>,
        world: World,
    }
    #[wasm_bindgen]
    impl Probe {
        #[wasm_bindgen(constructor)]
        pub fn new() -> Result<Probe, JsValue> {
            let mut world = World::new(60, 0);
            world.register_audio();
            world
                .resource_mut::<Sounds>()
                .add("chime", Synth::sine(880.0));
            world.play("chime").ui();
            Ok(Self {
                player: Player::new(NullOutput, 48000),
                world,
            })
        }
        pub fn unlock(&self) -> Result<(), JsValue> {
            Ok(())
        }
        pub fn frame(&mut self) {
            self.player.sync(&self.world, None, Default::default());
        }
    }
}
