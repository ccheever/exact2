#[cfg(any(target_os = "macos", target_os = "ios"))]
use crate::AppleOutput as Device;
#[cfg(target_arch = "wasm32")]
use crate::WebOutput as Device;
#[cfg(any(target_arch = "wasm32", target_os = "macos", target_os = "ios"))]
use crate::{Listener, Output};
use crate::{NullOutput, Player, Transport};
use exact_game::World;

/// Lazy audio owner for the renderer's dependency-free presentation hook.
/// Default construction and every seekable/headless frame open no device.
pub struct SurfacePlayer {
    null: Player<NullOutput>,
    #[cfg(any(target_arch = "wasm32", target_os = "macos", target_os = "ios"))]
    device: Option<Player<Device>>,
    #[cfg(any(target_arch = "wasm32", target_os = "macos", target_os = "ios"))]
    attempted: bool,
    unlocked: bool,
    epoch: u64,
}
impl Default for SurfacePlayer {
    fn default() -> Self {
        Self {
            null: Player::new(NullOutput, 48000),
            #[cfg(any(target_arch = "wasm32", target_os = "macos", target_os = "ios"))]
            device: None,
            #[cfg(any(target_arch = "wasm32", target_os = "macos", target_os = "ios"))]
            attempted: false,
            unlocked: false,
            epoch: 0,
        }
    }
}
impl SurfacePlayer {
    pub fn sync(&mut self, world: &World, generation: u64, playing: bool, seekable: bool) {
        #[cfg(any(target_arch = "wasm32", target_os = "macos", target_os = "ios"))]
        {
            if seekable {
                // Also close a previously human-owned device on entering agent mode.
                self.device = None;
                self.attempted = false;
                self.unlocked = false;
            } else {
                if !self.attempted {
                    self.attempted = true;
                    match Device::new() {
                        Ok(output) => self.device = Some(Player::new(output, 48000)),
                        Err(error) => eprintln!("game audio unavailable: {error:?}"),
                    }
                }
                if let Some(player) = &mut self.device {
                    let ready = player.output.ready();
                    if ready && !self.unlocked {
                        self.epoch = self.epoch.wrapping_add(1);
                    }
                    self.unlocked = ready;
                    player.sync(
                        world,
                        Listener::from_world(world),
                        Transport {
                            generation: generation.wrapping_add(self.epoch),
                            playing,
                        },
                    );
                    return;
                }
            }
        }
        let _ = (seekable, self.unlocked);
        self.null.sync(
            world,
            None,
            Transport {
                generation: generation.wrapping_add(self.epoch),
                playing,
            },
        );
    }
    pub fn unlock(&mut self) {
        #[cfg(any(target_arch = "wasm32", target_os = "macos", target_os = "ios"))]
        if let Some(player) = &mut self.device {
            Output::unlock(&mut player.output);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seekable_gestures_never_construct_a_device_or_pcm() {
        let mut surface = SurfacePlayer::default();
        let mut world = World::new(60, 0);
        world.register_audio();
        world
            .resource_mut::<exact_game::audio::Sounds>()
            .add("tone", exact_game::audio::Synth::sine(440.));
        world.play("tone").start();
        surface.unlock();
        for generation in 0..10 {
            surface.sync(&world, generation, true, true);
            surface.unlock();
        }
        assert_eq!(surface.null.cached_sounds(), 0);
        #[cfg(any(target_arch = "wasm32", target_os = "macos", target_os = "ios"))]
        assert!(surface.device.is_none() && !surface.attempted);
    }
}
