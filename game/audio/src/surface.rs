#[cfg(all(not(test), any(target_os = "macos", target_os = "ios")))]
use crate::AppleOutput as Device;
#[cfg(all(not(test), target_arch = "wasm32"))]
use crate::WebOutput as Device;
#[cfg(any(test, target_arch = "wasm32", target_os = "macos", target_os = "ios"))]
use crate::{Listener, Output};
use crate::{NullOutput, Player, Transport};
use exact_game::World;

/// Lazy audio owner for the renderer's dependency-free presentation hook.
/// Default construction and every seekable/headless frame open no device.
pub struct SurfacePlayer {
    null: Player<NullOutput>,
    #[cfg(any(test, target_arch = "wasm32", target_os = "macos", target_os = "ios"))]
    device: Option<Player<Device>>,
    #[cfg(any(test, target_arch = "wasm32", target_os = "macos", target_os = "ios"))]
    retry_frames: u32,
    #[cfg(any(test, target_arch = "wasm32", target_os = "macos", target_os = "ios"))]
    warned: bool,
    unlocked: bool,
    epoch: u64,
}
impl Default for SurfacePlayer {
    fn default() -> Self {
        Self {
            null: Player::new(NullOutput, 48000),
            #[cfg(any(test, target_arch = "wasm32", target_os = "macos", target_os = "ios"))]
            device: None,
            #[cfg(any(test, target_arch = "wasm32", target_os = "macos", target_os = "ios"))]
            retry_frames: 0,
            #[cfg(any(test, target_arch = "wasm32", target_os = "macos", target_os = "ios"))]
            warned: false,
            unlocked: false,
            epoch: 0,
        }
    }
}
impl SurfacePlayer {
    #[cfg(any(test, target_arch = "wasm32", target_os = "macos", target_os = "ios"))]
    fn ensure_device(&mut self) {
        if self.device.is_some() || self.retry_frames != 0 {
            return;
        }
        match Device::new() {
            Ok(output) => self.device = Some(Player::new(output, 48000)),
            Err(error) => {
                // Five seconds of frames at 60 Hz; input cannot defeat the bound.
                self.retry_frames = 300;
                if !self.warned {
                    eprintln!("game audio unavailable (retrying every 300 live frames): {error:?}");
                    self.warned = true;
                }
            }
        }
    }

    pub fn sync(&mut self, world: &World, generation: u64, playing: bool, seekable: bool) {
        #[cfg(any(test, target_arch = "wasm32", target_os = "macos", target_os = "ios"))]
        {
            if seekable {
                // Also close a previously human-owned device on entering agent mode.
                self.device = None;
                self.retry_frames = 0;
                self.unlocked = false;
            } else {
                self.retry_frames = self.retry_frames.saturating_sub(1);
                self.ensure_device();
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
        #[cfg(any(test, target_arch = "wasm32", target_os = "macos", target_os = "ios"))]
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
        #[cfg(any(test, target_arch = "wasm32", target_os = "macos", target_os = "ios"))]
        assert!(surface.device.is_none() && surface.retry_frames == 0);
    }
}

#[cfg(test)]
use test_device::Device;
#[cfg(test)]
mod test_device {
    use crate::Output;
    use std::{cell::Cell, sync::Arc};
    std::thread_local! {
        pub static ATTEMPTS: Cell<usize> = const { Cell::new(0) };
        pub static FAIL: Cell<bool> = const { Cell::new(false) };
        pub static UNLOCKS: Cell<usize> = const { Cell::new(0) };
    }
    pub struct Device;
    impl Device {
        pub fn new() -> Result<Self, &'static str> {
            ATTEMPTS.set(ATTEMPTS.get() + 1);
            if FAIL.get() {
                Err("test output unavailable")
            } else {
                Ok(Self)
            }
        }
    }
    impl Output for Device {
        fn unlock(&mut self) {
            UNLOCKS.set(UNLOCKS.get() + 1);
        }
        fn start_at(&mut self, _: u64, _: &Arc<[f32]>, _: u32, _: bool, _: usize, _: f32) {}
        fn set(&mut self, _: u64, _: f32, _: f32) {}
        fn stop(&mut self, _: u64) {}
    }
    #[test]
    fn failed_device_creation_retries_at_bounded_frame_intervals() {
        FAIL.set(true);
        ATTEMPTS.set(0);
        let mut surface = super::SurfacePlayer::default();
        let world = exact_game::World::new(60, 0);
        for _ in 0..601 {
            surface.sync(&world, 0, true, false);
        }
        assert_eq!(ATTEMPTS.get(), 3);
        FAIL.set(false);
        for _ in 0..300 {
            surface.sync(&world, 0, true, false);
        }
        assert_eq!(ATTEMPTS.get(), 4);
        assert!(surface.device.is_some());
        for _ in 0..600 {
            surface.sync(&world, 0, true, false);
        }
        assert_eq!(ATTEMPTS.get(), 4);
    }
}
