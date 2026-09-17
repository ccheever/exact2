//! Size probe: cargo build -p exact-game-audio --example web_probe --profile web --target wasm32-unknown-unknown
#[cfg(target_arch = "wasm32")]
mod probe {
    use exact_game::{
        audio::{Sounds, Synth},
        World,
    };
    use exact_game_audio::{Player, Transport, WebOutput};
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen]
    pub struct Probe {
        player: Player<WebOutput>,
        world: World,
        transport: Transport,
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
                player: Player::new(WebOutput::new()?, 48000),
                world,
                transport: Transport::default(),
            })
        }
        pub async fn unlock(&mut self) -> Result<(), JsValue> {
            self.player.output.unlock(&mut self.transport).await
        }
        pub fn frame(&mut self) {
            self.player.sync(&self.world, None, self.transport);
        }
    }
}
